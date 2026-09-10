use crate::flow::{
    self, FlowEdge, ParamRow, collect_edges, hit_node, hit_port, node_intersects, node_layout,
    paint_background, paint_edge, paint_marquee, ports_ok, snapshot_node, try_connect,
};
use crate::i18n::{Locale, T, category_key, locale, t, t_loc, tf, tf_loc, var_type_key};
use crate::theme::{
    Camera, Drag, EDITOR_H, HEADER_H, NODE_W, Vec2, category_color, latest_snapshot,
};
use crate::workspace::{AddMenu, VarDialog, Workspace};
use gpui_kit::base::ElementExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::{
    Disableable, Icon, IconName, IndexPath, Sizable, h_flex, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::HashSet;
use vf_core::{GraphVar, NodeId, PortRef, VarType};
use vf_sdk::{Category, ParamKind};

const RMB_PAN_PX: f32 = 4.0;
const MARQUEE_CLICK_PX: f32 = 4.0;

#[derive(Clone)]
struct VarDrag {
    name: String,
    ty: VarType,
}

struct VarDragGhost {
    name: String,
    ty: VarType,
}

impl Render for VarDragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x27272a))
            .border_1()
            .border_color(rgb(0x52525b))
            .items_center()
            .child(
                div()
                    .w(px(8.))
                    .h(px(8.))
                    .rounded_full()
                    .bg(rgb(self.ty.color())),
            )
            .child(div().text_xs().child(self.name.clone()))
    }
}

#[derive(Clone, PartialEq)]
struct VarTypeChoice {
    ty: VarType,
    title: SharedString,
}

impl SearchableListItem for VarTypeChoice {
    type Value = VarType;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.ty
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct BoolChoice(bool);

impl SearchableListItem for BoolChoice {
    type Value = BoolChoice;

    fn title(&self) -> SharedString {
        SharedString::from(if self.0 { "true" } else { "false" })
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

pub(crate) struct VarCreateForm {
    name: Entity<InputState>,
    value: Entity<InputState>,
    ty: VarType,
    type_select: Entity<SelectState<SearchableVec<VarTypeChoice>>>,
    bool_select: Entity<SelectState<SearchableVec<BoolChoice>>>,
    _type_sub: Option<Subscription>,
}

impl VarCreateForm {
    fn apply_type(&mut self, ty: VarType, window: &mut Window, cx: &mut Context<Self>) {
        if self.ty == ty {
            return;
        }
        self.ty = ty;
        if ty == VarType::Bool {
            self.bool_select.update(cx, |s, cx| {
                s.set_selected_value(&BoolChoice(false), window, cx);
            });
        } else {
            self.value.update(cx, |s, cx| {
                s.set_value(default_value_str(ty), window, cx);
            });
        }
        cx.notify();
    }

    fn current_ty(&self, cx: &App) -> VarType {
        self.type_select
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(self.ty)
    }

    fn current_value(&self, cx: &App) -> Option<serde_json::Value> {
        let ty = self.current_ty(cx);
        if ty == VarType::Bool {
            let b = self
                .bool_select
                .read(cx)
                .selected_value()
                .map(|c| c.0)
                .unwrap_or(false);
            Some(serde_json::json!(b))
        } else {
            parse_var_value(ty, &self.value.read(cx).value())
        }
    }
}

impl Render for VarCreateForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ty = self.ty;
        v_flex()
            .gap_2()
            .w_full()
            .child(div().text_xs().child(t(cx, T::GraphName)))
            .child(Input::new(&self.name).id("var-create-name"))
            .child(div().text_xs().child(t(cx, T::GraphType)))
            .child(
                div().w_full().child(
                    Select::new(&self.type_select)
                        .id("var-create-type")
                        .small()
                        .w_full(),
                ),
            )
            .child(div().text_xs().child(t(cx, T::GraphValue)))
            .when(ty == VarType::Bool, |d| {
                d.child(
                    div().w_full().child(
                        Select::new(&self.bool_select)
                            .id("var-create-bool")
                            .small()
                            .w_full(),
                    ),
                )
            })
            .when(ty != VarType::Bool, |d| {
                d.child(Input::new(&self.value).id("var-create-value"))
            })
    }
}

#[derive(Clone)]
enum MenuEntry {
    Header(String),
    Place {
        type_id: String,
        label: String,
        color: u32,
    },
    Get {
        name: String,
        ty: VarType,
    },
}

impl MenuEntry {
    fn actionable(&self) -> bool {
        !matches!(self, MenuEntry::Header(_))
    }
}

impl Workspace {
    pub(crate) fn add_node_at(
        &mut self,
        type_id: &str,
        world: Vec2,
        cx: &mut Context<Self>,
    ) -> NodeId {
        let before = self.snapshot_graph();
        let id = self.add_node_at_raw(type_id, world);
        self.select_only(id);
        self.close_add_menu();
        self.session.recompile();
        self.commit_graph_change(before, cx);
        id
    }

    fn add_node_at_raw(&mut self, type_id: &str, world: Vec2) -> NodeId {
        self.session
            .graph
            .lock()
            .add_node(type_id, world.x, world.y, &self.session.registry)
    }

    pub(crate) fn close_add_menu(&mut self) {
        self.add_menu = None;
        self.menu_search = None;
        self.menu_sub = None;
    }

    fn add_get_at(&mut self, name: &str, ty: VarType, world: Vec2, cx: &mut Context<Self>) {
        let before = self.snapshot_graph();
        let id = self.add_node_at_raw(ty.get_type_id(), world);
        self.select_only(id);
        self.close_add_menu();
        if let Some(n) = self.session.graph.lock().node_mut(id) {
            if let Some(obj) = n.params.as_object_mut() {
                obj.insert("name".into(), serde_json::json!(name));
            }
        }
        self.session
            .engine
            .set_param(id, "name".into(), serde_json::json!(name));
        self.session.recompile();
        self.commit_graph_change(before, cx);
    }

