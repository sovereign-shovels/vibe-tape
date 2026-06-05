# AGENTS.md — vibe-tape

> Constitution for agents working on this repo.

## What this repo is

An agent session recorder. Watches filesystem events during coding sessions and stores them as replayable tapes. Rust CLI.

## What this repo is NOT

- Not a version control system (use git for that)
- Not a screen recorder (no video)
- Not a telemetry system (all data stays local)

## Sovereignty rules

1. No cloud storage — tapes are local files only
2. No telemetry, no phone-home
3. Works offline entirely
4. Open tape format (JSONL) — no proprietary binary formats

## Scope boundaries

### In scope (v0.1)
- Filesystem event watching (create/modify/delete)
- JSONL tape format with timestamps
- CLI for record/list/show/export
- Marker events for manual annotations

### Out of scope (future versions)
- PTY capture (v0.2)
- API proxy recording (v0.3)
- Diff generation (v0.5)
- Replay mode (v0.8)
- Web UI (v1.0)

## Testing requirements

- Tape format tests must verify round-trip write/read
- Watcher tests must detect actual filesystem events
- Export tests must generate valid markdown
- CI must be green before merge

## No-nos

1. No executing code during replay (v0.8+)
2. No leaking absolute paths in shared tapes (relative paths only)
3. No storing file contents in tapes (only metadata)
