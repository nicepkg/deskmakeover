# 预设套装 v2 · 首席设计师策展（Preset Collection v2）

- Status: ✅ SHIPPED + ACCEPTED — historical curation record（Owner 直接下令 2026-07-10 全权策展；
  七套预设已落地并通过设计师验收，commits `b7dd226`/`f8eb20d`，详见 `docs/journal/2026-07.md`）
- 依据：两轴颜色模型（`docs/product/two-axis-colour-spec.md` / ADR-0018）、纸色文件带（Owner PASS c080912）、暗棕文件夹判决（否）
- 数据契约：`ConfigDto = { shape, subject, tint, monoStyle, plateColor, plateFallback, plateBand, shortcutShape, distinction, markStyle, markColor, size, filter }`；`TypePatch` 同字段可选，桶 = App/Folder/File/System

---

## 0. 三条 Owner 批评 → 三条策展铁律

1. **「几个预设没区别」** → 6 套各占一个**独立材质世界**（满彩／暖文具／液态玻璃／有机卵石／水墨／纯净），不是同一坐标挪一格。跨套至少差 3 个轴（形状族 + 主体/底板哲学 + 角标 + 滤镜）。
2. **「角标全是折纸角 Fold」** → 6 套用 **6 种不同角标**：Halo／Satin／Glass／Shadow／Arc／Ring，**本批次退役 Fold**（仍在轴上可选，只是不做预设脸面）。
3. **「文件夹暗棕好丑」** → `#65470D` 深金板**全线退役**。文件夹新方向二选一：① `随图标` 派生（留自带真色，Owner「多色更好分」已情绪背书）；② 浅**牛皮蜜色** `#EAD6A8`（暖、亮、低彩，是"金"的正确做法，不是脏棕）。**禁止任何预设再用暗棕。**

### 通用裁决 · 派生板豁免品牌禁色
`随图标`(derived) 板色来自图标**自身真色**，是"图标的事实"，**豁免品牌禁蓝紫**（禁令只管 authored/swatch 调过的色，不管派生自真图标的色——否则就是重上色、违法4）。所以蓝 App、蓝文件夹在 `随图标` 下保留其蓝，合法。**authored 固定板**（如蜜色、纸色、灰）必须品牌安全（暖色/中性 C<0.04）。

---

## 1. 满彩 · Full Spectrum 〔默认推荐〕

> **一句卖点**：满城彩色，各归其位——文档不再是灰墙，系统安静退场。
> **设计意图**：秀出派生引擎的灵魂（每个 App 一块专属彩板），暖纸文件带压住文档噪音，系统退灰立秩序——鲜艳但不乱。

```ts
// BASE_CONFIGS.spectrum（= App/全局）
{ shape:'Apple', subject:'Original', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:null, plateFallback:'derived', plateBand:'Vivid',
  shortcutShape:null, distinction:'Mark', markStyle:'Halo', markColor:null,
  size:'Mid', filter:'None' }

// PRESET_TYPE_OVERRIDES.spectrum
Folder: { source:'custom', patch:{ shape:'Folder', plateColor:null, plateFallback:'derived' } } // 留自带色，杜绝暗棕
File:   { source:'custom', patch:{ shape:'Tile',   plateColor:'#E9E2D4' } }                    // 暖纸带（Owner PASS）
System: { source:'custom', patch:{ shape:'Circle', subject:'BlackWhite', plateColor:'#EDEAE4' } } // 退灰
```
- 角标 **Halo**（柔光晕）替代 Fold：高级、不抢主体。滤镜 None。形状 Apple 稳。

---

## 2. 暖纸文具 · Warm Stationery 〔克制款〕

> **一句卖点**：一整张办公桌的牛皮纸与马尼拉信封——安静、暖、想摸。
> **设计意图**：全屏统一暖调（连 App 都走柔和派生），马尼拉蜜色文件夹 + 纸色文件，噪音全部压平，是纸带 PASS 的完整成套化。

```ts
// BASE_CONFIGS.stationery
{ shape:'Apple', subject:'Original', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:null, plateFallback:'derived', plateBand:'Quiet',   // 柔和带 = 淡彩，全屏降噪
  shortcutShape:null, distinction:'Mark', markStyle:'Satin', markColor:null,
  size:'Mid', filter:'None' }

// PRESET_TYPE_OVERRIDES.stationery
Folder: { source:'custom', patch:{ shape:'Folder', plateColor:'#EAD6A8' } }  // 马尼拉蜜色（"金"的正确做法）
File:   { source:'custom', patch:{ shape:'Tile',   plateColor:'#E9E2D4' } }  // 暖纸
System: { source:'custom', patch:{ shape:'Circle', subject:'BlackWhite', plateColor:'#EDEAE4' } }
```
- 角标 **Satin**（缎面微光）：温润，配纸感。Quiet 带让 App 也淡下来 = 整屏一个暖色温。

