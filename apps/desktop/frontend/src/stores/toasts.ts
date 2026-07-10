import { create } from 'zustand'

// Bottom-center toast pills (spec 02): auto-dismiss ~2.6s, newest replaces.
// A toast may carry ONE action (e.g. 已删除 · 撤销) — clicking runs it and
// dismisses; actionable toasts linger a little longer.

export interface ToastAction {
  label: string
  run: () => void
}

export interface Toast {
  id: number
  text: string
  tone: 'info' | 'success' | 'warn'
  action?: ToastAction
}

interface ToastState {
  toasts: Toast[]
  show: (text: string, tone?: Toast['tone'], action?: ToastAction) => void
  dismiss: (id: number) => void
}

let nextId = 1

export const useToasts = create<ToastState>((set) => ({
  toasts: [],
  show: (text, tone = 'info', action) => {
    const id = nextId++
    set((s) => ({ toasts: [...s.toasts.slice(-2), { id, text, tone, action }] }))
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), action ? 4200 : 2600)
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}))
