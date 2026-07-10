import { create } from 'zustand'
import { en } from './en'
import { zhHans } from './zh-hans'

// Runtime i18n: typed keys (missing key = compile error), instant re-render on
// language change. 'System' resolves like the host's UiText: Windows UI culture
// zh* → 简体中文, anything else → English.

export type StringKey = keyof typeof en
export type Language = 'System' | 'zh-Hans' | 'en'
export type ResolvedLanguage = 'zh-Hans' | 'en'

const tables: Record<ResolvedLanguage, Record<StringKey, string>> = {
  'zh-Hans': zhHans,
  en,
}

export function resolveLanguage(preference: Language): ResolvedLanguage {
  if (preference === 'zh-Hans' || preference === 'en') return preference
  const locale = (typeof navigator !== 'undefined' && navigator.language) || 'en'
  return locale.toLowerCase().startsWith('zh') ? 'zh-Hans' : 'en'
}

interface I18nState {
  lang: ResolvedLanguage
  setPreference: (preference: Language) => void
}

export const useI18n = create<I18nState>((set) => ({
  lang: resolveLanguage('System'),
  setPreference: (preference) => set({ lang: resolveLanguage(preference) }),
}))

/** Reactive translate — use inside components. */
export function useT(): (key: StringKey) => string {
  const lang = useI18n((s) => s.lang)
  return (key) => tables[lang][key]
}

/** Non-reactive translate — for stores/imperative code. */
export function t(key: StringKey): string {
  return tables[useI18n.getState().lang][key]
}

/** .NET-style composite formatting for the {0}/{1} placeholders the tables use. */
export function format(template: string, ...args: (string | number)[]): string {
  return template.replace(/\{(\d+)\}/g, (_, i) => String(args[Number(i)] ?? ''))
}