    fn select_only(&mut self, id: NodeId) {
        self.selected.clear();
        self.selected.insert(id);
        self.primary = Some(id);
    }

    fn sync_primary(&mut self) {
        if let Some(id) = self.primary {
            if self.selected.contains(&id) {
                return;
            }
        }
        self.primary = self.selected.iter().copied().next();
    }

    pub(crate) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<NodeId> = self.selected.drain().collect();
        self.primary = None;
        if !ids.is_empty() {
            let before = self.snapshot_graph();
            let mut g = self.session.graph.lock();
            for id in ids {
                g.remove_node(id);
            }
            drop(g);
            self.session.recompile();
            self.commit_graph_change(before, cx);
        }
        cx.notify();
    }

    fn break_pin(&mut self, port: PortRef, is_in: bool, cx: &mut Context<Self>) {
        let before = self.snapshot_graph();
        let mut g = self.session.graph.lock();
        if is_in {
            g.disconnect(port);
        } else {
            g.disconnect_from(port);
        }
        drop(g);
        self.session.recompile();
        self.commit_graph_change(before, cx);
    }

    fn apply_marquee(&mut self, start: Vec2, current: Vec2, additive: bool) {
        let a = self.camera.world_to_screen(start.x, start.y);
        let b = self.camera.world_to_screen(current.x, current.y);
        let dx = (f32::from(a.x) - f32::from(b.x)).abs();
        let dy = (f32::from(a.y) - f32::from(b.y)).abs();
        if dx < MARQUEE_CLICK_PX && dy < MARQUEE_CLICK_PX {
            if !additive {
                self.selected.clear();
                self.primary = None;
            }
            return;
        }
        let minx = start.x.min(current.x);
        let miny = start.y.min(current.y);
        let maxx = start.x.max(current.x);
        let maxy = start.y.max(current.y);
        let graph = self.session.graph.lock().clone();
        let mut hits = HashSet::new();
        for n in &graph.nodes {
            let layout = node_layout(&self.session, n, &graph);
            if node_intersects(n, &layout, minx, miny, maxx, maxy) {
                hits.insert(n.id);
            }
        }
        if additive {
            self.selected.extend(hits);
        } else {
            self.selected = hits;
        }
        self.sync_primary();
    }

    fn finish_wire(&mut self, from: PortRef, output: bool, pos: Point<Pixels>, cx: &App) {
        let world = self.world_of(pos);
        let graph = self.session.graph.lock().clone();
        for n in &graph.nodes {
            let layout = node_layout(&self.session, n, &graph);
            let Some((is_in, port)) = hit_port(n, world, &layout) else {
                continue;
            };
            let hit = PortRef { node: n.id, port };
            let (src, dst) = if output {
                if !is_in {
                    return;
                }
                (from, hit)
            } else {
                if is_in {
                    return;
                }
                (hit, from)
            };
            if src.node == dst.node {
                return;
            }
            if ports_ok(&self.session, src, dst) {
                if let Err(e) = try_connect(&self.session, src, dst) {
                    self.status = e;
                }
            } else {
                self.status = t(cx, T::IncompatiblePorts).to_string();
            }
            return;
        }
    }

