use crate::workspace::Workspace;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Workspace {
    pub(crate) fn log_page(&self, muted: Hsla) -> impl IntoElement {
        let lines = self.session.host.log.snapshot();
        v_flex()
            .id("log-page")
            .size_full()
            .min_w(px(0.))
            .p_4()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("日志"),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} 条", lines.len())),
                    ),
            )
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
                    .overflow_y_scroll()
                    .map(|el| {
                        if lines.is_empty() {
                            el.child(div().text_xs().text_color(muted).child("暂无日志"))
                        } else {
                            el.children(lines.into_iter().rev().map(|l| {
                                let color: Hsla = match l.level {
                                    0 => rgb(0xef4444).into(),
                                    1 => rgb(0xf59e0b).into(),
                                    2 => rgb(0xe2e8f0).into(),
                                    _ => muted,
                                };
                                let node = l.node.map(|n| format!(" node:{n}")).unwrap_or_default();
                                div().text_xs().text_color(color).child(format!(
                                    "[{}]{node}  {}",
                                    vf_core::level_name(l.level),
                                    l.message
                                ))
                            }))
                        }
                    }),
            )
    }
}
