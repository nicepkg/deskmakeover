# Spec 08 — 清爽 Module (Calm Windows)

Living spec (ADR-0023; panel `docs/reviews/2026-07-13-calm-windows-panel.md`).
**Capability truth** — which controls exist, their tiers, registry recipes, certification
and transaction contracts — lives in `docs/references/windows-settings-rust/README.md` and
is NOT duplicated here; this spec owns the product behaviour. Visuals are governed by
spec 02 v3; shell placement by spec 03.

## 1. Identity & scope

清爽 (rail label) / **清爽系统** (full name) helps the user quiet the surfaces where Windows
pushes content — Start promoted recommendations, search-box highlights, notification and
Settings suggestions, the widgets feed, taskbar chrome — reversibly, and honestly about what
it can and cannot do on THIS machine. Verb register: 收起打扰 / 让它安静 under the 美颜
umbrella. It is never a cleaner: banned copy 净化/清理/优化/加速/扫描/problem-counts.

Non-scope (v1): machine-level HKLM policies; per-item-consent back room for non-evaluable
controls (ad ID, Device Usage — ADR-0023 decision 5); the Direction-A noise-map canvas
(north star, post-v1); resident self-heal for tweaks. `Start_TrackDocs` and UCPD are
constitutionally untouchable.

## 2. IA (Direction B — the v1 form)

4th rail tile (above the pinned 设置). Shortcuts stay positional: Ctrl+1 图标 · Ctrl+2 壁纸 ·
Ctrl+3 清爽 · Ctrl+4 设置 (spec 03 §1/§5 amended accordingly). The module replaces the work
area like 设置 does (no desktop-mirror canvas in v1): a calm full page, page-scale type,
using the grouped inset-card grammar. Full-page modules (清爽, 设置, any future page) share
the ONE `FullPage` shell (`src/components/shell/full-page.tsx`): same max width, padding and
header rhythm, so the h1 sits at the identical position on every page (owner 2026-07-13 —
visual unity is a trust signal). Anatomy top-to-bottom:

1. **Hero strip** — module title + one-line honest promise + the hero CTA 「一键清爽」
   (coral-ink solid; fires automatic-certified switches ONLY). After a successful apply the
   strip shows the verified summary (「已让 N 处安静」) + 「恢复系统推送」 text-button.
2. **Group 1 「一键就能帮你关的」** — automatic-certified rows (exclusion toggles, default
   ON in the package; a row excluded by the user is remembered).
3. **Group 2 「带你去系统里关的」** — guided rows (NEVER toggles): label + one-line why +
   「带我去关」 chip. The widgets-feed row sits FIRST and is styled as the opening act.
4. **Group 3 「这个 Windows 版本暂时不碰的」** — fail-closed / managed rows, collapsed by
   default, quiet (tertiary text, existing disabled-slot grammar), each with a reason line.
   Never rendered as dead toggles.

Groups render only when non-empty. Row grammar: per-row mini-screen schematic (104×64 SVG,
`SurfaceSchematic`) + label + status chip (right) + optional caption line (collateral
disclosures, restart notes, asymmetry warnings). Rows in groups 1–2 carry a per-row 「恢复」
chip once the write is owned (`verified`/`setAwaiting`).

### 2.1 Schematic contract (W0.6-viz, acceptance-passed 2026-07-13)

The schematics are drawn AGAINST REAL Win11 screenshots (never from imagination; reference
set archived in the session scratchpad, pixel truths inlined as comments in
`src/components/calm/scenes.tsx`). Laws — each verifiable against the source:

- **One coral highlight per frame** marks the operation area; `done` renders NO ghost
  outline — the honest after-state is the clean surface, the row chip carries the receipt.
- **No hollow sockets**: when noise leaves a surface, siblings REFLOW into the freed space
  (`ReflowGroup`) or the panel RESIZES to hug remaining content (`ShrinkRect`), exactly like
  the real desktop compacts. A faded element must never leave a phantom gap or width.
- **The schematic is a view of `CalmRowState`** — it can never run ahead of verification;
  noise exits only on `verified`/`confirmedOff`/`userAttested`/`quiet`; `setAwaiting` dims
  in place.
