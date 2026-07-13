import * as React from 'react'
import type { ReactNode } from 'react'
import { Check, ChevronDown, ExternalLink, Info, RotateCcw } from 'lucide-react'
import { InspectorCard } from '@/components/common/inspector'
import { ConfirmSheet } from '@/components/common/ceremony'
import { CtaButton, type HeroPhase } from '@/components/common/cta-button'
import { SurfaceSchematic, applyStaggerDelay } from '@/components/calm/surface-schematic'
import { controlById, type CalmControl, type CalmControlId } from '@/lib/calm/catalog'
import type { CalmRowState } from '@/lib/calm/states'
import { applyCandidates, countOwnedWrites, countQuieted, groupedRows, useCalm } from '@/stores/calm'
import { format, useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// 清爽系统 (spec 08 + the viz panel 2026-07-13): schematic-first, words-second.
// Every row LEADS with a mini-screen wireframe marking the operation area; the
// hero is the honest establishing shot (start panel over taskbar) whose noise
// exits on apply. The tier grouping (one-click / guided / held) is ADR-0023's
// load-bearing honesty and never reorders. Guided rows are NEVER toggles.

export function CalmPage() {
  const t = useT()
  const probed = useCalm((s) => s.probed)
  const op = useCalm((s) => s.op)
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
  const owned = countOwnedWrites(rows)
  const candidates = applyCandidates(rows, excluded)
  const awaiting = groups.oneClick.filter((id) => rows[id] === 'setAwaiting').length

  // Honest hero phases: ready only with real candidates; synced only when our
  // verified writes exist AND nothing is still awaiting; awaiting gets its own
  // non-interactive truth; otherwise the CTA stays quiet — never a false ✓.
  const phase: HeroPhase =
    !probed || op === 'probe'
      ? 'scanning'
      : op !== 'idle'
        ? 'working'
        : candidates.length > 0
          ? 'ready'
          : awaiting > 0
            ? 'scanning'
            : quieted > 0
              ? 'synced'
              : 'scanning'
  const ctaLabel =
    op === 'restore'
      ? t('Calm_Restore_Working')
      : op === 'apply'
        ? t('Calm_Cta_Working')
        : phase === 'synced'
          ? t('Calm_Cta_Done')
          : awaiting > 0 && candidates.length === 0
            ? t('Calm_Cta_Awaiting')
            : t('Calm_Cta')
  const includedNames = candidates.map((id) => t(controlById(id).labelKey)).join(t('Calm_ListJoin'))

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[880px] px-10 py-8">
        <header className="mb-6">
          <h1 className="text-display font-medium text-t1">{t('Panel_CalmTitle')}</h1>
        </header>

        {/* Hero: dynamic words only (owner 2026-07-13) — a fixed establishing image
            would hard-code the starter count and lie the moment the user excludes a
            row or new controls land. The row schematics carry all the visuals. */}
        <div className="mb-6 flex items-center gap-6 rounded-2xl border border-hair bg-raised px-6 py-5">
          <div className="min-w-0 flex-1">
            <p className="text-cardtitle font-medium text-t1">
              {quieted > 0 ? format(t('Calm_Summary'), quieted) : format(t('Calm_CanQuiet'), candidates.length)}
            </p>
            <p className="mt-1.5 text-[12.5px] leading-relaxed text-t2">{t('Calm_HeroPromise')}</p>
          </div>
          <div className="w-[200px] shrink-0">
            <CtaButton phase={phase} onClick={() => setConfirmOpen(true)}>
              {ctaLabel}
            </CtaButton>
            {/* Restore gates on OWNED writes, not the verified count. */}
            {owned > 0 && (
              <button
                type="button"
                onClick={() => void useCalm.getState().restoreAll()}
                className="mt-2 inline-flex w-full items-center justify-center gap-1 text-[12px] text-t3 transition-colors hover:text-t1"
              >
                <RotateCcw size={12} />
                {t('Calm_Restore')}
              </button>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-6">
          {groups.oneClick.length > 0 && (
            <Group label={t('Calm_Group_OneClick')} subtitle={t('Calm_Group_OneClick_Sub')}>
              {groups.oneClick.map((id) => (
                <OneClickRow key={id} id={id} oneClickIds={groups.oneClick} />
              ))}
            </Group>
          )}

          {groups.guided.length > 0 && (
            <Group label={t('Calm_Group_Guided')} subtitle={t('Calm_Group_Guided_Sub')}>
              {groups.guided.map((id) => (
                <GuidedRow key={id} id={id} />
              ))}
            </Group>
          )}

          {groups.held.length > 0 && <HeldGroup ids={groups.held} rows={rows} />}
        </div>
      </div>

      {/* Explain before apply (spec 08 §4) — three lines, no dangling references. */}
      <ConfirmSheet
        open={confirmOpen}
        title={t('Calm_Confirm_Title')}
        body={
          <>
            <span className="block">{format(t('Calm_Confirm_List'), includedNames)}</span>
            <span className="mt-1.5 block">{t('Calm_Confirm_Body')}</span>
            <span className="mt-1.5 block text-t3">{t('Calm_Confirm_GuidedNote')}</span>
          </>
        }
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

/** Group header at cardtitle weight, with a one-line why-this-tier subtitle. */
function Group({ label, subtitle, footer, children }: { label: string; subtitle: string; footer?: string; children: ReactNode }) {
  return (
    <section>
      <p className="mb-0.5 px-1 text-cardtitle font-medium text-t1">{label}</p>
      <p className="mb-2 px-1 text-[12px] text-t3">{subtitle}</p>
      <InspectorCard>{children}</InspectorCard>
      {footer && <p className="mt-2 px-1 text-[11.5px] leading-relaxed text-t3/70">{footer}</p>}
    </section>
  )
}

/** Group 3 — collapsed by default; managed rows carry 「由你的组织管理」. */
function HeldGroup({ ids, rows }: { ids: CalmControlId[]; rows: Record<CalmControlId, CalmRowState> }) {
  const t = useT()
  const [open, setOpen] = React.useState(false)
  const hasUncertified = ids.some((id) => rows[id] === 'unsupported')
  return (
    <section>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="mb-0.5 flex items-center gap-1.5 px-1 text-cardtitle font-medium text-t1 transition-colors hover:text-coral-ink"
      >
        <ChevronDown size={14} className={cn('text-t3 transition-transform', !open && '-rotate-90')} />
        {t('Calm_Group_Held')}
        <span className="rounded-full bg-chip px-1.5 py-0.5 text-[11px] font-normal text-t3">{ids.length}</span>
      </button>
      <p className="mb-2 px-1 pl-6 text-[12px] text-t3">{t('Calm_Group_Held_Sub')}</p>
      {open && (
        <>
          <InspectorCard>
            {ids.map((id) => (
              <HeldRow key={id} id={id} />
            ))}
          </InspectorCard>
          {hasUncertified && (
            <p className="mt-2 px-1 text-[11.5px] leading-relaxed text-t3/70">{t('Calm_Held_Reason_Uncertified')}</p>
          )}
        </>
      )}
    </section>
  )
}

/** One row: the mini-screen schematic leads (WHERE), words follow, control right. */
function RowShell({
  control,
  state,
  schematicDelay = 0,
  compactSchematic = false,
  caption,
  right,
}: {
  control: CalmControl
  state: CalmRowState
  schematicDelay?: number
  compactSchematic?: boolean
  caption?: ReactNode
  right: ReactNode
}) {
  const t = useT()
  return (
    <div className="group/calmrow flex min-h-[80px] items-center gap-4 px-5 py-3.5">
      <SurfaceSchematic
        control={control}
        state={state}
        delay={schematicDelay}
        className={compactSchematic ? 'w-[64px]' : 'w-[104px]'}
      />
      <div className="min-w-0 flex-1">
        <p className="text-body text-t1">{t(control.labelKey)}</p>
        <p className="mt-0.5 text-[12px] text-t3">{t(control.descKey)}</p>
        {control.collateralKey && (
          <p className="mt-1 flex items-center gap-1 text-[11.5px] text-t3">
            <Info size={11} className="shrink-0" />
            {t(control.collateralKey)}
          </p>
        )}
        {caption}
      </div>
      <div className="flex min-w-[132px] shrink-0 items-center justify-end gap-2">{right}</div>
    </div>
  )
}

/** Status chip: verified wears the coral-ink check; everything else calm text. */
function StateChip({ state }: { state: CalmRowState }) {
  const t = useT()
  const text: Partial<Record<CalmRowState, string>> = {
    pending: t('Calm_State_Pending'),
    verified: t('Calm_State_Verified'),
    setAwaiting: t('Calm_State_Awaiting'),
    reverted: t('Calm_State_Reverted'),
    quiet: t('Calm_State_Quiet'),
    external: t('Calm_State_External'),
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
        (state === 'quiet' || state === 'setAwaiting' || state === 'external' || state === 'confirmedOff' ||
          state === 'userAttested' || state === 'needsReconfirm') && 'text-t3',
      )}
    >
      {state === 'verified' && '✓ '}
      {label}
    </span>
  )
}

/** Batch-inclusion checkbox — same grammar as the icons kindPolicy boxes. */
function IncludeCheckbox({ checked, onChange, label }: { checked: boolean; onChange: () => void; label: string }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      onClick={onChange}
      className={cn(
        'flex size-4 items-center justify-center rounded-[5px] border transition-colors',
        checked ? 'border-transparent bg-primary text-primary-foreground' : 'border-hair bg-transparent text-transparent hover:border-t3',
      )}
    >
      <Check size={12} strokeWidth={3} />
    </button>
  )
}

function OneClickRow({ id, oneClickIds }: { id: CalmControlId; oneClickIds: CalmControlId[] }) {
  const t = useT()
  const control = controlById(id)
  const state = useCalm((s) => s.rows[id])
  const excluded = useCalm((s) => s.excluded.has(id))
  const skipReason = useCalm((s) => s.skipReasons[id])
  const selectable = state === 'pushing' || state === 'reverted'
  return (
    <RowShell
      control={control}
      state={state}
      schematicDelay={applyStaggerDelay(id, oneClickIds)}
      caption={skipReason && <p className="mt-1 text-[11.5px] text-t3">{t('Calm_SkipReason_Changed')}</p>}
      right={
        selectable ? (
          <div className="flex items-center gap-2">
            {state === 'reverted' && <StateChip state={state} />}
            <IncludeCheckbox checked={!excluded} onChange={() => useCalm.getState().toggleExcluded(id)} label={t(control.labelKey)} />
          </div>
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
      state={state}
      caption={
        walked && control.routeKey ? (
          <p className="mt-1 text-[11.5px] text-t2">{t(control.routeKey)}</p>
        ) : undefined
      }
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
        state={state}
        compactSchematic
        right={
          <span className="whitespace-nowrap text-[12px] text-t3">
            {state === 'managed' ? t('Calm_Held_Managed') : t('Calm_Held_NotYet')}
          </span>
        }
      />
    </div>
  )
}

function ChipButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-1 whitespace-nowrap rounded-[9px] bg-chip px-2.5 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
    >
      {children}
    </button>
  )
}
