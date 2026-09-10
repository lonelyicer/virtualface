//! UI locale. Catalogs live in `crates/vf-ui/locales/*.json`; add a file to ship a language.

use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::{App, Global, SharedString};
use std::collections::HashMap;
use std::sync::OnceLock;
use vf_core::{load_locale, save_locale};

include!(concat!(env!("OUT_DIR"), "/locale_files.rs"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locale {
    id: &'static str,
}

impl Locale {
    pub fn id(self) -> &'static str {
        self.id
    }

    pub fn native_name(self) -> &'static str {
        catalog(self).map(|c| c.name.as_str()).unwrap_or(self.id)
    }

    pub fn all() -> impl Iterator<Item = Locale> {
        store().locales.iter().copied()
    }

    pub fn fallback() -> Self {
        store().fallback
    }

    pub fn parse(id: &str) -> Option<Self> {
        let id = id.trim();
        if id.is_empty() {
            return None;
        }
        store().catalogs.iter().find_map(|c| {
            if c.id == id || c.aliases.iter().any(|a| a == id) {
                Some(Locale { id: c.id })
            } else {
                None
            }
        })
    }

    pub fn detect() -> Self {
        if let Some(saved) = load_locale().and_then(|s| Self::parse(&s)) {
            return saved;
        }
        let hint = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_MESSAGES"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        let stem = hint
            .split(['.', '@'])
            .next()
            .unwrap_or("")
            .replace('_', "-");
        Self::parse(&stem)
            .or_else(|| stem.split('-').next().and_then(Self::parse))
            .unwrap_or_else(Self::fallback)
    }
}

impl SearchableListItem for Locale {
    type Value = Locale;

    fn title(&self) -> SharedString {
        SharedString::from(self.native_name())
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

struct Catalog {
    id: &'static str,
    name: String,
    aliases: Vec<String>,
    messages: HashMap<String, String>,
}

struct Store {
    catalogs: Vec<Catalog>,
    locales: Vec<Locale>,
    fallback: Locale,
}

fn store() -> &'static Store {
    static STORE: OnceLock<Store> = OnceLock::new();
    STORE.get_or_init(load_store)
}

fn load_store() -> Store {
    let mut catalogs = Vec::new();
    for (id, raw) in LOCALE_FILES {
        catalogs.push(parse_catalog(id, raw));
    }
    catalogs.sort_by_key(|c| c.id);
    if let Some(en) = catalogs.iter().find(|c| c.id == "en") {
        for key in T::ALL {
            assert!(
                en.messages.contains_key(key.as_str()),
                "locales/en.json missing key {}",
                key.as_str()
            );
        }
    }
    let locales: Vec<Locale> = catalogs.iter().map(|c| Locale { id: c.id }).collect();
    let fallback = locales
        .iter()
        .copied()
        .find(|l| l.id == "en")
        .or_else(|| locales.first().copied())
        .expect("at least one locale JSON");
    Store {
        catalogs,
        locales,
        fallback,
    }
}

fn parse_catalog(id: &'static str, raw: &str) -> Catalog {
    let root: serde_json::Value =
        serde_json::from_str(raw).unwrap_or_else(|e| panic!("locale {id}: {e}"));
    let meta = root.get("meta");
    let name = meta
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .to_string();
    let mut aliases: Vec<String> = meta
        .and_then(|m| m.get("aliases"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !aliases.iter().any(|a| a == id) {
        aliases.push(id.to_string());
    }
    let mut messages = HashMap::new();
    flatten(&root, "", &mut messages);
    Catalog {
        id,
        name,
        aliases,
        messages,
    }
}

fn flatten(value: &serde_json::Value, prefix: &str, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if prefix.is_empty() && k == "meta" {
                    continue;
                }
                let next = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(v, &next, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        _ => {}
    }
}

fn catalog(loc: Locale) -> Option<&'static Catalog> {
    store().catalogs.iter().find(|c| c.id == loc.id)
}

pub struct I18n {
    pub locale: Locale,
}

impl Global for I18n {}

pub fn init(cx: &mut App) {
    let _ = store();
    cx.set_global(I18n {
        locale: Locale::detect(),
    });
}

pub fn locale(cx: &App) -> Locale {
    cx.global::<I18n>().locale
}

pub fn set_locale(cx: &mut App, loc: Locale) {
    cx.global_mut::<I18n>().locale = loc;
    save_locale(loc.id());
}

pub fn t(cx: &App, key: T) -> SharedString {
    SharedString::from(t_loc(locale(cx), key))
}

pub fn tf(cx: &App, key: T, args: &[(&str, &str)]) -> String {
    interpolate(t_loc(locale(cx), key), args)
}

pub fn t_loc(loc: Locale, key: T) -> &'static str {
    lookup(loc, key)
        .or_else(|| lookup(Locale::fallback(), key))
        .unwrap_or_else(|| key.as_str())
}

pub fn tf_loc(loc: Locale, key: T, args: &[(&str, &str)]) -> String {
    interpolate(t_loc(loc, key), args)
}

fn lookup(loc: Locale, key: T) -> Option<&'static str> {
    catalog(loc)?.messages.get(key.as_str()).map(String::as_str)
}

fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    NavHome,
    NavGraph,
    NavSettings,
    NavLog,
    HomeRunning,
    HomeStopped,
    HomeStart,
    HomeStop,
    HomePlugins,
    HomeActivePlugins,
    HomeDataIn,
    HomeDataOut,
    HomeNoActive,
    SettingsPlugins,
    SettingsLanguage,
    SettingsLicenses,
    SettingsLicensesHint,
    SettingsLicensesOpen,
    LicensesTitle,
    LicensesBack,
    LicensesIntro,
    LicensesThisApp,
    LicensesLibraries,
    LicensesName,
    LicensesVersion,
    LicensesLicense,
    LicensesUnknown,
    LicensesNoText,
    GraphImport,
    GraphExport,
    GraphNew,
    GraphNewTitle,
    GraphRename,
    GraphDelete,
    GraphDeleteTitle,
    GraphDeleteConfirm,
    GraphUndo,
    GraphImportPrompt,
    GraphSearch,
    GraphVariables,
    GraphVarsHint,
    GraphCreateVar,
    GraphEditVar,
    GraphCancel,
    GraphOk,
    GraphName,
    GraphType,
    GraphValue,
    GraphInvalidValue,
    Unnamed,
    CatInput,
    CatProcess,
    CatOutput,
    CatUtility,
    GetVar,
    SetVar,
    PluginMissing,
    MissingNode,
    IncompatiblePorts,
    LogEmpty,
    StatusReady,
    StatusRunning,
    StatusStopped,
    StatusCreated,
    StatusImported,
    StatusExported,
    StatusDeleted,
    StatusRenamed,
    StatusUndone,
    StatusNothingToUndo,
    StatusSaveFailed,
    StatusLoaded,
    StatusLoadFailed,
    StatusOpenFailed,
    StatusExportFailed,
    StatusVariable,
    TypeFloat,
    TypeInt,
    TypeBool,
    TypeString,
}

