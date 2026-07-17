//! xtask — repo automation (ADR-0019). Spike 4 commands:
//!
//! - `spike4-native <dir>`: render the slice natively over the dumped sources
//!   (`<dir>/sources/*.rgba`, written by scripts/spike4-slice.ts) into
//!   `<dir>/native/<id>-<size>.rgba`.
//! - `spike4-compare <dir>`: tri-target comparison — native↔wasm byte
//!   equality, TS↔Rust per-byte max diff, plus the cross-language fixture
//!   probes (decode-LUT bits, srgbEncode curve, shadow tone, mask bits).
//!   Exit 1 when a gate fails.
//!
//! See tests/icon-parity/spike4/run.ts for the orchestrated pipeline.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};

use dm_icon_core::analysis::{
    bounds_h, bounds_w, corners_symmetric, find_content_bounds, foreground_bounds, matches_shape,
    max_scale_inside, solid_bounds, try_detect_background, visible_lightness_mean, ContentBounds,
};
use dm_icon_core::color::{field_shadow_tone, srgb_decode, srgb_encode};
use dm_icon_core::compose::{render_slice_tile, ComposeDiagnostics, RenderOpts};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::hue_spread::{compute_hue_spread, SpreadEntry};
use dm_icon_core::js_math::js_round;
use dm_icon_core::marks::set_native_arrow_raster;
use dm_icon_core::profile::{icon_profile, IconProfileKind};
use dm_icon_core::raster::{hex_to_int, shape_mask, Raster, Rgba, WHITE};
use dm_icon_core::render_session::RenderSession;
use dm_icon_core::segment::segment_subject;
use dm_icon_core::shapes::IconShape;

/// The Spike-4 slice sizes — must match scripts/spike4-slice.ts SIZES.
const SIZES: [usize; 2] = [256, 512];
const SOURCE_SIZE: usize = 256;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, dir] if cmd == "spike4-native" => spike4_native(Path::new(dir)),
        [cmd, dir] if cmd == "spike4-compare" => spike4_compare(Path::new(dir)),
        [cmd, dir] if cmd == "m5-shape-masks" => m5_shape_masks(Path::new(dir)),
        [cmd, dir] if cmd == "m5-profiles" => m5_profiles(Path::new(dir)),
        [cmd, dir] if cmd == "m5-hue" => m5_hue(Path::new(dir)),
        [cmd, dir] if cmd == "m5-pixels" => m5_pixels(Path::new(dir)),
        _ => {
            eprintln!(
                "usage: xtask spike4-native <dir> | spike4-compare <dir> | m5-shape-masks <dir> | m5-profiles <dir> | m5-hue <dir> | m5-pixels <dir>"
            );
            ExitCode::from(2)
        }
    }
}

// ---- M5 modules 4+5 gate: StageProfile deep-equal + mask byte-equal ---------

const CORPUS_SIZE: usize = 256;

fn round6(n: f64) -> f64 {
    js_round(n * 1e6) / 1e6
}

fn hex(c: Option<Rgba>) -> Value {
    match c {
        Some(c) => json!(format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)),
        None => Value::Null,
    }
}

fn rect(b: Option<ContentBounds>) -> Value {
    match b {
        Some(b) => json!([b.left, b.top, b.right, b.bottom]),
        None => Value::Null,
    }
}

