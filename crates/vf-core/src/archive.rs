//! Internal graph archives under the app config directory.
//!
//! Layout:
//!   ~/.config/virtualface/graphs/index.json
//!   ~/.config/virtualface/graphs/{id}.vfgraph.json

use crate::error::{CoreError, Result};
use crate::prefs::config_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ARCHIVE_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMeta {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveIndex {
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub items: Vec<ArchiveMeta>,
}

impl ArchiveIndex {
    pub fn current_meta(&self) -> Option<&ArchiveMeta> {
        self.items
            .iter()
            .find(|item| item.id == self.current)
            .or_else(|| self.items.first())
    }

    pub fn selected_index(&self) -> usize {
        self.items
            .iter()
            .position(|item| item.id == self.current)
            .unwrap_or(0)
    }

    pub fn unique_name(&self, base: &str) -> String {
        unique_archive_name_except(&self.items, base, None)
    }

    pub fn unique_name_except(&self, base: &str, except_id: &str) -> String {
        unique_archive_name_except(&self.items, base, Some(except_id))
    }
}

pub fn unique_archive_name(items: &[ArchiveMeta], base: &str) -> String {
    unique_archive_name_except(items, base, None)
}

fn unique_archive_name_except(
    items: &[ArchiveMeta],
    base: &str,
    except_id: Option<&str>,
) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "Untitled" } else { base };
    let taken = |name: &str| {
        items
            .iter()
            .any(|item| item.name == name && Some(item.id.as_str()) != except_id)
    };
    if !taken(base) {
        return base.to_string();
    }
    for n in 2..10_000 {
        let candidate = format!("{base} {n}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    format!("{base} x")
}

pub fn valid_archive_id(id: &str) -> bool {
    let mut n = 0usize;
    for c in id.chars() {
        if !c.is_ascii_hexdigit() {
            return false;
        }
        n += 1;
        if n > 64 {
            return false;
        }
    }
    n > 0
}

pub fn new_archive_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = ARCHIVE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}{seq:x}")
}

pub fn graphs_dir() -> Option<PathBuf> {
    Some(config_dir()?.join("graphs"))
}

fn index_path() -> Option<PathBuf> {
    Some(graphs_dir()?.join("index.json"))
}

fn archive_path(id: &str) -> Option<PathBuf> {
    if !valid_archive_id(id) {
        return None;
    }
    Some(graphs_dir()?.join(format!("{id}.vfgraph.json")))
}

pub fn load_archive_index() -> ArchiveIndex {
    let Some(path) = index_path() else {
        return ArchiveIndex::default();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return ArchiveIndex::default();
    };
    let mut index: ArchiveIndex = serde_json::from_str(&raw).unwrap_or_default();
    index.items.retain(|item| valid_archive_id(&item.id));
    if !index.items.iter().any(|item| item.id == index.current) {
        index.current = index
            .items
            .first()
            .map(|item| item.id.clone())
            .unwrap_or_default();
    }
    index
}

pub fn save_archive_index(index: &ArchiveIndex) -> Result<()> {
    let dir = graphs_dir().ok_or_else(|| CoreError::Graph("no config directory".into()))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("index.json");
    let json = serde_json::to_string_pretty(index)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn read_archive_json(id: &str) -> Result<String> {
    let path = archive_path(id).ok_or_else(|| CoreError::Graph("invalid archive id".into()))?;
    Ok(std::fs::read_to_string(path)?)
}

pub fn write_archive_json(id: &str, json: &str) -> Result<()> {
    let dir = graphs_dir().ok_or_else(|| CoreError::Graph("no config directory".into()))?;
    if !valid_archive_id(id) {
        return Err(CoreError::Graph("invalid archive id".into()));
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{id}.vfgraph.json")), json)?;
    Ok(())
}

pub fn delete_archive_file(id: &str) {
    if let Some(path) = archive_path(id) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_increment() {
        let items = vec![
            ArchiveMeta {
                id: "a".into(),
                name: "Untitled".into(),
            },
            ArchiveMeta {
                id: "b".into(),
                name: "Untitled 2".into(),
            },
        ];
        assert_eq!(unique_archive_name(&items, "Untitled"), "Untitled 3");
        assert_eq!(unique_archive_name(&items, "Other"), "Other");
        assert_eq!(unique_archive_name(&items, "  "), "Untitled 3");
        assert_eq!(
            unique_archive_name_except(&items, "Untitled", Some("a")),
            "Untitled"
        );
    }

    #[test]
    fn archive_ids_are_hex() {
        let id = new_archive_id();
        assert!(valid_archive_id(&id));
        assert!(!valid_archive_id(""));
        assert!(!valid_archive_id("../x"));
        assert!(!valid_archive_id("ab-cd"));
    }
}
