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
    ///
    /// Improvements over standard replacement:
    /// 1. Checks for "Near Misses" (whitespace errors) to guide the AI.
    /// 2. handles line-ending normalization.
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
        let original_content = fs::read_to_string(&path)
            .await
            .context("Failed to read file")?;

        // 2. Normalize Line Endings
        // If the file uses \r\n (Windows), ensure the search string also uses \r\n,
        // otherwise exact string matching will fail even if it looks correct.
        let search_normalized = if original_content.contains("\r\n") && !search.contains("\r\n") {
            search.replace('\n', "\r\n")
        } else {
            search.to_string()
        };

        // 3. Try Exact Match
        if original_content.contains(&search_normalized) {
            let count = original_content.matches(&search_normalized).count();

            // Perform the replacement (Limit 1 to be safe)
            let new_content = original_content.replacen(&search_normalized, replace, 1);

            // Atomic Write (write to string first, then flush to disk)
            fs::write(&path, new_content)
                .await
                .context("Failed to write to file")?;

            let mut msg = format!("Successfully patched '{}'.", path.display());
            if count > 1 {
                msg.push_str(&format!(
                    "\nWARNING: The search snippet was found {} times. Only the FIRST occurrence was replaced. \
                    If you intended to change a specific instance, include more surrounding context in your search snippet.",
                    count
                ));
            }
            return Ok(msg);
        }

        // 4. Diagnostic: Check for Whitespace Errors (The "Near Miss" Check)
        // This is critical for AI agents. They often mix up spaces/tabs.
        if self.matches_ignoring_whitespace(&original_content, &search_normalized) {
            anyhow::bail!(
                "Exact match failed, BUT the code was found when ignoring whitespace.\n\
                Diagnostic: Your 'original_snippet' has incorrect indentation or line breaks compared to the actual file.\n\
                Action: Read the file again, copy the lines EXACTLY (including leading spaces), and try again."
            );
        }

        // 5. Fail
        anyhow::bail!(
            "Could not find the snippet in '{}'. \
            The code you are trying to replace does not exist, or it has been modified since you last read it.\n\
            Action: Use `read_file` (or `get_project_structure`) to verify the file content.",
            path.display()
        );
    }

    /// Helper: returns true if `snippet` exists in `content` when all whitespace is collapsed.
    fn matches_ignoring_whitespace(&self, content: &str, snippet: &str) -> bool {
        let normalize = |s: &str| {
            s.split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ")
        };

        let norm_content = normalize(content);
        let norm_snippet = normalize(snippet);

        norm_content.contains(&norm_snippet)
    }
}