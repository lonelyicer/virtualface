#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppPage {
    Home,
    Graph,
    Settings,
    Log,
}

impl AppPage {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Home => "主页",
            Self::Graph => "节点图",
            Self::Settings => "设置",
            Self::Log => "日志",
        }
    }
}