/// Recompute the `stage-dump.ts` StageProfile in Rust + the resolved subject mask.
fn build_stage_profile(raster: &Raster) -> (Value, Vec<u8>) {
    let profile = icon_profile(raster);
    let content = find_content_bounds(raster);
    let min_dim = bounds_w(content).min(bounds_h(content));
    let bg = try_detect_background(raster);
    let fg = bg.and_then(|b| foreground_bounds(raster, content, b, 48));
    let canvas = raster.width * raster.height;
    let mask = profile
        .subject_mask
        .clone()
        .unwrap_or_else(|| segment_subject(raster).mask);
    let mask_solid: usize = mask.iter().map(|&v| v as usize).sum();

    let kind = match profile.kind {
        IconProfileKind::FullSquare => "fullSquare",
        IconProfileKind::OwnBoard => "ownBoard",
        IconProfileKind::Bare => "bare",
    };
    let v = json!({
        "kind": kind,
        "transparentEdges": profile.transparent_edges,
        "alphaBBox": rect(Some(content)),
        "coverage": round6((bounds_w(content) * bounds_h(content)) as f64 / canvas as f64),
        "solidBBox": rect(solid_bounds(raster)),
        "ownBackground": hex(profile.background),
        "ownBackgroundLightness": match profile.background_lightness {
            Some(l) => json!(round6(l)),
            None => Value::Null,
        },
        "anchorRect": rect(Some(content)),
        "cornerSymmetric": corners_symmetric(raster, content, min_dim),
        "rimColour": hex(profile.subject_rim_colour),
        "rimLightness": round6(profile.subject_rim_lightness),
        "subjectColour": hex(profile.subject_colour),
        "subjectLightness": round6(profile.subject_lightness),
        "foregroundBBox": rect(fg),
        "visibleLightness": round6(visible_lightness_mean(raster)),
        "matchesCircle": matches_shape(raster, IconShape::Circle),
        "matchesApple": matches_shape(raster, IconShape::Apple),
        "maxScaleCircle": round6(max_scale_inside(raster, content, IconShape::Circle)),
        "seed": hex(profile.subject_rim_colour),
        "maskCoverage": round6(mask_solid as f64 / canvas as f64),
        "profileKeepsMask": profile.subject_mask.is_some(),
    });
    (v, mask)
}

