import { BRIDGE_SCHEMA_VERSION } from './types'
import type { BridgeEvents, BridgeMethods, FrameMeta } from './types'
import { mockCall } from './mock'
import { isTauri, tauriCall, tauriHandles } from './tauri'

// The WebView2 script API surface we use (typed locally — no @types dependency).
interface WebViewApi {
  postMessage(message: unknown): void
  addEventListener(type: string, handler: (e: never) => void): void
  releaseBuffer(buffer: ArrayBuffer): void
}

interface HostMessage {
  kind: 'res' | 'event'
  id?: number
  ok?: boolean
  result?: unknown
  error?: { code: string; message: string }
  topic?: string
  data?: unknown
}

interface SharedBufferEvent {
  getBuffer(): ArrayBuffer
  additionalData: FrameMeta
}

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void }
type EventHandler = (data: unknown) => void
export type FrameHandler = (pixels: Uint8ClampedArray, meta: FrameMeta) => void

const webview =
  typeof window !== 'undefined'
    ? (window as { chrome?: { webview?: WebViewApi } }).chrome?.webview
    : undefined

let nextId = 1
const pending = new Map<number, Pending>()
const eventHandlers = new Map<string, Set<EventHandler>>()
const frameHandlers = new Set<FrameHandler>()

if (webview) {
  webview.addEventListener('message', ((e: { data: HostMessage }) => {
    const msg = e.data
    if (msg.kind === 'res' && msg.id !== undefined) {
      const p = pending.get(msg.id)
      if (!p) return
      pending.delete(msg.id)
      if (msg.ok) p.resolve(msg.result)
      else p.reject(new Error(`${msg.error?.code}: ${msg.error?.message}`))
    } else if (msg.kind === 'event' && msg.topic) {
      eventHandlers.get(msg.topic)?.forEach((h) => h(msg.data))
    }
  }) as never)

  webview.addEventListener('sharedbufferreceived', ((e: SharedBufferEvent) => {
    const buffer = e.getBuffer()
    const meta = e.additionalData
    try {
      // Copy synchronously — the host reuses the buffer for the next frame.
      const byteLength = meta.width * meta.height * 4
      const pixels = new Uint8ClampedArray(byteLength)
      pixels.set(new Uint8ClampedArray(buffer, 0, byteLength))
      frameHandlers.forEach((h) => h(pixels, meta))
    } finally {
      webview.releaseBuffer(buffer)
    }
  }) as never)
}

/** True when running inside the WebView2 host (false in a plain browser). */
export const isHosted = webview !== undefined

export function call<M extends keyof BridgeMethods>(
  method: M,
  ...args: BridgeMethods[M]['params'] extends void ? [] : [BridgeMethods[M]['params']]
): Promise<BridgeMethods[M]['result']> {
  const params = args[0]
  if (!webview) {
    // Under Tauri, the verbs Rust owns go to the host; the rest stay on the mock
    // so the browser dev loop is unchanged.
    if (isTauri() && tauriHandles(method)) {
      return tauriCall(method, params) as Promise<BridgeMethods[M]['result']>
    }
    return mockCall(method, params) as Promise<BridgeMethods[M]['result']>
  }

  const id = nextId++
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve: resolve as (v: unknown) => void, reject })
    webview.postMessage({ kind: 'req', id, method, params })
  })
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
 *  the WebView2 host uses. Declared as a function so the tauri↔client import
 *  cycle stays safe. */
export function emit<T extends keyof BridgeEvents>(topic: T, data: BridgeEvents[T]): void {
  eventHandlers.get(topic)?.forEach((h) => h(data))
}

export function onFrame(handler: FrameHandler): () => void {
  frameHandlers.add(handler)
  return () => frameHandlers.delete(handler)
}

/** Fails loudly when host and web disagree about the contract shape. */
export async function assertSchema(): Promise<void> {
  const info = await call('app.getInfo')
  if (info.schemaVersion !== BRIDGE_SCHEMA_VERSION) {
    throw new Error(
      `bridge schema mismatch: host=${info.schemaVersion} web=${BRIDGE_SCHEMA_VERSION}`,
    )
  }
}
