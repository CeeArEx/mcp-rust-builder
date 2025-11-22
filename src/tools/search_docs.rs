// src/tools/search_docs.rs
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use anyhow::Result;
use std::time::Instant;
use std::sync::Arc;
use tokio::sync::RwLock;

// --- Public Data Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocSearchResult {
    pub title: String,
    pub description: String,
    pub path: String,
    pub relevance_score: f64,
}

// --- Internal Data Structures ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexedDocument {
    path: String,
    title: String,
    description: String,
    term_frequencies: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SearchIndex {
    docs_path_hash: u64,
    documents: Vec<IndexedDocument>,
    idf: HashMap<String, f64>,
}

/// Represents the current state of the search engine
enum SearchState {
    Initializing,
    Ready(SearchIndex),
    Error(String),
}

// --- Main Implementation ---

#[derive(Clone)]
pub struct RustDocsSearcher {
    docs_path: PathBuf,
    state: Arc<RwLock<SearchState>>,
}

impl RustDocsSearcher {
    /// Creates a new searcher.
    /// Returns immediately while the index builds in the background.
    pub fn new(docs_path: PathBuf) -> Self {
        let state = Arc::new(RwLock::new(SearchState::Initializing));
        let searcher = Self {
            docs_path: docs_path.clone(),
            state: state.clone(),
        };

        // Spawn the heavy lifting in the background
        tokio::spawn(async move {
            let start = Instant::now();
            eprintln!("[RustDocsSearcher] Background indexing started...");

            // Run the synchronous indexing logic
            // We use a separate block/function to isolate the heavy logic
            let result = Self::build_or_load_index(docs_path);

            let mut guard = state.write().await;
            match result {
                Ok(index) => {
                    eprintln!("[RustDocsSearcher] Index ready in {:.2}s. {} documents.", start.elapsed().as_secs_f64(), index.documents.len());
                    *guard = SearchState::Ready(index);
                }
                Err(e) => {
                    eprintln!("[RustDocsSearcher] Indexing failed: {}", e);
                    *guard = SearchState::Error(e.to_string());
                }
            }
        });

        searcher
    }

    /// Performs a search.
    /// If indexing is still running, returns a friendly "wait" message.
    pub async fn search(&self, query: &str) -> Result<Vec<DocSearchResult>> {
        let state = self.state.read().await;

        match &*state {
            SearchState::Initializing => {
                Ok(vec![DocSearchResult {
                    title: "Indexing in progress...".to_string(),
                    description: "The documentation index is currently being built. Please try again in a few seconds.".to_string(),
                    path: "".to_string(),
                    relevance_score: 1.0,
                }])
            },
            SearchState::Error(msg) => {
                Ok(vec![DocSearchResult {
                    title: "Search Unavailable".to_string(),
                    description: format!("Indexing failed: {}", msg),
                    path: "".to_string(),
                    relevance_score: 0.0,
                }])
            },
            SearchState::Ready(index) => {
                Self::perform_search(index, query)
            }
        }
    }

    // --- Logic Wrappers (Static/Pure functions) ---

    /// Logic to load from cache or build fresh.
    /// This is synchronous code, but running inside the tokio::spawn wrapper.
    fn build_or_load_index(docs_path: PathBuf) -> Result<SearchIndex> {
        let path_hash = Self::get_path_hash(&docs_path);

        // 1. Try Cache
        if let Ok(mut index) = Self::load_from_cache() {
            if index.docs_path_hash == path_hash {
                return Ok(index);
            }
            eprintln!("[RustDocsSearcher] Cache outdated. Rebuilding...");
        }

        // 2. Build Fresh
        let index = Self::build_index_fresh(&docs_path)?;

        // 3. Save Cache
        if let Err(e) = Self::save_to_cache(&index) {
            eprintln!("[RustDocsSearcher] Warning: Failed to save cache: {}", e);
        }

        Ok(index)
    }

