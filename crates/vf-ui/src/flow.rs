//! ReactFlow viewport model on gpui.
//!
//! Mirrors `@xyflow/react`: a clipped pane, dotted background, bezier edges,
//! absolutely positioned nodes with left/right handles, and overlay controls.
//! Pan/zoom is the ReactFlow viewport transform `translate(x, y) scale(zoom)`.

use crate::i18n::{Locale, T, t_loc, tf_loc};
use crate::theme::{
    Camera, EDITOR_H, HEADER_H, NODE_W, PORT_H, PORT_R, Vec2, category_color, status_rgb, tag_color,
};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use image::{Frame, Rgba, RgbaImage};
use smallvec::smallvec;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use vf_abi::ports_compatible;
use vf_core::{Graph, GraphNode, PortRef, Session, Snapshot, VarType};
use vf_sdk::Category;

pub struct FlowEdge {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub color: Hsla,
    pub dashed: bool,
}

pub fn pane(bg: Hsla) -> Stateful<Div> {
    div()
        .id("react-flow")
        .flex_1()
        .w_full()
        .h_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .relative()
        .overflow_hidden()
        .bg(bg)
}

thread_local! {
    static DOT_TILES: RefCell<HashMap<(u32, u32), Arc<RenderImage>>> =
        RefCell::new(HashMap::new());
}

fn pack_bgra(color: Hsla) -> u32 {
    let c = color.to_rgb();
    let r = (c.r * 255.0) as u32;
    let g = (c.g * 255.0) as u32;
    let b = (c.b * 255.0) as u32;
    let a = (c.a * 255.0) as u32;
    b | (g << 8) | (r << 16) | (a << 24)
}

fn dot_tile(cell: u32, packed: u32) -> Arc<RenderImage> {
    DOT_TILES.with(|tiles| {
        let key = (cell, packed);
        if let Some(existing) = tiles.borrow().get(&key) {
            return existing.clone();
        }
        let cells = (256 / cell).max(1);
        let side = cells * cell;
        let b = (packed & 0xFF) as u8;
        let g = ((packed >> 8) & 0xFF) as u8;
        let r = ((packed >> 16) & 0xFF) as u8;
        let a = ((packed >> 24) & 0xFF) as u8;
        let dot = (cell / 8).clamp(1, 3);
        let mut img = RgbaImage::new(side, side);
        for cy in 0..cells {
            for cx in 0..cells {
                let x0 = cx * cell;
                let y0 = cy * cell;
                for dy in 0..dot {
                    for dx in 0..dot {
                        img.put_pixel(x0 + dx, y0 + dy, Rgba([b, g, r, a]));
                    }
                }
            }
        }
        let rendered = Arc::new(RenderImage::new(smallvec![Frame::new(img)]));
        tiles.borrow_mut().insert(key, rendered.clone());
        rendered
    })
}

/// GPU-tiled dot grid. gpui does not expose custom WGSL; this uses one cached
/// atlas tile sampled by the image shader (CSS `background-repeat`).
pub fn paint_background(window: &mut Window, bounds: Bounds<Pixels>, cam: Camera, color: Hsla) {
    let scale = window.scale_factor().max(0.5);
    let gap = 16.0 * cam.zoom;
    let cell = (gap * scale).round().clamp(8.0, 48.0) as u32;
    let packed = pack_bgra(color);
    let tile = dot_tile(cell, packed);
    let tile_dev = (256 / cell).max(1) * cell;
    let tile_px = tile_dev as f32 / scale;
    let gap_px = cell as f32 / scale;
    let origin_x = f32::from(bounds.origin.x);
    let origin_y = f32::from(bounds.origin.y);
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let ox = cam.pan.x.rem_euclid(gap_px);
    let oy = cam.pan.y.rem_euclid(gap_px);
    let corners = Corners::all(px(0.));
    let mut y = oy - tile_px;
    while y < h {
        let mut x = ox - tile_px;
        while x < w {
            let slot = Bounds {
                origin: point(px(origin_x + x), px(origin_y + y)),
                size: size(px(tile_px), px(tile_px)),
            };
            window
                .paint_image(slot, slot, corners, tile.clone(), 0, false)
                .ok();
            x += tile_px;
        }
        y += tile_px;
    }
}

