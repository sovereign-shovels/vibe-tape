use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum FsEvent {
    Create { path: String, hash: String },
    Write { path: String, hash_before: Option<String>, hash_after: String, size_delta: i64 },
    Delete { path: String, hash_before: Option<String> },
    Rename { from: String, to: String },
}

pub struct FsWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    rx: mpsc::Receiver<FsEvent>,
}

impl FsWatcher {
    pub fn new(cwd: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = mpsc::channel::<FsEvent>();
        let hash_cache: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let cwd_clone = cwd.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    handle_notify_event(event, &cwd_clone, &tx, &hash_cache);
                }
            },
            NotifyConfig::default(),
        )?;
        watcher.watch(&cwd, RecursiveMode::Recursive)?;
        Ok(Self { watcher, rx })
    }

    pub fn try_recv(&self) -> Result<FsEvent, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

fn handle_notify_event(
    event: Event,
    cwd: &Path,
    tx: &mpsc::Sender<FsEvent>,
    hash_cache: &Arc<Mutex<HashMap<String, String>>>,
) {
    let is_ignored = |p: &Path| {
        let s = p.to_string_lossy();
        s.contains("/.git/")
            || s.contains("/target/")
            || s.contains("/node_modules/")
            || s.ends_with(".tape")
            || s.contains("/.next/")
            || s.contains("/.vercel/")
    };

    match event.kind {
        EventKind::Create(_) => {
            for path in &event.paths {
                if is_ignored(path) {
                    continue;
                }
                if let Some(hash) = hash_file(path) {
                    let rel = relativize(path, cwd);
                    let mut cache = hash_cache.lock().unwrap();
                    cache.insert(rel.clone(), hash.clone());
                    drop(cache);
                    let _ = tx.send(FsEvent::Create {
                        path: rel,
                        hash,
                    });
                }
            }
        }
        EventKind::Modify(notify::event::ModifyKind::Data(_)) |
        EventKind::Modify(notify::event::ModifyKind::Any) => {
            for path in &event.paths {
                if is_ignored(path) {
                    continue;
                }
                let rel = relativize(path, cwd);
                let cache = hash_cache.lock().unwrap();
                let hash_before = cache.get(&rel).cloned();
                drop(cache);
                if let Some(hash_after) = hash_file(path) {
                    let size_delta = compute_size_delta(path, hash_before.as_ref(), &hash_after);
                    let mut cache = hash_cache.lock().unwrap();
                    cache.insert(rel.clone(), hash_after.clone());
                    drop(cache);
                    let _ = tx.send(FsEvent::Write {
                        path: rel,
                        hash_before,
                        hash_after,
                        size_delta,
                    });
                }
            }
        }
        EventKind::Remove(_) => {
            for path in &event.paths {
                if is_ignored(path) {
                    continue;
                }
                let rel = relativize(path, cwd);
                let mut cache = hash_cache.lock().unwrap();
                let hash_before = cache.remove(&rel);
                drop(cache);
                let _ = tx.send(FsEvent::Delete {
                    path: rel,
                    hash_before,
                });
            }
        }
        EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::Both)) => {
            if event.paths.len() >= 2 {
                let from = &event.paths[0];
                let to = &event.paths[1];
                if is_ignored(from) || is_ignored(to) {
                    return;
                }
                let rel_from = relativize(from, cwd);
                let rel_to = relativize(to, cwd);
                let mut cache = hash_cache.lock().unwrap();
                let hash = cache.remove(&rel_from);
                if let Some(ref h) = hash {
                    cache.insert(rel_to.clone(), h.clone());
                }
                drop(cache);
                let _ = tx.send(FsEvent::Rename {
                    from: rel_from,
                    to: rel_to,
                });
            }
        }
        _ => {}
    }
}

fn hash_file(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn relativize(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn compute_size_delta(path: &Path, _hash_before: Option<&String>, _hash_after: &String) -> i64 {
    std::fs::metadata(path)
        .map(|m| m.len() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_hash_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello").unwrap();
        let h1 = hash_file(&path).unwrap();
        let h2 = hash_file(&path).unwrap();
        assert_eq!(h1, h2);
        fs::write(&path, "world").unwrap();
        let h3 = hash_file(&path).unwrap();
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_relativize() {
        let cwd = Path::new("/home/user/project");
        let path = Path::new("/home/user/project/src/main.rs");
        assert_eq!(relativize(path, cwd), "src/main.rs");
    }
}
