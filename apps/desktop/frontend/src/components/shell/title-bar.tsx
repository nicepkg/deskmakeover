import { Minus, Square, X } from 'lucide-react'
import { call } from '@/bridge/client'
import { useApp } from '@/stores/app'
import { useT } from '@/lib/i18n'
import { DevMenu } from '@/components/shell/dev-menu'
import { KeymapLegend } from '@/components/shell/keymap-legend'
import appIcon from '@/assets/app-icon.svg'

// 46px custom titlebar (spec 02, ADR-0012): logo 24 + name 13/600 + a quiet `?`
// keymap affordance — no version chip pre-release. The whole band is a drag region;
// caption buttons and the `?` opt out. Snap/drag/double-click maximize come from
// WebView2 non-client support (spec 05 §2).

export function TitleBar() {
  const t = useT()
  const maximized = useApp((s) => s.maximized)

  return (
    <header className="app-drag flex h-[46px] shrink-0 items-center justify-between">
      {/* The logo slot is exactly as wide as the module rail (66px) and centers the
          mark — the app icon and the rail glyphs share one vertical axis. */}
      <div className="flex items-center">
        <span className="flex w-[66px] shrink-0 justify-center">
          <img src={appIcon} alt="" className="size-6" />
        </span>
        <span className="text-[12px] font-medium text-t1">{t('AppTitle')}</span>
      </div>
      <div className="app-no-drag flex h-full items-center">
        <DevMenu />
        <KeymapLegend />
        <div className="ml-1 flex h-full">
          <CaptionButton label={t('Cap_Minimize')} onClick={() => call('shell.minimize')}>
            <Minus size={14} strokeWidth={1.5} />
          </CaptionButton>
          <CaptionButton
            label={maximized ? t('Cap_Restore') : t('Cap_Maximize')}
            onClick={() => call(maximized ? 'shell.restore' : 'shell.maximize')}
          >
            {maximized ? <RestoreGlyph /> : <Square size={11.5} strokeWidth={1.5} />}
          </CaptionButton>
          <CaptionButton close label={t('Cap_Close')} onClick={() => call('shell.close')}>
            <X size={14.5} strokeWidth={1.5} />
          </CaptionButton>
        </div>
      </div>
    </header>
  )
}

function CaptionButton({
  label,
  onClick,
  close = false,
  children,
}: {
  label: string
  onClick: () => void
  close?: boolean
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={
        'flex h-full w-[46px] items-center justify-center text-t2 transition-colors duration-100 ' +
        (close ? 'hover:bg-[#E81123] hover:text-white' : 'hover:bg-raised-hov hover:text-t1')
      }
    >
      {children}
    </button>
  )
}

/** Win11-style restore-down glyph (two offset squares). */
function RestoreGlyph() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
      <rect x="1" y="3.5" width="7.5" height="7.5" rx="1" stroke="currentColor" strokeWidth="1.2" />
      <path d="M3.8 1.6 H9.4 A1.2 1.2 0 0 1 10.6 2.8 V8.4" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  )
}
