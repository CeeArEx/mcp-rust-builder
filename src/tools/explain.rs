use std::process::Command;
use anyhow::Result;
use regex::Regex;
use rmcp::schemars::JsonSchema;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ExplainRequest {
    #[schemars(description = "Error code (e.g., 'E0308')")]
    pub error_code: String,
}

pub struct ErrorExplainer;

impl ErrorExplainer {
    pub fn new() -> Self {
        Self
    }

    pub fn explain(&self, error_code: &str) -> Result<String> {
        // 1. Validate input to prevent command injection (must look like E0123)
        let re = Regex::new(r"^E\d{4}$").unwrap();
        if !re.is_match(error_code) {
            return Ok(format!("Invalid error code format: '{}'. Expected format like 'E0308'.", error_code));
        }

        // 2. Run rustc --explain
        let output = Command::new("rustc")
            .arg("--explain")
            .arg(error_code)
            .output()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            // Optional: Truncate if too long, but usually explanations are fine
            Ok(text)
        } else {
            Ok(format!("No explanation found for {}. It might not be a standard rustc error code.", error_code))
        }
    }
}