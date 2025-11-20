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
use tools::{CrateInfoProvider, RustDocsSearcher, CargoChecker, ErrorExplainer, ProjectManager,
            DependencyManager, FileSurgeon, TestRunner, McpToolScaffolder, McpPatterns, GitController, CodePolisher};
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
    dep_manager: Arc<DependencyManager>,
    surgeon: Arc<FileSurgeon>,
    test_runner: Arc<TestRunner>,
    scaffolder: Arc<McpToolScaffolder>,
    patterns: Arc<McpPatterns>,
    git: Arc<GitController>,
    polisher: Arc<CodePolisher>,
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

#[derive(Deserialize, JsonSchema)]
struct AddDepRequest {
    #[schemars(description = "Absoluter Pfad zum Projekt-Root (wo Cargo.toml liegt)")]
    project_path: String,
    #[schemars(description = "Name der Crate (z.B. 'axum')")]
    crate_name: String,
    #[schemars(description = "Optionale Features (z.B. ['macros', 'rt-multi-thread'])")]
    features: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
struct PatchFileRequest {
    #[schemars(description = "Absoluter Pfad zur Datei")]
    path: String,
    #[schemars(description = "Der exakte Code-Abschnitt, der ersetzt werden soll")]
    original_snippet: String,
    #[schemars(description = "Der neue Code, der eingefügt wird")]
    modified_snippet: String,
}

#[derive(Deserialize, JsonSchema)]
struct RunTestsRequest {
    #[schemars(description = "Absoluter Pfad zum Projekt-Root")]
    path: String,
    #[schemars(description = "Optionaler Filter: Name des Tests oder Moduls (z.B. 'tests::my_test')")]
    filter: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ScaffoldToolRequest {
    #[schemars(description = "Absoluter Pfad zum Projekt-Root")]
    project_path: String,
    #[schemars(description = "Name des Tools in snake_case (z.B. 'run_tests')")]
    tool_name: String,
    #[schemars(description = "Name des Structs in PascalCase (z.B. 'TestRunner')")]
    struct_name: String,
    #[schemars(description = "Kurze Beschreibung, was das Tool tut")]
    description: String,
}

#[derive(Deserialize, JsonSchema)]
struct GetPatternRequest {
    #[schemars(description = "The topic to get a template for: 'tool', 'prompt', 'resource', or 'server_setup'")]
    topic: String,
}

#[derive(Deserialize, JsonSchema)]
struct GitRequest {
    #[schemars(description = "Project root path")]
    path: String,
    #[schemars(description = "Operation: 'status', 'diff', 'commit', 'undo'")]
    operation: String,
    #[schemars(description = "Commit message (required for 'commit')")]
    message: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct PolishRequest {
    #[schemars(description = "Project root path")]
    path: String,
    #[schemars(description = "Mode: 'fmt' (Format code) or 'clippy' (Check for lint errors). Note: Clippy does NOT auto-fix.")]
    mode: String,
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


        Self {
            paths: Arc::new(paths),
            docs_searcher: Arc::new(docs_searcher),
            crate_provider: Arc::new(crate_provider),
            checker: Arc::new(CargoChecker::new()),
            explainer: Arc::new(ErrorExplainer::new()),
            project_manager: Arc::new(ProjectManager::new()),
            dep_manager: Arc::new(DependencyManager::new()),
            surgeon: Arc::new(FileSurgeon::new()),
            test_runner: Arc::new(TestRunner::new()),
            scaffolder: Arc::new(McpToolScaffolder::new()),
            patterns: Arc::new(McpPatterns::new()),
            git: Arc::new(GitController::new()),
            polisher: Arc::new(CodePolisher::new()),
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

    #[tool(description = "Fügt einem Projekt eine Dependency via 'cargo add' hinzu.")]
    async fn add_dependency(&self, params: Parameters<AddDepRequest>) -> Result<CallToolResult, McpError> {
        let AddDepRequest { project_path, crate_name, features } = params.0;
        let path = PathBuf::from(project_path);

        // Logic delegated to tool module for separation of concerns
        let result = self.dep_manager.add_dependency(path, &crate_name, features)
            .await
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Patcht eine Datei durch Suchen und Ersetzen (File Surgeon). Sicherer als komplettes Überschreiben. Pfade müssen immer die Datei beinhalten, also z.B. \"/home/.../.../tools/test.rs\"")]
    async fn patch_file(&self, params: Parameters<PatchFileRequest>) -> Result<CallToolResult, McpError> {
        let PatchFileRequest { path, original_snippet, modified_snippet } = params.0;
        let file_path = PathBuf::from(path);

        let result = self.surgeon.patch_file(file_path, &original_snippet, &modified_snippet)
            .await
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Führt 'cargo test' aus. Nutze dies, um Code-Änderungen zu verifizieren.")]
    async fn run_tests(&self, params: Parameters<RunTestsRequest>) -> Result<CallToolResult, McpError> {
        let RunTestsRequest { path, filter } = params.0;
        let project_path = PathBuf::from(path);

        let output = self.test_runner.run(project_path, filter)
            .await
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        // We return the output as text so the LLM can read the assertions
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Erstellt das Grundgerüst für ein neues MCP Tool (Datei erstellen + mod.rs update). Gibt Anweisungen für main.rs zurück.")]
    async fn scaffold_new_tool(&self, params: Parameters<ScaffoldToolRequest>) -> Result<CallToolResult, McpError> {
        let ScaffoldToolRequest { project_path, tool_name, struct_name, description } = params.0;

        let result = self.scaffolder.create_tool(
            PathBuf::from(project_path),
            tool_name,
            struct_name,
            description
        ).await.map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Returns verified code templates for `rmcp` (Tools, Prompts, Resources). Use this to avoid syntax hallucinations.")]
    async fn get_mcp_template(&self, params: Parameters<GetPatternRequest>) -> Result<CallToolResult, McpError> {
        let topic = params.0.topic.to_lowercase();
        let template = self.patterns.get_template(&topic)
            .map_err(|e| McpError::new(ErrorCode::INVALID_PARAMS, e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(template)]))
    }

    #[tool(description = "Manages version control. Use 'commit' to save progress, and 'undo' to revert the last edit if it broke the build.")]
    async fn git_operations(&self, params: Parameters<GitRequest>) -> Result<CallToolResult, McpError> {
        let GitRequest { path, operation, message } = params.0;
        let path_buf = PathBuf::from(path);

        let result = match operation.as_str() {
            "status" => self.git.status(path_buf).await,
            "diff" => self.git.diff(path_buf).await,
            "undo" => self.git.undo(path_buf).await,
            "commit" => {
                let msg = message.unwrap_or_else(|| "WIP: Auto-commit".to_string());
                self.git.commit(path_buf, msg).await
            },
            _ => Err(anyhow::anyhow!("Unknown git operation. Use status, diff, commit, or undo.")),
        };

        let text = result.map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(description = "Checks code quality. 'fmt' cleans up whitespace (Safe). 'clippy' reports lints/errors but does NOT change code (Safe).")]
    async fn polish_code(&self, params: Parameters<PolishRequest>) -> Result<CallToolResult, McpError> {
        let path_buf = PathBuf::from(params.0.path);

        let result = match params.0.mode.as_str() {
            "fmt" => self.polisher.run_fmt(path_buf).await,
            "clippy" => self.polisher.run_clippy(path_buf).await,
            _ => Err(anyhow::anyhow!("Unknown polish mode. Use 'fmt' or 'clippy'")),
        };

        let text = result.map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
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