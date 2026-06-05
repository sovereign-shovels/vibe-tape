# vibe-tape

> Agent session recorder — see what your agent actually did.

```bash
vibe-tape record .              # Start recording file changes in current dir
vibe-tape show session-20260115-143022  # Show session summary
vibe-tape export session-20260115-143022 --output report.md
```

## Install

```bash
cargo install --git https://github.com/sovereign-shovels/vibe-tape
```

Or build from source:

```bash
git clone https://github.com/sovereign-shovels/vibe-tape
cd vibe-tape
cargo build --release
```

## Why this exists

AI agents can run for hours, touching dozens of files. When something breaks, you need to know what changed and when. vibe-tape records every file operation during an agent session so you can replay, audit, or debug.

## How it works

1. **Record** — Watch a directory for filesystem events
2. **Store** — Write events as JSONL to a `.tape` file
3. **Inspect** — Show summaries, timelines, and file lists
4. **Export** — Generate markdown reports

## Usage

### Record a session

```bash
vibe-tape record . --tag "feature-x"
# ▶ Recording session: session-20260115-143022
#   Watching: /home/user/project
#   Tape: /home/user/.local/share/vibe-tape/session-20260115-143022.tape
#   Press Ctrl+C to stop
```

### List sessions

```bash
vibe-tape list
# 📼 Recorded Tapes
# ==================================================
#   session-20260115-143022    12m 34s   23 files [feature-x]
#   session-20260115-102100     5m 12s   8 files
```

### Show session details

```bash
vibe-tape show ~/.local/share/vibe-tape/session-20260115-143022.tape
# 📼 session-20260115-143022
# ==================================================
# Version:   0.1.0
# Created:   2026-01-15 14:30:22 UTC
# CWD:       /home/user/project
# Tags:      feature-x
# Events:    156
# Duration:  754s
#
# Filesystem activity:
#   modify     89
#   create     45
#   delete     22
#
# Files touched (23):
#   src/main.rs
#   src/lib.rs
#   Cargo.toml
#   ...
#
# Timeline:
#   [14:30:25] modify src/main.rs
#   [14:30:28] create tests/test_main.rs
#   [14:31:02] 🚩 checkpoint before refactor
```

### Export to markdown

```bash
vibe-tape export session-20260115-143022 --output report.md
```

### Add markers during recording

```bash
# In another terminal while recording:
vibe-tape mark "checkpoint" --note "before refactor"
```

## Tape format

Tapes are JSONL files with a header line followed by events:

```json
{"version":"0.1.0","created_at":"2026-01-15T14:30:22Z","cwd":"/home/user/project","tags":["feature-x"]}
{"timestamp":"2026-01-15T14:30:25Z","fs":{"path":"src/main.rs","operation":"modify","size_before":null,"size_after":2048}}
{"timestamp":"2026-01-15T14:31:02Z","marker":{"label":"checkpoint","note":"before refactor"}}
```

## Storage

Tapes are stored in `~/.local/share/vibe-tape/` by default.

## Performance

- **Startup time**: < 10ms
- **Memory footprint**: ~5 MB
- **Event latency**: < 50ms (filesystem notify)
- **Binary size**: ~3 MB (release)

## Roadmap

- v0.2: PTY capture (record terminal commands and output)
- v0.3: API proxy mode (record HTTP requests/responses)
- v0.5: Diff viewer (show what changed in each file)
- v0.8: Replay mode (replay file changes to a clean directory)
- v1.0: Web UI for browsing tapes

## License

Apache 2.0
