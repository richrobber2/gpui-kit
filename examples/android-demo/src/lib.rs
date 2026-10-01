use gpui_kit::component::{
    ActiveTheme, Disableable, Root, Sizable, StyledExt, Theme, ThemeMode, button::Button,
    switch::Switch,
};
use gpui_kit::*;

struct Demo {
    count: u32,
}

impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_6()
            .justify_center()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_2xl().font_semibold().child("GPUI Kit demo"))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Native Rust controls on Android. Tap Add to try a button."),
            )
            .child(div().text_3xl().child(format!("Count: {}", self.count)))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        Button::new("add")
                            .label("Add")
                            .large()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.count = this.count.saturating_add(1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("reset")
                            .label("Reset")
                            .large()
                            .disabled(self.count == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.count = 0;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Switch::new("dark-mode")
                    .label("Dark mode")
                    .checked(cx.theme().is_dark())
                    .large()
                    .on_click(|checked, window, cx| {
                        Theme::change(
                            if *checked {
                                ThemeMode::Dark
                            } else {
                                ThemeMode::Light
                            },
                            Some(window),
                            cx,
                        );
                    }),
            )
    }
}

#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("gpui-kit-demo"),
    );
    gpui_mobile::android::jni::install_panic_hook();
    gpui_mobile::android::jni::init_platform(&app);
    let Some(platform) = gpui_mobile::android::jni::shared_platform() else {
        log::error!("Android platform initialization failed");
        return;
    };
    Application::with_platform(platform.into_rc()).run(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let content = cx.new(|_| Demo { count: 0 });
            cx.new(|cx| Root::new(content, window, cx))
        })
        .expect("open demo window");
    });
}
