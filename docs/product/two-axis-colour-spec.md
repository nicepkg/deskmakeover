# 两轴颜色重构 · 主体 × 底板（Two-Axis Colour Spec）

- Status: Draft for implementation (chief-designer verdict, Owner-approved 2026-07-10)
- Supersedes: `ColorMode` 四值单枚举 + `plateColor` 旁挂字段的耦合模型
- Grounded in: `bridge/types.ts` (ADR-0016/0017)、`icon-compositor/color.ts`（frozen oracle port）、`lib/type-config.ts`
- 语言约定：散文中文，代码标识符/枚举/组件名 English，chip 标签中英双语

---

## 0. 结论与命名

乱源不是缺前景/背景拆分，是 **`ColorMode` 把「底板行为」伪装成「前景模式」**：`Field/满彩` 根本不是一种前景处理——`color.ts` 证实它是「主体原彩（law 4：subject 像素永不动）× 逐图标派生底板（`themedContrastTone`）」。它以模式姿态把底板绑进前景枚举，才逼出 `plateColor`「Original/Mono 生效、BW 到 v2 才生效」这种按模式分叉的天书，以及「全局 Original 时类型手风琴隐藏底板行 → 白板无处换色」的缺口。

**解法：把模式溶解成两条正交轴。**

| 轴 | 英文 | 管什么 | 默认 |
|---|---|---|---|
| **主体** | Subject | 图标画面本身怎么渲染（原彩/黑白/单色） | 原彩 Original |
| **底板** | Plate | 形状容器里填什么背景（随图标/白/低饱和色） | 随图标 Auto |

`满彩/Field` 从模式降级为**预设**（主体原彩 × 底板随图标·鲜明），出厂默认，零功能损失。滤镜（`filter`）、角标（`distinction/markStyle`）、统一角标形状（`shortcutShape`）是独立轴，本 spec 不动。

---

## 1. 两轴档位表

### 1.1 主体轴 Subject（前景处理）

| chip | glyph | 标签 中/英 | 选中语义 | 披露件 |
|---|---|---|---|---|
| 原彩 | `FieldGlyph` 三色点（复用，改义为「多色保留」） | 原彩 / Original | 主体保留原色（identity，`transformPixel` 直返）。**默认** | 无 |
| 黑白 | `BwGlyph` 黑\|白对半分割盘（v3 已修） | 黑白 / B&W | 主体逐像素灰度（`grayValue`，Rec.601 luma） | 无 |
| 单色·色点 | `SwatchDot` 实心色点 × N（黑/棕/珊/青/琥…） | 单色 / Mono | 主体按该 tint 走 OKLab 色调映射（`monoMapAdaptive`）。选中任一色点即进单色 | ‹渐层 Tonal \| 纯平 Flat›（MonoStyle） |
| 单色·自定义 | `WheelRing` 调色盘轮 | 自定义 / Custom | 打开单一色相拾取器，写 `tint` | 同上 |

- **「单色」不是一个独立 chip**：它由「选中任意色点或调色盘」隐式表达（正如旧 `Field` 是伪档，旧 `Segmented(Field/Mono/B&W)` 冗余）。旧三键段控整个删除。
- 披露件 `渐层|纯平` 仅在主体=单色时出现：`渐层`=`monoMapAdaptive` 明暗色调坡（经典）；`纯平`=极致单色（主体压平为单一 tint，配平板底）。

### 1.2 底板轴 Plate（背景）

| chip | glyph | 标签 中/英 | 选中语义 | 披露件 |
|---|---|---|---|---|
| 随图标 | **4 象限多色小板**（新 glyph，§5） | 随图标 / Auto | `plateColor=null`：底板色由「主导信号」派生（§2.2）。**默认** | ‹鲜明 Vivid \| 柔和 Quiet›（仅主体=原彩时，见 §2.3） |
| 白 | `SwatchDot` 白点（**加重环+内发丝**，v3 修法） | 白 / White | `plateColor='#FFFFFF'`。`clampPlateLightness` 白永远是白（law 1/4 near-neutral 豁免） | 无 |
| 低饱和色 | `SwatchDot` 实心低饱和点 × 6 | 底色 / Tint | `plateColor=` 该低饱和 hex；`clampPlateLightness` 夹入 `[0.6,0.8]` 亮窗 | 无 |
| 自定义 | `WheelRing` 调色盘轮 | 自定义 / Custom | 打开单一色相拾取器，写任意 `plateColor` hex（**仅全局**，类型级无此档，见 §3.3） | 无 |

### 1.3 「无板」裁决 ⛔ 不设此档

