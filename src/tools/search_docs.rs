use scraper::{Html, Selector};
use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DocSearchResult {
    pub title: String,
    pub description: String,
    pub path: String,
    pub relevance_score: f32,
}

pub struct RustDocsSearcher {
    docs_path: PathBuf,
}

impl RustDocsSearcher {
    pub fn new(docs_path: PathBuf) -> Self {
        Self { docs_path }
    }

    /// Sucht in den Rust Standard Library Docs nach einem Query
    pub fn search(&self, query: &str) -> anyhow::Result<Vec<DocSearchResult>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Durchsuche std-Dokumentation
        let std_path = self.docs_path.join("std");
        if std_path.exists() {
            self.search_directory(&std_path, &query_lower, &mut results)?;
        }

        // Sortiere nach Relevanz
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

        // Limitiere auf Top 10 Ergebnisse
        results.truncate(10);

        Ok(results)
    }

    /// Durchsucht rekursiv ein Verzeichnis nach HTML-Dateien
    fn search_directory(
        &self,
        dir: &PathBuf,
        query: &str,
        results: &mut Vec<DocSearchResult>,
    ) -> anyhow::Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Rekursiv in Unterverzeichnisse (mit Tiefenlimit)
                if results.len() < 50 {
                    let _ = self.search_directory(&path, query, results);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("html") {
                if let Ok(result) = self.search_html_file(&path, query) {
                    if result.relevance_score > 0.0 {
                        results.push(result);
                    }
                }
            }
        }

        Ok(())
    }

    /// Durchsucht eine einzelne HTML-Datei
    fn search_html_file(&self, file_path: &PathBuf, query: &str) -> anyhow::Result<DocSearchResult> {
        let content = fs::read_to_string(file_path)?;
        let document = Html::parse_document(&content);

        // Selektoren für verschiedene Teile der Dokumentation
        let title_selector = Selector::parse("h1.fqn, h1.main-heading").unwrap();
        let desc_selector = Selector::parse(".docblock p").unwrap();

        // Extrahiere Titel
        let title = document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_else(|| {
                file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string()
            });

        // Extrahiere Beschreibung
        let description = document
            .select(&desc_selector)
            .next()
            .map(|el| {
                let text = el.text().collect::<String>();
                if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text
                }
            })
            .unwrap_or_default();

        // Berechne Relevanz-Score
        let relevance_score = self.calculate_relevance(&title, &description, &content, query);

        // Relativer Pfad für bessere Lesbarkeit
        let relative_path = file_path
            .strip_prefix(&self.docs_path)
            .unwrap_or(file_path)
            .display()
            .to_string();

        Ok(DocSearchResult {
            title,
            description,
            path: relative_path,
            relevance_score,
        })
    }

    /// Berechnet Relevanz-Score basierend auf Query-Vorkommen
    fn calculate_relevance(&self, title: &str, description: &str, content: &str, query: &str) -> f32 {
        let title_lower = title.to_lowercase();
        let desc_lower = description.to_lowercase();
        let content_lower = content.to_lowercase();

        let mut score = 0.0;

        // Titel-Match ist am wichtigsten
        if title_lower.contains(query) {
            score += 10.0;
            // Exakter Match ist noch besser
            if title_lower == query {
                score += 20.0;
            }
        }

        // Beschreibungs-Match
        if desc_lower.contains(query) {
            score += 5.0;
        }

        // Content-Matches (limitiert, um Spam zu vermeiden)
        let content_matches = content_lower.matches(query).count().min(10);
        score += content_matches as f32 * 0.5;

        score
    }

    /// Sucht nach spezifischen Modulen/Traits/Structs
    pub fn search_item(&self, item_type: &str, name: &str) -> anyhow::Result<Option<DocSearchResult>> {
        let query = format!("{} {}", item_type, name);
        let results = self.search(&query)?;

        // Gib das beste Ergebnis zurück
        Ok(results.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::paths::RustPaths;

    #[test]
    fn test_search_docs() {
        let paths = RustPaths::discover();

        if let Some(docs_path) = paths.docs_path {
            let searcher = RustDocsSearcher::new(docs_path);

            // Test: Suche nach "Vec"
            let results = searcher.search("Vec").unwrap();
            println!("Found {} results for 'Vec'", results.len());

            for result in results.iter().take(3) {
                println!("  - {}: {}", result.title, result.path);
            }

            assert!(!results.is_empty(), "Sollte Ergebnisse für 'Vec' finden");
        } else {
            println!("Rust docs nicht installiert, Test übersprungen");
        }
    }
}