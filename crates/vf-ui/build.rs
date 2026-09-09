//! Fedora's `libxkbcommon-x11` runtime package ships `libxkbcommon-x11.so.0`
//! but not the unversioned `libxkbcommon-x11.so` linker name (that lives in
//! `-devel`). gpui's X11 backend still passes `-lxkbcommon-x11`. Point rustc
//! at a search dir that has that name so the GUI crate links without extra
//! packages. Harmless on systems that already have the devel symlink.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(target_os = "linux")]
    linux_xkbcommon_x11();
}

#[cfg(target_os = "linux")]
fn linux_xkbcommon_x11() {
    let src = std::path::Path::new("/lib64/libxkbcommon-x11.so.0");
    let src = if src.exists() {
        src
    } else {
        std::path::Path::new("/usr/lib64/libxkbcommon-x11.so.0")
    };
    if !src.exists() {
        return;
    }
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let dest = std::path::Path::new(&out).join("libxkbcommon-x11.so");
    if dest.exists() {
        let _ = std::fs::remove_file(&dest);
    }
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(src, &dest);
    }
    println!("cargo:rustc-link-search=native={out}");
}
