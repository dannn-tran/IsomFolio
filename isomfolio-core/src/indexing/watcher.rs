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

fn classify_event(event: &Event) -> Option<FileEvent> {
    let path = event.paths.first()?.to_str()?;
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

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
    let pending: Arc<Mutex<HashMap<String, (FileEvent, Instant)>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let pending_dispatch = Arc::clone(&pending);
    let dispatch = Arc::new(dispatch);
    let dispatch_clone = Arc::clone(&dispatch);

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_thread = Arc::clone(&shutdown);

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

            if let Some(file_event) = classify_event(&event) {
                let debounce_key = match &file_event {
                    FileEvent::Created(p) | FileEvent::Deleted(p) | FileEvent::Modified(p) => {
                        p.clone()
                    }
                    FileEvent::Renamed { new_path, .. } => new_path.clone(),
                    FileEvent::SyncProgress(_) => continue,
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

    Ok(FileWatcher { _watcher: watcher, shutdown })
}

pub fn stop_watcher(_watcher: FileWatcher) {
    // Drop the watcher — notify cleans up automatically
}

/// Watch the OS mount-container directories **non-recursively** (so we observe
/// volumes mounting/unmounting as entries appearing/disappearing — never the
/// contents of the drives themselves) and invoke `on_change` once per burst.
/// Event-driven removable-drive detection. `Err` if nothing could be watched
/// (e.g. Windows, where there's no mount directory — callers fall back to poll).
pub fn create_mount_watcher<F>(dirs: &[String], on_change: F) -> Result<FileWatcher, crate::models::AppError>
where
    F: Fn() + Send + Sync + 'static,
{
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_thread = Arc::clone(&shutdown);

    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
    let (sig_tx, sig_rx) = std::sync::mpsc::channel::<()>();

    // Forward every raw event to a debounce signal.
    std::thread::spawn(move || {
        for result in rx {
            if result.is_ok() {
                let _ = sig_tx.send(());
            }
        }
    });

    // Coalesce a burst of mount/unmount events into one callback.
    std::thread::spawn(move || {
        while sig_rx.recv().is_ok() {
            if shutdown_thread.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(400));
            while sig_rx.try_recv().is_ok() {}
            if shutdown_thread.load(Ordering::Relaxed) {
                break;
            }
            on_change();
        }
    });

    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .map_err(|e| crate::models::AppError::Watcher(e.to_string()))?;
    let mut watched = 0usize;
    for dir in dirs {
        if watcher
            .watch(Path::new(dir), RecursiveMode::NonRecursive)
            .is_ok()
        {
            watched += 1;
        }
    }
    if watched == 0 {
        return Err(crate::models::AppError::Watcher("no mount directories to watch".into()));
    }
    Ok(FileWatcher { _watcher: watcher, shutdown })
}