/// `@xyflow/system` `getBezierPath` for Right → Left handles.
pub fn paint_edge(window: &mut Window, bounds: Bounds<Pixels>, cam: Camera, e: &FlowEdge) {
    let a = cam.world_to_screen(e.x0, e.y0);
    let b = cam.world_to_screen(e.x1, e.y1);
    let width = px((1.5 * cam.zoom.max(0.7)).clamp(1.0, 2.5));
    let mut builder = if e.dashed {
        PathBuilder::stroke(width).dash_array(&[px(5.), px(4.)])
    } else {
        PathBuilder::stroke(width)
    };
    let ax = bounds.origin.x + a.x;
    let ay = bounds.origin.y + a.y;
    let bx = bounds.origin.x + b.x;
    let by = bounds.origin.y + b.y;
    builder.move_to(point(ax, ay));
    let dx = (f32::from(bx) - f32::from(ax)).abs().max(50.0);
    builder.cubic_bezier_to(
        point(bx, by),
        point(ax + px(dx * 0.5), ay),
        point(bx - px(dx * 0.5), by),
    );
    if let Ok(path) = builder.build() {
        window.paint_path(path, e.color);
    }
}

pub struct ParamRow {
    pub name: String,
    pub tag: Option<u32>,
    pub editor: Option<AnyElement>,
}

pub fn handle(is_in: bool, pin_y: f32, color: u32, zoom: f32) -> impl IntoElement {
    let y = pin_y * zoom;
    let x = if is_in {
        px(-PORT_R * zoom)
    } else {
        px(NODE_W * zoom - PORT_R * zoom)
    };
    div()
        .absolute()
        .left(x)
        .top(px(y - PORT_R * zoom))
        .w(px(PORT_R * 2.0 * zoom))
        .h(px(PORT_R * 2.0 * zoom))
        .rounded_full()
        .bg(rgb(color))
        .border_1()
        .border_color(rgb(0xf8fafc))
}

pub struct NodeLayout {
    pub height: f32,
    pub in_ys: Vec<f32>,
    pub out_ys: Vec<f32>,
}

impl NodeLayout {
    pub fn pin_y(&self, is_in: bool, port: u32) -> f32 {
        let ys = if is_in { &self.in_ys } else { &self.out_ys };
        ys.get(port as usize)
            .copied()
            .unwrap_or(HEADER_H + PORT_H * 0.5)
    }
}

pub fn node_layout(session: &Session, n: &GraphNode, graph: &Graph) -> NodeLayout {
    let Some(ty) = session.registry.get(&n.type_id) else {
        return NodeLayout {
            height: HEADER_H + PORT_H + 8.0,
            in_ys: Vec::new(),
            out_ys: Vec::new(),
        };
    };
    let n_data = ty.inputs.len();
    let n_param = ty.pin_params().count();
    let n_left = n_data + n_param;
    let n_out = ty.outputs.len();
    let wired: HashSet<u32> = graph.incoming(n.id).map(|e| e.to.port).collect();
    let extra = if ty
        .params
        .iter()
        .any(|p| p.kind == vf_sdk::ParamKind::Button)
    {
        22.0
    } else {
        0.0
    };
    let n_rows = n_left.max(n_out).max(1);
    let mut y = HEADER_H;
    let mut in_ys = vec![0.0; n_left];
    let mut out_ys = vec![0.0; n_out];
    for i in 0..n_rows {
        let has_ed = i >= n_data && i < n_left && !wired.contains(&(i as u32));
        let h = if has_ed { PORT_H + EDITOR_H } else { PORT_H };
        let pin = y + PORT_H * 0.5;
        if i < n_left {
            in_ys[i] = pin;
        }
        if i < n_out {
            out_ys[i] = pin;
        }
        y += h;
    }
    NodeLayout {
        height: y + 8.0 + extra,
        in_ys,
        out_ys,
    }
}

