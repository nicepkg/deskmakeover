# 两轴颜色重构 · 主体 × 底板（Two-Axis Colour Spec）

- Status: **Finalized** — normative design for **ADR-0018**（含 Amendment 1：faithful/minimal 拆解）；ready for dev-cycle
- Supersedes: `ColorMode` 四值单枚举 + `plateColor` 旁挂字段的耦合模型
- 编码权威：ADR-0018 Amendment 1 定 `plateFallback:'derived'|'white'`（仅 `plateColor===null` 时有意义），本 spec 与其一致
- Grounded in: `bridge/types.ts` (ADR-0016/0017)、`icon-compositor/color.ts`（frozen oracle port）、`lib/type-config.ts`
- 语言约定：散文中文，代码标识符/枚举/组件名 English，chip 标签中英双语

---

## 0. 结论与命名

乱源不是缺前景/背景拆分，是 **`ColorMode` 把「底板行为」伪装成「前景模式」**：`Field/满彩` 根本不是一种前景处理——`color.ts` 证实它是「主体原彩（law 4：subject 像素永不动）× 逐图标派生底板（`themedContrastTone`）」。它以模式姿态把底板绑进前景枚举，才逼出 `plateColor`「Original/Mono 生效、BW 到 v2 才生效」这种按模式分叉的天书，以及「全局 Original 时类型手风琴隐藏底板行 → 白板无处换色」的缺口。

**解法：把模式溶解成两条正交轴。**

| 轴 | 英文 | 管什么 | 默认 |
|---|---|---|---|
| **主体** | Subject | 图标画面本身怎么渲染（原彩/黑白/单色） | 原彩 Original |
| **底板** | Plate | 形状容器里填什么背景（随图标/本色/白/低饱和色） | 随图标 Auto |

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

底板轴有**两个 adaptive 档（`plateColor=null`，靠 `plateFallback` 区分）+ 三个 fixed 档（`plateColor` 有值）**：

| chip | glyph | 标签 中/英 | 编码 | 选中语义 | 披露件 |
|---|---|---|---|---|---|
| 随图标 | **4 象限多色小板**（新 glyph，§5） | 随图标 / Auto | `null` + `plateFallback:'derived'` | 底板色由「主导信号」派生（§2.2）；自带板 `clampPlateLightness` 夹进亮窗、裸图标 `themedContrastTone` 上色。**默认** | ‹鲜明 Vivid \| 柔和 Quiet›（仅主体=原彩，见 §2.3） |
| 本色 | **半拼图块 + 半白板**（新 glyph，§5） | 本色 / Faithful | `null` + `plateFallback:'white'` | 自带板 **1:1 锚定不夹**（Twitter 蓝、Xbox 绿原样保留）；裸图标才回退纯白。= spec 06 `detectFlatPlate` 保真 lane | 无 |
| 白 | `SwatchDot` 白点（**加重环+内发丝**，v3 修法） | 白 / White | `'#FFFFFF'` | 强制纯白覆盖一切（含自带板）。`clampPlateLightness` 白永远是白（law 1/4 near-neutral 豁免） | 无 |
| 低饱和色 | `SwatchDot` 实心低饱和点 × 6 | 底色 / Tint | 该低饱和 hex | 强制该色覆盖一切；`clampPlateLightness` 夹入 `[0.6,0.8]` 亮窗 | 无 |
| 自定义 | `WheelRing` 调色盘轮 | 自定义 / Custom | 任意 hex | 打开单一色相拾取器（**仅全局**，类型级无此档，见 §3.3） | 无 |

- **随图标 vs 本色 是两种「智能」哲学**：随图标 = 归一化派生（调整所有板进光带、给裸图标上彩，出一套整齐的集合）；本色 = 只保真不干预（自带板原样、只给真正空的填白）。二者相邻但语义正交，glyph 必须一眼可分（§5、§6）。

### 1.3 「无板」裁决 ⛔ 不设此档