    fn on_rmb_click(
        &mut self,
        origin: Point<Pixels>,
        world: Vec2,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let graph = self.session.graph.lock().clone();
        for n in graph.nodes.iter().rev() {
            let layout = node_layout(&self.session, n, &graph);
            if hit_node(n, world, &layout) {
                self.select_only(n.id);
                return;
            }
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, T::GraphSearch)));
        search.update(cx, |s, cx| s.focus(window, cx));
        self.menu_sub = Some(cx.subscribe_in(
            &search,
            window,
            |this, _input, ev: &InputEvent, _window, cx| match ev {
                InputEvent::Change => {
                    if let Some(menu) = this.add_menu.as_mut() {
                        menu.highlight = 0;
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.confirm_menu(cx),
                _ => {}
            },
        ));
        self.menu_search = Some(search);
        self.add_menu = Some(AddMenu {
            pos: self.local(origin),
            world,
            highlight: 0,
        });
    }

    pub(crate) fn on_canvas_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        self.close_add_menu();
        let world = self.world_of(ev.position);

        if ev.button == MouseButton::Middle {
            self.drag = Drag::Pan { last: ev.position };
            cx.notify();
            return;
        }
        if ev.button == MouseButton::Right {
            self.drag = Drag::Rmb {
                origin: ev.position,
                last: ev.position,
                world,
            };
            cx.notify();
            return;
        }
        if ev.button != MouseButton::Left {
            return;
        }

        let graph = self.session.graph.lock().clone();
        for n in graph.nodes.iter().rev() {
            let layout = node_layout(&self.session, n, &graph);
            if let Some((is_in, port)) = hit_port(n, world, &layout) {
                let pin = PortRef { node: n.id, port };
                let wirable = self
                    .session
                    .registry
                    .get(&n.type_id)
                    .map(|t| !is_in || t.input_is_wirable(port))
                    .unwrap_or(true);
                if !wirable && !ev.modifiers.alt {
                    // unwirable param row: let pin widgets handle the click
                } else {
                    if ev.modifiers.alt {
                        self.break_pin(pin, is_in, cx);
                        cx.notify();
                        return;
                    }
                    self.begin_graph_gesture();
                    if is_in {
                        let source = self.session.graph.lock().source_of(pin);
                        if let Some(from) = source {
                            self.session.graph.lock().disconnect(pin);
                            self.session.recompile();
                            self.drag = Drag::Wire {
                                from,
                                output: true,
                                current: world,
                            };
                        } else {
                            self.drag = Drag::Wire {
                                from: pin,
                                output: false,
                                current: world,
                            };
                        }
                    } else {
                        self.drag = Drag::Wire {
                            from: pin,
                            output: true,
                            current: world,
                        };
                    }
                    cx.notify();
                    return;
                }
            }
            if hit_node(n, world, &layout) {
                if ev.modifiers.secondary() {
                    if self.selected.contains(&n.id) {
                        self.selected.remove(&n.id);
                        self.sync_primary();
                        self.drag = Drag::None;
                    } else {
                        self.selected.insert(n.id);
                        self.primary = Some(n.id);
                        self.begin_graph_gesture();
                        self.drag = Drag::Nodes { last: world };
                    }
                } else {
                    if !self.selected.contains(&n.id) {
                        self.select_only(n.id);
                    } else {
                        self.primary = Some(n.id);
                    }
                    self.begin_graph_gesture();
                    self.drag = Drag::Nodes { last: world };
                }
                cx.notify();
                return;
            }
        }

        self.drag = Drag::Marquee {
            start: world,
            current: world,
            additive: ev.modifiers.secondary() || ev.modifiers.shift,
        };
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
            Drag::Nodes { last } => {
                let world = self.world_of(ev.position);
                let dx = world.x - last.x;
                let dy = world.y - last.y;
                let ids: Vec<NodeId> = self.selected.iter().copied().collect();
                {
                    let mut g = self.session.graph.lock();
                    for id in ids {
                        if let Some(n) = g.node_mut(id) {
                            n.x += dx;
                            n.y += dy;
                        }
                    }
                }
                self.drag = Drag::Nodes { last: world };
                cx.notify();
            }
            Drag::Wire { from, output, .. } => {
                self.drag = Drag::Wire {
                    from,
                    output,
                    current: self.world_of(ev.position),
                };
                cx.notify();
            }
            Drag::Marquee {
                start, additive, ..
            } => {
                self.drag = Drag::Marquee {
                    start,
                    current: self.world_of(ev.position),
                    additive,
                };
                cx.notify();
            }
            Drag::Rmb {
                origin,
                last,
                world,
            } => {
                let dx = f32::from(ev.position.x - origin.x);
                let dy = f32::from(ev.position.y - origin.y);
                if dx.hypot(dy) > RMB_PAN_PX {
                    self.camera.pan.x += f32::from(ev.position.x - last.x);
                    self.camera.pan.y += f32::from(ev.position.y - last.y);
                    self.drag = Drag::Pan { last: ev.position };
                } else {
                    self.drag = Drag::Rmb {
                        origin,
                        last: ev.position,
                        world,
                    };
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn on_canvas_up(
        &mut self,
        ev: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.drag {
            Drag::Wire { from, output, .. } if ev.button == MouseButton::Left => {
                self.finish_wire(from, output, ev.position, cx);
                self.drag = Drag::None;
                self.end_graph_gesture(cx);
            }
            Drag::Marquee {
                start,
                current,
                additive,
            } if ev.button == MouseButton::Left => {
                self.apply_marquee(start, current, additive);
                self.drag = Drag::None;
            }
            Drag::Nodes { .. } if ev.button == MouseButton::Left => {
                self.drag = Drag::None;
                self.end_graph_gesture(cx);
            }
            Drag::Rmb { origin, world, .. } if ev.button == MouseButton::Right => {
                self.on_rmb_click(origin, world, window, cx);
                self.drag = Drag::None;
            }
            Drag::Pan { .. }
                if ev.button == MouseButton::Right || ev.button == MouseButton::Middle =>
            {
                self.drag = Drag::None;
            }
            _ => {}
        }
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
            let h = node_layout(&self.session, n, &graph).height;
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
        let before = self.snapshot_graph();
        if let Some(n) = self.session.graph.lock().node_mut(id) {
            if let Some(obj) = n.params.as_object_mut() {
                obj.insert(key.to_string(), value.clone());
            }
        }
        self.session.engine.set_param(id, key.to_string(), value);
        self.commit_graph_change(before, cx);
        cx.notify();
    }

    fn archive_picker(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        let name = self.graph_name(cx);
        let open = self.archive_menu_open;
        let current = self.archives.current.clone();
        let items: Vec<(String, String)> = self
            .archives
            .items
            .iter()
            .map(|item| (item.id.clone(), item.name.clone()))
            .collect();
        let menu_origin = point(
            self.archive_picker_bounds.origin.x,
            self.archive_picker_bounds.origin.y + self.archive_picker_bounds.size.height + px(4.),
        );
        let menu_w = self.archive_picker_bounds.size.width.max(px(200.));
        let view = cx.entity();

        div()
            .id("graph-archive-picker")
            .relative()
            .flex_shrink_0()
            .w(px(240.))
            .on_prepaint(move |bounds, _, cx| {
                let _ = view.update(cx, |this, _| {
                    this.archive_picker_bounds = bounds;
                });
            })
            .child(
                h_flex()
                    .id("graph-archive-trigger")
                    .w_full()
                    .h_6()
                    .px_2()
                    .gap_1()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(surface)
                    .cursor_pointer()
                    .occlude()
                    .hover(|d| d.bg(rgb(0x27272a)))
                    .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count() >= 2 {
                            this.prompt_rename_graph_archive(window, cx);
                            return;
                        }
                        this.archive_menu_open = !this.archive_menu_open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .truncate()
                            .text_sm()
                            .child(name),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .xsmall()
                            .text_color(muted),
                    ),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        anchored()
                            .position(menu_origin)
                            .anchor(Anchor::TopLeft)
                            .child(
                                v_flex()
                                    .id("graph-archive-menu")
                                    .w(menu_w)
                                    .max_h(px(280.))
                                    .occlude()
                                    .overflow_y_scroll()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(border)
                                    .bg(surface)
                                    .shadow_md()
                                    .p_1()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_mouse_down_out(cx.listener(
                                        |this, ev: &MouseDownEvent, _, cx| {
                                            if this.archive_picker_bounds.contains(&ev.position) {
                                                return;
                                            }
                                            this.archive_menu_open = false;
                                            cx.notify();
                                        },
                                    ))
                                    .children(items.into_iter().map(|(id, item_name)| {
                                        let selected = id == current;
                                        h_flex()
                                            .id(SharedString::from(format!(
                                                "graph-archive-item-{id}"
                                            )))
                                            .w_full()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .cursor_pointer()
                                            .when(selected, |d| d.bg(rgb(0x3f3f46)))
                                            .hover(|d| d.bg(rgb(0x27272a)))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.switch_archive(id.clone(), cx);
                                            }))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.))
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .truncate()
                                                    .text_sm()
                                                    .child(item_name),
                                            )
                                    })),
                            ),
                    )
                    .with_priority(100),
                )
            })
    }

    pub(crate) fn graph_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        bg: Hsla,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        v_flex()
            .id("graph-page")
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_6()
            .gap_2()
            .child(
                h_flex()
                    .id("graph-file-bar")
                    .flex_shrink_0()
                    .w_full()
                    .min_w(px(0.))
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(self.archive_picker(cx, surface, border, muted))
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_shrink_0()
                            .justify_end()
                            .child(
                                Button::new("graph-import")
                                    .small()
                                    .label(t(cx, T::GraphImport))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pick_and_import_graph(cx)
                                    })),
                            )
                            .child(
                                Button::new("graph-export")
                                    .small()
                                    .label(t(cx, T::GraphExport))
                                    .on_click(cx.listener(|this, _, _, cx| this.export_graph(cx))),
                            )
                            .child(
                                Button::new("graph-new")
                                    .small()
                                    .label(t(cx, T::GraphNew))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.prompt_new_graph_archive(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("graph-delete")
                                    .small()
                                    .label(t(cx, T::GraphDelete))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.prompt_delete_graph_archive(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("graph-undo")
                                    .small()
                                    .label(t(cx, T::GraphUndo))
                                    .disabled(self.undo_stack.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| this.undo_graph(cx))),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("graph-card")
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .overflow_hidden()
                    .child(self.canvas_el(window, cx, bg, surface, border, muted)),
            )
    }

    fn variables_card(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        let vars = self.session.graph.lock().variables.clone();
        let open = self.vars_open;
        let rows: Vec<AnyElement> = if !open {
            Vec::new()
        } else if vars.is_empty() {
            vec![
                div()
                    .px_1()
                    .py_1()
                    .text_xs()
                    .text_color(muted)
                    .child(t(cx, T::GraphVarsHint))
                    .into_any_element(),
            ]
        } else {
            vars.into_iter()
                .map(|v| var_row(v, muted, cx).into_any_element())
                .collect()
        };
        v_flex()
            .id("var-card")
            .absolute()
            .bottom_3()
            .right_3()
            .w(px(220.))
            .occlude()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(surface)
            .shadow_md()
            .child(
                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .items_center()
                    .gap_1()
                    .child(
                        h_flex()
                            .id("var-toggle")
                            .flex_1()
                            .min_w(px(0.))
                            .gap_1()
                            .items_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.vars_open = !this.vars_open;
                                cx.notify();
                            }))
                            .child(div().text_xs().child(t(cx, T::GraphVariables)))
                            .child(div().text_xs().text_color(muted).child(if open {
                                "▾"
                            } else {
                                "▸"
                            })),
                    )
                    .child(
                        div()
                            .id("var-add")
                            .px_2()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(0x3f3f46)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_var_dialog(
                                    window,
                                    cx,
                                    VarDialog::Create { preset: None },
                                );
                            }))
                            .child("+"),
                    ),
            )
            .when(open, |el| {
                el.child(
                    v_flex()
                        .id("var-list")
                        .max_h(px(220.))
                        .overflow_y_scroll()
                        .p_1()
                        .gap_1()
                        .children(rows),
                )
            })
    }

    fn canvas_el(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        bg: Hsla,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> impl IntoElement {
        let graph = self.session.graph.lock().clone();
        let cam = self.camera;
        let drag = self.drag;
        let selected = self.selected.clone();
        let session = self.session.clone();
        let snap = latest_snapshot(&self.session);
        let this = cx.weak_entity();
        let mut edges = collect_edges(&session, &graph);
        if let Drag::Wire {
            from,
            output,
            current,
        } = drag
        {
            if let Some(sn) = graph.node(from.node) {
                let py = sn.y + node_layout(&session, sn, &graph).pin_y(!output, from.port);
                let (x0, y0, x1, y1) = if output {
                    (sn.x + NODE_W, py, current.x, current.y)
                } else {
                    (current.x, current.y, sn.x, py)
                };
                edges.push(FlowEdge {
                    x0,
                    y0,
                    x1,
                    y1,
                    color: rgb(0xb1b1b7).into(),
                    dashed: true,
                });
            }
        }

        let mut node_els: Vec<AnyElement> = Vec::new();
        for n in &graph.nodes {
            let (rows, buttons) = self.param_rows_for(n, window, cx);
            node_els.push(
                snapshot_node(
                    &session,
                    n,
                    &snap,
                    selected.contains(&n.id),
                    cam,
                    rows,
                    buttons,
                    locale(cx),
                )
                .into_any_element(),
            );
        }

        flow::pane(bg)
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
                cx.listener(|this, ev, window, cx| this.on_canvas_up(ev, window, cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, ev, window, cx| this.on_canvas_up(ev, window, cx)),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, ev, window, cx| this.on_canvas_up(ev, window, cx)),
            )
            .on_scroll_wheel(cx.listener(|this, ev, _, cx| this.on_scroll(ev, cx)))
            .on_drop(cx.listener(|this, drag: &VarDrag, window, cx| {
                let mut world = this.world_of(window.mouse_position());
                world.x -= NODE_W * 0.5;
                world.y -= HEADER_H * 0.5;
                this.add_get_at(&drag.name, drag.ty, world, cx);
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    if this.var_dialog.is_some() {
                        this.close_var_dialog(cx);
                        return;
                    }
                    this.close_add_menu();
                    this.drag = Drag::None;
                    this.end_graph_gesture(cx);
                    cx.notify();
                    return;
                }
                if this.var_dialog.is_some() || this.add_menu.is_some() {
                    return;
                }
                if ev.keystroke.key == "z"
                    && ev.keystroke.modifiers.secondary()
                    && !ev.keystroke.modifiers.shift
                {
                    this.undo_graph(cx);
                    return;
                }
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
                    },
                    move |bounds, _, window, _| {
                        paint_background(window, bounds, cam, muted.opacity(0.45));
                        for e in &edges {
                            paint_edge(window, bounds, cam, e);
                        }
                        if let Drag::Marquee { start, current, .. } = drag {
                            paint_marquee(window, bounds, cam, start, current);
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full(),
            )
            .children(node_els)
            .child(self.graph_controls(cx, surface, border))
            .child(self.variables_card(cx, surface, border, muted))
            .children(
                self.add_menu
                    .as_ref()
                    .map(|menu| self.place_menu(menu, cx, surface, border, muted)),
            )
            .children(self.var_dialog_layer(cx, surface, border))
    }

    fn place_menu(
        &self,
        menu: &AddMenu,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> AnyElement {
        let entries = self.menu_entries(cx);
        let mut action_i = 0usize;
        let highlight = menu.highlight;
        let world = menu.world;
        let pad = 8.0;
        let local_y = f32::from(menu.pos.y);
        let local_x = f32::from(menu.pos.x);
        let ch = f32::from(self.canvas_bounds.size.height);
        let cw = f32::from(self.canvas_bounds.size.width);
        let below = (ch - local_y - pad).max(96.0);
        let above = (local_y - pad).max(96.0);
        let right = (cw - local_x - pad).max(180.0);
        let left = (local_x - pad).max(180.0);
        let max_h = below.max(above).min(420.0);
        let max_w = right.max(left).min(280.0).max(200.0);
        let origin = point(
            self.canvas_bounds.origin.x + menu.pos.x,
            self.canvas_bounds.origin.y + menu.pos.y,
        );
        deferred(
            anchored().position(origin).anchor(Anchor::TopLeft).child(
                v_flex()
                    .id("add-node-menu")
                    .w(px(max_w))
                    .max_h(px(max_h))
                    .occlude()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(surface)
                    .shadow_md()
                    .p_1()
                    .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| {}))
                    .on_mouse_down(MouseButton::Right, cx.listener(|_, _, _, _| {}))
                    .child(
                        div().px_1().py_1().w_full().child(
                            self.menu_search
                                .as_ref()
                                .map(|state| {
                                    Input::new(state)
                                        .id("graph-search")
                                        .small()
                                        .cleanable(true)
                                        .into_any_element()
                                })
                                .unwrap_or_else(|| {
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(t(cx, T::GraphSearch))
                                        .into_any_element()
                                }),
                        ),
                    )
                    .child(
                        v_flex()
                            .id("menu-results")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .children(entries.into_iter().map(|entry| {
                                match entry {
                                    MenuEntry::Header(title) => div()
                                        .px_2()
                                        .pt_2()
                                        .pb_1()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(title)
                                        .into_any_element(),
                                    other => {
                                        let hi = action_i == highlight;
                                        action_i += 1;
                                        self.menu_row(other, world, hi, cx)
                                    }
                                }
                            })),
                    ),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn menu_row(
        &self,
        entry: MenuEntry,
        world: Vec2,
        highlight: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (id, color, label) = match &entry {
            MenuEntry::Place {
                type_id,
                label,
                color,
            } => (format!("m-{type_id}"), *color, label.clone()),
            MenuEntry::Get { name, ty } => (
                format!("m-get-{name}"),
                ty.color(),
                tf(cx, T::GetVar, &[("name", name)]),
            ),
            MenuEntry::Header(_) => unreachable!(),
        };
        div()
            .id(SharedString::from(id))
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .when(highlight, |d| d.bg(rgb(color).opacity(0.25)))
            .hover(|d| d.bg(rgb(color).opacity(0.15)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.run_menu_entry(&entry, world, cx);
            }))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(color)))
                    .child(div().text_xs().child(label)),
            )
            .into_any_element()
    }

    fn menu_query(&self, cx: &App) -> String {
        self.menu_search
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn menu_entries(&self, cx: &App) -> Vec<MenuEntry> {
        let q = self.menu_query(cx).trim().to_lowercase();
        let graph = self.session.graph.lock();
        let mut out = Vec::new();
        let mut vars_section = Vec::new();
        for v in &graph.variables {
            let mut hay = v.name.clone();
            for loc in Locale::all() {
                hay.push(' ');
                hay.push_str(&tf_loc(loc, T::GetVar, &[("name", &v.name)]));
            }
            let hay = hay.to_lowercase();
            if q.is_empty() || hay.contains(&q) {
                vars_section.push(MenuEntry::Get {
                    name: v.name.clone(),
                    ty: v.ty,
                });
            }
        }
        if !vars_section.is_empty() {
            out.push(MenuEntry::Header(t(cx, T::GraphVariables).to_string()));
            out.extend(vars_section);
        }
        drop(graph);

        let mut by_cat: Vec<(Category, Vec<MenuEntry>)> = vec![
            (Category::Input, vec![]),
            (Category::Process, vec![]),
            (Category::Output, vec![]),
            (Category::Utility, vec![]),
        ];
        for t in self.session.registry.all() {
            if VarType::from_type_id(&t.type_id).is_some() {
                continue;
            }
            let cat = category_key(t.category);
            let mut hay = format!("{} {}", t.display_name, t.type_id);
            for loc in Locale::all() {
                hay.push(' ');
                hay.push_str(t_loc(loc, cat));
            }
            let hay = hay.to_lowercase();
            if !q.is_empty() && !hay.contains(&q) {
                continue;
            }
            if let Some((_, bucket)) = by_cat.iter_mut().find(|(c, _)| *c == t.category) {
                bucket.push(MenuEntry::Place {
                    type_id: t.type_id.clone(),
                    label: t.display_name.clone(),
                    color: category_color(t.category),
                });
            }
        }
        for (cat, items) in by_cat {
            if items.is_empty() {
                continue;
            }
            out.push(MenuEntry::Header(t(cx, category_key(cat)).to_string()));
            out.extend(items);
        }
        out
    }

    fn confirm_menu(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.add_menu.as_ref() else {
            return;
        };
        let highlight = menu.highlight;
        let world = menu.world;
        let entries: Vec<MenuEntry> = self
            .menu_entries(cx)
            .into_iter()
            .filter(|e| e.actionable())
            .collect();
        let Some(entry) = entries.get(highlight).cloned() else {
            return;
        };
        self.run_menu_entry(&entry, world, cx);
    }

    fn run_menu_entry(&mut self, entry: &MenuEntry, world: Vec2, cx: &mut Context<Self>) {
        match entry {
            MenuEntry::Place { type_id, .. } => {
                self.add_node_at(type_id, world, cx);
            }
            MenuEntry::Get { name, ty } => {
                self.add_get_at(name, *ty, world, cx);
            }
            MenuEntry::Header(_) => {}
        }
    }

    fn var_dialog_layer(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
    ) -> Option<AnyElement> {
        let dialog = self.var_dialog.as_ref()?;
        let form = self.var_form.clone()?;
        let title = match dialog {
            VarDialog::Create { .. } => t(cx, T::GraphCreateVar),
            VarDialog::Edit { .. } => t(cx, T::GraphEditVar),
        };
        Some(
            div()
                .id("var-dialog-layer")
                .absolute()
                .inset_0()
                .occlude()
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    if ev.keystroke.key == "escape" {
                        this.close_var_dialog(cx);
                    }
                }))
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("var-dialog-dim")
                        .absolute()
                        .inset_0()
                        .bg(rgb(0x000000).opacity(0.4)),
                )
                .child(
                    div()
                        .id("var-dialog-center")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_var_dialog(cx);
                            }),
                        )
                        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                        .child(
                            v_flex()
                                .id("var-dialog")
                                .w(px(320.))
                                .occlude()
                                .rounded_md()
                                .border_1()
                                .border_color(border)
                                .bg(surface)
                                .shadow_md()
                                .p_3()
                                .gap_2()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                                .on_mouse_down(MouseButton::Middle, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(form)
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_end()
                                        .gap_2()
                                        .pt_1()
                                        .child(
                                            Button::new("var-dialog-cancel")
                                                .small()
                                                .label(t(cx, T::GraphCancel))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_var_dialog(cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("var-dialog-ok")
                                                .small()
                                                .primary()
                                                .label(t(cx, T::GraphOk))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.confirm_var_dialog(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn close_var_dialog(&mut self, cx: &mut Context<Self>) {
        self.var_dialog = None;
        self.var_form = None;
        cx.notify();
    }

    fn open_var_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>, kind: VarDialog) {
        self.close_add_menu();
        let (initial_name, initial_value, ty) = match &kind {
            VarDialog::Create { preset } => (
                preset.clone().unwrap_or_else(|| "NewVar".into()),
                default_value_str(VarType::Float).to_string(),
                VarType::Float,
            ),
            VarDialog::Edit { original } => {
                let g = self.session.graph.lock();
                let Some(v) = g.variables.iter().find(|v| v.name == *original) else {
                    return;
                };
                (v.name.clone(), var_value_edit_str(v), v.ty)
            }
        };
        let form = cx.new(|cx| {
            let name = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(t(cx, T::GraphName))
                    .default_value(initial_name.clone())
            });
            let value = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(t(cx, T::GraphValue))
                    .default_value(initial_value.clone())
            });
            let types: Vec<VarTypeChoice> = VarType::ALL
                .into_iter()
                .map(|vt| VarTypeChoice {
                    ty: vt,
                    title: t(cx, var_type_key(vt)),
                })
                .collect();
            let type_idx = types.iter().position(|c| c.ty == ty).unwrap_or(0);
            let type_select = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(types),
                    Some(IndexPath::new(type_idx)),
                    window,
                    cx,
                )
            });
            let bool_val = matches!(
                initial_value.to_ascii_lowercase().as_str(),
                "true" | "1" | "yes"
            );
            let bool_select = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec![BoolChoice(false), BoolChoice(true)]),
                    Some(IndexPath::new(bool_val as usize)),
                    window,
                    cx,
                )
            });
            name.update(cx, |s, cx| s.focus(window, cx));
            let type_sub = cx.subscribe_in(
                &type_select,
                window,
                |this: &mut VarCreateForm,
                 _,
                 ev: &SelectEvent<SearchableVec<VarTypeChoice>>,
                 window,
                 cx| {
                    if let SelectEvent::Confirm(Some(next)) = ev {
                        this.apply_type(*next, window, cx);
                    }
                },
            );
            VarCreateForm {
                name,
                value,
                ty,
                type_select,
                bool_select,
                _type_sub: Some(type_sub),
            }
        });
        self.var_form = Some(form);
        self.var_dialog = Some(kind);
        cx.notify();
    }

    fn confirm_var_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.var_form.clone() else {
            return;
        };
        let Some(kind) = self.var_dialog.clone() else {
            return;
        };
        let (name, value, ty) = {
            let f = form.read(cx);
            let ty = f.current_ty(cx);
            let Some(value) = f.current_value(cx) else {
                self.status = t(cx, T::GraphInvalidValue).to_string();
                cx.notify();
                return;
            };
            (f.name.read(cx).value().to_string(), value, ty)
        };
        match kind {
            VarDialog::Create { .. } => {
                self.create_variable(&name, ty, value, cx);
            }
            VarDialog::Edit { original } => {
                let before = self.snapshot_graph();
                let mut g = self.session.graph.lock();
                let new_name = g.update_variable(&original, name, ty, value);
                drop(g);
                self.session.recompile();
                self.commit_graph_change(before, cx);
                self.status = tf(cx, T::StatusVariable, &[("name", &new_name)]);
                cx.notify();
            }
        }
        self.close_var_dialog(cx);
    }

    fn create_variable(
        &mut self,
        name: &str,
        ty: VarType,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> String {
        let before = self.snapshot_graph();
        let mut g = self.session.graph.lock();
        let name = g.add_variable(name.to_string(), ty);
        if let Some(v) = g.var_mut(&name) {
            v.value = value;
        }
        drop(g);
        self.session.recompile();
        self.commit_graph_change(before, cx);
        self.status = tf(cx, T::StatusVariable, &[("name", &name)]);
        cx.notify();
        name
    }

    fn param_rows_for(
        &mut self,
        n: &vf_core::GraphNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Vec<ParamRow>, Vec<AnyElement>) {
        let Some(ty) = self.session.registry.get(&n.type_id).cloned() else {
            return (Vec::new(), Vec::new());
        };
        let connected: HashSet<u32> = self
            .session
            .graph
            .lock()
            .incoming(n.id)
            .map(|e| e.to.port)
            .collect();
        let mut rows = Vec::new();
        let mut buttons = Vec::new();
        for (i, p) in ty.pin_params().enumerate() {
            let port = (ty.inputs.len() + i) as u32;
            let wired = connected.contains(&port);
            let tag = vf_core::NodeType::param_kind_tag(p.kind).map(|t| t as u32);
            let editor = if wired {
                None
            } else {
                self.pin_editor(n.id, p, &n.params, window, cx)
            };
            rows.push(ParamRow {
                name: p.label.clone(),
                tag,
                editor,
            });
        }
        for p in ty.params.iter().filter(|p| p.kind == ParamKind::Button) {
            let key = p.key.clone();
            let id = n.id;
            buttons.push(
                Button::new(SharedString::from(format!("btn-{}-{}", id.0, key)))
                    .small()
                    .label(p.label.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_param(id, &key, serde_json::json!(true), cx);
                    }))
                    .into_any_element(),
            );
        }
        (rows, buttons)
    }

    fn pin_editor(
        &mut self,
        id: NodeId,
        p: &vf_sdk::ParamDef,
        params: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let key = p.key.clone();
        let cur = params
            .get(&p.key)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let zoom = self.camera.zoom.max(0.6);
        let font = px((10.0 * zoom).clamp(8.0, 12.0));
        match p.kind {
            ParamKind::Bool => {
                let on = cur.as_bool().unwrap_or(false);
                Some(
                    div()
                        .id(SharedString::from(format!("pin-b-{}-{}", id.0, key)))
                        .occlude()
                        .w(px(14. * zoom))
                        .h(px(14. * zoom))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xa1a1aa))
                        .when(on, |d| d.bg(rgb(0x22c55e)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_param(id, &key, serde_json::json!(!on), cx);
                        }))
                        .into_any_element(),
                )
            }
            ParamKind::Float | ParamKind::Int => {
                let intish = p.kind == ParamKind::Int;
                let initial = if intish {
                    cur.as_i64()
                        .or_else(|| cur.as_f64().map(|v| v as i64))
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "0".into())
                } else {
                    cur.as_f64()
                        .or_else(|| cur.as_i64().map(|i| i as f64))
                        .map(|v| format_float_text(v))
                        .unwrap_or_else(|| "0".into())
                };
                Some(self.pin_text_input(id, &key, initial, Some(intish), window, cx, zoom))
            }
            ParamKind::Enum => {
                let cur_s = cur.as_str().unwrap_or("").to_string();
                let opts = p.options.clone();
                let key2 = key.clone();
                Some(
                    div()
                        .id(SharedString::from(format!("pin-e-{}-{}", id.0, key)))
                        .occlude()
                        .w_full()
                        .h(px(EDITOR_H * zoom))
                        .px_1()
                        .rounded_sm()
                        .bg(rgb(0x27272a))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .text_size(font)
                        .child(if cur_s.is_empty() {
                            "…".into()
                        } else {
                            cur_s.clone()
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let idx = opts.iter().position(|o| o == &cur_s).unwrap_or(0);
                            let next = opts[(idx + 1) % opts.len().max(1)].clone();
                            this.set_param(id, &key2, serde_json::json!(next), cx);
                        }))
                        .into_any_element(),
                )
            }
            ParamKind::String => {
                if p.key == "name"
                    && VarType::from_type_id(
                        &self
                            .session
                            .graph
                            .lock()
                            .node(id)
                            .map(|n| n.type_id.clone())
                            .unwrap_or_default(),
                    )
                    .is_some()
                {
                    let vars: Vec<String> = self
                        .session
                        .graph
                        .lock()
                        .variables
                        .iter()
                        .map(|v| v.name.clone())
                        .collect();
                    let cur_s = cur.as_str().unwrap_or("").to_string();
                    let key2 = key.clone();
                    return Some(
                        div()
                            .id(SharedString::from(format!("pin-vn-{}-{}", id.0, key)))
                            .occlude()
                            .w_full()
                            .h(px(EDITOR_H * zoom))
                            .px_1()
                            .rounded_sm()
                            .bg(rgb(0x27272a))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .text_size(font)
                            .child(if cur_s.is_empty() {
                                "var".into()
                            } else {
                                cur_s.clone()
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if vars.is_empty() {
                                    return;
                                }
                                let idx = vars.iter().position(|o| o == &cur_s).unwrap_or(0);
                                let next = vars[(idx + 1) % vars.len()].clone();
                                this.set_param(id, &key2, serde_json::json!(next), cx);
                            }))
                            .into_any_element(),
                    );
                }
                let initial = cur.as_str().unwrap_or("").to_string();
                Some(self.pin_text_input(id, &key, initial, None, window, cx, zoom))
            }
            ParamKind::Button => None,
        }
    }

    fn pin_text_input(
        &mut self,
        id: NodeId,
        key: &str,
        initial: String,
        number: Option<bool>,
        window: &mut Window,
        cx: &mut Context<Self>,
        zoom: f32,
    ) -> AnyElement {
        let map_key = (id.0, key.to_string());
        if !self.pin_inputs.contains_key(&map_key) {
            let state = cx.new(|cx| InputState::new(window, cx).default_value(initial.clone()));
            let k = key.to_string();
            self.pin_subs.push(
                cx.subscribe(&state, move |this, input, ev: &InputEvent, cx| {
                    if !matches!(ev, InputEvent::Change | InputEvent::PressEnter { .. }) {
                        return;
                    }
                    let v = input.read(cx).value().to_string();
                    let value = match number {
                        Some(intish) => parse_pin_number(&v, intish),
                        None => Some(serde_json::json!(v)),
                    };
                    if let Some(value) = value {
                        this.set_param(id, &k, value, cx);
                    }
                }),
            );
            self.pin_inputs.insert(map_key.clone(), state);
        }
        let state = self.pin_inputs.get(&map_key).unwrap().clone();
        let font = px((10.0 * zoom).clamp(8.0, 13.0));
        let h = px(EDITOR_H * zoom);
        div()
            .w_full()
            .h(h)
            .occlude()
            .child(
                Input::new(&state)
                    .id(SharedString::from(format!("pin-s-{}-{}", id.0, key)))
                    .xsmall()
                    .h(h)
                    .text_size(font),
            )
            .into_any_element()
    }

    fn graph_controls(
        &self,
        cx: &mut Context<Self>,
        surface: Hsla,
        border: Hsla,
    ) -> impl IntoElement {
        v_flex()
            .id("graph-zoom")
            .absolute()
            .bottom_3()
            .left_3()
            .w(px(32.))
            .occlude()
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
            }))
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
        .w_full()
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|d| d.bg(rgb(0x3f3f46)))
        .on_click(cx.listener(handler))
        .child(div().text_xs().child(label))
}

