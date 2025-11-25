// src/tools/dependencies.rs
use std::path::PathBuf;
use anyhow::{Context, Result};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use tokio::process::Command;
use rmcp::schemars;

#[derive(Deserialize, JsonSchema)]
pub struct AddDepRequest {
    #[schemars(description = "Absolute path to the project root (where Cargo.toml is located)")]
    pub project_path: String,
    #[schemars(description = "Name of the crate (e.g., 'axum')")]
    pub crate_name: String,
    #[schemars(description = "Optional features (e.g., ['macros', 'rt-multi-thread'])")]
    pub features: Option<Vec<String>>,
}

pub struct DependencyManager;

impl DependencyManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn add_dependency(
        &self,
        project_path: PathBuf,
        crate_name: &str,
        features: Option<Vec<String>>,
    ) -> Result<String> {
        // 1. Validation
        if !project_path.exists() {
            anyhow::bail!("Project path '{}' does not exist", project_path.display());
        }

        let cargo_toml = project_path.join("Cargo.toml");
        if !cargo_toml.exists() {
            anyhow::bail!("No Cargo.toml found at '{}'", project_path.display());
        }

        // 2. Construct Command
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&project_path) // Execute inside the project folder
            .arg("add")
            .arg(crate_name);

        if let Some(feats) = features {
            if !feats.is_empty() {
                cmd.arg("--features");
                cmd.arg(feats.join(","));
            }
        }

        // 3. Execute Async
        let output = cmd.output().await.context("Failed to execute 'cargo add'")?;

        if output.status.success() {
            Ok(format!(
                "Successfully added '{}'.\n{}",
                crate_name,
                String::from_utf8_lossy(&output.stderr) // cargo add prints to stderr usually
            ))
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Cargo failed: {}", error_msg);
        }
    }
}