mod tools;
mod utils;

use std::path::PathBuf;
use rmcp::schemars;
use rmcp::{
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router, tool_handler,
    schemars::JsonSchema,
    ErrorData as McpError,
    ServiceExt,
    ServerHandler,
};
use serde::{Deserialize};
use tools::{CrateInfoProvider, RustDocsSearcher, CargoChecker, ErrorExplainer, ProjectManager};
use utils::RustPaths;
use std::sync::Arc;
use tokio::io::{stdin, stdout};

#[derive(Clone)]
pub struct RustBuilderServer {
    paths: Arc<RustPaths>,
    docs_searcher: Arc<Option<RustDocsSearcher>>,
    crate_provider: Arc<Option<CrateInfoProvider>>,
    checker: Arc<CargoChecker>,
    explainer: Arc<ErrorExplainer>,
    project_manager: Arc<ProjectManager>,
    tool_router: ToolRouter<Self>,
}

impl Default for RustBuilderServer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize, JsonSchema)]
struct SearchDocsRequest {
    #[schemars(description = "Suchbegriff (z.B. 'Vec', 'HashMap', 'async')")]
    query: String,
}

#[derive(Deserialize, JsonSchema)]
struct GetCrateInfoRequest {
    #[schemars(description = "Name der Crate (z.B. 'serde', 'tokio', 'rmcp')")]
    crate_name: String,
}

#[derive(Deserialize, JsonSchema)]
struct CheckCodeRequest {
    #[schemars(description = "Absoluter Pfad zum Rust-Projekt")]
    path: String,
}

#[derive(Deserialize, JsonSchema)]
struct ExplainRequest {
    #[schemars(description = "Fehlercode (z.B. 'E0308')")]
    error_code: String,
}

#[derive(Deserialize, JsonSchema)]
struct StructureRequest {
    #[schemars(description = "Absoluter Pfad zum Projekt-Root")]
    path: String,
}

#[tool_router]
impl RustBuilderServer {
    fn new() -> Self {
        // Entdecke Rust-Installation
        let paths = RustPaths::discover();
        eprintln!("{}", paths.status_report());

        // Initialisiere Tools
        let docs_searcher = paths.docs_path.clone().map(|p| RustDocsSearcher::new(p));
        let crate_provider = paths.cargo_registry.clone().map(|p| CrateInfoProvider::new(p));
        let checker = CargoChecker::new();


        Self {
            paths: Arc::new(paths),
            docs_searcher: Arc::new(docs_searcher),
            crate_provider: Arc::new(crate_provider),
            checker: Arc::new(CargoChecker::new()),
            explainer: Arc::new(ErrorExplainer::new()),
            project_manager: Arc::new(ProjectManager::new()),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Durchsucht die lokale Rust Standard Library Dokumentation")]
    async fn search_rust_docs(&self, params: Parameters<SearchDocsRequest>) -> Result<CallToolResult, McpError> {
        let SearchDocsRequest { query } = params.0;

        let searcher = self
            .docs_searcher
            .as_ref()
            .as_ref()
            .ok_or_else(|| McpError::new(
                ErrorCode::INTERNAL_ERROR,
                "Rust docs nicht installiert. Führe aus: rustup component add rust-docs",
                None
            ))?;

        let results = searcher.search(&query)
            .map_err(|e| McpError::new(ErrorCode::PARSE_ERROR, e.to_string(), None))?;

        let json_results = serde_json::to_value(&results)
            .map_err(|e| McpError::new(ErrorCode::PARSE_ERROR, e.to_string(), None))?;

        let response = serde_json::json!({
            "results": json_results,
            "count": results.len(),
            "query": query
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap()
        )]))
    }

