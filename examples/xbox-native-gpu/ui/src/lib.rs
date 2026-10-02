//! Real GPUI Kit application embedded in the UWP CoreWindow run loop.
mod dispatcher;
mod model_studio;
mod platform;
mod preview;
mod tools;
mod workbench;
use anyhow::{Result, anyhow, bail};
use gpui_kit::{
    component::{Root, Theme, ThemeMode},
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
use workbench::{Lab, Sample};

type RequestJob = workbench::RequestJob;
struct Host {
    app: ApplicationHandle,
    platform: Rc<XboxPlatform>,
    lab: Entity<Lab>,
    studio: Entity<model_studio::Studio>,
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
            // Scale the whole component system together for television viewing.
            Theme::global_mut(cx).font_size = px(24.0);
            Theme::sync_base(cx);
            let mut lab = None;
            let mut studio = None;
            let startup = cx
                .open_window(WindowOptions::default(), |window, cx| {
                    let content = cx.new(|cx| Lab::new(request, cx));
                    let images = cx.new(|cx| model_studio::Studio::new(window, cx));
                    let focus = images.read(cx).focus.clone();
                    focus.focus(window, cx);
                    lab = Some(content.clone());
                    studio = Some(images.clone());
                    let tools = cx.new(|_| tools::Tools::new(content, images));
                    cx.new(|cx| Root::new(tools, window, cx))
                })
                .and_then(|_| {
                    lab.zip(studio)
                        .ok_or_else(|| anyhow!("GPUI window failed to initialize"))
                });
            *launch_result.borrow_mut() = Some(startup);
        });
        let (lab, studio) = launched
            .borrow_mut()
            .take()
            .ok_or_else(|| anyhow!("GPUI launch callback did not run"))??;
        HOST.with(|host| {
            *host.borrow_mut() = Some(Host {
                app,
                platform,
                lab,
                studio,
            })
        });
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_frame() -> i32 {
    boundary(|| {
        with_host(|host| {
            host.app
                .update(|cx| host.studio.update(cx, |studio, cx| studio.poll(cx)));
            host.platform.tick()
        })
    })
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
                9 => "f1",
                10 => "escape",
                11 => "backspace",
                12 => "delete",
                _ => return Ok(()),
            };
            host.platform.key(key)
        })
    })
}
/// Host-owned ApplicationData path is copied; no cross-app storage is assumed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_xbox_storage(path: *const u16, length: usize) -> i32 {
    boundary(|| {
        if path.is_null() || length == 0 || length > 32768 {
            bail!("Invalid storage path");
        }
        let root = String::from_utf16(unsafe { std::slice::from_raw_parts(path, length) })?;
        with_host(|host| {
            host.app.update(|cx| {
                host.studio
                    .update(cx, |studio, cx| studio.storage(root.into(), cx))
            });
            Ok(())
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_character(code: u32) -> i32 {
    boundary(|| {
        let Some(character) = char::from_u32(code) else {
            return Ok(());
        };
        if character.is_control() {
            return Ok(());
        }
        with_host(|host| host.platform.text(&character.to_string()))
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
            host.app.update(|cx| {
                host.lab.update(cx, |lab, cx| {
                    lab.complete(
                        Sample {
                            n,
                            cpu,
                            gpu,
                            max_error,
                            memory,
                            mismatches,
                            saved: saved != 0,
                        },
                        cx,
                    );
                })
            });
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
                    lab.fail(message, cx);
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
