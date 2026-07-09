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

**Where it stands:** the web side is COMPLETE and green in the browser/mock loop
(297 tests). The remaining distance to release is **F8 — the Windows host pass**
(STATE.md §F8 is the authoritative list):

1. Host → bridge schema 3 (`wallpaper.getSource/applyBaked/...`, chunked
   `icons.applyBaked*`, `icons.scan` v2 with sourceUrls) + host-side error capture.
2. Golden parity fixtures (web renderer vs frozen C# oracle) + legacy renderer
   deletions (IconStyler, WallpaperBakeRenderer/Composer, …).
3. resx i18n sweep (PENDING-RESX markers → Strings*.resx → regenerate TS).
4. Release packaging made REAL (publish.ps1 currently App-only: no
   ElevatedHelper, no web build — unverified; see development.md §5).
5. `dotnet test` re-verified + real-host checks (fonts/IME/DPI/125% hairlines).

**Exit gate:** web + dotnet tests green (0 warnings) · parity fixtures pass ·
fresh-VM smoke (apply → reboot → restore, zero residue) · owner-supervised live
runs (icon bake + wallpaper apply; checklist pending F8 rewrite) · signed exe
passes SmartScreen (OV/individual cert — owner) · owner names the version
number · repo made public (it exists, currently PRIVATE).

> Platform guardrails (ADR-0004) stand: one hero action forever; modules are an
> exclusion checklist, not a toolbox; risk tiers cold/warm/hot with the global
> action never touching hot; cleaners/accelerators and global file-type
> association icons permanently banned.

## Next (post-first-release candidates — owner picks and numbers them)

- **新图标自动美化 (keep-up), done honestly** — a real watcher / app-launch
  catch-up consuming `keepNewIconsStyled`; the setting un-hides and the C# side
  renders via the reserved background renderer with the spec 06 §7 trust
  contract (default OFF, first-run proposal, undoable history entries).
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
