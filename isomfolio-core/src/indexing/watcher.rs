use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::file_index::is_supported_extension;
use crate::indexing::types::FileEvent;
use crate::path_utils::normalize_path;

const DEBOUNCE_MS: u64 = 250;
const SELF_WRITE_WINDOW_MS: u64 = 500;

/// Register an XMP path as written by us so the next watcher event is suppressed.
pub struct SelfWriteGuard {
    registry: Arc<Mutex<HashMap<String, Instant>>>,
}

impl SelfWriteGuard {
    pub fn register(&self, xmp_path: &str) {
        let mut map = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(xmp_path.to_string(), Instant::now());
    }
}

fn is_self_write(registry: &Arc<Mutex<HashMap<String, Instant>>>, path: &str) -> bool {
    let mut map = registry.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&ts) = map.get(path) {
        if ts.elapsed() < Duration::from_millis(SELF_WRITE_WINDOW_MS) {
            map.remove(path);
            return true;
        }
    }
    false
}

fn resolve_xmp_to_image(xmp_path: &str) -> Option<String> {
    let base = Path::new(xmp_path).with_extension("");
    ["jpg", "jpeg", "png", "webp", "gif"].iter().find_map(|ext| {
        let candidate = format!("{}.{}", base.display(), ext);
        if Path::new(&candidate).exists() {
            Some(normalize_path(&candidate))
        } else {
            None
        }
    })
}

fn classify_event(event: &Event, xmp_path: &str) -> Option<FileEvent> {
    let path = event.paths.first()?.to_str()?;
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "xmp" {
        return match &event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                resolve_xmp_to_image(xmp_path).map(FileEvent::SidecarChanged)
            }
            EventKind::Remove(_) => {
                resolve_xmp_to_image(xmp_path).map(FileEvent::SidecarRemoved)
            }
            _ => None,
        };
    }

    if !is_supported_extension(&ext) {
        return None;
    }

    let norm = normalize_path(path);
    match &event.kind {
        EventKind::Create(CreateKind::File) | EventKind::Create(CreateKind::Any) => {
            Some(FileEvent::Created(norm))
        }
        EventKind::Remove(RemoveKind::File) | EventKind::Remove(RemoveKind::Any) => {
            Some(FileEvent::Deleted(norm))
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            let old = event.paths.first()?.to_str().map(normalize_path)?;
            let new = event.paths.get(1)?.to_str().map(normalize_path)?;
            Some(FileEvent::Renamed { old_path: old, new_path: new })
        }
        EventKind::Modify(_) => Some(FileEvent::Modified(norm)),
        _ => None,
    }
}

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    pub self_write: SelfWriteGuard,
    shutdown: Arc<AtomicBool>,
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub fn create_watcher<F>(root_path: &str, dispatch: F) -> Result<FileWatcher, crate::models::AppError>
where
    F: Fn(FileEvent) + Send + Sync + 'static,
{
    let self_write_registry: Arc<Mutex<HashMap<String, Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let registry_clone = Arc::clone(&self_write_registry);

    // Pending debounce: path → (FileEvent, last_seen)
    let pending: Arc<Mutex<HashMap<String, (FileEvent, Instant)>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let pending_dispatch = Arc::clone(&pending);
    let dispatch = Arc::new(dispatch);
    let dispatch_clone = Arc::clone(&dispatch);

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_thread = Arc::clone(&shutdown);

    // Notification channel: event thread signals flusher whenever pending changes.
    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<()>();

    // Debounce flusher thread — blocks until the next expiry rather than polling.
    std::thread::spawn(move || {
        let debounce = Duration::from_millis(DEBOUNCE_MS);
        loop {
            let sleep_dur = {
                let map = pending_dispatch.lock().unwrap_or_else(|e| e.into_inner());
                if map.is_empty() {
                    Duration::from_secs(60)
                } else {
                    map.values()
                        .map(|(_, ts)| debounce.saturating_sub(ts.elapsed()))
                        .min()
                        .unwrap_or(debounce)
                }
            };
            let _ = notify_rx.recv_timeout(sleep_dur);
            if shutdown_thread.load(Ordering::Relaxed) {
                break;
            }
            let ready: Vec<FileEvent> = {
                let mut map = pending_dispatch.lock().unwrap_or_else(|e| e.into_inner());
                let now = Instant::now();
                let keys: Vec<String> = map
                    .iter()
                    .filter(|(_, (_, ts))| now.duration_since(*ts) >= debounce)
                    .map(|(k, _)| k.clone())
                    .collect();
                keys.into_iter().filter_map(|k| map.remove(&k).map(|(ev, _)| ev)).collect()
            };
            for ev in ready {
                dispatch_clone(ev);
            }
        }
    });

    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

    std::thread::spawn(move || {
        for result in rx {
            let event = match result {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Watcher error: {e}");
                    continue;
                }
            };

            let path_str = event.paths.first()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();

            if is_self_write(&registry_clone, &path_str) {
                continue;
            }

            if let Some(file_event) = classify_event(&event, &path_str) {
                let debounce_key = match &file_event {
                    FileEvent::Created(p) | FileEvent::Deleted(p)
                    | FileEvent::Modified(p) | FileEvent::SidecarChanged(p)
                    | FileEvent::SidecarRemoved(p) => p.clone(),
                    FileEvent::Renamed { new_path, .. } => new_path.clone(),
                    FileEvent::ScanProgress(_) => continue,
                };
                {
                    let mut map = pending.lock().unwrap_or_else(|e| e.into_inner());
                    map.insert(debounce_key, (file_event, Instant::now()));
                }
                let _ = notify_tx.send(());
            }
        }
    });

    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .map_err(|e| crate::models::AppError::Watcher(e.to_string()))?;

    watcher
        .watch(Path::new(root_path), RecursiveMode::Recursive)
        .map_err(|e| crate::models::AppError::Watcher(e.to_string()))?;

    Ok(FileWatcher {
        _watcher: watcher,
        self_write: SelfWriteGuard { registry: self_write_registry },
        shutdown,
    })
}

pub fn stop_watcher(_watcher: FileWatcher) {
    // Drop the watcher — notify cleans up automatically
}
