//! Single-CoreWindow GPUI platform. Unsupported desktop services fail explicitly.
use crate::dispatcher::Dispatcher;
use anyhow::{Result, bail};
use futures::channel::oneshot;
use gpui_kit::*;
use gpui_wgpu::{CosmicTextSystem, WgpuRenderer, WgpuSurfaceConfig};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WinRtWindowHandle, WindowHandle, WindowsDisplayHandle,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    ffi::{OsString, c_void},
    path::{Path, PathBuf},
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Debug)]
struct CoreHandle(NonNull<c_void>);
// The handle is an identity only. GPUI and all surface operations run on the host
// UI thread. The host keeps CoreWindow alive until gpui_xbox_shutdown returns;
// the patched HAL also owns an AddRef. WGPU requires these marker traits.
unsafe impl Send for CoreHandle {}
unsafe impl Sync for CoreHandle {}
impl HasWindowHandle for CoreHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::WinRt(WinRtWindowHandle::new(self.0)))
        })
    }
}
impl HasDisplayHandle for CoreHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Windows(WindowsDisplayHandle::new()))
        })
    }
}
#[derive(Debug)]
struct Display {
    size: Cell<Size<Pixels>>,
}
impl PlatformDisplay for Display {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }
    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::from_u128(1))
    }
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(Point::default(), self.size.get())
    }
}
type FrameCallback = Box<dyn FnMut(RequestFrameOptions)>;
type InputCallback = Box<dyn FnMut(PlatformInput) -> DispatchEventResult>;
type ResizeCallback = Box<dyn FnMut(Size<Pixels>, f32)>;
#[derive(Default)]
struct Callbacks {
    frame: Option<FrameCallback>,
    input: Option<InputCallback>,
    resize: Option<ResizeCallback>,
    active: Option<Box<dyn FnMut(bool)>>,
    visible: Option<Box<dyn FnMut(WindowVisibility)>>,
    close: Option<Box<dyn FnOnce()>>,
}
struct WindowState {
    raw: CoreHandle,
    display: Rc<Display>,
    renderer: RefCell<WgpuRenderer>,
    callbacks: RefCell<Callbacks>,
    visible: Cell<bool>,
    dirty: Cell<bool>,
    failure: RefCell<Option<String>>,
    missed_frames: Cell<u32>,
}
impl WindowState {
    fn tick(&self) {
        if !self.visible.get() {
            return;
        }
        let callback = self.callbacks.borrow_mut().frame.take();
        if let Some(mut callback) = callback {
            callback(RequestFrameOptions {
                require_presentation: self.dirty.replace(false),
                ..Default::default()
            });
            self.callbacks.borrow_mut().frame = Some(callback);
        }
    }
    fn resize(&self, width: f32, height: f32) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let size = size(px(width), px(height));
        if size == self.display.size.get() {
            return;
        }
        self.display.size.set(size);
        self.renderer
            .borrow_mut()
            .update_drawable_size(size_device(size));
        let callback = self.callbacks.borrow_mut().resize.take();
        if let Some(mut callback) = callback {
            callback(size, 1.0);
            self.callbacks.borrow_mut().resize = Some(callback);
        }
        self.dirty.set(true);
    }
}
fn size_device(size: Size<Pixels>) -> Size<DevicePixels> {
    gpui_kit::size(
        DevicePixels(f32::from(size.width).round() as i32),
        DevicePixels(f32::from(size.height).round() as i32),
    )
}
struct XboxWindow {
    state: Rc<WindowState>,
    input_handler: Option<PlatformInputHandler>,
}
impl HasWindowHandle for XboxWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.state.raw.window_handle()
    }
}
impl HasDisplayHandle for XboxWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.state.raw.display_handle()
    }
}
impl PlatformWindow for XboxWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.state.display.bounds()
    }
    fn is_maximized(&self) -> bool {
        true
    }
    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds())
    }
    fn content_size(&self) -> Size<Pixels> {
        self.state.display.size.get()
    }
    fn resize(&mut self, _: Size<Pixels>) {} // CoreWindow geometry is owned by UWP.
    fn scale_factor(&self) -> f32 {
        1.0
    }
    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.state.display.clone())
    }
    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }
    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }
    fn capslock(&self) -> Capslock {
        Capslock::default()
    }
    fn set_input_handler(&mut self, handler: PlatformInputHandler) {
        self.input_handler = Some(handler);
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.take()
    }
    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }
    fn activate(&self) {}
    fn is_active(&self) -> bool {
        self.state.visible.get()
    }
    fn visibility(&self) -> WindowVisibility {
        if self.state.visible.get() {
            WindowVisibility::Visible
        } else {
            WindowVisibility::Hidden
        }
    }
    fn is_hovered(&self) -> bool {
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, _: &str) {}
    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}
    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {}
    fn is_fullscreen(&self) -> bool {
        true
    }
    fn on_request_frame(&self, callback: FrameCallback) {
        self.state.callbacks.borrow_mut().frame = Some(callback);
    }
    fn on_input(&self, callback: InputCallback) {
        self.state.callbacks.borrow_mut().input = Some(callback);
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.state.callbacks.borrow_mut().active = Some(callback);
    }
    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.state.callbacks.borrow_mut().visible = Some(callback);
    }
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}
    fn on_resize(&self, callback: ResizeCallback) {
        self.state.callbacks.borrow_mut().resize = Some(callback);
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) {}
    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.state.callbacks.borrow_mut().close = Some(callback);
    }
    fn on_appearance_changed(&self, _: Box<dyn FnMut()>) {}
    fn draw(&self, scene: &Scene) {
        if !self.state.renderer.borrow_mut().draw(scene) {
            self.state.dirty.set(true);
            let missed = self.state.missed_frames.get().saturating_add(1);
            self.state.missed_frames.set(missed);
            if missed >= 120 {
                *self.state.failure.borrow_mut() =
                    Some("GPUI DirectX 12 presentation failed for 120 frames".into());
            }
        } else {
            self.state.missed_frames.set(0);
        }
    }
    fn schedule_frame(&self) {
        self.state.dirty.set(true);
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.state.renderer.borrow().sprite_atlas().clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.state.renderer.borrow().gpu_specs()
    }
    fn update_ime_position(&self, _: Bounds<Pixels>) {}
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(std::ptr::null_mut())
    }
}
struct KeyboardLayout;
impl PlatformKeyboardLayout for KeyboardLayout {
    fn id(&self) -> &str {
        "xbox-controller"
    }
    fn name(&self) -> &str {
        "Xbox controller / keyboard"
    }
}
pub struct XboxPlatform {
    pub dispatcher: Arc<Dispatcher>,
    raw: CoreHandle,
    display: Rc<Display>,
    text: Arc<CosmicTextSystem>,
    window: RefCell<Option<Rc<WindowState>>>,
    handle: Cell<Option<AnyWindowHandle>>,
    quit: Cell<bool>,
}
impl XboxPlatform {
    pub fn new(core: NonNull<c_void>, width: f32, height: f32, font: Vec<u8>) -> Result<Rc<Self>> {
        let text = Arc::new(CosmicTextSystem::new_without_system_fonts("Segoe UI"));
        text.add_fonts(vec![Cow::Owned(font)])?;
        Ok(Rc::new(Self {
            dispatcher: Dispatcher::new(),
            raw: CoreHandle(core),
            display: Rc::new(Display {
                size: Cell::new(size(px(width), px(height))),
            }),
            text,
            window: RefCell::new(None),
            handle: Cell::new(None),
            quit: Cell::new(false),
        }))
    }
    pub fn tick(&self) -> Result<()> {
        self.dispatcher.tick();
        if self.quit.get() {
            bail!("GPUI application requested shutdown");
        }
        let window = self.window.borrow().clone();
        if let Some(window) = window {
            window.tick();
            if let Some(error) = window.failure.borrow_mut().take() {
                bail!(error);
            }
        }
        Ok(())
    }
    pub fn key(&self, key: &str) -> Result<()> {
        let window = self.window.borrow().clone();
        if let Some(window) = window {
            let callback = window.callbacks.borrow_mut().input.take();
            if let Some(mut callback) = callback {
                callback(PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse(key)?,
                    is_held: false,
                    prefer_character_input: false,
                }));
                window.callbacks.borrow_mut().input = Some(callback);
            }
        }
        Ok(())
    }
    pub fn resize(&self, width: f32, height: f32) {
        if let Some(window) = self.window.borrow().clone() {
            window.resize(width, height);
        }
    }
    pub fn visibility(&self, visible: bool) {
        let window = self.window.borrow().clone();
        if let Some(window) = window {
            if window.visible.replace(visible) == visible {
                return;
            }
            let callback = window.callbacks.borrow_mut().visible.take();
            if let Some(mut callback) = callback {
                callback(if visible {
                    WindowVisibility::Visible
                } else {
                    WindowVisibility::Hidden
                });
                window.callbacks.borrow_mut().visible = Some(callback);
            }
            let callback = window.callbacks.borrow_mut().active.take();
            if let Some(mut callback) = callback {
                callback(visible);
                window.callbacks.borrow_mut().active = Some(callback);
            }
            window.dirty.set(true);
        }
    }
}
fn unsupported<T>() -> Result<T> {
    bail!("This desktop service is unavailable in the Xbox UWP host")
}
fn cancelled<T>() -> oneshot::Receiver<Result<Option<T>>> {
    let (tx, rx) = oneshot::channel();
    let _ = tx.send(Ok(None));
    rx
}
impl Platform for XboxPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, callback: Box<dyn FnOnce()>) {
        callback();
    }
    fn quit(&self) {
        self.quit.set(true);
    }
    fn restart(&self, _: Option<PathBuf>, _: Vec<OsString>) {
        self.quit();
    }
    fn activate(&self, _: bool) {}
    fn hide(&self) {}
    fn hide_other_apps(&self) {}
    fn unhide_other_apps(&self) {}
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.display.clone()]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.display.clone())
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.handle.get()
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        _: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        if self.window.borrow().is_some() {
            bail!("The UWP host supports one CoreWindow");
        }
        let renderer = WgpuRenderer::new(
            Rc::new(RefCell::new(None)),
            &self.raw,
            WgpuSurfaceConfig {
                size: size_device(self.display.size.get()),
                transparent: false,
                preferred_present_mode: Some(gpui_wgpu::wgpu::PresentMode::Fifo),
            },
            None,
        )?;
        let state = Rc::new(WindowState {
            raw: self.raw.clone(),
            display: self.display.clone(),
            renderer: RefCell::new(renderer),
            callbacks: RefCell::new(Callbacks::default()),
            visible: Cell::new(true),
            dirty: Cell::new(true),
            failure: RefCell::new(None),
            missed_frames: Cell::new(0),
        });
        self.handle.set(Some(handle));
        *self.window.borrow_mut() = Some(state.clone());
        Ok(Box::new(XboxWindow {
            state,
            input_handler: None,
        }))
    }
    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }
    fn open_url(&self, _: &str) {}
    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {}
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(unsupported())
    }
    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        cancelled()
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        cancelled()
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {}
    fn open_with_system(&self, _: &Path) {}
    fn on_quit(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_reopen(&self, _: Box<dyn FnMut()>) {}
    fn on_system_sleep(&self, _: Box<dyn FnMut()>) {}
    fn on_system_wake(&self, _: Box<dyn FnMut()>) {}
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {}
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {}
    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn Action)>) {}
    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) {}
    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn Action) -> bool>) {}
    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }
    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}
    fn prevent_idle_sleep(&self, _: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(unsupported())
    }
    fn app_path(&self) -> Result<PathBuf> {
        unsupported()
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        unsupported()
    }
    fn set_cursor_style(&self, _: CursorStyle) {}
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool {
        false
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        false
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }
    fn write_to_clipboard(&self, _: ClipboardItem) {}
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _: ClipboardItem) {}
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(unsupported())
    }
    fn read_credentials(&self, _: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(unsupported())
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(unsupported())
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(KeyboardLayout)
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}
}