- **Copy and picture must agree**: what the copy promises stays (e.g. 开始菜单 keeps the
  your-files row because the description says 「你自己的常用文件保留」).
- **No establishing hero image** — it would hard-code a starter count the user can falsify.
- Motion respects `prefers-reduced-motion` (fade-only / duration-0 fallbacks); OS-authentic
  mirror hexes `#0067C0`/`#4CC2FF` are the reviewed banned-colors exception set.
- **Type ladder** (owner 2026-07-13, designer-verified): hero count line 18px medium >
  group headers 16px medium > row names 14px medium > descriptions 12px `t3` > meta 11px
  `t3`. Explanatory sentences live in the faded scan layer at FULL `t3` (no opacity
  stacking — it falls below WCAG AA); names and numbers carry the page.
- **Taskbar schematic axis**: the centred cluster sits on the weather↔tray visual midpoint
  (x≈54.5 in the 104 frame) at rest AND after every reflow — survivors shift half the freed
  width from each side, mirroring the real centre-aligned taskbar.

## 3. Per-control state machine

```
unknown → probed ∈ { quiet (already off) | pushing (on) | unsupported(reason) | managed(org) }
pushing --[apply]--> pending(写入中/验证中, shimmer) --[raw ok + delayed ok + effect ok]--> verified 已生效
                                        └--[needs sign-out/restart]--> set-awaiting 已设置·重启后生效
                                        └--[verify fail / rollback]--> reverted + honest toast
verified --[healthcheck drift, in-cert-boundary]--> re-closed (notice) | re-proposed
verified --[feature update crossed boundary]--> needs-reconfirm 需重新确认 (never auto-replay)
any --[external conflict on restore]--> kept + skip-with-reason (mark, don't clobber)
guided rows: pushing → user walked → re-probe on window refocus →
   readable state: confirmed-off 已确认关闭 | still-on (带我再去一次)
   unreadable state: user-attested 你手动关的 (never counted as ours)
```

`已生效` NEVER lights on write success alone (research contract step 8). 「已帮你做的事」
counts verified writes only.

## 4. Hero flow 「一键清爽」

Explain-before-apply via the shared ConfirmSheet: what will change (the certified list, by
name), what will NOT (guided items named honestly — "小组件资讯需要你自己关，我带你去"),
all per-user + no UAC. Apply = snapshot → journaled writes → per-row pending → verification →
summary. Celebration: once-per-launch confetti only if this is the launch's first module
success; otherwise completion toast + 「去看看开始菜单」. Partial success is stated plainly
(N verified, M awaiting sign-in, K skipped with reasons). A second click with nothing
applicable = 「没有需要关的了」, never a fake re-run.

## 5. Degraded honesty (fail-closed / managed / EEA)

Framing law: the restraint is OURS — "为了不动坏东西，这几项我们还没在你的 Windows 版本上
验证过，暂时不碰" — never blame the machine or the user, never a red state, never a security
palette. Managed devices: 「由你的组织管理，无法更改」+ a "为什么" expander. EEA
search-provider surfaces: route to the official Windows settings entry. Every degraded row
keeps an exit (guided deep-link where one exists).

## 6. HealthCheck (drift) — re-detect + re-propose

On launch (and module entry) re-probe. In-boundary drift (value flipped back, same certified
environment): re-close silently is FORBIDDEN; show the additive notice 「Windows 更新后，有
N 项推送又打开了 [重新关闭]」 (one click re-applies through the same verify pipeline).
Boundary crossing (feature update, new build family): rows drop to 需重新确认; nothing is
replayed. This module's HealthCheck therefore differs from the pixel modules' re-apply —
recorded in ADR-0004 Amendment 2026-07-13.

## 7. Restore

「恢复系统推送」 = set every DeskMakeover-written value back to its recorded original
(absent → delete, per the research restore contract). Asymmetries are disclosed inline, not
hidden: drifted values are kept + reported (「M 项你自己改过，未动」); the restore DoneCard
mirrors the icons skip-with-reason grammar. Restore never guesses a Windows default and
never deletes non-empty app-created keys.

## 8. Default-package admission rule (binding)

