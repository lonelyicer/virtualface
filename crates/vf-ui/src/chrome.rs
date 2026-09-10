use crate::page::AppPage;
use crate::workspace::Workspace;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, InteractiveElementExt, Sizable, h_flex, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

const TITLE_BAR_H: f32 = 72.0;
const TITLE_BTN: f32 = 32.0;
const SIDEBAR_TITLE_PL: f32 = 28.0;

impl Workspace {
    pub(crate) fn title_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .id("title-bar")
            .flex_shrink_0()
            .w_full()
            .h(px(TITLE_BAR_H))
            .items_center()
            .px_6()
            .bg(cx.theme().background)
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.title_should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.title_should_move = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.title_should_move {
                    this.title_should_move = false;
                    window.start_window_move();
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, _, _| {
                this.title_should_move = false;
            }))
            .when(cfg!(target_os = "linux"), |this| {
                this.on_double_click(|_, window, _| window.zoom_window())
            })
            .child(
                h_flex()
                    .id("title-bar-label")
                    .flex_1()
                    .h_full()
                    .min_w(px(0.))
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .whitespace_nowrap()
                            .child(self.page.title(cx)),
                    ),
            )
            .child(window_controls(window, cx))
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
                h_flex()
                    .id("sidebar-title")
                    .w_full()
                    .h(px(TITLE_BAR_H))
                    .flex_shrink_0()
                    .items_center()
                    .pl(px(SIDEBAR_TITLE_PL))
                    .pr_4()
                    .child(
                        div()
                            .text_2xl()
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
                        AppPage::Home.title(cx),
                        IconName::LayoutDashboard,
                        page == AppPage::Home,
                        cx,
                        AppPage::Home,
                    ))
                    .child(nav_item(
                        "nav-graph",
                        AppPage::Graph.title(cx),
                        IconName::Network,
                        page == AppPage::Graph,
                        cx,
                        AppPage::Graph,
                    ))
                    .child(nav_item(
                        "nav-settings",
                        AppPage::Settings.title(cx),
                        IconName::Settings,
                        page == AppPage::Settings || page == AppPage::Licenses,
                        cx,
                        AppPage::Settings,
                    ))
                    .child(nav_item(
                        "nav-log",
                        AppPage::Log.title(cx),
                        IconName::FileText,
                        page == AppPage::Log,
                        cx,
                        AppPage::Log,
                    )),
            )
            .child(
                v_flex().px_3().pb_3().child(
                    h_flex()
                        .id("sidebar-version")
                        .w_full()
                        .h(px(44.))
                        .px_3()
                        .gap_3()
                        .items_center()
                        .rounded_md()
                        .text_base()
                        .child(Icon::new(IconName::Info).large())
                        .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                ),
            )
    }
}

fn window_controls(window: &Window, cx: &mut Context<Workspace>) -> impl IntoElement {
    if cfg!(target_os = "macos") || cfg!(target_family = "wasm") {
        return div().id("window-controls").into_any_element();
    }
    #[cfg(target_os = "linux")]
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return div().id("window-controls").into_any_element();
    }
    let supported = window.window_controls();
    h_flex()
        .id("window-controls")
        .items_center()
        .flex_shrink_0()
        .gap_2()
        .when(supported.minimize, |this| {
            this.child(window_btn(
                "minimize",
                IconName::WindowMinimize,
                WindowControlArea::Min,
                cx,
                |window, _| window.minimize_window(),
            ))
        })
        .when(supported.maximize, |this| {
            let restore = window.is_maximized();
            this.child(window_btn(
                if restore { "restore" } else { "maximize" },
                if restore {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                },
                WindowControlArea::Max,
                cx,
                |window, _| window.zoom_window(),
            ))
        })
        .child(window_btn(
            "close",
            IconName::WindowClose,
            WindowControlArea::Close,
            cx,
            |window, _| window.remove_window(),
        ))
        .into_any_element()
}

fn window_btn(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    cx: &mut Context<Workspace>,
    on_click: impl Fn(&mut Window, &mut Context<Workspace>) + 'static,
) -> impl IntoElement {
    let is_close = id == "close";
    let hover_bg = if is_close {
        cx.theme().danger
    } else {
        cx.theme().secondary_hover
    };
    let hover_fg = if is_close {
        cx.theme().danger_foreground
    } else {
        cx.theme().secondary_foreground
    };
    div()
        .id(id)
        .flex()
        .w(px(TITLE_BTN))
        .h(px(TITLE_BTN))
        .flex_shrink_0()
        .justify_center()
        .items_center()
        .rounded_full()
        .cursor_pointer()
        .window_control_area(area)
        .hover(|d| d.bg(hover_bg).text_color(hover_fg))
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(cx.listener(move |_, _, window, cx| {
            cx.stop_propagation();
            on_click(window, cx);
        }))
        .child(Icon::new(icon))
}

fn nav_item(
    id: &'static str,
    label: SharedString,
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
