//! VirtualFace gpui-kit UI.

mod archives;
mod autosave;
mod chrome;
mod flow;
mod i18n;
mod page;
pub(crate) mod pages;
mod theme;
mod workspace;

pub use theme::lock_dark_theme;
pub use workspace::{Workspace, open_workspace};

/// Call after `gpui_kit::init`. Forces dark theme; light mode is not offered.
pub fn init(cx: &mut gpui_kit::App) {
    lock_dark_theme(cx);
    i18n::init(cx);
}
