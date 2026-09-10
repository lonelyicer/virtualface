use crate::i18n::{T, t};
use crate::page::AppPage;
use crate::workspace::Workspace;
use gpui_kit::component::button::Button;
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
use gpui_kit::component::select::Select;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Workspace {
    pub(crate) fn settings_page(&self, cx: &mut Context<Self>, muted: Hsla) -> impl IntoElement {
        v_flex()
            .id("settings-page")
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_6()
            .gap_4()
            .overflow_y_scroll()
            .child(
                GroupBox::new()
                    .id("set-language")
                    .outline()
                    .title(t(cx, T::SettingsLanguage))
                    .child(
                        div().w(px(240.)).child(
                            Select::new(&self.language_select)
                                .id("settings-language")
                                .w_full()
                                .menu_width(px(240.)),
                        ),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("set-plugins")
                    .outline()
                    .title(t(cx, T::SettingsPlugins))
                    .children(self.session.host.plugins().iter().map(|p| {
                        v_flex()
                            .gap_1()
                            .child(div().font_weight(FontWeight::MEDIUM).child(p.name.clone()))
                            .child(div().text_xs().text_color(muted).child(format!(
                                "{}  {}  {}",
                                p.id,
                                p.version,
                                p.path.display()
                            )))
                    }))
                    .children(
                        self.session
                            .load_errors
                            .iter()
                            .map(|e| div().text_xs().text_color(rgb(0xef4444)).child(e.clone())),
                    ),
            )
            .child(
                GroupBox::new()
                    .id("set-licenses")
                    .outline()
                    .title(t(cx, T::SettingsLicenses))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t(cx, T::SettingsLicensesHint)),
                    )
                    .child(
                        Button::new("set-licenses-open")
                            .label(t(cx, T::SettingsLicensesOpen))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = AppPage::Licenses;
                                cx.notify();
                            })),
                    ),
            )
    }
}
