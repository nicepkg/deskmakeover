import * as React from 'react'
import { Shapes } from 'lucide-react'
import { AccordionAxis, ExpandAllToggle } from '@/components/common/accordion-axis'
import { AngleDial } from '@/components/common/angle-dial'
import { Card, CardRow } from '@/components/common/card'
import { Chip, ChipRow } from '@/components/common/chip'
import { ColorDot, MarkGlyph, ShapeSwatch } from '@/components/common/chip-preview'
import { ColorPickerPanel } from '@/components/common/color-picker'
import { CtaButton } from '@/components/common/cta-button'
import type { HeroPhase } from '@/components/common/cta-button'
import { DmSlider } from '@/components/common/dm-slider'
import { LinkChip } from '@/components/common/link-chip'
import { Segmented } from '@/components/common/segmented'
import { ToastHost } from '@/components/common/toast-host'
import { ToggleSwitch } from '@/components/common/toggle-switch'
import type { IconShape, MarkStyle } from '@/bridge/types'
import { useToasts } from '@/stores/toasts'

const SHAPES: { value: IconShape; label: string }[] = [
  { value: 'Apple', label: '苹果' },
  { value: 'Circle', label: '纯圆' },
  { value: 'Samsung', label: '三星' },
  { value: 'None', label: '无' },
  { value: 'Bookmark', label: '书签' },
  { value: 'Lemon', label: '柠檬' },
  { value: 'Tile', label: '瓷砖' },
  { value: 'Teardrop', label: '水滴' },
  { value: 'Diamond', label: '菱形' },
  { value: 'Flower', label: '花瓣' },
  { value: 'Pebble', label: '卵石' },
]

const COLORS: { value: string; label: string }[] = [
  { value: '#FF6F5E', label: '品牌珊瑚' },
  { value: '#3FB6A8', label: '湖水' },
  { value: '#D9A94E', label: '琥珀' },
  { value: '#FFFFFF', label: '纯白' },
  { value: '#141414', label: '纯黑' },
]

const MARKS: { value: MarkStyle; label: string }[] = [
  { value: 'Shadow', label: '双层卡片' },
  { value: 'Halo', label: '幽灵叠影' },
  { value: 'Satin', label: '缎光角' },
  { value: 'Arc', label: '珐琅光弧' },
  { value: 'Fold', label: '卷角' },
  { value: 'Ring', label: '细描边' },
]

// ?debug=components — screenshot surface for the design system (P2 verify gate).
// Not routed in production UI; ships inert.

