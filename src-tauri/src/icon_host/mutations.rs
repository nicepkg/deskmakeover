//! The desktop-mutating verbs: applyBaked begin/chunk/commit, restore, switchVersion, restoreOverlay.

use std::sync::atomic::Ordering;

use dm_contracts::{ArrowOverlayDto, IconChunkItemDto, IconOpResultDto, SettingsPatch, ToastDto};
use dm_domain::OverlayOutcome;
use dm_operations::icons::{
    MAX_APPLY_MASTERS, MAX_LABEL_BYTES, MAX_MASTER_B64_BYTES, MAX_SESSION_B64_BYTES,
    MAX_STYLE_JSON_BYTES,
};
use dm_operations::{IconApplySession, LedgerStore as _};

use super::dto::parse_style;
use super::export::now_secs;
use super::{IconHost, IconMutState};

impl IconHost {
    /// `icons.applyBakedBegin`: open a chunk-buffer session for scan `revision`, returning a fresh
    /// session token. Rejects a stale apply whose revision no longer matches the current scan (codex
    /// Block 2), and captures the op-epoch so the commit can detect an intervening mutation.
    pub fn apply_baked_begin(&self, revision: u32, count: u32) -> Result<String, String> {
        let mut st = self.mut_state.lock().unwrap();
        if revision != st.scan_revision {
            return Err(format!(
                "stale apply: begin revision {revision} does not match the current scan {}",
                st.scan_revision
            ));
        }
        // No valid scan snapshot: either nothing has been scanned yet, or a heal FENCED the previous
        // one (the fenced revision is synthetic — codex R10-#B). An apply must bind a REAL,
        // still-valid snapshot's fingerprints; rescan first. Validity is an EXPLICIT flag, not an
        // emptiness test: a genuinely empty desktop scan is valid, and its zero-target policy-only
        // Apply must go through (codex R11-#2).
        if !st.scan_valid {
            return Err("no valid scan to apply against: rescan first".into());
        }
        // Cap the untrusted `count` (audit F4): a session can never buffer more masters than the live
        // scan can produce (≤2 per item — primary + paired empty) nor more than the absolute ceiling,
        // so a hostile count can't force a huge up-front allocation before any master arrives.
        let max_masters = st.scan.len().saturating_mul(2).min(MAX_APPLY_MASTERS);
        if count as usize > max_masters {
            return Err(format!(
                "apply count {count} exceeds the {max_masters} masters the current scan can produce"
            ));
        }
        // A fresh Begin ABANDONS any prior in-flight session (a bake that errored mid-stream and never
        // committed must not strand the session and deadlock every future apply). Mixing is prevented
        // by the SESSION TOKEN: the new session gets a new monotonic id, so any still-in-flight Chunk/
        // Commit carrying the OLD token is rejected — masters never cross applies (codex R3-Block 1).
        if st.session.is_some() {
            log::warn!("icons.applyBakedBegin abandoned a prior uncommitted apply session");
        }
        st.session_id += 1;
        st.session = Some(IconApplySession::begin(revision, count as usize));
        st.session_epoch = st.op_epoch;
        st.session_scan = st.scan.clone();
        Ok(st.session_id.to_string())
    }

    /// `icons.applyBakedChunk`: buffer a batch of baked masters into the open session, validating the
    /// session token so a stale/foreign chunk can never land in the wrong buffer (codex R3-Block 1).
    pub fn apply_baked_chunk(&self, session_id: &str, items: Vec<IconChunkItemDto>) -> Result<(), String> {
        let mut st = self.mut_state.lock().unwrap();
        if session_id != st.session_id.to_string() {
            return Err("apply session token mismatch (a newer apply superseded this one)".into());
        }
        let session = st
            .session
            .as_mut()
            .ok_or("no apply session; call applyBakedBegin first")?;
        for it in items {
            // Cap the untrusted chunk stream (audit F4): reject an oversized master, and refuse to
            // grow the session past its master-count / cumulative-byte ceilings — a hostile chunk
            // loop can't OOM the process before the commit validates.
            if it.master_png.len() > MAX_MASTER_B64_BYTES {
                return Err(format!(
                    "baked master {:?} is {} bytes, over the {}-byte per-master limit",
                    it.id,
                    it.master_png.len(),
                    MAX_MASTER_B64_BYTES
                ));
            }
            if session.len() >= MAX_APPLY_MASTERS {
                return Err(format!("apply session exceeded the {MAX_APPLY_MASTERS}-master limit"));
            }
            if session.bytes().saturating_add(it.master_png.len()) > MAX_SESSION_B64_BYTES {
                return Err(format!(
                    "apply session exceeded its {MAX_SESSION_B64_BYTES}-byte cumulative budget"
                ));
            }
            session.push(it.id, it.source_index, it.master_png);
        }
        Ok(())
    }

