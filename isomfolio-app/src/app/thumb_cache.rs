//! Tier-2 decoded-thumbnail cache. See `dev-docs/thumbnail-cache.md`.
//!
//! Holds decoded RGBA `image::Handle`s for the visible grid window (+ margin) so
//! returning to the grid from the loupe/compare surface refills iced's GPU atlas
//! **synchronously** (RGBA handles reload without the disk-decode worker) instead of
//! re-decoding each tile from disk one by one. Budgeted by bytes, evicted LRU; the
//! resident set is driven by `App::visible_file_ids`, never by current-frame
//! visibility. The view only ever calls `get` (read-only); the update loop drives
//! `reconcile` + `insert`.

use iced::widget::image::Handle;
use image::ImageReader;
use std::collections::HashMap;

struct Entry {
    handle: Handle,
    bytes: usize,
    last_used: u64,
}

pub struct DecodedThumbCache {
    entries: HashMap<String, Entry>,
    budget_bytes: usize,
    used_bytes: usize,
    tick: u64,
}

impl DecodedThumbCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self { entries: HashMap::new(), budget_bytes, used_bytes: 0, tick: 0 }
    }

    /// Read-only lookup for the view. Clones the stored handle (preserving its iced
    /// id — rebuilding via `from_rgba` would mint a new id and force a re-upload
    /// every frame), so the renderer reuses the same atlas texture.
    pub fn get(&self, id: &str) -> Option<Handle> {
        self.entries.get(id).map(|e| e.handle.clone())
    }

    /// Declare the desired resident set (the visible window + margin). Marks the
    /// already-cached warm ids as most-recently-used (so LRU keeps them) and returns
    /// the warm ids **not** yet cached — the caller decodes and `insert`s those.
    pub fn reconcile(&mut self, warm_ids: &[String]) -> Vec<String> {
        self.tick += 1;
        let now = self.tick;
        let mut misses = Vec::new();
        for id in warm_ids {
            match self.entries.get_mut(id) {
                Some(e) => e.last_used = now,
                None => misses.push(id.clone()),
            }
        }
        misses
    }

    /// Store a freshly-decoded handle, then LRU-evict down to the byte budget.
    /// Because `reconcile` re-stamps the warm set and inserts are the newest entries,
    /// eviction only ever drops stragglers from earlier views.
    pub fn insert(&mut self, id: String, handle: Handle, bytes: usize) {
        self.tick += 1;
        let last_used = self.tick;
        if let Some(old) = self.entries.insert(id, Entry { handle, bytes, last_used }) {
            self.used_bytes = self.used_bytes.saturating_sub(old.bytes);
        }
        self.used_bytes += bytes;
        self.evict_to_budget();
    }

    fn evict_to_budget(&mut self) {
        while self.used_bytes > self.budget_bytes {
            let Some(victim) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            if let Some(e) = self.entries.remove(&victim) {
                self.used_bytes = self.used_bytes.saturating_sub(e.bytes);
            }
        }
    }

    #[cfg(test)]
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }
}

/// Decode a Tier-0 cached thumbnail JPEG into an RGBA `image::Handle` and its decoded
/// byte size (`w * h * 4`). Runs off the UI thread — see `App::warm_visible_thumbs`.
pub fn decode_thumb_rgba(path: &str) -> Option<(Handle, usize)> {
    let decoded = ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let rgba = decoded.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let bytes = rgba.into_raw();
    let len = bytes.len();
    Some((Handle::from_rgba(w, h, bytes), len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle() -> Handle {
        Handle::from_rgba(1, 1, vec![0u8; 4])
    }

    #[test]
    fn get_returns_inserted_and_none_for_missing() {
        let mut c = DecodedThumbCache::new(1000);
        c.insert("a".into(), handle(), 40);
        assert!(c.get("a").is_some());
        assert!(c.get("b").is_none());
    }

    #[test]
    fn reconcile_reports_only_missing_warm_ids() {
        let mut c = DecodedThumbCache::new(1000);
        c.insert("a".into(), handle(), 40);
        c.insert("b".into(), handle(), 40);
        let warm = vec!["a".to_string(), "b".to_string(), "x".to_string()];
        assert_eq!(c.reconcile(&warm), vec!["x".to_string()]);
    }

    #[test]
    fn insert_evicts_lru_to_stay_within_budget() {
        let mut c = DecodedThumbCache::new(100); // holds 2 × 40-byte entries
        c.insert("a".into(), handle(), 40);
        c.insert("b".into(), handle(), 40);
        c.insert("c".into(), handle(), 40); // 120 > 100 → evict LRU (a)
        assert!(c.used_bytes() <= 100);
        assert!(c.get("a").is_none());
        assert!(c.get("b").is_some());
        assert!(c.get("c").is_some());
    }

    #[test]
    fn reconcile_protects_warm_set_from_eviction() {
        let mut c = DecodedThumbCache::new(100);
        c.insert("a".into(), handle(), 40);
        c.insert("b".into(), handle(), 40); // {a,b}, c evicts a next
        c.insert("c".into(), handle(), 40); // {b,c}
        c.reconcile(&["b".to_string()]); // b is now most-recent
        c.insert("d".into(), handle(), 40); // over budget → evict LRU (c, not b)
        assert!(c.get("b").is_some(), "touched warm id must survive");
        assert!(c.get("d").is_some());
        assert!(c.get("c").is_none());
    }
}
