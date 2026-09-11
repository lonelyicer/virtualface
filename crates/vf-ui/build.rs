use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    write_locale_files();
    write_third_party_crates();
}

fn write_locale_files() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let locales = manifest.join("locales");
    println!("cargo:rerun-if-changed=locales");

    let mut files: Vec<PathBuf> = fs::read_dir(&locales)
        .unwrap_or_else(|e| panic!("read {}: {e}", locales.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no locale JSON files in {}",
        locales.display()
    );

    let mut out = String::from("pub const LOCALE_FILES: &[(&str, &str)] = &[\n");
    for path in &files {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("locale filename");
        let name = path.file_name().and_then(|s| s.to_str()).unwrap();
        out.push_str(&format!(
            "    (\"{id}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/locales/{name}\"))),\n"
        ));
    }
    out.push_str("];\n");

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("locale_files.rs");
    fs::write(&dest, out).expect("write locale_files.rs");
}

fn write_third_party_crates() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let lock = workspace.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    println!(
        "cargo:rerun-if-changed={}",
        workspace.join("Cargo.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        workspace.join("LICENSE.md").display()
    );
    println!("cargo:rerun-if-changed=spdx");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let target =
        std::env::var("TARGET").unwrap_or_else(|_| String::from("x86_64-unknown-linux-gnu"));
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--filter-platform",
            &target,
            "--manifest-path",
        ])
        .arg(workspace.join("Cargo.toml"))
        .output()
        .expect("run cargo metadata");
    if !output.status.success() {
        panic!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata");

    let empty = Vec::new();
    let workspace_ids: HashSet<&str> = meta["workspace_members"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let texts_dir = out_dir.join("license_texts");
    fs::create_dir_all(&texts_dir).expect("create license_texts");

    let mut intern: HashMap<String, usize> = HashMap::new();
    let mut texts: Vec<String> = Vec::new();
    let mut crates: Vec<(String, String, String, Option<usize>)> = Vec::new();
    let spdx_catalog = load_spdx_catalog(&manifest);

    if let Some(packages) = meta["packages"].as_array() {
        for pkg in packages {
            let Some(id) = pkg["id"].as_str() else {
                continue;
            };
            if workspace_ids.contains(id) {
                continue;
            }
            let Some(name) = pkg["name"].as_str() else {
                continue;
            };
            let version = pkg["version"].as_str().unwrap_or("");
            let license = pkg["license"].as_str().unwrap_or("");
            let mut text = package_license_text(pkg);
            if text.is_empty() {
                text = fallback_from_spdx(license, &spdx_catalog);
            }
            let text_id = intern_text(&mut intern, &mut texts, text);
            crates.push((
                name.to_string(),
                version.to_string(),
                license.to_string(),
                text_id,
            ));
        }
    }
    crates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    crates.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

    for (i, text) in texts.iter().enumerate() {
        fs::write(texts_dir.join(format!("{i}.txt")), text).expect("write interned license");
    }

    let app_text = read_app_license(workspace);
    fs::write(out_dir.join("app_license.txt"), &app_text).expect("write app_license.txt");

    let mut out = String::from(
        "pub struct ThirdPartyCrate {\n    pub name: &'static str,\n    pub version: &'static str,\n    pub license: &'static str,\n    pub text: &'static str,\n}\n\n",
    );
    out.push_str("pub const APP_LICENSE_TEXT: &str = include_str!(concat!(env!(\"OUT_DIR\"), \"/app_license.txt\"));\n\n");
    out.push_str("pub const LICENSE_TEXTS: &[&str] = &[\n");
    for i in 0..texts.len() {
        out.push_str(&format!(
            "    include_str!(concat!(env!(\"OUT_DIR\"), \"/license_texts/{i}.txt\")),\n"
        ));
    }
    out.push_str("];\n\n");
    out.push_str("pub const THIRD_PARTY: &[ThirdPartyCrate] = &[\n");
    for (name, version, license, text_id) in &crates {
        let text_expr = match text_id {
            Some(i) => format!("LICENSE_TEXTS[{i}]"),
            None => "\"\"".into(),
        };
        out.push_str("    ThirdPartyCrate { name: ");
        out.push_str(&format!("{name:?}"));
        out.push_str(", version: ");
        out.push_str(&format!("{version:?}"));
        out.push_str(", license: ");
        out.push_str(&format!("{license:?}"));
        out.push_str(", text: ");
        out.push_str(&text_expr);
        out.push_str(" },\n");
    }
    out.push_str("];\n");

    fs::write(out_dir.join("third_party_crates.rs"), out).expect("write third_party_crates.rs");
}

fn load_spdx_catalog(manifest: &Path) -> HashMap<String, String> {
    let dir = manifest.join("spdx");
    let mut catalog = HashMap::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return catalog;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(text) = read_text(&path) {
            catalog.insert(id.to_string(), text);
        }
    }
    catalog
}

