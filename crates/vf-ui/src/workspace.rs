use crate::page::AppPage;
use crate::theme::{Camera, Drag, Vec2};
use gpui_kit::component::{ActiveTheme, Root, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::sync::Arc;
use vf_core::{NodeId, Session};

pub struct Workspace {
    pub session: Arc<Session>,
    pub(crate) page: AppPage,
    pub(crate) selected: Option<NodeId>,
    pub(crate) camera: Camera,
    pub(crate) drag: Drag,
    pub(crate) canvas_bounds: Bounds<Pixels>,
    pub(crate) pending_type: Option<String>,
    pub(crate) graph_path: String,
    pub(crate) status: String,
    pub(crate) focus: FocusHandle,
    _appearance: Option<Subscription>,
}

impl Workspace {
    pub fn new(session: Arc<Session>, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(33))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        let graph_path = session.graph_path.lock().clone();
        Self {
            session,
            page: AppPage::Home,
            selected: None,
            camera: Camera::new(),
            drag: Drag::None,
            canvas_bounds: Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(800.), px(600.)),
            },
            pending_type: None,
            graph_path,
            status: "ready".into(),
            focus: cx.focus_handle(),
            _appearance: None,
        }
    }

    pub(crate) fn local(&self, p: Point<Pixels>) -> Point<Pixels> {
        point(
            p.x - self.canvas_bounds.origin.x,
            p.y - self.canvas_bounds.origin.y,
        )
    }

    pub(crate) fn world_of(&self, p: Point<Pixels>) -> Vec2 {
        let loc = self.local(p);
        self.camera.screen_to_world(loc)
    }

    pub(crate) fn note(&self, level: u32, msg: impl Into<String>) {
        self.session.host.log.log(level, None, msg);
    }

    pub(crate) fn start_engine(&mut self, cx: &mut Context<Self>) {
        self.session.recompile();
        self.session.engine.start();
        self.status = "running".into();
        cx.notify();
    }

    pub(crate) fn stop_engine(&mut self, cx: &mut Context<Self>) {
        self.session.engine.stop();
        self.status = "stopped".into();
        cx.notify();
    }

    pub(crate) fn reload_graph(&mut self, cx: &mut Context<Self>) {
        self.session.recompile();
        self.status = "graph recompiled".into();
        self.note(2, "graph recompiled");
        cx.notify();
    }

    pub(crate) fn save_graph(&mut self, cx: &mut Context<Self>) {
        match self.session.save_graph_str() {
            Ok(s) => match std::fs::write(&self.graph_path, s) {
                Ok(()) => {
                    self.status = format!("saved {}", self.graph_path);
                    self.note(2, self.status.clone());
                }
                Err(e) => {
                    self.status = format!("save failed: {e}");
                    self.note(0, self.status.clone());
                }
            },
            Err(e) => {
                self.status = format!("save failed: {e}");
                self.note(0, self.status.clone());
            }
        }
        cx.notify();
    }

    pub(crate) fn open_graph(&mut self, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&self.graph_path) {
            Ok(s) => match self.session.load_graph_str(&s) {
                Ok(()) => {
                    self.status = format!("loaded {}", self.graph_path);
                    self.note(2, self.status.clone());
                }
                Err(e) => {
                    self.status = format!("load failed: {e}");
                    self.note(0, self.status.clone());
                }
            },
            Err(e) => {
                self.status = format!("open failed: {e}");
                self.note(0, self.status.clone());
            }
        }
        self.selected = None;
        cx.notify();
    }

    pub(crate) fn bump_rate(&mut self, delta: f32, cx: &mut Context<Self>) {
        let mut g = self.session.graph.lock();
        g.rate_hz = (g.rate_hz + delta).clamp(1.0, 240.0);
        let rate = g.rate_hz;
        drop(g);
        self.session.recompile();
        self.status = format!("tick rate {rate:.0} Hz");
        self.note(2, self.status.clone());
        cx.notify();
    }

    fn page_body(
        &mut self,
        cx: &mut Context<Self>,
        bg: Hsla,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> AnyElement {
        match self.page {
            AppPage::Home => self.home_page(cx, muted).into_any_element(),
            AppPage::Graph => self
                .graph_page(cx, bg, surface, border, muted)
                .into_any_element(),
            AppPage::Settings => self.settings_page(cx, muted).into_any_element(),
            AppPage::Log => self.log_page(muted).into_any_element(),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bg = theme.background;
        let fg = theme.foreground;
        let border = theme.border;
        let muted = theme.muted_foreground;
        let surface = theme.secondary;

        h_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .text_sm()
            .child(self.sidebar(cx))
            .child(
                v_flex()
                    .id("main")
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .h_full()
                    .child(self.title_bar(cx))
                    .child(
                        div()
                            .id("page")
                            .flex_1()
                            .min_h(px(0.))
                            .min_w(px(0.))
                            .child(self.page_body(cx, bg, surface, border, muted)),
                    ),
            )
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub fn open_workspace(session: Arc<Session>, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.), px(520.))),
            window_decorations: Some(WindowDecorations::Client),
            app_id: Some("virtualface".into()),
            ..TitleBar::window_options()
        },
        |window, cx| {
            crate::theme::lock_dark_theme_for_window(window, cx);
            let view = cx.new(|cx| Workspace::new(session, cx));
            let appearance = window.observe_window_appearance(|window, cx| {
                crate::theme::lock_dark_theme_for_window(window, cx);
            });
            view.update(cx, |ws, _| {
                ws._appearance = Some(appearance);
            });
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .ok();
    cx.activate(true);
}
