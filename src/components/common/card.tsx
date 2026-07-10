import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

// Grouped inset card (spec 02 v2 · ADR-0012) — the layout unit of settings and the
// control panels. A raised, ELEVATED surface (separation by light, not lines) with an
// optional icon-badge header and hairline-separated rows (the macOS inset-list idiom).

export function Card({
  icon,
  title,
  desc,
  onDescClick,
  trailing,
  className,
  children,
}: {
  icon?: ReactNode
  title?: string
  desc?: ReactNode
  onDescClick?: () => void
  trailing?: ReactNode
  className?: string
  children?: ReactNode
}) {
  const hasHeader = title || icon || trailing
  return (
    <section className={cn('rounded-2xl bg-raised p-5 shadow-elev-1', className)}>
      {hasHeader && (
        <div className={cn('flex items-start justify-between gap-4', children && 'mb-3')}>
          <div className="flex items-start gap-2.5">
            {icon && (
              <span className="mt-0.5 flex size-7 items-center justify-center rounded-lg bg-wash-rail text-coral-ink">
                {icon}
              </span>
            )}
            {(title || desc) && (
              <div className="min-w-0">
                {title && <h3 className="text-cardtitle font-semibold text-t1">{title}</h3>}
                {desc &&
                  (onDescClick ? (
                    <button
                      type="button"
                      onClick={onDescClick}
                      className="mt-0.5 text-caption text-t3 underline-offset-2 hover:text-t1 hover:underline"
                    >
                      {desc}
                    </button>
                  ) : (
                    <p className="mt-0.5 text-caption text-t3">{desc}</p>
                  ))}
              </div>
            )}
          </div>
          {trailing}
        </div>
      )}
      {children}
    </section>
  )
}

// A hairline-separated row inside a grouped card. `first` drops the top divider.
export function CardRow({
  label,
  hint,
  trailing,
  first = false,
  className,
  children,
}: {
  label?: string
  hint?: string
  trailing?: ReactNode
  first?: boolean
  className?: string
  children?: ReactNode
}) {
  return (
    <div
      className={cn(
        'flex items-center justify-between gap-4 py-3',
        !first && 'border-t border-hair',
        className,
      )}
    >
      {(label || hint || children) && (
        <div className="min-w-0 flex-1">
          {label && <p className="text-body text-t1">{label}</p>}
          {hint && <p className="mt-0.5 text-caption text-t3">{hint}</p>}
          {children}
        </div>
      )}
      {trailing}
    </div>
  )
}
