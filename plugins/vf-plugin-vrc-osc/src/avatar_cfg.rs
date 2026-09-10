//! VRChat OSC avatar JSON (`OSC/<usr_*>/Avatars/<avtr_*>.json`).
//!
//! This is how VRCFT decides which parameters to send: match each derived name
//! against `input.address` (not the OSCQuery tree), then send to that address.
//! Files are UTF-8 with BOM.

use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct AvatarConfig {
    pub id: String,
    pub name: String,
    pub addresses: HashSet<String>,
}

pub fn parse_avatar_config(json: &str) -> Option<AvatarConfig> {
    let json = json.trim_start_matches('\u{feff}');
    let v: Value = serde_json::from_str(json).ok()?;
    let id = v.get("id").and_then(Value::as_str)?.to_string();
    if id.is_empty() {
        return None;
    }
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut addresses = HashSet::new();
    if let Some(arr) = v.get("parameters").and_then(Value::as_array) {
        for p in arr {
            if let Some(addr) = p
                .get("input")
                .and_then(|i| i.get("address"))
                .and_then(Value::as_str)
            {
                addresses.insert(addr.to_string());
            }
        }
    }
    Some(AvatarConfig {
        id,
        name,
        addresses,
    })
}

pub fn find_avatar_config(id: &str) -> Option<AvatarConfig> {
    if id.is_empty() {
        return None;
    }
    for root in osc_dirs() {
        if let Some(cfg) = scan_user_folders(&root, Some(id)) {
            return Some(cfg);
        }
    }
    None
}

pub fn latest_avatar_config() -> Option<AvatarConfig> {
    let mut best: Option<(SystemTime, AvatarConfig)> = None;
    for root in osc_dirs() {
        for (mtime, cfg) in iter_avatar_files(&root) {
            if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                best = Some((mtime, cfg));
            }
        }
    }
    best.map(|(_, c)| c)
}

/// VRCFT `BaseParam` regex: `(?<!(v\d+))(/name)$|^(name)$`.
pub fn vrcft_address_matches(address: &str, param_name: &str) -> bool {
    if address == param_name {
        return true;
    }
    let suffix = format!("/{param_name}");
    let Some(idx) = address.rfind(&suffix) else {
        return false;
    };
    if idx + suffix.len() != address.len() {
        return false;
    }
    !ends_with_v_digits(&address[..idx])
}

pub fn find_param_address(addresses: &HashSet<String>, param_name: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for addr in addresses {
        if !vrcft_address_matches(addr, param_name) {
            continue;
        }
        match &best {
            None => best = Some(addr.clone()),
            Some(cur) if addr.len() > cur.len() => best = Some(addr.clone()),
            _ => {}
        }
    }
    best
}

fn ends_with_v_digits(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = b.len();
    let mut digits = false;
    while i > 0 && b[i - 1].is_ascii_digit() {
        digits = true;
        i -= 1;
    }
    digits && i > 0 && b[i - 1] == b'v'
}

fn scan_user_folders(osc_root: &Path, want_id: Option<&str>) -> Option<AvatarConfig> {
    iter_avatar_files(osc_root)
        .into_iter()
        .filter_map(|(_, cfg)| {
            if let Some(id) = want_id {
                (cfg.id == id).then_some(cfg)
            } else {
                Some(cfg)
            }
        })
        .next()
}

fn iter_avatar_files(osc_root: &Path) -> Vec<(SystemTime, AvatarConfig)> {
    let mut out = Vec::new();
    let Ok(users) = fs::read_dir(osc_root) else {
        return out;
    };
    for user in users.flatten() {
        let avatars = user.path().join("Avatars");
        let Ok(files) = fs::read_dir(&avatars) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(cfg) = parse_avatar_config(&text) else {
                continue;
            };
            let mtime = file
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((mtime, cfg));
        }
    }
    out
}

fn osc_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(format!("{local}Low")).join("VRChat/VRChat/OSC");
        if p.is_dir() {
            out.push(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        let steam_roots = [
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            home.join("Library/Application Support/Steam"),
        ];
        for steam in steam_roots {
            let p = steam.join(
                "steamapps/compatdata/438100/pfx/drive_c/users/steamuser/AppData/LocalLow/VRChat/VRChat/OSC",
            );
            if p.is_dir() && !out.contains(&p) {
                out.push(p);
            }
            if let Some(extra) = osc_from_libraryfolders(&steam) {
                if extra.is_dir() && !out.contains(&extra) {
                    out.push(extra);
                }
            }
        }
    }
    out
}

fn osc_from_libraryfolders(steam: &Path) -> Option<PathBuf> {
    let vdf = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")).ok()?;
    for line in vdf.lines() {
        let t = line.trim();
        if !t.starts_with("\"path\"") {
            continue;
        }
        let Some(path) = t.split('"').nth(3) else {
            continue;
        };
        let p = PathBuf::from(path).join(
            "steamapps/compatdata/438100/pfx/drive_c/users/steamuser/AppData/LocalLow/VRChat/VRChat/OSC",
        );
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOM_JSON: &str = "\u{feff}{\"id\":\"avtr_test\",\"name\":\"Demo\",\"parameters\":[\
        {\"name\":\"FT/v2/JawOpen\",\"input\":{\"address\":\"/avatar/parameters/FT/v2/JawOpen\",\"type\":\"Float\"}},\
        {\"name\":\"Ignore\",\"output\":{\"address\":\"/avatar/parameters/out\",\"type\":\"Float\"}}\
    ]}";

    #[test]
    fn parse_bom_and_inputs() {
        let cfg = parse_avatar_config(BOM_JSON).unwrap();
        assert_eq!(cfg.id, "avtr_test");
        assert_eq!(cfg.name, "Demo");
        assert!(cfg.addresses.contains("/avatar/parameters/FT/v2/JawOpen"));
        assert_eq!(cfg.addresses.len(), 1);
    }

    #[test]
    fn remap_ft_prefix() {
        let mut addrs = HashSet::new();
        addrs.insert("/avatar/parameters/FT/v2/JawOpen".into());
        addrs.insert("/avatar/parameters/FT/v2/EyeLidLeft".into());
        assert_eq!(
            find_param_address(&addrs, "v2/JawOpen").as_deref(),
            Some("/avatar/parameters/FT/v2/JawOpen")
        );
        assert!(find_param_address(&addrs, "v2/TongueOut").is_none());
        assert!(
            !vrcft_address_matches("/avatar/parameters/FT/v2/JawOpen", "JawOpen"),
            "v1 JawOpen must not match v2/JawOpen"
        );
        assert!(vrcft_address_matches(
            "/avatar/parameters/v2/JawOpen",
            "v2/JawOpen"
        ));
    }

    #[test]
    fn live_osc_dir_if_present() {
        let Some(cfg) = latest_avatar_config() else {
            return;
        };
        if cfg.addresses.iter().any(|a| a.ends_with("/v2/JawOpen")) {
            let addr = find_param_address(&cfg.addresses, "v2/JawOpen").unwrap();
            assert!(addr.ends_with("/v2/JawOpen"));
        }
    }
}
