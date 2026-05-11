use std::collections::HashMap;
use std::{env, fs};
use std::path::{Path, PathBuf};
use crate::Todo;

pub fn global_todo_path() -> PathBuf {
    let home = env::var("HOME").expect("HOME not set");
    Path::new(&home).join(".tolight").join("todos.json")
}

pub fn save_to_file(todos: &[Todo], path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(todos)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_todos(path: &Path) -> Vec<Todo> {
    let data = fs::read_to_string(path).unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&data).unwrap()
}

pub fn update_config_line(path: &str, key: &str, new_value: &str) -> String {
    let content = fs::read_to_string(path).unwrap_or_default();

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut found = false;

    for line in &mut lines {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((k, _)) = trimmed.split_once('=')
            && k.trim() == key {
                *line = format!("{}={}", key, new_value);
                found = true;
                break;
        }
    }

    if !found {
        lines.push(format!("{}={}", key, new_value));
    }

    let new_content = lines.join("\n");
    fs::write(path, &new_content).expect("failed to write config");
    new_content
}

pub fn load_config(path: &str) -> HashMap<String, String> {
    let mut config = HashMap::new();
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        if let Some(parent) = path_obj.parent()
            && let Err(e) = fs::create_dir_all(parent) {
                eprintln!("failed to create dirs: {}", e);
        }

        let default = "\
# config file
show_hints=true
";

        if let Err(e) = fs::write(path, default) {
            eprintln!("failed to create config: {}", e);
        }
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return config,
    };

    config = parse_config(&content);

    config
}

pub fn parse_config(content: &str) -> HashMap<String, String> {
    let mut config = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            config.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    config
}