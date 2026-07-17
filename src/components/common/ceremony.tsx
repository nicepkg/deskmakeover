import * as React from 'react'
import type { ReactNode } from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { call } from '@/bridge/client'
import { WinArrowGlyph } from '@/components/common/chip-preview'
import { format, useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// The apply ceremony (ADR-0013 D9, shared by icons + wallpaper): the one moment the
// app crosses from the simulated mirror into the REAL desktop gets a doorway —
// a consent sheet before the first apply, a completion card with 「去看看桌面」
// after every apply (the change happens BEHIND the window), and a confirm before
// restore. One modal grammar, pop motion, Esc cancels, reduced-motion degrades.

function Scrim({ onClose, children }: { onClose?: () => void; children: ReactNode }) {
  const reduced = useReducedMotion()
  return (
    <motion.div
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-6 backdrop-blur-[2px]"
      initial={{ opacity: reduced ? 1 : 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: reduced ? 0 : 0.16 }}
      onClick={onClose}
      onKeyDown={(e) => e.key === 'Escape' && onClose?.()}
    >
      <motion.div
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-[360px] rounded-2xl border border-hair bg-raised p-5 shadow-elev-2"
        initial={{ scale: reduced ? 1 : 0.95, opacity: reduced ? 1 : 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: reduced ? 1 : 0.97, opacity: 0 }}
        transition={{ duration: reduced ? 0 : 0.16, ease: [0.33, 1, 0.68, 1] }}
      >
        {children}
      </motion.div>
    </motion.div>
  )
}

/** Generic confirm sheet (restore / replace-all …). `body` carries an optional
 *  explanatory paragraph (e.g. the arrow restore's honest "affects beautified
 *  icons too" note). */
export function ConfirmSheet({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel,
  destructive = false,
  confirmDisabled = false,
  onConfirm,
  onCancel,
}: {
  open: boolean
  title: string
  /** A string paragraph, or structured content (e.g. the calm sheet's three-line consent). */
  body?: ReactNode
  confirmLabel: string
  cancelLabel: string
  destructive?: boolean
  /** Gate the confirm on required input (visible disabled state, never a silent no-op click). */
  confirmDisabled?: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  return (
    <AnimatePresence>
      {open && (
        <Scrim onClose={onCancel}>
          <p className="text-body font-medium text-t1">{title}</p>
          {/* div, not p: callers pass structured bodies (input fields, checkbox rows) and
              block elements inside a <p> are invalid HTML the parser would re-nest. */}
          {body && <div className="mt-2 text-[12px] leading-relaxed text-t2">{body}</div>}
          <div className="mt-4 flex justify-end gap-1.5">
            <button
              type="button"
              onClick={onCancel}
              className="rounded-[9px] bg-chip px-3 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
            >
              {cancelLabel}
            </button>
            <button
              type="button"
              disabled={confirmDisabled}
              onClick={onConfirm}
              className={cn(
                'rounded-[9px] px-3 py-1.5 text-[12px] font-medium text-primary-foreground transition-transform active:scale-[0.98]',
                destructive ? 'bg-destructive' : 'bg-primary shadow-elev-cta',
                confirmDisabled && 'cursor-default opacity-40 active:scale-100',
              )}
            >
              {confirmLabel}
            </button>
          </div>
        </Scrim>
      )}
    </AnimatePresence>
  )
}

/** First-apply consent: what happens · what never happens · the one UAC prompt,
 *  plus the machine-wide arrow disclosure (ADR-0021). On multi-user machines
 *  (`multiUser`) the sheet is non-skippable: backdrop/Esc no longer dismiss it,
 *  so the whole-computer sentence cannot be clicked past (owner disposition 3). */
export function ConsentSheet({
  open,
  count,
  multiUser = false,
  onAgree,
  onCancel,
}: {
  open: boolean
  count: number
  multiUser?: boolean
  onAgree: () => void
  onCancel: () => void
}) {
  const t = useT()
  const rows: { glyph: string; text: string }[] = [
    { glyph: '✓', text: t('ConsentWhatFormat').replace('{0}', String(count)) },
    { glyph: '🛡', text: t('ConsentNot') },
    // Informed consent (owner 2026-07-17): applying restarts the shell so the desktop re-resolves
    // folder/.url custom icons (the only reliable refresh) — that closes any open folder windows.
    { glyph: '📂', text: t('ConsentRefresh') },
    { glyph: '🔐', text: t('ConsentUac') },
  ]
  return (
    <AnimatePresence>
      {open && (
        <Scrim onClose={multiUser ? undefined : onCancel}>
          <p className="text-cardtitle font-medium text-t1">{t('ConsentTitle')}</p>
          <div className="mt-3 space-y-2.5">
            {rows.map((r) => (
              <div key={r.glyph} className="flex items-start gap-2.5">
                <span aria-hidden className="mt-px w-4 shrink-0 text-center text-[12px]">
                  {r.glyph}
                </span>
                <p className="text-[12px] leading-relaxed text-t2">{r.text}</p>
              </div>
            ))}
          </div>
          {/* Machine-wide arrow disclosure — always shown; set apart from the
              three facts by a hairline (never a left bar, never colour-only).
              On multi-user machines the sheet above is non-skippable so this
              sentence can't be dismissed around. */}
          <p className="mt-3.5 border-t border-hair pt-3 text-[12px] leading-relaxed text-t2">
            {t('ConsentArrow')}
          </p>
          <div className="mt-4 flex justify-end gap-1.5">
            <button
              type="button"
              onClick={onCancel}
              className="rounded-[9px] bg-chip px-3 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
            >
              {t('ConsentCancel')}
            </button>
            <button
              type="button"
              onClick={onAgree}
              className="rounded-[9px] bg-primary px-3 py-1.5 text-[12px] font-medium text-primary-foreground shadow-elev-cta transition-transform active:scale-[0.98]"
            >
              {t('ConsentAgree')}
            </button>
          </div>
        </Scrim>
      )}
    </AnimatePresence>
  )
}

// Owner decree — the body copy names "sixty seconds"; keep them in sync.
// SIXTY seconds, EVERY time (owner decree, re-affirmed 2026-07-09 after a
// brief softening slipped through a batched disposition): the gate exists to
// be obnoxious — to USERS. Developers may shorten it via the dev menu's
// override, which only exists in DEV builds (Vite strips the branch);
// production is hard-wired to 60.
const ARROW_GATE_SECONDS = 60

function arrowGateSeconds(): number {
  if (import.meta.env.DEV) {
    const dev = Number(localStorage.getItem('dm.dev.arrowGateSeconds'))
    if (Number.isFinite(dev) && dev > 0) return dev
  }
  return ARROW_GATE_SECONDS
}

/**
 * 默认箭头 gate (owner decree): choosing the native Windows arrow is legal, but
 * it costs a sixty-second stare at the thing being chosen — the roast caption
 * escalates while the countdown runs. Cancel is instant at all times: misclicks
 * walk free, only conviction waits.
 */
export function ArrowGateSheet({
  open,
  onConfirm,
  onCancel,
}: {
  open: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  const t = useT()
  const [left, setLeft] = React.useState(ARROW_GATE_SECONDS)
  React.useEffect(() => {
    if (!open) return
    setLeft(arrowGateSeconds())
    const timer = setInterval(() => setLeft((v) => Math.max(0, v - 1)), 1000)
    return () => clearInterval(timer)
  }, [open])
  const stare = left > 40 ? t('ArrowGate_Stare1') : left > 15 ? t('ArrowGate_Stare2') : t('ArrowGate_Stare3')
  return (
    <AnimatePresence>
      {open && (
        <Scrim onClose={onCancel}>
          <p className="text-cardtitle font-medium text-t1">{t('ArrowGate_Title')}</p>
          <p className="mt-2 text-[12px] leading-relaxed text-t2">{t('ArrowGate_Body')}</p>
          <div className="mt-4 flex flex-col items-center gap-2 rounded-xl bg-chip/60 py-5">
            <WinArrowGlyph size={88} realistic />
            <p className="text-caption text-t3" aria-live="polite">{stare}</p>
          </div>
          <div className="mt-4 flex justify-end gap-1.5">
            <button
              type="button"
              onClick={onCancel}
              className="rounded-[9px] bg-chip px-3 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
            >
              {t('ArrowGate_Cancel')}
            </button>
            <button
              type="button"
              disabled={left > 0}
              onClick={onConfirm}
              className={cn(
                'rounded-[9px] px-3 py-1.5 text-[12px] font-medium transition-transform',
                left > 0
                  ? 'cursor-not-allowed bg-chip tabular-nums text-t3'
                  : 'bg-primary text-primary-foreground shadow-elev-cta active:scale-[0.98]',
              )}
            >
              {left > 0 ? format(t('ArrowGate_Wait'), left) : t('ArrowGate_Confirm')}
            </button>
          </div>
        </Scrim>
      )}
    </AnimatePresence>
  )
}

/** Post-apply completion: the change happened BEHIND the window — say so, offer to look.
 *  `note` = the module's "last step" line (paper: drag icons into zones). */
export function DoneCard({
  open,
  onClose,
  note,
  ctaLabel,
}: {
  open: boolean
  onClose: () => void
  note?: string
  ctaLabel?: string
}) {
  const t = useT()
  const goSee = () => {
    onClose()
    void call('shell.minimize')
  }
  return (
    <AnimatePresence>
      {open && (
        <Scrim onClose={onClose}>
          <p className="text-cardtitle font-medium text-t1">{t('DoneHeadline')}</p>
          {note && <p className="mt-2 text-[12px] leading-relaxed text-t2">{note}</p>}
          <div className="mt-4 flex justify-end gap-1.5">
            <button
              type="button"
              onClick={onClose}
              className="rounded-[9px] bg-chip px-3 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
            >
              {t('About_Back')}
            </button>
            <button
              type="button"
              onClick={goSee}
              className="rounded-[9px] bg-primary px-3 py-1.5 text-[12px] font-medium text-primary-foreground shadow-elev-cta transition-transform active:scale-[0.98]"
            >
              {ctaLabel ?? t('GoSeeDesktop')}
            </button>
          </div>
        </Scrim>
      )}
    </AnimatePresence>
  )
}
