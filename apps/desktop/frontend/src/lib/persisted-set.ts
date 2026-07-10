import * as React from 'react'

// A `Set<string>` mirrored to localStorage (spec 03 §3.1 — the 自定义 axes persist
// their open-state across sessions instead of collapsing on every launch). Absent key
// ⇒ `defaultOpen` (customization is visible by default); a stored set (even empty) is
// honoured so a deliberately-collapsed panel stays collapsed.

/** localStorage, or null when unavailable (SSR / privacy mode throws on access). */
function storage(): Storage | null {
  try {
    if (typeof window === 'undefined' || !window.localStorage) return null
    return window.localStorage
  } catch {
    return null
  }
}

/** Read a persisted set. Returns null when the key is absent or unreadable. */
export function readPersistedSet(key: string): Set<string> | null {
  const store = storage()
  if (!store) return null
  try {
    const raw = store.getItem(key)
    if (raw === null) return null
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return null
    return new Set(parsed.filter((v): v is string => typeof v === 'string'))
  } catch {
    return null
  }
}

/** Persist a set (best-effort; quota / disabled storage is swallowed). */
export function writePersistedSet(key: string, value: Set<string>): void {
  const store = storage()
  if (!store) return
  try {
    store.setItem(key, JSON.stringify([...value]))
  } catch {
    /* quota exceeded or storage disabled — the open-set is a preference, not truth */
  }
}

/**
 * A `Set<T>` state hook synced to localStorage under `key`. On first ever mount the
 * set is `defaultOpen`; afterwards it restores whatever was last persisted.
 */
export function usePersistedSet<T extends string>(
  key: string,
  defaultOpen: Iterable<T>,
): [Set<T>, React.Dispatch<React.SetStateAction<Set<T>>>] {
  const [set, setSet] = React.useState<Set<T>>(
    () => (readPersistedSet(key) as Set<T> | null) ?? new Set(defaultOpen),
  )
  React.useEffect(() => {
    writePersistedSet(key, set)
  }, [key, set])
  return [set, setSet]
}
