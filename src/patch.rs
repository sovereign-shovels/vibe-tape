use crate::tape::{read_tape, TapeEvent};
use std::collections::HashSet;
use std::path::Path;

pub fn patch(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let events = read_tape(path)?;

    // Prefer stored git diff from session end
    for event in &events {
        if let TapeEvent::GitDiff { diff, .. } = event {
            if !diff.is_empty() {
                print!("{}", diff);
                return Ok(());
            }
        }
    }

    // Fallback: compute patch for files touched during session
    let mut files = HashSet::new();
    let mut cwd: Option<String> = None;
    for event in &events {
        match event {
            TapeEvent::SessionStart { cwd: c, .. } => {
                cwd = Some(c.clone());
            }
            TapeEvent::FsWrite { path: p, .. }
            | TapeEvent::FsCreate { path: p, .. }
            | TapeEvent::FsDelete { path: p, .. } => {
                files.insert(p.clone());
            }
            TapeEvent::FsRename { path_after: p, .. } => {
                files.insert(p.clone());
            }
            _ => {}
        }
    }

    if files.is_empty() {
        println!("(no file changes recorded — empty patch)");
        return Ok(());
    }

    let cwd = cwd.as_deref().unwrap_or(".");
    let mut cmd = std::process::Command::new("git");
    cmd.arg("diff").arg("HEAD").arg("--patch").current_dir(cwd);
    for f in &files {
        cmd.arg(f);
    }
    let output = cmd.output()?;
    if output.status.success() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        eprintln!(
            "git diff --patch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
