use std::path::PathBuf;
use anyhow::{Context, Result};
use tokio::fs;
use tokio::io::AsyncWriteExt;

pub struct McpToolScaffolder;

impl McpToolScaffolder {
    pub fn new() -> Self {
        Self
    }

    /// Scaffolds a new MCP tool: creates the file and updates mod.rs
    pub async fn create_tool(
        &self,
        project_root: PathBuf,
        tool_name_snake: String, // e.g. "weather_checker"
        tool_struct_name: String, // e.g. "WeatherChecker"
        description: String,
    ) -> Result<String> {
        let tools_dir = project_root.join("src").join("tools");
        if !tools_dir.exists() {
            anyhow::bail!("Could not find src/tools directory at {}", tools_dir.display());
        }

        let file_path = tools_dir.join(format!("{}.rs", tool_name_snake));
        if file_path.exists() {
            anyhow::bail!("Tool file '{}' already exists.", file_path.display());
        }

        // 1. Generate the Tool Logic File
        // Double braces {{ }} are used to escape format! arguments
        let template = format!(r#"use anyhow::{{Context, Result}};

pub struct {struct_name};

impl {struct_name} {{
    pub fn new() -> Self {{
        Self
    }}

    pub async fn run(&self, input: String) -> Result<String> {{
        // TODO: Implement your logic here
        Ok(format!("Processed: {{}}", input))
    }}
}}
"#, struct_name = tool_struct_name);

        fs::write(&file_path, template).await.context("Failed to write tool file")?;

        // 2. Update tools/mod.rs
        let mod_path = tools_dir.join("mod.rs");
        let mut mod_content = fs::read_to_string(&mod_path).await.unwrap_or_default();

        let mod_line = format!("pub mod {};\n", tool_name_snake);
        let use_line = format!("pub use {}::{};\n", tool_name_snake, tool_struct_name);

        if !mod_content.contains(&mod_line) {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(&mod_path)
                .await?;

            file.write_all(format!("\n{}{}", mod_line, use_line).as_bytes()).await?;
        }

        // 3. Generate Instructions for main.rs integration
        let instructions = format!(
            r#"Successfully created `src/tools/{file_name}.rs` and updated `mod.rs`.

--- NEXT STEPS (Use `patch_file` to apply these) ---

1. **Add Import in `src/main.rs`**:
   Search for `use tools::{{...}};` and add `{struct_name}` to the list.

2. **Add Field to `RustBuilderServer`**:
   Search for `struct RustBuilderServer {{` and add:
   `{tool_name}: Arc<{struct_name}>,`

3. **Initialize in `new()`**:
   Search for `Self {{` inside `new()` and add:
   `{tool_name}: Arc::new({struct_name}::new()),`

4. **Define Request Struct**:
   Add this before `#[tool_router]`:
   ```rust
   #[derive(Deserialize, JsonSchema)]
   struct {struct_name}Request {{
       #[schemars(description = "Input description")]
       input: String,
   }}
   ```
5. **Implement Tool Function**:
    Add this inside impl RustBuilderServer:
    ```rust
    #[tool(description = "{desc}")]
    async fn {tool_name}(&self, params: Parameters<{struct_name}Request>) -> Result<CallToolResult, McpError> {{
        let result = self.{tool_name}.run(params.0.input)
            .await
            .map_err(|e| McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }}
    ```
    "#,
            file_name = tool_name_snake,
            struct_name = tool_struct_name,
            tool_name = tool_name_snake,
            desc = description
        );

        Ok(instructions)
    }

}
