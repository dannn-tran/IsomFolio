use image::imageops::FilterType;
use image::codecs::jpeg::JpegEncoder;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app_paths::{
    ensure_directories, preview_cache_dir, preview_cache_path, thumbnail_cache_dir,
};
use crate::models::AppError;

// 640 keeps the largest grid tile (TILE_SIZE_MAX 400px) crisp well into HiDPI.
// RAM is unaffected — the renderer decodes by path and evicts off-screen
// textures, so memory tracks the viewport, not thumbnail resolution; only disk
// grows modestly. The 2048px preview (loupe) is the pixel-accurate inspection
// path, so the grid thumb doesn't need to chase full 2× of the max tile.
const TARGET_SIZE: u32 = 640;
/// Long-edge size of the cached **preview** (the "smart preview" tier): big
/// enough to view/cull full-screen, small enough to keep on disk for offline use.
const PREVIEW_SIZE: u32 = 2048;
const JPEG_QUALITY: u8 = 85;
const RETRY_DELAY_SECS: u64 = 5;

pub fn thumbnail_cache_path(catalog_dir: &str, file_id: &str) -> String {
    Path::new(&thumbnail_cache_dir(catalog_dir))
        .join(format!("{file_id}.jpg"))
        .to_string_lossy()
        .into_owned()
}

pub fn is_cache_valid(catalog_dir: &str, file_id: &str) -> bool {
    Path::new(&thumbnail_cache_path(catalog_dir, file_id)).exists()
}

fn resize_and_save(
    img: image::DynamicImage,
    dest: &str,
    file_id: &str,
    target: u32,
) -> Result<(), AppError> {
    let (w, h) = (img.width(), img.height());
    // Never upscale past the source — keeps a small original from bloating into
    // a larger-but-blurry cache file.
    let scale = (target as f64 / w.max(h) as f64).min(1.0);
    let new_w = ((w as f64 * scale).round() as u32).max(1);
    let new_h = ((h as f64 * scale).round() as u32).max(1);
    let resized = img.resize_exact(new_w, new_h, FilterType::Triangle);
    let tmp = format!("{dest}.tmp");
    {
        let file = fs::File::create(&tmp)
            .map_err(|e| AppError::Thumbnail(file_id.to_string(), e.to_string()))?;
        let mut writer = BufWriter::new(file);
        JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY)
            .encode_image(&resized.to_rgb8())
            .map_err(|e| AppError::Thumbnail(file_id.to_string(), e.to_string()))?;
    }
    fs::rename(&tmp, dest).map_err(|e| AppError::Thumbnail(file_id.to_string(), e.to_string()))
}

// Extract the embedded JPEG thumbnail from a JPEG file's EXIF APP1 segment.
// Returns None if absent or malformed. No external dependencies — pure byte parsing.
fn read_exif_jpeg_thumbnail(file_path: &str) -> Option<Vec<u8>> {
    let data = fs::read(file_path).ok()?;
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        if marker == 0xD8 || marker == 0xD9 {
            pos += 2;
            continue;
        }
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if seg_len < 2 || pos + 2 + seg_len > data.len() {
            break;
        }
        if marker == 0xE1 {
            let seg = &data[pos + 4..pos + 2 + seg_len];
            if seg.starts_with(b"Exif\0\0") {
                if let Some(thumb) = extract_tiff_jpeg_thumbnail(&seg[6..]) {
                    return Some(thumb);
                }
            }
        }
        if marker == 0xDA {
            break;
        }
        pos += 2 + seg_len;
    }
    None
}

