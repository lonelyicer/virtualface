use crate::page::AppPage;
use crate::theme::{Camera, Drag, Vec2};
use gpui_kit::component::input::InputState;
use gpui_kit::component::{ActiveTheme, Root, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use vf_core::{
    NodeId, Session, absolute_path, graph_display_name, remember_last_graph, with_graph_extension,
};

pub(crate) struct AddMenu {
    pub pos: Point<Pixels>,
    pub world: Vec2,
    pub highlight: usize,
}

#[derive(Clone)]
pub(crate) enum VarDialog {
    Create { preset: Option<String> },
    Edit { original: String },
}

pub struct Workspace {
    pub session: Arc<Session>,
    pub(crate) page: AppPage,
    pub(crate) selected: HashSet<NodeId>,
    pub(crate) primary: Option<NodeId>,
    pub(crate) camera: Camera,
    pub(crate) drag: Drag,
    pub(crate) canvas_bounds: Bounds<Pixels>,
    pub(crate) add_menu: Option<AddMenu>,
    pub(crate) menu_search: Option<Entity<InputState>>,
    pub(crate) menu_sub: Option<Subscription>,
    pub(crate) pin_inputs: HashMap<(u64, String), Entity<InputState>>,
    pub(crate) pin_subs: Vec<Subscription>,
    pub(crate) vars_open: bool,
    pub(crate) var_dialog: Option<VarDialog>,
    pub(crate) var_form: Option<Entity<crate::pages::graph::VarCreateForm>>,
    pub(crate) graph_path: String,
    pub(crate) status: String,
    pub(crate) focus: FocusHandle,
    pub(crate) title_should_move: bool,
    pub(crate) log_scroll: ScrollHandle,
    pub(crate) log_len: usize,
    pub(crate) log_tail_ts: u64,
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
            selected: HashSet::new(),
            primary: None,
            camera: Camera::new(),
            drag: Drag::None,
            canvas_bounds: Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(800.), px(600.)),
            },
            add_menu: None,
            menu_search: None,
            menu_sub: None,
            pin_inputs: HashMap::new(),
            pin_subs: Vec::new(),
            vars_open: true,
            var_dialog: None,
            var_form: None,
            graph_path,
            status: "ready".into(),
            focus: cx.focus_handle(),
            title_should_move: false,
            log_scroll: ScrollHandle::default(),
            log_len: 0,
            log_tail_ts: 0,
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

    pub(crate) fn graph_name(&self) -> String {
        graph_display_name(&self.graph_path)
    }

    fn set_current_graph_path(&mut self, path: PathBuf) {
        let path = absolute_path(&path);
        let shown = path.to_string_lossy().into_owned();
        self.graph_path = shown.clone();
        *self.session.graph_path.lock() = shown;
        remember_last_graph(&path);
    }

    fn reset_editor_for_graph(&mut self) {
        self.selected.clear();
        self.primary = None;
        self.close_add_menu();
        self.pin_inputs.clear();
        self.pin_subs.clear();
    }

    fn write_graph_to(&mut self, path: &Path, cx: &mut Context<Self>) {
        let path = with_graph_extension(path.to_path_buf());
        match self.session.save_graph_str() {
            Ok(s) => match std::fs::write(&path, s) {
                Ok(()) => {
                    self.set_current_graph_path(path);
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

    pub(crate) fn save_graph(&mut self, cx: &mut Context<Self>) {
        if self.graph_path.is_empty() {
            self.export_graph(cx);
            return;
        }
        let path = PathBuf::from(&self.graph_path);
        self.write_graph_to(&path, cx);
    }

    pub(crate) fn load_graph_from_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        match std::fs::read_to_string(path) {
            Ok(s) => match self.session.load_graph_str(&s) {
                Ok(()) => {
                    self.set_current_graph_path(path.to_path_buf());
                    self.reset_editor_for_graph();
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
        cx.notify();
    }

    pub(crate) fn pick_and_load_graph(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("加载图".into()),
        });
        cx.spawn(async move |this, cx| {
            let outcome = match rx.await {
                Ok(v) => v,
                Err(_) => return,
            };
            match outcome {
                Ok(Some(paths)) => {
                    if let Some(path) = paths.into_iter().next() {
                        this.update(cx, |this, cx| this.load_graph_from_path(&path, cx))
                            .ok();
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.status = format!("open failed: {e}");
                        this.note(0, this.status.clone());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub(crate) fn export_graph(&mut self, cx: &mut Context<Self>) {
        let dir = Path::new(&self.graph_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let suggested = Path::new(&self.graph_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("graph.vfgraph.json");
        let rx = cx.prompt_for_new_path(&dir, Some(suggested));
        cx.spawn(async move |this, cx| {
            let outcome = match rx.await {
                Ok(v) => v,
                Err(_) => return,
            };
            match outcome {
                Ok(Some(path)) => {
                    this.update(cx, |this, cx| this.write_graph_to(&path, cx))
                        .ok();
                }
                Ok(None) => {}
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.status = format!("export failed: {e}");
                        this.note(0, this.status.clone());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
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
        window: &mut Window,
        cx: &mut Context<Self>,
        bg: Hsla,
        surface: Hsla,
        border: Hsla,
        muted: Hsla,
    ) -> AnyElement {
        match self.page {
            AppPage::Home => self.home_page(cx, muted).into_any_element(),
            AppPage::Graph => self
                .graph_page(window, cx, bg, surface, border, muted)
                .into_any_element(),
            AppPage::Settings => self.settings_page(cx, muted).into_any_element(),
            AppPage::Log => self.log_page(muted).into_any_element(),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bg = theme.background;
        let fg = theme.foreground;
        let border = theme.border;
        let muted = theme.muted_foreground;
        let surface = theme.secondary;
        let dialogs = Root::render_dialog_layer(window, cx);
        let sheets = Root::render_sheet_layer(window, cx);
        let notifications = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .relative()
            .child(
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
                            .child(self.title_bar(window, cx))
                            .child(
                                v_flex()
                                    .id("page")
                                    .flex_1()
                                    .size_full()
                                    .min_h(px(0.))
                                    .min_w(px(0.))
                                    .child(self.page_body(window, cx, bg, surface, border, muted)),
                            ),
                    ),
            )
            .children(sheets)
            .children(dialogs)
            .children(notifications)
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
