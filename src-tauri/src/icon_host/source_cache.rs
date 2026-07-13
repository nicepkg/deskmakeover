//! The `dmicon://` content-addressed source cache + the icon protocol URL helper.

use std::collections::{HashMap, VecDeque};

use dm_domain::DecodedImage;

/// The platform-correct custom-protocol URL for an icon source (mirrors the wallpaper protocol).
/// [WINDOWS-VERIFY] the `http://dmicon.localhost` WebView2 form on the real box.
fn icon_protocol_url(key: &str) -> String {
    if cfg!(windows) {
        format!("http://dmicon.localhost/{key}")
    } else {
        format!("dmicon://localhost/{key}")
    }
}

/// Inserts an extracted source into `sources` under a CONTENT-ADDRESSED key
/// `"<itemId>/<slot>/<hash>"` and returns its protocol URL. Content-addressing makes the
/// `immutable` Cache-Control header honest (codex Major 4): identical pixels → identical URL (a
/// legitimate cache hit); changed pixels → a new URL (never a stale reuse of a prior process's
/// bytes). Written into a caller-owned local map so the whole cache swaps atomically per scan.
/// The byte cap for the `dmicon://` source cache — generous enough to hold several scan generations
/// of a large desktop (256px PNGs are small), bounding memory while covering the handoff window.
pub(super) const SOURCE_CACHE_CAP: usize = 32 * 1024 * 1024;

/// The hard ceiling on ONE scan's decoded, deduped source-preview bytes (codex R2 B-5). Set below
/// `SOURCE_CACHE_CAP` so the live generation the scan pins leaves headroom for the previous
/// generation's in-flight URLs during the swap→adopt handoff. Items past it are served preview-less.
pub(super) const SCAN_SOURCE_BUDGET: usize = SOURCE_CACHE_CAP * 3 / 4;

/// The `dmicon://` source cache. A scan republishes the freshly-extracted generation, but the OLD
/// webview frame keeps requesting the PREVIOUS scan's content-addressed URLs until the frontend
/// re-renders against the new scan DTO — serving only the live generation would 404 those in-flight
/// requests during the swap→adopt handoff (codex R3-Major 5 / R4-Major 2). A fixed two-generation
/// window could still evict a URL the UI had not yet adopted (a scan whose adopt failed, then another
/// scan). Instead this is a byte-bounded, content-keyed LRU: each scan re-inserts its live keys
/// (refreshing their recency), so an unchanged icon never ages out and a CHANGED icon's superseded
/// key survives several more generations before the cap evicts it — covering the handoff generously
/// without unbounded growth. Content addressing dedups (one entry per unique pixel set).
pub(super) struct SourceCache {
    map: HashMap<String, Vec<u8>>,
    /// Insertion/refresh order, front = oldest — the LRU eviction queue.
    order: VecDeque<String>,
    bytes: usize,
    cap: usize,
}

impl SourceCache {
    pub(super) fn new(cap_bytes: usize) -> Self {
        Self { map: HashMap::new(), order: VecDeque::new(), bytes: 0, cap: cap_bytes }
    }

    /// Publishes a freshly-extracted generation: inserts every entry (a re-inserted content key
    /// refreshes its recency, so live icons never evict), then trims the oldest HISTORICAL keys past
    /// the byte cap while PINNING this generation's own keys (codex R5-#7). The scan DTO advertises
    /// every key in `next`, so the webview will request each one — evicting a live key mid-publish
    /// would 404 the current desktop. If this one generation alone exceeds the cap (a very large
    /// desktop of high-entropy icons), the cache is left temporarily over the cap holding the full
    /// working set, rather than dropping a live key: the next scan trims the then-historical excess.
    pub(super) fn publish(&mut self, next: HashMap<String, Vec<u8>>) {
        let pinned: std::collections::HashSet<String> = next.keys().cloned().collect();
        for (k, v) in next {
            self.insert_raw(k, v);
        }
        self.trim(&pinned);
    }

    /// Inserts/refreshes one entry without trimming (a re-inserted key moves to most-recent).
    fn insert_raw(&mut self, key: String, bytes: Vec<u8>) {
        if let Some(old) = self.map.remove(&key) {
            self.bytes -= old.len();
            self.order.retain(|k| k != &key);
        }
        self.bytes += bytes.len();
        self.order.push_back(key.clone());
        self.map.insert(key, bytes);
    }

    /// Evicts the oldest NON-pinned keys until under the cap. `pinned` (the current generation) is
    /// never evicted; once only pinned keys remain the cache stops trimming even if still over cap.
    fn trim(&mut self, pinned: &std::collections::HashSet<String>) {
        let mut idx = 0;
        while self.bytes > self.cap && idx < self.order.len() {
            if pinned.contains(&self.order[idx]) {
                idx += 1; // a live key — skip it, never evict the generation being served
                continue;
            }
            let key = self.order.remove(idx).expect("index in bounds");
            if let Some(v) = self.map.remove(&key) {
                self.bytes -= v.len();
            }
            // `remove(idx)` shifted the tail down, so the next candidate is again at `idx`.
        }
    }

    /// The bytes for a content-addressed key (read-only; recency is refreshed by re-scan, not by get).
    pub(super) fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.map.get(key).cloned()
    }
}

/// Caches one source's PNG (moving the buffer) and returns its protocol URL plus the NEW bytes it
/// added to `sources` — 0 when content addressing dedups an already-present key, so the caller's
/// running per-scan budget counts each unique source once (codex R2 B-5).
pub(super) fn cache_source_into(
    sources: &mut HashMap<String, Vec<u8>>,
    item_id: &str,
    slot: u32,
    src: DecodedImage,
) -> (String, usize) {
    let key = format!("{item_id}/{slot}/{}", &dm_icon_codec::content_hash(&src.png)[..16]);
    let added = if sources.contains_key(&key) { 0 } else { src.png.len() };
    sources.insert(key.clone(), src.png); // MOVE the owned PNG buffer — no clone (codex R2 B-12)
    (icon_protocol_url(&key), added)
}
