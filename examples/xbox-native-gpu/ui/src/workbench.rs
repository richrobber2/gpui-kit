//! Application composition; native compute results are retained independently of presentation.
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Selectable, Sizable, StyledExt,
        button::{Button, ButtonGroup, ButtonVariants},
        description_list::DescriptionList,
        progress::Progress,
        tab::{Tab, TabBar},
        tag::Tag,
    },
    *,
};

pub(super) type RequestJob = extern "C" fn(u32);
const SIZES: [u32; 3] = [64, 128, 256];
const HISTORY_LIMIT: usize = 5;

pub(super) struct Sample {
    pub(super) n: u32,
    pub(super) cpu: f64,
    pub(super) gpu: f64,
    pub(super) max_error: f64,
    pub(super) memory: u64,
    pub(super) mismatches: u32,
    pub(super) saved: bool,
}
pub(super) struct Lab {
    pub(super) focus: FocusHandle,
    selected: usize,
    busy: bool,
    error: Option<String>,
    history: Vec<Sample>,
    history_visible: bool,
    request: RequestJob,
}
impl Lab {
    pub(super) fn new(request: RequestJob, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            selected: 2,
            busy: false,
            error: None,
            history: Vec::new(),
            history_visible: false,
            request,
        }
    }
    fn run(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.selected = ix;
        self.busy = true;
        self.error = None;
        self.history_visible = false;
        (self.request)(SIZES[ix]);
        cx.notify();
    }
    pub(super) fn complete(&mut self, sample: Sample, cx: &mut Context<Self>) {
        self.busy = false;
        self.error = None;
        self.history.insert(0, sample);
        self.history.truncate(HISTORY_LIMIT);
        cx.notify();
    }
    pub(super) fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.error = Some(message);
        cx.notify();
    }
    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "left" if !self.busy => self.selected = (self.selected + 2) % 3,
            "right" if !self.busy => self.selected = (self.selected + 1) % 3,
            "up" => self.history_visible = false,
            "down" => self.history_visible = true,
            "tab" => self.history_visible = !self.history_visible,
            "enter" => self.run(self.selected, cx),
            "x" | "space" => self.run(1, cx),
            "y" => self.run(0, cx),
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }
    fn results(&self, cx: &App) -> AnyElement {
        let Some(sample) = self.history.first() else {
            return div()
                .py_6()
                .child("Run a workload to inspect timings and numerical verification.")
                .into_any_element();
        };
        let status = if sample.mismatches == 0 {
            Tag::success().outline().child("Verified")
        } else {
            Tag::danger().outline().child("Mismatch")
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(status.large())
                    .child(format!(
                        "{} × {} · {} outputs checked",
                        sample.n,
                        sample.n,
                        sample.n * sample.n
                    )),
            )
            .child(
                DescriptionList::vertical()
                    .columns(3)
                    .large()
                    .item("CPU reference", format!("{:.3} ms", sample.cpu), 1)
                    .item("GPU total", format!("{:.3} ms", sample.gpu), 1)
                    .item(
                        "Memory budget",
                        format!("{} MiB", sample.memory / (1024 * 1024)),
                        1,
                    ),
            )
            .child(
                DescriptionList::horizontal()
                    .columns(2)
                    .label_width(rems(8.0))
                    .large()
                    .item("Mismatches", sample.mismatches.to_string(), 1)
                    .item("Max error", format!("{:.6}", sample.max_error), 1),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if sample.saved {
                        "Report saved to LocalState/native-gpu-result.json"
                    } else {
                        "Report could not be saved; results remain visible in this session."
                    }),
            )
            .into_any_element()
    }
    fn recent(&self) -> AnyElement {
        if self.history.is_empty() {
            return div()
                .py_6()
                .child("No runs in this session. Press A to run the selected workload.")
                .into_any_element();
        }
        let mut rows = DescriptionList::horizontal()
            .columns(1)
            .label_width(rems(8.0))
            .large();
        for sample in &self.history {
            rows = rows.item(
                format!("{} × {}", sample.n, sample.n),
                format!(
                    "CPU {:.3} ms     GPU {:.3} ms     {}",
                    sample.cpu,
                    sample.gpu,
                    if sample.mismatches == 0 {
                        "Verified"
                    } else {
                        "Mismatch"
                    }
                ),
                1,
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Latest first · up to five runs in this session")
            .child(rows)
            .into_any_element()
    }
}
impl Render for Lab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let n = SIZES[self.selected];
        let controls = ButtonGroup::new("workload-size")
            .large()
            .disabled(self.busy)
            .children(SIZES.into_iter().enumerate().map(|(ix, n)| {
                Button::new(("matrix", n as usize))
                    .label(format!("{n} × {n}"))
                    .selected(ix == self.selected)
            }))
            .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                if !this.busy {
                    if let Some(ix) = selected.first() {
                        this.selected = *ix;
                        cx.notify();
                    }
                }
            }));
        div().size_full().flex().flex_col().p_8().gap_5()
            .font_family("Segoe UI").text_base().bg(theme.background).text_color(theme.foreground)
            .track_focus(&self.focus).on_key_down(cx.listener(Self::key))
            .child(div().flex().flex_col().gap_1()
                .child(div().text_2xl().font_semibold().child("Xbox GPU workbench"))
                .child(div().text_sm().text_color(theme.muted_foreground)
                    .child("Matrix multiplication · native D3D11 compute · GPUI / DirectX 12 interface")))
            .child(div().flex().items_end().justify_between().gap_4()
                .child(div().flex().flex_col().gap_2().child("Workload").child(controls))
                .child(Button::new("run-workload").primary().large().disabled(self.busy)
                    .label(if self.busy { "Computing…" } else { "Run workload" })
                    .on_click(cx.listener(|this, _, _, cx| this.run(this.selected, cx)))))
            .child(div().text_sm().text_color(theme.muted_foreground)
                .child(format!("Selected: {n} × {n} · {} outputs · checked against a scalar CPU reference", n*n)))
            .child(TabBar::new("results-navigation").underline().large()
                .child(Tab::new().label("Latest result"))
                .child(Tab::new().label("Recent runs"))
                .selected_index(usize::from(self.history_visible))
                .on_click(cx.listener(|this, ix, _, cx| { this.history_visible = *ix == 1; cx.notify(); })))
            .child(div().flex_1().min_h_0().flex().flex_col().gap_3()
                .when(self.busy, |this| this.child(Progress::new("compute-progress").loading(true)
                    .accessibility_label("Computing and verifying matrix outputs")))
                .when_some(self.error.as_ref(), |this, message| this.child(div().text_color(theme.danger)
                    .child(format!("Workload stopped: {message}"))))
                .child(if self.history_visible { self.recent() } else { self.results(cx) }))
            .child(div().flex().flex_col().gap_2().border_t_1().border_color(theme.border).pt_3()
                .child(div().text_sm().child("← / → Size    A / Enter Run    ↑ Result    ↓ Recent runs    X 128    Y 64"))
                .child(div().text_xs().text_color(theme.muted_foreground)
                    .child("Single samples. GPU total includes allocation, transfers, dispatch and readback; shader initialization is excluded.")))
    }
}
