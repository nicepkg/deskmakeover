# ADR-0010: Settings becomes a rail page; icon styling expands before release

**Status:** accepted
**Date:** 2026-07-07
Supersedes/amends: ADR-0008's glass-arrow default, ADR-0009's transient settings
drawer and dashed future slot, spec 02's three-shape-only icon shape set.

## Context

The owner reviewed the v1.1 rail build and found several interaction and visual
breaks before release:

- the dashed "+" future slot above settings was visible but inert;
- clicking 设置 opened a transient drawer while 图标/壁纸 were proper rail pages;
- dark/light/system existed, but the product should default to following the OS;
- the settings/about surface needed a stronger, calmer visual composition;
- the app should be bilingual from the start: Simplified Chinese and English,
  defaulting to the OS UI culture;
- the current generated app icon was rejected; the replacement must be a
  hand-authored SVG, not GPT Image output;
- the custom 玻璃箭头 shortcut mark was rejected; the classic Windows arrow is
  preferred over that design;
- the shape axis should grow beyond 苹果/三星/纯圆, using the public maskable-icon
  shape vocabulary as a reference set;
- the left rail glyph tiles should not repeat text inside the tile when a label
  already sits below.

## Decision

1. **Settings is a first-class rail page.** The rail has three modules: 图标,
   壁纸, 设置. The dashed future "+" slot is removed. 设置 is selected like the
   other modules, supports Ctrl+3, and swaps the main surface instead of opening
   a drawer. Settings no longer has a scrim, slide-in drawer, or close button.
2. **Settings page layout.** The old drawer content is rebuilt as a full settings
   surface with grouped sections: appearance/language, automation, local data,
   about/help. It uses the same quiet product chrome as the rest of the app:
   compact rail, calm cards, real icons, and no modal for normal navigation.
3. **Theme default.** New installs default to `System`; stored user preference
   still wins. The visible theme segmented control order is 跟随系统 / 深色 /
   浅色 so the recommended default is first.
4. **Language.** The app supports System / 简体中文 / English. New installs default
   to `System`, which uses Windows UI culture and falls back to English when the
   language is unsupported. User choice is persisted locally and applied at app
   startup and when changed from settings.
5. **App icon.** The app icon source becomes a local hand-authored vector spec:
   a warm-coral continuous-corner tile, a small desktop-panel grid, and a
   restrained sparkle. The render script writes the readable SVG plus generated
   PNG/ICO assets from that same spec; no AI image generation is used.
6. **Shortcut mark.** The rejected 玻璃箭头 is removed from the user-facing mark
   gallery. The default remains no custom mark unless the user chooses otherwise;
   when a shortcut needs an arrow, use the classic Windows arrow path rather than
   the custom glass-arrow design.
7. **Shape expansion.** The shape axis adds the reference masks shown by
   [Progressier's maskable icon editor](https://progressier.com/maskable-icons-editor):
   Google, Brave, Bookmark, Lemon, Squircle, Tile, Teardrop, Blob, and
   Rectellipse. Existing persisted enum values stay stable; new values append
   after `None`.
8. **Rail iconography.** The rail tile contains only an icon glyph. The label below
   remains for clarity and localization. No "图"/"纸" text lives inside the tile.

## Consequences

- `SettingsDrawerView` becomes a page view; drawer overlay code is deleted rather
  than hidden.
- `AppSettings` gains language preference; `UiText` gains runtime culture control.
- Shape geometry becomes a larger tested shape catalogue; preview and bake still
  use the same shape service.
- User-facing copy must remain bilingual. New keys are added to both resx files in
  the same change.
- The glass-arrow mark files may stay only if unreachable test scaffolding still
  needs them during cleanup; it must not be selectable from the UI.
