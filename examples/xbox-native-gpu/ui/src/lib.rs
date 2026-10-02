//! Real GPUI Kit application embedded in the UWP CoreWindow run loop.
mod controls;
mod dispatcher;
mod ide;
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
    cell::Cell,
    cell::RefCell,
    ffi::{CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    ptr::NonNull,
    rc::Rc,
    time::{Duration, Instant},
};
use workbench::{Lab, Sample};

type RequestJob = workbench::RequestJob;
struct Host {
    app: ApplicationHandle,
    platform: Rc<XboxPlatform>,
    lab: Entity<Lab>,
    studio: Entity<model_studio::Studio>,
    tools: Entity<tools::Tools>,
    window: WindowHandle<Root>,
    root: RefCell<Option<PathBuf>>,
    probe_at: Cell<Instant>,
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
        let app = Application::with_platform(platform.clone())
            .with_assets(gpui_kit::assets::Assets)
            .run_embedded(move |cx| {
                gpui_kit::init(cx);
                Theme::change(ThemeMode::Dark, None, cx);
                // Scale the whole component system together for television viewing.
                Theme::global_mut(cx).font_size = px(24.0);
                Theme::global_mut(cx).mono_font_size = px(22.0);
                Theme::sync_base(cx);
                let mut lab = None;
                let mut studio = None;
                let mut tool_entity = None;
                let startup = cx
                    .open_window(WindowOptions::default(), |window, cx| {
                        let content = cx.new(|cx| Lab::new(request, cx));
                        let images = cx.new(|cx| model_studio::Studio::new(window, cx));
                        let focus = images.read(cx).focus.clone();
                        focus.focus(window, cx);
                        lab = Some(content.clone());
                        studio = Some(images.clone());
                        let editor = cx.new(|cx| ide::Ide::new(window, cx));
                        let controls = cx.new(controls::Controls::new);
                        let tools =
                            cx.new(|cx| tools::Tools::new(content, images, editor, controls, cx));
                        tool_entity = Some(tools.clone());
                        cx.new(|cx| Root::new(tools, window, cx))
                    })
                    .and_then(|window| {
                        let (lab, studio) = lab
                            .zip(studio)
                            .ok_or_else(|| anyhow!("GPUI window failed to initialize"))?;
                        Ok((lab, studio, tool_entity.unwrap(), window))
                    });
                *launch_result.borrow_mut() = Some(startup);
            });
        let (lab, studio, tools, window) = launched
            .borrow_mut()
            .take()
            .ok_or_else(|| anyhow!("GPUI launch callback did not run"))??;
        HOST.with(|host| {
            *host.borrow_mut() = Some(Host {
                app,
                platform,
                lab,
                studio,
                tools,
                window,
                root: RefCell::new(None),
                probe_at: Cell::new(Instant::now()),
            })
        });
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_frame() -> i32 {
    boundary(|| {
        with_host(|host| {
            host.app.update(|cx| {
                host.studio.update(cx, |studio, cx| studio.poll(cx));
                let ide = host.tools.read(cx).ide.clone();
                ide.update(cx, |ide, cx| ide.poll(cx));
            });
            ui_probe(host)?;
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
            *host.root.borrow_mut() = Some(root.clone().into());
            host.app.update(|cx| {
                host.studio
                    .update(cx, |studio, cx| studio.storage(root.clone().into(), cx));
                let ide = host.tools.read(cx).ide.clone();
                host.window.update(cx, |_, window, cx| {
                    ide.update(cx, |ide, cx| ide.storage(root.into(), window, cx))
                })
            })?;
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
        with_host(|host| text_input(host, &character.to_string()))
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

fn command(host: &Host, code: u32) -> Result<()> {
    host.app.update(|cx| {
        host.window.update(cx, |_, window, cx| {
            host.tools
                .update(cx, |tools, cx| tools.command(code, window, cx))
        })
    })?;
    Ok(())
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_command(code: u32) -> i32 {
    boundary(|| with_host(|host| command(host, code)))
}
/// WinRT virtual key and modifier flags (Ctrl=1, Shift=2, Alt=4).
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_keyboard(vk: u32, modifiers: u32) -> i32 {
    boundary(|| {
        with_host(|host| {
            let ctrl = modifiers & 1 != 0;
            let shift = modifiers & 2 != 0;
            let global = match (vk, ctrl, shift) {
                (49..=52, true, _) => Some(vk - 48),
                (112, _, _) => Some(4),
                (117, _, true) => Some(8),
                (117, _, false) => Some(7),
                (80, true, true) => Some(9),
                (80, true, false) => Some(14),
                (83, true, _) => Some(11),
                (116, _, _) => Some(12),
                (27, _, _) => Some(10),
                _ => None,
            };
            if let Some(code) = global {
                return command(host, code);
            }
            let key = match vk {
                8 => "backspace".into(),
                9 => "tab".into(),
                13 => "enter".into(),
                32 => "space".into(),
                33 => "pageup".into(),
                34 => "pagedown".into(),
                35 => "end".into(),
                36 => "home".into(),
                37 => "left".into(),
                38 => "up".into(),
                39 => "right".into(),
                40 => "down".into(),
                46 => "delete".into(),
                65..=90 => char::from_u32(vk + 32).unwrap().to_string(),
                _ => return Ok(()),
            };
            let key = format!(
                "{}{}{}{}",
                if ctrl { "ctrl-" } else { "" },
                if shift { "shift-" } else { "" },
                if modifiers & 4 != 0 { "alt-" } else { "" },
                key
            );
            host.platform.key(&key)
        })
    })
}
/// Controller activation emits a complete key press, including release.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_activate() -> i32 {
    boundary(|| with_host(|host| host.platform.key("enter")))
}
#[unsafe(no_mangle)]
pub extern "C" fn gpui_xbox_scroll(direction: i32) -> i32 {
    boundary(|| {
        with_host(|host| {
            let handled = host.app.update(|cx| {
                host.window.update(cx, |_, window, cx| {
                    host.tools
                        .update(cx, |tools, cx| tools.scroll(direction, window, cx))
                })
            })?;
            if !handled {
                host.platform.scroll(direction)?;
            }
            Ok(())
        })
    })
}

#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeInput {
    Command { code: u32 },
    Key { key: String },
    Text { text: String },
    Activate,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    id: String,
    inputs: Vec<ProbeInput>,
}
fn ui_probe(host: &Host) -> Result<()> {
    if Instant::now() < host.probe_at.get() {
        return Ok(());
    }
    host.probe_at
        .set(Instant::now() + Duration::from_millis(250));
    let Some(root) = host.root.borrow().clone() else {
        return Ok(());
    };
    let file = root.join("navigation.request.json");
    if let Ok(metadata) = std::fs::metadata(&file) {
        if metadata.len() <= 65536 {
            let request = std::fs::read(&file)
                .ok()
                .and_then(|b| serde_json::from_slice::<ProbeRequest>(&b).ok());
            if let Some(request) = request {
                if request.id.len() <= 128
                    && request.inputs.len() <= 32
                    && std::fs::rename(
                        &file,
                        root.join(format!("navigation-{}.consumed.json", uuid::Uuid::new_v4())),
                    )
                    .is_ok()
                {
                    for input in request.inputs {
                        match input {
                            ProbeInput::Command { code } if (1..=14).contains(&code) => {
                                command(host, code)?
                            }
                            ProbeInput::Key { key } if key.len() <= 64 => {
                                host.platform.key(&key)?
                            }
                            ProbeInput::Text { text } if text.len() <= 4096 => {
                                text_input(host, &text)?
                            }
                            ProbeInput::Activate => host.platform.key("enter")?,
                            _ => {}
                        }
                    }
                    let _ = std::fs::write(root.join("navigation.consumed.txt"), request.id);
                }
            }
        }
    }
    let status = host.app.update(|cx| host.tools.read(cx).status(cx));
    let _ = std::fs::write(
        root.join("navigation-status.json"),
        serde_json::to_vec(&status)?,
    );
    Ok(())
}

/// Optional DirectWrite monospace font, copied before returning to the host.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpui_xbox_mono_font(font: *const u8, length: usize) -> i32 {
    boundary(|| {
        if font.is_null() || length == 0 || length > 32 * 1024 * 1024 {
            bail!("Invalid monospace font buffer");
        }
        let bytes = unsafe { std::slice::from_raw_parts(font, length) }.to_vec();
        with_host(|host| {
            host.platform.add_font(bytes)?;
            host.app.update(|cx| {
                Theme::global_mut(cx).mono_font_family = "Consolas".into();
                Theme::sync_base(cx);
                cx.refresh_windows();
            });
            Ok(())
        })
    })
}

fn text_input(host: &Host, text: &str) -> Result<()> {
    // A platform KeyDown alone does not commit characters in GPUI. Use the
    // window's dispatch path, which inserts through the focused input handler.
    // Update the window without borrowing its Root entity: key propagation
    // may update Root even when no text field is focused.
    host.app.update(|cx| {
        cx.update_window(host.window.into(), |_, window, cx| {
            window.dispatch_keystroke(
                Keystroke {
                    key: String::new(),
                    key_char: Some(text.into()),
                    modifiers: Modifiers::default(),
                },
                cx,
            );
        })
    })?;
    Ok(())
}
