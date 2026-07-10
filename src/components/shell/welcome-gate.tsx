import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
const appIcon = '/app-icon.svg'
import { WinArrowGlyph } from '@/components/common/chip-preview'
import { useApp } from '@/stores/app'
import { format, useI18n, useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// First-run welcome + the door password (owner decree, VOC-informed): a brand
// beat, then ONE gate question on the native shortcut arrow. Agreeing walks in
// with a promise that the distinction FUNCTION survives (the top-liked
// objection, answered at the door). Disagreeing gets the owner's verbatim
// send-off — and the "go uninstall" button calls the bluff: the app knows the
// loudest critics downloaded it anyway. Every exit path ends INSIDE the app.
//
// Shown once: localStorage `dm.welcome.done`. Title bar stays interactive above.

const DONE_KEY = 'dm.welcome.done'

// The language roster, OS-setup style (iOS/macOS first boot): every entry is
// SELF-LABELED in its own tongue — that's the whole i18n story for this screen,
// no caption translation needed. Supporting a new language = one more row.
const LANGUAGES: { value: 'zh-Hans' | 'en'; label: string }[] = [
  { value: 'zh-Hans', label: '简体中文' },
  { value: 'en', label: 'English' },
]

export function welcomePending(): boolean {
  try {
    return localStorage.getItem(DONE_KEY) !== '1'
  } catch {
    return false
  }
}

type Step = 'lang' | 'brand' | 'gate' | 'gate2' | 'in' | 'roast' | 'bluff'

/** Loose match for the typed confession: whitespace collapsed, CJK/latin
 *  punctuation unified. The words must all be there; the keyboard layout may差. */
function normalizeConfession(s: string): string {
  return s.replace(/\s+/g, '').replace(/，/g, ',').replace(/。/g, '.').replace(/！/g, '!')
}

export function WelcomeGate({ open, onDone }: { open: boolean; onDone: () => void }) {
  const t = useT()
  const reduced = useReducedMotion()
  const lang = useI18n((s) => s.lang)
  const [step, setStep] = React.useState<Step>('lang')

  // Language first (owner call): the OS locale can be English while the native
  // tongue is Chinese. Tapping a row SELECTS and live-previews the language;
  // 继续 commits (misclick-proof, owner call) — and the re-apply on confirm also
  // wins the race against boot()'s initial settings load stomping the pick.
  const [picked, setPicked] = React.useState<'zh-Hans' | 'en'>(lang)
  const pickLanguage = (language: 'zh-Hans' | 'en') => {
    setPicked(language)
    void useApp.getState().updateSettings({ language })
  }
  const confirmLanguage = () => {
    void useApp.getState().updateSettings({ language: picked })
    setStep('brand')
  }

  // The gate answers: chosen first, committed by 继续 (same dialect as language).
  // BOTH questions wear an innocent survey face — nothing on screen may hint
  // that the answers route anywhere (owner decree: no guard raised, no gaming),
  // and judgment happens only after the LAST answer, like a real survey would.
  const [gatePick, setGatePick] = React.useState<'yes' | 'no' | null>(null)
  const [gate2Pick, setGate2Pick] = React.useState<'yes' | 'no' | null>(null)

  // The bluff screen's fake tally — "holdout #N today". Stable per mount,
  // never 1 (being first would flatter them).
  const holdoutNo = React.useMemo(() => 17 + Math.floor(Math.random() * 70), [])

  // The typed confession (owner decree): hand-copied, character for character.
  // Paste and drop are refused with a taunt — the hands must do the work.
  const [confession, setConfession] = React.useState('')
  const [pasteTried, setPasteTried] = React.useState(false)
  const confessionDone = normalizeConfession(confession) === normalizeConfession(t('Welcome_Confession'))

  const finish = () => {
    try {
      localStorage.setItem(DONE_KEY, '1')
    } catch {
      /* privacy mode: the gate simply shows again next launch */
    }
    onDone()
  }

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="absolute inset-0 z-40 grid place-items-center bg-background p-8"
          initial={false}
          exit={{ opacity: 0 }}
          transition={{ duration: reduced ? 0 : 0.24, ease: [0.33, 1, 0.68, 1] }}
        >
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={step}
              className={cn(
                'w-full',
                // Editorial, not a centered totem pole (owner call): the brand
                // beat is a two-column composition; text steps are a LEFT-set
                // column whose ragged edge does the layout work.
                step === 'brand'
                  ? 'grid max-w-[660px] grid-cols-[1.05fr_0.95fr] items-center gap-12'
                  : step === 'lang'
                    ? 'flex max-w-[380px] flex-col items-center text-center'
                    : 'flex max-w-[420px] flex-col items-start text-left',
              )}
              initial={{ opacity: 0, y: reduced ? 0 : 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: reduced ? 0 : -8 }}
              transition={{ duration: reduced ? 0 : 0.2, ease: [0.33, 1, 0.68, 1] }}
            >
              {step === 'lang' && (
                <>
                  <img src={appIcon} alt="" className="size-[56px] drop-shadow-sm" />
                  {/* One universal word (owner call) — the rows below speak for themselves. */}
                  <h2 className="mt-5 text-cardtitle font-medium text-t1">Language</h2>
                  <div className="mt-5 w-full">
                    <ChoiceList options={LANGUAGES} value={picked} onPick={pickLanguage} />
                  </div>
                  {/* Right-set, content-width — the macOS setup convention. */}
                  <PrimaryButton className="mt-5 self-end" onClick={confirmLanguage}>
                    {t('Welcome_Continue')}
                  </PrimaryButton>
                </>
              )}

              {step === 'brand' && (
                <>
                  <div className="text-left">
                    <h1 className="text-[30px] font-medium leading-tight tracking-[-0.01em] text-t1">
                      {t('AppTitle')}
                    </h1>
                    <p className="mt-3 max-w-[26ch] text-[13px] leading-relaxed text-t2">{t('Welcome_Promise')}</p>
                    <PrimaryButton className="mt-8" onClick={() => setStep('gate')}>
                      {t('Welcome_Start')}
                    </PrimaryButton>
                  </div>
                  <BrandMosaic reduced={!!reduced} />
                </>
              )}

              {step === 'gate' && (
                <>
                  <p className="text-caption text-t3">{t('Welcome_GateTitle')}</p>
                  <div className="mt-4 flex items-center gap-3">
                    {/* The exhibit itself, framed like the mosaic's villain tile. */}
                    <motion.span
                      initial={reduced ? false : { scale: 0.85, rotate: -6, opacity: 0 }}
                      animate={{ scale: 1, rotate: 3, opacity: 1 }}
                      transition={{ type: 'spring', stiffness: 380, damping: 22, delay: 0.1 }}
                      className="grid size-12 shrink-0 place-items-center rounded-[14px] border border-hair bg-raised"
                    >
                      <WinArrowGlyph size={30} realistic />
                    </motion.span>
                    <h2 className="text-cardtitle font-medium leading-snug text-t1">{t('Welcome_GateQuestion')}</h2>
                  </div>
                  <div className="mt-6 w-full">
                    <ChoiceList
                      options={[
                        { value: 'yes', label: t('Welcome_GateYes') },
                        { value: 'no', label: t('Welcome_GateNo') },
                      ]}
                      value={gatePick}
                      onPick={setGatePick}
                    />
                  </div>
                  <PrimaryButton
                    className="mt-5 self-end"
                    disabled={gatePick === null}
                    onClick={() => setStep('gate2')}
                  >
                    {t('Welcome_Continue')}
                  </PrimaryButton>
                </>
              )}

              {step === 'gate2' && (
                <>
                  <p className="text-caption text-t3">{t('Welcome_GateTitle')}</p>
                  <h2 className="mt-4 text-cardtitle font-medium leading-snug text-t1">{t('Welcome_Gate2Question')}</h2>
                  <div className="mt-6 w-full">
                    <ChoiceList
                      options={[
                        { value: 'yes', label: t('Welcome_Gate2Yes') },
                        { value: 'no', label: t('Welcome_Gate2No') },
                      ]}
                      value={gate2Pick}
                      onPick={setGate2Pick}
                    />
                  </div>
                  <PrimaryButton
                    className="mt-5 self-end"
                    disabled={gate2Pick === null}
                    onClick={() => setStep(gatePick === 'yes' && gate2Pick === 'yes' ? 'in' : 'roast')}
                  >
                    {t('Welcome_Continue')}
                  </PrimaryButton>
                </>
              )}

              {step === 'in' && (
                <>
                  <h2 className="text-section font-medium text-t1">{t('Welcome_In')}</h2>
                  <p className="mt-3 text-caption text-t3">{t('Welcome_GateYesNote')}</p>
                  <PrimaryButton className="mt-8" onClick={finish}>
                    {t('Welcome_EnterCta')}
                  </PrimaryButton>
                </>
              )}

              {step === 'roast' && (
                <>
                  <p className="text-caption text-t3">{t('Welcome_RoastTitle')}</p>
                  <h2 className="mt-3 text-section font-medium leading-snug text-t1">{t('Welcome_RoastBody')}</h2>
                  <div className="mt-8 flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        setGatePick(null)
                        setGate2Pick(null)
                        setStep('gate')
                      }}
                      className="rounded-[10px] bg-chip px-4 py-2 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
                    >
                      {t('Welcome_RoastRethink')}
                    </button>
                    <PrimaryButton
                      onClick={() => {
                        setConfession('')
                        setPasteTried(false)
                        setStep('bluff')
                      }}
                    >
                      {t('Welcome_RoastUninstall')}
                    </PrimaryButton>
                  </div>
                </>
              )}

              {step === 'bluff' && (
                <>
                  <h2 className="text-cardtitle font-medium leading-snug text-t1">
                    {format(t('Welcome_BluffBody'), holdoutNo)}
                  </h2>
                  <div className="mt-5 w-full rounded-xl border border-hair bg-chip/60 px-4 py-3">
                    <p className="text-caption text-t3">{t('Welcome_CopyPrompt')}</p>
                    <p className="mt-1 select-none text-[13px] font-medium text-t1">{t('Welcome_Confession')}</p>
                  </div>
                  <input
                    value={confession}
                    onChange={(e) => setConfession(e.currentTarget.value)}
                    onPaste={(e) => {
                      e.preventDefault()
                      setPasteTried(true)
                    }}
                    onDrop={(e) => e.preventDefault()}
                    placeholder={t('Welcome_TypeHere')}
                    aria-label={t('Welcome_CopyPrompt')}
                    className="mt-3 w-full rounded-[10px] border border-hair bg-raised px-3 py-2 text-[13px] text-t1 outline-none transition-colors placeholder:text-t3 focus:border-coral/60"
                  />
                  {pasteTried && (
                    <motion.p
                      initial={{ opacity: 0, y: 4 }}
                      animate={{ opacity: 1, y: 0 }}
                      className="mt-2 text-caption font-medium text-coral-ink"
                    >
                      {t('Welcome_NoPaste')}
                    </motion.p>
                  )}
                  <PrimaryButton className="mt-5 self-end" disabled={!confessionDone} onClick={finish}>
                    {confessionDone ? t('Welcome_BluffCta') : t('Welcome_BluffCtaLocked')}
                  </PrimaryButton>
                </>
              )}
            </motion.div>
          </AnimatePresence>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