**底板轴不新增「无 None」档。** 「没有容器/无板」的语义已由 **形状轴 `shape=None`** 表达（无 mask = 无 fill = 无板）；再在底板轴造一个「无」= 两个入口表达同一件事，正是要消灭的重复语义。裁决落地：

- **底板需要容器才有意义**：当 `shape=None` 时，**底板整行 40% 禁用 + 不可点**（不是隐藏——隐藏就是原缺口的病根）。行尾附灰字提示「选一个形状后可换底色 / Pick a shape to set a plate」。
- `shape≠None` 时底板行恢复可用，容器 fill = 底板轴当前值。
- 因此「无板」= 去形状轴选 `None`，全局只有唯一一条路径表达它。

---

## 2. 合法组合矩阵

主体 ∈ {原彩, 黑白, 单色(tint,monoStyle)}；底板 ∈ {随图标(band), 白, 低饱和色}（`shape=None` 时底板 N/A）。

### 2.1 全格子行为

| 主体 ＼ 底板 | 随图标 Auto | 白 White | 低饱和色 Tint |
|---|---|---|---|
| **原彩** | = 旧 `满彩/Field`：主体原色，底板 `themedContrastTone`（同色相、亮度反差），band 控深浅。**出厂默认** | 主体原色 × 纯白板（极简/干净） | 主体原色 × 手选低饱和板 |
| **黑白** | 主体灰度 × **中性亮度板** `neutralContrastTone`（law：绝不给灰图强上色相；亮主体→暗板，暗主体→亮板） | 主体灰度 × 纯白板（经典极简黑白） | 主体灰度 × 手选低饱和板 |
| **单色(tint)** | 主体色调映射 × **底板=该 tint 的 ramp 近白端**（`buildRamp` 光端 L0.965/C0.22）→ 见 §2.2 撞色消解 | 主体色调 × 纯白板 | 主体色调 × 手选板（跨色相由用户负责） |

### 2.2 「随图标」的派生规则（撞色默认消解）

`随图标` 不是「永远取图标色」，而是 **「底板色跟随主导信号」**，主导信号按主体自适应——这条规则让单色永不与底板吵架：

- **主体=原彩** → 主导信号 = 图标自身主导色 → `themedContrastTone(seed, subjectMeanL, band)`（有彩）/ `neutralContrastTone`（画面本身无彩）。
- **主体=黑白** → 画面已无彩 → `neutralContrastTone`（纯亮度中性板）。
- **主体=单色(tint)** → 主导信号 = **所选 tint**，底板 = 该 tint 的 ramp 光端（近白微染）→ 主体与底板**同一色相**，不撞。这正是旧 `Mono + plateColor=null = ramp light end` 的既有语义，无新逻辑。

> 撞色只可能发生在用户**显式**「单色 × 手选一个不同色相的低饱和板」——那是用户主动越界，尊重其选择不代劳消解（explicit > auto）。

### 2.3 派生底板的特例

- **无彩 App 桶图标**（灰 .exe/工具）× 随图标：不吃中性板，改按 `APP_ACCENT_SEEDS` 确定性轮转一个品牌重音色（`lib/type-config.ts`，2026-07-10 Owner 特例）——程序永远是最响的一层；其余桶保持纯中性律。
- `鲜明|柔和`（band）只对**有彩派生板**（主体=原彩 × 随图标）有效：`themedContrastTone` 的 `lightL/darkL` 随 band 变。主体=黑白时板为中性（band inert）；主体=单色时深浅由 `渐层|纯平` 接管。**故 `鲜明|柔和` 披露件仅在「底板=随图标 且 主体=原彩」时出现。**

---

## 3. 数据模型与迁移映射

### 3.1 新 DTO（`ConfigDto` 颜色相关字段）

```ts
// bump BRIDGE_SCHEMA_VERSION 3 → 4；C# Contracts.cs / IconsContracts.cs 同步
type Subject = 'Original' | 'BlackWhite' | 'Mono'   // was ColorMode, 去掉 'Field'
type PlateBand = 'Vivid' | 'Quiet'                  // was FieldBand（重命名，语义=派生底板深浅带）

interface ConfigDto {
  // …非颜色字段不变（shape, shortcutShape, distinction, markStyle, size, filter…）
  subject: Subject
  tint: string          // 主体=Mono 时生效
  monoStyle: MonoStyle  // 'Tonal' | 'Flat'，主体=Mono 时披露
  plateColor: string | null   // null=随图标；'#FFFFFF'=白；低饱和 hex=底色；其它 hex=全局自定义
  plateBand: PlateBand  // 底板=随图标 且 主体=Original 时生效
}
```