**底板轴不新增「无 None」档。** 「没有容器/无板」的语义已由 **形状轴 `shape=None`** 表达（无 mask = 无 fill = 无板）；再在底板轴造一个「无」= 两个入口表达同一件事，正是要消灭的重复语义。裁决落地：

- **底板需要容器才有意义**：当 `shape=None` 时，**底板整行 40% 禁用 + 不可点**（不是隐藏——隐藏就是原缺口的病根）。行尾附灰字提示「选一个形状后可换底色 / Pick a shape to set a plate」。
- `shape≠None` 时底板行恢复可用，容器 fill = 底板轴当前值。
- 因此「无板」= 去形状轴选 `None`，全局只有唯一一条路径表达它。
- **`本色` ≠ `无板`（勿混）**：`无板`(shape=None) = 根本没有 DeskMakeover 容器，裸图标浮在壁纸上；`本色` = **有**容器，容器填图标自带板色（或白）。一个「没有板」，一个「板=你自己的」。

---

## 2. 合法组合矩阵

主体 ∈ {原彩, 黑白, 单色(tint,monoStyle)}；底板 ∈ {随图标(band), 本色, 白, 低饱和色}（`shape=None` 时底板 N/A）。

### 2.1 全格子行为

| 主体 ＼ 底板 | 随图标 Auto | 本色 Faithful | 白 White | 低饱和色 Tint |
|---|---|---|---|---|
| **原彩** | = 旧 `满彩/Field`：主体原色，底板派生（同色相、亮度反差），band 控深浅。**出厂默认** | 主体原色 × **自带板 1:1 / 裸图标回退白**（= 原彩保真预设） | 主体原色 × 纯白板（极简/干净） | 主体原色 × 手选低饱和板 |
| **黑白** | 主体灰度 × 中性亮度板 `neutralContrastTone` | 主体灰度 × 自带板 1:1（灰主体压在原色板上，用户显式所选） | 主体灰度 × 纯白板（经典极简黑白） | 主体灰度 × 手选低饱和板 |
| **单色(tint)** | 主体色调映射 × 该 tint 的 ramp 近白端 → §2.2 撞色消解 | 主体色调 × 自带板 1:1（同上，显式） | 主体色调 × 纯白板 | 主体色调 × 手选板（跨色相由用户负责） |

> `本色/Faithful` **主要与原彩配对**（即 `faithful` 预设）；与黑白/单色配对为合法但少见的显式组合（用户主动选「主体去饱和但背景保原样」）。

### 2.2 「随图标」的派生规则（撞色默认消解）

`随图标` 不是「永远取图标色」，而是 **「底板色跟随主导信号」**，主导信号按主体自适应——这条规则让单色永不与底板吵架：

- **主体=原彩** → 主导信号 = 图标自身主导色 → `themedContrastTone(seed, subjectMeanL, band)`（有彩）/ `neutralContrastTone`（画面本身无彩）。
- **主体=黑白** → 画面已无彩 → `neutralContrastTone`（纯亮度中性板）。
- **主体=单色(tint)** → 主导信号 = **所选 tint**，底板 = 该 tint 的 ramp 光端（近白微染）→ 主体与底板**同一色相**，不撞。这正是旧 `Mono + plateColor=null = ramp light end` 的既有语义，无新逻辑。

> 撞色只可能发生在用户**显式**「单色 × 手选一个不同色相的低饱和板」——那是用户主动越界，尊重其选择不代劳消解（explicit > auto）。

### 2.3 派生底板的特例

- **无彩 App 桶图标**（灰 .exe/工具）× 随图标：不吃中性板，改按 `APP_ACCENT_SEEDS` 确定性轮转一个品牌重音色（`lib/type-config.ts`，2026-07-10 Owner 特例）——程序永远是最响的一层；其余桶保持纯中性律。
- `鲜明|柔和`（band）只对**有彩派生板**（主体=原彩 × 随图标）有效：`themedContrastTone` 的 `lightL/darkL` 随 band 变。主体=黑白时板为中性（band inert）；主体=单色时深浅由 `渐层|纯平` 接管。**故 `鲜明|柔和` 披露件仅在「底板=随图标 且 主体=原彩」时出现。** `本色` 不派生，无 band。

