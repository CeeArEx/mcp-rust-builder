use std::path::PathBuf;
use tokio::process::Command;
use anyhow::{Context, Result};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use rmcp::schemars;

#[derive(Deserialize, JsonSchema)]
pub struct PolishRequest {
    #[schemars(description = "Project root path")]
    pub path: String,
    #[schemars(description = "Mode: 'fmt' (Format code) or 'clippy' (Check for lint errors). Note: Clippy does NOT auto-fix.")]
    pub mode: String,
}

pub struct CodePolisher;

impl CodePolisher {
    pub fn new() -> Self { Self }

    pub async fn run_fmt(&self, path: PathBuf) -> Result<String> {
        // cargo fmt is safe: it only affects style (indentation, spacing)
        let output = Command::new("cargo")
            .current_dir(&path)
            .arg("fmt")
            .output()
            .await?;

        if output.status.success() {
            Ok("Code formatted successfully.".to_string())
        } else {
            Ok(format!("❌ Format failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    pub async fn run_clippy(&self, path: PathBuf) -> Result<String> {
        // SAFETY: We do NOT use `--fix`. This is purely diagnostic.
        // We use `-D warnings` to treat warnings as errors so the AI takes them seriously.
        let output = Command::new("cargo")
            .current_dir(&path)
            .arg("clippy")
            .arg("--no-deps") // Only check this project, not dependencies (speed)
            .arg("--message-format=short")
            .arg("--")
            .arg("-D")
            .arg("warnings")
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok("Clippy is happy. No issues found.".to_string())
        } else {
            // Return the error message so the AI can read it and decide what to do.
            Ok(format!("Clippy Suggestions:\n{}", stderr))
        }
    }
}