fn var_row(v: GraphVar, muted: Hsla, cx: &mut Context<Workspace>) -> impl IntoElement {
    let name = v.name.clone();
    let name_del = name.clone();
    let name_edit = name.clone();
    let ty = v.ty;
    let value = var_value_label(&v);
    h_flex()
        .id(SharedString::from(format!("var-{name}")))
        .px_1()
        .py_1()
        .gap_1()
        .items_center()
        .rounded_md()
        .hover(|d| d.bg(rgb(0x27272a)))
        .child(
            h_flex()
                .id(SharedString::from(format!("var-drag-{name}")))
                .flex_1()
                .min_w(px(0.))
                .gap_1()
                .items_center()
                .cursor_grab()
                .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                    if ev.click_count() >= 2 {
                        this.open_var_dialog(
                            window,
                            cx,
                            VarDialog::Edit {
                                original: name_edit.clone(),
                            },
                        );
                    }
                }))
                .on_drag(
                    VarDrag {
                        name: name.clone(),
                        ty,
                    },
                    |drag, _, _, cx| {
                        cx.new(|_| VarDragGhost {
                            name: drag.name.clone(),
                            ty: drag.ty,
                        })
                    },
                )
                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(ty.color())))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_xs()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child(name),
                )
                .child(div().text_xs().text_color(muted).child(value)),
        )
        .child(
            div()
                .id(SharedString::from(format!("var-x-{name_del}")))
                .text_xs()
                .text_color(rgb(0xef4444))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    let before = this.snapshot_graph();
                    this.session.graph.lock().remove_variable(&name_del);
                    this.session.recompile();
                    this.commit_graph_change(before, cx);
                    cx.notify();
                }))
                .child("×"),
        )
}

