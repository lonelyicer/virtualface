use crate::i18n::{T, t, tf};
use crate::workspace::Workspace;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::{Confirm, DialogButtonProps, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use vf_core::{
    ArchiveIndex, ArchiveMeta, Graph, Session, absolute_path, delete_archive_file,
    graph_display_name, last_graph_path, load_archive_index, new_archive_id, read_archive_json,
    save_archive_index, with_graph_extension, write_archive_json,
};

const MAX_UNDO: usize = 80;

pub(crate) fn bootstrap_archives(session: &Arc<Session>, unnamed: &str) -> ArchiveIndex {
    let mut index = load_archive_index();
    let path_hint = session.graph_path.lock().clone();
    let file_hint = PathBuf::from(&path_hint);

    if !path_hint.is_empty() && file_hint.is_file() {
        let stem = graph_display_name(&path_hint, unnamed);
        if let Err(e) = import_session_as_archive(session, &mut index, &stem) {
            tracing::warn!("archive import failed: {e}");
        }
        return index;
    }

    if index.items.is_empty() {
        if let Some(legacy) = last_graph_path().filter(|p| p.is_file()) {
            if path_hint.is_empty()
                && let Ok(raw) = std::fs::read_to_string(&legacy)
            {
                let _ = session.load_graph_str(&raw);
            }
            let stem = graph_display_name(&legacy.to_string_lossy(), unnamed);
            if let Err(e) = import_session_as_archive(session, &mut index, &stem) {
                tracing::warn!("legacy graph import failed: {e}");
            }
            return index;
        }
        if let Err(e) = import_session_as_archive(session, &mut index, unnamed) {
            tracing::warn!("default archive failed: {e}");
        }
        return index;
    }

    if (path_hint.is_empty() || !index.items.iter().any(|item| item.id == path_hint))
        && let Some(meta) = index.current_meta()
    {
        match read_archive_json(&meta.id) {
            Ok(raw) => {
                if let Err(e) = session.load_graph_str(&raw) {
                    session.host.log.log(
                        0,
                        None,
                        format!("failed to load archive {}: {e}", meta.name),
                    );
                }
            }
            Err(e) => {
                session.host.log.log(
                    0,
                    None,
                    format!("failed to read archive {}: {e}", meta.name),
                );
            }
        }
        index.current = meta.id.clone();
    }

    *session.graph_path.lock() = index.current.clone();
    let _ = save_archive_index(&index);
    index
}

fn import_session_as_archive(
    session: &Session,
    index: &mut ArchiveIndex,
    name: &str,
) -> vf_core::Result<()> {
    let json = session.save_graph_str()?;
    let id = new_archive_id();
    write_archive_json(&id, &json)?;
    let name = index.unique_name(name);
    index.items.push(ArchiveMeta {
        id: id.clone(),
        name,
    });
    index.current = id.clone();
    *session.graph_path.lock() = id;
    save_archive_index(index)?;
    Ok(())
}

impl Workspace {
    pub(crate) fn graph_name(&self, cx: &App) -> String {
        self.archives
            .current_meta()
            .map(|m| m.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| t(cx, T::Unnamed).to_string())
    }

    pub(crate) fn snapshot_graph(&self) -> Graph {
        self.session.graph.lock().clone()
    }

    pub(crate) fn begin_graph_gesture(&mut self) {
        if self.gesture_before.is_none() {
            self.gesture_before = Some(self.snapshot_graph());
        }
    }

    pub(crate) fn end_graph_gesture(&mut self, cx: &mut Context<Self>) {
        let Some(before) = self.gesture_before.take() else {
            return;
        };
        self.commit_graph_change(before, cx);
    }

    pub(crate) fn commit_graph_change(&mut self, before: Graph, cx: &mut Context<Self>) {
        let now = self.snapshot_graph();
        if before == now {
            return;
        }
        self.undo_stack.push(before);
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
        if !self.archives.current.is_empty() {
            self.autosave.save(
                self.archives.current.clone(),
                now,
                self.session.engine.snapshot(),
            );
        }
        cx.notify();
    }

    fn persist_current_archive(&mut self, cx: &mut Context<Self>) {
        self.autosave.flush();
        if self.archives.current.is_empty() {
            return;
        }
        match self.session.save_graph_str() {
            Ok(json) => {
                if let Err(e) = write_archive_json(&self.archives.current, &json) {
                    let err = e.to_string();
                    self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
                    self.note(0, self.status.clone());
                }
            }
            Err(e) => {
                let err = e.to_string();
                self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
                self.note(0, self.status.clone());
            }
        }
    }

    fn persist_index(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = save_archive_index(&self.archives) {
            let err = e.to_string();
            self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
            self.note(0, self.status.clone());
        }
        *self.session.graph_path.lock() = self.archives.current.clone();
    }

    pub(crate) fn switch_archive(&mut self, id: String, cx: &mut Context<Self>) {
        self.archive_menu_open = false;
        if id == self.archives.current || !self.archives.items.iter().any(|item| item.id == id) {
            cx.notify();
            return;
        }
        self.persist_current_archive(cx);
        match read_archive_json(&id) {
            Ok(raw) => match self.session.load_graph_str(&raw) {
                Ok(()) => {
                    self.archives.current = id;
                    self.undo_stack.clear();
                    self.gesture_before = None;
                    self.reset_editor_for_graph();
                    self.persist_index(cx);
                    let name = self.graph_name(cx);
                    self.status = tf(cx, T::StatusLoaded, &[("name", &name)]);
                    self.note(2, self.status.clone());
                }
                Err(e) => {
                    let err = e.to_string();
                    self.status = tf(cx, T::StatusLoadFailed, &[("err", &err)]);
                    self.note(0, self.status.clone());
                }
            },
            Err(e) => {
                let err = e.to_string();
                self.status = tf(cx, T::StatusOpenFailed, &[("err", &err)]);
                self.note(0, self.status.clone());
            }
        }
        cx.notify();
    }

    pub(crate) fn prompt_new_graph_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.archive_menu_open = false;
        let unnamed = t(cx, T::Unnamed).to_string();
        let suggested = self.archives.unique_name(&unnamed);
        self.prompt_archive_name(
            window,
            cx,
            t(cx, T::GraphNewTitle),
            suggested,
            |this, name, window, cx| this.create_graph_archive(name, window, cx),
        );
    }

    pub(crate) fn prompt_rename_graph_archive(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(meta) = self.archives.current_meta().cloned() else {
            return;
        };
        self.archive_menu_open = false;
        let id = meta.id.clone();
        self.prompt_archive_name(
            window,
            cx,
            t(cx, T::GraphRename),
            meta.name,
            move |this, name, window, cx| this.rename_graph_archive(&id, name, window, cx),
        );
    }

    pub(crate) fn prompt_delete_graph_archive(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.archive_menu_open = false;
        let Some(meta) = self.archives.current_meta().cloned() else {
            return;
        };
        let title = t(cx, T::GraphDeleteTitle);
        let body = tf(cx, T::GraphDeleteConfirm, &[("name", &meta.name)]);
        let ok = t(cx, T::GraphDelete);
        let cancel = t(cx, T::GraphCancel);
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(title.clone())
                .description(body.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(ok.clone())
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(cancel.clone())
                        .show_cancel(true),
                )
                .on_ok({
                    let view = view.clone();
                    move |_, window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.delete_graph_archive(window, cx);
                        });
                        true
                    }
                })
        });
    }

    fn prompt_archive_name(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        title: SharedString,
        initial: String,
        on_confirm: impl Fn(&mut Self, String, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t(cx, T::GraphName))
                .default_value(initial)
        });
        name_input.update(cx, |s, cx| s.focus(window, cx));
        let ok = t(cx, T::GraphOk);
        let cancel = t(cx, T::GraphCancel);
        let view = cx.entity().downgrade();
        let on_confirm = std::rc::Rc::new(on_confirm);
        window.open_dialog(cx, move |dialog, _, _| {
            let name_input = name_input.clone();
            let on_confirm = on_confirm.clone();
            dialog
                .title(title.clone())
                .w(px(360.))
                .child(Input::new(&name_input).id("archive-name-input"))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("archive-name-cancel")
                                .small()
                                .flex_none()
                                .label(cancel.clone())
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("archive-name-ok")
                                .small()
                                .flex_none()
                                .primary()
                                .label(ok.clone())
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(
                                        Box::new(Confirm { secondary: false }),
                                        cx,
                                    );
                                }),
                        ),
                )
                .on_ok({
                    let view = view.clone();
                    move |_, window, cx| {
                        let name = name_input.read(cx).value().to_string();
                        let on_confirm = on_confirm.clone();
                        let _ = view.update(cx, |this, cx| {
                            on_confirm(this, name, window, cx);
                        });
                        true
                    }
                })
        });
    }

    pub(crate) fn create_graph_archive(
        &mut self,
        name: String,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.archive_menu_open = false;
        self.persist_current_archive(cx);
        let name = self.archives.unique_name(&name);
        let id = new_archive_id();
        *self.session.graph.lock() = Graph::default();
        self.session.recompile();
        match self.session.save_graph_str() {
            Ok(json) => {
                if let Err(e) = write_archive_json(&id, &json) {
                    let err = e.to_string();
                    self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
                    self.note(0, self.status.clone());
                    cx.notify();
                    return;
                }
            }
            Err(e) => {
                let err = e.to_string();
                self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
                self.note(0, self.status.clone());
                cx.notify();
                return;
            }
        }
        self.archives.items.push(ArchiveMeta {
            id: id.clone(),
            name: name.clone(),
        });
        self.archives.current = id;
        self.undo_stack.clear();
        self.gesture_before = None;
        self.reset_editor_for_graph();
        self.persist_index(cx);
        self.status = tf(cx, T::StatusCreated, &[("name", &name)]);
        self.note(2, self.status.clone());
        cx.notify();
    }

    pub(crate) fn rename_graph_archive(
        &mut self,
        id: &str,
        name: String,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self.archives.unique_name_except(&name, id);
        let Some(item) = self.archives.items.iter_mut().find(|item| item.id == id) else {
            return;
        };
        if item.name == name {
            return;
        }
        item.name = name.clone();
        self.persist_index(cx);
        self.status = tf(cx, T::StatusRenamed, &[("name", &name)]);
        self.note(2, self.status.clone());
        cx.notify();
    }

    pub(crate) fn delete_graph_archive(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(meta) = self.archives.current_meta().cloned() else {
            return;
        };
        self.archive_menu_open = false;
        self.autosave.flush();
        delete_archive_file(&meta.id);
        self.archives.items.retain(|item| item.id != meta.id);
        self.undo_stack.clear();
        self.gesture_before = None;
        if self.archives.items.is_empty() {
            let unnamed = t(cx, T::Unnamed).to_string();
            let id = new_archive_id();
            *self.session.graph.lock() = Graph::default();
            self.session.recompile();
            if let Ok(json) = self.session.save_graph_str() {
                let _ = write_archive_json(&id, &json);
            }
            self.archives.items.push(ArchiveMeta {
                id: id.clone(),
                name: unnamed,
            });
            self.archives.current = id;
        } else {
            let next = self.archives.items.last().cloned().unwrap();
            self.archives.current = next.id.clone();
            match read_archive_json(&next.id) {
                Ok(raw) => {
                    if let Err(e) = self.session.load_graph_str(&raw) {
                        let err = e.to_string();
                        self.status = tf(cx, T::StatusLoadFailed, &[("err", &err)]);
                        self.note(0, self.status.clone());
                    }
                }
                Err(e) => {
                    let err = e.to_string();
                    self.status = tf(cx, T::StatusOpenFailed, &[("err", &err)]);
                    self.note(0, self.status.clone());
                }
            }
        }
        self.reset_editor_for_graph();
        self.persist_index(cx);
        self.status = tf(cx, T::StatusDeleted, &[("name", &meta.name)]);
        self.note(2, self.status.clone());
        cx.notify();
    }

    pub(crate) fn pick_and_import_graph(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t(cx, T::GraphImportPrompt)),
        });
        cx.spawn(async move |this, cx| {
            let outcome = match rx.await {
                Ok(v) => v,
                Err(_) => return,
            };
            match outcome {
                Ok(Some(paths)) => {
                    if let Some(path) = paths.into_iter().next() {
                        this.update(cx, |this, cx| this.import_graph_from_path(&path, cx))
                            .ok();
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    this.update(cx, |this, cx| {
                        let err = e.to_string();
                        this.status = tf(cx, T::StatusOpenFailed, &[("err", &err)]);
                        this.note(0, this.status.clone());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub(crate) fn import_graph_from_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        let unnamed = t(cx, T::Unnamed).to_string();
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                let err = e.to_string();
                self.status = tf(cx, T::StatusOpenFailed, &[("err", &err)]);
                self.note(0, self.status.clone());
                cx.notify();
                return;
            }
        };
        self.persist_current_archive(cx);
        match self.session.load_graph_str(&raw) {
            Ok(()) => {
                let shown = path.to_string_lossy();
                let stem = graph_display_name(&shown, &unnamed);
                let name = self.archives.unique_name(&stem);
                let id = new_archive_id();
                match self.session.save_graph_str() {
                    Ok(json) => {
                        if let Err(e) = write_archive_json(&id, &json) {
                            let err = e.to_string();
                            self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
                            self.note(0, self.status.clone());
                            cx.notify();
                            return;
                        }
                    }
                    Err(e) => {
                        let err = e.to_string();
                        self.status = tf(cx, T::StatusSaveFailed, &[("err", &err)]);
                        self.note(0, self.status.clone());
                        cx.notify();
                        return;
                    }
                }
                self.archives.items.push(ArchiveMeta {
                    id: id.clone(),
                    name: name.clone(),
                });
                self.archives.current = id;
                self.undo_stack.clear();
                self.gesture_before = None;
                self.reset_editor_for_graph();
                self.persist_index(cx);
                self.status = tf(cx, T::StatusImported, &[("name", &name)]);
                self.note(2, self.status.clone());
            }
            Err(e) => {
                let err = e.to_string();
                self.status = tf(cx, T::StatusLoadFailed, &[("err", &err)]);
                self.note(0, self.status.clone());
            }
        }
        cx.notify();
    }

    pub(crate) fn export_graph(&mut self, cx: &mut Context<Self>) {
        let name = self.graph_name(cx);
        let suggested = format!("{name}.vfgraph.json");
        let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        cx.spawn(async move |this, cx| {
            let outcome = match rx.await {
                Ok(v) => v,
                Err(_) => return,
            };
            match outcome {
                Ok(Some(path)) => {
                    this.update(cx, |this, cx| this.write_export_to(&path, cx))
                        .ok();
                }
                Ok(None) => {}
                Err(e) => {
                    this.update(cx, |this, cx| {
                        let err = e.to_string();
                        this.status = tf(cx, T::StatusExportFailed, &[("err", &err)]);
                        this.note(0, this.status.clone());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn write_export_to(&mut self, path: &Path, cx: &mut Context<Self>) {
        let path = with_graph_extension(absolute_path(path));
        match self.session.save_graph_str() {
            Ok(s) => match std::fs::write(&path, s) {
                Ok(()) => {
                    let shown = path.to_string_lossy().into_owned();
                    self.status = tf(cx, T::StatusExported, &[("path", &shown)]);
                    self.note(2, self.status.clone());
                }
                Err(e) => {
                    let err = e.to_string();
                    self.status = tf(cx, T::StatusExportFailed, &[("err", &err)]);
                    self.note(0, self.status.clone());
                }
            },
            Err(e) => {
                let err = e.to_string();
                self.status = tf(cx, T::StatusExportFailed, &[("err", &err)]);
                self.note(0, self.status.clone());
            }
        }
        cx.notify();
    }

    pub(crate) fn undo_graph(&mut self, cx: &mut Context<Self>) {
        let Some(prev) = self.undo_stack.pop() else {
            self.status = t(cx, T::StatusNothingToUndo).to_string();
            cx.notify();
            return;
        };
        *self.session.graph.lock() = prev;
        self.session.recompile();
        self.persist_current_archive(cx);
        self.status = t(cx, T::StatusUndone).to_string();
        cx.notify();
    }
}
