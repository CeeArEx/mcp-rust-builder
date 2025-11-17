mod tools;
mod utils;

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
use tools::{CrateInfoProvider, RustDocsSearcher};
use utils::RustPaths;
use std::sync::Arc;
use tokio::io::{stdin, stdout};

#[derive(Clone)]
pub struct RustBuilderServer {
    paths: Arc<RustPaths>,
    docs_searcher: Arc<Option<RustDocsSearcher>>,
    crate_provider: Arc<Option<CrateInfoProvider>>,
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
            tool_router: Self::tool_router(),  // ← WICHTIG!
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