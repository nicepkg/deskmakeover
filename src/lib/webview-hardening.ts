// Web-side webview hardening (docs/references/webview2-pitfalls.md).
// Under Tauri there is NO host layer setting the equivalent CoreWebView2
// switches (wry doesn't expose AreBrowserAcceleratorKeysEnabled /
// IsPinchZoomEnabled / IsZoomControlEnabled / AllowExternalDrop) — these JS
// guards are the PRIMARY defense, not a belt over host settings, and they also
// protect the plain-browser dev loop. Install once, before first render.

import { isTauri } from '@/bridge/tauri'

/** True when running inside the packaged app's webview (vs a plain dev browser).
 *  Uses the Tauri signal, not `window.chrome.webview`: WebView2 (Windows Tauri)
 *  injects `chrome.webview` on every page, and WKWebView (macOS Tauri) never does,
 *  so the raw check both mis-fires and misses a platform. */
export function isHostedInWebView(): boolean {
  return isTauri()
}

export function installWebViewHardening(): void {
  // 1. Dropping a file must NEVER navigate the SPA away. tauri.conf's
  //    dragDropEnabled:false only disables Tauri's NATIVE drop interception
  //    (which is what lets our HTML5 drop-import receive events at all) — this
  //    guard is what stops a stray drop from navigating.
  window.addEventListener('dragover', (e) => e.preventDefault())
  window.addEventListener('drop', (e) => e.preventDefault())

  // 2. Ctrl+wheel / trackpad pinch must zoom the CANVAS (its own non-passive
  //    handler), never the page. This catches the wheel+ctrlKey form only;
  //    TOUCHSCREEN pinch rides native IsPinchZoomEnabled (default true, not
  //    exposed by wry) — see pitfalls doc §D5.
  window.addEventListener(
    'wheel',
    (e) => {
      if (e.ctrlKey) e.preventDefault()
    },
    { passive: false },
  )

  // 3. Browser page-zoom/reload/find chords read as "this is a browser" — the
  //    kiosk illusion dies. Best-effort: reload/print are handled in the browser
  //    process and can leak past preventDefault on some runtimes (wry exposes no
  //    AreBrowserAcceleratorKeysEnabled) — see pitfalls doc §D4.
  //    Only under the host: the dev browser keeps its own shortcuts.
  if (isHostedInWebView()) {
    window.addEventListener('keydown', (e) => {
      const key = e.key.toLowerCase()
      if ((e.ctrlKey && (key === '+' || key === '-' || key === '=' || key === '0' || key === 'r' || key === 'f' || key === 'p')) || key === 'f5' || key === 'f3' || key === 'f7') {
        e.preventDefault()
      }
    })

    // 4. The default context menu is Edge's, not ours. Never in the dev
    //    browser: inspect-element stays usable.
    window.addEventListener('contextmenu', (e) => e.preventDefault())
  }
}
