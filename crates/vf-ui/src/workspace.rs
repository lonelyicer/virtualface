use crate::i18n::{Locale, T, locale, set_locale, t};
use crate::page::AppPage;
use crate::theme::{Camera, Drag, Vec2};
use gpui_kit::base::VirtualListScrollHandle;
use gpui_kit::component::input::InputState;
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, IndexPath, Root, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vf_core::{ArchiveIndex, Graph, NodeId, Session, Snapshot};

use crate::archives::bootstrap_archives;

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LicenseId {
    App,
    Third(usize),
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
    pub(crate) archives: ArchiveIndex,
    pub(crate) autosave: crate::autosave::Autosave,
    pub(crate) archive_menu_open: bool,
    pub(crate) archive_picker_bounds: Bounds<Pixels>,
    pub(crate) undo_stack: Vec<Graph>,
    pub(crate) gesture_before: Option<Graph>,
    pub(crate) status: String,
    pub(crate) focus: FocusHandle,
    pub(crate) title_should_move: bool,
    pub(crate) log_view: crate::pages::log::LogView,
    pub(crate) graph_scene: Option<Arc<crate::flow::GraphScene>>,
    live_ui_pumping: bool,
    ui_snapshot: Arc<Snapshot>,
    ui_home_updated: Instant,
    ui_log_seq: u64,
    pub(crate) license_scroll: VirtualListScrollHandle,
    pub(crate) license_expanded: Option<LicenseId>,
    pub(crate) language_select: Entity<SelectState<SearchableVec<Locale>>>,
    _language_sub: Option<Subscription>,
    _appearance: Option<Subscription>,
}

impl Workspace {
    pub fn new(session: Arc<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locales: Vec<Locale> = Locale::all().collect();
        let current = locale(cx);
        let selected = locales.iter().position(|l| *l == current).unwrap_or(0);
        let language_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(locales),
                Some(IndexPath::new(selected)),
                window,
                cx,
            )
        });
        let language_sub = cx.subscribe(
            &language_select,
            |_, _, ev: &SelectEvent<SearchableVec<Locale>>, cx| {
                if let SelectEvent::Confirm(Some(loc)) = ev {
                    set_locale(cx, *loc);
                    cx.notify();
                }
            },
        );
        let archives = bootstrap_archives(&session, t(cx, T::Unnamed).as_ref());
        let autosave = crate::autosave::Autosave::new(session.host.log.clone());
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
            archives,
            autosave,
            archive_menu_open: false,
            archive_picker_bounds: Bounds::default(),
            undo_stack: Vec::new(),
            gesture_before: None,
            status: t(cx, T::StatusReady).to_string(),
            focus: cx.focus_handle(),
            title_should_move: false,
            log_view: crate::pages::log::LogView::new(),
            graph_scene: None,
            live_ui_pumping: false,
            ui_snapshot: Arc::new(Snapshot::default()),
            ui_home_updated: Instant::now(),
            ui_log_seq: 0,
            license_scroll: VirtualListScrollHandle::new(),
            license_expanded: None,
            language_select,
            _language_sub: Some(language_sub),
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

    fn wants_live_frames(&self) -> bool {
        match self.page {
            AppPage::Home | AppPage::Graph => self.session.engine.snapshot().running,
            AppPage::Log => true,
            AppPage::Settings | AppPage::Licenses => false,
        }
    }

    fn consume_live_ui_dirty(&mut self) -> bool {
        let snap = self.session.engine.snapshot();
        let log_seq = self.session.host.log.seq();
        let dirty = match self.page {
            AppPage::Graph => graph_status_changed(&self.ui_snapshot, &snap),
            AppPage::Home => {
                let changed = snap.running != self.ui_snapshot.running
                    || (snap.tick != self.ui_snapshot.tick
                        && self.ui_home_updated.elapsed() >= Duration::from_millis(100));
                if changed {
                    self.ui_home_updated = Instant::now();
                }
                changed
            }
            AppPage::Log => log_seq != self.ui_log_seq,
            AppPage::Settings | AppPage::Licenses => false,
        };
        // Keep the last displayed home sample until the next statistics update.
        if self.page != AppPage::Home || dirty {
            self.ui_snapshot = snap;
        }
        self.ui_log_seq = log_seq;
        dirty
    }

    fn ensure_live_ui_pump(&mut self, cx: &mut Context<Self>) {
        if self.live_ui_pumping || !self.wants_live_frames() {
            return;
        }
        self.live_ui_pumping = true;
        self.ui_snapshot = self.session.engine.snapshot();
        self.ui_log_seq = self.session.host.log.seq();
        // Polling data must not itself request a presentation. Input events still
        // invalidate immediately and are presented at the display's frame rate.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let keep_polling = this.update(cx, |this, cx| {
                    if this.consume_live_ui_dirty() {
                        cx.notify();
                    }
                    let keep_polling = this.wants_live_frames();
                    if !keep_polling {
                        this.live_ui_pumping = false;
                    }
                    keep_polling
                });
                if !matches!(keep_polling, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn start_engine(&mut self, cx: &mut Context<Self>) {
        self.session.recompile();
        self.session.engine.start();
        self.status = t(cx, T::StatusRunning).to_string();
        cx.notify();
    }

    pub(crate) fn stop_engine(&mut self, cx: &mut Context<Self>) {
        self.session.engine.stop();
        self.status = t(cx, T::StatusStopped).to_string();
        cx.notify();
    }

    pub(crate) fn reset_editor_for_graph(&mut self) {
        self.selected.clear();
        self.primary = None;
        self.close_add_menu();
        self.pin_inputs.clear();
        self.pin_subs.clear();
        self.graph_scene = None;
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
            AppPage::Licenses => self.licenses_page(cx, muted).into_any_element(),
            AppPage::Log => self.log_page(cx, muted).into_any_element(),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_live_ui_pump(cx);
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

// The graph displays status lights, not per-tick port payloads or timing.
fn graph_status_changed(before: &Snapshot, after: &Snapshot) -> bool {
    before.running != after.running
        || before.nodes.len() != after.nodes.len()
        || after.nodes.iter().any(|(id, node)| {
            before.nodes.get(id).map(|n| n.status_level) != Some(node.status_level)
        })
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
            let view = cx.new(|cx| Workspace::new(session, window, cx));
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use vf_core::{NodeSnap, SnapshotValue};

    #[test]
    fn graph_refreshes_status_changes_but_not_engine_ticks_or_payloads() {
        let mut before = Snapshot::default();
        before.nodes.insert(
            1,
            NodeSnap {
                status_level: 0,
                status_text: String::new(),
                outputs: vec![],
                disabled: false,
                state: None,
            },
        );
        let mut after = before.clone();
        after.tick += 1;
        after.dt_us = 10000;
        after
            .nodes
            .get_mut(&1)
            .unwrap()
            .outputs
            .push(SnapshotValue::Float(0.5));
        assert!(!graph_status_changed(&before, &after));
        after.nodes.get_mut(&1).unwrap().status_level = 1;
        assert!(graph_status_changed(&before, &after));
        after = before.clone();
        after.running = true;
        assert!(graph_status_changed(&before, &after));
        after = before.clone();
        let node = after.nodes.remove(&1).unwrap();
        after.nodes.insert(2, node);
        assert!(graph_status_changed(&before, &after));
        after.nodes.clear();
        assert!(graph_status_changed(&before, &after));
    }
}
