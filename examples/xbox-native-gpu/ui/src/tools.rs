use crate::{model_studio::Studio, workbench::Lab};
use gpui_kit::{
    component::{
        ActiveTheme, Sizable,
        tab::{Tab, TabBar},
    },
    *,
};
pub(super) struct Tools {
    lab: Entity<Lab>,
    studio: Entity<Studio>,
    images: bool,
}
impl Tools {
    pub(super) fn new(lab: Entity<Lab>, studio: Entity<Studio>) -> Self {
        Self {
            lab,
            studio,
            images: true,
        }
    }
    fn select(&mut self, images: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.images = images;
        if images {
            self.studio.read(cx).focus.clone().focus(window, cx);
        } else {
            self.lab.read(cx).focus.clone().focus(window, cx);
        }
        cx.notify();
    }
}
impl Render for Tools {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "f1" {
                    this.select(!this.images, window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div().px_8().pt_3().child(
                    TabBar::new("workbench-tools")
                        .large()
                        .underline()
                        .child(Tab::new().label("Image models"))
                        .child(Tab::new().label("GPU verification"))
                        .selected_index(usize::from(!self.images))
                        .on_click(
                            cx.listener(|this, ix, window, cx| this.select(*ix == 0, window, cx)),
                        ),
                ),
            )
            .child(div().flex_1().min_h_0().child(if self.images {
                self.studio.clone().into_any_element()
            } else {
                self.lab.clone().into_any_element()
            }))
    }
}
