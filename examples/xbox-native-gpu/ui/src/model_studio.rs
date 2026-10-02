//! GPUI composition over the privately staged native Anima engine.
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Selectable, Sizable, StyledExt,
        button::{Button, ButtonGroup, ButtonVariants},
        input::{Input, InputEvent, InputState},
        progress::Progress,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
const RUNTIME: &str = r"D:\DevelopmentFiles\AnimaModels\GpuiRuntime";
const MODELS: [(&str, &str, &str); 3] = [
    (
        "anima-preview3",
        "Anima Preview 3",
        "anima-preview3-base.safetensors",
    ),
    (
        "miaomiao-anima-1.6",
        "MiaoMiao Anima 1.6",
        "miaomiaoHarem_anima16.safetensors",
    ),
    (
        "miaomiao-anima-2.9b-beta1.1",
        "MiaoMiao Anima 2.9B Beta 1.1",
        "miaomiaoHarem_29BBETA11.safetensors",
    ),
];
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    model: String,
    prompt: String,
    negative_prompt: String,
    seed: u64,
    width: u32,
    height: u32,
    steps: u32,
    cfg_scale: f64,
    gpu: bool,
    preview_every: u32,
}
impl Request {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(MODELS.iter().any(|m| m.0 == self.model), "Unknown model");
        anyhow::ensure!(
            !self.prompt.trim().is_empty()
                && self.prompt.len() <= 1024
                && self.negative_prompt.len() <= 1024,
            "Prompt must contain 1–1024 bytes"
        );
        anyhow::ensure!(
            self.width >= 256
                && self.height >= 256
                && self.width <= 1024
                && self.height <= 1024
                && self.width % 16 == 0
                && self.height % 16 == 0,
            "Image dimensions must be multiples of 16 between 256 and 1024"
        );
        anyhow::ensure!(
            self.steps >= 1
                && self.steps <= 24
                && self.gpu
                && self.preview_every == 1
                && self.cfg_scale.is_finite()
                && (1.0..=12.0).contains(&self.cfg_scale),
            "Invalid sampling settings; hardware GPU and per-step previews are required"
        );
        Ok(())
    }
}
struct Job {
    child: Child,
    folder: PathBuf,
    model: String,
    steps: u32,
    width: u32,
    height: u32,
    preview_step: u32,
}
impl Drop for Job {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
pub(super) struct Studio {
    pub(super) focus: FocusHandle,
    prompt: Entity<InputState>,
    _input_events: Subscription,
    root: Option<PathBuf>,
    selected: usize,
    steps: u32,
    width: u32,
    height: u32,
    seed: u64,
    cfg: f64,
    available: [bool; 3],
    job: Option<Job>,
    image: Option<Arc<RenderImage>>,
    image_label: String,
    status: String,
    progress: f32,
    last_poll: Instant,
}
impl Studio {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prompt = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                "A red apple on a wooden table, soft natural light, detailed illustration",
            )
        });
        let events = cx.subscribe_in(&prompt, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.generate(cx);
            }
        });
        Self {
            focus: cx.focus_handle(),
            prompt,
            _input_events: events,
            root: None,
            selected: 0,
            steps: 4,
            width: 256,
            height: 256,
            seed: 42,
            cfg: 4.0,
            available: [false; 3],
            job: None,
            image: None,
            image_label: "No preview yet".into(),
            status: "Checking local models…".into(),
            progress: 0.0,
            last_poll: Instant::now(),
        }
    }
    pub(super) fn storage(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(root);
        self.scan();
        self.status = if self.available.iter().any(|a| *a) {
            "Choose a downloaded model, then generate a preview.".into()
        } else {
            "Downloaded checkpoints are not accessible to this app.".into()
        };
        self.publish_status();
        cx.notify();
    }
    fn scan(&mut self) {
        let common = [
            "anima-engine.exe",
            "qwen-tokenizer.json",
            "t5-tokenizer.json",
            "qwen_3_06b_base.safetensors",
            "qwen_image_vae.safetensors",
        ]
        .iter()
        .all(|n| fs::metadata(Path::new(RUNTIME).join(n)).is_ok());
        for (ix, m) in MODELS.iter().enumerate() {
            self.available[ix] = common
                && fs::metadata(Path::new(r"D:\DevelopmentFiles\AnimaModels").join(m.2))
                    .map(|v| v.len() > 0)
                    .unwrap_or(false);
        }
    }
    fn generate(&mut self, cx: &mut Context<Self>) {
        if self.job.is_some() {
            return;
        }
        let request = Request {
            model: MODELS[self.selected].0.into(),
            prompt: self.prompt.read(cx).value().to_string(),
            negative_prompt: "blurry, low quality".into(),
            seed: self.seed,
            width: self.width,
            height: self.height,
            steps: self.steps,
            cfg_scale: self.cfg,
            gpu: true,
            preview_every: 1,
        };
        if let Err(error) = self.start(request) {
            self.status = format!("Could not start: {error:#}");
            self.publish_status();
        }
        cx.notify();
    }
    fn start(&mut self, request: Request) -> anyhow::Result<()> {
        anyhow::ensure!(self.job.is_none(), "An image job is already running");
        request.validate()?;
        let ix = MODELS.iter().position(|m| m.0 == request.model).unwrap();
        self.scan();
        anyhow::ensure!(
            self.available[ix],
            "The selected checkpoint or shared runtime is unavailable"
        );
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Storage is not initialized"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let folder = root.join(format!("gpui-image-{id}"));
        fs::create_dir(&folder)?;
        let request_path = folder.join("request.json");
        fs::write(&request_path, serde_json::to_vec_pretty(&request)?)?;
        let output = folder.join("output");
        let mut command = Command::new(Path::new(RUNTIME).join("anima-engine.exe"));
        command
            .arg("--generate")
            .arg(RUNTIME)
            .arg(&request_path)
            .arg(&output)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(fs::File::create(folder.join("stdout.txt"))?)
            .stderr(fs::File::create(folder.join("stderr.txt"))?);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW on Xbox.
        }
        let mut child = command.spawn()?;
        // Xbox's NUL stdin fails; an owned pipe delivers EOF without a NUL open.
        drop(child.stdin.take());
        self.selected = ix;
        self.steps = request.steps;
        self.width = request.width;
        self.height = request.height;
        self.seed = request.seed;
        self.cfg = request.cfg_scale;
        self.progress = 0.0;
        self.status = format!("Starting {} on the Xbox GPU…", MODELS[ix].1);
        self.job = Some(Job {
            child,
            folder: output,
            model: request.model,
            steps: request.steps,
            width: request.width,
            height: request.height,
            preview_step: 0,
        });
        self.publish_status();
        Ok(())
    }
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = self.job.as_mut() {
            match job.child.kill() {
                Ok(()) => {
                    let _ = job.child.wait();
                    self.job = None;
                    self.status =
                        "Generation cancelled. Existing previews and output files are retained."
                            .into();
                }
                Err(e) => self.status = format!("Could not cancel: {e}"),
            }
            self.publish_status();
            cx.notify();
        }
    }
    fn publish_status(&self) {
        if let Some(root) = &self.root {
            let s = serde_json::json!({"version":1,"available":self.available,"selected":MODELS[self.selected].0,"busy":self.job.is_some(),"status":self.status,"progress":self.progress,"preview":self.image_label});
            let _ = fs::write(root.join("studio-status.json"), s.to_string());
        }
    }
    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        if self.last_poll.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_poll = Instant::now();
        if self.job.is_none() {
            if let Some(root) = &self.root {
                let path = root.join("studio-start.request");
                if path.is_file() {
                    // Explicit one-shot development request; never inspect Workbench presets.
                    let consumed =
                        root.join(format!("studio-start-{}.consumed", uuid::Uuid::new_v4()));
                    match fs::rename(&path, &consumed).and_then(|_| fs::read(&consumed)) {
                        Ok(bytes) => {
                            let outcome = (|| {
                                anyhow::ensure!(bytes.len() <= 8192, "Request too large");
                                self.start(serde_json::from_slice(&bytes)?)
                            })();
                            if let Err(e) = outcome {
                                self.status = format!("Request rejected: {e:#}");
                            }
                            self.publish_status();
                            cx.notify();
                        }
                        Err(e) => {
                            self.status = format!("Could not consume request: {e}");
                            cx.notify();
                        }
                    }
                }
            }
            return;
        }
        let job = self.job.as_mut().unwrap();
        let mut changed = false;
        if let Ok(bytes) = fs::read(job.folder.join("progress.json")) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let step = value["step"].as_u64().unwrap_or(0).min(job.steps as u64) as u32;
                let total = value["total"].as_u64().unwrap_or(job.steps as u64).max(1);
                let message = format!("Sampling {step} / {total} · approximate latent preview");
                if self.status != message {
                    self.status = message;
                    self.progress = step as f32 / total as f32 * 100.0;
                    changed = true;
                }
                let preview_step = value["preview_step"]
                    .as_u64()
                    .unwrap_or(0)
                    .min(job.steps as u64) as u32;
                if preview_step > job.preview_step {
                    if let Ok(bytes) =
                        fs::read(job.folder.join(format!("preview-{preview_step:04}.rgb")))
                    {
                        if let Ok(image) = decode_preview(&bytes) {
                            if let Some(old) = self.image.replace(image) {
                                cx.drop_image(old, None);
                            }
                            self.image_label = format!(
                                "{} · step {preview_step} · approximate preview",
                                job.model
                            );
                            job.preview_step = preview_step;
                            changed = true;
                        }
                    }
                }
            }
        } else if let Ok(stage) = fs::read_to_string(job.folder.join("stage.txt")) {
            let stage = stage.chars().take(256).collect::<String>();
            if self.status != stage {
                self.status = stage;
                changed = true;
            }
        }
        let exit = job.child.try_wait();
        match exit {
            Ok(Some(exit)) => {
                let job = self.job.take().unwrap();
                let finish = (|| -> anyhow::Result<Arc<RenderImage>> {
                    anyhow::ensure!(
                        exit.success(),
                        "Native engine exited with {exit}; diagnostics: {}",
                        job.folder.parent().unwrap().join("stderr.txt").display()
                    );
                    let report: serde_json::Value = serde_json::from_slice(&fs::read(
                        job.folder.join("generation-result.json"),
                    )?)?;
                    anyhow::ensure!(
                        report["compute_backend"] == "D3D11 hardware"
                            && report["gpu_linear"] == true,
                        "Generation did not report hardware GPU execution"
                    );
                    let used: serde_json::Value =
                        serde_json::from_slice(&fs::read(job.folder.join("request-used.json"))?)?;
                    anyhow::ensure!(
                        used["model"] == job.model && used["steps"] == job.steps,
                        "Engine reported a different model"
                    );
                    let bytes = fs::read(job.folder.join("anima-output.png"))?;
                    anyhow::ensure!(
                        bytes.len() <= 16 * 1024 * 1024
                            && bytes.len() > 24
                            && bytes[..8] == *b"\x89PNG\r\n\x1a\n",
                        "Invalid output PNG"
                    );
                    let w = u32::from_be_bytes(bytes[16..20].try_into()?);
                    let h = u32::from_be_bytes(bytes[20..24].try_into()?);
                    anyhow::ensure!(
                        w == job.width && h == job.height && w <= 1024 && h <= 1024,
                        "Invalid PNG dimensions"
                    );
                    let mut rgba = image::load_from_memory(&bytes)?.to_rgba8();
                    for pixel in rgba.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
                })();
                match finish {
                    Ok(image) => {
                        if let Some(old) = self.image.replace(image) {
                            cx.drop_image(old, None);
                        }
                        self.image_label = format!("{} · final decoded image", job.model);
                        self.status = "Generation complete · native Xbox GPU".into();
                        self.progress = 100.0;
                    }
                    Err(e) => self.status = format!("Generation failed: {e:#}"),
                }
                changed = true;
            }
            Err(e) => {
                self.status = format!("Could not check native process: {e}");
                changed = true;
            }
            _ => {}
        }
        if changed {
            self.publish_status();
            cx.notify();
        }
    }
    pub(super) fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.job.is_some() {
            self.cancel(cx);
        }
        self.focus.focus(window, cx);
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            if self.job.is_some() {
                self.cancel(cx);
            }
            self.focus.focus(window, cx);
            cx.stop_propagation();
            return;
        }
        if !self.focus.is_focused(window) {
            return;
        }
        match event.keystroke.key.as_str() {
            "left" if self.job.is_none() => self.selected = (self.selected + 2) % 3,
            "right" if self.job.is_none() => self.selected = (self.selected + 1) % 3,
            "enter" => self.generate(cx),
            "x" => self.prompt.read(cx).focus_handle(cx).focus(window, cx),
            "y" if self.job.is_none() => self.steps = if self.steps == 4 { 12 } else { 4 },
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }
}
fn decode_preview(bytes: &[u8]) -> anyhow::Result<Arc<RenderImage>> {
    let (width, height, rgba) = crate::preview::decode(bytes).map_err(anyhow::Error::msg)?;
    let buffer = image::RgbaImage::from_raw(width, height, rgba).unwrap();
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}
impl Render for Studio {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.job.is_some();
        let theme = cx.theme();
        let models = ButtonGroup::new("downloaded-models")
            .layout(Axis::Vertical)
            .large()
            .disabled(busy)
            .children(
                MODELS
                    .iter()
                    .enumerate()
                    .map(|(ix, m)| Button::new(m.0).label(m.1).selected(ix == self.selected)),
            )
            .on_click(cx.listener(|this, indices: &Vec<usize>, _, cx| {
                if this.job.is_none() {
                    if let Some(ix) = indices.first() {
                        this.selected = *ix;
                        cx.notify();
                    }
                }
            }));
        div().size_full().flex().flex_col().p_8().gap_4().bg(theme.background).text_color(theme.foreground)
            .font_family("Segoe UI").track_focus(&self.focus).on_key_down(cx.listener(Self::key))
            .child(div().text_2xl().font_semibold().child("Local image models"))
            .child(div().flex_1().min_h_0().flex().items_stretch().gap_8()
                .child(div().w(rems(23.0)).flex_shrink_0().flex().flex_col().gap_4()
                    .child("Downloaded checkpoints").child(models)
                    .child(div().text_sm().text_color(theme.muted_foreground).child(if self.available[self.selected]{"Checkpoint and shared runtime available"}else{"Selected checkpoint or runtime unavailable"}))
                    .child(div().flex().items_center().gap_4()
                        .child("Precision")
                        .child(ButtonGroup::new("compute-precision").large().children([
                            Button::new("precision-fp32").label("FP32").selected(true).disabled(true),
                            Button::new("precision-fp16").label("FP16").disabled(true),
                            Button::new("precision-bf16").label("BF16").disabled(true),
                        ])))
                    .child(div().text_sm().text_color(theme.muted_foreground)
                        .child("FP32 active; FP16/BF16 unavailable in this engine"))
                    .child("Prompt").child(Input::new(&self.prompt).large().disabled(busy))
                    .child(div().text_sm().text_color(theme.muted_foreground).child(format!("{} × {} · {} steps · seed {} · CFG {}",self.width,self.height,self.steps,self.seed,self.cfg)))
                    .child(Button::new("sampling-steps").label(format!("Sampling steps: {}",self.steps)).disabled(busy).on_click(cx.listener(|this,_,_,cx|{this.steps=if this.steps==4{12}else{4};cx.notify();})))
                    .child(div().flex().gap_3()
                        .child(Button::new("generate-image").primary().large().label("Generate").disabled(busy || !self.available[self.selected])
                            .on_click(cx.listener(|this,_,_,cx|this.generate(cx))))
                        .child(Button::new("cancel-image").large().label("Cancel").disabled(!busy).on_click(cx.listener(|this,_,_,cx|this.cancel(cx)))))
                    .child(div().text_sm().child(self.status.clone()))
                    .when(busy,|this|this.child(Progress::new("image-progress").loading(self.progress==0.0).value(self.progress).accessibility_label("Image sampling progress"))))
                .child(div().flex_1().min_w_0().flex().flex_col().gap_3()
                    .child("Live preview")
                    .child(div().flex_1().min_h_0().flex().items_center().justify_center().border_1().border_color(theme.border)
                        .when_some(self.image.clone(),|this,image|this.child(img(ImageSource::Render(image)).size_full().object_fit(ObjectFit::Contain)))
                        .when(self.image.is_none(),|this|this.child("Generate to see sampling previews here.")))
                    .child(div().text_sm().text_color(theme.muted_foreground).child(self.image_label.clone()))))
            .child(div().text_sm().border_t_1().border_color(theme.border).pt_3().child("F6 / D-pad Move focus · Enter / A Activate · LB / RB Switch tools · Esc / B Cancel"))
    }
}
