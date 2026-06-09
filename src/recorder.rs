use crate::tape::{TapeEvent, TapeWriter};
use crate::watcher::FsWatcher;
use chrono::Utc;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;

pub async fn run(
    cmd: &str,
    args: &[String],
    cwd: &Path,
    output_path: &Path,
) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let cwd = cwd.to_path_buf();
    let (tape_tx, tape_rx) = mpsc::channel::<TapeEvent>();
    let output_path = output_path.to_path_buf();

    let tape_thread = std::thread::spawn(move || {
        let file = std::fs::File::create(&output_path)?;
        let mut writer = TapeWriter::new(file);
        while let Ok(event) = tape_rx.recv() {
            writer.write_event(&event)?;
        }
        writer.finish()?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });

    let start = Utc::now();
    let full_cmd = if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args.join(" "))
    };
    tape_tx.send(TapeEvent::SessionStart {
        ts: start,
        cmd: full_cmd,
        cwd: cwd.to_string_lossy().to_string(),
    })?;

    if let Some(git_state) = capture_git_state(&cwd) {
        tape_tx.send(git_state)?;
    }

    let watcher = FsWatcher::new(cwd.clone())?;
    // Small grace period for watcher initialization
    tokio::time::sleep(Duration::from_millis(100)).await;
    let running = Arc::new(AtomicBool::new(true));
    let running_fs = running.clone();

    let mut child = tokio::process::Command::new(cmd)
        .args(args)
        .current_dir(&cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let tape_tx_stdout = tape_tx.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("{}", line);
            let _ = tape_tx_stdout.send(TapeEvent::TermOutput {
                ts: Utc::now(),
                stream: "stdout".into(),
                data: line,
            });
        }
    });

    let tape_tx_stderr = tape_tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("{}", line);
            let _ = tape_tx_stderr.send(TapeEvent::TermOutput {
                ts: Utc::now(),
                stream: "stderr".into(),
                data: line,
            });
        }
    });

    let tape_tx_fs = tape_tx.clone();
    let fs_handle = tokio::task::spawn(async move {
        while running_fs.load(Ordering::Relaxed) {
            match watcher.try_recv() {
                Ok(event) => {
                    let tape_event = match event {
                        crate::watcher::FsEvent::Create { path, hash } => TapeEvent::FsCreate {
                            ts: Utc::now(),
                            path,
                            hash_after: hash,
                        },
                        crate::watcher::FsEvent::Write {
                            path,
                            hash_before,
                            hash_after,
                            size_delta,
                        } => TapeEvent::FsWrite {
                            ts: Utc::now(),
                            path,
                            hash_before,
                            hash_after,
                            size_delta,
                        },
                        crate::watcher::FsEvent::Delete { path, hash_before } => TapeEvent::FsDelete {
                            ts: Utc::now(),
                            path,
                            hash_before,
                        },
                        crate::watcher::FsEvent::Rename { from, to } => TapeEvent::FsRename {
                            ts: Utc::now(),
                            path_before: from,
                            path_after: to,
                        },
                    };
                    let _ = tape_tx_fs.send(tape_event);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
    });

    let status = child.wait().await?;
    let exit_code = status.code().unwrap_or(-1);

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    // Grace period for FS events to arrive before stopping watcher
    tokio::time::sleep(Duration::from_millis(500)).await;
    running.store(false, Ordering::Relaxed);
    let _ = fs_handle.await;

    tape_tx.send(TapeEvent::TermExit {
        ts: Utc::now(),
        exit_code,
    })?;

    if let Some(diff) = capture_git_diff(&cwd) {
        tape_tx.send(TapeEvent::GitDiff {
            ts: Utc::now(),
            diff,
        })?;
    }

    let duration = (Utc::now() - start).num_seconds().max(0) as u64;
    tape_tx.send(TapeEvent::SessionEnd {
        ts: Utc::now(),
        duration_sec: duration,
    })?;

    drop(tape_tx);
    tape_thread
        .join()
        .map_err(|e| format!("tape thread panicked: {:?}", e))??;

    Ok(exit_code)
}

fn capture_git_state(cwd: &Path) -> Option<TapeEvent> {
    let branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(true);

    Some(TapeEvent::GitState {
        ts: Utc::now(),
        branch,
        commit,
        dirty,
    })
}

fn capture_git_diff(cwd: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}
