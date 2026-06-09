# vibe-tape

> Record and replay every AI agent session. Know exactly what the bot did to your codebase.

`vibe-tape` is a local-first CLI tool that wraps any command and records a complete, replayable session — file system changes, terminal output, git state, and API calls. Think of it as **asciinema for AI bots**.

## Philosophy

- **Sovereign-by-construction**: BYO API key, local-first, no telemetry, no required sign-up
- **No hardcoded secrets**. No phone-home. Default endpoint is `localhost:11434` (Ollama)
- **Your recordings stay on your machine**. Never upload `.tape` files to cloud services.
- **Open source** under Apache 2.0

## Install

```bash
cargo install --git https://github.com/sovereign-shovels/vibe-tape
```

Or build from source:

```bash
git clone https://github.com/sovereign-shovels/vibe-tape.git
cd vibe-tape
cargo build --release
```

## Usage

### Record a session

```bash
vibe-tape run claude-code
```

This wraps `claude-code` and writes a compressed `.tape` file (e.g. `session_20260609_120000.tape`) in the current directory.

### Replay a session

```bash
vibe-tape replay session_20260609_120000.tape
```

### Show diff against git HEAD

```bash
vibe-tape diff session_20260609_120000.tape
```

### Export as a git patch

```bash
vibe-tape patch session_20260609_120000.tape > changes.patch
```

## What gets recorded?

- **File system events**: creates, writes, deletes, renames (with SHA-256 before/after hashes)
- **Terminal output**: stdout/stderr of the wrapped process
- **Git state**: branch and commit before/after, plus a diff snapshot
- **API calls**: token usage for known providers (configurable list)

Sessions are stored as **compressed JSONL** (gzip) in `.tape` files.

## Configuration

Create `~/.config/vibe-tape/config.toml`:

```toml
[provider]
endpoint = "http://localhost:11434"
model = "llama3"
```

Or use environment variables:

```bash
export VIBE_TAPE_ENDPOINT="http://localhost:11434"
export VIBE_TAPE_MODEL="llama3"
```

## License

Apache 2.0. See [LICENSE](./LICENSE).

## Disclaimer

This is a community project and is **not affiliated with Anthropic**.