    #[tool(description = "Holt Informationen über eine Rust Crate aus dem lokalen Registry")]
    async fn get_crate_info(&self, params: Parameters<GetCrateInfoRequest>) -> Result<CallToolResult, McpError> {
        let GetCrateInfoRequest { crate_name } = params.0;

        let provider = self
            .crate_provider
            .as_ref()
            .as_ref()
            .ok_or_else(|| McpError::new(
                ErrorCode::RESOURCE_NOT_FOUND,
                "Cargo registry nicht gefunden",
                None
            ))?;

        let info = provider.get_crate_info(&crate_name)
            .map_err(|e| McpError::new(ErrorCode::RESOURCE_NOT_FOUND, e.to_string(), None))?;

        let response = if let Some(info) = info {
            serde_json::json!({
                "found": true,
                "crate": info
            })
        } else {
            serde_json::json!({
                "found": false,
                "message": format!("Crate '{}' nicht im lokalen Cache gefunden", crate_name)
            })
        };

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap()
        )]))
    }

    #[tool(description = "Gibt den Status der Rust-Installation zurück")]
    async fn get_installation_status(&self) -> Result<CallToolResult, McpError> {
        let response = serde_json::json!({
            "rustup_home": self.paths.rustup_home.as_ref().map(|p| p.display().to_string()),
            "docs_installed": self.paths.has_docs(),
            "docs_path": self.paths.docs_path.as_ref().map(|p| p.display().to_string()),
            "cargo_registry": self.paths.cargo_registry.as_ref().map(|p| p.display().to_string()),
            "status_report": self.paths.status_report()
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap()
        )]))
    }

    #[tool(description = "Führt 'cargo check' aus und gibt Compiler-Fehler zurück")]
    async fn check_code(&self, params: Parameters<CheckCodeRequest>) -> Result<CallToolResult, McpError> {
        let path = PathBuf::from(params.0.path);

        // 1. Security & Sanity Check
        if !path.exists() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Der Pfad '{}' existiert nicht. Bitte prüfe die Struktur mit 'get_project_structure'.", path.display()),
                None
            ));
        }

        // 2. Run the check
        let result = self.checker.check(path)
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        // 3. Construct response with summary (Best of both worlds)
        let response = serde_json::json!({
        "status": if result.success { "success" } else { "error" },
        "issue_count": result.messages.len(), // Very helpful for the AI
        "issues": result.messages             // The raw data
    });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap()
        )]))
    }

    #[tool(description = "Erklärt einen Rust-Fehlercode (z.B. E0308). Nutze dies, wenn 'check_code' einen Fehlercode zurückgibt.")]
    async fn explain_error(&self, params: Parameters<ExplainRequest>) -> Result<CallToolResult, McpError> {
        // POLISH 1: Normalize input (remove whitespace, force uppercase)
        let raw_code = params.0.error_code.trim().to_uppercase();

        // POLISH 2: Basic validation before calling the internal tool
        if !raw_code.starts_with('E') {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Der Fehlercode '{}' ist ungültig. Er muss mit 'E' beginnen (z.B. E0308).", raw_code),
                None
            ));
        }

        let explanation = self.explainer.explain(&raw_code)
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        // POLISH 3: Return plain text (Markdown)
        // The output from rustc --explain is usually Markdown-compatible.
        Ok(CallToolResult::success(vec![Content::text(explanation)]))
    }

    #[tool(description = "Zeigt die Dateistruktur eines Projekts an (ignoriert target/ und .git/).")]
    async fn get_project_structure(&self, params: Parameters<StructureRequest>) -> Result<CallToolResult, McpError> {
        let path = PathBuf::from(params.0.path);

        // POLISH 1: Path Existence Check (Fail Fast)
        if !path.exists() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Der Pfad '{}' wurde nicht gefunden.", path.display()),
                None
            ));
        }

        // POLISH 2: Directory Check
        if !path.is_dir() {
            return Err(McpError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Der Pfad '{}' ist eine Datei, kein Ordner. Bitte gib das Wurzelverzeichnis des Projekts an.", path.display()),
                None
            ));
        }

        let structure = self.project_manager.get_structure(path)
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(structure)]))
    }

}

#[tool_handler]
impl ServerHandler for RustBuilderServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Simple server for creating MCP (Model Context Protocol) Servers in Rust".to_string(),
            ),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = RustBuilderServer::new();

    eprintln!("MCP Rust Builder Server gestartet!");

    server.serve((stdin(), stdout())).await?.waiting().await?;

    Ok(())
}