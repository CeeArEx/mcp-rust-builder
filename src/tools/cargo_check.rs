use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::PathBuf;
use anyhow::Result;
use rmcp::schemars;
use rmcp::schemars::JsonSchema;

#[derive(Deserialize, JsonSchema)]
pub struct CheckCodeRequest {
    #[schemars(description = "Absolute path to the Rust project")]
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompilerMessage {
    pub level: String, // "error", "warning"
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub code: Option<String>, // e.g., "E0308"
}

pub struct CargoChecker;

impl CargoChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self, project_path: PathBuf) -> Result<CheckResult> {
        // 1. Run cargo check with JSON output
        let output = Command::new("cargo")
            .arg("check")
            .arg("--message-format=json")
            .current_dir(&project_path)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute cargo: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut messages = Vec::new();
        let success = output.status.success();

        // 2. Parse the JSON stream
        for line in stdout.lines() {
            // Skip non-JSON lines
            if !line.starts_with('{') { continue; }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if json["reason"] == "compiler-message" {
                    if let Some(msg) = json.get("message") {
                        let level = msg["level"].as_str().unwrap_or("unknown").to_string();

                        if level == "error" || level == "warning" {
                            let (file, line) = if let Some(spans) = msg["spans"].as_array() {
                                if let Some(first) = spans.first() {
                                    (
                                        first["file_name"].as_str().map(|s| s.to_string()),
                                        first["line_start"].as_u64().map(|n| n as usize),
                                    )
                                } else { (None, None) }
                            } else { (None, None) };

                            messages.push(CompilerMessage {
                                level,
                                message: msg["message"].as_str().unwrap_or("").to_string(),
                                code: msg["code"]["code"].as_str().map(|s| s.to_string()),
                                file,
                                line,
                            });
                        }
                    }
                }
            }
        }

        // Fallback for non-JSON errors (like missing Cargo.toml)
        if !success && messages.is_empty() {
            messages.push(CompilerMessage {
                level: "error".to_string(),
                message: stderr.to_string(),
                file: Some("Cargo.toml".to_string()),
                line: None,
                code: None,
            });
        }

        Ok(CheckResult {
            success,
            messages,
        })
    }
}

#[derive(Serialize)]
pub struct CheckResult {
    pub success: bool,
    pub messages: Vec<CompilerMessage>,
}