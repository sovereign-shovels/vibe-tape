use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TapeEvent {
    #[serde(rename = "session_start")]
    SessionStart {
        ts: DateTime<Utc>,
        cmd: String,
        cwd: String,
    },
    #[serde(rename = "session_end")]
    SessionEnd {
        ts: DateTime<Utc>,
        duration_sec: u64,
    },
    #[serde(rename = "fs_write")]
    FsWrite {
        ts: DateTime<Utc>,
        path: String,
        hash_before: Option<String>,
        hash_after: String,
        size_delta: i64,
    },
    #[serde(rename = "fs_create")]
    FsCreate {
        ts: DateTime<Utc>,
        path: String,
        hash_after: String,
    },
    #[serde(rename = "fs_delete")]
    FsDelete {
        ts: DateTime<Utc>,
        path: String,
        hash_before: Option<String>,
    },
    #[serde(rename = "fs_rename")]
    FsRename {
        ts: DateTime<Utc>,
        path_before: String,
        path_after: String,
    },
    #[serde(rename = "term_output")]
    TermOutput {
        ts: DateTime<Utc>,
        stream: String, // "stdout" or "stderr"
        data: String,
    },
    #[serde(rename = "term_exit")]
    TermExit {
        ts: DateTime<Utc>,
        exit_code: i32,
    },
    #[serde(rename = "api_call")]
    ApiCall {
        ts: DateTime<Utc>,
        provider: String,
        model: String,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    #[serde(rename = "git_state")]
    GitState {
        ts: DateTime<Utc>,
        branch: Option<String>,
        commit: Option<String>,
        dirty: bool,
    },
    #[serde(rename = "git_diff")]
    GitDiff {
        ts: DateTime<Utc>,
        diff: String,
    },
}

impl TapeEvent {
    #[allow(dead_code)]
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TapeEvent::SessionStart { ts, .. } => *ts,
            TapeEvent::SessionEnd { ts, .. } => *ts,
            TapeEvent::FsWrite { ts, .. } => *ts,
            TapeEvent::FsCreate { ts, .. } => *ts,
            TapeEvent::FsDelete { ts, .. } => *ts,
            TapeEvent::FsRename { ts, .. } => *ts,
            TapeEvent::TermOutput { ts, .. } => *ts,
            TapeEvent::TermExit { ts, .. } => *ts,
            TapeEvent::ApiCall { ts, .. } => *ts,
            TapeEvent::GitState { ts, .. } => *ts,
            TapeEvent::GitDiff { ts, .. } => *ts,
        }
    }
}

pub struct TapeWriter<W: Write> {
    encoder: GzEncoder<W>,
}

impl<W: Write> TapeWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            encoder: GzEncoder::new(writer, Compression::default()),
        }
    }

    pub fn write_event(&mut self, event: &TapeEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let line = serde_json::to_string(event)?;
        writeln!(self.encoder, "{}", line)?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, Box<dyn std::error::Error + Send + Sync>> {
        self.encoder.flush()?;
        Ok(self.encoder.finish()?)
    }
}

pub fn read_tape(path: &Path) -> Result<Vec<TapeEvent>, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() == 0 {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: TapeEvent = serde_json::from_str(&line)?;
        events.push(event);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_roundtrip_event() {
        let event = TapeEvent::SessionStart {
            ts: Utc::now(),
            cmd: "echo hello".into(),
            cwd: "/tmp".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: TapeEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            TapeEvent::SessionStart { cmd, cwd, .. } => {
                assert_eq!(cmd, "echo hello");
                assert_eq!(cwd, "/tmp");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_tape_writer_reader() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = TapeWriter::new(cursor);
        let event1 = TapeEvent::SessionStart {
            ts: Utc::now(),
            cmd: "test".into(),
            cwd: "/".into(),
        };
        let event2 = TapeEvent::TermOutput {
            ts: Utc::now(),
            stream: "stdout".into(),
            data: "hello".into(),
        };
        writer.write_event(&event1).unwrap();
        writer.write_event(&event2).unwrap();
        let cursor = writer.finish().unwrap();
        let bytes = cursor.into_inner();

        let mut reader = Cursor::new(bytes);
        let decoder = GzDecoder::new(&mut reader);
        let buf = BufReader::new(decoder);
        let lines: Vec<String> = buf.lines().map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), 2);
        let e1: TapeEvent = serde_json::from_str(&lines[0]).unwrap();
        let e2: TapeEvent = serde_json::from_str(&lines[1]).unwrap();
        match e1 {
            TapeEvent::SessionStart { cmd, .. } => assert_eq!(cmd, "test"),
            _ => panic!("expected session_start"),
        }
        match e2 {
            TapeEvent::TermOutput { data, .. } => assert_eq!(data, "hello"),
            _ => panic!("expected term_output"),
        }
    }

    #[test]
    fn test_roundtrip_all_event_types() {
        let events: Vec<TapeEvent> = vec![
            TapeEvent::SessionStart { ts: Utc::now(), cmd: "a".into(), cwd: "/".into() },
            TapeEvent::FsCreate { ts: Utc::now(), path: "f.txt".into(), hash_after: "h1".into() },
            TapeEvent::FsWrite { ts: Utc::now(), path: "f.txt".into(), hash_before: Some("h0".into()), hash_after: "h1".into(), size_delta: 1 },
            TapeEvent::FsDelete { ts: Utc::now(), path: "f.txt".into(), hash_before: Some("h1".into()) },
            TapeEvent::FsRename { ts: Utc::now(), path_before: "a.txt".into(), path_after: "b.txt".into() },
            TapeEvent::TermOutput { ts: Utc::now(), stream: "stderr".into(), data: "err".into() },
            TapeEvent::TermExit { ts: Utc::now(), exit_code: 0 },
            TapeEvent::ApiCall { ts: Utc::now(), provider: "p".into(), model: "m".into(), input_tokens: Some(1), output_tokens: Some(2) },
            TapeEvent::GitState { ts: Utc::now(), branch: Some("main".into()), commit: Some("abc".into()), dirty: false },
            TapeEvent::GitDiff { ts: Utc::now(), diff: "diff --git".into() },
            TapeEvent::SessionEnd { ts: Utc::now(), duration_sec: 1 },
        ];

        let cursor = Cursor::new(Vec::new());
        let mut writer = TapeWriter::new(cursor);
        for e in &events {
            writer.write_event(e).unwrap();
        }
        let cursor = writer.finish().unwrap();
        let bytes = cursor.into_inner();

        let mut reader = Cursor::new(bytes);
        let decoder = GzDecoder::new(&mut reader);
        let buf = BufReader::new(decoder);
        let lines: Vec<String> = buf.lines().map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), events.len());
    }

    #[test]
    fn test_tape_empty_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.tape");
        std::fs::write(&path, &[]).unwrap();
        let events = read_tape(&path).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_tape_session_end_duration() {
        let event = TapeEvent::SessionEnd {
            ts: Utc::now(),
            duration_sec: 42,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("42"));
        let decoded: TapeEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            TapeEvent::SessionEnd { duration_sec, .. } => assert_eq!(duration_sec, 42),
            _ => panic!("expected session_end"),
        }
    }
}
