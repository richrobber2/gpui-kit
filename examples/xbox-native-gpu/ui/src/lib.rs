//! Real GPUI Kit application embedded in the UWP CoreWindow run loop.
mod dispatcher;
mod platform;
use anyhow::{Result, anyhow, bail};
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Root, Sizable, StyledExt, Theme, ThemeMode, button::Button,
    },
    *,
};
use platform::XboxPlatform;
use std::{
    cell::RefCell,
    ffi::{CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
    rc::Rc,
};

type RequestJob = extern "C" fn(u32);
struct Lab {
    focus: FocusHandle,
    selected: usize,
    busy: bool,
    status: String,
    detail: String,
    request: RequestJob,
}
const SIZES: [u32; 3] = [64, 128, 256];
impl Lab {
    fn run(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.selected = index;
        self.busy = true;
        self.status = format!("Computing {} × {} matrices…", SIZES[index], SIZES[index]);
        self.detail = "Checking every GPU output against a CPU reference.".into();
        (self.request)(SIZES[index]);
        cx.notify();
    }
    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "left" | "up" => self.selected = (self.selected + 2) % 3,
            "right" | "down" | "tab" => self.selected = (self.selected + 1) % 3,
            "enter" => self.run(self.selected, cx),
            "x" | "space" => self.run(1, cx),
            "y" => self.run(0, cx),
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }
}
impl Render for Lab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut controls = div().flex().gap_4();
        for (index, n) in SIZES.into_iter().enumerate() {
            controls = controls.child(
                div()
                    .p_1()
                    .border_2()
                    .rounded_md()
                    .border_color(if index == self.selected {
                        theme.primary
                    } else {
                        theme.border
                    })
                    .child(
                        Button::new(("matrix", index))
                            .large()
                            .label(format!("{n} × {n}"))
                            .disabled(self.busy)
                            .on_click(cx.listener(move |this, _, _, cx| this.run(index, cx))),
                    ),
            );
        }
        div().size_full().flex().flex_col().justify_center().p_12().gap_6()
            .font_family("Segoe UI").text_size(px(24.0))
            .bg(theme.background).text_color(theme.foreground)
            .track_focus(&self.focus).on_key_down(cx.listener(Self::key))
            .child(div().text_3xl().font_semibold().child("Xbox GPU Lab"))
            .child(div().text_color(theme.muted_foreground).child("GPUI Kit interface · native Direct3D 11 compute"))
            .child(div().p_6().rounded_lg().border_1().border_color(theme.border).bg(theme.background)
                .child(div().flex().flex_col().gap_4()
                    .child(div().font_semibold().child(self.status.clone()))
                    .child(div().text_color(theme.muted_foreground).child(self.detail.clone()))))
            .child(controls)
            .child(div().text_color(theme.muted_foreground)
                .child("D-pad: select size   A / Enter: run   X: 128   Y: 64"))
            .child(div().text_size(px(18.0)).text_color(theme.muted_foreground)
                .child("One timing sample. GPU time includes allocation, transfers, dispatch and readback."))
    }
}
struct Host {
    app: ApplicationHandle,
    platform: Rc<XboxPlatform>,
    lab: Entity<Lab>,
}
thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}
fn boundary(operation: impl FnOnce() -> Result<()>) -> i32 {
    let outcome = catch_unwind(AssertUnwindSafe(operation));
    let error = match outcome {
        Ok(Ok(())) => return 0,
        Ok(Err(error)) => error.to_string(),
        Err(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "GPUI panic".into()),
    };
    LAST_ERROR.with(|value| *value.borrow_mut() = CString::new(error.replace('\0', " ")).unwrap());
    -1
}
fn with_host(operation: impl FnOnce(&Host) -> Result<()>) -> Result<()> {
    HOST.with(|host| {
        let host = host.borrow();
        operation(
            host.as_ref()
                .ok_or_else(|| anyhow!("GPUI host has not started"))?,
        )
    })
}
/// All ABI calls must run on the same CoreWindow thread. The CoreWindow must
/// remain alive until shutdown. Font bytes are copied during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_xbox_start(
    core: *mut c_void,
    width: f32,
    height: f32,
    font: *const u8,
    length: usize,
    request: Option<RequestJob>,
) -> i32 {
    boundary(|| {
        if HOST.with(|h| h.borrow().is_some()) {
            bail!("GPUI host already started");
        }
        let core = NonNull::new(core).ok_or_else(|| anyhow!("CoreWindow is null"))?;
        if font.is_null() || length == 0 || length > 32 * 1024 * 1024 {
            bail!("Invalid system font buffer");
        }
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            bail!("Invalid CoreWindow dimensions");
        }
        let request = request.ok_or_else(|| anyhow!("Missing compute callback"))?;
        let platform = XboxPlatform::new(
            core,
            width,
            height,
            unsafe { std::slice::from_raw_parts(font, length) }.to_vec(),
        )?;
        let launched = Rc::new(RefCell::new(None));
        let launch_result = launched.clone();
        let app = Application::with_platform(platform.clone()).run_embedded(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            let mut lab = None;
            let startup = cx
                .open_window(WindowOptions::default(), |window, cx| {
                    let content = cx.new(|cx| Lab {
                        focus: cx.focus_handle(),
                        selected: 2,
                        busy: false,
                        status: "Ready to run a GPU test".into(),
                        detail: "Choose a matrix size, then press A.".into(),
                        request,
                    });
                    let focus = content.read(cx).focus.clone();
                    focus.focus(window, cx);
                    lab = Some(content.clone());
                    cx.new(|cx| Root::new(content, window, cx))
                })
                .and_then(|_| lab.ok_or_else(|| anyhow!("GPUI window failed to initialize")));
            *launch_result.borrow_mut() = Some(startup);
        });
        let lab = launched
            .borrow_mut()
            .take()
            .ok_or_else(|| anyhow!("GPUI launch callback did not run"))??;
        HOST.with(|host| *host.borrow_mut() = Some(Host { app, platform, lab }));
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_frame() -> i32 {
    boundary(|| with_host(|host| host.platform.tick()))
}
/// Stable input codes shared with GpuiBridge.h.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_key(code: u32) -> i32 {
    boundary(|| {
        with_host(|host| {
            let key = match code {
                1 => "enter",
                2 => "left",
                3 => "right",
                4 => "up",
                5 => "down",
                6 => "x",
                7 => "y",
                8 => "tab",
                _ => return Ok(()),
            };
            host.platform.key(key)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_resize(width: f32, height: f32) -> i32 {
    boundary(|| {
        with_host(|host| {
            if !width.is_finite() || !height.is_finite() {
                bail!("Invalid resize dimensions");
            }
            host.platform.resize(width, height);
            Ok(())
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_visibility(visible: i32) -> i32 {
    boundary(|| {
        with_host(|host| {
            host.platform.visibility(visible != 0);
            Ok(())
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_result(
    n: u32,
    cpu: f64,
    gpu: f64,
    max_error: f64,
    memory: u64,
    mismatches: u32,
    saved: i32,
) -> i32 {
    boundary(|| {
        with_host(|host| {
            host.app.update(|cx| host.lab.update(cx, |lab, cx| {
            lab.busy = false;
            lab.status = if mismatches == 0 { format!("Passed · all {} outputs verified", n * n) } else { format!("Failed · {mismatches} numerical mismatches") };
            lab.detail = format!("{n} × {n} · CPU: {cpu:.3} ms · GPU total: {gpu:.3} ms · max error: {max_error:.6}\nMemory budget: {} MiB\n{}", memory / (1024 * 1024), if saved != 0 { "Saved: LocalState/native-gpu-result.json" } else { "Could not save the report." });
            cx.notify();
        }));
            Ok(())
        })
    })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_xbox_error(message: *const c_char) -> i32 {
    boundary(|| {
        if message.is_null() {
            bail!("Null error message");
        }
        let message = unsafe { std::ffi::CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned();
        with_host(|host| {
            host.app.update(|cx| {
                host.lab.update(cx, |lab, cx| {
                    lab.busy = false;
                    lab.status = "GPU test stopped".into();
                    lab.detail = message;
                    cx.notify();
                })
            });
            Ok(())
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_last_error() -> *const c_char {
    LAST_ERROR.with(|value| value.borrow().as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_shutdown() -> i32 {
    boundary(|| {
        HOST.with(|host| {
            host.borrow_mut().take();
        });
        Ok(())
    })
}
