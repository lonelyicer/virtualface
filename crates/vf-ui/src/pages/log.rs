use crate::i18n::{T, t};
use crate::workspace::Workspace;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::VecDeque;
use vf_core::LogBus;

struct LogRow {
    seq: u64,
    level: u32,
    text: SharedString,
}

pub(crate) struct LogView {
    seq: u64,
    rows: VecDeque<LogRow>,
    list: ListState,
}

impl LogView {
    pub fn new() -> Self {
        let list = ListState::new(0, ListAlignment::Top, px(100.));
        list.set_follow_mode(FollowMode::Tail);
        Self {
            seq: 0,
            rows: VecDeque::new(),
            list,
        }
    }

    fn sync(&mut self, bus: &LogBus) {
        if self.seq == bus.seq() {
            return;
        }
        let update = bus.snapshot_since(self.seq);
        let removed = self
            .rows
            .iter()
            .take_while(|row| row.seq < update.retained_from)
            .count();
        self.rows.drain(..removed);
        if removed > 0 {
            self.list.splice(0..removed, 0);
        }
        let start = self.rows.len();
        self.rows
            .extend(update.lines.into_iter().map(|(seq, line)| LogRow {
                seq,
                level: line.level,
                text: line.format_text().into(),
            }));
        if self.rows.len() > start {
            self.list.splice(start..start, self.rows.len() - start);
        }
        self.seq = update.seq;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn log_retention_preserves_history_scroll_and_formats_only_new_rows() {
        let bus = LogBus::new();
        for i in 0..500 {
            bus.log(2, None, format!("line {i}\ncontinued"));
        }
        let mut view = LogView::new();
        view.sync(&bus);
        assert_eq!(view.rows.len(), 500);
        assert_eq!(view.list.item_count(), 500);
        assert!(view.list.is_following_tail());
        assert!(view.rows[0].text.contains("line 0\ncontinued"));
        let existing_text = view.rows[150].text.clone();
        view.list.scroll_to(ListOffset {
            item_ix: 150,
            offset_in_item: px(3.),
        });
        bus.log(2, None, "newest");
        view.sync(&bus);
        assert_eq!(view.rows.len(), 500);
        assert_eq!(view.list.item_count(), 500);
        assert!(!view.list.is_following_tail());
        assert_eq!(view.list.logical_scroll_top().item_ix, 149);
        assert_eq!(view.list.logical_scroll_top().offset_in_item, px(3.));
        assert_eq!(view.rows[149].text, existing_text);
        assert!(view.rows.back().unwrap().text.ends_with("newest"));
        view.sync(&bus);
        assert_eq!(view.list.logical_scroll_top().item_ix, 149);
    }
}

impl Workspace {
    pub(crate) fn log_page(&mut self, cx: &mut Context<Self>, muted: Hsla) -> impl IntoElement {
        self.log_view.sync(&self.session.host.log);
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
                    .min_w(px(0.))
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .overflow_hidden()
                    .map(|el| {
                        if self.log_view.rows.is_empty() {
                            el.child(div().text_xs().text_color(muted).child(t(cx, T::LogEmpty)))
                        } else {
                            // Variable-height virtualization preserves wrapped and multi-line logs.
                            el.child(
                                list(
                                    self.log_view.list.clone(),
                                    cx.processor(move |this, ix: usize, _, _| {
                                        let row = &this.log_view.rows[ix];
                                        let color: Hsla = match row.level {
                                            0 => rgb(0xef4444).into(),
                                            1 => rgb(0xf59e0b).into(),
                                            2 => rgb(0xe2e8f0).into(),
                                            _ => muted,
                                        };
                                        div()
                                            .w_full()
                                            .pb_1()
                                            .text_xs()
                                            .text_color(color)
                                            .child(row.text.clone())
                                            .into_any_element()
                                    }),
                                )
                                .size_full(),
                            )
                        }
                    }),
            )
    }
}