pub fn node(
    n: &GraphNode,
    title: String,
    cat: Category,
    inputs: Vec<vf_core::PortType>,
    outputs: Vec<vf_core::PortType>,
    param_rows: Vec<ParamRow>,
    buttons: Vec<AnyElement>,
    selected: bool,
    missing: bool,
    level: u32,
    cam: Camera,
    loc: Locale,
) -> impl IntoElement {
    let n_in = inputs.len();
    let n_out = outputs.len();
    let extra = if buttons.is_empty() { 0.0 } else { 22.0 };
    let screen = cam.world_to_screen(n.x, n.y);
    let zoom = cam.zoom;
    let accent = rgb(category_color(cat));
    let font = px((11.0 * zoom).clamp(8.0, 14.0));
    let param_handles: Vec<(usize, u32)> = param_rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| row.tag.map(|tag| (n_in + i, tag)))
        .collect();
    let mut left_cells: Vec<(String, Option<AnyElement>)> =
        inputs.iter().map(|p| (p.name.clone(), None)).collect();
    for row in param_rows {
        left_cells.push((row.name, row.editor));
    }
    let n_rows = left_cells.len().max(n_out).max(1);
    let mut y = HEADER_H;
    let mut in_pin_ys = Vec::new();
    let mut out_pin_ys = Vec::new();
    for i in 0..n_rows {
        let has_ed = left_cells.get(i).is_some_and(|(_, e)| e.is_some());
        let h = if has_ed { PORT_H + EDITOR_H } else { PORT_H };
        let pin = y + PORT_H * 0.5;
        if i < left_cells.len() {
            in_pin_ys.push(pin);
        }
        if i < n_out {
            out_pin_ys.push(pin);
        }
        y += h;
    }
    let h = y + 8.0 + extra;
    let mut handles: Vec<AnyElement> = Vec::new();
    for (i, p) in inputs.iter().enumerate() {
        handles.push(handle(true, in_pin_ys[i], tag_color(p.value_tag()), zoom).into_any_element());
    }
    for (idx, tag) in param_handles {
        if let Some(&pin_y) = in_pin_ys.get(idx) {
            handles.push(handle(true, pin_y, tag, zoom).into_any_element());
        }
    }
    for (i, p) in outputs.iter().enumerate() {
        handles
            .push(handle(false, out_pin_ys[i], tag_color(p.value_tag()), zoom).into_any_element());
    }

    let mut body: Vec<AnyElement> = Vec::new();
    for i in 0..n_rows {
        let (in_name, editor) = left_cells
            .get_mut(i)
            .map(|(n, e)| (n.clone(), e.take()))
            .unwrap_or_default();
        let out_name = outputs.get(i).map(|p| p.name.clone()).unwrap_or_default();
        body.push(
            v_flex()
                .w_full()
                .child(
                    h_flex()
                        .h(px(PORT_H * zoom))
                        .px(px(10.0 * zoom))
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(font)
                                .text_color(rgb(0xa1a1aa))
                                .whitespace_nowrap()
                                .child(in_name),
                        )
                        .child(
                            div()
                                .text_size(font)
                                .text_color(rgb(0xa1a1aa))
                                .whitespace_nowrap()
                                .child(out_name),
                        ),
                )
                .children(editor.map(|e| {
                    div()
                        .w_full()
                        .h(px(EDITOR_H * zoom))
                        .px(px(10.0 * zoom))
                        .pb(px(2.0 * zoom))
                        .child(e)
                        .into_any_element()
                }))
                .into_any_element(),
        );
    }
    if !buttons.is_empty() {
        body.push(
            h_flex()
                .px(px(8.0 * zoom))
                .py(px(2.0 * zoom))
                .gap_1()
                .children(buttons)
                .into_any_element(),
        );
    }

    let radius = px((8.0 * zoom).max(4.0));
    let border = if selected {
        rgb(0x3b82f6)
    } else {
        rgb(0x3f3f46)
    };
    let node_w = px(NODE_W * zoom);
    let node_h = px(h * zoom);
    let accent_w = px(4.0 * zoom);
    let header_h = px(HEADER_H * zoom);
    div()
        .id(SharedString::from(format!("rf-node-{}", n.id.0)))
        .absolute()
        .left(screen.x)
        .top(screen.y)
        .w(node_w)
        .h(node_h)
        .rounded(radius)
        .shadow_md()
        .child(div().absolute().inset_0().rounded(radius).bg(rgb(0x1a1a22)))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(accent_w)
                .h(header_h)
                .overflow_hidden()
                .child(div().w(node_w).h(node_h).rounded(radius).bg(accent)),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(radius)
                .border_1()
                .border_color(border),
        )
        .child(
            v_flex()
                .absolute()
                .inset_0()
                .child(
                    h_flex()
                        .h(header_h)
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0x3f3f46))
                        .child(div().w(accent_w).h_full().flex_shrink_0())
                        .child(
                            h_flex()
                                .flex_1()
                                .px_2()
                                .gap_1()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(7.))
                                        .h(px(7.))
                                        .rounded_full()
                                        .bg(rgb(status_rgb(level))),
                                )
                                .child(
                                    div()
                                        .text_size(font)
                                        .font_weight(FontWeight::MEDIUM)
                                        .whitespace_nowrap()
                                        .child(title),
                                ),
                        ),
                )
                .children(body)
                .when(missing, |d| {
                    d.child(
                        div()
                            .p_1()
                            .text_size(font)
                            .text_color(rgb(0xef4444))
                            .child(t_loc(loc, T::PluginMissing)),
                    )
                }),
        )
        .children(handles)
}

