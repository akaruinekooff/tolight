use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fs};

type Registry = HashMap<String, String>;

fn registry_path() -> PathBuf {
    let home = env::var("HOME").expect("HOME not set");
    Path::new(&home).join(".tolight").join("projects.json")
}

pub fn load_registry() -> Registry {
    let path = registry_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_registry(registry: &Registry) {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(registry) {
        fs::write(&path, json).ok();
    }
}

pub fn register_project(name: &str, root: &Path) {
    let mut registry = load_registry();
    registry.insert(name.to_string(), root.to_string_lossy().to_string());
    save_registry(&registry);
}

/// Walk up from cwd looking for `.tolight/todos.json` (existing project) or `.git/` (potential project).
/// Returns (name, root_path).
pub fn detect_project(cwd: &Path) -> Option<(String, PathBuf)> {
    let mut current = Some(cwd);
    while let Some(dir) = current {
        if dir.join(".tolight").join("todos.json").exists() {
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            return Some((name, dir.to_path_buf()));
        }
        if dir.join(".git").is_dir() {
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            return Some((name, dir.to_path_buf()));
        }
        current = dir.parent();
    }
    None
}

/// Walk up from cwd to find the project root (git or existing tolight), fall back to cwd.
pub fn find_project_root(cwd: &Path) -> PathBuf {
    let mut current = Some(cwd);
    while let Some(dir) = current {
        if dir.join(".tolight").join("todos.json").exists() {
            return dir.to_path_buf();
        }
        if dir.join(".git").is_dir() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    cwd.to_path_buf()
}