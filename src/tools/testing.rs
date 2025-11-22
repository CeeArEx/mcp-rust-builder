// src/tools/testing.rs
use std::path::PathBuf;
use tokio::process::Command;
use anyhow::{Context, Result};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use::rmcp::schemars;

#[derive(Deserialize, JsonSchema)]
pub struct RunTestsRequest {
    #[schemars(description = "Absolute path to the project root")]
    pub path: String,
    #[schemars(description = "Optional filter: Name of the test or module (e.g., 'tests::my_test')")]
    pub filter: Option<String>,
}

pub struct TestRunner;

impl TestRunner {
    pub fn new() -> Self {
        Self
    }

    /// Runs cargo test in the specified directory.
    /// Returns the combined stdout/stderr output.
    pub async fn run(&self, project_path: PathBuf, filter: Option<String>) -> Result<String> {
        // 1. Validation
        if !project_path.exists() {
            anyhow::bail!("Path '{}' does not exist", project_path.display());
        }

        // Simple heuristic: It needs a Cargo.toml to be a testable project
        if !project_path.join("Cargo.toml").exists() {
            anyhow::bail!("No Cargo.toml found at '{}'. cannot run tests.", project_path.display());
        }

        // 2. Build Command
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&project_path)
            .arg("test")
            .arg("--color").arg("never"); // Optimization: Plain text output for AI

        // 3. Apply Filter (e.g. "tests::test_authentication")
        if let Some(test_name) = filter {
            if !test_name.trim().is_empty() {
                cmd.arg(&test_name);
            }
        }

        // 4. Execute
        // We capture output regardless of success/failure.
        // A failed test returns a non-zero exit code, but we WANT that output.
        let output = cmd.output().await.context("Failed to execute 'cargo test'")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // 5. Format Output
        let status_msg = if output.status.success() {
            "Tests passed!"
        } else {
            "Tests failed."
        };

        Ok(format!(
            "{}\n\n=== STDOUT ===\n{}\n=== STDERR ===\n{}",
            status_msg, stdout, stderr
        ))
    }
}