---

## 3. 澄玻璃 · Liquid Glass 〔大胆款 · iOS26〕

> **一句卖点**：一整屏会呼吸的液态玻璃，光在图标间流动。
> **设计意图**：`Glass` 滤镜把每块派生彩板磨成半透磨砂，`Glass` 角标同材质呼应——最抓眼、最"新 OS"的一套，给想炫的用户。

```ts
// BASE_CONFIGS.glass
{ shape:'Samsung', subject:'Original', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:null, plateFallback:'derived', plateBand:'Vivid',
  shortcutShape:null, distinction:'Mark', markStyle:'Glass', markColor:null,
  size:'Mid', filter:'Glass' }                                   // 液态玻璃滤镜全屏

// PRESET_TYPE_OVERRIDES.glass
Folder: { source:'custom', patch:{ shape:'Samsung', plateColor:null, plateFallback:'derived' } }
File:   { source:'custom', patch:{ shape:'Samsung', plateColor:'#FFFFFF' } }  // 磨砂白文件（冷玻璃里纸太暖，改霜白）
System: { source:'custom', patch:{ shape:'Circle',  subject:'BlackWhite', plateColor:'#ECECEE' } }
```
- 形状 **Samsung**（方中带圆）读作现代玻璃砖，跟默认 Apple 拉开。角标 **Glass**、滤镜 **Glass** 同材质。

---

## 4. 卵石花园 · Pebble Garden 〔大胆款 · 有机形〕

> **一句卖点**：一桌温润的鹅卵石，没有一个尖角。
> **设计意图**：全套有机异形（Pebble/Teardrop），贴纸滤镜给软性模切边，柔和派生板 + 投影角标——最治愈、最"非方块"的一套，专治审美疲劳。

```ts
// BASE_CONFIGS.pebble
{ shape:'Pebble', subject:'Original', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:null, plateFallback:'derived', plateBand:'Quiet',
  shortcutShape:null, distinction:'Mark', markStyle:'Shadow', markColor:null,
  size:'Mid', filter:'Sticker' }                                 // 贴纸模切边

// PRESET_TYPE_OVERRIDES.pebble
Folder: { source:'custom', patch:{ shape:'Folder',   plateColor:'#EAD6A8' } }   // 蜜色，暖
File:   { source:'custom', patch:{ shape:'Teardrop', plateColor:'#E9E2D4' } }   // 水滴 + 纸
System: { source:'custom', patch:{ shape:'Circle',   subject:'BlackWhite', plateColor:'#EAE7E0' } }
```
- 形状：App=**Pebble**、File=**Teardrop**、Folder 留 Folder（可辨识）、System=Circle（退）。角标 **Shadow**、滤镜 **Sticker** 给软立体。

---

## 5. 水墨宣 · Ink Wash 〔黑白款 · 保 BlackWhite 语义〕

> **一句卖点**：一屏黑白见筋骨，宣纸上的墨相。
> **设计意图**：全主体灰度（`BlackWhite`）压在暖宣纸上，圆形 + 细弧角标 = 极简高级；是黑白语义的正式归宿，也是最"设计师"的一套。

```ts
// BASE_CONFIGS.ink
{ shape:'Circle', subject:'BlackWhite', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:'#F4F1EA', plateFallback:'white', plateBand:'Vivid',   // 暖宣纸白
  shortcutShape:null, distinction:'Mark', markStyle:'Arc', markColor:null,
  size:'Mid', filter:'None' }

// PRESET_TYPE_OVERRIDES.ink
Folder: { source:'custom', patch:{ shape:'Bookmark', plateColor:'#EDE8DC' } }  // 书签形 + 更暖纸
File:   { source:'custom', patch:{ shape:'Tile',     plateColor:'#F4F1EA' } }
System: { source:'custom', patch:{ shape:'Circle',   plateColor:'#EEEBE4' } }  // 已灰，板更淡
```
- 全部 `subject:'BlackWhite'`（System 继承即可）。角标 **Arc**（细弧，像一笔）。形状 Circle/Bookmark，禅意。

---

## 6. 纯粹一对 · Pure Pair 〔克制款 · 保 极简白 + 本色 语义〕

同一格里两枚"极简主义"孪生，都用 **Ring**（细描边）角标、`filter:'None'`、`shape:'Apple'`，只差底板哲学——满足"极简白/本色语义保留"：