### 2.4 随图标 vs 本色（两个 null 档的行为差）

二者 `plateColor` 都为 `null`，靠 `plateFallback` 分岔（`derived` vs `white`）。对两类图标行为不同：

| 图标类型 | 随图标（derived） | 本色（white） |
|---|---|---|
| **自带板**（Twitter 蓝、Xbox 绿、Office 白） | `clampPlateLightness`：保色相、亮度夹进 `[0.6,0.8]` 窗（归一成整齐集合） | **1:1 锚定**：原板原样，不夹亮度 |
| **裸图标**（透明无板） | `themedContrastTone`：按主导色派生有彩板 | **纯白** fallback（容器不空） |

引擎在 `(Original, null)` 格内再按 `plateFallback` 分岔：`derived`→满彩 FIELD 管线；`white`→保真管线（锚定+白）。ADR-0018 engine-mapping 的 `Original×null→FIELD` 一行是 `derived` 支，`white` 支为其保真变体。

---

## 3. 数据模型与迁移映射

### 3.1 新 DTO（`ConfigDto` 颜色相关字段）

```ts
// bump BRIDGE_SCHEMA_VERSION 3 → 4；C# Contracts.cs / IconsContracts.cs 同步
type Subject = 'Original' | 'BlackWhite' | 'Mono'   // was ColorMode, 去掉 'Field'
type PlateBand = 'Vivid' | 'Quiet'                  // was FieldBand（重命名，语义=派生底板深浅带）
type PlateFallback = 'derived' | 'white'            // ADR-0018 Amendment 1：仅 plateColor===null 时有意义

interface ConfigDto {
  // …非颜色字段不变（shape, shortcutShape, distinction, markStyle, size, filter…）
  subject: Subject
  tint: string              // 主体=Mono 时生效
  monoStyle: MonoStyle      // 'Tonal' | 'Flat'，主体=Mono 时披露
  plateColor: string | null // null=adaptive（看 plateFallback）；hex=fixed fill（'#FFFFFF'=白/低饱和/自定义）
  plateFallback: PlateFallback // null 时：'derived'=随图标 / 'white'=本色。plateColor 有值时 moot
  plateBand: PlateBand      // 底板=随图标(null+derived) 且 主体=Original 时生效
}
```

- 底板档位**由 `(plateColor, plateFallback)` 派生**，不加 mode 判别字段：`(null,'derived')`→随图标；`(null,'white')`→本色；`'#FFFFFF'`→白；低饱和集内→底色；其它 hex→自定义。
- `plateColor` 语义**拓宽到全部主体**（旧只 Original/Mono 生效、BW inert）；BW 的「until v2」= 现在。

### 3.2 迁移映射表（旧 → 新，确定、无损、单向、保外观）

| 旧 colorMode | 旧 plateColor | → subject | → plateColor | → plateFallback | → plateBand | 备注 |
|---|---|---|---|---|---|---|
| Field | null | Original | **null** | **derived**（随图标） | = 旧 fieldBand | 满彩原样 |
| Field | `#hex` | Original | `#hex` | moot | — | 手选覆盖派生 |
| Original | null | Original | **null** | **white**（本色） | — | ⚠️旧 Original 空板 = detected-bg/白 = 保真 lane，故 → 本色**（非白！这是 Amendment 1 修正）** |
| Original | `#hex` | Original | `#hex` | moot | — | 显式 fill（如 minimal 的 `#FFFFFF`→白） |
| BlackWhite | null | BlackWhite | **null** | **derived**（随图标=中性板） | — | 黑白×随图标合法化，清 v2 欠账（裁决③） |
| BlackWhite | `#hex` | BlackWhite | **`#hex`** | moot | — | 旧 inert 值**直接激活**（裁决③，黑白×色板净增格子） |
| Mono | null | Mono | **null** | **derived**（随图标=tint ramp 光端） | — | tint/monoStyle 原样带过 |
| Mono | `#hex` | Mono | `#hex` | moot | — | tint/monoStyle 原样带过 |

