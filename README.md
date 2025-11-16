# MCP Rust Builder Server

Ein MCP (Model Context Protocol) Server, der LLMs dabei hilft, Rust-Code und MCP-Tools zu entwickeln - mit offline-first Ansatz.

## Features

✅ **Offline-First**: Nutzt lokale Rust-Dokumentation und Cargo Registry  
✅ **Schnell**: Keine API-Calls notwendig für grundlegende Queries  
✅ **3 Tools**:
- `search_rust_docs` - Durchsucht lokale Rust Standard Library Docs
- `get_crate_info` - Holt Crate-Informationen aus lokalem Cache
- `get_installation_status` - Prüft Rust-Installation

## Installation

### 1. Rust Docs installieren (falls noch nicht geschehen)

```bash
rustup component add rust-docs
```

### 2. Projekt bauen

```bash
cargo build --release
```

### 3. In Claude Desktop / OpenCode konfigurieren

**Für Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json` auf macOS):

```json
{
  "mcpServers": {
    "rust-builder": {
      "command": "/pfad/zu/mcp-rust-builder/target/release/mcp-rust-builder",
      "args": []
    }
  }
}
```

**Für OpenCode** (noch zu testen):

```json
{
  "mcpServers": {
    "rust-builder": {
      "command": "/pfad/zu/mcp-rust-builder/target/release/mcp-rust-builder",
      "transport": "stdio"
    }
  }
}
```

## Verwendung

### Beispiel 1: Rust Docs durchsuchen

```
User: "Wie funktioniert Vec in Rust?"
LLM ruft auf: search_rust_docs(query="Vec")
→ Bekommt Dokumentation zu Vec aus lokalen Docs
```

### Beispiel 2: Crate-Informationen

```
User: "Welche Dependencies hat tokio?"
LLM ruft auf: get_crate_info(crate_name="tokio")
→ Bekommt Version, Dependencies aus lokalem Cache
```

### Beispiel 3: Installation prüfen

```
User: "Sind die Rust Docs installiert?"
LLM ruft auf: get_installation_status()
→ Bekommt vollständigen Status-Report
```

## Projekt-Struktur

```
mcp-rust-builder/
├─ src/
│  ├─ main.rs              # MCP Server Setup
│  ├─ tools/
│  │  ├─ mod.rs
│  │  ├─ search_docs.rs    # Rust Docs Suche
│  │  └─ crate_info.rs     # Crate Informationen
│  └─ utils/
│     ├─ mod.rs
│     └─ paths.rs          # Rust Installation finden
├─ Cargo.toml
└─ README.md
```

## Geplante Features

- [ ] `update_documentation` - Aktualisiert lokale Docs
- [ ] `scaffold_mcp_project` - Generiert MCP-Projekt Boilerplate
- [ ] `search_mcp_examples` - Durchsucht GitHub nach MCP Examples
- [ ] `validate_mcp_tool` - Prüft MCP-Tool Code auf Korrektheit

## Testen

```bash
# Unit Tests
cargo test

# Server manuell starten (für Debugging)
cargo run

# Mit JSON-RPC testen
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | cargo run
```

## Troubleshooting

### "Rust docs nicht installiert"

```bash
rustup component add rust-docs
```

### "Cargo registry nicht gefunden"

Stelle sicher, dass du bereits Crates heruntergeladen hast:

```bash
cargo build  # In irgendeinem Projekt
```

### Server startet nicht

Prüfe die Installation:

```bash
cargo run
# Schaut in stderr output für Status-Report
```

## Lizenz

MIT