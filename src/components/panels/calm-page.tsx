import * as React from 'react'
import type { ReactNode } from 'react'
import { ExternalLink, RotateCcw } from 'lucide-react'
import { InspectorCard } from '@/components/common/inspector'
import { ConfirmSheet } from '@/components/common/ceremony'
import { CtaButton, type HeroPhase } from '@/components/common/cta-button'
import { ToggleSwitch } from '@/components/common/toggle-switch'
import { controlById, type CalmControl, type CalmControlId } from '@/lib/calm/catalog'
import type { CalmRowState } from '@/lib/calm/states'
import { countQuieted, groupedRows, useCalm } from '@/stores/calm'
import { format, useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// 清爽系统 (spec 08, Direction B): a calm full PAGE — hero strip + three honest
// outcome groups. Page-scale type like 设置 (13px labels, 54px rows). The three
// groups ARE the honesty: what one click can quiet / what we walk you through /
// what this build holds back. Guided rows are NEVER toggles (ADR-0023 D3).

export function CalmPage() {
  const t = useT()
  const probed = useCalm((s) => s.probed)
  const probing = useCalm((s) => s.probing)
  const applying = useCalm((s) => s.applying)
  const rows = useCalm((s) => s.rows)
  const excluded = useCalm((s) => s.excluded)
  const [confirmOpen, setConfirmOpen] = React.useState(false)

  // Guided return-probe: when the window regains focus after a walk, re-check the
  // walked row (readable rows confirm themselves; unreadable ones ask the user).
  React.useEffect(() => {
    const onFocus = () => void useCalm.getState().reProbeWalked()
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [])

  const groups = groupedRows(rows)
  const quieted = countQuieted(rows)
  const pushable = groups.oneClick.filter(
    (id) => rows[id] === 'pushing' && controlById(id).inDefaultPackage && !excluded.has(id),
  ).length

  const phase: HeroPhase = !probed || probing ? 'scanning' : applying ? 'working' : pushable > 0 ? 'ready' : quieted > 0 ? 'synced' : 'scanning'
  const ctaLabel = applying ? t('Calm_Cta_Working') : phase === 'synced' ? t('Calm_Cta_Done') : t('Calm_Cta')

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[880px] px-10 py-8">
        <header className="mb-6">
          <h1 className="text-display font-medium text-t1">{t('Panel_CalmTitle')}</h1>
          <p className="mt-1.5 text-[12.5px] text-t2">{t('Calm_HeroPromise')}</p>
        </header>

        {/* Hero strip: the one-click covers automatic-certified switches ONLY. */}
        <div className="mb-6 flex items-center gap-4">
          <div className="w-[280px] shrink-0">
            <CtaButton phase={phase} onClick={() => setConfirmOpen(true)}>
              {ctaLabel}
            </CtaButton>
          </div>
          {quieted > 0 && (
            <p className="text-[12.5px] text-t2">{format(t('Calm_Summary'), quieted)}</p>
          )}
          {quieted > 0 && (
            <button
              type="button"
              onClick={() => void useCalm.getState().restoreAll()}
              className="ml-auto inline-flex items-center gap-1 text-[12px] text-t3 transition-colors hover:text-t1"
            >
              <RotateCcw size={12} />
              {t('Calm_Restore')}
            </button>
          )}
        </div>

        <div className="flex flex-col gap-6">
          {groups.oneClick.length > 0 && (
            <Group label={t('Calm_Group_OneClick')}>
              {groups.oneClick.map((id) => (
                <OneClickRow key={id} id={id} />
              ))}
            </Group>
          )}

          {groups.guided.length > 0 && (
            <Group label={t('Calm_Group_Guided')}>
              {groups.guided.map((id) => (
                <GuidedRow key={id} id={id} />
              ))}
            </Group>
          )}

          {groups.held.length > 0 && (
            <Group label={t('Calm_Group_Held')} footer={t('Calm_Held_Reason_Uncertified')}>
              {groups.held.map((id) => (
                <HeldRow key={id} id={id} />
              ))}
            </Group>
          )}
        </div>
      </div>

      {/* Explain before apply (spec 08 §4): what changes, what does NOT — the
          guided note names the widgets feed honestly instead of implying 全部. */}
      <ConfirmSheet
        open={confirmOpen}
        title={t('Calm_Confirm_Title')}
        body={`${t('Calm_Confirm_Body')} ${t('Calm_Confirm_GuidedNote')}`}
        confirmLabel={t('Calm_Confirm_Go')}
        cancelLabel={t('Calm_Confirm_Cancel')}
        onConfirm={() => {
          setConfirmOpen(false)
          void useCalm.getState().applyAll()
        }}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  )
}

function Group({ label, footer, children }: { label: string; footer?: string; children: ReactNode }) {
  return (
    <section>
      <p className="mb-2 px-1 text-[12px] font-medium text-t3">{label}</p>
      <InspectorCard>{children}</InspectorCard>
      {footer && <p className="mt-2 px-1 text-[11.5px] leading-relaxed text-t3/70">{footer}</p>}
    </section>
  )
}

function RowShell({ control, right }: { control: CalmControl; right: ReactNode }) {
  const t = useT()
  return (
    <div className="flex min-h-[54px] items-center justify-between gap-6 px-5 py-3">
      <div className="min-w-0">
        <p className="text-body text-t1">{t(control.labelKey)}</p>
        <p className="mt-1 text-[12px] text-t3">{t(control.descKey)}</p>
        {control.collateralKey && <p className="mt-0.5 text-[11.5px] text-t3/70">{t(control.collateralKey)}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{right}</div>
    </div>
  )
}

/** Status chip: quiet text states — verified wears the coral-ink check, pending
 *  breathes, everything else is calm tertiary text. Never a security palette. */
function StateChip({ state }: { state: CalmRowState }) {
  const t = useT()
  const text: Partial<Record<CalmRowState, string>> = {
    pending: t('Calm_State_Pending'),
    verified: t('Calm_State_Verified'),
    setAwaiting: t('Calm_State_Awaiting'),
    reverted: t('Calm_State_Reverted'),
    quiet: t('Calm_State_Quiet'),
    confirmedOff: t('Calm_State_ConfirmedOff'),
    userAttested: t('Calm_State_UserAttested'),
    needsReconfirm: t('Calm_State_NeedsReconfirm'),
  }
  const label = text[state]
  if (!label) return null
  return (
    <span
      className={cn(
        'whitespace-nowrap text-[12px]',
        state === 'verified' && 'font-medium text-coral-ink',
        state === 'pending' && 'animate-pulse text-t3',
        state === 'reverted' && 'text-t2',
        (state === 'quiet' || state === 'setAwaiting' || state === 'confirmedOff' || state === 'userAttested' || state === 'needsReconfirm') &&
          'text-t3',
      )}
    >
      {state === 'verified' && '✓ '}
      {label}
    </span>
  )
}

function OneClickRow({ id }: { id: CalmControlId }) {
  const t = useT()
  const control = controlById(id)
  const state = useCalm((s) => s.rows[id])
  const excluded = useCalm((s) => s.excluded.has(id))
  return (
    <RowShell
      control={control}
      right={
        state === 'pushing' ? (
          // Exclusion semantics (ADR-0004 §2): checked = stays in the package.
          <ToggleSwitch checked={!excluded} onChange={() => useCalm.getState().toggleExcluded(id)} label={t(control.labelKey)} />
        ) : (
          <StateChip state={state} />
        )
      }
    />
  )
}

function GuidedRow({ id }: { id: CalmControlId }) {
  const t = useT()
  const control = controlById(id)
  const state = useCalm((s) => s.rows[id])
  const walked = useCalm((s) => s.walkedId === id)
  const settled = state === 'confirmedOff' || state === 'userAttested'
  // Unreadable rows we walked: the app cannot know — ask, and record it as YOURS.
  const askAttest = walked && !control.readableState && state === 'pushing'
  return (
    <RowShell
      control={control}
      right={
        settled ? (
          <StateChip state={state} />
        ) : askAttest ? (
          <div className="flex items-center gap-1.5">
            <span className="text-[12px] text-t3">{t('Calm_Guided_ConfirmAsk')}</span>
            <ChipButton onClick={() => useCalm.getState().attestGuided(id)}>{t('Calm_Guided_ConfirmYes')}</ChipButton>
            <ChipButton onClick={() => void useCalm.getState().walkGuided(id)}>{t('Calm_Guided_Again')}</ChipButton>
          </div>
        ) : (
          // NEVER a toggle (ADR-0023 D3): a guided row is an action, and says so.
          <ChipButton onClick={() => void useCalm.getState().walkGuided(id)}>
            <ExternalLink size={12} />
            {t('Calm_GoGuided')}
          </ChipButton>
        )
      }
    />
  )
}

function HeldRow({ id }: { id: CalmControlId }) {
  const t = useT()
  const control = controlById(id)
  const state = useCalm((s) => s.rows[id])
  return (
    <div className="opacity-60">
      <RowShell
        control={control}
        right={<span className="whitespace-nowrap text-[12px] text-t3">{state === 'managed' ? t('Calm_Held_Managed') : null}</span>}
      />
    </div>
  )
}

function ChipButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-1 rounded-[9px] bg-chip px-2.5 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
    >
      {children}
    </button>
  )
}
