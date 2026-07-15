//! Read-only [WINDOWS-VERIFY] harness — exercises every non-mutating Windows platform
//! path against the LIVE desktop and prints a pass/fail report. Safe to run on a real
//! logged-in box: it never writes the desktop, the registry, or a wallpaper (the
//! owner-supervised icon-bake / wallpaper-apply gates are untouched).
//!
//! Run: `cargo run -p dm-windows --example verify_readonly`
//!
//! Covers the read-only side of the ship-readiness `[WINDOWS-VERIFY]` battery: STA actor
//! (#1), known folders via the scanner (#2), scan+classify (#3), IDesktopWallpaper topology
//! + wallpaper capture (#8), desktop geometry/positions (#9), and icon source extraction.

#[cfg(windows)]
fn main() {
    use std::sync::Arc;

    use dm_domain::item::ItemTarget;
    use dm_domain::ports::{
        DesktopGeometryReader, DesktopScanner, IconSourceExtractor, ItemStateReader,
        MonitorTopology, WallpaperApplier,
    };
    use dm_windows::{
        StaExecutor, WindowsDesktopGeometry, WindowsIconSourceExtractor, WindowsMonitorTopology,
        WindowsScanner, WindowsStateReader, WindowsWallpaper,
    };

    println!("== DeskMakeover read-only Windows verification ==\n");

    // 1. STA actor — the single COM apartment thread every adapter runs on.
    let exec = match StaExecutor::spawn() {
        Ok(e) => {
            println!("[PASS] StaExecutor::spawn — COM STA thread up");
            Arc::new(e)
        }
        Err(e) => {
            println!("[FAIL] StaExecutor::spawn: {e:?}");
            return;
        }
    };

    // 2. Monitor topology via IDesktopWallpaper (the CLSCTX_ALL activation).
    let topo = WindowsMonitorTopology::new(exec.clone());
    match topo.enumerate() {
        Ok(t) => {
            println!(
                "[PASS] MonitorTopology::enumerate — {} monitor(s), position {:?}",
                t.monitors.len(),
                t.position
            );
            for m in &t.monitors {
                println!(
                    "        · {} {}x{} @({},{}) source={} slideshow={}",
                    m.name,
                    m.bounds.w,
                    m.bounds.h,
                    m.bounds.x,
                    m.bounds.y,
                    m.source_path.as_deref().unwrap_or("<none>"),
                    m.slideshow_active
                );
            }
        }
        Err(e) => println!("[FAIL] MonitorTopology::enumerate: {e:?}"),
    }

    // 3. Wallpaper capture — the pre-apply restore snapshot (read only, nothing set).
    let wp = WindowsWallpaper::new(exec.clone());
    match wp.capture() {
        Ok(s) => println!(
            "[PASS] WallpaperApplier::capture — bg=0x{:06X} pos={} slideshow={} monitors={}",
            s.background_color,
            s.position,
            s.slideshow_active,
            s.monitors.len()
        ),
        Err(e) => println!("[FAIL] WallpaperApplier::capture: {e:?}"),
    }

    // 4. Desktop scan — enumerate + classify (also proves SHGetKnownFolderPath resolves the
    //    user + public desktop roots, [WINDOWS-VERIFY] #2/#3).
    let scanner = WindowsScanner::new(exec.clone());
    let items = match scanner.scan() {
        Ok(items) => {
            println!("[PASS] DesktopScanner::scan — {} item(s)", items.len());
            for it in items.iter().take(15) {
                println!("        · [{:?}] {}  ({})", it.kind, it.name, it.path);
            }
            items
        }
        Err(e) => {
            println!("[FAIL] DesktopScanner::scan: {e:?}");
            Vec::new()
        }
    };

    // 5. Desktop geometry + live icon positions (both degrade gracefully → WARN, not FAIL).
    let geo = WindowsDesktopGeometry::new(exec.clone());
    match geo.geometry() {
        Ok(g) => println!(
            "[PASS] DesktopGeometryReader::geometry — {}x{} taskbar={}",
            g.screen_width, g.screen_height, g.taskbar_height
        ),
        Err(e) => println!("[WARN] DesktopGeometryReader::geometry: {e:?} (host falls back to synthetic grid)"),
    }
    match geo.positions() {
        Ok(p) => {
            println!("[PASS] DesktopGeometryReader::positions — {} slot(s)", p.len());
            for s in p.iter().take(8) {
                println!("        · {} @({},{})", s.name, s.x, s.y);
            }
        }
        Err(e) => println!("[WARN] DesktopGeometryReader::positions: {e:?} (host falls back to synthetic grid)"),
    }

    // 6. Icon source extraction + fingerprint read on the first styleable item.
    if let Some(item) = items.iter().find(|i| i.kind.is_styleable()) {
        let extractor = WindowsIconSourceExtractor::new(exec.clone());
        match extractor.extract(item, None) {
            Ok(imgs) => {
                println!(
                    "[PASS] IconSourceExtractor::extract('{}') — {} image(s)",
                    item.name,
                    imgs.len()
                );
                for (i, im) in imgs.iter().enumerate() {
                    println!("        · [{i}] {}x{} png={} bytes", im.width, im.height, im.png.len());
                }
            }
            Err(e) => println!("[FAIL] IconSourceExtractor::extract('{}'): {e:?}", item.name),
        }

        let reader = WindowsStateReader::new(exec.clone());
        let target = ItemTarget {
            id: item.id.clone(),
            kind: item.kind,
            path: item.path.clone(),
        };
        match reader.read_fingerprint(&target) {
            Ok(fp) => println!("[PASS] ItemStateReader::read_fingerprint('{}') — {fp:?}", item.name),
            Err(e) => println!("[WARN] ItemStateReader::read_fingerprint('{}'): {e:?}", item.name),
        }
    } else {
        println!("[INFO] no styleable item on the desktop to extract/fingerprint");
    }

    // 7. `.url` (UrlShortcut) styleability — the encoding-regression check (owner report
    //    2026-07-15: Steam writes `.url` as UTF-16 LE; the old UTF-8 read_to_string errored, so
    //    read_fingerprint failed → degraded → styleable:false → the tile ignored every config
    //    change). Reproduce scan.rs's EXACT `styleable` derivation per `.url` item.
    let url_items: Vec<_> =
        items.iter().filter(|i| i.kind == dm_domain::ItemKind::UrlShortcut).collect();
    if url_items.is_empty() {
        println!("\n[INFO] no `.url` items on this desktop to check");
    } else {
        println!("\n== `.url` styleability (Steam UTF-16 regression) ==");
        let extractor = WindowsIconSourceExtractor::new(exec.clone());
        let reader = WindowsStateReader::new(exec.clone());
        for item in url_items {
            let target =
                ItemTarget { id: item.id.clone(), kind: item.kind, path: item.path.clone() };
            let fp = reader.read_fingerprint(&target);
            let extract = extractor.extract(item, None);
            // scan.rs: degraded_reason is Some when the fingerprint is unreadable OR extract fails;
            // styleable = can_style() && degraded_reason.is_none().
            let degraded = fp.is_err() || extract.is_err();
            let styleable = item.can_style() && !degraded;
            let tag = if styleable { "PASS" } else { "FAIL" };
            println!(
                "[{tag}] {}  styleable={}  fingerprint={}  extract={}  icon_ref={:?}",
                item.name,
                styleable,
                match &fp {
                    Ok(_) => "ok".to_string(),
                    Err(e) => format!("ERR({e:?})"),
                },
                match &extract {
                    Ok(imgs) => format!("{}img", imgs.len()),
                    Err(e) => format!("ERR({e:?})"),
                },
                item.icon.as_ref().map(|r| format!("{}#{}", r.location, r.index)),
            );
        }
    }

    println!("\n== done ==");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("verify_readonly is Windows-only; nothing to do on this host.");
}
