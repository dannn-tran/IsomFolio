use image::imageops::FilterType;
use image::codecs::jpeg::JpegEncoder;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app_paths::{ensure_directories, thumbnail_cache_dir};
use crate::models::AppError;

// 640 keeps the largest grid tile (TILE_SIZE_MAX 400px) crisp well into HiDPI.
// RAM is unaffected — the renderer decodes by path and evicts off-screen
// textures, so memory tracks the viewport, not thumbnail resolution; only disk
// grows modestly. The loupe decodes the original on demand for pixel-accurate
// inspection, so the grid thumb doesn't need to chase the full max tile.
const TARGET_SIZE: u32 = 640;
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

/// Decode a JPEG at a reduced DCT scale (1/8, 1/4, 1/2 of full) — the smallest
/// step still ≥ `target` on the long edge. The decoder skips most of the inverse
/// DCT / upsampling, so a 24MP frame downscales to a thumbnail several times
/// faster (and with a fraction of the RAM) than a full decode. Returns `None`
/// for unusual JPEGs (CMYK, 16-bit, decode error) so the caller can fall back to
/// the general `image::open` path.
fn decode_jpeg_scaled(file_path: &str, target: u32) -> Option<image::DynamicImage> {
    use jpeg_decoder::{Decoder, PixelFormat};
    let file = fs::File::open(file_path).ok()?;
    let mut dec = Decoder::new(std::io::BufReader::new(file));
    let (w, h) = dec.scale(target as u16, target as u16).ok()?;
    let pixels = dec.decode().ok()?;
    let (w, h) = (w as u32, h as u32);
    let info = dec.info()?;
    match info.pixel_format {
        PixelFormat::RGB24 => {
            (pixels.len() as u32 == w * h * 3)
                .then(|| image::RgbImage::from_raw(w, h, pixels))
                .flatten()
                .map(image::DynamicImage::ImageRgb8)
        }
        PixelFormat::L8 => {
            (pixels.len() as u32 == w * h)
                .then(|| image::GrayImage::from_raw(w, h, pixels))
                .flatten()
                .map(image::DynamicImage::ImageLuma8)
        }
        _ => None,
    }
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

    // Fast path for JPEG: decode at a reduced DCT scale instead of full-res.
    if matches!(ext.as_str(), "jpg" | "jpeg") {
        if let Some(img) = decode_jpeg_scaled(file_path, TARGET_SIZE) {
            resize_and_save(img, &dest, file_id, TARGET_SIZE)?;
            return Ok(dest);
        }
        // Fall through to the general decoder for CMYK / 16-bit / odd JPEGs.
    }

    let img = image::open(file_path)
        .map_err(|e| AppError::Thumbnail(file_id.to_string(), e.to_string()))?;
    resize_and_save(img, &dest, file_id, TARGET_SIZE)?;
    Ok(dest)
}

// Worker pool

#[derive(Debug)]
pub enum ThumbnailMsg {
    Enqueue { file_id: String, file_path: String, priority: i32, retry_count: u32 },
    /// Pull these already-queued ids to the front (in the given order) so the
    /// folder/view the user just opened generates ahead of any backlog.
    Prioritize { file_ids: Vec<String> },
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

