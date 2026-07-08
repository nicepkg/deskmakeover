# Icon Frontend Migration + Desktop Fidelity — Recon & Expert Panel Record

Date: 2026-07-09 · Mode: product-studio `feature` · Seats: PM / UX-Interaction / Visual Designer / Chief Engineer (isolated, same-vendor subagents) · Recon: 4 parallel agents (pipeline, win11-sim repos, Windows-icon domain, web image libs).

Owner request: (1) move icon editing/rendering to pure frontend with live preview + bidirectional sync incl. per-kind metadata; (2) raise desktop-preview fidelity (esp. taskbar) referencing piyushsuthar/windows-11-web + blueedgetechno/win11React; (3) local realistic messy icon mock pack for browser dev; (4) constraint: future tray-resident background auto-styling of NEW desktop icons (debounced, user-set) must remain possible.

## Recon facts (load-bearing)

1. **Portability**: `DeskMakeover.IconRendering/` is pure managed Rgba32 math, zero Win32/GDI+. ~70% maps onto pixi v8 core + pixi-filters; glass filter needs one custom SDF shader; analysis (silhouette/background classifiers) stays CPU TS. Toolkit: pixi-filters + culori (~30KB gz), optional pica (bake only). OpenCV.js/magick-wasm = lazy worker reserves only.
2. **Today's loop**: every knob change → C# re-renders N tiles × 2 PNGs → disk → custom-scheme `<img>` swap; 420ms config debounce (`stores/icons.ts:114-121`). Web is a dumb viewer. `mock-desktop.ts` (627 lines) already duplicates the pipeline approximately — two divergent renderers exist today.
3. **Apply is irreducibly C#**: IShellLink COM (.lnk), INI (.url), desktop.ini+attrs (folder), wrapper-.lnk+hide (loose exe/file), HKCU CLSID registry (Recycle Bin), elevated helper (arrow overlay). All with full restore metadata. UWP/MSIX icons have NO safe reversible override (preview-only, gray out). ICO assembly tested in C# (`IcoWriter`, uncompressed DIB frames).
4. **Discovery gap**: `ShellNamespaceDesktopItemSource.Scan()` returns empty — Recycle Bin (writer exists!), This PC, Network never reach the frontend. This PC/Network CLSID writers unwired.
5. **Key boundary fact**: `GeneratedIconStore.Save` renders ONE 256px master then linear-light downsamples to [256,48,32,24,20,16] via `IconResampler` before `IcoWriter`. Holding this boundary means web emits 256 masters only; parity pinned at 256 only; pica/sRGB resample risk void.
6. **Background operation**: C# bake chain (`DesktopBakeService.ApplyAsync`) runs headless from stored config today (whole-desktop, not incremental). No watcher/tray/startup infra exists (net-new). `AppSettings.KeepNewIconsStyled` plumbed but inert; legacy `MakeoverService.CatchUpAsync` dormant.
7. **Hidden-WebView2 rendering is unproven here**: `WebShellWindow.cs:66-69` sets no anti-throttling browser args; wallpaper bake runs on the visible main-thread pixi app (`stores/wallpaper.ts:368`), not a worker. Hidden/occluded WebView2 throttles rAF, backgrounds GPU, can lose WebGL context; Runtime 142 has a 4K canvas perf regression. Mitigation args exist but are unproven in-repo.
8. **Wallpaper precedent unvalidated**: F8 items (`wallpaper.getSource`/`applyBaked`, 5-look parity fixtures, C# renderer deletion) all pending Windows work; `WallpaperSession.cs` still on old Compose path.
9. **Win11 sim repos**: code licenses permissive (Apache-2.0 / CC0) — port structure freely; ALL bundled icon/wallpaper/font assets are extracted Microsoft property — dev-reference only, never ship, never commit. win11React taskbar is the faithful one (centered pinned row, running-app underline pills, tray cluster, show-desktop sliver, `saturate(3) blur(20px)` acrylic). Both repos' grid math is eyeballed flexbox — ours (Win32-metric-driven) is superior; keep ours. Mock drift found: mock Big=64 vs C# Big=96.
10. **Windows ground truth**: icon sizes 32/48/96; live cell spacing via `IFolderView2.GetSpacing` (we read it); size-switch reflow on packed desktops is undocumented + non-deterministic → mirror observed positions, never predict; re-scan after apply.
11. **Mock icon sources**: ship-safe = Fluent UI System Icons (MIT) + open Win11-style remakes + synthetic messy generator; encumbered (extracted packs, brand marks) = local dev only, never in git.

## Panel verdicts

**PM**: Direction A (web sole interactive renderer) — chosen for velocity + one-truth (Mac-unblocked icon visual iteration; retires the lying mock), stated honestly: the only user-visible gain is live preview, and C (incremental C# rendering) could fake most of it cheaply. Reject B (2× tax on the style vocabulary = the moat). Don't let the v1.2 background feature dictate v1.0 architecture. 30%-cut: faithful taskbar port, right-click view/sort menu, mock pack beyond ~40-60. Ranks fixing the discovery gap ABOVE preview latency.

**UX**: Do NOT copy wallpaper's direct-manipulation model — icons have no app-owned spatial verb (positions are Windows'). The renderer's real prize: instant discrete picks, 60fps color-wheel scrubbing, hover-preview (hover swatch = whole-desktop try-on, click commits, hover never snapshots undo). Undo: per-pick steps (no 1200ms window for discrete picks); gesture-bounded coalescing for color wheels only. Show un-editable items at full fidelity but gate the ⋯/context menu on `styleable` (today every tile gets fake affordances). Make per-tile exceptions visible (badge) + "clear all exceptions". Size preview = in-place scale + honesty line, never re-packed grid (mock currently lies). Right-click: app-styled menu with owned verbs only (icon size / refresh / per-tile keep-follow-tint); never clone Windows' Sort menu. `applyVersion` writes the real desktop with zero ceremony — must match apply's ceremony. Background auto-style trust contract: default OFF, invite in post-apply DoneCard, every run = first-class undoable history entry, "new since last visit" markers, always-visible kill-switch chip, first run is a PROPOSAL not a silent write.

**Designer**: Taskbar is the top realism-per-effort buy — real user icons above vs 5 candy placeholder squares below is the #1 tell. Bounded P0: (a) pinned row = neutral recognizable Fluent glyphs (Start/Search/Explorer/Edge/Store) 40×40 + running-indicator pills (::after, 6px gray open / 16px OS-blue active); (b) tray cluster (chevron + wifi/volume/battery pill + two-line tabular-nums clock + show-desktop sliver); (c) acrylic follows app theme (light `rgba(243,243,243,.82)` / dark `rgba(32,32,34,.72)`, `blur(20px) saturate(1.6-1.8)`), not white-on-dark-wallpaper; (d) Start flag fix (4 panes, 1.5px gaps, slight skew). SKIP: widgets/Copilot/flyouts/any interactivity. Stance: "looks like real Windows, doesn't impersonate YOUR taskbar"; OS-blue only, no coral in the OS layer. Label: double text-shadow. Selection: white wash + 1px dotted. Mock pack distribution table (30% flat / 15% skeuo / 12% photo / 10% badged / 12% transparent-edge / 8% letter / 8% folders / 5% docs) + 11 generator randomization axes (worst case: pre-baked rounded corners → double-rounding). Style quality: Glass = hero; Pixel = weakest, off-brand; Satin/Arc too faint to pass the 3s-read gate at 48px. Proposal: unify icon materials with wallpaper's five-material vocabulary (Frost/Luminous/Solid/Halo/Outline) — one system across modules.

**Engineer**: **Sharpened A** — web (pixi) = authoritative interactive renderer emitting 256px RGBA masters only; C# keeps IconResampler ladder + IcoWriter + all shell writers unchanged; **C# TileRenderer NOT deleted** — frozen as parity oracle + reserved background renderer; new styles ship TS-only until background mode exists (no 2× tax while frozen). Do not bet unattended real-desktop writes on hidden WebView2 (evidence above). Bridge: sources = 256px PNGs via WebAssets once per scan (~3-6MB, 300 icons, cached; recycle bin = 2 sources per item); preview crosses nothing; apply = PNG-encoded masters chunked (~5-7MB; raw 78.6MB won't fit one JSON postMessage; SharedBuffer is host→web only). Parity: goldens at 256 only, ~15-20 source canvases × sampled ~60-100 style cells; tolerance bands (flat ΔE<2/SSIM>0.995; filters SSIM>0.98, no bit-exactness). Perf: preview <16ms@50 / <50ms@300 tiles; 300-icon apply 20-30s, shell-I/O-bound regardless of renderer. Migration: port math → converge/delete mock-desktop.ts → flat parity → filters (glass SDF hardest) → applyBaked behind flag → background later on C#. Flag: prove wallpaper A on real Windows before/alongside.

## Cross-seat clashes (owner disposition needed)

1. Taskbar: PM cut ("fidelity theater") vs Designer P0 (top realism-per-effort, bounded).
2. Right-click menu: PM cut vs UX bounded owned-verbs menu vs owner's original clone-Windows-menu idea.
3. Mock pack size: PM 40-60 vs Designer 100-300 distribution (generator makes count cheap).
4. C# renderer fate: PM "defer decision to v1.2" vs Engineer "keep frozen indefinitely, decide with hardware evidence".

## Owner dispositions (2026-07-09, all approved as recommended)

| Q | Decision | Disposition |
|---|---|---|
| Q1 | Sharpened A: web interactive renderer + 256 master boundary; C# TileRenderer frozen (oracle + reserved background renderer); new styles TS-only | **accept** → ADR-0015 D1-D3 |
| Q2 | Filter WYSIWYG = visual tolerance (flat ΔE<2/SSIM≥0.995; filters SSIM≥0.98), no bit-exactness | **accept** → D5 |
| Q3 | Keep building on Mac; batch ALL Windows-gated validation (wallpaper F8 + icon parity + applyBaked host + discovery) into one session | **accept** → D6 (engineer's serialize-first preference overruled) |
| Q4 | 300-icon apply 20-30s accepted for v1 + progress UI; incremental apply arrives with the background ledger | **accept** |
| Q5 | Background auto-format trust contract (default OFF, DoneCard invite, undoable history entries, new-icon markers, kill-switch chip, first-run proposal, never override user exceptions; next-open audit primary) | **accept** → spec 06 §7 (build = v1.2) |
| Q6 | Taskbar: designer bounded P0 (PM's cut overruled by owner) | **accept** → D7 |
| Q7 | (a) neutral recognizable glyphs, not user's real pinned apps; (b) taskbar stays original-Windows scenery, styling never applies to it | **accept** |
| Q8 | Right-click: app-styled owned verbs only (tile keep/follow/tint; canvas icon-size + refresh); Sort never; no Win32-lookalike chrome | **accept** → D8 |
| Q9 | Mock pack: Fluent-style own art + synthetic generator in git; encumbered assets dev-local only; ~120 icons | **accept** → D9 |
| Q10 | Five-material unification (磨砂/柔光/实色/悬浮/描边 on icons): approved as direction, sequenced AFTER migration as first TS-only styles; 柔光 takes 像素's curated slot (像素 demoted); 缎光/珐琅弧 heavied or demoted | **accept** → D10 |
| Q11 | Discovery fix (Recycle Bin surfacing + This PC/Network CLSID writers) joins the Windows batch | **accept** |
| Q12 | (a) Recycle Bin styleable: yes; (b) 60s arrow gate softened → one-time explainer + 8s pause; (c) size preview = in-place scale + honesty caption + post-apply rescan | **accept** |

Standing owner orders for the run: write ALL docs before building; codex review
afterwards with verification (fix only real findings — design-as-intended stands);
C# frozen code gets banner comments + doc record; visual acceptance mandatory;
no legacy compat; delete permanently-dead code on sight.
