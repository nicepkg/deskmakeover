# WebView2 pitfalls — Tauri-era edition (2026-07-15)

Supersedes the 2026-07-08 WPF-era report (git remembers it). Compiled from four
dedicated research agents (rendering/compositing · Tauri×WebView2 integration ·
runtime/deployment · input/IME) plus live findings from the 2026-07-15 Windows
debugging session. Every finding carries source URLs and a status tag audited
against THIS codebase:

> **[HIT]** we hit it (fixed or reproduced) · **[COVERED]** code/config already
> handles it — do NOT "fix" again · **[AT-RISK]** real gap, action listed ·
> **[N-A]** doesn't apply to this app.

## ⭐ Meta-finding: the WPF→Tauri migration orphaned the old doc's host layer

The 07-08 report deferred ~a dozen mitigations to "🏗 host-side, do on
`Host/WebShellWindow.cs`" — **that C# host no longer exists**. Under Tauri,
**wry owns WebView2 environment creation** and exposes almost none of the
`CoreWebView2Settings`/`EnvironmentOptions` knobs the old plan relied on
(`AreBrowserAcceleratorKeysEnabled`, `IsPinchZoomEnabled`, `IsZoomControlEnabled`,
`DefaultBackgroundColor`, `ScrollBarStyle`, `ProcessFailed`…). Three consequences:

1. The JS/CSS belts in `src/lib/webview-hardening.ts` are now the **sole**
   defense, not a redundant belt — its comments claiming "the host disables the
   equivalent settings" describe a host that is gone. (Comments to be corrected;
   the belts themselves work.)
2. Levers now come in exactly three shapes: wry built-in behavior · a Chromium
   flag via `windows.additionalBrowserArgs` · JS/CSS. Anything else is "patch wry".
3. A few old 🔴 items became **DIY projects** (crash watchdog) or **decisions**
   (install mode, signing) — triaged below.