fn extract_tiff_jpeg_thumbnail(tiff: &[u8]) -> Option<Vec<u8>> {
    if tiff.len() < 8 {
        return None;
    }
    let le = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |off: usize| -> Option<u16> {
        if off + 2 > tiff.len() { return None; }
        Some(if le { u16::from_le_bytes([tiff[off], tiff[off+1]]) }
             else  { u16::from_be_bytes([tiff[off], tiff[off+1]]) })
    };
    let u32_at = |off: usize| -> Option<u32> {
        if off + 4 > tiff.len() { return None; }
        Some(if le { u32::from_le_bytes([tiff[off], tiff[off+1], tiff[off+2], tiff[off+3]]) }
             else  { u32::from_be_bytes([tiff[off], tiff[off+1], tiff[off+2], tiff[off+3]]) })
    };
    if u16_at(2)? != 42 { return None; }
    let ifd0_off = u32_at(4)? as usize;
    let ifd0_count = u16_at(ifd0_off)? as usize;
    let ifd1_off = u32_at(ifd0_off + 2 + ifd0_count * 12)? as usize;
    if ifd1_off == 0 || ifd1_off + 2 > tiff.len() { return None; }
    let ifd1_count = u16_at(ifd1_off)? as usize;
    let (mut off, mut len) = (None::<u32>, None::<u32>);
    for i in 0..ifd1_count {
        let e = ifd1_off + 2 + i * 12;
        if e + 12 > tiff.len() { break; }
        match u16_at(e)? {
            0x0201 => off = Some(u32_at(e + 8)?),
            0x0202 => len = Some(u32_at(e + 8)?),
            _ => {}
        }
    }
    let off = off? as usize;
    let len = len? as usize;
    if len == 0 || off + len > tiff.len() { return None; }
    let thumb = &tiff[off..off + len];
    if !thumb.starts_with(&[0xFF, 0xD8]) { return None; }
    Some(thumb.to_vec())
}

pub fn is_raw_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "cr2" | "cr3" | "crw" | "nef" | "nrw" | "arw" | "raf" | "orf"
            | "rw2" | "pef" | "dng" | "srw" | "erf" | "mrw"
    )
}

fn decode_raw_preview(file_path: &str) -> Option<image::DynamicImage> {
    use rawler::decoders::RawDecodeParams;
    use rawler::rawsource::RawSource;
    let path = Path::new(file_path);
    let source = RawSource::new(path).ok()?;
    let decoder = rawler::get_decoder(&source).ok()?;
    let params = RawDecodeParams::default();
    // preview_image is the camera-embedded large JPEG — fastest and sufficient for culling.
    // Fall back through smaller thumbnail then full demosaic.
    decoder.preview_image(&source, &params).ok().flatten()
        .or_else(|| decoder.thumbnail_image(&source, &params).ok().flatten())
        .or_else(|| decoder.full_image(&source, &params).ok().flatten())
}

pub fn generate_thumbnail(
    catalog_dir: &str,
    file_id: &str,
    file_path: &str,
) -> Result<String, AppError> {
    let dest = thumbnail_cache_path(catalog_dir, file_id);
    if Path::new(&dest).exists() {
        return Ok(dest);
    }
    ensure_directories(catalog_dir);

    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // RAW files: extract embedded preview (camera-rendered JPEG, no demosaicing needed).
    if is_raw_extension(&ext) {
        let img = decode_raw_preview(file_path)
            .ok_or_else(|| AppError::Thumbnail(file_id.to_string(), "no decodable preview in RAW file".into()))?;
        resize_and_save(img, &dest, file_id, TARGET_SIZE)?;
        return Ok(dest);
    }

    // Fast path for JPEG: try the embedded EXIF thumbnail first.
    // Only use it when large enough to avoid upscaling artefacts.
    if matches!(ext.as_str(), "jpg" | "jpeg") {
        if let Some(thumb_bytes) = read_exif_jpeg_thumbnail(file_path) {
            if let Ok(img) = image::load_from_memory(&thumb_bytes) {
                if img.width().max(img.height()) >= TARGET_SIZE {
                    resize_and_save(img, &dest, file_id, TARGET_SIZE)?;
                    return Ok(dest);
                }
            }
        }
    }

    let img = image::open(file_path)
        .map_err(|e| AppError::Thumbnail(file_id.to_string(), e.to_string()))?;
    resize_and_save(img, &dest, file_id, TARGET_SIZE)?;
    Ok(dest)
}