pub fn snapshot_node(
    session: &Session,
    n: &GraphNode,
    snap: &Snapshot,
    selected: bool,
    cam: Camera,
    param_rows: Vec<ParamRow>,
    buttons: Vec<AnyElement>,
    loc: Locale,
) -> impl IntoElement {
    let ty = session.registry.get(&n.type_id);
    let mut title = ty
        .map(|t| t.display_name.clone())
        .unwrap_or_else(|| tf_loc(loc, T::MissingNode, &[("id", &n.type_id)]));
    if let Some((is_get, _)) = VarType::from_type_id(&n.type_id) {
        let name = n.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if !name.is_empty() {
            title = if is_get {
                tf_loc(loc, T::GetVar, &[("name", name)])
            } else {
                tf_loc(loc, T::SetVar, &[("name", name)])
            };
        }
    }
    let cat = ty.map(|t| t.category).unwrap_or(Category::Utility);
    let inputs = ty.map(|t| t.inputs.clone()).unwrap_or_default();
    let outputs = ty.map(|t| t.outputs.clone()).unwrap_or_default();
    let level = snap.nodes.get(&n.id.0).map(|s| s.status_level).unwrap_or(3);
    node(
        n, title, cat, inputs, outputs, param_rows, buttons, selected, n.missing, level, cam, loc,
    )
}

pub fn paint_marquee(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    cam: Camera,
    start: Vec2,
    current: Vec2,
) {
    let a = cam.world_to_screen(start.x, start.y);
    let b = cam.world_to_screen(current.x, current.y);
    let x0 = f32::from(a.x).min(f32::from(b.x));
    let y0 = f32::from(a.y).min(f32::from(b.y));
    let x1 = f32::from(a.x).max(f32::from(b.x));
    let y1 = f32::from(a.y).max(f32::from(b.y));
    let origin = point(bounds.origin.x + px(x0), bounds.origin.y + px(y0));
    let size = size(px(x1 - x0), px(y1 - y0));
    let r = Bounds { origin, size };
    window.paint_quad(fill(r, rgb(0x3b82f6).opacity(0.15)));
    let mut path = PathBuilder::stroke(px(1.));
    path.add_polygon(
        &[
            r.origin,
            point(r.origin.x + r.size.width, r.origin.y),
            point(r.origin.x + r.size.width, r.origin.y + r.size.height),
            point(r.origin.x, r.origin.y + r.size.height),
        ],
        true,
    );
    if let Ok(p) = path.build() {
        window.paint_path(p, rgb(0x3b82f6));
    }
}

