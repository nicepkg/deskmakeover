# 预设套装 v3 · Owner 亲手策展（Preset Collection v3）

- Status: ✅ SHIPPED — normative lineup record（Owner 在真实画布上手调 9 套并导出 `.dmpreset`，
  2026-07-16；配方数值**一字未改**照搬导出包，命名由 Commander 提案、Owner 批准）
- Supersedes: `preset-collection-v2.md`（七套 v2 预设整批退役：spectrum/stationery/glass/
  pebble/ink/white/ascast——只剩 git 历史；系统默认卡不属于预设集，不受影响）
- 数据真理源：`src/lib/icons-assemble.ts` `BASE_CONFIGS` + `PRESET_TYPE_OVERRIDES`
  （key 顺序 = 卡片顺序）；名字/文案 `src/lib/i18n/{zh-hans,en}.ts` `Preset_<id>[_Desc]`
- 回归锁：`tests/preset-factory-lineup.test.ts`（九套清单 + squircle 默认 + 每套过 ONE validator）

## 策展原则（v3）

1. **Owner 的手调值就是法律** —— 每套配方来自 Owner 在 Mac 画布上的实际调参导出，
   任何"顺手优化"（改个色、换个 mark）都是违规；改动必须重新走 Owner 导出流程。
2. **出厂默认 = 方圆 squircle** —— Owner 的第一套、也是最"默认脸"的一套
   （`DEFAULT_PRESET_ID` 随之从 spectrum 迁移）。
3. **蓝色豁免**：蓝图的单色墨 `#0F4F93`、釉光的冷调板 `#DDE6F2` 是**用户可选的桌面内容**，
   不是 app chrome accent——`tests/banned-colors.test.ts` 对 `lib/icons-assemble.ts`
   有已评审的数据豁免（doctrine 同 调色盘 光谱豁免）。

## 九套清单（卡片顺序）

| id | 中文 | English | 一句话 | 配方骨架 |
|----|------|---------|--------|----------|
| squircle | 方圆 | Squircle | 一枚方圆，各有其形 | Apple 方圆 + Ring 细环；Folder→Folder 形，File→File 形 |
| porthole | 圆窗 | Porthole | 程序是圆窗，文件各安其位 | Circle 圆 + Ring；Folder→Folder，File→Apple |
| pixel | 像素纪元 | Pixel Era | 回到八比特的下午 | Apple + Pixel 滤镜 + Comet；类型板 `#E7E7E5` |
| creek | 溪石 | Creekstone | 溪水磨圆的石头 | Pebble 卵石 + Shadow；Folder→Samsung(derived 板)，File→Samsung |
| scrapbook | 拼贴手帐 | Scrapbook | 一页随手拼贴的手帐 | Samsung + Sticker 滤镜 + Fold 折角；File→Pebble `#E7E7E5` |
| gleam | 浮光 | Gleam | 原样的图标，掠过一层光 | 无形状 + Glass 滤镜 + Comet；Folder/File 有形 |
| diecut | 随形贴 | Die-Cut | 沿轮廓裁开的贴纸包 | 无形状 + Sticker 滤镜 + Comet；无类型覆写 |
| blueprint | 蓝图 | Blueprint | 一整套工程蓝图 | Samsung + Mono `#0F4F93` + Shadow；无类型覆写 |
| glaze | 釉光 | Glaze | 上过釉的冷瓷面 | Apple + Gloss 滤镜 + 白 Comet；类型板 `#DDE6F2`（Folder derived） |

共性：`plateBand: Vivid` · `plateFallback: white`（除类型覆写另说）· `monoStyle: Tonal` ·
`distinction: Mark` · `shortcutShape: null`（统一快捷方式形状始终是用户 opt-in，预设不带）。
