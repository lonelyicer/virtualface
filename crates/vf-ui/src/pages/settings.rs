use crate::workspace::Workspace;
use gpui_kit::component::button::Button;
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
use gpui_kit::component::{Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Workspace {
    pub(crate) fn settings_page(&self, cx: &mut Context<Self>, muted: Hsla) -> impl IntoElement {
        let rate = self.session.graph.lock().rate_hz;
        v_flex()
            .id("settings-page")
            .size_full()
            .min_w(px(0.))
            .p_6()
            .gap_4()
            .overflow_y_scroll()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("设置"),
            )
            .child(
                GroupBox::new()
                    .id("set-graph")
                    .outline()
                    .title("图文件")
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(self.graph_path.clone()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("set-open")
                                    .label("打开")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_graph(cx))),
                            )
                            .child(
                                Button::new("set-save")
                                    .label("保存")
                                    .on_click(cx.listener(|this, _, _, cx| this.save_graph(cx))),
                            )
                            .child(
                                Button::new("set-reload")
                                    .label("重新编译")
                                    .on_click(cx.listener(|this, _, _, cx| this.reload_graph(cx))),
                            ),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("set-engine")
                    .outline()
                    .title("引擎频率")
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("rate-dec").small().label("−").on_click(
                                    cx.listener(|this, _, _, cx| this.bump_rate(-5.0, cx)),
                                ),
                            )
                            .child(div().w(px(72.)).child(format!("{rate:.0} Hz")))
                            .child(
                                Button::new("rate-inc").small().label("+").on_click(
                                    cx.listener(|this, _, _, cx| this.bump_rate(5.0, cx)),
                                ),
                            ),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("set-plugins")
                    .outline()
                    .title("已加载插件")
                    .children(self.session.host.plugins().iter().map(|p| {
                        v_flex()
                            .gap_1()
                            .child(div().font_weight(FontWeight::MEDIUM).child(p.name.clone()))
                            .child(div().text_xs().text_color(muted).child(format!(
                                "{}  {}  {}",
                                p.id,
                                p.version,
                                p.path.display()
                            )))
                    }))
                    .children(
                        self.session
                            .load_errors
                            .iter()
                            .map(|e| div().text_xs().text_color(rgb(0xef4444)).child(e.clone())),
                    ),
            )
    }
}