/// Deep-equal with numeric nodes compared by f64 value (JSON emits integral
/// rounded floats as ints — 1 vs 1.0 must still match). Returns the first diff.
fn json_diff(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.as_f64() == y.as_f64() {
                None
            } else {
                Some(format!("{path}: {x} != {y}"))
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, xv) in x {
                match y.get(k) {
                    Some(yv) => {
                        if let Some(d) = json_diff(xv, yv, &format!("{path}.{k}")) {
                            return Some(d);
                        }
                    }
                    None => return Some(format!("{path}.{k}: missing in committed")),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: array len {} != {}", x.len(), y.len()));
            }
            for (i, (xv, yv)) in x.iter().zip(y).enumerate() {
                if let Some(d) = json_diff(xv, yv, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            None
        }
        _ => {
            if a == b {
                None
            } else {
                Some(format!("{path}: {a} != {b}"))
            }
        }
    }
}

// ---- M5 modules 8/9/10 gate: full pixel differential -----------------------

fn parse_config(v: &Value) -> Config {
    let s = |k: &str| v[k].as_str().unwrap_or_else(|| panic!("config.{k} not a string in {v}"));
    Config {
        shape: parse_shape(s("shape")),
        subject: match s("subject") {
            "Original" => Subject::Original,
            "BlackWhite" => Subject::BlackWhite,
            "Mono" => Subject::Mono,
            o => panic!("subject {o}"),
        },
        tint: hex_to_int(s("tint")),
        mono_style: match s("monoStyle") {
            "Tonal" => MonoStyle::Tonal,
            "Flat" => MonoStyle::Flat,
            o => panic!("monoStyle {o}"),
        },
        plate_band: match s("plateBand") {
            "Vivid" => Band::Vivid,
            "Quiet" => Band::Quiet,
            o => panic!("plateBand {o}"),
        },
        shortcut_shape: v["shortcutShape"].as_str().map(parse_shape),
        distinction: match s("distinction") {
            "Mark" => Distinction::Mark,
            "Keep" => Distinction::Keep,
            "None" => Distinction::None,
            o => panic!("distinction {o}"),
        },
        mark_style: match s("markStyle") {
            "Glass" => MarkStyle::Glass,
            "Shadow" => MarkStyle::Shadow,
            "Halo" => MarkStyle::Halo,
            "Satin" => MarkStyle::Satin,
            "Arc" => MarkStyle::Arc,
            "Fold" => MarkStyle::Fold,
            "Ring" => MarkStyle::Ring,
            "Comet" => MarkStyle::Comet,
            o => panic!("markStyle {o}"),
        },
        mark_color: v["markColor"].as_str().map(hex_to_int),
        filter: match s("filter") {
            "None" => FilterStyle::None,
            "Gloss" => FilterStyle::Gloss,
            "Glass" => FilterStyle::Glass,
            "Pixel" => FilterStyle::Pixel,
            "Sticker" => FilterStyle::Sticker,
            o => panic!("filter {o}"),
        },
        plate_color: v["plateColor"].as_str().map(hex_to_int),
        plate_fallback: match s("plateFallback") {
            "derived" => PlateFallback::Derived,
            "white" => PlateFallback::White,
            o => panic!("plateFallback {o}"),
        },
        // Corpus configs are frozen-oracle fixtures: absent => false keeps the
        // golden byte-parity; a fixture may opt in explicitly.
        auto_separation: v["autoSeparation"].as_bool().unwrap_or(false),
    }
}

#[derive(Default)]
struct LaneStat {
    cells: usize,
    equal: usize,
    diff_bytes: u64,
    total_bytes: u64,
}

fn m5_pixels(dir: &Path) -> ExitCode {
    let base = dir.join("pixels");

    // Load the genuine Win11 arrow badge exactly as the app worker does at boot.
    let meta: Value = serde_json::from_slice(&fs::read(base.join("arrow.json")).unwrap())
        .expect("parse arrow.json");
    let aw = meta["width"].as_u64().unwrap() as usize;
    let ah = meta["height"].as_u64().unwrap() as usize;
    let arrow_data = read_rgba(&base.join("arrow.rgba"), aw * ah * 4);
    set_native_arrow_raster(Some(Raster { width: aw, height: ah, data: arrow_data }));

    let lines = fs::read_to_string(base.join("cells.jsonl"))
        .unwrap_or_else(|e| panic!("read cells.jsonl: {e} — run tests/icon-parity/m5/cells.ts"));
    let mut per_lane: BTreeMap<String, LaneStat> = BTreeMap::new();
    let mut total = LaneStat::default();
    let mut lane_mismatch = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut worst: Option<(String, u8)> = None;
    // Render through RenderSession (register + set_look + render) — the SAME session
    // path the wasm cert drives, and the home of the Phase-1 session-owned mask cache.
    // Driving the bare `render_tile` here would leave a session warm-hit/eviction
    // regression untested on native AND wasm (Codex Phase-0 audit #5).
    let mut session = RenderSession::new();
    let mut registered: BTreeMap<String, u64> = BTreeMap::new();
    let mut next_hash: u64 = 1;

    for line in lines.lines().filter(|l| !l.is_empty()) {
        let rec: Value = serde_json::from_str(line).expect("parse cell record");
        let file = rec["file"].as_str().unwrap();
        let source_id = rec["sourceId"].as_str().unwrap();
        let config = parse_config(&rec["config"]);
        let is_shortcut = rec["isShortcut"].as_bool().unwrap();
        let show_original = rec["showOriginal"].as_bool().unwrap();
        let opts = RenderOpts { field_seed: rec["opts"]["fieldSeed"].as_str().map(hex_to_int) };
        let expected_lane = rec["lane"].as_str().unwrap();
        let expected_field = rec["fieldLane"].as_str();

        if !registered.contains_key(source_id) {
            let data = read_rgba(&base.join(format!("sources/{source_id}.rgba")), CORPUS_SIZE * CORPUS_SIZE * 4);
            let hash = next_hash;
            next_hash += 1;
            session.register(source_id.to_owned(), hash, Raster { width: CORPUS_SIZE, height: CORPUS_SIZE, data });
            registered.insert(source_id.to_owned(), hash);
        }

        session.set_look(config);
        let mut diag = ComposeDiagnostics::default();
        let out = session
            .render(source_id, is_shortcut, show_original, CORPUS_SIZE, &opts, &mut diag)
            .expect("RenderSession produced no tile (missing source or look?)");
        let expected = read_rgba(&base.join(format!("expected/{file}.rgba")), CORPUS_SIZE * CORPUS_SIZE * 4);

        let lane_str = diag.lane.as_str().to_owned();
        let entry = per_lane.entry(lane_str.clone()).or_default();
        entry.cells += 1;
        total.cells += 1;
        entry.total_bytes += expected.len() as u64;
        total.total_bytes += expected.len() as u64;

        // Length guard BEFORE the zip — a short/long render otherwise truncates to the
        // shorter and reports 0 diff / exact total, masking the defect (audit #6).
        if out.data.len() != expected.len() {
            let d = out.data.len().abs_diff(expected.len()) as u64;
            entry.diff_bytes += d;
            total.diff_bytes += d;
            if failures.len() < 40 {
                failures.push(format!(
                    "{file} [{lane_str}]: output length {} != expected {}",
                    out.data.len(),
                    expected.len()
                ));
            }
            continue;
        }

        let mut first: Option<usize> = None;
        let mut diff = 0u64;
        for (i, (&a, &b)) in out.data.iter().zip(expected.iter()).enumerate() {
            if a != b {
                diff += 1;
                if first.is_none() {
                    first = Some(i);
                }
                let d = a.abs_diff(b);
                if worst.as_ref().map(|w| d > w.1).unwrap_or(true) {
                    worst = Some((file.to_owned(), d));
                }
            }
        }
        entry.diff_bytes += diff;
        total.diff_bytes += diff;
        if diff == 0 {
            entry.equal += 1;
            total.equal += 1;
        } else if failures.len() < 40 {
            let at = first.unwrap();
            failures.push(format!(
                "{file} [{lane_str}]: {diff} diff bytes, first at ({},{}) ch {} rust={} ts={}",
                (at / 4) % CORPUS_SIZE,
                (at / 4) / CORPUS_SIZE,
                at % 4,
                out.data[at],
                expected[at],
            ));
        }

        let field_str = diag.field_lane.map(|f| f.as_str());
        if lane_str != expected_lane || field_str != expected_field {
            lane_mismatch += 1;
            if failures.len() < 40 {
                failures.push(format!(
                    "{file}: lane {lane_str}/{field_str:?} != {expected_lane}/{expected_field:?}"
                ));
            }
        }
    }

    println!("== M5 full pixel differential (compose + marks + filters) ==");
    println!("{:<18} {:>6} {:>12} {:>18}", "lane", "cells", "equal-cells", "diff-bytes");
    for (lane, s) in &per_lane {
        println!("{:<18} {:>6} {:>12} {:>18}", lane, s.cells, s.equal, format!("{}/{}", s.diff_bytes, s.total_bytes));
    }
    println!("{:<18} {:>6} {:>12} {:>18}", "TOTAL", total.cells, total.equal, format!("{}/{}", total.diff_bytes, total.total_bytes));
    println!("lane/fieldLane mismatches: {lane_mismatch}");
    if let Some((f, d)) = &worst {
        println!("worst byte delta: {d} ({f})");
    }

    if total.equal == total.cells && lane_mismatch == 0 {
        println!("RESULT: PASS — every tier-A + tier-B cell is byte-identical and every lane exact");
        return ExitCode::SUCCESS;
    }
    println!("RESULT: FAIL");
    for f in failures.iter().take(40) {
        println!("  {f}");
    }
    ExitCode::FAILURE
}

// ---- M5 module-6 gate: computeHueSpread parity -----------------------------

fn m5_hue(dir: &Path) -> ExitCode {
    let hue = dir.join("hue");
    let mut files: Vec<PathBuf> = fs::read_dir(&hue)
        .unwrap_or_else(|e| panic!("read {}: {e} — run tests/icon-parity/m5/hue-spread.ts", hue.display()))
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension().is_some_and(|x| x == "json")).then_some(p)
        })
        .collect();
    files.sort();

    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let preset = path.file_stem().unwrap().to_str().unwrap();
        let v: Value = serde_json::from_slice(&fs::read(path).unwrap()).expect("parse hue json");
        let entries: Vec<SpreadEntry> = v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| SpreadEntry {
                id: e["id"].as_str().unwrap().to_owned(),
                art_key: e["artKey"].as_str().unwrap().to_owned(),
                seed: e["seed"].as_str().map(str::to_owned),
            })
            .collect();
        let expected = v["output"].as_object().unwrap();
        let got = compute_hue_spread(&entries);

        let mut diff: Option<String> = None;
        if got.len() != expected.len() {
            diff = Some(format!("size {} != {}", got.len(), expected.len()));
        } else {
            for (k, val) in &got {
                match expected.get(k).and_then(|x| x.as_str()) {
                    Some(ev) if ev == val => {}
                    Some(ev) => {
                        diff = Some(format!("{k}: {val} != {ev}"));
                        break;
                    }
                    None => {
                        diff = Some(format!("{k}: missing in expected"));
                        break;
                    }
                }
            }
        }
        match diff {
            None => ok += 1,
            Some(d) => failures.push(format!("{preset}: {d}")),
        }
    }

    println!("== M5 hue-spread parity ==");
    println!("presets: {}   id→hex map identical: {}/{}", files.len(), ok, files.len());
    if ok == files.len() {
        println!("RESULT: PASS — computeHueSpread is identical to the TS oracle on every preset");
        return ExitCode::SUCCESS;
    }
    println!("RESULT: FAIL");
    for f in failures.iter().take(20) {
        println!("  {f}");
    }
    ExitCode::FAILURE
}