impl T {
    pub const ALL: &'static [T] = &[
        Self::NavHome,
        Self::NavGraph,
        Self::NavSettings,
        Self::NavLog,
        Self::HomeRunning,
        Self::HomeStopped,
        Self::HomeStart,
        Self::HomeStop,
        Self::HomePlugins,
        Self::HomeActivePlugins,
        Self::HomeDataIn,
        Self::HomeDataOut,
        Self::HomeNoActive,
        Self::SettingsPlugins,
        Self::SettingsLanguage,
        Self::SettingsLicenses,
        Self::SettingsLicensesHint,
        Self::SettingsLicensesOpen,
        Self::LicensesTitle,
        Self::LicensesBack,
        Self::LicensesIntro,
        Self::LicensesThisApp,
        Self::LicensesLibraries,
        Self::LicensesName,
        Self::LicensesVersion,
        Self::LicensesLicense,
        Self::LicensesUnknown,
        Self::LicensesNoText,
        Self::GraphImport,
        Self::GraphExport,
        Self::GraphNew,
        Self::GraphNewTitle,
        Self::GraphRename,
        Self::GraphDelete,
        Self::GraphDeleteTitle,
        Self::GraphDeleteConfirm,
        Self::GraphUndo,
        Self::GraphImportPrompt,
        Self::GraphSearch,
        Self::GraphVariables,
        Self::GraphVarsHint,
        Self::GraphCreateVar,
        Self::GraphEditVar,
        Self::GraphCancel,
        Self::GraphOk,
        Self::GraphName,
        Self::GraphType,
        Self::GraphValue,
        Self::GraphInvalidValue,
        Self::Unnamed,
        Self::CatInput,
        Self::CatProcess,
        Self::CatOutput,
        Self::CatUtility,
        Self::GetVar,
        Self::SetVar,
        Self::PluginMissing,
        Self::MissingNode,
        Self::IncompatiblePorts,
        Self::LogEmpty,
        Self::StatusReady,
        Self::StatusRunning,
        Self::StatusStopped,
        Self::StatusCreated,
        Self::StatusImported,
        Self::StatusExported,
        Self::StatusDeleted,
        Self::StatusRenamed,
        Self::StatusUndone,
        Self::StatusNothingToUndo,
        Self::StatusSaveFailed,
        Self::StatusLoaded,
        Self::StatusLoadFailed,
        Self::StatusOpenFailed,
        Self::StatusExportFailed,
        Self::StatusVariable,
        Self::TypeFloat,
        Self::TypeInt,
        Self::TypeBool,
        Self::TypeString,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NavHome => "nav.home",
            Self::NavGraph => "nav.graph",
            Self::NavSettings => "nav.settings",
            Self::NavLog => "nav.log",
            Self::HomeRunning => "home.running",
            Self::HomeStopped => "home.stopped",
            Self::HomeStart => "home.start",
            Self::HomeStop => "home.stop",
            Self::HomePlugins => "home.plugins",
            Self::HomeActivePlugins => "home.active_plugins",
            Self::HomeDataIn => "home.data_in",
            Self::HomeDataOut => "home.data_out",
            Self::HomeNoActive => "home.no_active",
            Self::SettingsPlugins => "settings.plugins",
            Self::SettingsLanguage => "settings.language",
            Self::SettingsLicenses => "settings.licenses",
            Self::SettingsLicensesHint => "settings.licenses_hint",
            Self::SettingsLicensesOpen => "settings.licenses_open",
            Self::LicensesTitle => "licenses.title",
            Self::LicensesBack => "licenses.back",
            Self::LicensesIntro => "licenses.intro",
            Self::LicensesThisApp => "licenses.this_app",
            Self::LicensesLibraries => "licenses.libraries",
            Self::LicensesName => "licenses.name",
            Self::LicensesVersion => "licenses.version",
            Self::LicensesLicense => "licenses.license",
            Self::LicensesUnknown => "licenses.unknown",
            Self::LicensesNoText => "licenses.no_text",
            Self::GraphImport => "graph.import",
            Self::GraphExport => "graph.export",
            Self::GraphNew => "graph.new",
            Self::GraphNewTitle => "graph.new_title",
            Self::GraphRename => "graph.rename",
            Self::GraphDelete => "graph.delete",
            Self::GraphDeleteTitle => "graph.delete_title",
            Self::GraphDeleteConfirm => "graph.delete_confirm",
            Self::GraphUndo => "graph.undo",
            Self::GraphImportPrompt => "graph.import_prompt",
            Self::GraphSearch => "graph.search",
            Self::GraphVariables => "graph.variables",
            Self::GraphVarsHint => "graph.vars_hint",
            Self::GraphCreateVar => "graph.create_var",
            Self::GraphEditVar => "graph.edit_var",
            Self::GraphCancel => "graph.cancel",
            Self::GraphOk => "graph.ok",
            Self::GraphName => "graph.name",
            Self::GraphType => "graph.type",
            Self::GraphValue => "graph.value",
            Self::GraphInvalidValue => "graph.invalid_value",
            Self::Unnamed => "graph.unnamed",
            Self::CatInput => "category.input",
            Self::CatProcess => "category.process",
            Self::CatOutput => "category.output",
            Self::CatUtility => "category.utility",
            Self::GetVar => "graph.get_var",
            Self::SetVar => "graph.set_var",
            Self::PluginMissing => "graph.plugin_missing",
            Self::MissingNode => "graph.missing_node",
            Self::IncompatiblePorts => "graph.incompatible_ports",
            Self::LogEmpty => "log.empty",
            Self::StatusReady => "status.ready",
            Self::StatusRunning => "status.running",
            Self::StatusStopped => "status.stopped",
            Self::StatusCreated => "status.created",
            Self::StatusImported => "status.imported",
            Self::StatusExported => "status.exported",
            Self::StatusDeleted => "status.deleted",
            Self::StatusRenamed => "status.renamed",
            Self::StatusUndone => "status.undone",
            Self::StatusNothingToUndo => "status.nothing_to_undo",
            Self::StatusSaveFailed => "status.save_failed",
            Self::StatusLoaded => "status.loaded",
            Self::StatusLoadFailed => "status.load_failed",
            Self::StatusOpenFailed => "status.open_failed",
            Self::StatusExportFailed => "status.export_failed",
            Self::StatusVariable => "status.variable",
            Self::TypeFloat => "type.float",
            Self::TypeInt => "type.int",
            Self::TypeBool => "type.bool",
            Self::TypeString => "type.string",
        }
    }
}

pub fn category_key(cat: vf_sdk::Category) -> T {
    match cat {
        vf_sdk::Category::Input => T::CatInput,
        vf_sdk::Category::Process => T::CatProcess,
        vf_sdk::Category::Output => T::CatOutput,
        vf_sdk::Category::Utility => T::CatUtility,
    }
}

pub fn var_type_key(ty: vf_core::VarType) -> T {
    match ty {
        vf_core::VarType::Float => T::TypeFloat,
        vf_core::VarType::Int => T::TypeInt,
        vf_core::VarType::Bool => T::TypeBool,
        vf_core::VarType::String => T::TypeString,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_interpolate() {
        assert_eq!(Locale::parse("zh_CN").map(Locale::id), Some("zh_Hans"));
        assert_eq!(Locale::parse("en-US").map(Locale::id), Some("en"));
        assert_eq!(
            interpolate("saved {path}", &[("path", "/tmp/a")]),
            "saved /tmp/a"
        );
        let en = Locale::parse("en").unwrap();
        let zh = Locale::parse("zh_Hans").unwrap();
        assert_ne!(t_loc(en, T::NavHome), t_loc(zh, T::NavHome));
    }

    #[test]
    fn catalogs_cover_every_key() {
        let en = Locale::parse("en").expect("en locale");
        for key in T::ALL {
            assert!(
                lookup(en, *key).is_some(),
                "en.json missing {}",
                key.as_str()
            );
        }
        for loc in Locale::all() {
            for key in T::ALL {
                assert!(
                    !t_loc(loc, *key).is_empty(),
                    "{} empty for {}",
                    key.as_str(),
                    loc.id()
                );
            }
        }
    }
}