export function ComponentGallery() {
  const [chip, setChip] = React.useState('苹果')
  const [seg, setSeg] = React.useState<'a' | 'b' | 'c'>('a')
  const [on, setOn] = React.useState(true)
  const [slider, setSlider] = React.useState(62)
  const [angle, setAngle] = React.useState(135)
  const [color, setColor] = React.useState('#FF6F5E')
  const [open, setOpen] = React.useState<Record<string, boolean>>({ 外形: true, 配色: false })
  const [shapePick, setShapePick] = React.useState<IconShape>('Apple')
  const [colorPick, setColorPick] = React.useState('#FF6F5E')
  const [markPick, setMarkPick] = React.useState<MarkStyle>('Shadow')
  const show = useToasts((s) => s.show)
  const [theme, setTheme] = React.useState<'dark' | 'light'>('dark')

  React.useEffect(() => {
    document.documentElement.className = theme
  }, [theme])

  const phases: HeroPhase[] = ['scanning', 'ready', 'working', 'dirty', 'synced']
  const phaseText: Record<HeroPhase, string> = {
    scanning: '正在扫描…',
    ready: '一键美化',
    working: '正在应用…',
    dirty: '更新桌面',
    synced: '✓ 已与桌面同步',
  }

  return (
    <div className="h-screen overflow-auto bg-background p-8 text-foreground">
      <div className="mx-auto grid max-w-4xl grid-cols-2 gap-8">
        <section className="space-y-3">
          <h2 className="text-xs font-semibold tracking-wider text-t2">CTA · HeroPhase</h2>
          {phases.map((p) => (
            <CtaButton key={p} phase={p} onClick={() => show(`CTA: ${p}`)}>
              {phaseText[p]}
            </CtaButton>
          ))}
          <div className="flex gap-2 pt-1">
            <LinkChip onClick={() => show('还原完成', 'success')}>还原</LinkChip>
            <LinkChip>上一版</LinkChip>
            <LinkChip active>历史 3</LinkChip>
            <LinkChip disabled>对比图</LinkChip>
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-xs font-semibold tracking-wider text-t2">Chips · Segmented · Toggle</h2>
          <ChipRow>
            {['苹果', '纯圆', '三星', '花瓣'].map((s) => (
              <Chip key={s} selected={chip === s} onClick={() => setChip(s)}>
                {s}
              </Chip>
            ))}
          </ChipRow>
          <Segmented
            value={seg}
            onChange={setSeg}
            options={[
              { value: 'a', label: '跟随系统' },
              { value: 'b', label: '深色' },
              { value: 'c', label: '浅色' },
            ]}
          />
          <div className="flex items-center gap-3">
            <ToggleSwitch checked={on} onChange={setOn} label="keep up" />
            <span className="text-[12.5px] text-t2">新图标自动跟上</span>
          </div>
          <div className="flex items-center gap-3">
            <DmSlider value={slider} onChange={setSlider} aria-label="strength" />
            <span className="w-9 text-right text-[12.5px] tabular-nums text-t1">{slider}%</span>
          </div>
          <div className="flex items-center gap-4">
            <AngleDial value={angle} onChange={setAngle} />
            <span className="text-[12.5px] tabular-nums text-t1">{angle}°</span>
            <button
              className="rounded-md border border-hair px-2 py-1 text-xs text-t2"
              onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            >
              切换 {theme === 'dark' ? '浅色' : '深色'}
            </button>
          </div>
        </section>

        <section className="space-y-1 rounded-xl bg-raised p-4">
          <div className="mb-1 flex items-center justify-between">
            <h2 className="text-xs font-semibold tracking-wider text-t2">自定义</h2>
            <ExpandAllToggle
              allOpen={Object.values(open).every(Boolean)}
              onToggle={() => {
                const all = Object.values(open).every(Boolean)
                setOpen({ 外形: !all, 配色: !all })
              }}
            />
          </div>
          <AccordionAxis
            first
            title="外形"
            summary={chip}
            open={open.外形}
            onToggle={() => setOpen((o) => ({ ...o, 外形: !o.外形 }))}
          >
            <ChipRow>
              {['苹果', '纯圆', '三星'].map((s) => (
                <Chip key={s} selected={chip === s} onClick={() => setChip(s)}>
                  {s}
                </Chip>
              ))}
            </ChipRow>
          </AccordionAxis>
          <AccordionAxis
            title="配色"
            summary="原彩"
            open={open.配色}
            onToggle={() => setOpen((o) => ({ ...o, 配色: !o.配色 }))}
          >
            <ChipRow>
              <Chip selected>原彩</Chip>
              <Chip>黑白</Chip>
              <Chip>单色</Chip>
            </ChipRow>
          </AccordionAxis>
        </section>

        <section className="space-y-3">
          <h2 className="text-xs font-semibold tracking-wider text-t2">调色盘</h2>
          <div className="w-fit rounded-[14px] bg-popover p-3 shadow-xl">
            <ColorPickerPanel
              value={color}
              onChange={setColor}
              wallpaperSwatches={['#7BA8C4', '#3E5C73', '#D9CBB8', '#8FA893']}
              quickSwatches={['#FFFFFF', '#141414', '#FF6F5E', '#3FB6A8', '#D9A94E', '#E56E9C']}
            />
          </div>
        </section>

        <section className="col-span-2 space-y-4">
          <h2 className="text-section font-semibold text-t1">Chip previews · type ladder</h2>

          {/* Type-ladder specimen — proves the five steps generate as real font-size
              utilities. The card-title step is `text-cardtitle` (renamed from
              `text-card`, which collided with the shadcn `--color-card` colour utility). */}
          <div className="space-y-0.5">
            <p className="text-caption text-t3">type ladder (26 · 19 · 15 · 13 · 11)</p>
            <p className="text-display text-t1">Display 26 · 桌面美颜</p>
            <p className="text-section text-t1">Section 19 · 桌面美颜</p>
            <p className="text-cardtitle text-t1">Cardtitle 15 · 桌面美颜</p>
            <p className="text-body text-t1">Body 13 · 桌面美颜</p>
            <p className="text-caption text-t1">Caption 11 · 桌面美颜</p>
          </div>

          <Card
            icon={<Shapes size={15} />}
            title="预览徽章"
            desc="外形 14px 裁剪 · 配色 10px 圆点 · 标识 22px 微渲染"
          >
            <CardRow
              first
              label="外形裁剪"
              hint="每个形状一枚 14px 实时裁剪"
              trailing={<ShapeSwatch shape={shapePick} active />}
            />
            <CardRow
              label="配色圆点"
              hint="10px 填充圆点"
              trailing={<ColorDot color={colorPick} />}
            />
            <CardRow
              label="快捷标识"
              hint="22px 微渲染"
              trailing={<MarkGlyph mark={markPick} active />}
            />
          </Card>

          <div className="space-y-2">
            <p className="text-caption text-t3">外形 · shape chips (14px live clip swatch)</p>
            <ChipRow>
              {SHAPES.map((s) => (
                <Chip
                  key={s.value}
                  selected={shapePick === s.value}
                  onClick={() => setShapePick(s.value)}
                  leading={<ShapeSwatch shape={s.value} active={shapePick === s.value} />}
                >
                  {s.label}
                </Chip>
              ))}
            </ChipRow>
          </div>

          <div className="space-y-2">
            <p className="text-caption text-t3">配色 · colour chips (10px dot)</p>
            <ChipRow>
              {COLORS.map((c) => (
                <Chip
                  key={c.value}
                  selected={colorPick === c.value}
                  onClick={() => setColorPick(c.value)}
                  leading={<ColorDot color={c.value} />}
                >
                  {c.label}
                </Chip>
              ))}
            </ChipRow>
          </div>

          <div className="space-y-2">
            <p className="text-caption text-t3">标识 · mark chips (22px live render)</p>
            <ChipRow>
              {MARKS.map((m) => (
                <Chip
                  key={m.value}
                  selected={markPick === m.value}
                  onClick={() => setMarkPick(m.value)}
                  leading={<MarkGlyph mark={m.value} active={markPick === m.value} />}
                >
                  {m.label}
                </Chip>
              ))}
            </ChipRow>
          </div>
        </section>
      </div>
      <ToastHost />
    </div>
  )
}
