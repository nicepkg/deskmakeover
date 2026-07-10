import * as React from 'react'
import { Pipette } from 'lucide-react'
import { hexToHsv, hsvToHex, normalizeHex } from '@/lib/color'
import type { Hsv } from '@/lib/color'
import { cn } from '@/lib/utils'

// The shared 调色盘 (spec 02: width 244, SV field 122, hue bar 14, hex mono,
// eyedropper, wallpaper + quick swatch rows). One component, every consumer —
// icon 单色 / 标识配色 / scrim / zone fill / title ink.

interface EyeDropperApi {
  open(): Promise<{ sRGBHex: string }>
}

export function ColorPickerPanel({
  value,
  onChange,
  wallpaperSwatches = [],
  quickSwatches = [],
  className,
}: {
  value: string
  onChange: (hex: string) => void
  wallpaperSwatches?: string[]
  quickSwatches?: string[]
  className?: string
}) {
  // Hue survives while s/v sit at 0 (otherwise the field snaps to red).
  const [hsv, setHsv] = React.useState<Hsv>(() => hexToHsv(value))
  const [hexDraft, setHexDraft] = React.useState(value)
  const svRef = React.useRef<HTMLDivElement>(null)

  React.useEffect(() => {
    const normalized = normalizeHex(value)
    if (normalized && normalized !== hsvToHex(hsv)) {
      setHsv(hexToHsv(normalized))
    }
    setHexDraft(normalizeHex(value) ?? value)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value])

  const commit = (next: Hsv) => {
    setHsv(next)
    const hex = hsvToHex(next)
    setHexDraft(hex)
    onChange(hex)
  }

  const pickSv = (e: { clientX: number; clientY: number }) => {
    const rect = svRef.current!.getBoundingClientRect()
    const s = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width))
    const v = 1 - Math.min(1, Math.max(0, (e.clientY - rect.top) / rect.height))
    commit({ ...hsv, s, v })
  }

  const eyeDropper = (window as { EyeDropper?: new () => EyeDropperApi }).EyeDropper

  return (
    <div className={cn('w-[244px] space-y-2.5', className)}>
      <div
        ref={svRef}
        role="slider"
        aria-label="saturation and brightness"
        aria-valuetext={`s ${Math.round(hsv.s * 100)}%, v ${Math.round(hsv.v * 100)}%`}
        className="relative h-[122px] cursor-crosshair touch-none rounded-[10px]"
        style={{
          background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hsv.h} 100% 50%))`,
        }}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId)
          pickSv(e)
        }}
        onPointerMove={(e) => {
          if (e.buttons & 1) pickSv(e)
        }}
      >
        <span
          className="pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_1px_4px_rgba(0,0,0,0.5)]"
          style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, background: hsvToHex(hsv) }}
        />
      </div>

      <input
        type="range"
        aria-label="hue"
        min={0}
        max={359}
        value={Math.round(hsv.h)}
        onChange={(e) => commit({ ...hsv, h: Number(e.currentTarget.value) })}
        className="dm-hue w-full"
      />

      <div className="flex items-center gap-1.5">
        <span className="size-6 shrink-0 rounded-md border border-hair" style={{ background: hsvToHex(hsv) }} />
        <input
          value={hexDraft}
          onChange={(e) => {
            setHexDraft(e.currentTarget.value)
            const hex = normalizeHex(e.currentTarget.value)
            if (hex) {
              setHsv(hexToHsv(hex))
              onChange(hex)
            }
          }}
          spellCheck={false}
          aria-label="hex"
          className="h-[26px] w-[78px] rounded-md border border-hair bg-chip px-1.5 font-mono text-[11.5px] text-t1 outline-none focus:border-coral/50"
        />
        {eyeDropper && (
          <button
            type="button"
            aria-label="屏幕取色"
            className="flex h-[26px] w-7 items-center justify-center rounded-md border border-hair bg-chip text-t2 hover:text-t1"
            onClick={async () => {
              try {
                const result = await new eyeDropper().open()
                const hex = normalizeHex(result.sRGBHex)
                if (hex) {
                  setHsv(hexToHsv(hex))
                  setHexDraft(hex)
                  onChange(hex)
                }
              } catch {
                // user cancelled the eyedropper — nothing to do
              }
            }}
          >
            <Pipette size={13} />
          </button>
        )}
      </div>

      {wallpaperSwatches.length > 0 && (
        <SwatchRow label="从壁纸自动提取" colors={wallpaperSwatches} active={value} onPick={onChange} />
      )}
      {quickSwatches.length > 0 && (
        <SwatchRow label="快捷选择" colors={quickSwatches} active={value} onPick={onChange} />
      )}
    </div>
  )
}

function SwatchRow({
  label,
  colors,
  active,
  onPick,
}: {
  label: string
  colors: string[]
  active: string
  onPick: (hex: string) => void
}) {
  return (
    <div>
      <p className="mb-1.5 text-[10.5px] text-t3">{label}</p>
      <div className="flex flex-wrap gap-1.5">
        {colors.map((c) => (
          <button
            key={c}
            type="button"
            aria-label={c}
            onClick={() => onPick(c)}
            className={cn(
              'size-5 rounded-full border border-hair transition-transform hover:scale-110',
              normalizeHex(active) === normalizeHex(c) && 'ring-2 ring-coral ring-offset-2 ring-offset-popover',
            )}
            style={{ background: c }}
          />
        ))}
      </div>
    </div>
  )
}
