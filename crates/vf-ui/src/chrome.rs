use crate::page::AppPage;
use crate::workspace::Workspace;
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Workspace {
    pub(crate) fn title_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().h(px(48.))
    }

    pub(crate) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page;
        v_flex()
            .id("app-nav")
            .w(px(220.))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().tokens.sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                div().w_full().px_4().py_4().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Virtual Face"),
                ),
            )
            .child(
                v_flex()
                    .id("nav-items")
                    .flex_1()
                    .px_3()
                    .gap_1()
                    .child(nav_item(
                        "nav-home",
                        AppPage::Home.title(),
                        IconName::LayoutDashboard,
                        page == AppPage::Home,
                        cx,
                        AppPage::Home,
                    ))
                    .child(nav_item(
                        "nav-graph",
                        AppPage::Graph.title(),
                        IconName::Network,
                        page == AppPage::Graph,
                        cx,
                        AppPage::Graph,
                    ))
                    .child(nav_item(
                        "nav-settings",
                        AppPage::Settings.title(),
                        IconName::Settings,
                        page == AppPage::Settings,
                        cx,
                        AppPage::Settings,
                    ))
                    .child(nav_item(
                        "nav-log",
                        AppPage::Log.title(),
                        IconName::FileText,
                        page == AppPage::Log,
                        cx,
                        AppPage::Log,
                    )),
            )
    }
}

fn nav_item(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    active: bool,
    cx: &mut Context<Workspace>,
    page: AppPage,
) -> impl IntoElement {
    let accent = cx.theme().tokens.sidebar_accent;
    let accent_fg = cx.theme().sidebar_accent_foreground;
    h_flex()
        .id(id)
        .w_full()
        .h(px(44.))
        .px_3()
        .gap_3()
        .items_center()
        .rounded_md()
        .cursor_pointer()
        .text_base()
        .when(active, |d| {
            d.bg(accent)
                .text_color(accent_fg)
                .font_weight(FontWeight::MEDIUM)
        })
        .when(!active, |d| {
            d.hover(|d| d.bg(accent.opacity(0.8)).text_color(accent_fg))
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.page = page;
            cx.notify();
        }))
        .child(Icon::new(icon).large())
        .child(label)
}
