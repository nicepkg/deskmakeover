import type { BridgeEvents, BridgeMethods } from './types'
import { mockCall } from './mock'
import { isTauri, tauriCall, tauriHandles } from './tauri'

// The app talks to its host through this bridge. Under Tauri, the verbs Rust owns
// route to the real command path (`tauriCall`); everything else falls through to
// the browser mock so the plain-browser dev loop is unchanged.
//
// The retired .NET WebView2 `window.chrome.webview` postMessage host has been
// removed (ADR-0019 Tauri replatform). It was not just dead — on Windows the
// WebView2 runtime injects `window.chrome.webview` into every Tauri page, so any
// residual "is the legacy host present?" check would hijack every bridge call away
// from Tauri and hang the app.

type EventHandler = (data: unknown) => void

const eventHandlers = new Map<string, Set<EventHandler>>()

export function call<M extends keyof BridgeMethods>(
  method: M,
  ...args: BridgeMethods[M]['params'] extends void ? [] : [BridgeMethods[M]['params']]
): Promise<BridgeMethods[M]['result']> {
  const params = args[0]
  // Under Tauri, the verbs Rust owns go to the host; the rest stay on the mock
  // so the browser dev loop is unchanged.
  if (isTauri() && tauriHandles(method)) {
    return tauriCall(method, params) as Promise<BridgeMethods[M]['result']>
  }
  return mockCall(method, params) as Promise<BridgeMethods[M]['result']>
}

export function on<T extends keyof BridgeEvents>(
  topic: T,
  handler: (data: BridgeEvents[T]) => void,
): () => void {
  let set = eventHandlers.get(topic)
  if (!set) {
    set = new Set()
    eventHandlers.set(topic, set)
  }
  set.add(handler as EventHandler)
  return () => set.delete(handler as EventHandler)
}

/** Dispatch a bridge event to registered `on()` handlers. Used by the Tauri
 *  bridge to feed host-side signals (e.g. window-state) through the same stream
 *  the app already listens on. Declared as a function so the tauri↔client import
 *  cycle stays safe. */
export function emit<T extends keyof BridgeEvents>(topic: T, data: BridgeEvents[T]): void {
  eventHandlers.get(topic)?.forEach((h) => h(data))
}
