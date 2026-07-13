import { describe, expect, test } from 'bun:test'
import { en } from '../src/lib/i18n/en'
import { zhHans } from '../src/lib/i18n/zh-hans'

// Copy gate for the 清爽 module (spec 08 §9, ADR-0023 D7) — the same pattern as
// banned-colors: the words that would turn 美颜 into 优化大师 are test-gated, and
// the per-item precision rules (hide ≠ disable; no ad-count claims) are pinned.

const calmKeys = (Object.keys(en) as (keyof typeof en)[]).filter(
  (k) => k.startsWith('Calm_') || k === 'Rail_Calm' || k === 'Panel_CalmTitle',
)

// Cleaner-register vocabulary is constitutionally banned for this module
// (ADR-0004 §1/§5 + ADR-0023 D7). 扫描 etc. would reframe restraint as anxiety.
const BANNED_ZH = ['净化', '清理', '优化', '加速', '扫描', '风险', '问题', '广告']
const BANNED_EN = [/\bclean\b/i, /\bcleaner\b/i, /\boptimi[sz]e/i, /\bboost/i, /\bscan/i, /\brisk/i, /\bproblem/i, /\bads?\b/i]
// Global user-copy bans (spec 01 §UI Language + the no-dash decree, ADR-0013).
const BANNED_GLOBAL_ZH = ['快照', '注册表', '缓存', 'HKLM', 'journal', '—']
const BANNED_GLOBAL_EN = [/registry/i, /snapshot/i, /HKLM/, /journal/i, /—/]

describe('calm copy gate', () => {
  test('every calm key exists in both dictionaries', () => {
    for (const k of calmKeys) {
      expect(zhHans[k as keyof typeof zhHans], `zh missing ${k}`).toBeTruthy()
      expect(en[k], `en missing ${k}`).toBeTruthy()
    }
    expect(calmKeys.length).toBeGreaterThan(50)
  })

  test('cleaner/anxiety register is banned (zh)', () => {
    for (const k of calmKeys) {
      const v = zhHans[k as keyof typeof zhHans] as string
      for (const w of [...BANNED_ZH, ...BANNED_GLOBAL_ZH]) {
        expect(v.includes(w), `${k} contains banned "${w}": ${v}`).toBe(false)
      }
    }
  })

  test('cleaner/anxiety register is banned (en)', () => {
    for (const k of calmKeys) {
      const v = en[k] as string
      for (const re of [...BANNED_EN, ...BANNED_GLOBAL_EN]) {
        expect(re.test(v), `${k} matches banned ${re}: ${v}`).toBe(false)
      }
    }
  })

  test('precision: hiding taskbar entries never claims disabling the feature', () => {
    expect(zhHans.Calm_TaskbarSearch_Desc).toContain('Win+S')
    expect(en.Calm_TaskbarSearch_Desc).toContain('Win+S')
    expect(zhHans.Calm_TaskView_Desc).toContain('Win+Tab')
    expect(en.Calm_TaskView_Desc).toContain('Win+Tab')
  })

  test('precision: Start copy reduces, never promises removal of every promo', () => {
    expect(zhHans.Calm_StartRecs_Desc.startsWith('减少')).toBe(true)
    expect(en.Calm_StartRecs_Desc.toLowerCase()).toContain('fewer')
    for (const k of calmKeys) {
      const v = zhHans[k as keyof typeof zhHans] as string
      expect(v.includes('所有推'), `${k} overclaims: ${v}`).toBe(false)
      expect(v.includes('全部关'), `${k} overclaims: ${v}`).toBe(false)
    }
  })

  test('precision: sync-provider collateral is disclosed', () => {
    expect(zhHans.Calm_SyncNotif_Collateral).toContain('同步')
    expect(en.Calm_SyncNotif_Collateral.toLowerCase()).toContain('sync')
  })

  test('restore copy says 恢复系统推送, never 完整还原', () => {
    expect(zhHans.Calm_Restore).toBe('恢复系统推送')
    for (const k of calmKeys) {
      const v = zhHans[k as keyof typeof zhHans] as string
      expect(v.includes('完整还原'), `${k}: ${v}`).toBe(false)
    }
  })
})
