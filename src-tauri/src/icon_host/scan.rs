//! `icons.scan` (enumerate + classify + extract sources) and `icons.getPersisted`.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use dm_contracts::{GridMetricsDto, IconItemDto, IconPersistedDto, IconScanDto};
use dm_operations::{LedgerStore, ScannedItem};

use super::dto::{map_kind, synthetic_layout};
use super::source_cache::{cache_source_into, SCAN_SOURCE_BUDGET};
use super::IconHost;

impl IconHost {
    /// `icons.scan`: enumerate + classify + extract 256px sources (served over `dmicon://`) into raw
    /// items. NO embedded state (D1: the frontend assembles it).
    pub fn scan(&self) -> Result<IconScanDto, String> {
        let items = self.scanner.scan().map_err(|e| e.to_string())?;
        let rev = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        // Ledger-aware extraction (codex extractor-review 🔴1): for an item WE own whose live
        // surface still equals our last-applied fingerprint, the live icon is this app's styled
        // output — extracting it as "the source" would compound Style(Style(orig)) on every
        // re-scan. Snapshot the committed anchors up front (short lock; extraction below is slow
        // and must not hold `mut_state`). The JOURNAL overlays the ledger (codex icons2-🔴1): a
        // committed txn whose ledger upsert faulted is desktop truth the ledger has not caught up
        // to — its Prepared anchor + Applied fingerprint win over a missing/stale row. An
        // INCOMPLETE txn leaves an item's live provenance unknowable: that item is DEGRADED below
        // (shown from live, never bake-able, never anchor-substituted) until recovery reconciles.
        // The snapshot also pins `op_epoch` so a mutation landing during the slow extraction
        // fails the publish fence instead of publishing just-styled pixels as "the raw source"
        // (codex icons2-🔴2).
        let (anchors, unknown_provenance, epoch_at_snapshot) = {
            use dm_operations::{JournalRecord as JR, JournalSink as _};
            let st = self.mut_state.lock().unwrap();
            let mut anchors: HashMap<String, (dm_domain::Fingerprint, dm_domain::RestoreAnchor)> =
                st.ledger
                    .all()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .filter(|e| e.state.is_committed())
                    .map(|e| {
                        (
                            e.item.as_str().to_string(),
                            (e.last_applied_fingerprint, e.original_anchor),
                        )
                    })
                    .collect();
            let mut unknown: std::collections::HashSet<String> = std::collections::HashSet::new();
            let records = st.journal.read_all().map_err(|e| e.to_string())?;
            let terminal: HashMap<u64, bool> = records
                .iter()
                .filter_map(|r| match r {
                    JR::TxnCommitted { txn } => Some((*txn, true)),
                    JR::TxnRolledBack { txn } => Some((*txn, false)),
                    _ => None,
                })
                .collect();
            let mut prepared: HashMap<(u64, String), dm_domain::RestoreAnchor> = HashMap::new();
            for r in &records {
                if let JR::ItemPrepared { txn, item, anchor, .. } = r {
                    match terminal.get(txn) {
                        Some(true) => {
                            prepared.insert((*txn, item.as_str().to_string()), anchor.clone());
                        }
                        // Rolled back → the desktop was walked back; ledger/live are authoritative.
                        Some(false) => {}
                        None => {
                            unknown.insert(item.as_str().to_string());
                        }
                    }
                }
            }
            for r in &records {
                if let JR::ItemApplied { txn, item, new_fingerprint } = r {
                    if let Some(anchor) = prepared.get(&(*txn, item.as_str().to_string())) {
                        anchors.insert(
                            item.as_str().to_string(),
                            (new_fingerprint.clone(), anchor.clone()),
                        );
                    }
                }
            }
            (anchors, unknown, st.op_epoch)
        };
        // Build the new content-addressed source cache in a LOCAL map, then atomically swap it in
        // after extraction (codex R2-Major 3): a failed refresh must not leave the previous scan's
        // still-displayed URLs 404-ing against a half-cleared cache. One bad item does NOT fail the
        // whole scan (codex extractor-review 🟠3): it degrades to styleable:false with a reason —
        // one OneDrive placeholder must not blank a 40-icon desktop.
        // Live positions (technique A) matched BY NAME, the oracle's own matching rule; an
        // unreadable layout (headless session, denied QI) or an unmatched item degrades to the
        // synthetic grid slot — positions are a mirror nicety, never fatal.
        let live_slots: HashMap<String, (i32, i32)> = self
            .geometry
            .positions()
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.name, (s.x, s.y)))
            .collect();
        let mut next_sources: HashMap<String, Vec<u8>> = HashMap::new();
        // Hard per-scan source-preview budget (codex R2 B-5). The source cache pins the whole live
        // generation (a live key is never evicted mid-serve), so without an upstream bound a
        // pathological desktop of thousands of unique high-entropy 256² icons would pin hundreds of MiB
        // past SOURCE_CACHE_CAP. Once the decoded, deduped source bytes reach this budget the remaining
        // items are served WITHOUT a preview (an honest bounded scan) rather than growing unbounded.
        let mut source_bytes = 0usize;
        let mut budget_logged = false;
        let mut dtos = Vec::with_capacity(items.len());
        let mut scanned = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            // Capture the CAS anchor AT SCAN TIME (codex Block 2): a hand-edit during the bake then
            // fails the driver's CAS instead of being silently overwritten. Read BEFORE
            // extraction: the fingerprint decides whether the live surface is our own styled
            // output (→ extract from the original anchor instead). An unreadable surface keeps a
            // sentinel fingerprint for display purposes but is stripped of APPLY AUTHORITY
            // (`source_ok:false` → the commit refuses it; codex icons2-🟠5 — the sentinel is a
            // legal CAS value for empty bytes, so it must never carry authority on its own).
            let read = self.reader.read_fingerprint(&item.target());
            let unreadable = read.is_err();
            let fingerprint =
                read.unwrap_or_else(|_| dm_domain::Fingerprint::of_bytes(b""));
            // Journal-incomplete items have unknowable live provenance — never anchor-substitute,
            // never offer for styling; show the live pixels with an honest reason.
            let provenance_unknown = unknown_provenance.contains(item.id.as_str());
            let original = if provenance_unknown || unreadable {
                None
            } else {
                anchors
                    .get(item.id.as_str())
                    .filter(|(last_applied, _)| last_applied == &fingerprint)
                    .map(|(_, anchor)| anchor)
            };
            // Past the per-scan source budget → serve this item without a preview (codex R2 B-5). NOT
            // silent: log once so a truncated scan is visible, never mistaken for full coverage.
            if source_bytes >= SCAN_SOURCE_BUDGET && !budget_logged {
                log::warn!(
                    "icons scan: reached the {}-byte source-preview budget; remaining icons served without a preview",
                    SCAN_SOURCE_BUDGET
                );
                budget_logged = true;
            }
            let (urls, extract_err) = if source_bytes >= SCAN_SOURCE_BUDGET {
                (Vec::new(), Some("图标过多：已达本次扫描的预览内存预算，部分图标暂无预览".to_string()))
            } else {
                match self.extractor.extract(&item, original) {
                    Ok(sources) => {
                        let mut urls = Vec::with_capacity(sources.len());
                        // Consume the extracted sources: each PNG buffer MOVES into the cache instead of
                        // being cloned (codex R2 B-12) — `extract` already handed us owned `DecodedImage`s
                        // and the Vec is dropped right after, so a large desktop pays one fewer full copy
                        // per source. Each unique source's bytes count against the per-scan budget.
                        for (slot, src) in sources.into_iter().enumerate() {
                            let (url, added) = cache_source_into(
                                &mut next_sources,
                                item.id.as_str(),
                                slot as u32,
                                src,
                            );
                            source_bytes += added;
                            urls.push(url);
                        }
                        (urls, None)
                    }
                    Err(e) => (Vec::new(), Some(format!("图标读取失败：{e}"))),
                }
            };
            let degraded_reason = if provenance_unknown {
                Some("待修复：上次操作未完成，刷新后重试".to_string())
            } else if unreadable {
                Some("图标状态读取失败".to_string())
            } else {
                extract_err
            };
            let (x, y) = live_slots.get(&item.name).copied().unwrap_or_else(|| synthetic_layout(i));
            dtos.push(IconItemDto {
                id: item.id.as_str().into(),
                label: item.name.clone(),
                kind: map_kind(item.kind),
                is_shortcut: item.kind.is_shortcut(),
                styleable: item.can_style() && degraded_reason.is_none(),
                status_reason: degraded_reason.clone().or_else(|| item.status_message.clone()),
                x,
                y,
                source_urls: urls,
            });
            // `source_ok` is the ONE apply-authority bit shared with the commit path (codex
            // icons2-🟠5): the DTO's styleable, the commit's acceptance, and the restore planner
            // all derive from it instead of three drifting definitions.
            scanned.push(ScannedItem { item, fingerprint, source_ok: degraded_reason.is_none() });
        }
        // Every extract succeeded → publish atomically, ALL inside one critical section ordered
        // acceptance-check → source cache → snapshot (codex R10-#B + R11-#1). The revision check runs
        // FIRST: this scan allocated its revision BEFORE the (slow) extraction and does not hold the
        // op gate, so a heal FENCE (or a competing scan) may have advanced `scan_revision` past it. A
        // superseded scan must lose WITHOUT side effects — publishing its cache before erroring could
        // evict URLs the live generation still serves (the failed-refresh contract promises "当前画面
        // 保持不变"). Lock order st → sources appears only here and nothing locks sources → st, so no
        // cycle.
        {
            let mut st = self.mut_state.lock().unwrap();
            if rev <= st.scan_revision {
                return Err(format!(
                    "scan superseded (revision {rev} <= current {}): rescan",
                    st.scan_revision
                ));
            }
            // Epoch fence (codex icons2-🔴2): a desktop mutation (apply-commit / restore /
            // overlay) that landed between the anchor snapshot and here means the extraction ran
            // against a MIXED generation — its output could publish just-styled pixels as "the
            // raw source". Lose without side effects; the caller rescans against the new epoch.
            if st.op_epoch != epoch_at_snapshot {
                return Err(format!(
                    "scan raced a desktop mutation (epoch {} -> {}): rescan",
                    epoch_at_snapshot, st.op_epoch
                ));
            }
            self.sources.lock().unwrap().publish(next_sources);
            st.scan = scanned;
            st.scan_revision = rev;
            // A REAL scan (even of a genuinely empty desktop) is a valid apply target (codex R11-#2).
            st.scan_valid = true;
        }
        // Observed desktop metrics (the frontend assembles its grid from these, never fabricated
        // dims). [WINDOWS-VERIFY] real SM_C*SCREEN + SPI_GETWORKAREA; the dev host reports a
        // plausible 1080p work area matching `synthetic_layout`; an unreadable platform degrades
        // to the same shape rather than failing the scan.
        let grid = self
            .geometry
            .geometry()
            .map(|g| GridMetricsDto {
                screen_width: g.screen_width,
                screen_height: g.screen_height,
                taskbar_height: g.taskbar_height,
            })
            .unwrap_or(GridMetricsDto { screen_width: 1920, screen_height: 1080, taskbar_height: 48 });
        Ok(IconScanDto { revision: rev, items: dtos, grid })
    }

    /// `icons.getPersisted`: the ②③ + native bits the frontend overlays onto its assembled state.
    /// `read_state` folds the repair-pending signal into `applied` (codex R6-#6), so a styled desktop
    /// a degraded recovery left un-ledgered keeps its restore affordance reachable here AND on every
    /// apply/reset op-result — the signal lives in ONE place, the shared `read_state`.
    pub fn get_persisted(&self) -> Result<IconPersistedDto, String> {
        let st = self.mut_state.lock().unwrap();
        let stores = self
            .ops()
            .read_state(&st.history, &st.ledger, &st.journal)
            .map_err(|e| e.to_string())?;
        drop(st);
        Ok(self.finish_persisted(self.to_persisted_dto_locked(&stores)))
    }
}
