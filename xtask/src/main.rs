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

use dm_icon_core::color::{field_shadow_tone, srgb_decode, srgb_encode};
use dm_icon_core::raster::{shape_mask, Raster, WHITE};
use dm_icon_core::shapes::IconShape;
use dm_icon_core::slice::render_slice_tile;

/// The Spike-4 slice sizes — must match scripts/spike4-slice.ts SIZES.
const SIZES: [usize; 2] = [256, 512];
const SOURCE_SIZE: usize = 256;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, dir] if cmd == "spike4-native" => spike4_native(Path::new(dir)),
        [cmd, dir] if cmd == "spike4-compare" => spike4_compare(Path::new(dir)),
        _ => {
            eprintln!("usage: xtask spike4-native <dir> | spike4-compare <dir>");
            ExitCode::from(2)
        }
    }
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
                    .or_insert_with(|| shape_mask(IconShape::Circle, size, size, 0, 0));
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
