# DeskMakeover Roadmap

Living document: edit in place as versions ship; history lives in CHANGELOG + git
tags. Standing rule (ADR-0002): every new capability folds into the default
result or lives in settings — the primary flow (one CTA) never gains steps.

> **Re-sliced 2026-07-10.** The old "v1.0 prototype-parity (icons only) → v1.1
> rail+wallpaper" train is void: the prototype-parity build was replatformed to
> WebView2+React (ADR-0011), redesigned twice (ADR-0012 → ADR-0013 v3), and the
> renderers inverted into the web (ADR-0014/0015) — all BEFORE anything shipped.
> Nothing has been released; the version is `Unreleased` until the owner names
> the first number at release time. Prototype parity is NOT a release gate.

## Unreleased → 首个公开版 (the first public release)

**What ships:** the v3 "Premium Flat" app — ONE window with the module rail:
**图标** (11-shape catalog, colour treatments, marks, filters, per-icon overrides,
kindPolicy, history/restore) + **壁纸** (zone editor: five materials, four title
styles, clarity dim, import/export, backup/one-click return) + **设置** (theme /
language / local data / about + changelog). zh-Hans + English, light-first
following the system. Specs 01-06 describe it; `docs/STATE.md` tracks it.

**Two v1 additions (owner, 2026-07-10):** 后台常驻自动美化 (background resident
auto-format, spec 07 + ADR-0020) and the global transparent shortcut-arrow
default (ADR-0021) ship in the first release.

**Where it stands:** the web side is COMPLETE and green in the browser/mock loop
(356 tests). **The old "F8 Windows host pass" is VOID** — the product replatforms
to Tauri 2 + Rust (ADR-0019: the C# host never left schema 1 and its bridge was
WebView2-specific; the panel priced finish-F8-then-migrate as the worst path).
The remaining distance to release is the migration plan
`docs/plans/2026-07-10-tauri-migration.md` (M0 freeze → M1 go/no-go spikes → M2
Tauri foundation → M3 vertical slice → M4 platform breadth ∥ M5 Rust icon core →
M6 single-truth cutover (WASM+native, TS pixel code deleted after certification)
→ M7 resident mode → M8 release engineering + .NET deletion).

**Exit gate:** bun + cargo tests green · TS↔Rust parity corpus certified +
wasm↔native byte-equality CI · kill-point recovery + spec 07 resident battery ·
fresh-VM matrix (apply → reboot → restore, zero residue; Win10 22H2/Win11) ·
owner-supervised live runs (manual + resident) · signed exe passes SmartScreen
(OV/individual cert — owner) · .NET tree deleted (`last-dotnet` tag) · owner
names the version number · repo made public (it exists, currently PRIVATE).

> Platform guardrails (ADR-0004) stand: one hero action forever; modules are an
> exclusion checklist, not a toolbox; risk tiers cold/warm/hot with the global
> action never touching hot; cleaners/accelerators and global file-type
> association icons permanently banned.

## Next (post-first-release candidates — owner picks and numbers them)

- ~~新图标自动美化 (keep-up)~~ — **promoted into v1** (ADR-0020, spec 07; native
  Rust renderer, reconcile-led watcher, incremental-ledger restore). Post-v1
  candidates here are the EXTENSIONS: silent file-wrapping promotion, Shell-level
  virtual-item coverage, machine-level public-desktop mode.
- **系统净化 module** (HKCU one-shot, warm tier, no elevation): start-menu
  recommendations, lock-screen Spotlight tips, Explorer promotions, settings
  suggestions, advertising ID, search highlights. Toggle list + 并入一键美化 +
  honest footer 「不是清理软件…」. Requires the `Modules.Contracts` host
  refactor first (the rail today is a lightweight switch).
- 「整理到分区」 icon auto-placement experiment (explicit, previewable,
  journaled) — candidate per ADR-0009 §6.
- Trust hardening: AV whitelist submissions (Microsoft/360/火绒), installer +
  emergency restore entry, multi-monitor / mixed-DPI edge coverage.

## Later — 第二战场 (Explorer)

- **资源管理器 module**: folder icons (desktop.ini) + drive icons (registry
  DriveIcons only — autorun.inf never), same shape×colour engine, per-surface
  toggles, full restore.
- Global file-type-association icons: **permanently out** (constitution).

## Later — 代差 (moat)

- **壁纸 curated sets**: curated wallpaper sets matched to the icon look —
  auto-backup + one-click revert, never replaced without consent.
- AI icon generation (`IIconGenerator` extension point reserved); whole-desktop
  unified colour-filter style packs; EV cert when volume justifies.

## Standing open questions

- First release version number + name (owner — release time).
- Signing entity/name for the OV certificate (owner; release gate).
- Repo visibility: `nicepkg/deskmakeover` exists but is PRIVATE — make public at
  release (the About card and the 免费开源 chip promise it).
- Distribution channel (direct download + pinned comment reply).
