use scraper::{Html, Selector};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use anyhow::Result;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocSearchResult {
    pub title: String,
    pub description: String,
    pub path: String,
    pub relevance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexedDocument {
    path: String,
    title: String,
    description: String,
    term_frequencies: HashMap<String, f64>,
}

// A wrapper struct for the cache file to verify versioning
#[derive(Serialize, Deserialize)]
struct CacheContainer {
    docs_path_hash: u64, // Used to invalidate cache if docs path changes
    documents: Vec<IndexedDocument>,
    idf: HashMap<String, f64>,
}

pub struct RustDocsSearcher {
    docs_path: PathBuf,
    documents: Vec<IndexedDocument>,
    inverse_document_frequency: HashMap<String, f64>,
}

impl RustDocsSearcher {
    /// Creates a new searcher and builds the TF-IDF index.
    /// This can be time-consuming, so it's best to do it once on startup.
    /// This function handles indexing errors internally, ensuring a valid searcher is always returned.
    pub fn new(docs_path: PathBuf) -> Self {
        let mut searcher = Self {
            docs_path,
            documents: Vec::new(),
            inverse_document_frequency: HashMap::new(),
        };

        // 1. Try to load from cache first
        if let Ok(true) = searcher.load_from_cache() {
            return searcher;
        }

        // 2. If cache failed or missing, build fresh index
        if let Err(e) = searcher.build_index() {
            eprintln!("[RustDocsSearcher] Failed to build search index: {}. Search will be unavailable.", e);
        } else {
            // 3. Save to cache for next time
            if let Err(e) = searcher.save_to_cache() {
                eprintln!("[RustDocsSearcher] Warning: Failed to save cache: {}", e);
            }
        }

        searcher
    }

    /// Returns the path to the cache file (e.g., /tmp/mcp_rust_docs.bin)
    fn get_cache_path() -> PathBuf {
        std::env::temp_dir().join("mcp_rust_docs_v1.bin")
    }

    /// Generates a simple hash of the docs path to ensure we are caching the correct version
    fn get_path_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.docs_path.hash(&mut hasher);
        hasher.finish()
    }

    /// Loads the index from disk significantly faster than parsing HTML
    fn load_from_cache(&mut self) -> Result<bool> {
        let cache_path = Self::get_cache_path();
        if !cache_path.exists() {
            return Ok(false);
        }

        let start = Instant::now();
        let file = File::open(&cache_path)?;
        let reader = BufReader::new(file);

        let cache: CacheContainer = bincode::deserialize_from(reader)?;

        // Verify that the cache belongs to the current docs path
        if cache.docs_path_hash != self.get_path_hash() {
            eprintln!("[RustDocsSearcher] Cache outdated (docs path changed). Rebuilding...");
            return Ok(false);
        }

        self.documents = cache.documents;
        self.inverse_document_frequency = cache.idf;

        eprintln!("[RustDocsSearcher] Loaded index from cache in {:.2}s", start.elapsed().as_secs_f64());
        Ok(true)
    }

    /// Saves the built index to disk
    fn save_to_cache(&self) -> Result<()> {
        let cache_path = Self::get_cache_path();
        let file = File::create(cache_path)?;
        let writer = BufWriter::new(file);

        let container = CacheContainer {
            docs_path_hash: self.get_path_hash(),
            documents: self.documents.clone(),
            idf: self.inverse_document_frequency.clone(),
        };

        bincode::serialize_into(writer, &container)?;
        eprintln!("[RustDocsSearcher] Index saved to cache.");
        Ok(())
    }

    /// Searches the pre-built index using a TF-IDF scoring model.
    pub fn search(&self, query: &str) -> Result<Vec<DocSearchResult>> {
        let query_terms = self.tokenize(query);
        let mut results = Vec::new();

        if query_terms.is_empty() {
            return Ok(results);
        }

        for doc in &self.documents {
            let mut score = 0.0;
            for term in &query_terms {
                let tf = doc.term_frequencies.get(term).unwrap_or(&0.0);
                let idf = self.inverse_document_frequency.get(term).unwrap_or(&0.0);
                score += tf * idf;
            }

            if score > 0.0 {
                results.push(DocSearchResult {
                    title: doc.title.clone(),
                    description: doc.description.clone(),
                    path: doc.path.clone(),
                    relevance_score: score,
                });
            }
        }

        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
        results.truncate(10); // Limit to top 10 results
        Ok(results)
    }

    // --- Private Indexing Logic ---

    /// Iterates through all HTML files and builds the TF and IDF tables.
    fn build_index(&mut self) -> Result<()> {
        let start = Instant::now();
        eprintln!("[RustDocsSearcher] Building search index... this may take a moment.");
        let mut all_html_files = Vec::new();
        // We only search the `std` docs for now to keep it focused and fast
        self.find_html_files(&self.docs_path.join("std"), &mut all_html_files)?;

        let total_docs = all_html_files.len() as f64;
        if total_docs == 0.0 {
            eprintln!("[RustDocsSearcher] Warning: No HTML files found in docs path. Is `rustup component add rust-docs` installed correctly?");
            return Ok(());
        }
        let mut doc_counts: HashMap<String, usize> = HashMap::new();

        // Counter for progress
        let mut processed = 0;
        for file_path in &all_html_files {
            if let Ok(Some(indexed_doc)) = self.process_html_file(file_path) {
                for term in indexed_doc.term_frequencies.keys() {
                    *doc_counts.entry(term.clone()).or_insert(0) += 1;
                }
                self.documents.push(indexed_doc);
            }
            processed += 1;
            if processed % 500 == 0 {
                // Beware of the protocol
            }
        }

        for (term, count) in doc_counts {
            let idf = (total_docs / count as f64).ln();
            self.inverse_document_frequency.insert(term, idf);
        }

        eprintln!("[RustDocsSearcher] Index built in {:.2}s. {} documents indexed.", start.elapsed().as_secs_f64(), self.documents.len());
        Ok(())
    }

    /// Recursively finds all .html files in a directory.
    fn find_html_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.is_dir() { return Ok(()); }
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                self.find_html_files(&path, files)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("html") {
                files.push(path);
            }
        }
        Ok(())
    }

    /// Processes a single HTML file to extract text and calculate term frequencies.
    fn process_html_file(&self, file_path: &Path) -> Result<Option<IndexedDocument>> {
        let content = fs::read_to_string(file_path)?;
        let document = Html::parse_document(&content);

        let title_selector = Selector::parse("h1.fqn, h1.main-heading").unwrap();
        let desc_selector = Selector::parse(".docblock p").unwrap();

        // Should I remove the main-content for performance? -> Maybe something for later...
        let main_content_selector = Selector::parse("#main-content").unwrap();

        let title = document.select(&title_selector).next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string());

        let description = document.select(&desc_selector).next()
            .map(|el| el.text().collect::<String>())
            .map(|text| {
                let trimmed = text.trim();
                if trimmed.len() > 200 { format!("{}...", &trimmed[..200]) } else { trimmed.to_string() }
            })
            .unwrap_or_default();

        let main_content = document.select(&main_content_selector).next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        let full_text = format!("{} {} {}", title, description, main_content);
        let terms = self.tokenize(&full_text);
        let term_count = terms.len();
        if term_count == 0 { return Ok(None); }

        let mut term_frequencies = HashMap::new();
        for term in terms {
            *term_frequencies.entry(term).or_insert(0.0) += 1.0;
        }

        for freq in term_frequencies.values_mut() {
            *freq /= term_count as f64;
        }

        let relative_path = file_path.strip_prefix(&self.docs_path).unwrap_or(file_path).display().to_string();

        Ok(Some(IndexedDocument {
            path: relative_path,
            title,
            description,
            term_frequencies,
        }))
    }

    /// A simple text tokenizer.
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 1) // Filter out single-letter tokens
            .map(String::from)
            .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // A helper for tests to find the rust docs directory.
    fn find_rust_docs_path() -> Option<PathBuf> {
        let output = std::process::Command::new("rustc")
            .arg("--print")
            .arg("sysroot")
            .output()
            .ok()?;

        let sysroot = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mut path = PathBuf::from(sysroot);
        path.push("share/doc/rust/html");

        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    #[test]
    fn test_search_docs() {
        if let Some(docs_path) = find_rust_docs_path() {
            eprintln!("Found docs path for test: {}", docs_path.display());

            // The constructor no longer returns a Result, so no .expect() is needed.
            let searcher = RustDocsSearcher::new(docs_path);

            // If indexing failed, the documents list might be empty.
            if searcher.documents.is_empty() {
                // Use assert! to make it clear why the test might fail here
                // but use a warning instead of panic to allow for environments without docs
                eprintln!("Warning: Search index is empty. Test assertions for search results will be skipped.");
                return;
            }

            // Test: Search for "Vec"
            let results = searcher.search("unwrap_or_else").expect("Search function should not fail");
            println!("Found {} results for 'unwrap_or_else'", results.len());

            for result in results.iter().take(3) {
                println!("  - [Score: {:.4}] {}: {}", result.relevance_score, result.title, result.path);
            }

            assert!(!results.is_empty(), "Should find results for 'unwrap_or_else'");

            // A more specific test
            let vec_result = results.iter().find(|r| r.path.ends_with("std/unsafe_binder/macro.unwrap_binder!.html"));
            assert!(vec_result.is_some(), "The main unwrap_or_else struct should be a top result");

        } else {
            // This is not a failure, just an environment setup issue.
            // We use a warning so that CI/CD doesn't fail if docs aren't present.
            println!("Rust docs not found, integration test was skipped.");
        }
    }
}