- 底板档位**由 `plateColor` 派生**，不加新判别字段：`null`→随图标 chip，`'#FFFFFF'`→白 chip，低饱和集内→底色 chip，其它→自定义。
- `plateColor` 语义**拓宽到全部主体**（旧只 Original/Mono 生效、BW inert）；BW 的「until v2」= 现在。

### 3.2 迁移映射表（旧 → 新，确定、无损、单向、保外观）

| 旧 colorMode | 旧 plateColor | → subject | → plateColor | → plateBand | 备注 |
|---|---|---|---|---|---|
| Field | null | Original | **null**（随图标） | = 旧 fieldBand | 满彩原样 |
| Field | `#hex` | Original | `#hex` | — | 手选覆盖派生 |
| Original | null | Original | **`#FFFFFF`**（白） | — | 旧 Original 空板≈白，物化保外观 |
| Original | `#hex` | Original | `#hex` | — | |
| BlackWhite | null | BlackWhite | **`#FFFFFF`**（白） | — | 旧 BW 板 inert=白 |
| BlackWhite | `#hex` | BlackWhite | **`#FFFFFF`**（白） | — | 旧 inert，保外观取白；stored hex 现可激活（见开放问题③） |
| Mono | null | Mono | **null**（随图标=tint ramp 光端） | — | tint/monoStyle 原样带过 |
| Mono | `#hex` | Mono | `#hex` | — | tint/monoStyle 原样带过 |

`tint / monoStyle` 恒等带过；`fieldBand`→`plateBand`。迁移一次性跑在：当前 config + 四预设 + 全部历史条目 + 全部 `TypeOverrides.patch`。

### 3.3 TypePatch 重定义（类型减档规则）

```ts
interface TypePatch {
  shape?: IconShape
  subject?: 'BlackWhite' | 'Mono'   // ⛔ 原彩排除：类型不许「原色岛」（ADR-0017 D3）
  tint?: string
  monoStyle?: MonoStyle
  plateColor?: string | null        // 有界：仅 null(随图标) | '#FFFFFF'(白) | 六低饱和之一。⛔ 无自由 hex
  plateBand?: PlateBand             // 继承的主体=原彩 × 随图标 时的深浅
}
```

**减档法则一句话：类型只能「让某类退下去」，不能「让某类跳出来」。**
- 主体去掉 `原彩`（no colour island）；保留 `黑白/单色/单色自定义`（皆属去饱和族，合规）。
- 底板去掉自由 `调色盘`（ADR「bounded plate」）；只留 `随图标/白/六低饱和`。
- 每轴仍首位 `跟随全局` 锚点（复用 v3 已发机制）；跟随进全局原彩 = 全桌统一 ≠ island，合法。

**旧 TypePatch 迁移（顺带修掉合约内在矛盾）**：旧 `TypePatch.colorMode` 竟含 `Field`（原色主体），与「never Original islands」冲突。新映射自然消解：

| 旧 type colorMode | → type subject | → type plateColor |
|---|---|---|
| Field | **unset（跟随全局）** | null（随图标）+ plateBand=fieldBand |
| Mono | Mono（带 tint/monoStyle） | 旧 plateColor（限有界集，越界→就近夹白） |
| BlackWhite | BlackWhite | `#FFFFFF`（白） |

即：曾是「满彩」的类型 → 主体跟随全局、只自留彩色底板；再不能对着灰桌面强行原色岛。

---

## 4. 面板布局

### 4.1 全局「Colour」区 → 两行

现单一 `Colour` 块（一行 swatch + Vivid/Soft 段控）拆成 **主体行 + 底板行**，节奏对齐 `shape-more-crop2.png`：

```
主体 Subject
[原彩][黑白][●黑][●棕][●珊][●青][●琥][◉调色盘]
   └ 选中单色时披露： ‹ 渐层 Tonal │ 纯平 Flat ›
──────────────── hair ────────────────
底板 Plate
[▦随图标][□白][○低饱和×6][◉调色盘]
   └ 底板=随图标 且 主体=原彩时披露： ‹ 鲜明 Vivid │ 柔和 Quiet ›
   └ shape=None 时整行 40% 禁用 + 灰字「选一个形状后可换底色」
```

其下 `Filter / Shortcut mark / Shortcut shape / Beautified types` 各块不变。原 `Vivid/Soft` 段控即旧 `fieldBand`，迁到底板·随图标下改称 `鲜明/柔和`。

### 4.2 调色盘弹窗 ⛔ 废除前景/背景双 tab

两条永显轴行**本身就是 fg/bg 拆分**，双 tab 模态冗余。裁决：
- **废除**旧「前景 tab / 背景 tab」双页弹窗。
- 每个轴的 `◉调色盘` chip 打开一个**单一用途色相拾取器**（主体轮→写 `tint`；底板轮→写 `plateColor`）。少一个模态概念。