### 6a. 极简白 · Clean White
> **卖点**：白纸一张，只统一形状。全白板 + 原色主体，最干净。
```ts
{ shape:'Apple', subject:'Original', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:'#FFFFFF', plateFallback:'white', plateBand:'Vivid',
  shortcutShape:null, distinction:'Mark', markStyle:'Ring', markColor:null,
  size:'Mid', filter:'None' }
// 阶梯：Folder{shape:'Folder',plateColor:'#FFFFFF'} · File{shape:'Tile',plateColor:'#FFFFFF'} · System{shape:'Circle',subject:'BlackWhite',plateColor:'#F2F2F2'}
```

### 6b. 本色 · As-Cast
> **卖点**：原样保真，只理齐轮廓——每个 App 留自己的真牌面板，只统一形状。
```ts
{ shape:'Apple', subject:'Original', tint:'#FF6F5E', monoStyle:'Tonal',
  plateColor:null, plateFallback:'white', plateBand:'Vivid',   // 本色档：自带板 1:1、裸图标回退白
  shortcutShape:null, distinction:'Mark', markStyle:'Ring', markColor:null,
  size:'Mid', filter:'None' }
// 阶梯：Folder{shape:'Folder',plateColor:null,plateFallback:'white'} · File{shape:'Tile',plateColor:'#E9E2D4'} · System{shape:'Circle',subject:'BlackWhite',plateColor:'#EDEAE4'}
```
- 6a 强制白覆盖一切；6b 保留每个图标自带板（Twitter 蓝、Xbox 绿 1:1）。二者是两轴 `白` 档 vs `本色` 档的预设化，语义完整保留。

---

## 7. 角标 / 滤镜 / 形状 分布（证明"有区别"）

| 套 | 角标 markStyle | 滤镜 filter | 招牌形状 | 主体×底板哲学 | 胆量 |
|---|---|---|---|---|---|
| 满彩 Spectrum | **Halo** | None | Apple | 原彩×随图标 Vivid | 中 · 默认 |
| 暖纸文具 Stationery | **Satin** | None | Apple + 蜜/纸 | 原彩×随图标 Quiet | 克制 |
| 澄玻璃 Liquid Glass | **Glass** | **Glass** | Samsung | 原彩×随图标 + 磨砂 | 大胆 |
| 卵石花园 Pebble | **Shadow** | **Sticker** | Pebble/Teardrop | 原彩×随图标 Quiet | 大胆 |
| 水墨宣 Ink Wash | **Arc** | None | Circle/Bookmark | **黑白**×暖宣纸 | 中 · 设计师 |
| 纯粹一对 Pure | **Ring** | None | Apple | 原彩×白 / 原彩×本色 | 克制 |

- **6 套 6 种角标，零 Fold**（Fold 退役本批次，仍在轴上）。滤镜用到 Glass/Sticker 各一。形状横跨 Apple/Samsung/Pebble/Teardrop/Circle/Bookmark/Folder/Tile 8 种。文件夹**无一暗棕**（随图标 或 蜜色 或 白）。

---

## 8. 出厂默认推荐

**默认 = 满彩 Full Spectrum。** 理由：① 首屏必须秀产品灵魂（派生彩板 = 别家没有的核心价值）；② 已吸收 Owner 刚 PASS 的暖纸文件带，文档不再炭墙；③ 系统退灰立秩序，鲜艳但不乱；④ Halo 角标高级不土。
**次选 = 暖纸文具**：若用户群偏文档/办公、想要"安静高级"，一键切它即可。
（两者 App 都走 `随图标`，切换时最能展示"同一引擎、不同性格"。）

---

## 9. 实现备注

- 6 套（6b 含孪生 = 7 个 BASE 条目）直接进 `mock-desktop.ts` 的 `BASE_CONFIGS` + `PRESET_TYPE_OVERRIDES`；`plateColor` 有值时 `plateFallback` moot 可省。
- authored 固定板色板全部品牌安全：纸 `#E9E2D4`、蜜 `#EAD6A8`、暖灰 `#EDEAE4/#EAE7E0`、宣纸 `#F4F1EA/#EDE8DC`、纯白/淡灰。**无一落禁蓝紫弧、无一暗棕。**
- 角标 markColor 全 null（自动取色）；如需与主体拉开可后调。
- 预设卡 mini 预览须用**真实带自带板的图标**（Twitter/Xbox/Office）才能看出 满彩 vs 本色 vs 极简白 的底板差，否则三者预览会撞脸（沿用海报"真素材验收"教训）。
