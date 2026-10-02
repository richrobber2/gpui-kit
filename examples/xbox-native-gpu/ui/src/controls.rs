use gpui_kit::{component::ActiveTheme, *};
use std::sync::Arc;

pub(super) struct Controls {
    pub(super) focus: FocusHandle,
    illustration: Option<Arc<RenderImage>>,
}
impl Controls {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let illustration =
            image::load_from_memory(include_bytes!("../../Assets/EliteController.png"))
                .ok()
                .map(|image| {
                    let mut rgba = image.into_rgba8();
                    for pixel in rgba.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]))
                });
        Self {
            focus: cx.focus_handle(),
            illustration,
        }
    }
}
impl Render for Controls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row = |binding: &'static str, meaning: &'static str| {
            div()
                .flex()
                .gap_4()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .w_40()
                        .text_color(cx.theme().foreground)
                        .child(binding),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(cx.theme().muted_foreground)
                        .child(meaning),
                )
        };
        div().size_full().p_6().flex().flex_col().gap_3().track_focus(&self.focus)
            .child(div().text_2xl().child("Controller & keyboard"))
            .child(div().text_color(cx.theme().muted_foreground).child("One navigation scheme across Models, IDE, GPU and Controls."))
            .child(div().flex_1().min_h_0().flex().gap_6()
                .child(div().flex_1().min_w_0().flex().flex_col().justify_center()
                    .children(self.illustration.clone().map(|image| img(ImageSource::Render(image)).w_full().h(rems(22.)).object_fit(ObjectFit::Contain)))
                    .child(div().text_color(cx.theme().muted_foreground).child("Elite paddles follow your Xbox Accessories profile.")))
                .child(div().w(rems(24.)).flex().flex_col().justify_center()
                    .child(row("D-pad / Left stick", "Move focus"))
                    .child(row("A", "Activate focused control"))
                    .child(row("B", "Back / cancel"))
                    .child(row("LB / RB", "Previous / next tool"))
                    .child(row("Menu", "Commands"))
                    .child(row("View", "Controls"))
                    .child(row("LT / RT", "Scroll active pane"))))
            .child(div().border_t_1().border_color(cx.theme().border).pt_3().flex().flex_col().gap_1()
                .child("Keyboard")
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Ctrl 1–4  Tools     F6 / Shift F6  Move focus     Ctrl Shift P  Commands     F1  Controls"))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("IDE: Ctrl P  Files     Ctrl S  Save     F5  Check Rust     Esc  Leave editor     Tab  Indent")))
    }
}
