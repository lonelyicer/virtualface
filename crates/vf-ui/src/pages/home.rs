use crate::i18n::{T, t, tf};
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
        let name = self.graph_name(cx);

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
                    .child(t(cx, T::HomeStatus)),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(stat_card(
                        t(cx, T::HomeEngine),
                        if running {
                            t(cx, T::HomeRunning)
                        } else {
                            t(cx, T::HomeStopped)
                        },
                        running,
                    ))
                    .child(stat_card(
                        t(cx, T::HomeTick),
                        SharedString::from(snap.tick.to_string()),
                        running,
                    ))
                    .child(stat_card(
                        t(cx, T::HomeDrops),
                        SharedString::from(snap.drops.to_string()),
                        snap.drops == 0,
                    ))
                    .child(stat_card(
                        t(cx, T::HomeRate),
                        SharedString::from(format!("{:.0} Hz", graph.rate_hz)),
                        true,
                    )),
            )
            .child(
                GroupBox::new()
                    .id("home-engine")
                    .outline()
                    .title(t(cx, T::HomeEngine))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("home-start")
                                    .primary()
                                    .label(t(cx, T::HomeStart))
                                    .on_click(cx.listener(|this, _, _, cx| this.start_engine(cx))),
                            )
                            .child(
                                Button::new("home-stop")
                                    .label(t(cx, T::HomeStop))
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
                    .title(t(cx, T::HomeGraph))
                    .child(div().child(tf(cx, T::HomeGraphName, &[("name", &name)])))
                    .when(!self.graph_path.is_empty(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(self.graph_path.clone()),
                        )
                    })
                    .child(div().child(tf(
                        cx,
                        T::HomeGraphStats,
                        &[
                            ("nodes", &graph.nodes.len().to_string()),
                            ("edges", &graph.edges.len().to_string()),
                            ("missing", &missing.to_string()),
                        ],
                    ))),
            )
            .child(
                GroupBox::new()
                    .id("home-plugins")
                    .outline()
                    .title(t(cx, T::HomePlugins))
                    .child(div().child(tf(
                        cx,
                        T::HomePluginsLoaded,
                        &[
                            ("plugins", &plugin_n.to_string()),
                            ("types", &type_n.to_string()),
                        ],
                    )))
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
                    .title(t(cx, T::HomeNodeStatus))
                    .map(|box_| {
                        if graph.nodes.is_empty() {
                            return box_.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(t(cx, T::HomeGraphEmpty)),
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

fn stat_card(label: SharedString, value: SharedString, ok: bool) -> impl IntoElement {
    v_flex()
        .flex_1()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x3f3f46))
        .gap_1()
        .child(div().text_xs().child(label))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if ok { rgb(0x22c55e) } else { rgb(0xf59e0b) })
                .child(value),
        )
}
