# 🦀 Rust Builder MCP Server

**An autonomous Model Context Protocol (MCP) server designed to build, debug, and maintain Rust projects.**

This is a **Meta-MCP Server**: It is specifically architected to help LLMs (like Claude) build *other* MCP servers. It provides the AI with "eyes" to understand code structure, "hands" to safely edit files, and a "brain" containing verified MCP patterns.

## ⚡ Key Features

✅ **Offline Intelligence**:
- **Zero-Latency Search**: Built-in vector search for local Rust documentation (`std`, etc.) without API calls.
- **Local Registry**: Queries the local `~/.cargo` cache for crate versions and features.

✅ **Autonomous Development Workflow**:
- **"Golden Workflow"**: Enforces a strict *Check → Git Save → Edit → Verify* loop.
- **Cargo Integration**: Autonomously runs `check`, `test`, `fmt`, `clippy`, and `add` (dependencies).

✅ **Advanced Tooling**:
- **AST Analysis**: Parses Rust code to understand huge files (structs/traits) using minimal tokens.
- **Surgical Editing**: Uses `patch_file` logic to modify code precisely without overwriting whole files.
- **Meta-Scaffolding**: Generates boilerplate for new MCP Tools, Prompts, and Resources automatically.

---

## 🛠️ Tool Suite

The server exposes a comprehensive set of tools to the LLM:

| Category | Tool | Description |
| :--- | :--- | :--- |
| **👀 Eyes** | `search_rust_docs` | Search local documentation (TF-IDF). |
| | `analyze_code` | Parse file AST to see structs, fields, and signatures. |
| | `read_file` | Read files with line numbers for precise editing. |
| | `get_project_structure` | Visualize the file tree (ignoring target/git). |
| **✋ Hands** | `patch_file` | Edit code safely (handles whitespace normalization). |
| | `scaffold_new_tool` | Create new MCP tool boilerplate & `mod.rs` entries. |
| | `add_dependency` | Run `cargo add` with feature selection. |
| **🧠 Brain** | `get_mcp_template` | Retrieve verified `rmcp` code patterns. |
| | `explain_error` | Get `rustc --explain` output for error codes. |
| **🛡️ Safety** | `check_code` | Run `cargo check --message-format=json`. |
| | `run_tests` | Run `cargo test` (with optional filtering). |
| | `git_operations` | Commit, Diff, Status, or Undo changes. |

---

## 🚀 Installation

### 1. Prerequisites
Ensure Rust is installed and the documentation component is available:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rust-docs
```

### 2. Build the Server

```bash
git clone https://github.com/CeeArEx/mcp-rust-builder.git
cd mcp-rust-builder
cargo build --release
```

### 3. Configure Client

**For Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "rust-builder": {
      "command": "/ABSOLUTE/PATH/TO/mcp-rust-builder/target/release/mcp-rust-builder",
      "args": []
    }
  }
}
```

**For OpenCode / VS Code (Generic)**:

```json
{
  "mcpServers": {
    "rust-builder": {
      "command": "/ABSOLUTE/PATH/TO/mcp-rust-builder/target/release/mcp-rust-builder",
      "transport": "stdio"
    }
  }
}
```

---

## 💡 Usage Examples

### Scenario 1: Developing a new Tool
> **User:** "Create a new tool called `weather_fetcher` that takes a city name."

**Agent Action:**
1.  `scaffold_new_tool(name="weather_fetcher", ...)` -> Creates file & updates `mod.rs`.
2.  `add_dependency(crate="reqwest", ...)` -> Adds HTTP client.
3.  `patch_file(...)` -> Implements logic in the new file.
4.  `check_code()` -> Verifies compilation.

### Scenario 2: Debugging
> **User:** "Why is my build failing?"

**Agent Action:**
1.  `check_code()` -> Returns error JSON (e.g., Error `E0308`).
2.  `explain_error(code="E0308")` -> Reads compiler explanation.
3.  `read_file(...)` -> Reads the specific lines causing the error.
4.  `patch_file(...)` -> Fixes the types.

### Scenario 3: Research
> **User:** "How do I use `std::fs::File`?"

**Agent Action:**
1.  `search_rust_docs(query="std::fs::File")` -> Returns local HTML documentation summary.

---

## 📂 Project Structure

```text
mcp-rust-builder/
├── src/
│   ├── main.rs              # Entry point & Tool Router wiring
│   ├── tools/
│   │   ├── mod.rs           # Tool registration
│   │   ├── analyzer.rs      # AST Parsing (syn/quote)
│   │   ├── surgeon.rs       # Smart File Patching
│   │   ├── search_docs.rs   # TF-IDF Search Engine
│   │   ├── scaffolder.rs    # Code Generation
│   │   ├── git.rs           # Version Control
│   │   ├── testing.rs       # Test Runner
│   │   └── ... (other tools)
│   └── utils/
│       └── paths.rs         # System discovery
├── Cargo.toml
└── README.md
```

## 🔧 Troubleshooting

*   **"Rust docs not installed"**: Run `rustup component add rust-docs`.
*   **"Git remote error"**: Ensure you have a git repo initialized. The server expects a `.git` folder in the working directory to perform safety saves.
*   **"Connection Timeout"**: The initial documentation indexing happens in the background. If the server is slow to start, ensure `search_docs.rs` is using `tokio::spawn`.

## 📄 License

MIT