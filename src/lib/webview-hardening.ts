// Web-side webview hardening (docs/references/webview2-pitfalls.md, F6):
// the host disables the equivalent webview settings, but every guard here also
// protects the plain-browser dev loop AND covers old Runtimes where a host
// setting is missing or silently ignored. Install once, before first render.

import { isTauri } from '@/bridge/tauri'

/** True when running inside the packaged app's webview (vs a plain dev browser).
 *  Uses the Tauri signal, not `window.chrome.webview`: WebView2 (Windows Tauri)
 *  injects `chrome.webview` on every page, and WKWebView (macOS Tauri) never does,
 *  so the raw check both mis-fires and misses a platform. */
export function isHostedInWebView(): boolean {
  return isTauri()
}

export function installWebViewHardening(): void {
  // 1. Dropping a file must NEVER navigate the SPA away (AllowExternalDrop=false
  //    host-side; this is the JS belt for the browser loop + old Runtimes).
  window.addEventListener('dragover', (e) => e.preventDefault())
  window.addEventListener('drop', (e) => e.preventDefault())

  // 2. Ctrl+wheel / pinch must zoom the CANVAS (its own non-passive handler),
  //    never the page (IsZoomControlEnabled/IsPinchZoomEnabled=false host-side;
  //    known-flaky on some Runtimes, and browsers zoom the page by default).
  window.addEventListener(
    'wheel',
    (e) => {
      if (e.ctrlKey) e.preventDefault()
    },
    { passive: false },
  )

  // 3. Browser page-zoom/reload/find chords read as "this is a browser" — the
  //    kiosk illusion dies (AreBrowserAcceleratorKeysEnabled=false host-side).
  //    Only under the host: the dev browser keeps its own shortcuts.
  if (isHostedInWebView()) {
    window.addEventListener('keydown', (e) => {
      const key = e.key.toLowerCase()
      if ((e.ctrlKey && (key === '+' || key === '-' || key === '=' || key === '0' || key === 'r' || key === 'f' || key === 'p')) || key === 'f5' || key === 'f3' || key === 'f7') {
        e.preventDefault()
      }
    })

    // 4. The default context menu is Edge's, not ours (AreDefaultContextMenusEnabled
    //    =false host-side). Never in the dev browser: inspect-element stays usable.
    window.addEventListener('contextmenu', (e) => e.preventDefault())
  }
}