fn m5_profiles(dir: &Path) -> ExitCode {
    let base = dir.join("profiles");
    let srcs = base.join("sources");
    let corpus = Path::new("testdata/icons/sources/profiles");

    let mut ids: Vec<String> = fs::read_dir(&srcs)
        .unwrap_or_else(|e| panic!("read {}: {e} — run tests/icon-parity/m5/profiles.ts", srcs.display()))
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".rgba").map(str::to_owned)
        })
        .collect();
    ids.sort();

    let mut json_ok = 0usize;
    let mut mask_ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for id in &ids {
        let data = read_rgba(&srcs.join(format!("{id}.rgba")), CORPUS_SIZE * CORPUS_SIZE * 4);
        let raster = Raster { width: CORPUS_SIZE, height: CORPUS_SIZE, data };
        let (mine, mask) = build_stage_profile(&raster);

        let committed: Value = serde_json::from_slice(
            &fs::read(corpus.join(format!("{id}.json")))
                .unwrap_or_else(|e| panic!("read corpus {id}.json: {e}")),
        )
        .expect("parse committed profile");
        match json_diff(&mine, &committed, id) {
            None => json_ok += 1,
            Some(d) => failures.push(format!("profile {d}")),
        }

        let mask_ref = fs::read(base.join(format!("masks/{id}.bin"))).expect("read mask.bin");
        if mask_ref == mask {
            mask_ok += 1;
        } else {
            let at = mask_ref.iter().zip(mask.iter()).position(|(a, b)| a != b);
            failures.push(format!(
                "mask {id}: differs (len {}/{}, first {:?})",
                mask.len(),
                mask_ref.len(),
                at
            ));
        }
    }

    println!("== M5 profile + mask parity (analysis + segment + profile) ==");
    println!("sources: {}   profile deep-equal: {}/{}   mask byte-equal: {}/{}", ids.len(), json_ok, ids.len(), mask_ok, ids.len());
    if json_ok == ids.len() && mask_ok == ids.len() {
        println!("RESULT: PASS — every StageProfile matches the committed corpus and every mask is byte-identical");
        return ExitCode::SUCCESS;
    }
    println!("RESULT: FAIL");
    for f in failures.iter().take(30) {
        println!("  {f}");
    }
    ExitCode::FAILURE
}