pub fn hit_node(n: &GraphNode, world: Vec2, layout: &NodeLayout) -> bool {
    world.x >= n.x && world.x <= n.x + NODE_W && world.y >= n.y && world.y <= n.y + layout.height
}

pub fn node_intersects(
    n: &GraphNode,
    layout: &NodeLayout,
    minx: f32,
    miny: f32,
    maxx: f32,
    maxy: f32,
) -> bool {
    n.x < maxx && n.x + NODE_W > minx && n.y < maxy && n.y + layout.height > miny
}

pub fn hit_port(n: &GraphNode, world: Vec2, layout: &NodeLayout) -> Option<(bool, u32)> {
    let r = PORT_R + 4.0;
    for (i, &pin_y) in layout.in_ys.iter().enumerate() {
        let px_ = n.x;
        let py = n.y + pin_y;
        if (world.x - px_).hypot(world.y - py) <= r {
            return Some((true, i as u32));
        }
    }
    for (i, &pin_y) in layout.out_ys.iter().enumerate() {
        let px_ = n.x + NODE_W;
        let py = n.y + pin_y;
        if (world.x - px_).hypot(world.y - py) <= r {
            return Some((false, i as u32));
        }
    }
    None
}

pub fn try_connect(session: &Arc<Session>, from: PortRef, to: PortRef) -> Result<(), String> {
    let mut g = session.graph.lock();
    let previous = g.edges.iter().find(|e| e.to == to).cloned();
    g.disconnect(to);
    match g.connect(from, to, &session.registry) {
        Ok(()) => {
            drop(g);
            session.recompile();
            Ok(())
        }
        Err(e) => {
            if let Some(edge) = previous {
                g.edges.push(edge);
            }
            Err(e.to_string())
        }
    }
}

pub fn ports_ok(session: &Arc<Session>, from: PortRef, to: PortRef) -> bool {
    let g = session.graph.lock();
    let (Some(sn), Some(dn)) = (g.node(from.node), g.node(to.node)) else {
        return false;
    };
    let (Some(st), Some(dt)) = (
        session.registry.get(&sn.type_id),
        session.registry.get(&dn.type_id),
    ) else {
        return false;
    };
    let Some(sp) = st.outputs.get(from.port as usize) else {
        return false;
    };
    if let Some(dp) = dt.inputs.get(to.port as usize) {
        return ports_compatible(
            sp.value_tag(),
            Some(&sp.schema),
            dp.value_tag(),
            Some(&dp.schema),
        );
    }
    dt.input_tag(to.port)
        .map(|tag| ports_compatible(sp.value_tag(), Some(&sp.schema), tag, None))
        .unwrap_or(false)
}

pub fn collect_edges(session: &Arc<Session>, graph: &vf_core::Graph) -> Vec<FlowEdge> {
    let mut edges = Vec::new();
    for e in &graph.edges {
        let Some(sn) = graph.node(e.from.node) else {
            continue;
        };
        let Some(dn) = graph.node(e.to.node) else {
            continue;
        };
        let color = session
            .registry
            .get(&sn.type_id)
            .and_then(|t| t.outputs.get(e.from.port as usize))
            .map(|p| rgb(tag_color(p.value_tag())).into())
            .unwrap_or_else(|| rgb(0xb1b1b7).into());
        let src = node_layout(session, sn, graph);
        let dst = node_layout(session, dn, graph);
        edges.push(FlowEdge {
            x0: sn.x + NODE_W,
            y0: sn.y + src.pin_y(false, e.from.port),
            x1: dn.x,
            y1: dn.y + dst.pin_y(true, e.to.port),
            color,
            dashed: false,
        });
    }
    edges
}
