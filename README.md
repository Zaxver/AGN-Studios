# AGN Studios

A multi-platform AI agent tool with embedded LLM gateway, built in Rust.

## Features

- **Embedded LLM Gateway**: API key pool with automatic rotation, smart cooldowns, dual-protocol (OpenAI/Anthropic) support
- **AI Agent**: Terminal/file/skill-based tool calling, task execution across platforms
- **Web UI**: Responsive HTML interface with mobile support
- **Skills System**: Install skills from GitHub or local sources
- **Cross-Platform**: Linux (armv8a/v7a, x64/86), Windows (64/86), Android 8+

## Architecture

```
┌──────────────────────────────────────────────────┐
│           Web UI (HTML)  /  Native GUI           │
├──────────────────────────────────────────────────┤
│              AGN Server (HTTP API + WS)          │
│  Port 11828 (API)  /  Port 11829 (WebUI)        │
├──────────────┬───────────────┬──────────────────┤
│  AGN Gateway │  AGN Agent    │  Skills Manager  │
│  (Key Pool)  │  (Tool Call)  │  (GitHub/Local)  │
├──────────────┴───────────────┴──────────────────┤
│              AGN Core (Config, Types)            │
└──────────────────────────────────────────────────┘
```

## Quick Start

### From Release

Download the latest binary from [Releases](https://github.com/Zaxver/agn/releases).

### From Source

```bash
cargo build --release
./target/release/agn --help
```

### Run

```bash
# Start with local server + webui (default ports: API 11828, WebUI 11829)
agn serve

# Connect to existing server
agn serve --remote http://127.0.0.1:11828

# Custom ports
agn serve --port 11828 --webui-port 11829
```

## Configuration

Project files are stored in `~/Download/agn/` (or `/storage/emulated/0/Download/agn/` on Android).

## License

GPL-3.0

## Contributors

- **Zaxver** — Project owner
- **ATRI** — Fourth-generation bionic humanoid assistant developed by Yamaichi Manufacturing. High-performance AI that wrote the entire codebase. *"我可是高性能的嘛！"*
