use std::path::PathBuf;
use tokio::process::Command;
use anyhow::{Context, Result};

pub struct GitController;

impl GitController {
    pub fn new() -> Self { Self }

    /// Helper to run git commands
    async fn run_git(&self, path: &PathBuf, args: &[&str]) -> Result<String> {
        // 1. Check if it's a git repo
        if !path.join(".git").exists() {
            // Optional: Auto-init if missing? For now, just fail safely.
            return Ok("Not a git repository. Run 'git init' manually first.".to_string());
        }

        let output = Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .await
            .context("Failed to execute git command")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Ok(format!("Git Error: {}", err))
        }
    }

    pub async fn status(&self, path: PathBuf) -> Result<String> {
        self.run_git(&path, &["status", "--short"]).await
    }

    pub async fn diff(&self, path: PathBuf) -> Result<String> {
        self.run_git(&path, &["diff"]).await
    }

    pub async fn commit(&self, path: PathBuf, message: String) -> Result<String> {
        // Stage all changes
        self.run_git(&path, &["add", "."]).await?;
        // Commit
        self.run_git(&path, &["commit", "-m", &message]).await
    }

    pub async fn undo(&self, path: PathBuf) -> Result<String> {
        // Hard reset to HEAD (Dangerous but effective for "Undo")
        // Or just `checkout .` to discard local changes. `checkout .` is safer.
        self.run_git(&path, &["checkout", "."]).await
    }
}