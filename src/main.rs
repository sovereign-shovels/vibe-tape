mod config;
mod diff;
mod patch;
mod recorder;
mod replay;
mod tape;
mod watcher;

use std::env;
use std::path::Path;

fn print_usage() {
    eprintln!("vibe-tape 0.1.0");
    eprintln!("Record and replay every AI agent session.");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  vibe-tape run <command> [args...]   Record a session");
    eprintln!("  vibe-tape replay <file.tape>        Replay a session");
    eprintln!("  vibe-tape diff <file.tape>          Show diff against git HEAD");
    eprintln!("  vibe-tape patch <file.tape>         Export as git patch");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --output <path>    Set output path for 'run' (default: session_<timestamp>.tape)");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "run" => {
            if args.len() < 3 {
                eprintln!("Error: missing command to run");
                print_usage();
                std::process::exit(1);
            }

            let mut output_path = None;
            let mut cmd_idx = 2;

            if args[2] == "--output" {
                if args.len() < 5 {
                    eprintln!("Error: --output requires a path");
                    std::process::exit(1);
                }
                output_path = Some(args[3].clone());
                cmd_idx = 4;
            }

            let cmd = &args[cmd_idx];
            let cmd_args = &args[(cmd_idx + 1)..];
            let cwd = env::current_dir().expect("failed to get current directory");

            let out = output_path.map(|p| std::path::PathBuf::from(p)).unwrap_or_else(|| {
                let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                std::path::PathBuf::from(format!("session_{}.tape", ts))
            });

            match recorder::run(cmd, cmd_args, &cwd, &out).await {
                Ok(exit_code) => {
                    eprintln!("\n[vibe-tape] Session saved to {}", out.display());
                    std::process::exit(exit_code);
                }
                Err(e) => {
                    eprintln!("[vibe-tape] Recording failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "replay" => {
            if args.len() < 3 {
                eprintln!("Error: missing tape file");
                print_usage();
                std::process::exit(1);
            }
            if let Err(e) = replay::replay(Path::new(&args[2])) {
                eprintln!("[vibe-tape] Replay failed: {}", e);
                std::process::exit(1);
            }
        }
        "diff" => {
            if args.len() < 3 {
                eprintln!("Error: missing tape file");
                print_usage();
                std::process::exit(1);
            }
            if let Err(e) = diff::diff(Path::new(&args[2])) {
                eprintln!("[vibe-tape] Diff failed: {}", e);
                std::process::exit(1);
            }
        }
        "patch" => {
            if args.len() < 3 {
                eprintln!("Error: missing tape file");
                print_usage();
                std::process::exit(1);
            }
            if let Err(e) = patch::patch(Path::new(&args[2])) {
                eprintln!("[vibe-tape] Patch failed: {}", e);
                std::process::exit(1);
            }
        }
        "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Error: unknown command '{}'", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tape::{read_tape, TapeEvent, TapeWriter};
    use chrono::Utc;

    #[test]
    fn test_tape_read_write_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.tape");
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut writer = TapeWriter::new(file);
            writer.write_event(&TapeEvent::SessionStart {
                ts: Utc::now(),
                cmd: "echo hi".into(),
                cwd: "/tmp".into(),
            }).unwrap();
            writer.finish().unwrap();
        }
        let events = read_tape(&path).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            TapeEvent::SessionStart { cmd, .. } => assert_eq!(cmd, "echo hi"),
            _ => panic!("expected session_start"),
        }
    }

    #[test]
    fn test_relativize_paths_in_tape() {
        let event = TapeEvent::FsWrite {
            ts: Utc::now(),
            path: "src/main.rs".into(),
            hash_before: None,
            hash_after: "abc123".into(),
            size_delta: 42,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("src/main.rs"));
    }
}
