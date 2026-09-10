use crate::i18n::{T, t};
use gpui_kit::{App, SharedString};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppPage {
    Home,
    Graph,
    Settings,
    Log,
}

impl AppPage {
    pub(crate) fn title(self, cx: &App) -> SharedString {
        t(
            cx,
            match self {
                Self::Home => T::NavHome,
                Self::Graph => T::NavGraph,
                Self::Settings => T::NavSettings,
                Self::Log => T::NavLog,
            },
        )
    }
}
