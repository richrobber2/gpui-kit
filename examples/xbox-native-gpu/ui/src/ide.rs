//! Retained GPUI editor buffers backed by an app-owned workspace and native rustc.
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Selectable,
        button::{Button, ButtonVariants},
        input::{Editor, EditorState, Input, InputEvent, InputState},
    },
    *,
};
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const COMPILER: &str = r"D:\DevelopmentFiles\GpuiRuntime\compiler\bin\rustc.exe";
struct Buffer {
    name: String,
    editor: Entity<EditorState>,
    saved: String,
    dirty: bool,
}
struct Check {
    child: Child,
    directory: PathBuf,
    started: Instant,
    file: String,
}
impl Drop for Check {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
pub(super) struct Ide {
    pub(super) focus: FocusHandle,
    root: Option<PathBuf>,
    buffers: Vec<Buffer>,
    selected: usize,
    filter: Entity<InputState>,
    diagnostics: String,
    check: Option<Check>,
    files_scroll: ScrollHandle,
    diagnostics_scroll: ScrollHandle,
    _filter_events: Subscription,
    buffer_events: Vec<Subscription>,
}
impl Ide {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Find a file · Ctrl P"));
        let filter_events =
            cx.subscribe_in(&filter, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let query = this.filter.read(cx).value().to_lowercase();
                    if let Some(ix) = this
                        .buffers
                        .iter()
                        .position(|b| b.name.to_lowercase().contains(&query))
                    {
                        this.select(ix, window, cx);
                    }
                }
                InputEvent::Change => cx.notify(),
                _ => {}
            });
        Self {
            focus: cx.focus_handle(),
            root: None,
            buffers: vec![],
            selected: 0,
            filter,
            diagnostics:
                "Open a Rust file, edit, then check with F5. Checks use the Xbox's native compiler."
                    .into(),
            check: None,
            files_scroll: ScrollHandle::new(),
            diagnostics_scroll: ScrollHandle::new(),
            _filter_events: filter_events,
            buffer_events: Vec::new(),
        }
    }
    pub(super) fn storage(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let shared = PathBuf::from(r"D:\DevelopmentFiles\GpuiWorkspace");
        let root = if shared.is_dir() {
            shared
        } else {
            root.join("workspace")
        };
        let result = (|| -> anyhow::Result<()> {
            fs::create_dir_all(&root)?;
            let main = root.join("main.rs");
            if !main.exists() {
                fs::write(
                    &main,
                    "fn main() {\n    println!(\"Hello from GPUI on Xbox!\");\n}\n",
                )?;
            }
            self.root = Some(root.clone());
            fn scan(
                root: &std::path::Path,
                base: &std::path::Path,
                files: &mut Vec<PathBuf>,
                depth: usize,
            ) -> std::io::Result<()> {
                if depth > 8 || files.len() >= 64 {
                    return Ok(());
                }
                let mut entries: Vec<_> = fs::read_dir(root)?.filter_map(Result::ok).collect();
                entries.sort_by_key(|e| e.file_name());
                for e in entries {
                    if files.len() >= 64 {
                        break;
                    }
                    let kind = e.file_type()?;
                    if kind.is_dir() && e.file_name() != "target" && e.file_name() != ".git" {
                        scan(&e.path(), base, files, depth + 1)?;
                    } else if kind.is_file()
                        && e.path().extension().is_some_and(|e| e == "rs")
                        && e.metadata()?.len() <= 1024 * 1024
                    {
                        files.push(e.path().strip_prefix(base).unwrap().to_owned());
                    }
                }
                Ok(())
            }
            let mut files = Vec::new();
            scan(&root, &root, &mut files, 0)?;
            for file in files.into_iter().take(64) {
                let name = file.to_string_lossy().to_string();
                let saved = fs::read_to_string(root.join(&file))?;
                let value = saved.clone();
                let editor = cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language("rust")
                        .default_value(value)
                });
                let id = editor.entity_id();
                let events = cx.subscribe(&editor, move |this, editor, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        if let Some(buffer) =
                            this.buffers.iter_mut().find(|b| b.editor.entity_id() == id)
                        {
                            buffer.dirty = editor.read(cx).value().as_ref() != buffer.saved;
                        }
                        cx.notify();
                    }
                });
                self.buffer_events.push(events);
                self.buffers.push(Buffer {
                    name,
                    editor,
                    saved,
                    dirty: false,
                });
            }
            self.selected = self
                .buffers
                .iter()
                .position(|b| b.name == "main.rs")
                .unwrap_or(0);
            Ok(())
        })();
        if let Err(error) = result {
            self.diagnostics = format!("Workspace: {error}");
        }
        cx.notify();
    }
    pub(super) fn focus_files(&self, window: &mut Window, cx: &mut App) {
        self.filter
            .read(cx)
            .focus_handle(cx)
            .clone()
            .focus(window, cx);
    }
    pub(super) fn scroll(
        &mut self,
        direction: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .buffers
            .get(self.selected)
            .is_some_and(|b| b.editor.read(cx).focus_handle(cx).is_focused(window))
        {
            return false;
        }
        let handle = if self.filter.read(cx).focus_handle(cx).is_focused(window) {
            &self.files_scroll
        } else {
            &self.diagnostics_scroll
        };
        let mut offset = handle.offset();
        offset.y = (offset.y - px(direction as f32 * 180.)).min(px(0.));
        handle.set_offset(offset);
        cx.notify();
        true
    }
    pub(super) fn status(&self, _cx: &App) -> serde_json::Value {
        serde_json::json!({"files":self.buffers.iter().map(|b|serde_json::json!({"name":b.name,"dirty":b.dirty})).collect::<Vec<_>>(),"selected":self.buffers.get(self.selected).map(|b|&b.name),"checking":self.check.is_some(),"diagnostics":self.diagnostics})
    }
    pub(super) fn focus_editor(&self, window: &mut Window, cx: &mut App) {
        if let Some(buffer) = self.buffers.get(self.selected) {
            buffer
                .editor
                .read(cx)
                .focus_handle(cx)
                .clone()
                .focus(window, cx);
        }
    }
    fn select(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = ix;
        self.focus_editor(window, cx);
        cx.notify();
    }
    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(buffer) = self.buffers.get_mut(self.selected) else {
            return;
        };
        let Some(root) = &self.root else { return };
        let value = buffer.editor.read(cx).value().to_string();
        let result = (|| -> anyhow::Result<()> {
            let destination = root.join(&buffer.name);
            // Keep another writer's change instead of silently overwriting it.
            anyhow::ensure!(
                fs::read_to_string(&destination)? == buffer.saved,
                "File changed on disk; save stopped to preserve both versions"
            );
            let temporary = root.join(format!(".save-{}.tmp", uuid::Uuid::new_v4()));
            fs::write(&temporary, &value)?;
            // Windows rename does not replace a destination. Preserve the previous version as a backup.
            let backup = destination.with_file_name(format!(
                ".backup-{}-{}",
                uuid::Uuid::new_v4(),
                destination.file_name().unwrap().to_string_lossy()
            ));
            fs::rename(&destination, &backup)?;
            if let Err(error) = fs::rename(&temporary, &destination) {
                let _ = fs::rename(&backup, &destination);
                return Err(error.into());
            }
            buffer.saved = value;
            buffer.dirty = false;
            Ok(())
        })();
        self.diagnostics = match result {
            Ok(()) => format!(
                "Saved {}. Previous version kept in the workspace.",
                buffer.name
            ),
            Err(e) => format!("Save failed: {e}"),
        };
        cx.notify();
    }
    pub(super) fn check(&mut self, cx: &mut Context<Self>) {
        if self.check.is_some() {
            return;
        }
        let Some(buffer) = self.buffers.get(self.selected) else {
            return;
        };
        let Some(root) = &self.root else { return };
        let result = (|| -> anyhow::Result<Check> {
            anyhow::ensure!(
                PathBuf::from(COMPILER).is_file(),
                "Native compiler not staged at {COMPILER}"
            );
            let directory = root
                .parent()
                .unwrap()
                .join(format!("rust-check-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&directory)?;
            // Compile a snapshot, including other open buffers, without saving or mutating editor history.
            for buffer in &self.buffers {
                let destination = directory.join(&buffer.name);
                fs::create_dir_all(destination.parent().unwrap())?;
                fs::write(
                    directory.join(&buffer.name),
                    buffer.editor.read(cx).value().as_bytes(),
                )?;
            }
            let mut command = Command::new(COMPILER);
            command
                .current_dir(&directory)
                .arg(&buffer.name)
                .args([
                    "--edition=2024",
                    "--emit=metadata",
                    "--crate-name=gpui_workspace",
                    "--error-format=human",
                    "--color=never",
                ])
                .arg("-o")
                .arg(directory.join("check.rmeta"))
                .stdin(Stdio::piped())
                .stdout(fs::File::create(directory.join("stdout.txt"))?)
                .stderr(fs::File::create(directory.join("stderr.txt"))?);
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let mut child = command.spawn()?;
            drop(child.stdin.take());
            Ok(Check {
                child,
                directory,
                started: Instant::now(),
                file: buffer.name.clone(),
            })
        })();
        match result {
            Ok(job) => {
                self.diagnostics = format!("Checking {} on Xbox…", job.file);
                self.check = Some(job);
            }
            Err(e) => self.diagnostics = format!("Check failed: {e}"),
        };
        cx.notify();
    }
    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(mut job) = self.check.take() {
            let _ = job.child.kill();
            let _ = job.child.wait();
            self.diagnostics = "Native check cancelled.".into();
            cx.notify();
        }
    }
    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.check.as_mut() else {
            return;
        };
        if job.started.elapsed() > Duration::from_secs(90) {
            self.cancel(cx);
            return;
        }
        match job.child.try_wait() {
            Ok(None) => {}
            result => {
                let job = self.check.take().unwrap();
                let stderr =
                    fs::read_to_string(job.directory.join("stderr.txt")).unwrap_or_default();
                self.diagnostics = match result {
                    Ok(Some(exit)) => format!(
                        "{}: {}\n{}",
                        job.file,
                        if exit.success() {
                            "check passed"
                        } else {
                            "check failed"
                        },
                        stderr.chars().take(16000).collect::<String>()
                    ),
                    Err(e) => format!("Check failed: {e}"),
                    _ => unreachable!(),
                };
                cx.notify();
            }
        }
    }
}
impl Render for Ide {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.filter.read(cx).value().to_lowercase();
        div().size_full().p_4().flex().flex_col().gap_3().track_focus(&self.focus)
            .child(div().flex().items_center().gap_3().child(div().flex_1().text_xl().child("Rust workspace"))
                .child(Button::new("ide-save").label("Save · Ctrl S").on_click(cx.listener(|this,_,_,cx|this.save(cx))))
                .child(Button::new("ide-check").primary().label("Check · F5").disabled(self.check.is_some()).on_click(cx.listener(|this,_,_,cx|this.check(cx))))
                .child(Button::new("ide-cancel").label("Cancel").disabled(self.check.is_none()).on_click(cx.listener(|this,_,_,cx|this.cancel(cx)))))
            .child(div().flex_1().min_h_0().flex().gap_3()
                .child(div().id("ide-files").w(rems(12.)).overflow_y_scroll().track_scroll(&self.files_scroll).flex().flex_col().gap_2().child("Files")
                    .child(Input::new(&self.filter))
                    .children(self.buffers.iter().enumerate().filter(|(_,b)|b.name.to_lowercase().contains(&query)).map(|(ix,buffer)|{
                        let dirty=buffer.dirty;
                        Button::new(SharedString::from(format!("ide-file:{}",buffer.name))).label(format!("{}{}",buffer.name,if dirty{" •"}else{""})).selected(ix==self.selected)
                            .on_click(cx.listener(move|this,_,window,cx|this.select(ix,window,cx)))
                    })))
                .child(div().flex_1().min_w_0().children(self.buffers.get(self.selected).map(|buffer|Editor::new(&buffer.editor).h(relative(1.)).aria_label(format!("Source editor: {}",buffer.name))))))
            .child(div().id("ide-diagnostics").h(rems(7.)).overflow_y_scroll().track_scroll(&self.diagnostics_scroll).border_t_1().border_color(cx.theme().border).pt_2().text_sm().child(self.diagnostics.clone()))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("F6 moves focus · Esc leaves editor · Tab indents · Buffers stay open when switching tools"))
    }
}
