# Panel record: where does "restore the native shortcut arrow" live?

**Date:** 2026-07-11 · **Seats:** chief-PM + chief-UX/UI (isolated, independent, cross-compared by the lead)
**Question (owner):** the global transparent-arrow overlay (ADR-0021, machine-wide HKLM `Shell Icons\29`)
means shortcuts created OUTSIDE DeskMakeover's managed set show no arrow. Users may think Windows broke.
Where does the restore capability go, and how do we avoid complaints?

## Consensus (both seats, independently)

1. **Reframe: discoverability is the least important dimension.** The user who hits the pain does not
   search for "restore arrow" — he thinks "Windows broke" and cannot attribute it to a beautifier app
   he ran days ago (attribution break). Rescue happens **before** the pain, via disclosure, not after.
2. **Layered system, not one button:**
   - **Lifecycle auto-restore** — uninstall/disable restores the overlay automatically, zero residue
     (ADR-0021 §4 already mandates; both seats rate it non-negotiable, the worst trust collapse is
     "uninstalled and arrows are still gone").
   - **First-apply disclosure** — fold ONE plain-language sentence into the existing first-run
     ConsentSheet (do NOT resurrect a dedicated ArrowGateSheet); explicitly name "outside the
     desktop" and "other accounts on this PC".
   - **DoneCard reinforcement** — after apply: arrows are now hidden, restore lives in Settings.
   - **Settings page = the canonical home** of the manual control (a dedicated row named by the
     user's own vocabulary 「快捷方式箭头」, status text + 「恢复系统箭头」 action).
   - **Help/FAQ safety net** — 「小箭头不见了？」 deep-links to the Settings row.
   - **Rejected by both:** tray entry (resident is default-OFF so no tray; elevation cannot happen
     from the background per spec 07 — restore MUST run in the open main window) and icons-panel
     inline (scope mismatch: the panel edits the desktop, the pain lives outside the desktop;
     panel height is precious).
3. **Two restores with different semantics must both exist:**
   - 「还原全部美化」 (existing footer) — undo everything, icons AND arrow.
   - 「恢复系统箭头」 (new, Settings) — keep shapes/colours, only bring the system arrow back.
   A user who wants his arrows back should not have to forfeit the whole beautification.
4. **Physical constraint the UI must never lie about:** the registry key is machine-wide and binary.
   "No arrow on my desktop, arrow everywhere else" is impossible on Windows. Restoring the overlay
   puts the system arrow on **every** .lnk, including beautified desktop icons.
5. **Status visibility:** the Settings row's status TEXT is the authority (已隐藏，由 DeskMakeover
   统一绘制 / Windows 默认). No persistent warning badge anywhere — this is a working feature, not
   an error state (v3 calm/restraint; never colour-only status).
6. **Resident tie-in (PM):** pair the arrow-gap education with the resident opt-in offer on the same
   DoneCard — resident (default OFF) heals the own-desktop gap (new icons re-styled in ~4s) but
   never heals outside-desktop or other users; do not sell it as the cure.

### Disclosure copy (UX seat, accepted draft)

ConsentSheet addition:
> 应用美化时，我们会隐藏 Windows 自带的快捷方式小箭头，改由 DeskMakeover 统一绘制，让图标更清爽。
> 这项改动对整台电脑生效，也会影响桌面以外、以及本机其他账户的快捷方式；随时可以在设置里一键恢复。

DoneCard addition:
> 系统快捷方式箭头已隐藏，桌面更清爽了。想找回小箭头，到设置里可以一键恢复。

Restore ConfirmSheet (ceremonied, spec 06 §3.7 real-desktop crossing; not destructive-red):
> 标题：恢复系统快捷方式箭头？
> 正文：所有快捷方式会重新显示 Windows 自带的箭头，包括你在桌面美化过的图标。你的形状和配色
> 不会改变。恢复时系统会弹出一次权限确认。
> 确认：恢复箭头　取消：取消

## The one real divergence — double-arrow handling

Beautified ICOs may carry a **baked** drawn arrow (mark styles). Restore the system overlay and those
icons show TWO arrows (drawn + system).

- **PM seat:** "clean partial restore" — flip the overlay AND re-bake shortcuts without the drawn
  arrow in one operation. No double arrow ever; costs a re-bake pass and silently removes the arrow
  mark the user (or the owner default) chose.
- **UX seat:** never silently change the user's chosen mark (owner-signature rule). Restore the
  overlay only; the ConfirmSheet warns about the double arrow and suggests switching the mark to
  「无」, but the user does it.
- **Lead synthesis (recommended):** put the choice INSIDE the ceremony — the ConfirmSheet carries a
  default-ON checkbox 「同时把角标样式改为无，避免出现两个箭头（会重新应用一次）」. Explicit user
  choice, no silent mutation, clean result by default.

## Owner dispositions — RESOLVED 2026-07-11

1. **Two separate restores** — **APPROVED**: keep-beautification 「恢复系统箭头」 lands as a
   dedicated Settings row, alongside the existing full-restore footer action.
2. **Double-arrow handling** — **owner's own ruling: educate in place, no mutation machinery.**
   When the NATIVE system arrow is detected as active (pre-first-apply, and again after a restore),
   the 标识 (mark) section shows a one-line contextual hint explaining that marks are drawn, the
   native arrow gets hidden on apply, and restore lives in Settings. The hint disappears while the
   overlay is active, so it never nags. No auto re-bake, no restore-time checkbox; the restore
   ConfirmSheet keeps its honest "including beautified icons" sentence.
   Polished copy (owner asked for shorter/clearer):
   > 这些角标都是画上去的。应用后，系统自带的小箭头会隐藏，随时可以在设置里恢复。
3. **Multi-user machines** — **APPROVED (PM rec)**: overlay default stays ON everywhere; when
   multiple active user profiles are detected, the machine-wide sentence in the ConsentSheet is
   non-skippable.

**Implementation timing:** the elevated restore verb already exists (`dm-elevated` RestoreOverlay,
exact byte restore). The Settings row + hint + copy are web-side and can be built on the mock loop
any time; the live wiring of the restore verb belongs to the M6 bridge cutover batch. Spec edits
(spec 06 mark section hint + settings row; spec 07 DoneCard pairing) happen as capability edits
when the feature is built.

**Implementation home:** Settings row lands in `src/components/panels/settings-page.tsx` (existing
Row grammar); ceremony components in `src/components/common/ceremony.tsx`; restore verb already
exists (`dm-elevated` RestoreOverlay, exact byte restore, zero residue). Spec edits: fold into
spec 06 (arrow/mark section) + spec 07 (DoneCard pairing) as capability edits when built.