// ---- M5 module-2 gate: shape-mask bit parity -------------------------------

fn parse_shape(name: &str) -> IconShape {
    match name {
        "Apple" => IconShape::Apple,
        "Circle" => IconShape::Circle,
        "Samsung" => IconShape::Samsung,
        "None" => IconShape::None,
        "Bookmark" => IconShape::Bookmark,
        "Lemon" => IconShape::Lemon,
        "Tile" => IconShape::Tile,
        "Teardrop" => IconShape::Teardrop,
        "Diamond" => IconShape::Diamond,
        "Flower" => IconShape::Flower,
        "Pebble" => IconShape::Pebble,
        "Folder" => IconShape::Folder,
        "File" => IconShape::File,
        other => panic!("unknown shape {other}"),
    }
}

/// Compare the Rust `shape_mask` against the TS-dumped Float64Array masks
/// (`<dir>/shapes/<shape>-<size>-<shapeSize>-<ox>-<oy>.f64`) bit-for-bit.
fn m5_shape_masks(dir: &Path) -> ExitCode {
    let shapes = dir.join("shapes");
    let mut files: Vec<PathBuf> = fs::read_dir(&shapes)
        .unwrap_or_else(|e| panic!("read {}: {e} — run tests/icon-parity/m5/shape-masks.ts", shapes.display()))
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension().is_some_and(|x| x == "f64")).then_some(p)
        })
        .collect();
    files.sort();

    let mut cells = 0usize;
    let mut equal = 0usize;
    let mut total_diff_bits = 0u64;
    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let f: Vec<&str> = stem.split('-').collect();
        let shape = parse_shape(f[0]);
        let size: usize = f[1].parse().unwrap();
        let shape_size: usize = f[2].parse().unwrap();
        let ox: f64 = f[3].parse().unwrap();
        let oy: f64 = f[4].parse().unwrap();

        let bytes = fs::read(path).unwrap();
        assert_eq!(bytes.len(), size * size * 8, "{} wrong length", path.display());
        let ts: Vec<f64> = bytes
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let rust = shape_mask(shape, size, shape_size, ox, oy);

        cells += 1;
        let mut cell_ok = true;
        let mut first: Option<usize> = None;
        for (i, (&a, &b)) in ts.iter().zip(rust.iter()).enumerate() {
            if a.to_bits() != b.to_bits() {
                cell_ok = false;
                total_diff_bits += 1;
                if first.is_none() {
                    first = Some(i);
                }
            }
        }
        if cell_ok {
            equal += 1;
        } else {
            let i = first.unwrap();
            failures.push(format!(
                "{stem}: {} diff cells, first at index {i} ({},{}) TS={} Rust={}",
                total_diff_bits.min(size as u64 * size as u64),
                i % size,
                i / size,
                ts[i],
                rust[i],
            ));
        }
    }

    println!("== M5 shape-mask parity ==");
    println!("mask cases: {cells}   bit-identical: {equal}/{cells}");
    if equal == cells {
        println!("RESULT: PASS — every catalog shape's mask is bit-identical to the TS oracle");
        return ExitCode::SUCCESS;
    }
    println!("total differing f64 cells: {total_diff_bits}");
    println!("RESULT: FAIL");
    for f in failures.iter().take(20) {
        println!("  {f}");
    }
    ExitCode::FAILURE
}

