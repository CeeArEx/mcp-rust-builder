// src/tools/patterns.rs
use anyhow::Result;

pub struct McpPatterns;

impl McpPatterns {
    pub fn new() -> Self {
        Self
    }

    pub fn get_template(&self, topic: &str) -> Result<String> {
        let template = match topic {
            "tool" => r#"
// PATTERN: Defining an MCP Tool
// Dependency: rmcp, serde, schemars
use rmcp::{tool, tool_router, model::*, ErrorData as McpError};
use rmcp::handler::server::tool::ToolRouter; // Important import!
use serde::Deserialize;
use schemars::JsonSchema;
use std::sync::Arc;

#[derive(Deserialize, JsonSchema)]
struct MyToolRequest {
    #[schemars(description = "Description of the argument")]
    input: String,
}

#[derive(Clone)]
struct MyServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MyServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "A brief description of what this tool does")]
    async fn my_tool_name(&self, params: Parameters<MyToolRequest>) -> Result<CallToolResult, McpError> {
        let input = params.0.input;
        // Logic here...
        Ok(CallToolResult::success(vec![Content::text(format!("Processed: {}", input))]))
    }
}
"#,
            "prompt" => r#"
// PATTERN: Defining an MCP Prompt
// Dependency: rmcp-macros (usually re-exported by rmcp)
use rmcp::{prompt, prompt_router, model::*, ErrorData as McpError};
use rmcp::handler::server::prompt::PromptRouter;

#[derive(Deserialize, JsonSchema)]
struct CodeReviewArgs {
    code: String,
}

#[derive(Clone)]
struct MyServer {
    // You can combine tool_router and prompt_router
    prompt_router: PromptRouter<Self>,
}

#[prompt_router]
impl MyServer {
    #[prompt(name = "code_review", description = "Review code for best practices")]
    async fn code_review_prompt(&self, params: Parameters<CodeReviewArgs>) -> Result<GetPromptResult, McpError> {
        let code = params.0.code;

        let message = PromptMessage {
            role: Role::User,
            content: PromptMessageContent::Text {
                text: format!("Please review this code:\n\n{}", code)
            }
        };

        Ok(GetPromptResult {
            description: Some("Code Review".to_string()),
            messages: vec![message],
        })
    }
}
"#,
            "resource" => r#"
// PATTERN: Defining a Resource (Manual Implementation)
// NOTE: rmcp currently lacks a #[resource] macro. You must implement `ServerHandler` manually.

use rmcp::{ServerHandler, model::*};

#[tool_handler] // Handles tools/prompts automatically, but we override methods for resources
impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources() // <--- Enable Resources!
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("My Server".to_string()),
            ..Default::default()
        }
    }

    // Manual Resource Handling
    async fn list_resources(&self, _params: ListResourcesRequest) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                Resource {
                    uri: "file:///logs/app.log".to_string(),
                    name: "Application Logs".to_string(),
                    description: Some("Recent log file entries".to_string()),
                    mime_type: Some("text/plain".to_string()),
                    ..Default::default()
                }
            ],
            next_cursor: None,
        })
    }

    async fn read_resource(&self, params: ReadResourceRequest) -> Result<ReadResourceResult, McpError> {
        if params.uri == "file:///logs/app.log" {
             Ok(ReadResourceResult {
                contents: vec![
                    ResourceContents::TextResourceContents {
                        uri: params.uri,
                        mime_type: Some("text/plain".to_string()),
                        text: "Log entry 1\nLog entry 2".to_string(),
                    }
                ]
             })
        } else {
            Err(McpError::new(ErrorCode::INVALID_REQUEST, "Resource not found", None))
        }
    }
}
"#,
            "server_setup" => r#"
// PATTERN: Main Server Setup (main.rs)
// Standard boilerplate for starting an MCP server over Stdio.

use rmcp::{ServerHandler, ServiceExt};
use tokio::io::{stdin, stdout};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Create your server instance
    let server = MyServer::new();

    // 2. Connect via Stdio (standard MCP transport)
    eprintln!("MCP Server starting...");
    server.serve((stdin(), stdout())).await?.waiting().await?;

    Ok(())
}
"#,
            _ => "Topic not found. Available: 'tool', 'prompt', 'resource', 'server_setup'",
        };
        Ok(template.to_string())
    }
}