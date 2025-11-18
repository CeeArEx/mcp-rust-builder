use walkdir::WalkDir;
use std::path::PathBuf;
use anyhow::Result;

pub struct ProjectManager;

impl ProjectManager {
    pub fn new() -> Self {
        Self
    }

    pub fn get_structure(&self, root_path: PathBuf) -> Result<String> {
        let mut structure = String::new();

        // Common folders to ignore to keep the context window small
        let ignore_dirs = vec!["target", ".git", "node_modules", ".idea", ".vscode"];

        for entry in WalkDir::new(&root_path).max_depth(5).sort_by_file_name() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            // Calculate depth for indentation
            let relative_path = match path.strip_prefix(&root_path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Skip if path is empty (root)
            if relative_path.as_os_str().is_empty() { continue; }

            let depth = relative_path.components().count();
            let file_name = entry.file_name().to_string_lossy();

            // Check for ignored directories
            if ignore_dirs.contains(&file_name.as_ref()) {
                if entry.file_type().is_dir() {
                    // Add the folder but indicate it's skipped
                    structure.push_str(&format!("{}|-- {}/ (skipped)\n", "    ".repeat(depth - 1), file_name));
                    continue;
                }
            }

            // Simple check: if any parent is in ignore_dirs, skip printing
            let path_str = relative_path.to_string_lossy();
            if ignore_dirs.iter().any(|d| path_str.contains(d)) {
                continue;
            }

            let prefix = "    ".repeat(depth - 1);
            if entry.file_type().is_dir() {
                structure.push_str(&format!("{}|-- {}/\n", prefix, file_name));
            } else {
                structure.push_str(&format!("{}|-- {}\n", prefix, file_name));
            }
        }

        if structure.is_empty() {
            Ok("Directory is empty or path is invalid.".to_string())
        } else {
            Ok(structure)
        }
    }
}