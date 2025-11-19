// src/tools/surgeon.rs
use std::path::PathBuf;
use anyhow::{Context, Result};
use tokio::fs;

pub struct FileSurgeon;

impl FileSurgeon {
    pub fn new() -> Self {
        Self
    }

    /// Replaces the *first* occurrence of `search` with `replace` in the file at `path`.
    /// Returns the diff/summary of the operation.
    pub async fn patch_file(
        &self,
        path: PathBuf,
        search: &str,
        replace: &str,
    ) -> Result<String> {
        if !path.exists() {
            anyhow::bail!("File '{}' not found", path.display());
        }

        // 1. Read file (Async)
        let content = fs::read_to_string(&path)
            .await
            .context("Failed to read file")?;

        // 2. Check if search string exists
        if !content.contains(search) {
            // Provide a helpful error - maybe whitespace is wrong?
            // For now, strict matching.
            anyhow::bail!(
                "Could not find the exact search snippet in '{}'. \
                Please ensure whitespace and formatting match exactly.",
                path.display()
            );
        }

        // 3. Safety Check: Ambiguity
        // If the snippet appears multiple times, it might be dangerous to auto-patch.
        // Ideally, we only patch if it's unique, or we default to the first one.
        // Decision: Default to first occurrence (replacen limit 1) but warn if multiple.
        let count = content.matches(search).count();

        // 4. Apply Patch
        // replacen(from, to, count)
        let new_content = content.replacen(search, replace, 1);

        // 5. Write back (Atomic write pattern is better, but direct overwrite is acceptable for dev tools)
        fs::write(&path, new_content)
            .await
            .context("Failed to write to file")?;

        let mut msg = format!("Successfully patched '{}'.", path.display());
        if count > 1 {
            msg.push_str(&format!("\nWARNING: The search snippet was found {} times. Only the first occurrence was replaced.", count));
        }
        Ok(msg)
    }
}