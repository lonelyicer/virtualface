use crate::theme::{latest_snapshot, status_rgb};
use crate::workspace::Workspace;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Workspace {
    pub(crate) fn home_page(&self, cx: &mut Context<Self>, muted: Hsla) -> impl IntoElement {
        let snap = latest_snapshot(&self.session);
        let graph = self.session.graph.lock().clone();
        let plugin_n = self.session.host.plugins().len();
        let type_n = self.session.registry.all().len();
        let missing = graph.nodes.iter().filter(|n| n.missing).count();
        let running = snap.running;

        v_flex()
            .id("home-page")
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_6()
            .gap_4()
            .overflow_y_scroll()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("状态"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(stat_card(
                        "引擎",
                        if running { "运行中" } else { "已停止" },
                        running,
                    ))
                    .child(stat_card("Tick", &snap.tick.to_string(), running))
                    .child(stat_card("丢帧", &snap.drops.to_string(), snap.drops == 0))
                    .child(stat_card("频率", &format!("{:.0} Hz", graph.rate_hz), true)),
            )
            .child(
                GroupBox::new()
                    .id("home-engine")
                    .outline()
                    .title("引擎")
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("home-start")
                                    .primary()
                                    .label("启动")
                                    .on_click(cx.listener(|this, _, _, cx| this.start_engine(cx))),
                            )
                            .child(
                                Button::new("home-stop")
                                    .label("停止")
                                    .on_click(cx.listener(|this, _, _, cx| this.stop_engine(cx))),
                            )
                            .child(
                                div()
                                    .ml_auto()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(self.status.clone()),
                            ),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("home-graph")
                    .outline()
                    .title("节点图")
                    .child(div().child(format!("图  {}", self.graph_name())))
                    .when(!self.graph_path.is_empty(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(self.graph_path.clone()),
                        )
                    })
                    .child(div().child(format!(
                        "节点 {}  ·  连线 {}  ·  缺失插件 {}",
                        graph.nodes.len(),
                        graph.edges.len(),
                        missing
                    ))),
            )
            .child(
                GroupBox::new()
                    .id("home-plugins")
                    .outline()
                    .title("插件")
                    .child(div().child(format!("已加载 {plugin_n} 个插件，{type_n} 种节点")))
                    .children(self.session.host.plugins().iter().map(|p| {
                        div()
                            .text_xs()
                            .child(format!("{}  {}  {}", p.name, p.version, p.id))
                    }))
                    .children(
                        self.session
                            .load_errors
                            .iter()
                            .map(|e| div().text_xs().text_color(rgb(0xef4444)).child(e.clone())),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("home-nodes")
                    .outline()
                    .title("节点状态")
                    .map(|box_| {
                        if graph.nodes.is_empty() {
                            return box_.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child("图是空的，到节点图添加节点"),
                            );
                        }
                        box_.children(graph.nodes.iter().map(|n| {
                            let st = snap.nodes.get(&n.id.0);
                            let level = st.map(|s| s.status_level).unwrap_or(3);
                            let text = st
                                .map(|s| s.status_text.clone())
                                .unwrap_or_else(|| n.type_id.clone());
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(8.))
                                        .h(px(8.))
                                        .rounded_full()
                                        .bg(rgb(status_rgb(level))),
                                )
                                .child(div().text_xs().child(format!("{}  {}", n.type_id, text)))
                        }))
                    }),
            )
    }
}

fn stat_card(label: &str, value: &str, ok: bool) -> impl IntoElement {
    v_flex()
        .flex_1()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x3f3f46))
        .gap_1()
        .child(div().text_xs().child(SharedString::from(label.to_string())))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if ok { rgb(0x22c55e) } else { rgb(0xf59e0b) })
                .child(SharedString::from(value.to_string())),
        )
}
