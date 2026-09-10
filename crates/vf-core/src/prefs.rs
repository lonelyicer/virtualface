use std::path::{Path, PathBuf};

pub fn config_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("virtualface"));
        }
    }
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".config").join("virtualface"))
}

fn last_graph_file() -> Option<PathBuf> {
    Some(config_dir()?.join("last_graph"))
}

fn locale_file() -> Option<PathBuf> {
    Some(config_dir()?.join("locale"))
}

/// Saved UI locale id (`en` / `zh-CN`), if the user picked one.
pub fn load_locale() -> Option<String> {
    let raw = std::fs::read_to_string(locale_file()?).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn save_locale(id: &str) {
    let Some(dir) = config_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join("locale"), id.as_bytes());
}

/// Absolute path of the most recently opened or saved graph, if any.
pub fn last_graph_path() -> Option<PathBuf> {
    let raw = std::fs::read_to_string(last_graph_file()?).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn remember_last_graph(path: &Path) {
    let Some(dir) = config_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let stored = absolute_path(path);
    let _ = std::fs::write(dir.join("last_graph"), stored.to_string_lossy().as_bytes());
}

pub fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

pub fn with_graph_extension(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.ends_with(".vfgraph.json") || s.ends_with(".json") {
        path
    } else {
        let mut out = path;
        out.set_extension("vfgraph.json");
        out
    }
}

pub fn graph_display_name(path: &str, unnamed: &str) -> String {
    if path.is_empty() {
        return unnamed.to_string();
    }
    let name = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    name.strip_suffix(".vfgraph.json")
        .or_else(|| name.strip_suffix(".json"))
        .unwrap_or(name)
        .to_string()
}
