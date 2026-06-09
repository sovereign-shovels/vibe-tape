# vibe-tape — Agent Guide

## Project Context

`vibe-tape` is a Rust CLI in the sovereign-shovels open-source portfolio. It records and replays AI agent sessions. v0.1 scope is intentionally small: wrap a command, capture FS + terminal + git state, output a `.tape` file.

## Build & Test

```bash
cargo test
cargo build --release
```

Integration tests live in `tests/integration.rs` and require the binary to be built. Unit tests are inline in source files.

## Architecture

- `src/main.rs` — CLI argument parsing and dispatch
- `src/recorder.rs` — Spawns child process, starts FS watcher, captures stdout/stderr, writes tape
- `src/watcher.rs` — `notify`-based FS watcher with SHA-256 hashing and a hash cache
- `src/tape.rs` — Tape format (JSONL + gzip), serde types, read/write helpers
- `src/replay.rs` — Reads a `.tape` file and prints a human-readable replay
- `src/diff.rs` — Shows diff against git HEAD (uses stored git diff if available)
- `src/patch.rs` — Exports a git patch (uses stored git diff if available)
- `src/config.rs` — Config loader (TOML + env vars). Not fully wired in v0.1.

## The `.tape` Format

Compressed JSONL (gzip). Each line is a JSON object with a `type` field. See `src/tape.rs` for the `TapeEvent` enum.

Key event types:
- `session_start` / `session_end`
- `fs_create` / `fs_write` / `fs_delete` / `fs_rename`
- `term_output` / `term_exit`
- `api_call` (placeholder in v0.1)
- `git_state` / `git_diff`

## 18 Universal No-Nos

1. Do not add telemetry, analytics, or phone-home behavior.
2. Do not hardcode secrets, API keys, or credentials.
3. Do not require user sign-up or cloud accounts.
4. Do not upload `.tape` files to any cloud service by default.
5. Do not introduce network dependencies for core functionality.
6. Do not break the wrapped process's stdout/stderr streaming.
7. Do not record sensitive env vars (e.g., API keys) in the tape.
8. Do not watch or hash files outside the session cwd without explicit opt-in.
9. Do not modify the source code of the wrapped process.
10. Do not introduce blocking I/O on the async runtime without `spawn_blocking`.
11. Do not panic on expected error conditions (missing git, non-writable path, etc.).
12. Do not produce `.tape` files that are not valid gzip JSONL.
13. Do not change the CLI interface without updating the README.
14. Do not add new dependencies without updating the lockfile and reviewing licenses.
15. Do not ignore CI failures on any supported platform.
16. Do not commit merge conflict markers or debug prints.
17. Do not disable existing tests to make new code pass.
18. Do not exceed the v0.1 scope creep without explicit PR discussion.

## Additional Rules

- Session recordings stay local. Never upload `.tape` files to cloud services.
- Default endpoint is `localhost:11434` (Ollama). No hardcoded cloud provider URLs.
- Keep the binary small: `strip = true` in release profile.
- Cross-platform: CI tests on Ubuntu, macOS, and Windows.

## License

Apache 2.0. All contributions must be compatible.
