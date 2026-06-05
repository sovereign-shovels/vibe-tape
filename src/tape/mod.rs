use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    #[serde(rename = "fs")]
    Fs(FsEvent),
    #[serde(rename = "command")]
    Command(CommandEvent),
    #[serde(rename = "api")]
    Api(ApiEvent),
    #[serde(rename = "marker")]
    Marker(MarkerEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FsEvent {
    pub path: String,
    pub operation: String, // create, modify, delete, rename
    pub size_before: Option<u64>,
    pub size_after: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandEvent {
    pub command: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub stdout_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiEvent {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarkerEvent {
    pub label: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapeEvent {
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub event: EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapeHeader {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub cwd: String,
    pub tags: Vec<String>,
}

pub struct TapeWriter {
    file: File,
}

impl TapeWriter {
    pub fn create(path: &Path, header: TapeHeader) -> Result<Self> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("Failed to create tape at {}", path.display()))?;

        let header_line = serde_json::to_string(&header)?;
        writeln!(file, "{}", header_line)?;

        Ok(Self { file })
    }

    pub fn append(&mut self, event: TapeEvent) -> Result<()> {
        let line = serde_json::to_string(&event)?;
        writeln!(self.file, "{}", line)?;
        self.file.flush()?;
        Ok(())
    }
}

pub struct TapeReader {
    header: TapeHeader,
    events: Vec<TapeEvent>,
}

impl TapeReader {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open tape at {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let header_line = lines
            .next()
            .context("Empty tape file")??;
        let header: TapeHeader = serde_json::from_str(&header_line)
            .context("Failed to parse tape header")?;

        let mut events = Vec::new();
        for line in lines {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: TapeEvent = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse event: {}", line))?;
            events.push(event);
        }

        Ok(Self { header, events })
    }

    pub fn header(&self) -> &TapeHeader {
        &self.header
    }

    pub fn events(&self) -> &[TapeEvent] {
        &self.events
    }

    pub fn duration_secs(&self) -> i64 {
        if self.events.len() < 2 {
            return 0;
        }
        let start = self.events.first().unwrap().timestamp;
        let end = self.events.last().unwrap().timestamp;
        (end - start).num_seconds()
    }

    pub fn fs_summary(&self) -> Vec<(String, usize)> {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for event in &self.events {
            if let EventType::Fs(fs) = &event.event {
                *counts.entry(fs.operation.clone()).or_insert(0) += 1;
            }
        }
        let mut result: Vec<_> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    pub fn files_touched(&self) -> Vec<String> {
        let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();
        for event in &self.events {
            if let EventType::Fs(fs) = &event.event {
                files.insert(fs.path.clone());
            }
        }
        let mut result: Vec<_> = files.into_iter().collect();
        result.sort();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_and_read_tape() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.tape");

        let header = TapeHeader {
            version: "0.1.0".to_string(),
            created_at: Utc::now(),
            cwd: "/tmp".to_string(),
            tags: vec!["test".to_string()],
        };

        let mut writer = TapeWriter::create(&path, header.clone()).unwrap();
        writer
            .append(TapeEvent {
                timestamp: Utc::now(),
                event: EventType::Fs(FsEvent {
                    path: "src/main.rs".to_string(),
                    operation: "modify".to_string(),
                    size_before: Some(100),
                    size_after: Some(150),
                }),
            })
            .unwrap();
        writer
            .append(TapeEvent {
                timestamp: Utc::now(),
                event: EventType::Marker(MarkerEvent {
                    label: "checkpoint".to_string(),
                    note: Some("before refactor".to_string()),
                }),
            })
            .unwrap();

        let reader = TapeReader::open(&path).unwrap();
        assert_eq!(reader.header().version, "0.1.0");
        assert_eq!(reader.events().len(), 2);
        assert_eq!(reader.files_touched(), vec!["src/main.rs".to_string()]);
        assert_eq!(reader.fs_summary(), vec![("modify".to_string(), 1)]);
    }

    #[test]
    fn test_empty_tape() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.tape");

        let header = TapeHeader {
            version: "0.1.0".to_string(),
            created_at: Utc::now(),
            cwd: "/".to_string(),
            tags: vec![],
        };

        let _writer = TapeWriter::create(&path, header).unwrap();
        let reader = TapeReader::open(&path).unwrap();
        assert_eq!(reader.events().len(), 0);
        assert_eq!(reader.duration_secs(), 0);
    }
}
