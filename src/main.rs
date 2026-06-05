use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;
use vibe_tape::tape::{EventType, MarkerEvent, TapeEvent, TapeHeader, TapeReader, TapeWriter};
use vibe_tape::watcher::FsWatcher;

#[derive(Parser)]
#[command(name = "vibe-tape")]
#[command(about = "Agent session recorder — filesystem watcher, PTY capture, API proxy")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start recording a session
    Record {
        /// Directory to watch
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Tags for the session
        #[arg(short, long)]
        tag: Vec<String>,
        /// Tape file name
        #[arg(short, long)]
        name: Option<String>,
    },
    /// List recorded tapes
    List {
        /// Tape directory
        #[arg(short, long, default_value = "~/.local/share/vibe-tape")]
        dir: PathBuf,
    },
    /// Show tape contents and summary
    Show {
        /// Tape file path
        tape: PathBuf,
    },
    /// Export tape to markdown
    Export {
        /// Tape file path
        tape: PathBuf,
        /// Output path
        #[arg(short, long, default_value = "-")]
        output: String,
    },
    /// Add a marker to the current recording
    Mark {
        /// Marker label
        label: String,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
}

fn expand_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&s[2..]);
        }
    }
    path.to_path_buf()
}

fn tape_dir() -> PathBuf {
    expand_path(&PathBuf::from("~/.local/share/vibe-tape"))
}

fn pid_file() -> PathBuf {
    tape_dir().join(".recording")
}

