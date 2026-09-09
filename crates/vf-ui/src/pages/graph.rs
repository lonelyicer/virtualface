use crate::canvas::{
    hit_node, hit_port, node_io_counts, paint_dots, paint_wire, port_dot, ports_ok, try_connect,
};
use crate::theme::{
    Camera, Drag, HEADER_H, NODE_W, PORT_H, Vec2, category_color, latest_snapshot, node_height,
    port_y, status_rgb, tag_color,
};
use crate::workspace::Workspace;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use vf_core::{NodeId, PortRef};
use vf_sdk::{Category, ParamKind};

impl Workspace {
    pub(crate) fn add_node_at(&mut self, type_id: &str, world: Vec2, cx: &mut Context<Self>) {
        let id = {
            let mut g = self.session.graph.lock();
            g.add_node(type_id, world.x, world.y, &self.session.registry)
        };
        self.selected = Some(id);
        self.session.recompile();
        cx.notify();
    }

    pub(crate) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.take() {
            self.session.graph.lock().remove_node(id);
            self.session.recompile();
        }
        cx.notify();
    }

    pub(crate) fn on_canvas_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let world = self.world_of(ev.position);
        if ev.button == MouseButton::Middle
            || (ev.button == MouseButton::Left && ev.modifiers.shift)
        {
            self.drag = Drag::Pan { last: ev.position };
            cx.notify();
            return;
        }
        if ev.button == MouseButton::Right {
            if let Some(ty) = self.pending_type.clone() {
                self.add_node_at(&ty, world, cx);
                self.pending_type = None;
            }
            return;
        }
        if ev.button != MouseButton::Left {
            return;
        }
        if let Some(ty) = self.pending_type.clone() {
            self.add_node_at(&ty, world, cx);
            self.pending_type = None;
            return;
        }
        let graph = self.session.graph.lock().clone();
        for n in graph.nodes.iter().rev() {
            let (ni, no) = node_io_counts(&self.session, n.id);
            if let Some((is_in, port)) = hit_port(n, world, ni, no) {
                if !is_in {
                    self.drag = Drag::Wire {
                        from: PortRef { node: n.id, port },
                        current: world,
                    };
                    self.selected = Some(n.id);
                    cx.notify();
                    return;
                } else {
                    self.session
                        .graph
                        .lock()
                        .disconnect(PortRef { node: n.id, port });
                    self.session.recompile();
                    cx.notify();
                    return;
                }
            }
            if hit_node(n, world, ni, no) {
                self.selected = Some(n.id);
                self.drag = Drag::Node {
                    id: n.id,
                    grab: Vec2::new(world.x - n.x, world.y - n.y),
                };
                cx.notify();
                return;
            }
        }
        self.selected = None;
        self.drag = Drag::Pan { last: ev.position };
        cx.notify();
    }

    pub(crate) fn on_canvas_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        match self.drag {
            Drag::None => {}
            Drag::Pan { last } => {
                let dx = f32::from(ev.position.x - last.x);
                let dy = f32::from(ev.position.y - last.y);
                self.camera.pan.x += dx;
                self.camera.pan.y += dy;
                self.drag = Drag::Pan { last: ev.position };
                cx.notify();
            }
            Drag::Node { id, grab } => {
                let world = self.world_of(ev.position);
                if let Some(n) = self.session.graph.lock().node_mut(id) {
                    n.x = world.x - grab.x;
                    n.y = world.y - grab.y;
                }
                cx.notify();
            }
            Drag::Wire { from, .. } => {
                self.drag = Drag::Wire {
                    from,
                    current: self.world_of(ev.position),
                };
                cx.notify();
            }
        }
    }

    pub(crate) fn on_canvas_up(&mut self, ev: &MouseUpEvent, cx: &mut Context<Self>) {
        if let Drag::Wire { from, .. } = self.drag {
            let world = self.world_of(ev.position);
            let graph = self.session.graph.lock().clone();
            for n in &graph.nodes {
                let (ni, no) = node_io_counts(&self.session, n.id);
                if let Some((is_in, port)) = hit_port(n, world, ni, no) {
                    if is_in {
                        let to = PortRef { node: n.id, port };
                        if ports_ok(&self.session, from, to) {
                            if let Err(e) = try_connect(&self.session, from, to) {
                                self.status = e;
                            }
                        } else {
                            self.status = "incompatible ports".into();
                        }
                    }
                }
            }
        }
        self.drag = Drag::None;
        cx.notify();
    }

    pub(crate) fn on_scroll(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let dy = match ev.delta {
            ScrollDelta::Pixels(p) => f32::from(p.y),
            ScrollDelta::Lines(l) => l.y * 24.0,
        };
        let loc = self.local(ev.position);
        let factor = if dy > 0.0 { 1.08 } else { 0.92 };
        self.camera
            .zoom_at(factor, Vec2::new(f32::from(loc.x), f32::from(loc.y)));
        cx.notify();
    }

    pub(crate) fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let pivot = Vec2::new(
            f32::from(self.canvas_bounds.size.width) * 0.5,
            f32::from(self.canvas_bounds.size.height) * 0.5,
        );
        self.camera.zoom_at(factor, pivot);
        cx.notify();
    }

    pub(crate) fn fit_view(&mut self, cx: &mut Context<Self>) {
        let graph = self.session.graph.lock().clone();
        if graph.nodes.is_empty() {
            self.camera = Camera::new();
            cx.notify();
            return;
        }
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for n in &graph.nodes {
            let (ni, no) = node_io_counts(&self.session, n.id);
            let h = node_height(ni, no);
            min_x = min_x.min(n.x);
            min_y = min_y.min(n.y);
            max_x = max_x.max(n.x + NODE_W);
            max_y = max_y.max(n.y + h);
        }
        let vw = f32::from(self.canvas_bounds.size.width).max(1.0);
        let vh = f32::from(self.canvas_bounds.size.height).max(1.0);
        let gw = (max_x - min_x).max(1.0);
        let gh = (max_y - min_y).max(1.0);
        let pad = 80.0;
        let zoom = ((vw - pad) / gw).min((vh - pad) / gh).clamp(0.2, 1.25);
        self.camera.zoom = zoom;
        self.camera.pan.x = (vw - gw * zoom) * 0.5 - min_x * zoom;
        self.camera.pan.y = (vh - gh * zoom) * 0.5 - min_y * zoom;
        cx.notify();
    }

    pub(crate) fn set_param(
        &mut self,
        id: NodeId,
        key: &str,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if let Some(n) = self.session.graph.lock().node_mut(id) {
            if let Some(obj) = n.params.as_object_mut() {
                obj.insert(key.to_string(), value.clone());
            }
        }
        self.session.engine.set_param(id, key.to_string(), value);
        cx.notify();
    }

    pub(crate) fn graph_page(
        &mut self,
        cx: &mut Context<Self>,
        bg: Hsla,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        h_flex()
            .id("graph-body")
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .child(self.palette(cx, surface, border, muted))
            .child(self.canvas_el(cx, bg, surface, border, muted))
            .when(self.selected.is_some(), |d| {
                d.child(self.inspector(cx, surface, border, muted))
            })
    }

    fn palette(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        let types = self.session.registry.all().to_vec();
        v_flex()
            .w(px(220.))
            .h_full()
            .p_2()
            .gap_1()
            .bg(surface)
            .border_r_1()
            .border_color(border)
            .child(div().px_1().text_xs().text_color(muted).child("节点"))
            .children(types.into_iter().map(|t| {
                let id = t.type_id.clone();
                let selected = self.pending_type.as_deref() == Some(id.as_str());
                let color = rgb(category_color(t.category));
                div()
                    .id(SharedString::from(format!("pal-{}", t.type_id)))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(color.opacity(0.25)))
                    .hover(|d| d.bg(color.opacity(0.15)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pending_type = Some(id.clone());
                        this.status = format!("click canvas to place {}", id);
                        cx.notify();
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(color))
                            .child(t.display_name.clone()),
                    )
            }))
    }

    fn canvas_el(
        &mut self,
        cx: &mut Context<Self>,
        bg: Hsla,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        let graph = self.session.graph.lock().clone();
        let cam = self.camera;
        let drag = self.drag;
        let selected = self.selected;
        let session = self.session.clone();
        let snap = latest_snapshot(&self.session);
        let this = cx.weak_entity();

        let mut node_els: Vec<AnyElement> = Vec::new();
        for n in &graph.nodes {
            node_els.push(self.node_el(n, &snap, selected, cam, cx).into_any_element());
        }

        div()
            .id("canvas")
            .flex_1()
            .relative()
            .min_w(px(200.))
            .overflow_hidden()
            .bg(bg)
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev, _, cx| this.on_canvas_down(ev, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, ev, _, cx| this.on_canvas_down(ev, cx)),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, ev, _, cx| this.on_canvas_down(ev, cx)),
            )
            .on_mouse_move(cx.listener(|this, ev, _, cx| this.on_canvas_move(ev, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev, _, cx| this.on_canvas_up(ev, cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, ev, _, cx| this.on_canvas_up(ev, cx)),
            )
            .on_scroll_wheel(cx.listener(|this, ev, _, cx| this.on_scroll(ev, cx)))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "delete" || ev.keystroke.key == "backspace" {
                    this.delete_selected(cx);
                }
            }))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        this.update(cx, |ws, _| {
                            ws.canvas_bounds = bounds;
                        })
                        .ok();
                        bounds
                    },
                    move |bounds, _, window, _| {
                        paint_dots(window, bounds, cam, muted.opacity(0.35));
                        for e in &graph.edges {
                            let Some(sn) = graph.node(e.from.node) else {
                                continue;
                            };
                            let Some(dn) = graph.node(e.to.node) else {
                                continue;
                            };
                            let x0 = sn.x + NODE_W;
                            let y0 = sn.y + port_y(e.from.port);
                            let x1 = dn.x;
                            let y1 = dn.y + port_y(e.to.port);
                            let color = session
                                .registry
                                .get(&sn.type_id)
                                .and_then(|t| t.outputs.get(e.from.port as usize))
                                .map(|p| rgb(tag_color(p.value_tag())))
                                .unwrap_or(rgb(0x64748b));
                            paint_wire(window, bounds, cam, x0, y0, x1, y1, color, false);
                        }
                        if let Drag::Wire { from, current } = drag {
                            if let Some(sn) = graph.node(from.node) {
                                paint_wire(
                                    window,
                                    bounds,
                                    cam,
                                    sn.x + NODE_W,
                                    sn.y + port_y(from.port),
                                    current.x,
                                    current.y,
                                    rgb(0xb1b1b7),
                                    true,
                                );
                            }
                        }
                    },
                )
                .size_full(),
            )
            .children(node_els)
            .child(self.graph_controls(cx, surface, border, muted))
    }

    fn graph_controls(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        v_flex()
            .absolute()
            .bottom_3()
            .left_3()
            .gap_2()
            .occlude()
            .child(
                v_flex()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(surface)
                    .overflow_hidden()
                    .child(control_btn("zoom-in", "+", cx, |this, _, _, cx| {
                        this.zoom_by(1.15, cx)
                    }))
                    .child(div().h(px(1.)).w_full().bg(border))
                    .child(control_btn("zoom-out", "−", cx, |this, _, _, cx| {
                        this.zoom_by(1.0 / 1.15, cx)
                    }))
                    .child(div().h(px(1.)).w_full().bg(border))
                    .child(control_btn("fit-view", "fit", cx, |this, _, _, cx| {
                        this.fit_view(cx)
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("滚轮缩放 · 拖动画布 · 选节点后点画布添加"),
            )
    }

    fn node_el(
        &self,
        n: &vf_core::GraphNode,
        snap: &vf_core::Snapshot,
        selected: Option<NodeId>,
        cam: Camera,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ty = self.session.registry.get(&n.type_id);
        let (n_in, n_out, title, cat) = if let Some(t) = ty {
            (
                t.inputs.len(),
                t.outputs.len(),
                t.display_name.clone(),
                t.category,
            )
        } else {
            (0, 0, format!("missing: {}", n.type_id), Category::Utility)
        };
        let h = node_height(n_in, n_out);
        let screen = cam.world_to_screen(n.x, n.y);
        let zoom = cam.zoom;
        let ns = snap.nodes.get(&n.id.0);
        let level = ns.map(|s| s.status_level).unwrap_or(3);
        let accent = rgb(category_color(cat));
        let sel = selected == Some(n.id);
        let font = px((11.0 * zoom).clamp(8.0, 14.0));

        let mut body: Vec<AnyElement> = Vec::new();
        let rows = n_in.max(n_out).max(1);
        for i in 0..rows {
            let in_name = ty
                .and_then(|t| t.inputs.get(i))
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let out_name = ty
                .and_then(|t| t.outputs.get(i))
                .map(|p| p.name.clone())
                .unwrap_or_default();
            body.push(
                h_flex()
                    .h(px(PORT_H * zoom))
                    .px(px(12.0 * zoom))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
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
                    )
                    .into_any_element(),
            );
        }

        let mut ports: Vec<AnyElement> = Vec::new();
        if let Some(t) = ty {
            for (i, p) in t.inputs.iter().enumerate() {
                ports.push(port_dot(true, i as u32, p, zoom).into_any_element());
            }
            for (i, p) in t.outputs.iter().enumerate() {
                ports.push(port_dot(false, i as u32, p, zoom).into_any_element());
            }
        }

        div()
            .absolute()
            .left(screen.x)
            .top(screen.y)
            .w(px(NODE_W * zoom))
            .h(px(h * zoom))
            .rounded(px(8.))
            .bg(rgb(0x1a1a22))
            .border_1()
            .border_color(if sel { rgb(0x3b82f6) } else { rgb(0x3f3f46) })
            .shadow_md()
            .child(
                h_flex()
                    .h(px(HEADER_H * zoom))
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0x3f3f46))
                    .child(div().w(px(4. * zoom)).h_full().bg(accent))
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
            .children(ports)
            .when(n.missing, |d| {
                d.child(
                    div()
                        .p_1()
                        .text_size(font)
                        .text_color(rgb(0xef4444))
                        .child("plugin missing"),
                )
            })
    }

    fn inspector(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        let snap = latest_snapshot(&self.session);
        v_flex()
            .w(px(280.))
            .h_full()
            .p_2()
            .gap_2()
            .bg(surface)
            .border_l_1()
            .border_color(border)
            .child(div().text_xs().text_color(muted).child("检查器"))
            .map(|el| {
                let Some(id) = self.selected else {
                    return el.child(div().text_xs().text_color(muted).child("选中一个节点"));
                };
                let g = self.session.graph.lock();
                let Some(n) = g.node(id).cloned() else {
                    return el.child(div().child("gone"));
                };
                drop(g);
                let ty = self.session.registry.get(&n.type_id).cloned();
                let ns = snap.nodes.get(&id.0).cloned();
                let mut el = el.child(
                    div().font_weight(FontWeight::MEDIUM).child(
                        ty.as_ref()
                            .map(|t| t.display_name.clone())
                            .unwrap_or(n.type_id.clone()),
                    ),
                );
                if let Some(st) = &ns {
                    el = el.child(
                        div()
                            .text_xs()
                            .text_color(rgb(status_rgb(st.status_level)))
                            .child(st.status_text.clone()),
                    );
                    for (i, v) in st.outputs.iter().enumerate() {
                        el = el.child(div().text_xs().child(format!("out[{i}]  {}", v.preview())));
                    }
                }
                if let Some(ty) = ty {
                    for p in &ty.params {
                        el = el.child(self.param_row(id, p, &n.params, cx));
                    }
                }
                el.child(
                    Button::new("insp-del")
                        .danger()
                        .label("删除节点")
                        .on_click(cx.listener(|this, _, _, cx| this.delete_selected(cx))),
                )
            })
    }

    fn param_row(
        &self,
        id: NodeId,
        p: &vf_sdk::ParamDef,
        params: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = p.key.clone();
        let cur = params
            .get(&p.key)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        match p.kind {
            ParamKind::Bool => {
                let on = cur.as_bool().unwrap_or(false);
                Switch::new(SharedString::from(format!("sw-{}-{}", id.0, key)))
                    .label(p.label.clone())
                    .checked(on)
                    .on_change(cx.listener(move |this, v: &bool, _, cx| {
                        this.set_param(id, &key, serde_json::json!(*v), cx);
                    }))
                    .into_any_element()
            }
            ParamKind::Button => Button::new(SharedString::from(format!("btn-{}-{}", id.0, key)))
                .label(p.label.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_param(id, &key, serde_json::json!(true), cx);
                }))
                .into_any_element(),
            ParamKind::Float | ParamKind::Int => {
                let v = cur
                    .as_f64()
                    .or_else(|| cur.as_i64().map(|i| i as f64))
                    .unwrap_or(0.0);
                let step = p.step.unwrap_or(0.1);
                let key_dec = key.clone();
                let key_inc = key.clone();
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(div().w(px(110.)).text_xs().child(p.label.clone()))
                    .child(
                        Button::new(SharedString::from(format!("d-{}-{}", id.0, key)))
                            .small()
                            .label("−")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_param(id, &key_dec, serde_json::json!(v - step), cx);
                            })),
                    )
                    .child(div().text_xs().w(px(48.)).child(format!("{v:.2}")))
                    .child(
                        Button::new(SharedString::from(format!("i-{}-{}", id.0, key)))
                            .small()
                            .label("+")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_param(id, &key_inc, serde_json::json!(v + step), cx);
                            })),
                    )
                    .into_any_element()
            }
            ParamKind::Enum => {
                let cur_s = cur.as_str().unwrap_or("").to_string();
                let opts = p.options.clone();
                let key2 = key.clone();
                h_flex()
                    .gap_1()
                    .child(div().text_xs().child(p.label.clone()))
                    .child(
                        Button::new(SharedString::from(format!("en-{}-{}", id.0, key)))
                            .small()
                            .label(if cur_s.is_empty() {
                                "…".into()
                            } else {
                                cur_s.clone()
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let idx = opts.iter().position(|o| o == &cur_s).unwrap_or(0);
                                let next = opts[(idx + 1) % opts.len().max(1)].clone();
                                this.set_param(id, &key2, serde_json::json!(next), cx);
                            })),
                    )
                    .into_any_element()
            }
            ParamKind::String => div()
                .text_xs()
                .child(format!("{}: {}", p.label, cur.as_str().unwrap_or("")))
                .into_any_element(),
        }
    }
}

fn control_btn(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<Workspace>,
    handler: impl Fn(&mut Workspace, &ClickEvent, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(32.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|d| d.bg(rgb(0x3f3f46)))
        .on_click(cx.listener(handler))
        .child(div().text_xs().child(label))
}