/// Generate the cached **preview** (a `PREVIEW_SIZE` JPEG) — the offline /
/// loupe tier. Unlike the thumbnail it always decodes the real image (no tiny
/// embedded-thumb fast path), so the result is full-screen quality. Idempotent.
pub fn generate_preview(
    catalog_dir: &str,
    file_id: &str,
    file_path: &str,
) -> Result<String, AppError> {
    let dest = preview_cache_path(catalog_dir, file_id);
    if Path::new(&dest).exists() {
        return Ok(dest);
    }
    let _ = fs::create_dir_all(preview_cache_dir(catalog_dir));

    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let img = if is_raw_extension(&ext) {
        decode_raw_preview(file_path)
            .ok_or_else(|| AppError::Thumbnail(file_id.to_string(), "no decodable preview in RAW file".into()))?
    } else {
        image::open(file_path).map_err(|e| AppError::Thumbnail(file_id.to_string(), e.to_string()))?
    };
    resize_and_save(img, &dest, file_id, PREVIEW_SIZE)?;
    Ok(dest)
}

// Worker pool

#[derive(Debug)]
pub enum ThumbnailMsg {
    Enqueue { file_id: String, file_path: String, priority: i32, retry_count: u32 },
    Done { file_id: String, success: bool, msg: String },
    CancelAll,
    Shutdown,
}

pub struct ThumbnailPool {
    sender: std::sync::mpsc::Sender<ThumbnailMsg>,
}

impl ThumbnailPool {
    pub fn enqueue(&self, file_id: &str, file_path: &str, priority: i32) {
        let _ = self.sender.send(ThumbnailMsg::Enqueue {
            file_id: file_id.to_string(),
            file_path: file_path.to_string(),
            priority,
            retry_count: 0,
        });
    }

    pub fn cancel_all(&self) {
        let _ = self.sender.send(ThumbnailMsg::CancelAll);
    }

    pub fn shutdown(&self) {
        let _ = self.sender.send(ThumbnailMsg::Shutdown);
    }
}

struct PoolState {
    queue: VecDeque<(String, String, i32, u32)>, // (file_id, file_path, priority, retry)
    in_flight: HashSet<String>,
    queued: HashSet<String>,
    active_count: usize,
}

impl PoolState {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            in_flight: HashSet::new(),
            queued: HashSet::new(),
            active_count: 0,
        }
    }

    fn enqueue(&mut self, file_id: String, file_path: String, priority: i32, retry: u32) {
        if !self.in_flight.contains(&file_id) && !self.queued.contains(&file_id) {
            // Insert respecting priority (lower = first)
            let pos = self.queue.partition_point(|(_, _, p, _)| *p <= priority);
            self.queue.insert(pos, (file_id.clone(), file_path, priority, retry));
            self.queued.insert(file_id);
        }
    }

    fn dequeue(&mut self) -> Option<(String, String, u32)> {
        let (file_id, file_path, _, retry) = self.queue.pop_front()?;
        self.queued.remove(&file_id);
        Some((file_id, file_path, retry))
    }
}