### 4.3 类型手风琴展开体 → 同两行（减档）

```
Shape        [跟随全局][None][shapes…][More ⌄]
主体 Subject  [跟随全局][黑白][●色点…][◉调色盘]        ‹渐层│纯平›
底板 Plate    [跟随全局][▦随图标][□白][○低饱和×6]      ‹鲜明│柔和›（无调色盘）
             ↺ Reset to global（仅 custom 时）
```

叠在全局两行上应「看不出是两套控件」——唯一差异是每轴首位 `跟随全局` 锚点 + 主体缺 `原彩`、底板缺自由 `调色盘`（皆一句法则可解释）。旧类型级 `Segmented(Field/Mono/B&W)` 删除。

### 4.4 四预设卡文案

**卡名不改**（满彩/极简白/柔和/本色是 outcome 级、仍准确；轴是控件，两者共存），仅其存储定义迁到 (subject, plate)。⚠️ 见开放问题①：真理源需先对账。推荐定义：

| 卡 中/英 | subject | plateColor | plateBand |
|---|---|---|---|
| 满彩 / Colour field | Original | null（随图标） | Vivid |
| 极简白 / Minimal white | Original | `#FFFFFF` | — |
| 柔和 / Quiet pastel | Original | null（随图标） | Quiet |
| 本色 / True colour | Original | `#FFFFFF`（或 shape=None，待定） | — |

---

## 5. 「随图标」chip glyph 规格（UX 席方向）

- 底座：与所有 `SwatchButton` 同尺寸（~36px 圆角方，radius ~10px），**填充分 4 象限**，读作「一块颜色被派生出来的小板」。
- 4 象限取 **OKLab harmony band 真实 token 色**（非任意）：暖—冷各两色，取 `FIELD_SLOTS.Vivid`（L0.87, C0.09–0.12）附近的珊/琥/青/紫，中低饱和，读作「板色」而非「logo」。
- 象限交界 1px 发丝缝（或中心微留白），传达「派生/合成」，区别于纯色 chip。
- **band-aware（推荐）**：当 `柔和 Quiet` 为当前 band，glyph 4 色切到 `FIELD_SLOTS.Quiet`（L0.91, C0.04–0.07）淡彩，chip 直接预览当前带。
- 选中态：同 2px 珊瑚描边。**必须与 `原彩` 三色点明显区分**（三颗漂浮点 vs 一块四分实心板），不得让两个「彩色 chip」混淆。

---

## 6. 反 slop 校验清单（对齐已 PASS 控件语法）

1. 所有档位都是 `SwatchButton` chip——无 pill 开关、无 bg-wash-chip 大色块、无裸 glyph、无悬浮确认圆勾。
2. 首位锚点律：全局主体首 `原彩`（默认）、全局底板首 `随图标`（默认）；类型两轴首 `跟随全局`。
3. `Segmented` 只用于两个 ≤3 披露件（渐层│纯平、鲜明│柔和），且**上下文披露**，绝不作主色选择。
4. 白/浅档用 v3 加重环+内发丝——`白` chip 与任何浅底色不得读作空心圈。
5. `BwGlyph` 渲染为可见黑\|白分割盘（v3），与任何深色点区分。
6. `随图标` 4 象限板与 `原彩` 三色点明显区分（§5）。
7. 底板行在 `shape=None` 时**灰置不隐藏**（避免消失行=原缺口病根）。
8. 主体/底板/滤镜之间 hair 分割，密度对齐 `shape-more-crop2.png`。
9. 类型两行 = 全局两行「减响档 + 跟随锚点」，叠加看不出两套控件。
10. `调色盘` chip 打开单一用途拾取器（无双 tab 弹窗）。

---

## 附：开放问题（实现前须裁）

1. **预设真理源对账**：`NamedStyle.cs` 仍是旧 `apple/candy/bw/wall`，与全景图四卡 `满彩/极简白/柔和/本色` 对不上——疑似中途迁移/双系统。落 §4.4 前必须确认当前 web 面板的四预设到底由谁定义、现值为何。
2. **`True colour/本色` 语义**：= 原彩 × 白，还是 原彩 × `shape=None`（无板本色）？取决于当前定义。
3. **BW 旧 stored `plateColor`**：迁移保外观取白（旧 inert）；但既然 BW 底板现已可用，产品可选择激活旧存值。二选一需 Owner 拍板（默认取白，不制造迁移瞬间的外观突变）。
4. **类型级主体自定义 tint 饱和是否再设上限**：合约 `tint:string` 自由，本 spec 依合约放行；若 Owner 要「类型也不许高饱和主体」，另加一层 curate（默认不加）。
