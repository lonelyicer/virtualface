use crate::error::{CoreError, Result};
use crate::log::LogBus;
use crate::registry::{NodeRegistry, NodeType};
use libloading::Library;
use std::path::{Path, PathBuf};
use vf_abi::{VF_ABI_VERSION, VF_PLUGIN_ENTRY, VfPluginDescriptor, VfPluginEntry};

pub struct LoadedPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub homepage: String,
    pub repository: String,
    pub issues: String,
    pub keywords: String,
    pub path: PathBuf,
    _lib: Library,
}

pub struct PluginHost {
    plugins: Vec<LoadedPlugin>,
    pub log: LogBus,
}

impl PluginHost {
    pub fn new(log: LogBus) -> Self {
        Self {
            plugins: Vec::new(),
            log,
        }
    }

    pub fn plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    pub fn scan_and_load(&mut self, dirs: &[PathBuf], registry: &mut NodeRegistry) -> Vec<String> {
        let mut errors = Vec::new();
        let mut files = Vec::new();
        for dir in dirs {
            if !dir.is_dir() {
                continue;
            }
            match std::fs::read_dir(dir) {
                Ok(rd) => {
                    for ent in rd.flatten() {
                        let p = ent.path();
                        if is_plugin_lib(&p) {
                            files.push(p);
                        }
                    }
                }
                Err(e) => errors.push(format!("cannot read {}: {e}", dir.display())),
            }
        }
        files.sort();
        files.dedup();
        let mut seen_names = std::collections::HashSet::new();
        for path in files {
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            // debug/libfoo.so and deps/libfoo.so are often hardlinks of the same cdylib.
            if !seen_names.insert(name.to_string()) {
                continue;
            }
            match self.load_one(&path, registry) {
                Ok(id) => {
                    self.log.log(
                        2,
                        None,
                        format!("loaded plugin {id} from {}", path.display()),
                    );
                }
                Err(e) => {
                    self.log.log(0, None, format!("{}: {e}", path.display()));
                    errors.push(format!("{}: {e}", path.display()));
                }
            }
        }
        errors
    }

    fn load_one(&mut self, path: &Path, registry: &mut NodeRegistry) -> Result<String> {
        let lib = unsafe { Library::new(path) }
            .map_err(|e| CoreError::Load(format!("{}: {e}", path.display())))?;
        let entry: libloading::Symbol<VfPluginEntry> = unsafe {
            lib.get(VF_PLUGIN_ENTRY)
                .map_err(|e| CoreError::Load(format!("missing vf_plugin_entry: {e}")))?
        };
        let desc_ptr = unsafe { entry(VF_ABI_VERSION) };
        if desc_ptr.is_null() {
            return Err(CoreError::Load(
                "vf_plugin_entry returned null (ABI mismatch?)".into(),
            ));
        }
        let desc: &VfPluginDescriptor = unsafe { &*desc_ptr };
        if desc.abi_version != VF_ABI_VERSION {
            return Err(CoreError::AbiMismatch {
                plugin: desc.abi_version,
                host: VF_ABI_VERSION,
            });
        }
        let id = cstr(desc.id);
        let name = cstr(desc.name);
        let version = cstr(desc.version);
        let nodes = if desc.nodes.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(desc.nodes, desc.node_count as usize) }
        };
        for n in nodes {
            let ty = unsafe { NodeType::from_c(&id, &name, n)? };
            registry.register(ty);
        }
        self.plugins.push(LoadedPlugin {
            id: id.clone(),
            name,
            version,
            description: cstr(desc.description),
            author: cstr(desc.author),
            license: cstr(desc.license),
            homepage: cstr(desc.homepage),
            repository: cstr(desc.repository),
            issues: cstr(desc.issues),
            keywords: cstr(desc.keywords),
            path: path.to_path_buf(),
            _lib: lib,
        });
        Ok(id)
    }
}

fn cstr(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned()
    }
}

fn is_plugin_lib(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let is_lib = matches!(ext, "so" | "dll" | "dylib");
    if !is_lib {
        return false;
    }
    // Skip the host binary and rustc extra artifacts.
    if name.contains("vf_plugin") || name.contains("vf-plugin") {
        return true;
    }
    // Also accept any cdylib sitting in an explicit plugins/ folder.
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        == Some("plugins")
}

/// Default directories searched when none are supplied.
pub fn default_plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(PathBuf::from("plugins"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("plugins"));
            dirs.push(parent.to_path_buf());
        }
    }
    // Workspace target dir during development.
    dirs.push(PathBuf::from("target/debug"));
    dirs.push(PathBuf::from("target/release"));
    dirs
}
