use crate::tape::{EventType, FsEvent, TapeEvent, TapeWriter};
use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

pub struct FsWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
}

impl FsWatcher {
    pub fn new(path: &Path) -> Result<Self> {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )?;
        watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(Self { _watcher: watcher, rx })
    }

    pub fn poll_events(&self, writer: &mut TapeWriter, base_path: &Path) -> Result<bool> {
        // Non-blocking poll
        let mut had_events = false;
        while let Ok(result) = self.rx.try_recv() {
            had_events = true;
            match result {
                Ok(event) => {
                    for path in &event.paths {
                        let rel_path = path.strip_prefix(base_path).unwrap_or(path);
                        let operation = match event.kind {
                            notify::EventKind::Create(_) => "create",
                            notify::EventKind::Modify(_) => "modify",
                            notify::EventKind::Remove(_) => "delete",
                            _ => continue,
                        };

                        let size_after = if path.exists() {
                            std::fs::metadata(path).ok().map(|m| m.len())
                        } else {
                            None
                        };

                        let event = TapeEvent {
                            timestamp: chrono::Utc::now(),
                            event: EventType::Fs(FsEvent {
                                path: rel_path.to_string_lossy().to_string(),
                                operation: operation.to_string(),
                                size_before: None, // Would need tracking for accurate before
                                size_after,
                            }),
                        };
                        if let Err(e) = writer.append(event) {
                            tracing::warn!("Failed to write event: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Watch error: {:?}", e);
                }
            }
        }
        Ok(had_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tape::TapeHeader;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_detects_file_creation() {
        let dir = TempDir::new().unwrap();
        let tape_path = dir.path().join("session.tape");
        let watch_path = dir.path().join("workspace");
        std::fs::create_dir(&watch_path).unwrap();

        let header = TapeHeader {
            version: "0.1.0".to_string(),
            created_at: chrono::Utc::now(),
            cwd: watch_path.to_string_lossy().to_string(),
            tags: vec![],
        };
        let mut writer = TapeWriter::create(&tape_path, header).unwrap();
        let watcher = FsWatcher::new(&watch_path).unwrap();

        // Create a file
        std::fs::write(watch_path.join("test.txt"), "hello").unwrap();
        thread::sleep(Duration::from_millis(200));

        watcher.poll_events(&mut writer, &watch_path).unwrap();

        let reader = crate::tape::TapeReader::open(&tape_path).unwrap();
        let fs_events: Vec<_> = reader
            .events()
            .iter()
            .filter(|e| matches!(e.event, EventType::Fs(_)))
            .collect();
        assert!(!fs_events.is_empty(), "Should have detected file creation");
    }
}