fn fallback_from_spdx(spdx: &str, catalog: &HashMap<String, String>) -> String {
    let mut parts: Vec<(&str, &str)> = Vec::new();
    for token in spdx.split(|c: char| c == '/' || c == '(' || c == ')' || c.is_whitespace()) {
        let id = token.trim();
        if id.is_empty() {
            continue;
        }
        if id.eq_ignore_ascii_case("or")
            || id.eq_ignore_ascii_case("and")
            || id.eq_ignore_ascii_case("with")
        {
            continue;
        }
        if let Some(text) = catalog.get(id) {
            if !parts.iter().any(|(seen, _)| *seen == id) {
                parts.push((id, text));
            }
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].1.to_string();
    }
    let mut out = String::new();
    for (i, (id, text)) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str("--- ");
        out.push_str(id);
        out.push_str(" ---\n\n");
        out.push_str(text);
    }
    out
}

fn intern_text(
    intern: &mut HashMap<String, usize>,
    texts: &mut Vec<String>,
    text: String,
) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    if let Some(&i) = intern.get(&text) {
        return Some(i);
    }
    let i = texts.len();
    intern.insert(text.clone(), i);
    texts.push(text);
    Some(i)
}

fn read_app_license(workspace: &Path) -> String {
    for name in [
        "LICENSE.md",
        "LICENSE",
        "LICENSE.txt",
        "LICENCE.md",
        "LICENCE",
    ] {
        if let Some(text) = read_text(&workspace.join(name)) {
            return text;
        }
    }
    String::new()
}

fn package_license_text(pkg: &serde_json::Value) -> String {
    let Some(manifest) = pkg["manifest_path"].as_str() else {
        return String::new();
    };
    let dir = match Path::new(manifest).parent() {
        Some(dir) => dir,
        None => return String::new(),
    };

    let mut explicit = Vec::new();
    if let Some(rel) = pkg["license_file"].as_str() {
        explicit.push(dir.join(rel));
    }
    if let Some(files) = pkg["license_files"].as_array() {
        for f in files {
            if let Some(rel) = f.as_str() {
                explicit.push(dir.join(rel));
            } else if let Some(rel) = f["path"].as_str() {
                explicit.push(dir.join(rel));
            }
        }
    }
    if let Some(text) = join_license_files(&explicit) {
        return text;
    }

    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if is_license_filename(name) {
                found.push(path);
            }
        }
    }
    found.sort();
    join_license_files(&found).unwrap_or_default()
}

fn join_license_files(paths: &[PathBuf]) -> Option<String> {
    let mut parts: Vec<(String, String)> = Vec::new();
    for path in paths {
        let Some(text) = read_text(path) else {
            continue;
        };
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("LICENSE")
            .to_string();
        parts.push((label, text));
    }
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        return Some(parts.pop().unwrap().1);
    }
    let mut out = String::new();
    for (i, (label, text)) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str("--- ");
        out.push_str(label);
        out.push_str(" ---\n\n");
        out.push_str(text);
    }
    Some(out)
}

fn is_license_filename(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if upper.contains("DEPENDENC") || upper.ends_with(".json") {
        return false;
    }
    upper == "LICENSE"
        || upper == "LICENCE"
        || upper == "COPYING"
        || upper == "UNLICENSE"
        || upper.starts_with("LICENSE.")
        || upper.starts_with("LICENSE-")
        || upper.starts_with("LICENCE.")
        || upper.starts_with("LICENCE-")
        || upper.starts_with("COPYING.")
        || upper.starts_with("COPYING-")
}

fn read_text(path: &Path) -> Option<String> {
    const MAX: usize = 512 * 1024;
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let mut text = String::from_utf8_lossy(&bytes).replace('\0', "");
    if text.trim().is_empty() {
        return None;
    }
    if text.len() > MAX {
        text.truncate(MAX);
        text.push_str("\n\n[...]\n");
    }
    if text.contains('\r') {
        text = text.replace("\r\n", "\n").replace('\r', "\n");
    }
    Some(text)
}
