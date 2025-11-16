use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    name: String,
    vers: String,
    #[serde(default)]
    deps: Vec<Dependency>,
    #[serde(default)]
    features: std::collections::HashMap<String, Vec<String>>,
    yanked: bool,
}

#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    #[serde(default)]
    optional: bool,
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    package: Package,
    #[serde(default)]
    dependencies: std::collections::HashMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    documentation: Option<String>,
    #[serde(default)]
    license: Option<String>,
}

pub struct CrateInfoProvider {
    registry_path: PathBuf,
}

impl CrateInfoProvider {
    pub fn new(registry_path: PathBuf) -> Self {
        Self { registry_path }
    }

    /// Holt Crate-Informationen aus dem lokalen Registry
    pub fn get_crate_info(&self, crate_name: &str) -> anyhow::Result<Option<CrateInfo>> {
        // Versuche zuerst aus dem Index zu lesen
        if let Some(info) = self.get_from_index(crate_name)? {
            return Ok(Some(info));
        }

        // Fallback: Suche in src/ nach entpackten Crates
        self.get_from_src(crate_name)
    }

    /// Liest aus dem crates.io-v3 Index (neueres Format)
    fn get_from_index(&self, crate_name: &str) -> anyhow::Result<Option<CrateInfo>> {
        let index_path = self.registry_path.join("index");

        // Index-Pfad-Logik für crates.io
        let index_file = if crate_name.len() == 1 {
            index_path.join("1").join(crate_name)
        } else if crate_name.len() == 2 {
            index_path.join("2").join(crate_name)
        } else if crate_name.len() == 3 {
            index_path.join("3").join(&crate_name[..1]).join(crate_name)
        } else {
            index_path
                .join(&crate_name[..2])
                .join(&crate_name[2..4])
                .join(crate_name)
        };

        if !index_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(index_file)?;

        // Jede Zeile ist ein JSON-Eintrag für eine Version
        let mut latest_entry: Option<IndexEntry> = None;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let entry: IndexEntry = serde_json::from_str(line)?;

            // Überspringe "yanked" Versionen
            if entry.yanked {
                continue;
            }

            // Nimm die neueste Version
            if latest_entry.is_none() || self.is_newer_version(&entry.vers, &latest_entry.as_ref().unwrap().vers) {
                latest_entry = Some(entry);
            }
        }

        if let Some(entry) = latest_entry {
            let dependencies = entry
                .deps
                .iter()
                .filter(|d| !d.optional)
                .map(|d| d.name.clone())
                .collect();

            Ok(Some(CrateInfo {
                name: entry.name,
                version: entry.vers,
                description: None, // Index hat keine description
                repository: None,
                documentation: Some(format!("https://docs.rs/{}", crate_name)),
                license: None,
                dependencies,
            }))
        } else {
            Ok(None)
        }
    }

    /// Liest aus entpackten Crates in src/
    fn get_from_src(&self, crate_name: &str) -> anyhow::Result<Option<CrateInfo>> {
        let src_path = self.registry_path.join("src");

        if !src_path.exists() {
            return Ok(None);
        }

        // Durchsuche src/ nach passenden Crates
        for entry in fs::read_dir(src_path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Crate-Verzeichnisse haben Format: github.com-xxx
            for crate_dir in fs::read_dir(path)? {
                let crate_dir = crate_dir?;
                let crate_path = crate_dir.path();

                // Format: crate_name-version
                if let Some(dir_name) = crate_path.file_name().and_then(|s| s.to_str()) {
                    if dir_name.starts_with(crate_name) {
                        let cargo_toml = crate_path.join("Cargo.toml");

                        if cargo_toml.exists() {
                            return self.parse_cargo_toml(&cargo_toml);
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Parst eine Cargo.toml Datei
    fn parse_cargo_toml(&self, path: &PathBuf) -> anyhow::Result<Option<CrateInfo>> {
        let content = fs::read_to_string(path)?;
        let cargo_toml: CargoToml = toml::from_str(&content)?;

        let dependencies = cargo_toml
            .dependencies
            .keys()
            .cloned()
            .collect();

        Ok(Some(CrateInfo {
            name: cargo_toml.package.name,
            version: cargo_toml.package.version,
            description: cargo_toml.package.description,
            repository: cargo_toml.package.repository,
            documentation: cargo_toml.package.documentation,
            license: cargo_toml.package.license,
            dependencies,
        }))
    }

    /// Vergleicht Versionsnummern (simpel)
    fn is_newer_version(&self, v1: &str, v2: &str) -> bool {
        v1 > v2 // Lexikographischer Vergleich funktioniert für semver meist
    }

    /// Liste verfügbare Crates (limitiert)
    pub fn list_available_crates(&self, limit: usize) -> anyhow::Result<Vec<String>> {
        let index_path = self.registry_path.join("index");
        let mut crates = Vec::new();

        if !index_path.exists() {
            return Ok(crates);
        }

        // Durchsuche Index-Verzeichnisse
        for entry in fs::read_dir(&index_path)?.take(limit) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        crates.push(name.to_string());
                    }
                }
            }
        }

        Ok(crates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::paths::RustPaths;

    #[test]
    fn test_get_crate_info() {
        let paths = RustPaths::discover();

        if let Some(registry_path) = paths.cargo_registry {
            let provider = CrateInfoProvider::new(registry_path);

            // Test: Suche nach "serde"
            if let Ok(Some(info)) = provider.get_crate_info("serde") {
                println!("Found serde: v{}", info.version);
                println!("  Description: {:?}", info.description);
                println!("  Dependencies: {:?}", info.dependencies);
            } else {
                println!("serde nicht im lokalen Cache gefunden");
            }
        }
    }
}