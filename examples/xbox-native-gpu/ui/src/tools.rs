use crate::{controls::Controls, ide::Ide, model_studio::Studio, workbench::Lab};
use gpui_kit::{
    component::{
        ActiveTheme, Selectable,
        button::{Button, ButtonVariants},
    },
    *,
};
const NAMES: [&str; 4] = ["Models", "IDE", "GPU", "Controls"];
pub(super) struct Tools {
    lab: Entity<Lab>,
    studio: Entity<Studio>,
    pub(super) ide: Entity<Ide>,
    controls: Entity<Controls>,
    selected: usize,
    previous: usize,
    palette: bool,
    focus: FocusHandle,
    remembered: [Option<FocusHandle>; 4],
    scroll: [ScrollHandle; 4],
}
impl Tools {
    pub(super) fn new(
        lab: Entity<Lab>,
        studio: Entity<Studio>,
        ide: Entity<Ide>,
        controls: Entity<Controls>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            lab,
            studio,
            ide,
            controls,
            selected: 0,
            previous: 0,
            palette: false,
            focus: cx.focus_handle(),
            remembered: std::array::from_fn(|_| None),
            scroll: std::array::from_fn(|_| ScrollHandle::new()),
        }
    }
    fn select(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix != self.selected {
            self.previous = self.selected;
            self.remembered[self.selected] = window.focused(cx);
        }
        self.selected = ix;
        self.palette = false;
        let focus = self.remembered[ix].clone().unwrap_or_else(|| match ix {
            0 => self.studio.read(cx).focus.clone(),
            1 => self.ide.read(cx).focus.clone(),
            2 => self.lab.read(cx).focus.clone(),
            _ => self.controls.read(cx).focus.clone(),
        });
        focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn scroll(
        &mut self,
        direction: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.selected == 1 && !self.palette {
            return self
                .ide
                .update(cx, |ide, cx| ide.scroll(direction, window, cx));
        }
        let handle = &self.scroll[self.selected];
        let mut offset = handle.offset();
        offset.y = (offset.y - px(direction as f32 * 240.)).min(px(0.));
        handle.set_offset(offset);
        cx.notify();
        true
    }
    pub(super) fn status(&self, cx: &App) -> serde_json::Value {
        serde_json::json!({"tool":NAMES[self.selected],"commands":self.palette,"ide":self.ide.read(cx).status(cx)})
    }
    pub(super) fn command(&mut self, command: u32, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            1..=4 => self.select((command - 1) as usize, window, cx),
            5 => self.select((self.selected + 3) % 4, window, cx),
            6 => self.select((self.selected + 1) % 4, window, cx),
            7 => window.focus_next(cx),
            8 => window.focus_prev(cx),
            9 => {
                self.palette = !self.palette;
                self.focus.focus(window, cx);
                cx.notify();
            }
            10 => {
                if self.palette {
                    self.palette = false;
                    self.select(self.selected, window, cx);
                } else if self.selected == 3 {
                    self.select(self.previous, window, cx);
                } else {
                    match self.selected {
                        0 => self.studio.update(cx, |s, cx| s.back(window, cx)),
                        1 => {
                            self.ide.update(cx, |s, cx| s.cancel(cx));
                            self.ide.read(cx).focus.clone().focus(window, cx);
                        }
                        _ => self.lab.read(cx).focus.clone().focus(window, cx),
                    };
                }
            }
            11 => self.ide.update(cx, |ide, cx| ide.save(cx)),
            12 => self.ide.update(cx, |ide, cx| ide.check(cx)),
            13 => {
                self.select(1, window, cx);
                self.ide.update(cx, |ide, cx| ide.focus_editor(window, cx));
            }
            14 => {
                self.select(1, window, cx);
                self.ide.update(cx, |ide, cx| ide.focus_files(window, cx));
            }
            _ => {}
        }
    }
}
impl Render for Tools {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.palette {
            div().p_6().flex().flex_col().gap_3().child(div().text_2xl().child("Commands"))
                .child(div().text_color(cx.theme().muted_foreground).child("Move focus with F6 or D-pad. Enter / A runs the focused command. Esc / B returns."))
                .children([(1,"Models"),(2,"IDE"),(3,"GPU verification"),(4,"Controls"),(13,"IDE: Focus editor"),(14,"IDE: Browse files"),(11,"IDE: Save file"),(12,"IDE: Check Rust"),(10,"Back")].into_iter().map(|(code,label)|
                    Button::new(("command",code as usize)).label(label).on_click(cx.listener(move|this,_,window,cx|this.command(code,window,cx)))))
                .into_any_element()
        } else {
            match self.selected {
                0 => self.studio.clone().into_any_element(),
                1 => self.ide.clone().into_any_element(),
                2 => self.lab.clone().into_any_element(),
                _ => self.controls.clone().into_any_element(),
            }
        };
        div().size_full().flex().flex_col().bg(cx.theme().background).track_focus(&self.focus)
            .child(div().flex().flex_1().min_h_0()
                .child(div().w(rems(8.)).py_6().px_3().flex().flex_col().gap_3().border_r_1().border_color(cx.theme().border)
                    .child(div().mb_4().text_sm().text_color(cx.theme().muted_foreground).child("GPUI / XBOX"))
                    .children(NAMES.into_iter().enumerate().map(|(ix,name)|Button::new(("tool",ix)).label(format!("{}  {}",ix+1,name)).selected(self.selected==ix).on_click(cx.listener(move|this,_,window,cx|this.select(ix,window,cx)))))
                    .child(div().flex_1())
                    .child(Button::new("commands").ghost().label("Commands").on_click(cx.listener(|this,_,window,cx|this.command(9,window,cx)))))
                .child(div().id("tool-content").flex_1().min_w_0().min_h_0().overflow_y_scroll().track_scroll(&self.scroll[self.selected]).child(content)))
            .child(div().px_4().py_2().border_t_1().border_color(cx.theme().border).text_sm().text_color(cx.theme().muted_foreground)
                .child("LB / RB  Tools     D-pad  Focus     A  Activate     B  Back     Menu  Commands     F1  Controls"))
    }
}