    fn perform_search(index: &SearchIndex, query: &str) -> Result<Vec<DocSearchResult>> {
        let query_terms = Self::tokenize(query);
        let mut results = Vec::new();

        if query_terms.is_empty() {
            return Ok(results);
        }

        for doc in &index.documents {
            let mut score = 0.0;
            for term in &query_terms {
                let tf = doc.term_frequencies.get(term).unwrap_or(&0.0);
                let idf = index.idf.get(term).unwrap_or(&0.0);
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
        results.truncate(15);
        Ok(results)
    }

    // --- Private Helpers (FileSystem & Parsing) ---

    fn get_cache_path() -> PathBuf {
        std::env::temp_dir().join("mcp_rust_docs_v2.bin")
    }

    fn get_path_hash(path: &Path) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    }

    fn load_from_cache() -> Result<SearchIndex> {
        let cache_path = Self::get_cache_path();
        if !cache_path.exists() {
            anyhow::bail!("Cache missing");
        }
        let file = File::open(&cache_path)?;
        let reader = BufReader::new(file);
        let index: SearchIndex = bincode::deserialize_from(reader)?;
        Ok(index)
    }

    fn save_to_cache(index: &SearchIndex) -> Result<()> {
        let cache_path = Self::get_cache_path();
        let file = File::create(cache_path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, index)?;
        Ok(())
    }

    fn build_index_fresh(docs_path: &Path) -> Result<SearchIndex> {
        let mut all_html_files = Vec::new();
        // Only index `std` to keep it manageable, or remove .join("std") for full docs
        Self::find_html_files(&docs_path.join("std"), &mut all_html_files)?;

        if all_html_files.is_empty() {
            // Fallback: try root if std doesn't exist
            Self::find_html_files(docs_path, &mut all_html_files)?;
        }

        let total_docs = all_html_files.len() as f64;
        if total_docs == 0.0 {
            anyhow::bail!("No HTML files found in {}. Check 'rustup component add rust-docs'", docs_path.display());
        }

        let mut documents = Vec::new();
        let mut doc_counts: HashMap<String, usize> = HashMap::new();

        let mut processed = 0;
        for file_path in &all_html_files {
            if let Ok(Some(indexed_doc)) = Self::process_html_file(file_path, docs_path) {
                for term in indexed_doc.term_frequencies.keys() {
                    *doc_counts.entry(term.clone()).or_insert(0) += 1;
                }
                documents.push(indexed_doc);
            }
            processed += 1;
            // Optional: Log progress occasionally
            if processed % 1000 == 0 {
                // eprintln!("Indexed {} files...", processed);
            }
        }

        let mut idf = HashMap::new();
        for (term, count) in doc_counts {
            let val = (total_docs / count as f64).ln();
            idf.insert(term, val);
        }

        Ok(SearchIndex {
            docs_path_hash: Self::get_path_hash(docs_path),
            documents,
            idf,
        })
    }

    fn find_html_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.is_dir() { return Ok(()); }
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                Self::find_html_files(&path, files)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("html") {
                files.push(path);
            }
        }
        Ok(())
    }

    fn process_html_file(file_path: &Path, root_path: &Path) -> Result<Option<IndexedDocument>> {
        // Read file
        let content = fs::read_to_string(file_path)?;
        let document = Html::parse_document(&content);

        // Selectors
        let title_selector = Selector::parse("h1.fqn, h1.main-heading").map_err(|_| anyhow::anyhow!("Bad selector"))?;
        let desc_selector = Selector::parse(".docblock p").map_err(|_| anyhow::anyhow!("Bad selector"))?;

        // Extract Title
        let title = document.select(&title_selector).next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string());

        // Extract Description
        let description = document.select(&desc_selector).next()
            .map(|el| el.text().collect::<String>())
            .map(|text| {
                let trimmed = text.trim();
                if trimmed.len() > 200 { format!("{}...", &trimmed[..200]) } else { trimmed.to_string() }
            })
            .unwrap_or_default();

        // Tokenize
        // Combine title and description for indexing.
        // Note: Ignoring main content body for speed/memory optimization in this embedded server.
        let full_text = format!("{} {}", title, description);
        let terms = Self::tokenize(&full_text);

        let term_count = terms.len();
        if term_count == 0 { return Ok(None); }

        let mut term_frequencies = HashMap::new();
        for term in terms {
            *term_frequencies.entry(term).or_insert(0.0) += 1.0;
        }
        for freq in term_frequencies.values_mut() {
            *freq /= term_count as f64;
        }

        let relative_path = file_path.strip_prefix(root_path).unwrap_or(file_path).display().to_string();

        Ok(Some(IndexedDocument {
            path: relative_path,
            title,
            description,
            term_frequencies,
        }))
    }

    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2) // Skip very short words
            .map(String::from)
            .collect()
    }
}