    /// `icons.applyBakedCommit`: package + apply the buffered masters, persist ②③, install the
    /// arrow overlay. Serialized under the mut lock (the apply/GC lifecycle-lock).
    pub fn apply_baked_commit(
        &self,
        session_id: &str,
        style_json: String,
        restore_ids: Vec<String>,
        label: Option<String>,
    ) -> Result<IconOpResultDto, String> {
        // Cap untrusted commit inputs before any work (audit F4 / codex B2-🔴): reject a huge
        // styleJson / restore-id list / label, and (below) validate the session token, BEFORE
        // parsing the style into a second JSON value.
        if style_json.len() > MAX_STYLE_JSON_BYTES {
            return Err(format!(
                "styleJson is {} bytes, over the {MAX_STYLE_JSON_BYTES}-byte limit",
                style_json.len()
            ));
        }
        if restore_ids.len() > MAX_APPLY_MASTERS {
            return Err(format!(
                "restore-id list of {} exceeds the {MAX_APPLY_MASTERS}-item limit",
                restore_ids.len()
            ));
        }
        if label.as_deref().map_or(0, str::len) > MAX_LABEL_BYTES {
            return Err(format!("label exceeds the {MAX_LABEL_BYTES}-byte limit"));
        }
        // Hold the mutation gate for the WHOLE verb — the ledger commit AND the overlay install +
        // set_arrow below (which run after `mut_state` is dropped) — so no concurrent restore /
        // arrow-restore can interleave its overlay helper with this one (codex R3-Block 2).
        let _op_gate = self.op_gate.lock().unwrap();
        let mut st = self.mut_state.lock().unwrap();
        // Reject a commit whose token no longer matches the current session — a newer Begin
        // superseded it, so its buffer/styleJson belong to a stale apply (codex R3-Block 1).
        if session_id != st.session_id.to_string() {
            let stores = self.ops().read_state(&st.history, &st.ledger, &st.journal).map_err(|e| e.to_string())?;
            let dto = self.to_persisted_dto_locked(&stores);
            drop(st);
            return Ok(IconOpResultDto {
                ok: false,
                toast: Some(ToastDto { key: "Toast_ApplySuperseded".into(), arg: None }),
                persisted: self.finish_persisted(dto),
            });
        }
        // Parse the style AFTER the token check — a superseded commit returns above without paying
        // for (or trusting) a large recipe (codex B2-🔴).
        let style = parse_style(&style_json)?;
        let session = st.session.take().ok_or("no apply session to commit")?;
        // Reject a malformed buffer (short/over) — a stale scan or a dropped chunk (codex Block 2).
        if session.len() != session.expected() {
            return Err(format!(
                "incomplete apply buffer: {} masters of {} promised",
                session.len(),
                session.expected()
            ));
        }
        // Reject a SUPERSEDED apply: ANY mutation (Restore, another Apply, or a restoreOverlay)
        // landed during this apply's bake, so committing now would write a stale look OVER the
        // user's newer intent on the real desktop (codex Block 3 / R2-Block 3). Fail closed WITHOUT
        // mutating — the store keeps the draft dirty + rescans.
        if st.op_epoch != st.session_epoch {
            let stores = self.ops().read_state(&st.history, &st.ledger, &st.journal).map_err(|e| e.to_string())?;
            let dto = self.to_persisted_dto_locked(&stores);
            drop(st);
            return Ok(IconOpResultDto {
                ok: false,
                toast: Some(ToastDto { key: "Toast_ApplySuperseded".into(), arg: None }),
                persisted: self.finish_persisted(dto),
            });
        }
        // Resolve against the session's OWN scan snapshot (bound at Begin), never the live `scan`,
        // so an intervening rescan cannot swap the CAS anchors (codex R2-Block 1). Split the guard
        // into disjoint field borrows so the ops call can hold &mut of several at once.
        let IconMutState { ledger, journal, history, txn, session_scan, op_epoch, .. } = &mut *st;
        let created_at = now_secs();
        // The id must be GLOBALLY unique: the txn counter alone resets when the journal
        // checkpoints across restarts, which minted three different looks as `look-1` — every
        // id-keyed lookup (UI selection, switch_to_version) then resolved to the OLDEST match,
        // i.e. "the second apply wore the first apply's icons" (owner box 2026-07-17). The
        // timestamp+counter pair cannot repeat across restarts.
        let look_id = format!("look-{}-{}", created_at, txn.peek());
        let outcome = self
            .ops()
            .commit_apply(session, style, label, look_id, created_at, session_scan, &restore_ids, &self.scope_roots, self.elevated(), txn, journal, ledger, history)
            .map_err(|e| e.to_string())?;
        // This apply mutated the desktop → bump the epoch so any concurrent stale apply rejects.
        *op_epoch += 1;
        // Per-item shell notifications (below) need each touched item's (path, is-folder) — capture
        // from the session scan BEFORE it is cleared. A folder's custom icon only refreshes on a
        // directory-scoped SHCNE_UPDATEDIR (owner 2026-07-17: the folder lagged one apply behind).
        // ROLLED-BACK items are included too: the desktop was written then restored, so Explorer may
        // have cached the transient styled icon and the global refresh does not clear a folder's
        // (codex 2026-07-17 P2).
        let touched_paths: Vec<(String, bool)> = outcome
            .committed
            .iter()
            .chain(outcome.reverted.iter())
            .chain(outcome.rolled_back.iter())
            .filter_map(|id| session_scan.iter().find(|s| &s.item.id == id))
            .map(|s| (s.item.path.clone(), s.item.kind == dm_domain::ItemKind::Folder))
            .collect();
        session_scan.clear();
        // FENCE the scan revision when the ops demand it (codex R9-#1): a stale poison row was healed
        // (dropped) this round — or a driver bare-Err left the heal set unknown — so a same-revision
        // retry would find no ledger row and pass the ordinary fresh CAS, silently overwriting what
        // could be the user's manual restore-to-original (the ABA). Advancing `scan_revision` off the
        // shared counter makes every applyBakedBegin carrying the old revision fail "stale apply"
        // until a REAL rescan publishes fresh fingerprints; the frontend's rescan-after-conflict UX is
        // a follow-up, but this fence is the structural safety boundary, not the toast.
        if outcome.requires_rescan {
            st.scan_revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
            // The fenced revision is synthetic — it corresponds to NO real scan. Mark the snapshot
            // INVALID (an explicit flag, not an emptiness sentinel — codex R11-#2) so even a Begin
            // somehow carrying the fenced number cannot bind pre-heal fingerprints; clear the stale
            // items too. Only a REAL rescan (which republishes both) reopens the gate (codex R10-#B).
            st.scan_valid = false;
            st.scan.clear();
        }
        let committed_any = !outcome.committed.is_empty();
        let dto = self.to_persisted_dto_locked(&outcome.stores);
        drop(st);

        // A global apply that styled at least one icon installs the machine-wide transparent overlay
        // (native arrow hidden, ADR-0021), pointing the elevated helper at a real transparent ICO
        // (the helper rejects an empty path — codex Block 5). The elevated verb is the host's; on the
        // dev host it succeeds. A pure revert-only apply leaves the overlay state untouched.
        // Fold the machine-wide overlay finalize into the result (codex R2 B-2/B-3): a styled apply must
        // hide the native arrow, and any way that fails — the helper Declined (UAC cancel) / Failed /
        // errored, OR the Applied state could not be PERSISTED (lost `Hidden` marker → restart residue)
        // — leaves the desktop styled but the arrow still native (visually doubling the baked mark), so
        // the op is NOT a clean success. Mirrors the full-restore path's `overlay_failed` handling.
        let mut overlay_incomplete = false;
        if committed_any {
            // Skip the elevated overlay verb — and its UAC prompt — when the machine already wears
            // this exact overlay (owner 2026-07-17: every apply asked for TWO authorizations; the
            // second was this unconditional reinstall). The arrow state must also already be
            // persisted as Hidden, or the marker alone could mask a lost arrow record.
            let overlay_sig = format!("hidden:{}", self.overlay_ico_sha);
            let already_installed = self.overlay_install_is(&overlay_sig)
                && *self.arrow_overlay.lock().unwrap() == ArrowOverlayDto::Hidden;
            if !already_installed {
                match self
                    .overlay
                    .apply(dm_domain::OverlayStyle::Transparent, &self.overlay_ico.to_string_lossy())
                {
                    Ok(OverlayOutcome::Applied) => {
                        if let Err(e) = self.set_arrow(ArrowOverlayDto::Hidden) {
                            overlay_incomplete = true;
                            log::warn!("icons apply: arrow overlay installed but its state was not persisted: {e}");
                        } else if self.overlay_install_changed(&overlay_sig) {
                            // The effective overlay CHANGED (a native→hidden transition, or the overlay asset
                            // content changed across an app update) — Explorer caches Shell Icons\29 at
                            // startup, so reload it or the ugly native arrow / a stale overlay keeps showing
                            // (owner report 2026-07-16). A repeat apply of the same overlay does NOT flicker.
                            self.refresh_shell_icon_overlay();
                        }
                    }
                    other => {
                        overlay_incomplete = true;
                        log::warn!("icons apply: arrow overlay not installed ({other:?}) — native arrow remains");
                    }
                }
            }
            let _ = self.refresher.notify_icons_changed();
        }
        // Item-scoped refresh for every icon this apply actually touched (styled OR reverted): a
        // folder needs SHCNE_UPDATEDIR or Explorer keeps showing its cached desktop.ini icon.
        for (path, is_dir) in &touched_paths {
            let _ = self.refresher.notify_item_changed(path, *is_dir);
        }
        // A finalize step failed AFTER the desktop committed (codex R3-Block 4): log the detail for
        // the operator, surface a generic "applied but finalize incomplete" toast, and return ok:false
        // with the authoritative persisted state — the store then keeps the draft dirty for a retry,
        // never a bare bridge error that reads as "nothing changed".
        if let Some(reason) = &outcome.degraded {
            log::warn!("icons apply finalize degraded: {reason}");
        }
        // The batch error must reach the LOG, not just the toast — the 2026-07-16 mid-batch
        // failure was undiagnosable server-side because the reason lived only in the frontend.
        if let Some(e) = &outcome.error {
            log::warn!(
                "icons apply batch error ({} committed, {} conflicts, mutated={}): {e}",
                outcome.committed.len(),
                outcome.conflicts.len(),
                outcome.desktop_mutated
            );
        }
        let (ok, toast) = if let Some(e) = &outcome.error {
            if outcome.reverted.is_empty() && !outcome.desktop_mutated {
                // The styling batch failed BEFORE touching the desktop AND no keep-revert landed →
                // truly nothing changed (a preflight/CAS failure). Only here is "桌面没有改动" honest.
                (false, Some(ToastDto { key: "Toast_ApplyFailed".into(), arg: Some(e.clone()) }))
            } else {
                // The batch rolled back / abandoned AFTER moving the desktop, or keep-reverts already
                // changed it — NOT "nothing changed" (codex R4-Block 1 + R5-#1): partial → ok:false +
                // repair toast + the real state, so the UI never claims the desktop is untouched.
                (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
            }
        } else if outcome.degraded.is_some() {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else if !outcome.intent_persisted {
            // The ops did NOT persist this Apply's intent — with error/degraded already handled above,
            // this is exactly the zero-effect-WITH-conflicts case (all-conflicts, or a restore-only
            // batch whose every opt-out was a hand-edit): nothing landed, something was refused, ②③
            // untouched. Report a no-effect + keep the draft dirty (codex R8-#2/#3, R9-#2). A
            // conflict-free zero-target (policy-only) Apply has `intent_persisted == true` — its ②③
            // WAS written — and correctly falls through to a clean success.
            (false, Some(ToastDto { key: "Toast_ApplyNoEffect".into(), arg: None }))
        } else if !outcome.conflicts.is_empty() {
            // A PARTIAL success: some icons styled/reverted, others conflicted (changed under the user
            // since the scan) or were left as a trust-first hand-edit skip. ②③ WAS written (a real
            // effect landed). Surface the skipped count so the user knows to rescan + retry them — spec
            // 01 requires skipped items be visible, never silently swallowed (codex R8-#3).
            (
                true,
                Some(ToastDto {
                    key: "Toast_ApplySkipped".into(),
                    arg: Some(outcome.conflicts.len().to_string()),
                }),
            )
        } else {
            (true, None)
        };
        // Downgrade an otherwise-clean success when the arrow-overlay finalize did not fully land
        // (codex R2 B-2/B-3): keep the draft dirty for a retry + a degraded toast. A more-severe txn
        // toast set above already wins.
        let (ok, toast) = if ok && overlay_incomplete {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else {
            (ok, toast)
        };
        Ok(IconOpResultDto { ok, toast, persisted: self.finish_persisted(dto) })
    }

    /// `icons.restore`: full reset — revert every styled icon to its true original (trust-first,
    /// spec 07 §10) AND lift the arrow overlay (icons + arrow back to native).
    pub fn restore(&self) -> Result<IconOpResultDto, String> {
        // Hold the mutation gate across the ledger reset AND the arrow lift below (codex R3-Block 2).
        let _op_gate = self.op_gate.lock().unwrap();
        let mut st = self.mut_state.lock().unwrap();
        let IconMutState { ledger, history, journal, op_epoch, .. } = &mut *st;
        // Capture each styled row's (id, path, is-folder) BEFORE the reset drops the rows — the
        // per-item shell notifications below need the paths (a reverted folder only refreshes on a
        // directory-scoped SHCNE_UPDATEDIR, owner 2026-07-17).
        let row_paths: Vec<(dm_domain::ItemId, String, bool)> = ledger
            .all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|e| {
                let is_dir = e.target.kind == dm_domain::ItemKind::Folder;
                (e.item, e.target.path, is_dir)
            })
            .collect();
        let outcome = self
            .ops()
            // §14 privileged-scope roots + the elevated applier — a Public Desktop / ProgramData row
            // still wearing our style reverts through the elevated helper (one UAC); an unwired host
            // (`None` / `Unresolved` scope) leaves it as an honest skip (fail closed).
            .reset_to_original(&self.scope_roots, self.elevated(), journal, ledger, history)
            .map_err(|e| e.to_string())?;
        // A reset is a mutation → bump the epoch so a concurrent in-flight apply rejects.
        *op_epoch += 1;
        // FENCE the scan (mirrors switch_version + the apply's requires_rescan path). The reset
        // reverted every icon to its original, so the cached scan — which still holds the PRE-reset
        // STYLED fingerprints — is now stale. Without this fence a following apply binds those stale
        // fingerprints as its CAS anchors, every one mismatches the now-original desktop, and the
        // whole apply reports "nothing styled" (owner 2026-07-17: reset → pick a preset → apply → 0
        // succeeded). Invalidating the scan forces a fresh rescan (original fingerprints) first.
        st.scan_revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        st.scan_valid = false;
        st.scan.clear();
        let restored_paths: Vec<(String, bool)> = row_paths
            .into_iter()
            .filter(|(id, ..)| outcome.restored.contains(id))
            .map(|(_, path, is_dir)| (path, is_dir))
            .collect();
        let dto = self.to_persisted_dto_locked(&outcome.stores);
        let skipped = outcome.skipped.len();
        let degraded = outcome.degraded;
        // The ops DEFERRED the ledger reset because up-front recovery had to heal a prior crash first
        // (codex R6-#4). The reset did NOT run, so its finalizers (auto-format off + arrow lift) MUST
        // be skipped — running them would leave a partial state (arrow native + resident off, yet icons
        // still styled). The user re-syncs from the returned state and retries.
        let deferred = outcome.deferred;
        drop(st);

        let mut overlay_failed = false;
        let mut autoformat_off_failed = false;
        if !deferred {
            // §10 three-part coupling (spec 07 §8.4): clearing ② (done in the ops) is paired with
            // turning auto-format OFF so the resident stays dormant after a reset. A write fault here is
            // NOT swallowed (codex R7-#4): a reset reporting ok:true while `keep_new_icons_styled` is
            // still true would leave the resident re-styling new icons after a "restore to original".
            if self
                .settings
                .set(&SettingsPatch { keep_new_icons_styled: Some(false), ..Default::default() })
                .is_err()
            {
                autoformat_off_failed = true;
            }
            // Lift the overlay if it was installed. A helper FAILURE is surfaced (codex R2-Block 3): the
            // icons reverted but the machine-wide arrow is still hidden, so the op is NOT a clean success.
            if *self.arrow_overlay.lock().unwrap() == ArrowOverlayDto::Hidden {
                match self.overlay.restore() {
                    Ok(OverlayOutcome::Applied) => {
                        if let Err(e) = self.set_arrow(ArrowOverlayDto::Native) {
                            // Fail-safe: the arrow IS native on the machine; a lost marker only costs an
                            // extra idempotent restore next launch (codex R2 B-3), not a reset failure.
                            log::warn!("icons reset: arrow restored but its state was not persisted: {e}");
                        } else if self.overlay_install_changed("native") {
                            // hidden→native transition: reload Explorer so the arrow actually comes back.
                            self.refresh_shell_icon_overlay();
                        }
                    }
                    _ => overlay_failed = true,
                }
            }
        }
        let _ = self.refresher.notify_icons_changed();
        // Item-scoped refresh for every reverted icon — a folder's restored (default) icon only
        // shows once Explorer re-reads the directory (SHCNE_UPDATEDIR).
        for (path, is_dir) in &restored_paths {
            let _ = self.refresher.notify_item_changed(path, *is_dir);
        }
        // A finalize step failed after some icons already reverted (codex R3-Block 4): log the detail,
        // return ok:false + a repair toast + the authoritative state. Surface the trust-first skips, or
        // the arrow-restore failure — never a blanket ok:true. Priority: arrow fault → finalize
        // degraded (incl. an auto-format-off write fault) → trust-first skips.
        if let Some(reason) = &degraded {
            log::warn!("icons reset finalize degraded: {reason}");
        }
        let (ok, toast) = if overlay_failed {
            (false, Some(ToastDto { key: "Toast_RestoreArrowFailed".into(), arg: None }))
        } else if degraded.is_some() || autoformat_off_failed {
            (false, Some(ToastDto { key: "Toast_ResetDegraded".into(), arg: None }))
        } else if skipped > 0 {
            (true, Some(ToastDto { key: "Toast_ResetSkipped".into(), arg: Some(skipped.to_string()) }))
        } else {
            (true, None)
        };
        Ok(IconOpResultDto { ok, toast, persisted: self.finish_persisted(dto) })
    }

    /// `icons.switchVersion`: switch the desktop to a saved appearance version (spec 07 §9). Reads
    /// the ③ entry, promotes its recipe to ②, and projects it onto the LIVE scan through the same
    /// resolve→bake→driver path auto-format uses. CAS-safe (a hand-edited icon is skipped), fenced
    /// (the scan revision + op-epoch bump so an in-flight apply built on the old desktop rejects).
    pub fn switch_version(&self, version_id: &str) -> Result<IconOpResultDto, String> {
        use dm_operations::icons::version_switch::{switch_to_version, VersionSwitchPorts};
        // Serialize the whole desktop-mutating verb (same discipline as apply-commit / restore).
        let _op_gate = self.op_gate.lock().unwrap();
        let mut st = self.mut_state.lock().unwrap();
        let ports = VersionSwitchPorts {
            scanner: &*self.scanner,
            extractor: &*self.extractor,
            reader: &*self.reader,
            applier: &*self.applier,
            assets: &self.assets,
            // §14 privileged-scope roots — `Unresolved` on an unwired Windows host fails CLOSED here.
            scope: &self.scope_roots,
        };
        let IconMutState { ledger, journal, history, txn, op_epoch, output_cache, .. } = &mut *st;
        let outcome = switch_to_version(
            version_id, &ports, &self.settings, history, txn, journal, ledger, Some(output_cache),
        )
        .map_err(|e| e.to_string())?;
        // A switch is a desktop mutation: bump the epoch (an in-flight apply rejects) and FENCE the
        // scan (the CAS anchors the old snapshot holds are stale) so the next apply must rescan.
        *op_epoch += 1;
        st.scan_revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        st.scan_valid = false;
        st.scan.clear();
        let stores = self
            .ops()
            .read_state(&st.history, &st.ledger, &st.journal)
            .map_err(|e| e.to_string())?;
        let dto = self.to_persisted_dto_locked(&stores);
        drop(st);
        let _ = self.refresher.notify_icons_changed();

        // A version switch that committed styled icons must hide the native arrow exactly like a fresh
        // apply (codex R2 B-7): without this the switch leaves `arrowOverlay: native` while the icons are
        // styled — the browser mock hides it, so browser testing mispredicts the Windows state. Fold any
        // failure (declined/failed/errored, or a lost `Hidden` marker) into the result. The native switch
        // caller is currently unwired [T8]; this keeps the host honest for when it lands.
        let mut overlay_incomplete = false;
        if !outcome.outcome.committed.is_empty() {
            match self
                .overlay
                .apply(dm_domain::OverlayStyle::Transparent, &self.overlay_ico.to_string_lossy())
            {
                Ok(OverlayOutcome::Applied) => {
                    if let Err(e) = self.set_arrow(ArrowOverlayDto::Hidden) {
                        overlay_incomplete = true;
                        log::warn!("icons switchVersion: arrow overlay installed but its state was not persisted: {e}");
                    }
                }
                other => {
                    overlay_incomplete = true;
                    log::warn!("icons switchVersion: arrow overlay not installed ({other:?}) — native arrow remains");
                }
            }
        }

        let (ok, toast) = if outcome.deferred {
            // A prior crash's recovery ran; the switch stood down BEFORE ② was promoted, so
            // nothing changed. The UI re-syncs + retries — honest, never a phantom success.
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else if outcome.outcome.error.is_some() {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else if !outcome.outcome.conflicts.is_empty() && outcome.outcome.committed.is_empty() {
            (true, Some(ToastDto {
                key: "Toast_ApplySkipped".into(),
                arg: Some(outcome.outcome.conflicts.len().to_string()),
            }))
        } else {
            (true, None)
        };
        // Downgrade a clean switch whose arrow finalize did not land (codex R2 B-7), as apply does.
        let (ok, toast) = if ok && overlay_incomplete {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else {
            (ok, toast)
        };
        Ok(IconOpResultDto { ok, toast, persisted: self.finish_persisted(dto) })
    }

    /// `icons.restoreOverlay`: keep-beautification restore — lift ONLY the arrow overlay (the icon
    /// look stays). Faithful to the elevated Applied|Declined|Failed contract; the OBSERVED
    /// post-op arrow state is authoritative.
    pub fn restore_overlay(&self) -> Result<IconOpResultDto, String> {
        // Hold the mutation gate across the overlay helper + epoch bump + set_arrow so a concurrent
        // apply-commit / full-restore can never interleave its own overlay call (codex R3-Block 2).
        let _op_gate = self.op_gate.lock().unwrap();
        // Read the authoritative ②③ persisted state BEFORE the machine-level overlay mutation, so a
        // ledger/settings I/O fault fails the op with the desktop UNCHANGED — never a bare Err AFTER
        // the arrow already flipped (codex R4-Block 4). This op only touches the arrow overlay, so
        // the ②③ half of the snapshot is still exact post-op; we overwrite just the arrow field.
        let mut persisted = self.get_persisted()?;
        let outcome = self.overlay.restore().map_err(|e| e.to_string())?;
        let (arrow, ok, toast_key) = match outcome {
            OverlayOutcome::Applied => (ArrowOverlayDto::Native, true, "Toast_ArrowRestored"),
            // Declined/Failed leave the arrow hidden so the affordance stays for a retry.
            OverlayOutcome::Declined => (ArrowOverlayDto::Hidden, false, "Toast_ArrowRestoreDeclined"),
            OverlayOutcome::Failed => (ArrowOverlayDto::Hidden, false, "Toast_RestoreArrowFailed"),
        };
        // Bump the op-epoch ONLY when the arrow actually flipped to native (a real machine-wide
        // mutation): an in-flight apply that began before it then rejects rather than re-hiding the
        // arrow the user just lifted (codex R2-Block 3). A Declined/Failed changed nothing, so it must
        // NOT invalidate an in-flight apply.
        if outcome == OverlayOutcome::Applied {
            self.mut_state.lock().unwrap().op_epoch += 1;
        }
        if let Err(e) = self.set_arrow(arrow) {
            // Fail-safe direction (Applied→Native, or Declined/Failed leaving the already-Hidden marker):
            // a lost marker at worst re-runs an idempotent restore next launch (codex R2 B-3).
            log::warn!("restore_overlay: arrow state not persisted: {e}");
        }
        persisted.arrow_overlay = arrow; // the one field this op mutated; ②③ carried from the pre-read
        Ok(IconOpResultDto {
            ok,
            toast: Some(ToastDto { key: toast_key.into(), arg: None }),
            persisted,
        })
    }
}