fn is_recording() -> bool {
    pid_file().exists()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("vibe_tape=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Record { path, tag, name } => {
            if is_recording() {
                eprintln!("Already recording. Run `vibe-tape stop` first.");
                std::process::exit(1);
            }

            let watch_path = std::fs::canonicalize(&path)
                .with_context(|| format!("Cannot access {}", path.display()))?;
            let dir = tape_dir();
            std::fs::create_dir_all(&dir)?;

            let tape_name = name.unwrap_or_else(|| {
                format!(
                    "session-{}",
                    chrono::Local::now().format("%Y%m%d-%H%M%S")
                )
            });
            let tape_path = dir.join(format!("{}.tape", tape_name));

            let header = TapeHeader {
                version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: Utc::now(),
                cwd: watch_path.to_string_lossy().to_string(),
                tags: tag,
            };

            let mut writer = TapeWriter::create(&tape_path, header)?;

            // Write PID file
            let pid = std::process::id();
            std::fs::write(&pid_file(), format!("{}\n{}", pid, tape_path.display()))?;

            println!(
                "▶ Recording session: {}\n  Watching: {}\n  Tape: {}\n  Press Ctrl+C to stop",
                tape_name,
                watch_path.display(),
                tape_path.display()
            );

            let watcher = FsWatcher::new(&watch_path)?;

            // Main loop
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                if let Err(e) = watcher.poll_events(&mut writer, &watch_path) {
                    tracing::warn!("Watch error: {}", e);
                }
                // Check if we should stop (SIGINT handled by tokio)
            }
        }
        Commands::List { dir } => {
            let dir = expand_path(&dir);
            if !dir.exists() {
                println!("No tapes directory found at {}", dir.display());
                return Ok(());
            }

            let mut tapes: Vec<(PathBuf, TapeReader)> = Vec::new();
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("tape") {
                    if let Ok(reader) = TapeReader::open(&path) {
                        tapes.push((path, reader));
                    }
                }
            }

            // Sort by creation time, newest first
            tapes.sort_by(|a, b| b.1.header().created_at.cmp(&a.1.header().created_at));

            println!("\n📼 Recorded Tapes\n{}", "=".repeat(50));
            for (path, reader) in tapes {
                let header = reader.header();
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                let dur = reader.duration_secs();
                let dur_str = if dur < 60 {
                    format!("{}s", dur)
                } else {
                    format!("{}m {}s", dur / 60, dur % 60)
                };
                let files = reader.files_touched().len();
                let tags = if header.tags.is_empty() {
                    "".to_string()
                } else {
                    format!(" [{}]", header.tags.join(", "))
                };
                println!(
                    "  {:20} {:>8}  {:>3} files{}",
                    name, dur_str, files, tags
                );
            }
            println!();
        }
        Commands::Show { tape } => {
            let reader = TapeReader::open(&tape)?;
            let header = reader.header();

            println!("\n📼 {}\n{}", tape.file_stem().unwrap_or_default().to_string_lossy(), "=".repeat(50));
            println!("Version:   {}", header.version);
            println!(
                "Created:   {}",
                header.created_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            println!("CWD:       {}", header.cwd);
            if !header.tags.is_empty() {
                println!("Tags:      {}", header.tags.join(", "));
            }
            println!("Events:    {}", reader.events().len());
            println!("Duration:  {}s", reader.duration_secs());

            let fs_summary = reader.fs_summary();
            if !fs_summary.is_empty() {
                println!("\nFilesystem activity:");
                for (op, count) in fs_summary {
                    println!("  {:10} {}", op, count);
                }
            }

            let files = reader.files_touched();
            if !files.is_empty() {
                println!("\nFiles touched ({}):", files.len());
                for f in files.iter().take(20) {
                    println!("  {}", f);
                }
                if files.len() > 20 {
                    println!("  ... and {} more", files.len() - 20);
                }
            }

            println!("\nTimeline:");
            for event in reader.events() {
                let time = event.timestamp.format("%H:%M:%S");
                match &event.event {
                    EventType::Fs(fs) => {
                        println!("  [{}] {} {}", time, fs.operation, fs.path);
                    }
                    EventType::Marker(m) => {
                        println!(
                            "  [{}] 🚩 {} {}",
                            time,
                            m.label,
                            m.note.as_deref().unwrap_or("")
                        );
                    }
                    _ => {}
                }
            }
            println!();
        }
        Commands::Export { tape, output } => {
            let reader = TapeReader::open(&tape)?;
            let header = reader.header();
            let mut md = String::new();

            md.push_str(&format!("# Session: {}\n\n", tape.file_stem().unwrap_or_default().to_string_lossy()));
            md.push_str(&format!("- **Date:** {}\n", header.created_at.format("%Y-%m-%d %H:%M:%S UTC")));
            md.push_str(&format!("- **CWD:** `{}`\n", header.cwd));
            md.push_str(&format!("- **Duration:** {}s\n", reader.duration_secs()));
            if !header.tags.is_empty() {
                md.push_str(&format!("- **Tags:** {}\n", header.tags.join(", ")));
            }
            md.push_str("\n## Filesystem Activity\n\n");
            md.push_str("| Time | Operation | File |\n");
            md.push_str("|------|-----------|------|\n");

            for event in reader.events() {
                let time = event.timestamp.format("%H:%M:%S");
                match &event.event {
                    EventType::Fs(fs) => {
                        md.push_str(&format!(
                            "| {} | {} | `{}` |\n",
                            time, fs.operation, fs.path
                        ));
                    }
                    EventType::Marker(m) => {
                        md.push_str(&format!(
                            "| {} | 🚩 **{}** | {} |\n",
                            time,
                            m.label,
                            m.note.as_deref().unwrap_or("")
                        ));
                    }
                    _ => {}
                }
            }

            if output == "-" {
                println!("{}", md);
            } else {
                std::fs::write(&output, md)
                    .with_context(|| format!("Failed to write to {}", output))?;
                println!("✓ Exported to {}", output);
            }
        }
        Commands::Mark { label, note } => {
            if !is_recording() {
                eprintln!("No active recording. Run `vibe-tape record` first.");
                std::process::exit(1);
            }

            let pid_content = std::fs::read_to_string(pid_file())?;
            let tape_path = pid_content.lines().nth(1).context("Invalid PID file")?;

            // We need to append to an existing tape. For v0.1, we'll just
            // note that markers can be added if we had a running writer.
            // Since the writer is in another process, we'd need IPC.
            // For now, write a marker file that the recorder picks up.
            let marker_path = PathBuf::from(tape_path).with_extension("marker");
            let event = TapeEvent {
                timestamp: Utc::now(),
                event: EventType::Marker(MarkerEvent { label, note }),
            };
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&marker_path)?;
            writeln!(file, "{}", serde_json::to_string(&event)?)?;
            println!("🚩 Marker added");
        }
    }

    Ok(())
}
