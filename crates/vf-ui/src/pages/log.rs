use crate::i18n::{T, t};
use crate::workspace::Workspace;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Workspace {
    pub(crate) fn log_page(&mut self, cx: &App, muted: Hsla) -> impl IntoElement {
        let lines = self.session.host.log.snapshot();
        let tail = lines.last().map(|l| l.ts_us).unwrap_or(0);
        if lines.len() != self.log_len || tail != self.log_tail_ts {
            self.log_len = lines.len();
            self.log_tail_ts = tail;
            self.log_scroll.scroll_to_bottom();
        }
        v_flex()
            .id("log-page")
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_6()
            .child(
                v_flex()
                    .id("log-list")
                    .flex_1()
                    .min_h(px(0.))
                    .p_2()
                    .gap_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .track_scroll(&self.log_scroll)
                    .overflow_y_scroll()
                    .map(|el| {
                        if lines.is_empty() {
                            el.child(div().text_xs().text_color(muted).child(t(cx, T::LogEmpty)))
                        } else {
                            el.children(lines.into_iter().map(|l| {
                                let color: Hsla = match l.level {
                                    0 => rgb(0xef4444).into(),
                                    1 => rgb(0xf59e0b).into(),
                                    2 => rgb(0xe2e8f0).into(),
                                    _ => muted,
                                };
                                div().text_xs().text_color(color).child(l.format_text())
                            }))
                        }
                    }),
            )
    }
}
