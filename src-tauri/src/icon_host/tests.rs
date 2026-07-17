//! Icon host behavioural tests (dev-host ports).

    use super::*;
    use super::export::utc_stamp;
    use std::collections::HashMap;
    use dm_contracts::{IconChunkItemDto, IconKindDto, IconOpResultDto, IconScanDto};
    use dm_domain::{DecodedImage, OverlayOutcome};
    use crate::devhost_icons::{
        DevDesktopGeometry, DevDesktopScanner, DevExplorerRefresher, DevIconApplier,
        DevIconDesktop, DevIconReader, DevIconSourceExtractor, DevOverlayControl,
    };
    use serde_json::json;

    /// An overlay control that always DECLINES (models a cancelled UAC prompt), to drive the
    /// overlay-incomplete finalize path (codex R2 B-2).
    struct DeclinedOverlay;
    impl OverlayControl for DeclinedOverlay {
        fn apply(&self, _s: dm_domain::OverlayStyle, _ico: &str) -> dm_domain::PortResult<OverlayOutcome> {
            Ok(OverlayOutcome::Declined)
        }
        fn restore(&self) -> dm_domain::PortResult<OverlayOutcome> {
            Ok(OverlayOutcome::Declined)
        }
    }

    /// A big DISTINCT source per item (the bytes vary by item id so content addressing never dedups
    /// them), to push the per-scan source budget past its ceiling (codex R2 B-5).
    struct BigSourceExtractor {
        bytes_per_source: usize,
    }
    impl IconSourceExtractor for BigSourceExtractor {
        fn extract(
            &self,
            item: &dm_domain::DesktopItem,
            _original: Option<&dm_domain::RestoreAnchor>,
        ) -> dm_domain::PortResult<Vec<DecodedImage>> {
            let mut png = vec![0u8; self.bytes_per_source];
            for (i, b) in item.id.as_str().bytes().enumerate() {
                if i < png.len() {
                    png[i] = b;
                }
            }
            Ok(vec![DecodedImage { width: 256, height: 256, png }])
        }
    }

    fn host_with(
        dir: &std::path::Path,
        overlay: Arc<dyn OverlayControl + Send + Sync>,
        extractor: Option<Arc<dyn IconSourceExtractor + Send + Sync>>,
    ) -> (IconHost, Arc<DevIconDesktop>) {
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.join("settings.sqlite3")).unwrap());
        let extractor =
            extractor.unwrap_or_else(|| Arc::new(DevIconSourceExtractor(desk.clone())));
        let host = IconHost::new(
            IconHostPorts {
                scanner: Arc::new(DevDesktopScanner),
                extractor,
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk.clone())),
                overlay,
                refresher: Arc::new(DevExplorerRefresher),
                elevated: None,
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir,
            1,
            ScopeRoots::Unprivileged,
        );
        (host, desk)
    }

    fn host_with_overlay(
        dir: &std::path::Path,
        overlay: Arc<dyn OverlayControl + Send + Sync>,
    ) -> (IconHost, Arc<DevIconDesktop>) {
        host_with(dir, overlay, None)
    }

    fn host_with_desk(dir: &std::path::Path) -> (IconHost, Arc<DevIconDesktop>) {
        host_with(dir, Arc::new(DevOverlayControl), None)
    }

    #[test]
    fn a_scan_past_the_source_budget_serves_later_items_preview_less() {
        // codex R2 B-5: the source cache pins the whole live generation, so the scan must bound its own
        // preview bytes. With 8 dev items each yielding a DISTINCT ~4 MiB source (32 MiB > the 24 MiB
        // budget), the later items must be served WITHOUT source URLs (an honest bounded scan) rather
        // than pinning unbounded memory. The earlier items still get previews.
        let dir = tempfile::tempdir().unwrap();
        let big = Arc::new(BigSourceExtractor { bytes_per_source: 4 * 1024 * 1024 });
        let (h, _desk) = host_with(dir.path(), Arc::new(DevOverlayControl), Some(big));
        let scan = h.scan().unwrap();
        let with_preview = scan.items.iter().filter(|it| !it.source_urls.is_empty()).count();
        let without = scan.items.iter().filter(|it| it.source_urls.is_empty()).count();
        assert!(with_preview > 0, "the first items still get previews");
        assert!(without > 0, "items past the budget are served preview-less, not pinned unbounded");
        assert!(with_preview < scan.items.len(), "the budget bounded the previewed set");
    }

    fn host(dir: &std::path::Path) -> IconHost {
        host_with_desk(dir).0
    }

    fn style_json(seed: i64) -> String {
        json!({ "config": { "seed": seed }, "kindPolicy": {}, "typeOverrides": {} }).to_string()
    }

    /// Bakes an item by streaming its scanned sources back as chunk masters (a 1×1 stand-in PNG per
    /// slot), mirroring what the frontend does after a scan.
    fn tiny_master() -> String {
        use base64::Engine;
        use image::ImageEncoder;
        // The contract-required 256×256 master size (the host packages exactly this).
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([120, 90, 200, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    #[test]
    fn scan_serves_every_source_over_the_protocol() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        assert!(scan.revision >= 1 && !scan.items.is_empty());
        // Every advertised source URL resolves through the protocol handler.
        for item in &scan.items {
            for url in &item.source_urls {
                let key = url.split("localhost/").nth(1).unwrap().split('?').next().unwrap();
                assert!(h.png_for(key).is_some(), "protocol miss for {url}");
            }
        }
        // The Recycle Bin advertises two sources (primary + empty).
        let bin = scan.items.iter().find(|i| i.kind == IconKindDto::RecycleBin).unwrap();
        assert_eq!(bin.source_urls.len(), 2);
    }

    /// The `dmicon://` PNG a scan currently serves for an item's slot-0 source.
    fn served_png(h: &IconHost, scan: &IconScanDto, id: &str) -> Vec<u8> {
        let item = scan.items.iter().find(|i| i.id == id).unwrap();
        let key =
            item.source_urls[0].split("localhost/").nth(1).unwrap().split('?').next().unwrap();
        h.png_for(key).expect("protocol serves the advertised source")
    }

    #[test]
    fn a_rescan_of_an_owned_unmodified_item_serves_the_original_source_not_the_styled_output() {
        // codex extractor-review 🔴1: after an apply, the LIVE icon is our styled output. A naive
        // re-scan reads it back as "the source", so the next apply styles the styled image —
        // Style(Style(orig)) compounds forever. The ledger-aware scan must serve the ORIGINAL.
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan1 = h.scan().unwrap();
        let original_png = served_png(&h, &scan1, "edge");

        let sid = h.apply_baked_begin(scan1.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto {
            id: "edge".into(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        assert!(h.apply_baked_commit(&sid, style_json(1), vec![], None).unwrap().ok);

        // Owned + unmodified → the re-scan extracts from the ledger's original anchor.
        let scan2 = h.scan().unwrap();
        assert_eq!(
            served_png(&h, &scan2, "edge"),
            original_png,
            "the re-scan must serve the true original source, not the styled surface"
        );

        // An EXTERNAL hand-edit breaks ownership → the live (foreign) surface is the honest
        // source again; the anchor must NOT shadow the user's own change.
        let second = dir.path().join("second");
        std::fs::create_dir_all(&second).unwrap();
        let (h2, desk2) = host_with_desk(&second);
        let s1 = h2.scan().unwrap();
        let orig2 = served_png(&h2, &s1, "code");
        let sid2 = h2.apply_baked_begin(s1.revision, 1).unwrap();
        h2.apply_baked_chunk(&sid2, vec![IconChunkItemDto {
            id: "code".into(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        assert!(h2.apply_baked_commit(&sid2, style_json(2), vec![], None).unwrap().ok);
        desk2.force_foreign("code");
        let s2 = h2.scan().unwrap();
        assert_ne!(
            served_png(&h2, &s2, "code"),
            orig2,
            "a hand-edited surface no longer matches last_applied → live extraction wins"
        );
    }

    #[test]
    fn export_compare_saves_a_validated_png_and_toasts_the_path() {
        use base64::Engine;
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let out = dir.path().join("pictures");

        // A real PNG payload saves + toasts its path.
        let res = h.export_compare(&tiny_master(), Some(out.clone())).unwrap();
        assert!(res.ok);
        let arg = res.toast.as_ref().unwrap().arg.clone().unwrap();
        assert!(arg.contains("DeskMakeover-"), "toast carries the saved path: {arg}");
        let saved = std::path::Path::new(&arg);
        assert!(saved.exists(), "the artifact is on disk");

        // A second export in the same second gets a suffixed name, never a clobber.
        let res2 = h.export_compare(&tiny_master(), Some(out.clone())).unwrap();
        let arg2 = res2.toast.as_ref().unwrap().arg.clone().unwrap();
        assert_ne!(arg, arg2, "same-second exports never overwrite");

        // A non-PNG payload (real GIF magic, so it is a plausible image but not our format) must
        // be REJECTED by the magic-byte gate before decode (codex icons2-🟠8: PNG-only, so a
        // non-PNG never lands under our `.png` name).
        let gif = base64::engine::general_purpose::STANDARD.encode(b"GIF89a\x01\x00\x01\x00");
        // Garbage / oversize / wrong-format payloads never land on disk — honest ok:false.
        let oversize = "A".repeat(12_000_001);
        for bad in [
            "not-base64!!!",
            &base64::engine::general_purpose::STANDARD.encode(b"nonsense"),
            &gif,
            &oversize,
        ] {
            let res = h.export_compare(bad, Some(out.clone())).unwrap();
            assert!(!res.ok, "rejected payload stays off disk");
            assert_eq!(res.toast.as_ref().unwrap().key, "Toast_CompareFailed");
        }
        assert_eq!(
            std::fs::read_dir(&out).unwrap().count(),
            2,
            "only the two valid exports exist"
        );
    }

    #[test]
    fn utc_stamps_follow_the_civil_calendar() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        // 2026-07-13 00:00:00 UTC = 1783900800.
        assert_eq!(utc_stamp(1_783_900_800), "20260713-000000");
        // Leap-year day: 2024-02-29 12:34:56 UTC = 1709210096.
        assert_eq!(utc_stamp(1_709_210_096), "20240229-123456");
    }

    /// codex icons2-🔴1: a TxnCommitted whose ledger upsert faulted is desktop truth the ledger
    /// hasn't caught up to. The scan must overlay the JOURNAL's Prepared anchor + Applied
    /// fingerprint onto the (missing) ledger row, so it extracts the ORIGINAL — not the styled
    /// surface the committed txn wrote — instead of compounding Style(Style(orig)).
    #[test]
    fn a_committed_but_unledgered_txn_extracts_the_original_via_the_journal_overlay() {
        use dm_operations::txn::{FileJournal, JournalSink};
        use dm_operations::JournalRecord;

        let dir = tempfile::tempdir().unwrap();
        // Baseline: what the ORIGINAL source renders to for edge.
        let baseline_png = {
            let bdir = dir.path().join("baseline");
            std::fs::create_dir_all(&bdir).unwrap();
            let h = host(&bdir);
            let s = h.scan().unwrap();
            served_png(&h, &s, "edge")
        };

        let (h, desk) = host_with_desk(dir.path());
        // Simulate the committed write landing on the desktop (styled bytes) with NO ledger row.
        desk.force_foreign("edge");
        let live_styled = b"styled:foreign-hand-edit:edge".to_vec();
        let new_fp = dm_domain::Fingerprint::of_bytes(&live_styled);
        let original = b"original:edge".to_vec();
        let orig_fp = dm_domain::Fingerprint::of_bytes(&original);
        let target = dm_domain::ItemTarget::new(
            dm_domain::ItemId::from_raw("edge"),
            dm_domain::ItemKind::Shortcut,
            "C:/Users/Dev/Desktop/edge",
        );
        {
            let mut j = FileJournal::new(dir.path().join("txn.log"));
            j.append(&JournalRecord::TxnBegin { txn: 1, items: vec![target.id.clone()] }).unwrap();
            j.append(&JournalRecord::ItemPrepared {
                txn: 1,
                item: target.id.clone(),
                target: target.clone(),
                anchor: dm_domain::RestoreAnchor::FileBytes { bytes: original.clone() },
                original_fingerprint: orig_fp.clone(),
                expected_fingerprint: orig_fp,
                asset_hash: "deadbeef".into(),
                owned: dm_domain::OwnedFields::icon_only(),
                pinned_seed: None,
            })
            .unwrap();
            j.append(&JournalRecord::ItemApplied {
                txn: 1,
                item: target.id.clone(),
                new_fingerprint: new_fp,
            })
            .unwrap();
            j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        }

        let scan = h.scan().unwrap();
        // The overlay recovered the original anchor → the served source is the ORIGINAL, not the
        // styled live surface.
        assert_eq!(
            served_png(&h, &scan, "edge"),
            baseline_png,
            "the journal overlay extracts the true original, not Style(orig)"
        );
    }

    /// codex icons2-🔴1 (the incomplete case): a Prepared item with NO terminal record has
    /// unknowable live provenance. The scan must NEVER anchor-substitute or offer it for styling
    /// — it degrades until recovery reconciles it.
    #[test]
    fn an_incomplete_journal_item_degrades_and_is_not_styleable() {
        use dm_operations::txn::{FileJournal, JournalSink};
        use dm_operations::JournalRecord;

        let dir = tempfile::tempdir().unwrap();
        let (h, _desk) = host_with_desk(dir.path());
        let target = dm_domain::ItemTarget::new(
            dm_domain::ItemId::from_raw("edge"),
            dm_domain::ItemKind::Shortcut,
            "C:/Users/Dev/Desktop/edge",
        );
        let orig_fp = dm_domain::Fingerprint::of_bytes(b"original:edge");
        {
            let mut j = FileJournal::new(dir.path().join("txn.log"));
            j.append(&JournalRecord::TxnBegin { txn: 1, items: vec![target.id.clone()] }).unwrap();
            j.append(&JournalRecord::ItemPrepared {
                txn: 1,
                item: target.id.clone(),
                target: target.clone(),
                anchor: dm_domain::RestoreAnchor::FileBytes { bytes: b"original:edge".to_vec() },
                original_fingerprint: orig_fp.clone(),
                expected_fingerprint: orig_fp,
                asset_hash: "deadbeef".into(),
                owned: dm_domain::OwnedFields::icon_only(),
                pinned_seed: None,
            })
            .unwrap();
            // No ItemApplied, no terminal record — an interrupted txn.
        }
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        assert!(!edge.styleable, "unknown provenance is never offered for styling");
        assert!(
            edge.status_reason.as_deref().unwrap_or("").contains("待修复"),
            "the degradation reason is honest: {:?}",
            edge.status_reason
        );
    }

    #[test]
    fn one_failing_extract_degrades_that_item_instead_of_failing_the_whole_scan() {
        // codex extractor-review 🟠3: one unreadable icon (OneDrive placeholder, vanished file)
        // must not blank the whole desktop scan — it degrades to styleable:false with a reason.
        struct OneBadExtractor(Arc<DevIconDesktop>);
        impl dm_domain::IconSourceExtractor for OneBadExtractor {
            fn extract(
                &self,
                item: &dm_domain::DesktopItem,
                original: Option<&dm_domain::RestoreAnchor>,
            ) -> dm_domain::PortResult<Vec<dm_domain::DecodedImage>> {
                if item.id.as_str() == "edge" {
                    return Err(dm_domain::PortError::Io("cloud placeholder offline".into()));
                }
                DevIconSourceExtractor(self.0.clone()).extract(item, original)
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.path().join("settings.sqlite3")).unwrap());
        let h = IconHost::new(
            IconHostPorts {
                scanner: Arc::new(DevDesktopScanner),
                extractor: Arc::new(OneBadExtractor(desk.clone())),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk)),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(DevExplorerRefresher),
                elevated: None,
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir.path(),
            1,
            ScopeRoots::Unprivileged,
        );
        let scan = h.scan().unwrap();
        let bad = scan.items.iter().find(|i| i.id == "edge").unwrap();
        assert!(!bad.styleable, "the unreadable item is not offered for styling");
        assert!(bad.source_urls.is_empty());
        assert!(bad.status_reason.as_deref().unwrap_or("").contains("图标读取失败"));
        // Everyone else still scanned + serves sources.
        let good = scan.items.iter().find(|i| i.id == "code").unwrap();
        assert!(good.styleable && !good.source_urls.is_empty());
        assert!(scan.items.len() >= 7, "the rest of the desktop survives one bad item");
    }

    #[test]
    fn apply_then_get_persisted_reads_back_applied_with_saved_style_and_arrow_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();

        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto {
            id: edge.id.clone(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        let res = h.apply_baked_commit(&sid, style_json(1), vec![], Some("第一版".into())).unwrap();
        assert!(res.ok);
        assert!(res.persisted.applied);
        assert_eq!(res.persisted.arrow_overlay, ArrowOverlayDto::Hidden);
        assert!(res.persisted.saved_style_json.is_some());
        assert_eq!(res.persisted.history.len(), 1);

        // getPersisted reads the same truth on a cold call.
        let p = h.get_persisted().unwrap();
        assert!(p.applied && p.saved_style_json.is_some());
    }

    #[test]
    fn a_styled_apply_whose_overlay_is_declined_is_not_a_clean_success() {
        // codex R2 B-2: the icons commit, but the arrow overlay was declined (a cancelled UAC) → the
        // native arrow remains and can double the baked mark, so the op must report ok:false (draft
        // stays dirty for a retry) with a toast, and leave the persisted arrow Native — never a phantom
        // Hidden that the finalize did not actually reach.
        let dir = tempfile::tempdir().unwrap();
        let (h, _desk) = host_with_overlay(dir.path(), Arc::new(DeclinedOverlay));
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto {
            id: edge.id.clone(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        let res = h.apply_baked_commit(&sid, style_json(1), vec![], Some("v1".into())).unwrap();
        assert!(!res.ok, "a declined overlay makes the styled apply a degraded, retryable result");
        assert!(res.toast.is_some(), "the user is told the finalize was incomplete");
        assert!(res.persisted.applied, "the icons themselves committed");
        assert_eq!(
            res.persisted.arrow_overlay,
            ArrowOverlayDto::Native,
            "the arrow was never hidden, so its persisted state must not claim Hidden"
        );
    }

    #[test]
    fn a_genuinely_empty_desktop_scan_is_a_valid_apply_target() {
        // codex R11-#2: a real scan of an EMPTY desktop (nothing to style) is still a valid apply
        // target — the user can submit a policy-only global Apply (kindPolicy/typeOverrides) whose
        // intent must persist to ②③. Validity is an explicit flag, not `scan.is_empty()`; only the
        // never-scanned and fenced states reject a Begin.
        struct EmptyScanner;
        impl DesktopScanner for EmptyScanner {
            fn scan(&self) -> dm_domain::PortResult<Vec<dm_domain::DesktopItem>> {
                Ok(Vec::new())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.path().join("settings.sqlite3")).unwrap());
        let h = IconHost::new(
            IconHostPorts {
                scanner: Arc::new(EmptyScanner),
                extractor: Arc::new(DevIconSourceExtractor(desk.clone())),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk)),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(DevExplorerRefresher),
                elevated: None,
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir.path(),
            1,
            ScopeRoots::Unprivileged,
        );

        // Before ANY scan: no valid snapshot → Begin rejects.
        let err = h.apply_baked_begin(0, 0).unwrap_err();
        assert!(err.contains("no valid scan"), "never-scanned must reject: {err}");

        // A real empty scan: valid target, zero items.
        let scan = h.scan().unwrap();
        assert!(scan.items.is_empty());
        let sid = h.apply_baked_begin(scan.revision, 0).unwrap();
        let res = h.apply_baked_commit(&sid, style_json(5), vec![], Some("策略".into())).unwrap();
        assert!(res.ok, "a conflict-free zero-target (policy-only) Apply is a clean success");
        assert!(res.persisted.saved_style_json.is_some(), "② carries the policy intent");
        assert_eq!(res.persisted.history.len(), 1, "③ recorded the completed Apply");
    }

    #[test]
    fn an_ambiguous_heal_fences_the_scan_revision_until_a_real_rescan() {
        // codex R9-#1: the ABA. scan(O, r1) → apply styles S → the user MANUALLY restores the icon to
        // its exact original outside the app (indistinguishable from a poison row). Apply#2 at r1
        // heals (drops the row) + conflicts — and the host must then FENCE r1: a THIRD apply at the
        // same revision would find no ledger row, pass the ordinary fresh CAS (current O == scan O),
        // and silently overwrite the manual restore. Only a REAL rescan reopens the gate, after which
        // styling is the user's current, unambiguous intent.
        let dir = tempfile::tempdir().unwrap();
        let (h, desk) = host_with_desk(dir.path());
        let scan1 = h.scan().unwrap();
        let edge = scan1.items.iter().find(|i| i.id == "edge").unwrap().clone();

        // Apply #1: style edge (ledger row: original=O, last_applied=S).
        let sid = h.apply_baked_begin(scan1.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        assert!(h.apply_baked_commit(&sid, style_json(1), vec![], Some("A".into())).unwrap().ok);

        // The user manually restores edge to its exact original (ledger row lingers → ambiguous tuple).
        desk.force_original("edge");

        // Apply #2 at the SAME revision: the heal drops the row + conflicts — never silently restyles.
        let sid2 = h.apply_baked_begin(scan1.revision, 1).unwrap();
        h.apply_baked_chunk(&sid2, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        let res2 = h.apply_baked_commit(&sid2, style_json(2), vec![], Some("B".into())).unwrap();
        assert!(!res2.ok, "the ambiguous heal must not read as success");
        assert_eq!(res2.toast.unwrap().key, "Toast_ApplyNoEffect");

        // The fence: a THIRD apply at the same stale revision is REJECTED before it can slip through
        // the now-row-less fresh CAS.
        let err = h.apply_baked_begin(scan1.revision, 1).unwrap_err();
        assert!(err.contains("stale apply"), "same-revision retry must be fenced: {err}");
        // And the fenced state means "NO valid scan" (codex R10-#B): the snapshot was cleared, so even
        // a Begin that somehow carries the synthetic fenced revision cannot bind pre-heal fingerprints.
        // Probe the fenced revision by brute force over the small window above scan1.
        for fenced in scan1.revision + 1..scan1.revision + 4 {
            if let Err(e) = h.apply_baked_begin(fenced, 1) {
                assert!(
                    e.contains("stale apply") || e.contains("no valid scan"),
                    "a fenced/unknown revision must never bind a snapshot: {e}"
                );
            } else {
                panic!("Begin({fenced}) bound a snapshot inside the fenced window");
            }
        }

        // A real rescan reopens the gate; styling is now the user's current, unambiguous intent.
        let scan2 = h.scan().unwrap();
        assert!(scan2.revision > scan1.revision);
        let sid3 = h.apply_baked_begin(scan2.revision, 1).unwrap();
        h.apply_baked_chunk(&sid3, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        assert!(h.apply_baked_commit(&sid3, style_json(2), vec![], Some("B2".into())).unwrap().ok);
    }

    #[test]
    fn restore_reverts_and_returns_arrow_native_no_saved_style() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        h.apply_baked_commit(&sid, style_json(1), vec![], Some("A".into())).unwrap();

        let res = h.restore().unwrap();
        assert!(res.ok);
        assert!(!res.persisted.applied, "everything reverted");
        assert!(res.persisted.saved_style_json.is_none(), "saved-style cleared");
        assert_eq!(res.persisted.arrow_overlay, ArrowOverlayDto::Native);
    }

    #[test]
    fn restore_overlay_keeps_the_look_and_only_lifts_the_arrow() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        h.apply_baked_commit(&sid, style_json(1), vec![], Some("A".into())).unwrap();

        let res = h.restore_overlay().unwrap();
        assert!(res.ok);
        assert_eq!(res.persisted.arrow_overlay, ArrowOverlayDto::Native, "arrow lifted");
        assert!(res.persisted.applied, "the icon look is UNTOUCHED (keep-beautify)");
        assert!(res.persisted.saved_style_json.is_some());
    }

    #[test]
    fn a_chunk_without_a_session_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        // No Begin ⇒ session token "0"; any presented token mismatches ⇒ rejected.
        assert!(h
            .apply_baked_chunk("1", vec![IconChunkItemDto { id: "edge".into(), source_index: 0, master_png: tiny_master() }])
            .is_err());
    }

    #[test]
    fn a_chunk_or_commit_with_a_stale_token_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        // Begin A gets token "1"; Begin B abandons it + gets "2".
        let sid_a = h.apply_baked_begin(scan.revision, 1).unwrap();
        let sid_b = h.apply_baked_begin(scan.revision, 1).unwrap();
        assert_ne!(sid_a, sid_b, "each Begin mints a fresh token");
        // A's stale chunk is rejected; only B's token is live.
        assert!(h
            .apply_baked_chunk(&sid_a, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }])
            .is_err());
        h.apply_baked_chunk(&sid_b, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }])
            .unwrap();
        // A's stale COMMIT is rejected (ok:false), never mutating with B's buffer.
        let stale = h.apply_baked_commit(&sid_a, style_json(1), vec![], Some("A".into())).unwrap();
        assert!(!stale.ok, "a stale-token commit must not succeed");
    }

    #[test]
    fn source_cache_keeps_many_generations_and_never_evicts_a_live_key() {
        // codex R3-Major 5 / R4-Major 2: a changed icon's OLD content-addressed URL must still resolve
        // across the swap→adopt handoff — for MORE than one generation, since a scan whose adopt failed
        // leaves the UI on an even-older generation. The byte-bounded LRU covers it; an unchanged icon
        // that is re-scanned every generation must NEVER age out.
        let mut c = SourceCache::new(1024); // small cap, but each entry is tiny → many survive
        c.publish(HashMap::from([("live/0/h".to_string(), vec![1u8]), ("a/0/h1".to_string(), vec![2u8])]));
        // Several more generations where `live` is unchanged (re-inserted, same key) and `a` changes.
        c.publish(HashMap::from([("live/0/h".to_string(), vec![1u8]), ("a/0/h2".to_string(), vec![3u8])]));
        c.publish(HashMap::from([("live/0/h".to_string(), vec![1u8]), ("a/0/h3".to_string(), vec![4u8])]));
        // The live key survives every generation; the changed key's older versions survive well past a
        // single generation (all under the cap here).
        assert_eq!(c.get("live/0/h"), Some(vec![1]), "an unchanged, re-scanned icon never evicts");
        assert_eq!(c.get("a/0/h1"), Some(vec![2]), "two generations back still resolves");
        assert_eq!(c.get("a/0/h3"), Some(vec![4]), "the live generation resolves");
    }

    #[test]
    fn source_cache_evicts_oldest_past_the_byte_cap() {
        let mut c = SourceCache::new(100);
        // Insert entries that together exceed the cap; the oldest must be evicted, the newest kept.
        c.publish(HashMap::from([("old/0/h".to_string(), vec![0u8; 60])]));
        c.publish(HashMap::from([("new/0/h".to_string(), vec![0u8; 60])])); // now 120 > 100 → evict oldest
        assert_eq!(c.get("old/0/h"), None, "the oldest key was evicted past the cap");
        assert_eq!(c.get("new/0/h").map(|v| v.len()), Some(60), "the newest key is always kept");
    }

    #[test]
    fn source_cache_never_evicts_a_key_of_the_generation_being_published() {
        // codex R5-#7: a single scan whose OWN sources exceed the cap must still resolve EVERY key it
        // advertises — the DTO points the webview at all of them, so evicting one mid-publish would 404
        // the live desktop. The cap bounds HISTORICAL generations, never the current one.
        let mut c = SourceCache::new(100);
        c.publish(HashMap::from([("old/0/h".to_string(), vec![0u8; 90])])); // historical, will be trimmed
        // One generation of three 50-byte icons = 150 bytes > the 100 cap. All three must survive.
        c.publish(HashMap::from([
            ("live/0/h".to_string(), vec![1u8; 50]),
            ("live/1/h".to_string(), vec![2u8; 50]),
            ("live/2/h".to_string(), vec![3u8; 50]),
        ]));
        assert_eq!(c.get("old/0/h"), None, "the prior generation is evicted to make room");
        assert_eq!(c.get("live/0/h").map(|v| v.len()), Some(50), "live key 0 is pinned, never evicted");
        assert_eq!(c.get("live/1/h").map(|v| v.len()), Some(50), "live key 1 is pinned, never evicted");
        assert_eq!(c.get("live/2/h").map(|v| v.len()), Some(50), "live key 2 is pinned, never evicted");
    }

    #[test]
    fn get_persisted_keeps_the_restore_affordance_when_an_in_flight_txn_lingers() {
        // codex R5-#6: a degraded prior-crash recovery leaves an IN-FLIGHT (no-terminal) txn in the
        // journal and can style the desktop with NO ledger row. `applied` off the ledger alone would
        // then be false and HIDE the restore affordance — stranding the user. get_persisted must keep
        // `applied: true` off the retained in-flight journal so the restore path (which re-runs
        // recovery and heals) stays reachable.
        use dm_operations::txn::{journal::JournalRecord, FileJournal, JournalSink};
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        assert!(!h.get_persisted().unwrap().applied, "clean start: nothing applied, no in-flight txn");

        // Simulate the degraded recovery's residue: an in-flight txn lingering in the journal.
        let mut j = FileJournal::new(dir.path().join("txn.log"));
        j.append(&JournalRecord::TxnBegin { txn: 1, items: vec![dm_domain::ItemId::from_raw("x")] })
            .unwrap();

        assert!(
            h.get_persisted().unwrap().applied,
            "a lingering in-flight txn forces applied:true so restore stays reachable (ledger is still empty)"
        );

        // A CLEANLY-terminated txn (rolled back) is NOT pending repair — it must NOT trip the signal.
        j.append(&JournalRecord::TxnRolledBack { txn: 1 }).unwrap();
        assert!(
            !h.get_persisted().unwrap().applied,
            "a terminal (rolled-back) txn awaiting checkpoint never spuriously shows restore"
        );
    }

    /// An [`ExplorerRefresher`] that counts `restart_shell()` calls, so a test can assert exactly
    /// when a desktop-mutating op restarts the shell (the reliable folder/.url refresh, owner box
    /// 2026-07-17). The `notify_*` methods no-op like the dev refresher.
    struct RecordingRefresher(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl ExplorerRefresher for RecordingRefresher {
        fn notify_icons_changed(&self) -> dm_domain::PortResult<()> {
            Ok(())
        }
        fn restart_shell(&self) -> dm_domain::PortResult<()> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    /// A 256×256 solid-color master PNG. Distinct colors bake to DISTINCT bytes, so a re-style with a
    /// new color is a real desktop mutation (not a content-addressed CAS-skip).
    fn solid_master(rgb: [u8; 3]) -> String {
        use base64::Engine;
        use image::ImageEncoder;
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    /// Build a host whose refresher counts shell restarts.
    fn host_recording(
        dir: &std::path::Path,
        restarts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> IconHost {
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.join("settings.sqlite3")).unwrap());
        IconHost::new(
            IconHostPorts {
                scanner: Arc::new(DevDesktopScanner),
                extractor: Arc::new(DevIconSourceExtractor(desk.clone())),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk.clone())),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(RecordingRefresher(restarts)),
                elevated: None,
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir,
            1,
            ScopeRoots::Unprivileged,
        )
    }

    /// Style a SINGLE item to a solid color under a labelled version.
    fn apply_one(h: &IconHost, rev: u32, id: &str, seed: i64, rgb: [u8; 3], label: &str) -> IconOpResultDto {
        let sid = h.apply_baked_begin(rev, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto {
            id: id.into(),
            source_index: 0,
            master_png: solid_master(rgb),
        }])
        .unwrap();
        h.apply_baked_commit(&sid, style_json(seed), vec![], Some(label.into())).unwrap()
    }

    #[test]
    fn every_desktop_mutating_op_restarts_the_shell_but_an_idempotent_reapply_does_not() {
        // owner box 2026-07-17: a SECOND preset applied over a first left the desktop's FOLDER
        // (`desktop.ini`) / `.url` custom icons stuck on their stale first-preset bitmap. The desktop
        // caches container-kind icons and — pixel-verified — ONLY an Explorer restart reliably
        // re-resolves them; `.lnk` self-refresh masked the bug. So the target here is `docs`, the fake
        // desktop's FOLDER item (NOT a `.lnk`, which would refresh on its own and prove nothing): every
        // desktop-MUTATING op on it (first apply, a re-styling second preset, a restore) must restart
        // the shell, while an idempotent re-apply that changes nothing must NOT (no gratuitous flash).
        use std::sync::atomic::{AtomicUsize, Ordering};
        let restarts = Arc::new(AtomicUsize::new(0));
        let dir = tempfile::tempdir().unwrap();
        let h = host_recording(dir.path(), restarts.clone());
        // The target is a FOLDER, the exact kind the desktop caches (the bug's real surface).
        let scan = h.scan().unwrap();
        assert_eq!(
            scan.items.iter().find(|i| i.id == "docs").unwrap().kind,
            IconKindDto::Folder,
            "the regression target must be the container kind that actually got stuck"
        );

        // Preset A styles the folder → the shell restarts once.
        assert!(apply_one(&h, scan.revision, "docs", 1, [120, 90, 200], "vA").ok);
        assert_eq!(restarts.load(Ordering::SeqCst), 1, "the first apply restarts the shell");

        // Preset B (different baked bytes) re-styles the folder — THE 2ND-PRESET REGRESSION: it too
        // must restart, or the folder stays on preset A's cached icon.
        let scan2 = h.scan().unwrap();
        assert!(apply_one(&h, scan2.revision, "docs", 2, [20, 200, 80], "vB").ok);
        assert_eq!(restarts.load(Ordering::SeqCst), 2, "a second, different preset also restarts the shell");

        // Re-applying preset B unchanged is a content-addressed CAS-skip (nothing committed). Assert
        // the folder is STILL styled (a genuine no-op, not a failure that reverted — else the "no
        // restart" check would pass spuriously, codex 2026-07-17), then that no restart fired.
        let scan3 = h.scan().unwrap();
        let reapply = apply_one(&h, scan3.revision, "docs", 2, [20, 200, 80], "vB");
        assert!(reapply.persisted.applied, "the idempotent re-apply left the folder styled");
        assert_eq!(restarts.load(Ordering::SeqCst), 2, "an idempotent re-apply does not restart the shell");

        // Restore reverts the folder → the shell restarts so the default folder icon re-resolves.
        assert!(h.restore().unwrap().ok);
        assert_eq!(restarts.load(Ordering::SeqCst), 3, "restore restarts the shell");
    }

    #[test]
    fn switching_appearance_version_restarts_the_shell_only_when_the_desktop_changes() {
        // switchVersion is the third desktop-mutating verb; its restart gate is
        // `!committed.is_empty() || desktop_mutated` — NOT `committed` alone. A switch whose driver
        // writes then rolls back / bare-errors leaves `committed` EMPTY yet `desktop_mutated` TRUE, and
        // the folder would keep a cached transient icon if the host skipped the restart (codex
        // 2026-07-17 Block). DO NOT simplify the gate back to a `committed`-only check.
        use std::sync::atomic::{AtomicUsize, Ordering};
        let restarts = Arc::new(AtomicUsize::new(0));
        let dir = tempfile::tempdir().unwrap();
        let h = host_recording(dir.path(), restarts.clone());

        // Two saved versions of the FOLDER's look; the desktop ends on vB.
        let scan = h.scan().unwrap();
        let ra = apply_one(&h, scan.revision, "docs", 1, [200, 40, 40], "vA");
        let va = ra.persisted.history.iter().find(|v| v.label.as_deref() == Some("vA")).unwrap().id.clone();
        let scan2 = h.scan().unwrap();
        assert!(apply_one(&h, scan2.revision, "docs", 2, [40, 40, 200], "vB").ok);
        assert_eq!(restarts.load(Ordering::SeqCst), 2, "the two applies each restarted the shell");

        // Switch the folder back to vA — a real re-style (vB → vA) → the shell restarts.
        assert!(h.switch_version(&va).unwrap().ok);
        assert_eq!(restarts.load(Ordering::SeqCst), 3, "a version switch that re-styles the folder restarts the shell");

        // Switch to vA again — the folder already wears vA → CAS-skip, nothing committed, desktop
        // untouched → NO restart (no gratuitous flash).
        let _ = h.switch_version(&va).unwrap();
        assert_eq!(restarts.load(Ordering::SeqCst), 3, "a no-op version switch does not restart the shell");
    }