    /// Reorder the queue so the given ids (current view, in display order) come
    /// first. Ids not currently queued (already done or in flight) are ignored.
    pub fn prioritize(&self, file_ids: &[String]) {
        if file_ids.is_empty() {
            return;
        }
        let _ = self.sender.send(ThumbnailMsg::Prioritize {
            file_ids: file_ids.to_vec(),
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

    /// Move the given queued ids to the front, preserving their order. Each call
    /// assigns a fresh priority band strictly below the current queue minimum, so
    /// the most recently opened view always sorts ahead of older backlog — and
    /// the band is recomputed from the live minimum each time, so priorities
    /// never drift unbounded. In-flight ids (already processing) are left alone.
    fn prioritize(&mut self, ids: &[String]) {
        use std::collections::HashMap;
        let order: HashMap<&String, usize> =
            ids.iter().enumerate().map(|(i, id)| (id, i)).collect();
        let mut pulled: Vec<(String, String, i32, u32)> = Vec::new();
        self.queue.retain(|item| {
            if order.contains_key(&item.0) {
                pulled.push(item.clone());
                false
            } else {
                true
            }
        });
        if pulled.is_empty() {
            return;
        }
        pulled.sort_by_key(|item| order.get(&item.0).copied().unwrap_or(usize::MAX));
        let cur_min = self.queue.iter().map(|(_, _, p, _)| *p).min().unwrap_or(0);
        let base = cur_min.saturating_sub(pulled.len() as i32);
        for (k, (id, path, _, retry)) in pulled.into_iter().enumerate() {
            self.queue.push_back((id, path, base + k as i32, retry));
        }
        // Restore the ascending-priority invariant the partition_point inserts in
        // `enqueue` rely on; the promoted band (all < cur_min) sorts to the front.
        self.queue.make_contiguous().sort_by_key(|(_, _, p, _)| *p);
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
                ThumbnailMsg::Prioritize { file_ids } => {
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    s.prioritize(&file_ids);
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

/// Drop cached thumbnails whose file is no longer in the catalog (folder
/// removed, files purged, edited externally). Cheap directory scan — safe to run
/// on catalog open.
pub fn sweep_caches(conn: &rusqlite::Connection, catalog_dir: &str) -> Result<usize, AppError> {
    let known: HashSet<String> = crate::storage::db::get_all_file_ids(conn)?.into_iter().collect();
    sweep_orphans(&thumbnail_cache_dir(catalog_dir), &known)
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

    #[test]
    fn generate_thumbnail_downscales_large_jpeg() {
        let cat = TempDir::new().unwrap();
        let src = TempDir::new().unwrap();
        let src_path = src.path().join("big.jpg");
        // A 2000x1500 JPEG → the DCT path should decode small and the saved thumb
        // must be capped at TARGET_SIZE on its long edge.
        image::RgbImage::from_fn(2000, 1500, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        })
        .save(&src_path)
        .unwrap();

        let out = generate_thumbnail(
            cat.path().to_str().unwrap(),
            "fid",
            src_path.to_str().unwrap(),
        )
        .unwrap();

        let thumb = image::open(&out).unwrap();
        assert_eq!(thumb.width().max(thumb.height()), TARGET_SIZE);
        // Idempotent: a second call returns the cached path without re-decoding.
        assert_eq!(
            generate_thumbnail(cat.path().to_str().unwrap(), "fid", src_path.to_str().unwrap()).unwrap(),
            out
        );
    }

    mod prioritize_queue {
        use super::*;

        fn enq(s: &mut PoolState, ids: &[&str]) {
            for (i, id) in ids.iter().enumerate() {
                s.enqueue((*id).to_string(), format!("/{id}.jpg"), i as i32, 0);
            }
        }

        fn drain(s: &mut PoolState) -> Vec<String> {
            let mut out = Vec::new();
            while let Some((id, _, _)) = s.dequeue() {
                out.push(id);
            }
            out
        }

        fn ids(v: &[&str]) -> Vec<String> {
            v.iter().map(|s| s.to_string()).collect()
        }

        #[test]
        fn promotes_subset_to_front_in_order() {
            let mut s = PoolState::new();
            enq(&mut s, &["a", "b", "c", "d"]);
            s.prioritize(&ids(&["c", "d"]));
            assert_eq!(drain(&mut s), ids(&["c", "d", "a", "b"]));
        }

        #[test]
        fn newest_view_sorts_ahead_of_earlier_promotion() {
            let mut s = PoolState::new();
            enq(&mut s, &["a", "b", "c", "d"]);
            s.prioritize(&ids(&["a", "b"])); // open folder 1
            s.prioritize(&ids(&["c", "d"])); // then folder 2 — should win
            assert_eq!(drain(&mut s), ids(&["c", "d", "a", "b"]));
        }

        #[test]
        fn unknown_ids_are_ignored() {
            let mut s = PoolState::new();
            enq(&mut s, &["a", "b"]);
            s.prioritize(&ids(&["zzz", "b"]));
            assert_eq!(drain(&mut s), ids(&["b", "a"]));
        }

        #[test]
        fn later_enqueue_stays_behind_promoted_band() {
            let mut s = PoolState::new();
            enq(&mut s, &["a", "b"]);
            s.prioritize(&ids(&["b"])); // b jumps ahead with a negative band
            s.enqueue("e".into(), "/e.jpg".into(), 0, 0); // new normal-priority job
            // b (promoted) first, then a and e by ascending priority.
            assert_eq!(drain(&mut s), ids(&["b", "a", "e"]));
        }
    }
}
