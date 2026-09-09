use crate::theme::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::sync::Arc;
use vf_abi::ports_compatible;
use vf_core::{GraphNode, NodeId, PortRef, Session};

pub fn paint_dots(window: &mut Window, bounds: Bounds<Pixels>, cam: Camera, color: Hsla) {
    let gap = 18.0 * cam.zoom;
    if gap < 8.0 {
        return;
    }
    let r = (1.1 * cam.zoom).clamp(0.7, 1.5);
    let origin = bounds.origin;
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let mut builder = PathBuilder::fill();
    let ox = (cam.pan.x.rem_euclid(gap) + gap) % gap;
    let oy = (cam.pan.y.rem_euclid(gap) + gap) % gap;
    let mut y = oy;
    while y < h {
        let mut x = ox;
        while x < w {
            let px_ = f32::from(origin.x) + x;
            let py_ = f32::from(origin.y) + y;
            builder.add_polygon(
                &[
                    point(px(px_ - r), px(py_ - r)),
                    point(px(px_ + r), px(py_ - r)),
                    point(px(px_ + r), px(py_ + r)),
                    point(px(px_ - r), px(py_ + r)),
                ],
                true,
            );
            x += gap;
        }
        y += gap;
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

pub fn paint_wire(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    cam: Camera,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: impl Into<Hsla>,
    dashed: bool,
) {
    let color = color.into();
    let a = cam.world_to_screen(x0, y0);
    let b = cam.world_to_screen(x1, y1);
    let width = px((1.6 * cam.zoom.max(0.7)).clamp(1.0, 2.4));
    let mut builder = if dashed {
        PathBuilder::stroke(width).dash_array(&[px(6.), px(4.)])
    } else {
        PathBuilder::stroke(width)
    };
    let ax = bounds.origin.x + a.x;
    let ay = bounds.origin.y + a.y;
    let bx = bounds.origin.x + b.x;
    let by = bounds.origin.y + b.y;
    builder.move_to(point(ax, ay));
    let dx = (f32::from(bx) - f32::from(ax)).abs().max(48.0);
    builder.cubic_bezier_to(
        point(bx, by),
        point(ax + px(dx * 0.5), ay),
        point(bx - px(dx * 0.5), by),
    );
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

pub fn hit_node(n: &GraphNode, world: crate::theme::Vec2, n_in: usize, n_out: usize) -> bool {
    let h = node_height(n_in, n_out);
    world.x >= n.x && world.x <= n.x + NODE_W && world.y >= n.y && world.y <= n.y + h
}

pub fn hit_port(
    n: &GraphNode,
    world: crate::theme::Vec2,
    n_in: usize,
    n_out: usize,
) -> Option<(bool, u32)> {
    let r = PORT_R + 4.0;
    for i in 0..n_in as u32 {
        let px_ = n.x;
        let py = n.y + port_y(i);
        if (world.x - px_).hypot(world.y - py) <= r {
            return Some((true, i));
        }
    }
    for i in 0..n_out as u32 {
        let px_ = n.x + NODE_W;
        let py = n.y + port_y(i);
        if (world.x - px_).hypot(world.y - py) <= r {
            return Some((false, i));
        }
    }
    None
}

pub fn try_connect(session: &Arc<Session>, from: PortRef, to: PortRef) -> Result<(), String> {
    let mut g = session.graph.lock();
    g.connect(from, to, &session.registry)
        .map_err(|e| e.to_string())?;
    drop(g);
    session.recompile();
    Ok(())
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
    let Some(dp) = dt.inputs.get(to.port as usize) else {
        return false;
    };
    ports_compatible(
        sp.value_tag(),
        Some(&sp.schema),
        dp.value_tag(),
        Some(&dp.schema),
    )
}

pub fn node_io_counts(session: &Arc<Session>, id: NodeId) -> (usize, usize) {
    let g = session.graph.lock();
    let Some(n) = g.node(id) else {
        return (0, 0);
    };
    if let Some(t) = session.registry.get(&n.type_id) {
        (t.inputs.len(), t.outputs.len())
    } else {
        (0, 0)
    }
}

pub fn port_dot(is_in: bool, i: u32, p: &vf_core::PortType, zoom: f32) -> impl IntoElement {
    let y = port_y(i) * zoom;
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
        .bg(rgb(tag_color(p.value_tag())))
        .border_1()
        .border_color(rgb(0xf8fafc))
}
