use crate::i18n::{T, t};
use crate::theme::{latest_snapshot, status_rgb};
use crate::workspace::Workspace;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::BTreeMap;
use vf_core::{Graph, NodeId, Snapshot};

struct PluginIo {
    name: String,
    in_bytes: usize,
    out_bytes: usize,
    status: u32,
}

impl Workspace {
    pub(crate) fn home_page(&self, cx: &mut Context<Self>, muted: Hsla) -> impl IntoElement {
        let snap = latest_snapshot(&self.session);
        let graph = self.session.graph.lock().clone();
        let running = snap.running;
        let plugins = active_plugin_io(&graph, &snap, &self.session);
        let total_in: usize = plugins.iter().map(|p| p.in_bytes).sum();
        let total_out: usize = plugins.iter().map(|p| p.out_bytes).sum();
        let dt = snap.dt_us;

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
                            .text_color(if running { rgb(0x22c55e).into() } else { muted })
                            .child(if running {
                                t(cx, T::HomeRunning)
                            } else {
                                t(cx, T::HomeStopped)
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(stat_card(
                        t(cx, T::HomeDataIn),
                        format_volume(total_in, dt, running),
                        running && total_in > 0,
                    ))
                    .child(stat_card(
                        t(cx, T::HomeDataOut),
                        format_volume(total_out, dt, running),
                        running && total_out > 0,
                    )),
            )
            .child(
                GroupBox::new()
                    .id("home-plugins")
                    .outline()
                    .title(t(cx, T::HomeActivePlugins))
                    .child(io_header(cx, muted))
                    .map(|box_| {
                        if plugins.is_empty() {
                            box_.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(t(cx, T::HomeNoActive)),
                            )
                        } else {
                            box_.children(
                                plugins
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, p)| plugin_row(i, p, dt, running, muted)),
                            )
                        }
                    }),
            )
    }
}

fn active_plugin_io(graph: &Graph, snap: &Snapshot, session: &vf_core::Session) -> Vec<PluginIo> {
    let mut by_id: BTreeMap<String, PluginIo> = BTreeMap::new();
    for n in &graph.nodes {
        if n.missing {
            continue;
        }
        let Some(ty) = session.registry.get(&n.type_id) else {
            continue;
        };
        if ty.plugin_id == "virtualface" {
            continue;
        }
        let out_bytes = snap
            .nodes
            .get(&n.id.0)
            .map(|s| s.outputs.iter().map(|v| v.payload_bytes()).sum())
            .unwrap_or(0);
        let in_bytes = incoming_bytes(graph, snap, n.id);
        let status = snap.nodes.get(&n.id.0).map(|s| s.status_level).unwrap_or(3);
        let entry = by_id.entry(ty.plugin_id.clone()).or_insert(PluginIo {
            name: ty.plugin_name.clone(),
            in_bytes: 0,
            out_bytes: 0,
            status: 3,
        });
        if entry.name.is_empty() {
            entry.name = ty.plugin_name.clone();
        }
        entry.in_bytes += in_bytes;
        entry.out_bytes += out_bytes;
        entry.status = worse_status(entry.status, status);
    }
    by_id.into_values().collect()
}

fn worse_status(a: u32, b: u32) -> u32 {
    fn rank(s: u32) -> u32 {
        match s {
            2 => 3,
            1 => 2,
            0 => 1,
            _ => 0,
        }
    }
    if rank(b) > rank(a) { b } else { a }
}

fn incoming_bytes(graph: &Graph, snap: &Snapshot, id: NodeId) -> usize {
    graph
        .incoming(id)
        .map(|e| {
            snap.nodes
                .get(&e.from.node.0)
                .and_then(|n| n.outputs.get(e.from.port as usize))
                .map(|v| v.payload_bytes())
                .unwrap_or(0)
        })
        .sum()
}

fn io_header(cx: &App, muted: Hsla) -> impl IntoElement {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .text_xs()
        .text_color(muted)
        .child(div().flex_1().min_w(px(0.)).child(t(cx, T::HomePlugins)))
        .child(div().w(px(140.)).child(t(cx, T::HomeDataIn)))
        .child(div().w(px(140.)).child(t(cx, T::HomeDataOut)))
}

fn plugin_row(
    i: usize,
    plugin: PluginIo,
    dt_us: u64,
    running: bool,
    muted: Hsla,
) -> impl IntoElement {
    h_flex()
        .id(("home-plugin", i))
        .w_full()
        .gap_3()
        .items_center()
        .py_1()
        .child(
            h_flex()
                .flex_1()
                .min_w(px(0.))
                .gap_2()
                .items_center()
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "home-plugin-{i}-st-{}",
                            plugin.status
                        )))
                        .w(px(8.))
                        .h(px(8.))
                        .rounded_full()
                        .bg(rgb(status_rgb(plugin.status))),
                )
                .child(
                    div()
                        .text_xs()
                        .truncate()
                        .child(SharedString::from(plugin.name)),
                ),
        )
        .child(
            div()
                .w(px(140.))
                .text_xs()
                .text_color(muted)
                .child(format_volume(plugin.in_bytes, dt_us, running)),
        )
        .child(
            div()
                .w(px(140.))
                .text_xs()
                .text_color(muted)
                .child(format_volume(plugin.out_bytes, dt_us, running)),
        )
}

fn format_volume(bytes: usize, dt_us: u64, running: bool) -> SharedString {
    let size = format_bytes(bytes);
    if running && dt_us > 0 {
        let per_sec = (bytes as u128).saturating_mul(1_000_000) / dt_us as u128;
        SharedString::from(format!("{size}  ·  {}/s", format_bytes(per_sec as usize)))
    } else {
        SharedString::from(size)
    }
}

fn format_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
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