**关键**：`plateFallback` 派生规则 = **旧 `colorMode=Original` 且 `plateColor=null` → `white`（本色）；其余 `null` → `derived`（随图标）**。这一条让 `field`(满彩) 与 `faithful`(本色) 天然分开——它们从来不是同一坐标，是初稿把 Original+null 错物化成白（Amendment 1 已纠）。`tint/monoStyle` 恒等带过；`fieldBand`→`plateBand`。迁移一次性跑在：当前 config + 四预设 + 全部历史条目 + 全部 `TypeOverrides.patch`。

> **预设真理源（裁决①）**：唯一真理源 = web `bridge/mock-desktop.ts` 的 `BASE_CONFIGS` + `PRESET_TYPE_OVERRIDES`（field/minimal/quiet/faithful 四张，见 §4.4）。C# `NamedStyle.cs`（apple/candy/bw/wall）是 **schema-1 冻结遗产、当前无运行路径消费**，F8 整体重移植——本次不动它，仅记一行 F8 note：`NamedStyle.cs` 待随 host 侧一起迁到两轴。
>
> **active-preset 匹配器**：`mock-desktop.ts` 的 preset 匹配逻辑（现按 `shape/colorMode/typeOverrides/fieldBand/plateColor/tint`）须改为按 `shape/subject/plateColor/plateFallback/plateBand/tint/monoStyle/typeOverrides` 比对。四张现两两可分：`field↔quiet` 只差 band、`field↔faithful` 只差 `plateFallback`（derived vs white）、`minimal` 唯一 `plateColor='#FFFFFF'`。

### 3.3 TypePatch 重定义（类型减档规则）

```ts
interface TypePatch {
  shape?: IconShape
  subject?: 'BlackWhite' | 'Mono'   // ⛔ 原彩排除：类型不许「原色岛」（ADR-0017 D3）
  tint?: string
  monoStyle?: MonoStyle
  plateColor?: string | null        // 有界低彩度：null(随图标/本色) | '#FFFFFF'(白) | 界内 muted 色
                                    //（含桶语义色，如 factory Folder 的 #65470D 棕）。⛔ 无自由高彩 hex
  plateFallback?: PlateFallback     // null 时分 随图标(derived)/本色(white)
  plateBand?: PlateBand             // 继承的主体=原彩 × 随图标 时的深浅
}
```

**减档法则一句话：类型只能「让某类退下去」，不能「让某类跳出来」。**
- 主体去掉 `原彩`（no colour island）；保留 `黑白/单色/单色自定义`（皆属去饱和族，合规）。
- 底板去掉自由 `调色盘`（ADR「bounded plate」= **彩度上限，非固定 6 色**）；用户拾取集 = `随图标/本色/白/有界 muted 色板`；authored 阶梯（factory/存档预设）可携任意**界内**色——如 factory `field` 的 `Folder → plateColor:'#65470D'`（棕，低彩度在界内）。自由高彩 `调色盘` 仅全局有。
- `本色` 在类型级保留（保真=「退让」意图，不属「跳出来」）；主要给「某桶保原背景」的诉求。
- 每轴仍首位 `跟随全局` 锚点（复用 v3 已发机制）；跟随进全局原彩 = 全桌统一 ≠ island，合法。

**旧 TypePatch 迁移（顺带修掉合约内在矛盾）**：旧 `TypePatch.colorMode` 竟含 `Field`（原色主体），与「never Original islands」冲突。新映射自然消解：

| 旧 type colorMode | → type subject | → type plateColor |
|---|---|---|
| Field | **unset（跟随全局）** | null（随图标）+ plateBand=fieldBand |
| Mono | Mono（带 tint/monoStyle） | 旧 plateColor 带过（界内直用，越界高彩才夹） |
| BlackWhite | BlackWhite | 旧 plateColor **直接带过**（裁决③；null→随图标中性板，界内色直用） |

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
[▦随图标][◧本色][□白][○低饱和×6][◉调色盘]
   └ 底板=随图标 且 主体=原彩时披露： ‹ 鲜明 Vivid │ 柔和 Quiet ›
   └ shape=None 时整行 40% 禁用 + 灰字「选一个形状后可换底色」
   （随图标/本色 = 两个 adaptive 档，相邻排；白/低饱和/调色盘 = fixed 档）
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
底板 Plate    [跟随全局][▦随图标][◧本色][□白][○低饱和×6]  ‹鲜明│柔和›（无调色盘）
             ↺ Reset to global（仅 custom 时）