fn source_ids(dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(dir.join("sources"))
        .expect("sources/ missing — run scripts/spike4-slice.ts first")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".rgba").map(str::to_owned)
        })
        .collect();
    ids.sort();
    ids
}

fn read_rgba(path: &Path, expect_len: usize) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert_eq!(bytes.len(), expect_len, "{} has unexpected length", path.display());
    bytes
}

fn spike4_native(dir: &Path) -> ExitCode {
    let out_dir = dir.join("native");
    fs::create_dir_all(&out_dir).expect("create native/");
    let ids = source_ids(dir);
    for id in &ids {
        let data =
            read_rgba(&dir.join(format!("sources/{id}.rgba")), SOURCE_SIZE * SOURCE_SIZE * 4);
        let artwork = Raster { width: SOURCE_SIZE, height: SOURCE_SIZE, data };
        for size in SIZES {
            let tile = render_slice_tile(&artwork, size);
            fs::write(out_dir.join(format!("{id}-{size}.rgba")), &tile.data).expect("write tile");
        }
    }
    println!("spike4 native: {} sources × {} sizes rendered → {}", ids.len(), SIZES.len(), out_dir.display());
    ExitCode::SUCCESS
}

// ---- compare -----------------------------------------------------------------

#[derive(Default)]
struct DiffStat {
    cells: usize,
    equal_cells: usize,
    diff_bytes: u64,
    total_bytes: u64,
    max_diff: u8,
    worst: Option<(String, usize, usize, usize, &'static str, u8, u8)>, // id,size,x,y,ch,ts,rust
}

const CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

impl DiffStat {
    fn record(&mut self, id: &str, size: usize, ts: &[u8], rust: &[u8]) {
        self.cells += 1;
        self.total_bytes += ts.len() as u64;
        let mut cell_equal = true;
        for (i, (&a, &b)) in ts.iter().zip(rust.iter()).enumerate() {
            if a != b {
                cell_equal = false;
                self.diff_bytes += 1;
                let d = a.abs_diff(b);
                if d > self.max_diff {
                    self.max_diff = d;
                    let px = i / 4;
                    self.worst =
                        Some((id.to_owned(), size, px % size, px / size, CHANNELS[i % 4], a, b));
                }
            }
        }
        if cell_equal {
            self.equal_cells += 1;
        }
    }
}

struct Cell {
    id: String,
    size: usize,
    lane: String,
}

fn read_cells(dir: &Path) -> Vec<Cell> {
    let tsv = fs::read_to_string(dir.join("cells.tsv")).expect("cells.tsv missing");
    tsv.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Cell { id: f[0].to_owned(), size: f[1].parse().unwrap(), lane: f[3].to_owned() }
        })
        .collect()
}