fn default_value_str(ty: VarType) -> &'static str {
    match ty {
        VarType::Float | VarType::Int => "0",
        VarType::Bool => "false",
        VarType::String => "",
    }
}

fn parse_var_value(ty: VarType, raw: &str) -> Option<serde_json::Value> {
    let s = raw.trim();
    if s.is_empty() {
        return Some(ty.default_value());
    }
    match ty {
        VarType::Float => s.parse::<f64>().ok().map(|v| serde_json::json!(v)),
        VarType::Int => {
            if let Ok(v) = s.parse::<i64>() {
                Some(serde_json::json!(v))
            } else {
                s.parse::<f64>().ok().map(|v| serde_json::json!(v as i64))
            }
        }
        VarType::Bool => match s.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(serde_json::json!(true)),
            "false" | "0" | "no" => Some(serde_json::json!(false)),
            _ => None,
        },
        VarType::String => Some(serde_json::Value::String(raw.to_string())),
    }
}

fn parse_pin_number(raw: &str, intish: bool) -> Option<serde_json::Value> {
    let s = raw.trim();
    if s.is_empty() || s == "-" || s == "." || s == "-." {
        return None;
    }
    if intish {
        if let Ok(v) = s.parse::<i64>() {
            return Some(serde_json::json!(v));
        }
        return s.parse::<f64>().ok().map(|v| serde_json::json!(v as i64));
    }
    s.parse::<f64>().ok().map(|v| serde_json::json!(v))
}

fn format_float_text(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.to_string()
    }
}

fn var_value_edit_str(v: &GraphVar) -> String {
    match &v.value {
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn var_value_label(v: &GraphVar) -> String {
    match &v.value {
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.chars().count() > 16 {
                format!("{}…", s.chars().take(16).collect::<String>())
            } else {
                s.clone()
            }
        }
        other => other.to_string(),
    }
}
