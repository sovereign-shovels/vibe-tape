use crate::tape::{read_tape, TapeEvent};
use std::path::Path;

pub fn replay(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let events = read_tape(path)?;
    if events.is_empty() {
        println!("(empty tape)");
        return Ok(());
    }

    println!("═══ vibe-tape replay ═══");
    for event in events {
        match event {
            TapeEvent::SessionStart { ts, cmd, cwd } => {
                println!("[{}] START  {}  (cwd: {})", ts, cmd, cwd);
            }
            TapeEvent::SessionEnd { ts, duration_sec } => {
                println!("[{}] END    duration: {}s", ts, duration_sec);
            }
            TapeEvent::FsCreate { ts, path, hash_after } => {
                println!("[{}] CREATE {} (hash: {})", ts, path, &hash_after[..hash_after.len().min(8)]);
            }
            TapeEvent::FsWrite { ts, path, hash_before, hash_after, size_delta } => {
                let hb = hash_before.as_ref().map(|h| &h[..h.len().min(8)]).unwrap_or("?");
                let ha = &hash_after[..hash_after.len().min(8)];
                println!("[{}] WRITE  {} ({} → {}, Δ{} bytes)", ts, path, hb, ha, size_delta);
            }
            TapeEvent::FsDelete { ts, path, hash_before } => {
                let hb = hash_before.as_ref().map(|h| &h[..h.len().min(8)]).unwrap_or("?");
                println!("[{}] DELETE {} (hash: {})", ts, path, hb);
            }
            TapeEvent::FsRename { ts, path_before, path_after } => {
                println!("[{}] RENAME {} → {}", ts, path_before, path_after);
            }
            TapeEvent::TermOutput { ts, stream, data } => {
                println!("[{}] [{}] {}", ts, stream, data);
            }
            TapeEvent::TermExit { ts, exit_code } => {
                println!("[{}] EXIT   code: {}", ts, exit_code);
            }
            TapeEvent::ApiCall { ts, provider, model, input_tokens, output_tokens } => {
                println!(
                    "[{}] API    {} / {}  tokens: {:?} → {:?}",
                    ts, provider, model, input_tokens, output_tokens
                );
            }
            TapeEvent::GitState { ts, branch, commit, dirty } => {
                let b = branch.as_deref().unwrap_or("?");
                let c = commit.as_deref().map(|s| &s[..s.len().min(8)]).unwrap_or("?");
                println!("[{}] GIT    {} @ {} dirty={}", ts, b, c, dirty);
            }
            TapeEvent::GitDiff { ts, diff } => {
                if diff.is_empty() {
                    println!("[{}] GIT    (no diff)", ts);
                } else {
                    println!("[{}] GIT    diff ({} bytes)", ts, diff.len());
                }
            }
        }
    }
    Ok(())
}