fn sweep_tmp_files(catalog_dir: &str) {
    let dir = thumbnail_cache_dir(catalog_dir);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("tmp") {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

pub fn create_worker_pool(
    catalog_dir: &str,
    concurrency: usize,
    gen_previews: bool,
    on_ready: impl Fn(String, String) + Send + Sync + 'static,
    on_failed: impl Fn(String, String) + Send + Sync + 'static,
) -> ThumbnailPool {
    sweep_tmp_files(catalog_dir);
    let catalog_dir = catalog_dir.to_string();
    let (tx, rx) = std::sync::mpsc::channel::<ThumbnailMsg>();
    let on_ready = Arc::new(on_ready);
    let on_failed = Arc::new(on_failed);
    let tx_worker = tx.clone();

    std::thread::spawn(move || {
        let state = Arc::new(Mutex::new(PoolState::new()));

        let process_msg = |msg: ThumbnailMsg| -> bool {
            match msg {
                ThumbnailMsg::Shutdown => return false,
                ThumbnailMsg::CancelAll => {
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    s.queue.clear();
                    s.queued.clear();
                }
                ThumbnailMsg::Enqueue { file_id, file_path, priority, retry_count } => {
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    s.enqueue(file_id, file_path, priority, retry_count);
                }
                ThumbnailMsg::Done { file_id, success, msg } => {
                    {
                        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                        s.in_flight.remove(&file_id);
                        s.active_count = s.active_count.saturating_sub(1);
                    }
                    if success {
                        on_ready(file_id, msg);
                    } else {
                        on_failed(file_id, msg);
                    }
                }
            }
            true
        };

        loop {
            // Block until the next message — a new request when idle, or a worker
            // completion when busy. Workers send ThumbnailMsg::Done on finish, so
            // recv() naturally wakes the coordinator without polling or spin.
            match rx.recv() {
                Err(_) => return,
                Ok(msg) => if !process_msg(msg) { return; }
            }
            // Drain any additional messages that arrived concurrently.
            loop {
                match rx.try_recv() {
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Ok(msg) => if !process_msg(msg) { return; }
                }
            }

            // Spawn workers up to concurrency limit.
            loop {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                if s.active_count >= concurrency {
                    break;
                }
                match s.dequeue() {
                    None => break,
                    Some((file_id, file_path, retry)) => {
                        s.in_flight.insert(file_id.clone());
                        s.active_count += 1;
                        drop(s);

                        let catalog = catalog_dir.clone();
                        let tx_done = tx_worker.clone();

                        std::thread::spawn(move || {
                            match generate_thumbnail(&catalog, &file_id, &file_path) {
                                Ok(path) => {
                                    // Best-effort preview for offline/loupe use; a
                                    // failure here must not fail the thumbnail.
                                    if gen_previews {
                                        if let Err(e) = generate_preview(&catalog, &file_id, &file_path) {
                                            eprintln!("[thumbnail] preview gen failed for {file_id}: {e}");
                                        }
                                    }
                                    let _ = tx_done.send(ThumbnailMsg::Done {
                                        file_id, success: true, msg: path,
                                    });
                                }
                                Err(e) => {
                                    if retry < 1 {
                                        let fid = file_id.clone();
                                        let fp = file_path.clone();
                                        let tx_retry = tx_done.clone();
                                        std::thread::spawn(move || {
                                            std::thread::sleep(Duration::from_secs(RETRY_DELAY_SECS));
                                            let _ = tx_retry.send(ThumbnailMsg::Enqueue {
                                                file_id: fid,
                                                file_path: fp,
                                                priority: 99,
                                                retry_count: retry + 1,
                                            });
                                        });
                                        let _ = tx_done.send(ThumbnailMsg::Done {
                                            file_id, success: false, msg: format!("retry scheduled: {e}"),
                                        });
                                    } else {
                                        let _ = tx_done.send(ThumbnailMsg::Done {
                                            file_id, success: false, msg: e.to_string(),
                                        });
                                    }
                                }
                            }
                        });
                    }
                }
            }
        }
    });

    ThumbnailPool { sender: tx }
}

/// Remove `<id>.jpg` files in `cache_dir` whose `id` isn't in `known`.
fn sweep_orphans(cache_dir: &str, known: &HashSet<String>) -> Result<usize, AppError> {
    if !Path::new(cache_dir).exists() {
        return Ok(0);
    }
    let removed = fs::read_dir(cache_dir)
        .map_err(|e| AppError::Sync(e.to_string()))?
        .filter_map(|e| e.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("jpg"))
        .filter(|path| {
            let file_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            !known.contains(file_id)
        })
        .inspect(|path| {
            let _ = fs::remove_file(path);
        })
        .count();
    Ok(removed)
}

pub fn sweep_thumbnail_cache(
    conn: &rusqlite::Connection,
    catalog_dir: &str,
) -> Result<usize, AppError> {
    let known: HashSet<String> = crate::storage::db::get_all_file_ids(conn)?.into_iter().collect();
    sweep_orphans(&thumbnail_cache_dir(catalog_dir), &known)
}

pub fn sweep_preview_cache(
    conn: &rusqlite::Connection,
    catalog_dir: &str,
) -> Result<usize, AppError> {
    let known: HashSet<String> = crate::storage::db::get_all_file_ids(conn)?.into_iter().collect();
    sweep_orphans(&preview_cache_dir(catalog_dir), &known)
}

/// Drop cached thumbnails and previews whose file is no longer in the catalog
/// (folder removed, files purged, edited externally). One DB id-set query for
/// both. Cheap directory scan — safe to run on catalog open.
pub fn sweep_caches(conn: &rusqlite::Connection, catalog_dir: &str) -> Result<usize, AppError> {
    let known: HashSet<String> = crate::storage::db::get_all_file_ids(conn)?.into_iter().collect();
    let t = sweep_orphans(&thumbnail_cache_dir(catalog_dir), &known)?;
    let p = sweep_orphans(&preview_cache_dir(catalog_dir), &known)?;
    Ok(t + p)
}

/// Bound the preview cache to `max_bytes` by deleting the oldest previews
/// (by mtime) until it fits. `0` = unlimited (no-op). Returns files removed.
/// Previews regenerate on demand when their drive is online, so eviction is safe.
pub fn enforce_preview_cache_cap(catalog_dir: &str, max_bytes: u64) -> Result<usize, AppError> {
    if max_bytes == 0 {
        return Ok(0);
    }
    let dir = preview_cache_dir(catalog_dir);
    if !Path::new(&dir).exists() {
        return Ok(0);
    }
    let mut entries: Vec<(std::path::PathBuf, u64, std::time::SystemTime)> = fs::read_dir(&dir)
        .map_err(|e| AppError::Sync(e.to_string()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("jpg") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            Some((path, meta.len(), mtime))
        })
        .collect();

    let total: u64 = entries.iter().map(|(_, size, _)| *size).sum();
    if total <= max_bytes {
        return Ok(0);
    }
    // Oldest first — evict until under the cap.
    entries.sort_by_key(|(_, _, mtime)| *mtime);
    let mut over = total - max_bytes;
    let mut removed = 0usize;
    for (path, size, _) in entries {
        if over == 0 {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            over = over.saturating_sub(size);
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn cache_path_format() {
        let path = thumbnail_cache_path("/catalog", "abc123");
        assert!(path.contains("abc123.jpg"));
        assert!(path.contains("thumbnails"));
    }

    #[test]
    fn generate_thumbnail_missing_file_errors() {
        let dir = TempDir::new().unwrap();
        let result = generate_thumbnail(dir.path().to_str().unwrap(), "fid", "/nonexistent/x.jpg");
        assert!(result.is_err());
    }

    mod preview_cap {
        use super::*;

        fn write_preview(catalog: &str, name: &str, bytes: usize, mtime_secs: u64) {
            let p = Path::new(&preview_cache_dir(catalog)).join(name);
            fs::write(&p, vec![0u8; bytes]).unwrap();
            let f = fs::OpenOptions::new().write(true).open(&p).unwrap();
            f.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(mtime_secs)).unwrap();
        }

        fn exists(catalog: &str, name: &str) -> bool {
            Path::new(&preview_cache_dir(catalog)).join(name).exists()
        }

        #[test]
        fn evicts_oldest_until_under_cap() {
            let dir = TempDir::new().unwrap();
            let cat = dir.path().to_str().unwrap();
            fs::create_dir_all(preview_cache_dir(cat)).unwrap();
            write_preview(cat, "old.jpg", 1000, 100);
            write_preview(cat, "mid.jpg", 1000, 200);
            write_preview(cat, "new.jpg", 1000, 300);

            // 3000 total, cap 1500 → drop oldest two (down to 1000 ≤ 1500).
            let removed = enforce_preview_cache_cap(cat, 1500).unwrap();
            assert_eq!(removed, 2);
            assert!(!exists(cat, "old.jpg"));
            assert!(!exists(cat, "mid.jpg"));
            assert!(exists(cat, "new.jpg"));
        }

        #[test]
        fn under_cap_keeps_everything() {
            let dir = TempDir::new().unwrap();
            let cat = dir.path().to_str().unwrap();
            fs::create_dir_all(preview_cache_dir(cat)).unwrap();
            write_preview(cat, "a.jpg", 1000, 100);
            assert_eq!(enforce_preview_cache_cap(cat, 5000).unwrap(), 0);
            assert!(exists(cat, "a.jpg"));
        }

        #[test]
        fn zero_means_unlimited() {
            let dir = TempDir::new().unwrap();
            let cat = dir.path().to_str().unwrap();
            fs::create_dir_all(preview_cache_dir(cat)).unwrap();
            write_preview(cat, "a.jpg", 10_000, 100);
            assert_eq!(enforce_preview_cache_cap(cat, 0).unwrap(), 0);
            assert!(exists(cat, "a.jpg"));
        }
    }
}
