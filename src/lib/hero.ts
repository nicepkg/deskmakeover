import type { HeroPhase } from '@/components/common/cta-button'
import { useIcons } from '@/stores/icons'
import { useWallpaper } from '@/stores/wallpaper'
import { format, useT } from '@/lib/i18n'

// Hero/CTA state derivation shared by the full panels and the compact toolbars
// (spec 01 §Hero state machine; wallpaper mapping per spec 04).

export function useIconsHero() {
  const t = useT()
  const state = useIcons((s) => s.state)

  const phase: HeroPhase = !state || state.scanning
    ? 'scanning'
    : state.working
      ? 'working'
      : !state.applied
        ? 'ready'
        : state.dirty
          ? 'dirty'
          : 'synced'

  const statusText = !state || phase === 'scanning'
    ? t('Hero_Scanning')
    : format(
        t(phase === 'dirty' ? 'Hero_DirtyStatus' : phase === 'synced' ? 'Hero_CleanStatus' : 'Hero_ReadyStatus'),
        state.styleableCount,
      )
  const heroTitle = t(phase === 'dirty' ? 'Hero_TitleDirty' : phase === 'synced' ? 'Hero_TitleClean' : 'Hero_Title')
  const ctaText = t(
    phase === 'scanning'
      ? 'Cta_Scanning'
      : phase === 'working'
        ? 'Cta_Working'
        : phase === 'dirty'
          ? 'Cta_Update'
          : phase === 'synced'
            ? 'Cta_Synced'
            : 'Cta_Apply',
  )

  return { state, phase, statusText, heroTitle, ctaText }
}

export function usePaperHero() {
  const t = useT()
  const state = useWallpaper((s) => s.state)
  // The CTA's applying shimmer is driven by the STORE's `applying` flag (the
  // bake+write in flight), matching the icons module — the DTO's `working`
  // resolves too fast in the mock to ever show (owner call 2026-07-09: the
  // wallpaper button had no loading state).
  const applying = useWallpaper((s) => s.applying)

  const phase: HeroPhase = !state
    ? 'scanning'
    : applying || state.working
      ? 'working'
      : state.dirty
        ? state.hasBackup
          ? 'dirty'
          : 'ready'
        : state.hasBackup
          ? 'synced'
          : 'scanning'

  const statusText = t(
    !state
      ? 'Paper_StatusIdle'
      : state.working
        ? 'Paper_StatusWorking'
        : state.dirty
          ? state.hasBackup
            ? 'Paper_StatusDirty'
            : 'Paper_StatusIdle'
          : state.hasBackup
            ? 'Paper_StatusApplied'
            : 'Paper_StatusIdle',
  )
  const heroTitle = t(state?.hasBackup && !state.dirty ? 'Paper_HeroApplied' : 'Paper_Hero')
  const ctaText = t(
    phase === 'working'
      ? 'Paper_Cta_Working'
      : phase === 'dirty'
        ? 'Paper_Cta_Update'
        : phase === 'synced'
          ? 'Paper_Cta_Synced'
          : 'Paper_Cta_Apply',
  )

  return { state, phase, statusText, heroTitle, ctaText }
}