A control enters the default one-click package only if ALL hold: **(a) user-perceivable
effect · (b) programmatically verifiable effect · (c) zero-residue reversible · (d) any
legitimate-content collateral disclosed on the row face**. Starter write slice (v1
candidate, capability-gated per ADR-0023 decision 6): `SearchboxTaskbarMode` (任务栏搜索框),
`ShowTaskViewButton` (任务视图按钮), `Start_IrisRecommendations` (开始菜单推荐).
Next-in-line as lab rows land: search highlights, notification/settings suggestions,
welcome/finish-setup, sync-provider notifications (ships with its collateral caption:
「也会隐藏网盘同步的提示通知」). Excluded from default: advertising ID (fails a+c),
Device Usage (advanced/empty allowlist). Guided (never writes): widgets feed/hover/badges/
announcements, taskbar Widgets button, lock-screen status, system tray.

## 9. Copy precision table (binding, test-gated)

| Surface | MUST say | MUST NOT say |
|---|---|---|
| 任务栏搜索框 | 隐藏任务栏上的搜索框（Win+S 搜索仍可用） | 关闭搜索 |
| 任务视图按钮 | 隐藏任务栏上的任务视图按钮（Win+Tab 仍可用） | 关闭任务视图 |
| 开始菜单推荐 | 减少开始菜单里的推广推荐 | 移除开始菜单所有推广/账号提示 |
| 广告 ID (back room only) | 减少个性化追踪；重新打开会生成新的广告 ID | 减少广告数量 |
| 同步提供程序通知 | 关闭资源管理器里的推广通知；也会隐藏网盘同步的提示通知 | （隐藏 collateral） |
| 锁屏 tips (advanced) | 减少锁屏上的小贴士 | 保留 Spotlight 图片但去掉内容 |
| Restore | 恢复系统推送 / 调回 Windows 默认 | 完整还原/复原 |

Module-wide bans: 净化 清理 优化 加速 扫描 问题计数 · plus the global bans (dashes,
快照/注册表/HKLM/journal in user copy — spec 01). Enforced by a copy-gate test over the
i18n dictionaries (same pattern as `tests/banned-colors.test.ts`).

## 10. Visual language

Everything inherits spec 02 v3: coral `#FF6F5E` accents only; verified = coral-ink check;
attention = the reserved amber, sparingly; NO green-shield/red-alert/blue-guard security
palette ever. Rail glyph: a coral craft glyph in the 微风/sparkle-sweep family, 16px keyline,
never a shield, never OS blue. Pending rows reuse the cta-working shimmer family. Group 3
uses the existing 40%-opacity disabled-slot grammar. No screenshots of the user's own
Windows anywhere in v1.

## 11. Safety rules (module addendum)

All writes per-user HKCU, no elevation, batched in one journaled transaction; fail-closed on
any uncertified tuple; never write policy keys, never delete management guards, never kill
Explorer/SearchHost/StartMenuExperienceHost/Widgets silently; activation-pending is reported,
with the documented sign-out route. Apply/restore are user-clicked only.

## 12. Engineering split (pointer)

Per the research README production layout: `dm-domain/system-tweaks` (IDs, environment
snapshot, states, anchors, ports) · `dm-operations/system-tweaks/` (catalog evaluation, WAL/
ledger, apply/restore/recovery driver, fakes) · `dm-windows/system-tweaks/` (winreg backend,
probes, refresh/Settings adapters) · `dm-contracts` DTOs (schema bump when wired) ·
`src-tauri` composition + a devhost fake for the Mac loop. Reference crates are copied by
boundary, never runtime dependencies. Build order: `docs/plans/2026-07-13-calm-windows-module.md`.

## 13. Acceptance

- Rail shows 4 tiles; Ctrl+4 switches; module page renders the three groups from a probed
  catalog; empty groups absent.
- Guided rows have no toggle affordance anywhere in the accessibility tree.
- A row reaches 已生效 only through the pending→verified pipeline (unit-tested state machine).
- Copy-gate test fails on any banned word or precision violation; banned-colour gate stays green.
- Degraded rows always render reason + exit; zero red/security styling.
- Designer-seat pixel acceptance on real renders before the visual work is called done
  (owner rule 2026-07-10); codex adversarial review over the module diff before merge.