fn spike4_compare(dir: &Path) -> ExitCode {
    let cells = read_cells(dir);
    let mut failures: Vec<String> = Vec::new();

    // Gate 1 — byte-affecting fixture probes (encode curve, shadow tone, mask
    // bits) fail the run; decode-LUT f64 bit differences are MEASURED intel
    // (JSC Math.pow vs libm::pow ulp drift) — they only matter if they surface
    // in pixel bytes, which Gate 3 checks over the whole corpus.
    let fx = check_fixtures(dir, &mut failures);

    // Gate 2 — native ↔ wasm byte identity; Gate 3 — TS ↔ Rust byte diff.
    let mut nw_equal = 0usize;
    let mut per_lane: BTreeMap<String, DiffStat> = BTreeMap::new();
    let mut total = DiffStat::default();
    for c in &cells {
        let len = c.size * c.size * 4;
        let name = format!("{}-{}.rgba", c.id, c.size);
        let ts = read_rgba(&dir.join("ts").join(&name), len);
        let native = read_rgba(&dir.join("native").join(&name), len);
        let wasm = read_rgba(&dir.join("wasm").join(&name), len);
        if native == wasm {
            nw_equal += 1;
        } else {
            let at = native.iter().zip(wasm.iter()).position(|(a, b)| a != b).unwrap();
            failures.push(format!("native≠wasm: {name} first diff at byte {at}"));
        }
        per_lane.entry(c.lane.clone()).or_default().record(&c.id, c.size, &ts, &native);
        total.record(&c.id, c.size, &ts, &native);
    }

    // ---- summary table ----
    println!("== Spike 4 tri-target comparison ==");
    println!("sources: {}   cells: {} (sizes {:?})", cells.len() / SIZES.len(), cells.len(), SIZES);
    println!("native ↔ wasm byte-identical: {}/{} cells", nw_equal, cells.len());
    println!();
    println!("{:<14} {:>6} {:>12} {:>16} {:>9}", "TS ↔ Rust", "cells", "equal-cells", "diff-bytes", "max-diff");
    for (lane, s) in &per_lane {
        println!("{:<14} {:>6} {:>12} {:>16} {:>9}", lane, s.cells, s.equal_cells, format!("{}/{}", s.diff_bytes, s.total_bytes), s.max_diff);
    }
    println!("{:<14} {:>6} {:>12} {:>16} {:>9}", "TOTAL", total.cells, total.equal_cells, format!("{}/{}", total.diff_bytes, total.total_bytes), total.max_diff);
    if let Some((id, size, x, y, ch, ts, rust)) = &total.worst {
        println!("worst diff: {id}@{size} ({x},{y}) {ch}: TS={ts} Rust={rust}");
    }
    println!(
        "transcendental intel: decode LUT (JSC Math.pow vs libm::pow) {}/256 entries differ, max {} ulp — {}",
        fx.lut_diffs,
        fx.lut_max_ulp,
        if total.diff_bytes == 0 { "never surfaced in pixel bytes on this corpus" } else { "SEE PIXEL DIFFS" },
    );
    println!("byte-affecting probes (srgbEncode 4097 + shadow tone + mask bits): {} mismatches", fx.gate_diffs);

    let byte_equal = total.diff_bytes == 0;
    if nw_equal == cells.len() && byte_equal && failures.is_empty() {
        println!("\nRESULT: PASS — native↔wasm byte-identical AND TS↔Rust byte-identical");
        return ExitCode::SUCCESS;
    }
    // The ADR pixel gate for non-byte-equal slices is SSIM ≥ 0.995 — but this
    // slice targets byte equality; anything else is reported as FAIL with the
    // exact numbers so the owner re-prices instead of improvising (ADR-0019).
    println!("\nRESULT: FAIL");
    for f in failures.iter().take(20) {
        println!("  {f}");
    }
    ExitCode::FAILURE
}

