import { ChevronDown } from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

// The house dropdown (one dialect for every enum-style pick that outgrows a
// segmented): a quiet field trigger showing the current choice, a pop list of
// options. Locale-proof — the trigger truncates gracefully, options wrap never.
// `compact` matches the IconAction scale (22px tall) for inline property rows;
// its option list may grow wider than the trigger so labels stay whole.

export function SelectPopover<T extends string>({
  open,
  setOpen,
  value,
  options,
  onPick,
  compact = false,
}: {
  open: boolean
  setOpen: (open: boolean) => void
  value: T
  options: { value: T; label: string }[]
  onPick: (value: T) => void
  compact?: boolean
}) {
  const current = options.find((o) => o.value === value)
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            'flex w-full items-center justify-between border border-hair bg-chip text-t1',
            compact ? 'h-[22px] gap-1 rounded-[7px] px-1.5 text-[10px]' : 'h-7 rounded-[8px] px-2 text-[12px]',
          )}
        >
          <span className="truncate">{current?.label ?? value}</span>
          <ChevronDown size={compact ? 9 : 11} className="shrink-0 text-t3" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        side="bottom"
        align={compact ? 'end' : 'start'}
        sideOffset={4}
        className="w-auto min-w-[var(--radix-popover-trigger-width)] gap-0 rounded-[10px] p-1 shadow-elev-2"
      >
        {options.map((o) => (
          <button
            key={o.value}
            type="button"
            onClick={() => {
              onPick(o.value)
              setOpen(false)
            }}
            className={cn(
              'flex w-full items-center justify-between gap-2 whitespace-nowrap rounded-[7px] text-left hover:bg-raised-hov',
              compact ? 'h-6 px-1.5 text-[11px]' : 'h-7 px-2 text-[12px]',
              o.value === value ? 'text-coral-ink' : 'text-t1',
            )}
          >
            {o.label}
            {o.value === value && <span className="text-[10px]">✓</span>}
          </button>
        ))}
      </PopoverContent>
    </Popover>
  )
}
