use gpui_kit::component::{ActiveTheme, Theme, ThemeMode};
use gpui_kit::*;
use std::sync::Arc;
use vf_abi::VfValueTag;
use vf_core::{NodeId, PortRef, Session, Snapshot};
use vf_sdk::Category;

pub const NODE_W: f32 = 220.0;
pub const HEADER_H: f32 = 32.0;
pub const PORT_H: f32 = 22.0;
pub const PORT_R: f32 = 5.0;
pub const ZOOM_MIN: f32 = 0.2;
pub const ZOOM_MAX: f32 = 2.5;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Camera {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            pan: Vec2::new(40.0, 40.0),
            zoom: 1.0,
        }
    }

    pub fn world_to_screen(&self, x: f32, y: f32) -> Point<Pixels> {
        point(
            px(x * self.zoom + self.pan.x),
            px(y * self.zoom + self.pan.y),
        )
    }

    pub fn screen_to_world(&self, p: Point<Pixels>) -> Vec2 {
        Vec2::new(
            (f32::from(p.x) - self.pan.x) / self.zoom,
            (f32::from(p.y) - self.pan.y) / self.zoom,
        )
    }

    pub fn zoom_at(&mut self, factor: f32, pivot: Vec2) {
        let old = self.zoom.max(0.001);
        self.zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let wx = (pivot.x - self.pan.x) / old;
        let wy = (pivot.y - self.pan.y) / old;
        self.pan.x = pivot.x - wx * self.zoom;
        self.pan.y = pivot.y - wy * self.zoom;
    }
}

#[derive(Clone, Copy)]
pub enum Drag {
    None,
    Node { id: NodeId, grab: Vec2 },
    Pan { last: Point<Pixels> },
    Wire { from: PortRef, current: Vec2 },
}

pub fn category_color(cat: Category) -> u32 {
    match cat {
        Category::Input => 0x3b82f6,
        Category::Process => 0xa855f7,
        Category::Output => 0xf59e0b,
        Category::Utility => 0x64748b,
    }
}

pub fn tag_color(tag: VfValueTag) -> u32 {
    match tag {
        VfValueTag::Float => 0x22c55e,
        VfValueTag::Int => 0x84cc16,
        VfValueTag::Bool => 0xef4444,
        VfValueTag::Vec2 | VfValueTag::Vec3 => 0x06b6d4,
        VfValueTag::UnifiedFrame => 0xa855f7,
        VfValueTag::Blendshapes => 0x3b82f6,
        VfValueTag::Bytes => 0x78716c,
        VfValueTag::Empty => 0x52525b,
    }
}

pub fn node_height(n_in: usize, n_out: usize) -> f32 {
    HEADER_H + (n_in.max(n_out).max(1) as f32) * PORT_H + 8.0
}

pub fn port_y(index: u32) -> f32 {
    HEADER_H + PORT_H * (index as f32 + 0.5)
}

pub fn status_rgb(level: u32) -> u32 {
    match level {
        0 => 0x22c55e,
        1 => 0xf59e0b,
        2 => 0xef4444,
        _ => 0x64748b,
    }
}

pub fn latest_snapshot(session: &Arc<Session>) -> Arc<Snapshot> {
    session.engine.snapshot()
}

/// Pin the UI to dark. gpui-kit `init` starts in light; call this immediately after.
pub fn lock_dark_theme(cx: &mut App) {
    cx.set_window_appearance(Some(WindowAppearance::Dark));
    Theme::change(ThemeMode::Dark, None, cx);
}

pub fn lock_dark_theme_for_window(window: &mut Window, cx: &mut App) {
    cx.set_window_appearance(Some(WindowAppearance::Dark));
    Theme::change(ThemeMode::Dark, Some(window), cx);
}

#[allow(dead_code)]
pub fn theme_bg(cx: &App) -> Hsla {
    cx.theme().background
}

#[allow(dead_code)]
pub fn theme_fg(cx: &App) -> Hsla {
    cx.theme().foreground
}
