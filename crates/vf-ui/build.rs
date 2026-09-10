use std::fs;
use std::path::PathBuf;

fn main() {
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