⚠️ `additionalBrowserArgs` gotcha: setting it **replaces** wry's defaults — any
custom string must re-include `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
or those quietly re-enable. ([tauri#11144](https://github.com/tauri-apps/tauri/issues/11144))

---

## TL;DR triage

### 🔴 Before public ship
1. **Webview crash = frozen white window, no recovery.** wry surfaces no
   `ProcessFailed`/`BrowserProcessExited`. DIY watchdog needed (§C6).
2. **Missing/corrupt runtime at launch has no friendly fallback UI** (§C2);
   pair with a UDF delete-and-restart recovery (§C1).
3. **Install mode for China**: `downloadBootstrapper` needs the MS CDN at
   install time — decide `offlineInstaller` (+127 MB) vs accept a dead-first-launch
   tail on offline/throttled networks (§C5).
4. **Signing + CN AV whitelisting**: unsigned Tauri NSIS gets false-flagged by
   Defender/360/火绒; EV no longer buys SmartScreen reputation (2024 change) —
   buy OV, sign installer + exe + `dm-elevated.exe`, pre-register 360/火绒
   whitelists (§C4).
5. **Elevated-launch UDF conflict** under Win11 24H2 Administrator Protection
   ([tauri#13926](https://github.com/tauri-apps/tauri/issues/13926)) — keep the
   main window non-elevated forever; only `dm-elevated` elevates (§C1).

> Note: item 1 (§C6) is the *renderer-process* crash — still DIY-only because wry
> exposes no `ProcessFailed`. The *GL-context* loss (§A1), which is the crash users
> actually hit from driver updates, is now recovered — see ✅ below.

### 🟡 Verify on real devices before ship
Touch: title-bar drag + hover-only affordances (§D3/§D6) · tray-restore
white-screen ×20 cycle (§B4) · IME candidate window under stacked zoom (§D1) ·
maximize/multi-monitor/minimize-restore blank (§B3) · first-click-after-restore
swallowed → `set_focus` on tray show (§D7) · fractional-DPI hairlines at
125%/150% · P3 wide-gamut color drift (§A7).

### ✅ Fixed this session (2026-07-15)
- `.url` UTF-16 encoding → Steam icons stylable (`bdb1d7b`)
- 系统默认 real draft reset (`834be70`)
- Backdrop-RT float-key churn → zone-delete white screen (`7703dd4`)
- Auto UI zoom, √fit curve, WebView2 ZoomFactor raises DPR (`49088d5`+`1217312`)
- Icon tiles render at exactly 2× on-screen physical px (SSAA) (`06c70ff`)
- DPR-change tracking (monitor move re-renders canvases), invalidate-on-show
  belt, pre-paint backgroundColor unified (`c9fe802`)
- IME composition guard + spellCheck=false on zone rename (`cfafa65`)
- WebGL context-loss recovery: `preventDefault` on `lost` + full rebuild on
  `restored`, plain-`<img>` fallback meanwhile (§A1, `db96a5a`)
- `bake()` caps its render targets to the probed GPU `MAX_TEXTURE_SIZE` → no more
  black export on 5K/8K or old iGPU; the live scale respects it too (§A5, `db96a5a`)
- `WEBVIEW2_*` env-var sanitization in release builds — arg injection
  (`--remote-debugging-port`), runtime-folder swap (code exec), profile redirect;
  gated by a private `DESKMAKEOVER_DEVTOOLS` opt-in for our CDP workflow (§C9, `db96a5a`)

---

## A. Rendering & compositing

**A1 [✅ FIXED `db96a5a`] WebGL context loss — no recovery path.** GPU-process crash,
TDR/driver reset, sleep wake, RDP, or an Evergreen update mid-session loses the
GL context; Pixi v8's internal restore is unreliable (textures created before
the loss come back empty). Our `renderNow` try/catch only catches synchronous
throws — loss is async and silent. Fix (in `useWallpaperCompositor`):
`webglcontextlost` → `preventDefault()` (required or `restored` never fires) +
fall back to the plain wallpaper `<img>` (the `loadError` path) instead of a blank
canvas; `webglcontextrestored` bumps a nonce in the create-effect deps → full
teardown + `create` + re-decode from the store's source URL (the effect re-fetches;
we don't rely on the `close()`d ImageBitmap). Listeners re-bind to each recreated
canvas. The `renderNow` retry budget (≤5) still absorbs invalidations that land
while the context is down. Same-canvas rebuild (canvas keyed on grid dims, so it
persists across loss); the `create` catch degrades gracefully if re-init ever fails.
[pixijs#6494](https://github.com/pixijs/pixijs/issues/6494) ·
[pixijs#5386](https://github.com/pixijs/pixijs/issues/5386) ·
[WebView2Feedback#3817](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3817)

**A2 [🟡 AT-RISK] Software rendering / no-WebGL fallback.** SwiftShader is now
disabled by default in Chromium (`--allow-unsafe-swiftshader`) — blocklisted
driver/VM/RDP may yield **no WebGL at all** → `Application.init` rejects → blank.
And under software raster the per-frame full-screen `BlurFilter` (quality 6) is
seconds-per-frame. Fix: probe `UNMASKED_RENDERER_WEBGL` once; on
software/`SwiftShader` cap renderScale, drop blur quality, freeze gesture
re-blur; on init rejection fall back to the static `<img>` mirror we already
render while `!ready`.
[chromium#40277080](https://issues.chromium.org/issues/40277080)

**A3 [HIT→fixed `7703dd4`] Fractional-DPI RenderTexture churn.** `RT.width` is
`pixelWidth/resolution` — float division that misses strict equality at 125%/150%
(and at our fractional zoom DPRs) → per-frame destroy/recreate of two full-screen
RTs → dying-ghost sampled a destroyed texture → white canvas. Keep the creation-key
pattern (`${w}x${h}@${scale}`); never compare against RT getters.

**A4 [HIT — workaround designed, deferred by owner] Composited rounded-corner
clip is not antialiased.** With the desktop-space under `scale()` transform,
WebView2's compositor hard-scissors accelerated layers (WebGL canvas,
backdrop-filter taskbar) at the container's `border-radius` — jagged 1px fringe
at the corner arcs, flush case only. None of isolation/translateZ/contain:paint/
clip-path/mask/opacity fix it (all tried live via CDP). Root: intermediate RTs
aren't MSAA + rounded-clip slow path; MSAA on RTs is flaky on ANGLE D3D11.
**Verified fix (to apply when owner green-lights):** square-clip the container
(integer rect scissor is pixel-exact) + paint the four corner arcs with DOM
radial-gradient caps + a rounded hairline ring overlay.
[pixijs#4509](https://github.com/pixijs/pixijs/issues/4509)

**A5 [✅ FIXED `db96a5a`] `bake()` exceeds MAX_TEXTURE_SIZE on big screens/old iGPU.**
Live path caps the long edge at 4096; `bake()` used to allocate full
`screenWidth×screenHeight` RTs at resolution 1 — a 5K/8K/Span target on a
4096-limit iGPU failed → black/empty export PNG. Now `readMaxTextureSize` probes
`gl.getParameter(gl.MAX_TEXTURE_SIZE)` once at init; `bake()` sets its scale to
`min(1, max / longEdge)` so every RT (frost/backdrop/final, all keyed off
`renderScale`) fits — a slightly-soft upscaled wallpaper beats a black one. The
live `setRenderScale` cap also respects the probe (sub-4096 iGPU). Non-overflowing
GPUs (the 99% case: 4K = 3840 < 4096, modern GPUs 8192+) are byte-identical to before.
Tiling to preserve full native res on overflow is a future refinement, not shipped.

**A6 [🟡] Occlusion/backgrounding.** Chromium's native occlusion tracker can
false-positive a *visible* window (white content) and throttles rAF when
minimized; WebView2 also has a blank-after-minimize-restore repaint bug
([#5171](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5171)).
Our invalidate-driven ticker + direct-render bake are the right architecture,
and `c9fe802` added the invalidate-on-focus/visibility belt. If field reports
still show blanks: add
`--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --disable-backgrounding-occluded-windows --disable-renderer-backgrounding --disable-background-timer-throttling`
(note the re-included wry defaults). Tauri's `backgroundThrottling` config is
**not supported on Windows** — the flag route is the only lever.
[tauri#5250](https://github.com/tauri-apps/tauri/issues/5250)

**A7 [🟡] Color management.** Wide-gamut panels under Win11 Auto Color
Management can oversaturate untagged sRGB; the WebGL wallpaper and DOM swatches
travel different color paths and can drift apart. If a P3-device test shows it:
`--force-color-profile=srgb` pins both. OKLCH/`color-mix` themselves are fine
across the Evergreen range (111+).
[chromium#328856550](https://issues.chromium.org/issues/328856550)

**A8 [🟡] Pixi `extract.canvas` returns premultiplied alpha** — any α<1 pixel
in an exported PNG gets darkened RGB ([pixijs#10820](https://github.com/pixijs/pixijs/issues/10820)).
Our baked wallpaper is opaque cover-fit so bulk pixels are safe; if any
translucent layer ever reaches the export, flatten onto an opaque background
before extract (also guarantees an opaque wallpaper file).

**A9 [🟢 notes]** Fonts: bundled-font FOUT is COVERED (block + fonts.ready
gate). Text on composited layers renders grayscale-AA (no ClearType) — expected
Chromium behavior since 115, don't fight it; if CN text at 11–13px reads thin on
1× displays, bump the smallest CJK sizes to weight 500. DPI story: WebView2
ZoomFactor **multiplies devicePixelRatio** (our auto-zoom exploits this; macOS
`pageZoom` does NOT change DPR — WebKit #124862). Runtime 142.0.3595.65 has a
canvas-≥3840px perf regression ([WebView2Feedback#5426](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5426)) — keep the 4096 renderScale cap.

## B. Tauri × WebView2 integration

**B1 [COVERED] Custom schemes.** `dmwallpaper:`/`dmicon:`/`dmpreset:` are served
as `http://<scheme>.localhost` = cross-origin → the `ACAO:*` headers in `lib.rs`
are load-bearing (we hit this once; keep on 404s too). CSP correctly lists both
URL forms. ⚠️ Windows custom protocols buffer the **whole body** (no streaming,
sync handler) — keep responses modest; avoid re-fetch when the path is unchanged
([wry#1022](https://github.com/tauri-apps/wry/issues/1022)).

**B2 [COVERED] IPC.** Pixels ride the custom protocols, never invoke (10 MB
through invoke ≈ 200 ms on Windows + base64 bloat); icon bytes are chunked
(`icons_apply_baked_chunk`). Residual: `*.localhost` interception by corporate
proxies can kill `ipc.localhost` entirely — document, don't engineer around.

**B3 [🟡] Frameless window.** Drag works via `data-tauri-drag-region` →
`startDragging()` (Win32), **not** CSS `app-region` — Tauri reverted
`IsNonClientRegionSupportEnabled` on Windows, so the `.app-drag` CSS rules are
likely dead grammar (harmless; decide: enable the setting via wry, or drop the
CSS). Known holes: **touch can't drag or double-tap-maximize** (§D3), drag
fails when unfocused ([#11605](https://github.com/tauri-apps/tauri/issues/11605)),
Win11 Snap-Layouts flyout doesn't appear on a custom maximize button (plugins
exist), double-click-maximize restore-size bugs
([#11945](https://github.com/tauri-apps/tauri/issues/11945)), verify DWM rounded
corners/shadow on-device. Multi-monitor mixed-DPI maximize/minimize/restore
blank is the classic #2549 family — the `c9fe802` invalidate belt is the cheap
mitigation; add a Rust-side repaint nudge on `show()` if field reports persist.

**B4 [🟡] Tray residency (hide → show).** tauri#9393 white-screen-on-show is
fixed but the pattern stays version-sensitive — regression-test 20 cycles on
every wry bump. While hidden, the webview throttles (fine — the reconcile loop
is native Rust). Memory: a hidden webview holds ~100–200 MB by design. Add
`set_focus()` after tray-show so the first click and IME land (§D7).

**B5 [COVERED] window-state plugin** validates restored geometry against
connected monitors; our setup-time first-zoom-pass already closes the
maximized-restore race. Verify the plugin saves pre-hide geometry (not a
minimized 0×0) under residency.

**B6 [COVERED] Zoom.** Host-set ZoomFactor persists across navigations and
survives hide/show (only a webview recreate resets it — which we never do while
resident). Zoom compounds with OS scale: 150% monitor × zoom 2.0 = DPR 3 —
watch Pixi texture memory on 4K@150% machines.

## C. Runtime / deployment / environment

**C1 [🔴] UDF.** Default: `%LOCALAPPDATA%\com.xiaominglab.deskmakeover\EBWebView`
(keyed on identifier). Crash dumps in `…\EBWebView\Crashpad\reports\`. Add a
**delete-and-restart-once** recovery on webview init failure (corrupt UDF is the
common consumer killer; guard with a retry-count file). Win11 24H2
Administrator Protection: an **elevated main window** points the UDF at the
elevated user while WebView2 de-elevates → instant death
([tauri#13926](https://github.com/tauri-apps/tauri/issues/13926)). Law: the UI
process never elevates; only the `dm-elevated` sidecar does (already our
security boundary — this is one more reason it stays that way). Add
`tauri-plugin-single-instance` (second launch = focus, not UDF lock race).

**C2 [🔴] Runtime missing/corrupt at launch.** `downloadBootstrapper` installs
at install-time only; later removal/AV-block/corruption = bare error, no
friendly UI. Tauri ≥ the May-2025 pre-flight check
([PR#13406](https://github.com/tauri-apps/tauri/pull/13406)) at least surfaces a
proper error — route it to an "install WebView2" screen with the bootstrapper
link. Verify our pinned Tauri includes that PR.

**C3 [🟡] Evergreen auto-update mid-session** can crash the browser process
(binaries deleted under a running instance; e.g. the 136.0.3222.0 regression
wave). wry doesn't surface `NewBrowserVersionAvailable`. Mitigation = the C6
watchdog + "restart to update" toast. `fixedRuntime` (+~180 MB, manual security
updates, App-Container ACL quirk on Win10) is the escape hatch only if field
telemetry shows instability — stay Evergreen by default.

**C4 [🔴] Signing & CN antivirus.** 2024 change: **EV certs no longer grant
instant SmartScreen reputation** — reputation accrues from volume for OV and EV
alike; buy **OV**, sign installer + main exe + `dm-elevated.exe`. Unsigned Tauri
NSIS is routinely false-flagged (Defender `Wacatac`, plus 360/火绒 heuristics —
even Tauri's own `nsis_tauri_utils.dll` got flagged,
[#14882](https://github.com/tauri-apps/tauri/issues/14882)). Pre-register:
360 `open.soft.360.cn/report.htm` · 火绒 `seclab@huorong.cn`; ship a
"被安全软件拦截" support doc (allowlist `DeskMakeover.exe` + `msedgewebview2.exe`).

**C5 [🔴 decision] Install mode for CN consumers.** The bootstrapper pulls from
the MS delivery CDN (reachable in mainland China, unlike GitHub — the GitHub
fetches are **build-time only**, a dev-box concern). But offline/locked-down/
throttled installs still die with a white first launch. For a consumer visual
app, first-run success = retention: prefer **`offlineInstaller`** (+~127 MB) or
at least `embedBootstrapper`. Win10 LTSC (common in CN) ships without the
runtime most often — it's the population this decision protects.

**C6 [🔴 DIY] Crash watchdog.** wry exposes no `ProcessFailed` — on
renderer/browser crash the app is a frozen white window. Build: JS heartbeat →
Rust; on silence, recreate the WebviewWindow (bonus: re-applies zoom, reloads
SPA); plus `window.onerror`/`unhandledrejection` → `invoke('report_crash')`.
This is the single biggest regression vs the WPF plan — the old doc assumed a
config toggle. ([tauri#10157](https://github.com/tauri-apps/tauri/issues/10157))

**C7 [🟡] Enterprise policies.** Edge *browser* GPO does NOT apply (good);
WebView2-specific policies DO: `BrowserExecutableFolder` redirects,
`AdditionalBrowserArguments` injection, `ReleaseChannels`, `UpdatesSuppressed`
(`HKLM\SOFTWARE\Policies\Microsoft\Edge\WebView2\…`). Defense = C10 telemetry
(log effective runtime path/version/args) so support can spot hijacks.

**C8 [🟡] ARM64 (Snapdragon).** x64-only NSIS runs emulated; x64 host + ARM64
runtime mixing has documented Office-class failures. Add a native
`aarch64-pc-windows-msvc` installer, or at minimum test first-run on a
Snapdragon X device.

**C9 [✅ FIXED `db96a5a`] Env-var sanitization in `run()`** before `tauri::Builder`.
`sanitize_webview_env()` removes `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (env **wins
over code**, [#5571](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5571) —
injects `--remote-debugging-port`/`--no-sandbox`), `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`
(runtime replacement = arbitrary code in our process — the highest-severity vector),
`WEBVIEW2_USER_DATA_FOLDER` (profile redirect), and `WEBVIEW2_RELEASE_CHANNEL_PREFERENCE`.
`#[cfg(not(debug_assertions))]` so it only fires in release, and skipped when the private
`DESKMAKEOVER_DEVTOOLS` opt-in is set so our own CDP workflow (release binary +
`--remote-debugging-port`) still works. Called at the top of `run()`, single-threaded,
so `remove_var` is sound.

**C10 [🟡] Startup diagnostics block** (rotating local log): runtime version
(wry `Webview::version()`), effective UDF path, GPU/software-render probe, OS
build + arch (x64-native vs ARM64-emulated), env/policy overrides detected,
Crashpad dump presence. This is what makes every other field issue diagnosable.

## D. Input / IME (CN users are the default)

**D1 [HIT→fixed `cfafa65`] IME composition vs Enter/Escape in zone rename** —
Enter picks the candidate, Escape cancels the pinyin; the editor was
committing/closing mid-composition. Guard `isComposing || keyCode 229`; also
`spellCheck=false` (WebView2 spellcheck keys off OS env language, no disable
API, squiggles pinyin). **Rule for every future text input**: same guard; drive
search-as-you-type from `input`+`compositionend`, never `keydown`.

**D2 [🟡] Candidate window drift under stacked zoom.** The rename input lives
inside `transform: scale(view.scale)` (0.2–3) × native ZoomFactor (1.0–2.0);
Chromium's IME caret-rect math is weak inside CSS-scaled containers → the
candidate list can float away from the caret. If confirmed on-device: render the
rename editor in host space (like the zone context menu) instead of the scaled
layer. ([WebView2Feedback#5570](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5570))

**D3 [🔴 for touch devices] Touch/pen can't move the frameless window.** Both
drag paths are mouse-only (`data-tauri-drag-region` keys on mouse buttons;
WebView2 non-client is mouse-only by design, [#2243](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2243)).
Fix: `onPointerDown` with `pointerType !== 'mouse'` → `startDragging()` + a
~300 ms double-tap → `toggleMaximize()` on the title bar.

**D4 [🟡] Keyboard accelerators are JS-only.** Host never sets
`AreBrowserAcceleratorKeysEnabled=false` (wry doesn't expose it) — reload/print
are handled in the browser process and can leak past `preventDefault` on some
runtimes; `Ctrl+Shift+I`/`F12`/`F11`/`Ctrl+U` aren't in the JS list at all.
Verify DevTools is compiled out of release (wry `devtools` feature); consider
`tauri-plugin-prevent-default` (release-only) to close the rest.

**D5 [🟡] Pinch zoom.** JS ctrl+wheel guard catches trackpad pinch; touchscreen
pinch goes through native `IsPinchZoomEnabled` (default **true**, not disabled —
wry doesn't expose it). On touch devices two-finger pinch will page-zoom and
fight our ZoomFactor. Same lever options as D4.

**D6 [🟡] Hover-only affordances are unreachable by touch** (tile ⋯ button,
zoom stepper popover, bare try-on, inspector hover). Pen sends hover; finger
doesn't. Audit for a tap path / `@media (hover: none)` fallback.

**D7 [🟡] First click after tray-restore is swallowed** (wry#637 class) — call
`set_focus()` in the tray-show handler so the first click, autofocus, and IME
land. Pairs with B4.

**D8 [🟢 notes]** `dragDropEnabled:false` is **inverted semantics** — it
disables Tauri's native drop interception, which is precisely what lets our
HTML5 drop-import work; a future feature needing real file *paths* (not bytes)
would need re-architecture. Clipboard: prefer the Tauri clipboard plugin over
`navigator.clipboard` (focus-gated, permission quirks). Window hotkeys are
IME-safe today (input-element guards); broaden the `HTMLInputElement` check if a
textarea/contenteditable ever ships. Forced-colors/high-contrast: verify the
rename input caret; scroll feel is steppier than macOS (fixed wheel deltas) —
accept. `user-select:none` + context-menu suppression also removes IME
reconversion — acceptable.
