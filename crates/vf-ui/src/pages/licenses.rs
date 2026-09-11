use crate::i18n::{T, t};
use crate::page::AppPage;
use crate::workspace::{LicenseId, Workspace};
use gpui_kit::base::v_virtual_list;
use gpui_kit::component::button::Button;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::rc::Rc;

include!(concat!(env!("OUT_DIR"), "/third_party_crates.rs"));

const ROW_H: f32 = 34.0;
const LINE_H: f32 = 18.0;
const SECTION_H: f32 = 28.0;

#[derive(Clone, Copy)]
enum LicenseRow {
    Section(bool),
    Header(LicenseId),
    Line { text: &'static str },
}

impl LicenseRow {
    fn height(self) -> f32 {
        match self {
            Self::Section(_) => SECTION_H,
            Self::Header(_) => ROW_H,
            Self::Line { .. } => LINE_H,
        }
    }
}

impl Workspace {
    pub(crate) fn licenses_page(&self, cx: &mut Context<Self>, muted: Hsla) -> impl IntoElement {
        let border = cx.theme().border;
        let rows = Rc::new(collect_rows(self.license_expanded));
        let sizes = Rc::new(
            rows.iter()
                .map(|row| size(px(1.), px(row.height())))
                .collect::<Vec<_>>(),
        );
        let measure_ix = rows
            .iter()
            .position(|row| matches!(row, LicenseRow::Header(_)))
            .unwrap_or(0);
        v_flex()
            .id("licenses-page")
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_6()
            .gap_3()
            .overflow_hidden()
            .child(
                Button::new("licenses-back")
                    .small()
                    .label(t(cx, T::LicensesBack))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = AppPage::Settings;
                        this.license_expanded = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(t(cx, T::LicensesIntro)),
            )
            .child(
                crate_header(cx, muted)
                    .px_2()
                    .h(px(ROW_H))
                    .border_b_1()
                    .border_color(border),
            )
            .child(
                v_flex()
                    .id("licenses-viewport")
                    .flex_1()
                    .size_full()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .relative()
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(
                        v_virtual_list(cx.entity(), "license-rows", sizes, {
                            let rows = rows.clone();
                            move |this, range, _, cx| {
                                let muted = cx.theme().muted_foreground;
                                let border = cx.theme().border;
                                let mut out = Vec::with_capacity(range.len());
                                for i in range {
                                    out.push(
                                        this.render_license_row(i, rows[i], muted, border, cx),
                                    );
                                }
                                out
                            }
                        })
                        .with_item_to_measure_index(measure_ix)
                        .size_full()
                        .track_scroll(&self.license_scroll),
                    )
                    .child(Scrollbar::vertical(&self.license_scroll)),
            )
    }

    fn render_license_row(
        &self,
        ix: usize,
        row: LicenseRow,
        muted: Hsla,
        border: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match row {
            LicenseRow::Section(app) => h_flex()
                .id(("license-section", ix))
                .w_full()
                .h(px(SECTION_H))
                .px_2()
                .items_center()
                .text_xs()
                .text_color(muted)
                .child(t(
                    cx,
                    if app {
                        T::LicensesThisApp
                    } else {
                        T::LicensesLibraries
                    },
                ))
                .into_any_element(),
            LicenseRow::Header(id) => {
                let (name, version, spdx, expanded) = crate_meta(id, self.license_expanded, cx);
                crate_row(name, version, spdx, muted, expanded)
                    .id(("license-header", ix))
                    .h(px(ROW_H))
                    .px_2()
                    .border_b_1()
                    .border_color(border.opacity(0.6))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(0x3f3f46).opacity(0.45)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.license_expanded = if this.license_expanded == Some(id) {
                            None
                        } else {
                            Some(id)
                        };
                        cx.notify();
                    }))
                    .into_any_element()
            }
            LicenseRow::Line { text } => {
                let body: SharedString = if text.is_empty() {
                    t(cx, T::LicensesNoText)
                } else {
                    SharedString::from(text)
                };
                div()
                    .id(("license-line", ix))
                    .w_full()
                    .h(px(LINE_H))
                    .px_3()
                    .bg(rgb(0x18181b))
                    .text_xs()
                    .text_color(muted)
                    .truncate()
                    .child(body)
                    .into_any_element()
            }
        }
    }
}

fn collect_rows(expanded: Option<LicenseId>) -> Vec<LicenseRow> {
    let mut rows = Vec::with_capacity(THIRD_PARTY.len() + 8);
    rows.push(LicenseRow::Section(true));
    push_crate(&mut rows, LicenseId::App, APP_LICENSE_TEXT, expanded);
    rows.push(LicenseRow::Section(false));
    for (i, crate_) in THIRD_PARTY.iter().enumerate() {
        push_crate(&mut rows, LicenseId::Third(i), crate_.text, expanded);
    }
    rows
}

fn push_crate(
    rows: &mut Vec<LicenseRow>,
    id: LicenseId,
    text: &'static str,
    expanded: Option<LicenseId>,
) {
    rows.push(LicenseRow::Header(id));
    if expanded != Some(id) {
        return;
    }
    if text.is_empty() {
        rows.push(LicenseRow::Line { text: "" });
        return;
    }
    for line in text.lines() {
        rows.push(LicenseRow::Line { text: line });
    }
}

fn crate_meta(
    id: LicenseId,
    expanded: Option<LicenseId>,
    cx: &App,
) -> (&'static str, &'static str, SharedString, bool) {
    let open = expanded == Some(id);
    match id {
        LicenseId::App => {
            let spdx = if env!("CARGO_PKG_LICENSE").is_empty() {
                t(cx, T::LicensesUnknown)
            } else {
                SharedString::from(env!("CARGO_PKG_LICENSE"))
            };
            ("VirtualFace", env!("CARGO_PKG_VERSION"), spdx, open)
        }
        LicenseId::Third(i) => {
            let crate_ = &THIRD_PARTY[i];
            let spdx = if crate_.license.is_empty() {
                t(cx, T::LicensesUnknown)
            } else {
                SharedString::from(crate_.license)
            };
            (crate_.name, crate_.version, spdx, open)
        }
    }
}

fn crate_header(cx: &App, muted: Hsla) -> Div {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .text_xs()
        .text_color(muted)
        .child(div().w(px(16.)))
        .child(div().flex_1().min_w(px(0.)).child(t(cx, T::LicensesName)))
        .child(div().w(px(96.)).child(t(cx, T::LicensesVersion)))
        .child(div().w(px(200.)).child(t(cx, T::LicensesLicense)))
}

fn crate_row(
    name: impl Into<SharedString>,
    version: impl Into<SharedString>,
    license: SharedString,
    muted: Hsla,
    expanded: bool,
) -> Div {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(
            div()
                .w(px(16.))
                .text_xs()
                .text_color(muted)
                .child(if expanded { "▾" } else { "▸" }),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_xs()
                .truncate()
                .child(name.into()),
        )
        .child(
            div()
                .w(px(96.))
                .text_xs()
                .text_color(muted)
                .truncate()
                .child(version.into()),
        )
        .child(div().w(px(200.)).text_xs().truncate().child(license))
}