```

叠在全局两行上应「看不出是两套控件」——唯一差异是每轴首位 `跟随全局` 锚点 + 主体缺 `原彩`、底板缺自由 `调色盘`（皆一句法则可解释）。旧类型级 `Segmented(Field/Mono/B&W)` 删除。

### 4.4 四预设迁移（真理源 = `mock-desktop.ts`，卡名不改）

**卡名不改**（满彩/极简白/安静/原彩保真 是 outcome 级、仍准确；轴是控件，两者共存），仅存储定义迁到 (subject, plate)。四张 `BASE_CONFIGS` 的确定迁移：

| 卡 id / 中 | 旧 (colorMode, fieldBand, plateColor) | → 新 (subject, plateColor, plateFallback, plateBand) | 底板档 | 说明 |
|---|---|---|---|---|
| `field` / 满彩（默认） | (Field, Vivid, null) | (Original, null, **derived**, Vivid) | 随图标 | 出厂默认，主体原彩 × 派生彩板·鲜明 |
| `minimal` / 极简白 | (Original, —, `#FFFFFF`) | (Original, **`#FFFFFF`**, moot, —) | 白 | 显式纯白，覆盖一切自带板 |
| `quiet` / 安静 | (Field, Quiet, null) | (Original, null, **derived**, Quiet) | 随图标 | 与 field 只差 band=柔和 |
| `faithful` / 原彩保真 | (Original, —, null) | (Original, null, **white**, —) | **本色** | Amendment 1：自带板 1:1、裸图标回退白 |

> ✅ **minimal 与 faithful 不再重合**：`minimal`=白档（`plateColor='#FFFFFF'`，强制白覆盖自带板）；`faithful`=本色档（`plateColor=null`+`plateFallback='white'`，自带板 1:1 保留）。四张预设四个不同坐标，匹配器精确点亮一张。这修掉了初稿把 `Original+null` 错物化成白导致的塌缩（Amendment 1）。

**typeOverrides 阶梯迁移**（`PRESET_TYPE_OVERRIDES`）——`minimal/quiet/faithful` 均空阶梯（统一容器），只有 `field` 携带出厂 saliency 阶梯（ADR-0017 D4，形状扛类型区分、System 退灰、Field 保逐图标身份）：

| field 阶梯项 | 旧 patch | → 新 patch |
|---|---|---|
| Folder | `{shape:'Folder', plateColor:'#65470D'}` | `{shape:'Folder', plateColor:'#65470D'}`（棕板低彩度在界内，直接带过） |
| File | `{shape:'Tile'}` | `{shape:'Tile'}`（无色覆盖，主体/底板跟随全局） |
| System | `{shape:'Circle', colorMode:'BlackWhite'}` | `{shape:'Circle', subject:'BlackWhite'}`（退灰 = 阶梯要 System「退下去」） |

阶梯迁移无损：`colorMode→subject` 字段级换名，其余原样。

---

## 5. 两个新 glyph 规格（UX 席方向）

### 5.1 「随图标 Auto」= 4 象限多色小板
- 底座：与所有 `SwatchButton` 同尺寸（~36px 圆角方，radius ~10px），**填充分 4 象限**，读作「一块颜色被派生出来的小板」。
- 4 象限取 **OKLab harmony band 真实 token 色**（非任意）：暖—冷各两色，取 `FIELD_SLOTS.Vivid`（L0.87, C0.09–0.12）附近的珊/琥/青/紫，中低饱和，读作「板色」而非「logo」。
- 象限交界 1px 发丝缝（或中心微留白），传达「派生/合成」，区别于纯色 chip。
- **band-aware（推荐）**：当 `柔和 Quiet` 为当前 band，glyph 4 色切到 `FIELD_SLOTS.Quiet`（L0.91, C0.04–0.07）淡彩，chip 直接预览当前带。
- 选中态：同 2px 珊瑚描边。**必须与 `原彩` 三色点明显区分**（三颗漂浮点 vs 一块四分实心板）。