/**
 * The brand-beat artwork: a loose collage of desktop "tiles" — the app icon
 * leading, clean shaped tiles around it, and ONE greyed tile wearing the native
 * arrow (the villain cameo; the very next screen asks about it). Flat surfaces,
 * token colours, slight rotations; entrance staggers, then everything is still.
 */
function BrandMosaic({ reduced }: { reduced: boolean }) {
  const enter = (i: number) =>
    reduced
      ? {}
      : {
          initial: { opacity: 0, y: 12, scale: 0.96 },
          animate: { opacity: 1, y: 0, scale: 1 },
          transition: { duration: 0.32, delay: 0.08 + i * 0.07, ease: [0.33, 1, 0.68, 1] as const },
        }
  return (
    <div aria-hidden className="relative aspect-square w-full max-w-[264px] justify-self-center">
      {/* quiet backdrop plate — depth without a gradient */}
      <motion.div {...enter(0)} className="absolute left-[8%] top-[10%] size-[74%] -rotate-6 rounded-[36px] bg-chip/70" />
      {/* the app icon, leading the composition */}
      <motion.img
        {...enter(1)}
        src={appIcon}
        alt=""
        className="absolute left-[20%] top-[16%] w-[44%] -rotate-3 drop-shadow-md"
      />
      {/* a clean beautified tile */}
      <motion.div
        {...enter(2)}
        className="absolute right-[5%] top-[7%] grid size-[27%] rotate-6 place-items-center rounded-[26%] border border-coral/35 bg-wash-chip"
      >
        <span className="size-1/2 rounded-[30%] bg-coral/85" />
      </motion.div>
      {/* the villain: a greyed tile still wearing the native arrow */}
      <motion.div
        {...enter(3)}
        className="absolute bottom-[9%] left-[4%] grid size-[30%] rotate-3 place-items-center rounded-[26%] border border-hair bg-raised opacity-80"
      >
        <WinArrowGlyph size={34} realistic className="opacity-70 grayscale" />
      </motion.div>
      {/* a small quiet tile balancing the corner */}
      <motion.div
        {...enter(4)}
        className="absolute bottom-[4%] right-[12%] size-[19%] -rotate-12 rounded-[28%] bg-t3/20"
      />
      {/* sparkles echoing the app icon */}
      <motion.span {...enter(5)} className="absolute right-[26%] top-[44%] text-coral">
        <Sparkle size={14} />
      </motion.span>
      <motion.span {...enter(6)} className="absolute bottom-[20%] left-[40%] text-coral/70">
        <Sparkle size={9} />
      </motion.span>
    </div>
  )
}

