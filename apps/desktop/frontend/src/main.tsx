import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { CrashGate } from './components/shell/crash-gate'
import { installWebViewHardening } from './lib/webview-hardening'
import { installGlobalErrorCapture, logError } from './lib/error-log'
import { on } from './bridge/client'

installWebViewHardening()
// Error capture installs BEFORE the first render so boot failures are on the
// record too; host-side errors stream into the same log over the bridge.
installGlobalErrorCapture()
on('host-error', (e) => logError('host', e.message, e.stack))

// First-frame gate (ADR-0013 D2): bundled fonts load from local disk in
// milliseconds — waiting for them means the first painted frame already wears the
// final type (zero FOUT). Faces are loaded explicitly (FontFaceSet is lazy until
// first use); the timeout is a belt-and-braces fallback so a broken font file can
// never blank the app.
const fontsReady = Promise.race([
  Promise.all([
    document.fonts.load('500 19px Inter'),
    document.fonts.load('400 13px "HarmonyOS Sans SC"'),
    document.fonts.load('500 15px "HarmonyOS Sans SC"'),
  ]),
  new Promise((resolve) => setTimeout(resolve, 1500)),
])

void fontsReady.then(() => {
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <CrashGate>
        <App />
      </CrashGate>
    </StrictMode>,
  )
})
