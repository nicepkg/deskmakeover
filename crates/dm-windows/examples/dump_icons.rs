//! Debug harness: scan the live desktop and write EACH item's raw extracted source PNG(s)
//! to `%TEMP%\dm-icon-dump\` so the extraction output can be inspected directly (independent
//! of the web compositor). Read-only. Run: `cargo run -p dm-windows --example dump_icons`.

#[cfg(windows)]
fn main() {
    use std::sync::Arc;

    use dm_domain::ports::{DesktopScanner, IconSourceExtractor};
    use dm_windows::{StaExecutor, WindowsIconSourceExtractor, WindowsScanner};

    let exec = Arc::new(StaExecutor::spawn().expect("STA spawn"));
    let scanner = WindowsScanner::new(exec.clone());
    let extractor = WindowsIconSourceExtractor::new(exec.clone());

    let out = std::env::temp_dir().join("dm-icon-dump");
    std::fs::create_dir_all(&out).expect("mkdir dump");

    let items = scanner.scan().expect("scan");
    println!("scanned {} items → {}", items.len(), out.display());

    for it in &items {
        match extractor.extract(it, None) {
            Ok(imgs) => {
                for (i, im) in imgs.iter().enumerate() {
                    let safe: String = it
                        .name
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect();
                    let file = out.join(format!("{:?}_{}_{}.png", it.kind, safe, i));
                    std::fs::write(&file, &im.png).expect("write png");
                    // Sniff the corner + center alpha so we can spot an opaque-square source
                    // without opening every file.
                    let (w, h, corner_a, center_a, opaque_px) = probe(&im.png);
                    println!(
                        "  [{:?}] {:<28} {}x{}  corner_alpha={} center_alpha={} opaque%={:.0}",
                        it.kind, it.name, w, h, corner_a, center_a, opaque_px
                    );
                }
            }
            Err(e) => println!("  [{:?}] {:<28} EXTRACT ERR: {e:?}", it.kind, it.name),
        }
    }
    println!("done — open {} to inspect", out.display());

    // Decode a PNG and report dims + the alpha of the top-left corner + the center pixel + the
    // fraction of fully-opaque pixels. An icon that should be transparent-cornered but reads
    // corner_alpha=255 with a high opaque% is the "white square" artifact.
    fn probe(png: &[u8]) -> (u32, u32, u8, u8, f64) {
        let img = image::load_from_memory(png).expect("decode");
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let corner = rgba.get_pixel(0, 0)[3];
        let center = rgba.get_pixel(w / 2, h / 2)[3];
        let opaque = rgba.pixels().filter(|p| p[3] == 255).count();
        (w, h, corner, center, opaque as f64 * 100.0 / (w as f64 * h as f64))
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("dump_icons is Windows-only.");
}