### 5.2 「本色 Faithful」= 半拼图块 + 半白板
- 底座同 `SwatchButton`。**左 ~55% = 一块拼图/契合块**（muted 品牌占位色，读作「保留它自己的板」），**右 ~45% = 平白板**（读作「裸图标回退白」），中间 1px 发丝缝。
- 语义直读：「有自己的 → 用它自己的；没有的 → 白」。拼图块隐喻「1:1 契合本来的样子」。
- **三向可分性硬约束**：本色（拼图+白）≠ 随图标（4 象限全彩、无白、无拼图）≠ 白（整块纯白）≠ 原彩（三颗漂浮点）。四者放一行任意两两不得混淆。
- 选中态：同 2px 珊瑚描边。类型级同一 glyph。

---

## 6. 反 slop 校验清单（对齐已 PASS 控件语法）

1. 所有档位都是 `SwatchButton` chip——无 pill 开关、无 bg-wash-chip 大色块、无裸 glyph、无悬浮确认圆勾。
2. 首位锚点律：全局主体首 `原彩`（默认）、全局底板首 `随图标`（默认）；类型两轴首 `跟随全局`。
3. `Segmented` 只用于两个 ≤3 披露件（渐层│纯平、鲜明│柔和），且**上下文披露**，绝不作主色选择。
4. 白/浅档用 v3 加重环+内发丝——`白` chip 与任何浅底色不得读作空心圈。
5. `BwGlyph` 渲染为可见黑\|白分割盘（v3），与任何深色点区分。
6. **四向可分**：`随图标`(4象限全彩) / `本色`(拼图+白) / `白`(纯白) / `原彩`(三漂浮点) 放一行任意两两不混淆（§5）。尤其 `本色` 不得读成 `白`、不得读成 `随图标`。
7. 底板行在 `shape=None` 时**灰置不隐藏**（避免消失行=原缺口病根）。
8. 主体/底板/滤镜之间 hair 分割，密度对齐 `shape-more-crop2.png`。
9. 类型两行 = 全局两行「减响档 + 跟随锚点」，叠加看不出两套控件。
10. `调色盘` chip 打开单一用途拾取器（无双 tab 弹窗）。

---

## 附：开放问题裁决（team-lead 以工程真值定，2026-07-10）

1. ✅ **预设真理源** — 不是双系统。唯一真理源 = web `mock-desktop.ts` 的 `BASE_CONFIGS` + `PRESET_TYPE_OVERRIDES`（四张，§4.4）。C# `NamedStyle.cs` 是 schema-1 冻结遗产、当前无运行路径消费，F8 整体重移植 → 记一行 F8 note，本次不动。
2. ✅ **`faithful`/原彩保真** — Amendment 1 修正：= 主体原彩 × **本色档**（`plateColor=null`+`plateFallback='white'`：自带板 1:1、裸图标回退白），**非**塌缩到白。底板轴新增 `本色/Faithful` stop；`minimal`=白档、`faithful`=本色档，四预设四坐标不重合（§1.2/§2.4/§3.2/§4.4/§5.2 已落）。
3. ✅ **BW 旧存 `plateColor`** — **激活**。「BW 底板 inert until v2」这笔 v2 欠账正是两轴净增格子（黑白×色板合法化），迁移时旧值直接生效（§3.2/§3.3 已改）。
4. ✅ **类型主体 tint 饱和上限** — v1 **不加**（saliency 阶梯已管住显著性；单色 tint 影响 glyph ramp 而非底板）。**记入 polish 观察项**：验收时若刺眼再收。

实现拆分 → team-lead 出 ADR-0018 + dev-cycle；本席保留**面板改造后的像素级验收**。