#[derive(Default)]
struct FixtureReport {
    /// decode-LUT f64 entries whose bits differ (intel, not a gate).
    lut_diffs: usize,
    lut_max_ulp: u64,
    /// byte-affecting probe mismatches (srgbEncode bytes, shadow tone, mask bits).
    gate_diffs: usize,
}

/// Verify the TS-emitted fixture probes. Decode-LUT rows are compared
/// bit-for-bit but only MEASURED (ulp intel); the byte-affecting rows are
/// gates and push failures.
fn check_fixtures(dir: &Path, failures: &mut Vec<String>) -> FixtureReport {
    let path: PathBuf = dir.join("fixtures.tsv");
    let tsv = fs::read_to_string(&path).expect("fixtures.tsv missing");
    let mut report = FixtureReport::default();
    let mut masks: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for line in tsv.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "lut" => {
                let i: usize = f[1].parse().unwrap();
                let ts_bits = u64::from_str_radix(f[2], 16).unwrap();
                let rust_bits = srgb_decode(i as u8).to_bits();
                if ts_bits != rust_bits {
                    report.lut_diffs += 1;
                    report.lut_max_ulp = report.lut_max_ulp.max(ts_bits.abs_diff(rust_bits));
                }
            }
            "enc" => {
                let input = f64::from_bits(u64::from_str_radix(f[1], 16).unwrap());
                let ts_byte: u8 = f[2].parse().unwrap();
                let rust_byte = srgb_encode(input);
                if ts_byte != rust_byte {
                    report.gate_diffs += 1;
                    failures.push(format!("srgbEncode({input}): TS {ts_byte} ≠ Rust {rust_byte}"));
                }
            }
            "shadow" => {
                let s = field_shadow_tone(WHITE);
                let ts: Vec<u8> = f[1..4].iter().map(|v| v.parse().unwrap()).collect();
                if (s.r, s.g, s.b) != (ts[0], ts[1], ts[2]) {
                    report.gate_diffs += 1;
                    failures.push(format!(
                        "fieldShadowTone(WHITE): TS {:?} ≠ Rust {:?}",
                        ts,
                        (s.r, s.g, s.b)
                    ));
                }
            }
            "mask" => {
                let size: usize = f[1].parse().unwrap();
                let i: usize = f[2].parse().unwrap();
                let ts_bits = u64::from_str_radix(f[3], 16).unwrap();
                let mask = masks
                    .entry(size)
                    .or_insert_with(|| shape_mask(IconShape::Circle, size, size, 0.0, 0.0));
                let rust_bits = mask[i].to_bits();
                if ts_bits != rust_bits {
                    report.gate_diffs += 1;
                    failures.push(format!(
                        "mask[{size}][{i}]: TS {ts_bits:016x} ≠ Rust {rust_bits:016x}"
                    ));
                }
            }
            other => panic!("unknown fixture row {other} in {}", path.display()),
        }
    }
    report
}