function Sparkle({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path d="M12 0 L14.6 9.4 L24 12 L14.6 14.6 L12 24 L9.4 14.6 L0 12 L9.4 9.4 Z" fill="currentColor" />
    </svg>
  )
}

function PrimaryButton({
  className,
  disabled = false,
  onClick,
  children,
}: {
  className?: string
  disabled?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'rounded-[10px] px-5 py-2 text-[13px] font-medium transition-all duration-150',
        disabled
          ? 'cursor-not-allowed bg-chip text-t3'
          : 'bg-primary text-primary-foreground shadow-elev-cta active:scale-[0.98]',
        className,
      )}
    >
      {children}
    </button>
  )
}

/** THE welcome-flow selection dialect (one grammar for language AND the gate):
 *  an inset list of rows — label left, a spring-popped ✓ right, coral ink when
 *  picked. Choices select; a right-set 继续 commits. */
function ChoiceList<T extends string>({
  options,
  value,
  onPick,
}: {
  options: { value: T; label: string }[]
  value: T | null
  onPick: (value: T) => void
}) {
  return (
    <div className="w-full divide-y divide-hair overflow-hidden rounded-xl border border-hair bg-raised text-left">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          aria-pressed={value === o.value}
          onClick={() => onPick(o.value)}
          className={cn(
            'flex h-11 w-full items-center justify-between px-4 text-[13px] transition-colors hover:bg-raised-hov',
            value === o.value ? 'font-medium text-coral-ink' : 'text-t1',
          )}
        >
          {o.label}
          {value === o.value && (
            <motion.span
              initial={{ scale: 0.4, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ type: 'spring', stiffness: 520, damping: 26 }}
              className="text-[11px]"
            >
              ✓
            </motion.span>
          )}
        </button>
      ))}
    </div>
  )
}
