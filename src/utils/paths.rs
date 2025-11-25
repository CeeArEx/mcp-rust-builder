use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone)]
pub struct RustPaths {
    pub docs_path: Option<PathBuf>,
    pub cargo_registry: Option<PathBuf>,
    pub rustup_home: Option<PathBuf>,
}

impl RustPaths {
    /// Findet alle relevanten Rust-Installationspfade
    pub fn discover() -> Self {
        let rustup_home = Self::find_rustup_home();
        let docs_path = Self::find_rust_docs(&rustup_home);
        let cargo_registry = Self::find_cargo_registry();

        Self {
            docs_path,
            cargo_registry,
            rustup_home,
        }
    }

    /// Findet RUSTUP_HOME (normalerweise ~/.rustup)
    fn find_rustup_home() -> Option<PathBuf> {
        // Erst Umgebungsvariable prüfen
        if let Ok(rustup_home) = std::env::var("RUSTUP_HOME") {
            let path = PathBuf::from(rustup_home);
            if path.exists() {
                return Some(path);
            }
        }

        // Fallback: Standard-Pfad
        if let Some(home) = dirs::home_dir() {
            let rustup_home = home.join(".rustup");
            if rustup_home.exists() {
                return Some(rustup_home);
            }
        }

        None
    }

    /// Findet die installierten Rust Docs
    fn find_rust_docs(rustup_home: &Option<PathBuf>) -> Option<PathBuf> {
        let rustup_home = rustup_home.as_ref()?;

        // Suche im toolchains Verzeichnis
        let toolchains_dir = rustup_home.join("toolchains");
        if !toolchains_dir.exists() {
            return None;
        }

        // Finde stable toolchain
        let entries = fs::read_dir(toolchains_dir).ok()?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();

                // Suche nach stable-* Toolchains
                if name_str.starts_with("stable") {
                    let docs_path = path.join("share/doc/rust/html");
                    if docs_path.exists() {
                        return Some(docs_path);
                    }
                }
            }
        }

        None
    }

    /// Findet das Cargo Registry Verzeichnis
    fn find_cargo_registry() -> Option<PathBuf> {
        // Erst CARGO_HOME prüfen
        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            let path = PathBuf::from(cargo_home).join("registry");
            if path.exists() {
                return Some(path);
            }
        }

        // Fallback: Standard-Pfad
        if let Some(home) = dirs::home_dir() {
            let registry = home.join(".cargo/registry");
            if registry.exists() {
                return Some(registry);
            }
        }

        None
    }

    /// Prüft ob Rust Docs installiert sind
    pub fn has_docs(&self) -> bool {
        self.docs_path.is_some()
    }

    /// Gibt Installations-Status als String zurück
    pub fn status_report(&self) -> String {
        let mut report = String::new();

        report.push_str("Rust Installation Status:\n");
        report.push_str(&format!("  RUSTUP_HOME: {}\n",
                                 self.rustup_home.as_ref()
                                     .map(|p| p.display().to_string())
                                     .unwrap_or_else(|| "NOT FOUND".to_string())
        ));

        report.push_str(&format!("  Rust Docs: {}\n",
                                 self.docs_path.as_ref()
                                     .map(|p| p.display().to_string())
                                     .unwrap_or_else(|| "NOT INSTALLED (run: rustup component add rust-docs)".to_string())
        ));

        report.push_str(&format!("  Cargo Registry: {}\n",
                                 self.cargo_registry.as_ref()
                                     .map(|p| p.display().to_string())
                                     .unwrap_or_else(|| "NOT FOUND".to_string())
        ));

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_paths() {
        let paths = RustPaths::discover();
        println!("{}", paths.status_report());

        // Diese Tests sollten auf einem System mit Rust installiert funktionieren
        assert!(paths.rustup_home.is_some(), "RUSTUP_HOME sollte gefunden werden");
    }
}