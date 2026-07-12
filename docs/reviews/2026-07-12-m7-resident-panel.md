# Panel record: M7 常驻自动 format — appearance model, reset semantics, trust model

**Date:** 2026-07-12 · **Seats:** chief-PM + chief-UX/UI + chief-架构师 (isolated first round,
cross-examined second round, converged by the lead)
**Question (owner):** M7 常驻自动 format 该怎么设计——`version` 到底存的是什么、切外观/重置该怎么
做、新图标要不要静默改、常驻什么时候能开。上一轮设计留下的困惑：「上次 style」有没有偏、增量 apply
要不要新建 version、version 要不要记图标清单。

## Round 1 — independent positions

### 架构师 seat

1. **Reframe first, storage second.** `version` 一个词过去指了三样无关的东西——必须拆成三个正交存储
   才能解开"乱": ① 活跃 ledger(唯一可逆真相源，一图标一行，持有 `original_anchor` +
   `last_applied_fingerprint`) ② saved-style(单例，"现在按什么样") ③ look-history(10 个可来回切的
   外观菜谱，**不存图标清单**)。图标清单永远查 `ledger.all()`，存了必 stale、必撞切换算法。
2. **性能认知纠正(4.1)。** 不存在"系统拖缓才切 native"分支——常驻是无 WebView 的纯后台 Rust 进程
   (spec07 §1)，WASM 没有 JS 宿主可跑，从第一行代码起只能调 native。这不是可调优的设计选项，把它当
   一个决策点讨论是**范畴错误**。真正的性能杠杆是编译期 `--features fast`(cold 7.6-18.4ms/icon →
   warm 0.67-2.5ms)。NATIVE_ARROW 线程安全前置已修(`marks/mod.rs` RwLock 进程级)，
   `render_icons_par` 现在对 M7 是安全的。
3. **UAC 已经不是设计问题(4.2)。** ADR-0020 §4 + ADR-0021 §5 已经判死"后台进程弹 UAC"——Windows
   安全模型硬约束，`requireAdministrator` 走 `runas`，OS 不允许无人值守静默提权。已核实现有
   `WindowsIconApplier` 全是用户级写(`.lnk`/AppxShortcut COM、RegularFile 文件包装、
   Url/Folder 文件系统、RecycleBin **HKCU only**)，自动 format 触发面无一需要提权。唯一需要
   `dm-elevated` 的是"全局透明箭头覆盖"，与自动 format 正交。剩下的工程工作是硬性 kind 过滤
   (遇需提权/public-desktop 项直接进"待处理特权项"队列，绝不调写路径) + 一道带副作用的红线测试，
   不是重新设计权限模型。
4. **能不能开工的诚实判断。** M7 现在建不了。整条链悬在 M6-WIRE Wave B(② saved-style 存储扩展、
   真实 `FsAssetStore`、`TxnDriver::apply` 生产接线、commit→ledger 间隙修复、③ `LookHistoryStore`、
   source fingerprint、`watcher.rs` 从 SKELETON 变真实)——spec07 / M6-WIRE plan §7 已经把 M7 排在
   Wave B 之后。

### chief-PM seat

1. **纯静默是错误的 v1 默认。** 对未发布/无签名/零信誉 app，后台静默改写桌面文件是"这软件偷改你
   东西"投诉贴的经典触发场景。Owner 的"applied 且没改 = 想要"这个 revealed-preference 逻辑本身没
   错，但要成立，前提是用户**能感知到**这次自动改动发生过。
2. **"上次 style"有偏的前提要收紧。** 只有严格限定"上次一次**全局** Apply"(而非单图标编辑，也非
   `setLook` 拖拽草稿态)时，"用户最近一次全局操作代表当下意图"才成立；单图标编辑不该污染这个信号。
3. **意图新鲜度检查(Q1)。** 距上次全局 Apply 超过 60 天，或用户已经手动把若干图标单独改回系统默认
   (逐步退出该风格的行为信号)，都说明"意图可能已经过期"——命中任一时该功能应从静默套用降级为
   "批量提议"，而不是一路悄悄执行到底。
4. **常驻开启前置。** 赞同架构师——开关必须要求"先成功全局 Apply 过一次"才能点亮，顺带让"saved-style
   一定存在"成为不变量，消灭一整类空指针边缘。

### chief-UX/UI seat

1. **`version` 的心智模型是全场最大的坑。** 用户从不期待"换外观"能找回已删除的图标——"图标集变了"
   的困惑只在**快照**模型里存在，在**外观**模型里根本不会产生。缩略图必须是风格样张(3-4 个示例图标
   用该外观渲染 + 壁纸色卡)，**绝不能是历史桌面截图**——截图会把已经被架构拆掉的快照心智重新招回
   用户脑子里。UI 术语铁律：禁用「版本/快照/回退/时光机/恢复到某刻」，一律用「外观/外观方案/应用/
   恢复系统原始外观」(内部代码可继续叫 version)。
2. **"纯静默"会摧毁 revealed-preference 逻辑赖以成立的前提。** 同意 PM 的信任成本论点，但补一刀：
   Owner"没改 = 想要"这个判断，前提是用户能感知到发生了什么；"没改"很可能只是"没注意到"。这不是
   礼貌问题，是这套信任逻辑能不能自洽的**结构性问题**——纯静默会直接把前提条件打掉。
3. **重置语义有双向歧义，是全场最容易漏掉的坑。**「恢复原始外观」如果字面执行：①对用户自己后来
   手动改过的图标是销毁数据；②如果只清空 saved-style、不同时关自动整理，重置完新图标马上又被自动
   套回旧风格——用户会以为"重置根本没生效"。这两个方向必须在同一个操作里同时堵死，缺一不可。

## Round 2 — cross-examination + convergence

**UX → 架构师：** 重置到底该不该"如从未修改"字面执行？

**架构师最强反驳(6.1，本轮最重的一次翻案)：** 字面 revert 对"用户后来自己改过"的图标 =
**销毁用户数据**，直接违反产品头号承诺——README "Restore must stay visible and reliable" +
spec07 §5 "user/installer wins, no silent overwrite" + `driver.rs:239` 现有的 CAS 语义。重置
必须只还原"仍是我们上次留下的样子"的图标(指纹匹配)；用户已经改过的**跳过 + 在报告里透明告知**
"已跳过 N 项(你自己改过)"。"如从未修改"对最常见情况(图标还是我们留的样子)一字不差成立；对冲突项
= 跳过 + 告知，而不是覆盖。

**PM 补充：** 这也回应了我 round 1 的信任风险论点——如果重置会静默吃掉用户自己的修改，那和"静默
auto-format"是同一类信任风险，不能留这个洞。

**UX 追加(重置双向歧义，round 1 遗留)：** 光"跳过用户改过的"还不够，重置还必须**同时**做三件事
才算真正的"如从未修改"：清空 saved-style + 关闭自动整理开关(否则重置完新图标又被套旧风格) +
还原机器级箭头覆盖(ADR-0021，一次 UAC)。三者是一个原子操作，不是三个可选项。

**架构师验证边缘情况(呼应 UX 的字面执行担忧)：** ①删后重建同名图标 → 同 ItemId 但内容不同 →
指纹≠`last_applied` → 正确地命中冲突分支不还原(否则用旧 `.lnk` 字节覆盖用户的新快捷方式，才是
真正的数据丢失)。②"无 original 的条目"结构上不可能进 ledger(`driver.rs:257 has_material()==false`
在 `prepare` 就跳过)。这两个边缘现有语义已经是对的，不需要新代码。

**PM → 全体：** 那"信任模型"要不要索性做成纯静默的对立面——每次都弹确认？

**UX 反驳(收敛到"批量提议 + 超时")：** 零点击价值不能丢——用户开了自动整理就是不想天天点确认。
方案是**批量提议 + 一键确认 + 超时(如 2h)自动生效**，反馈载体用 **Windows 原生 Toast**
(不自绘 webview toast)——白送无障碍(Narrator 可读)、勿扰模式遵从、操作中心留痕、内联撤销按钮。
分级：环境层(托盘脉冲 2s + 被改图标桌面淡入高亮)永远在；信任建立期(前 3 次)原生 Toast + 撤销；
3 次未撤销后降级为仅环境层；异常永远弹不降级。

**架构师确认可行性：** 原生 Toast(tauri-plugin-notification/WinRT)+ 托盘脉冲都是纯前台/托盘操作，
不涉及提权，和 round 1 #3 的 UAC 判死结论不冲突。

**PM → 架构师：** 常驻到底什么时候能开工建？

**架构师(收尾，呼应 round 1 #4)：** 建不了，不是态度问题是事实问题——② saved-style 存储、真实
`FsAssetStore`、`TxnDriver::apply` 生产接线、commit→ledger 间隙、③ `LookHistoryStore`、source
fingerprint、`watcher.rs` 都是 0% 或 fake-only。这些是 Wave B 的范畴，M7 只能排在 Wave B 之后。

**收敛(三席一致)：** M7 的三存储模型 + 状态机耦合表 + 切换算法(投影到当前扫描，见 spec07)+
重置 trust-first + 信任模型(批量提议 + 超时 + 原生 Toast)+ 常驻开启前置(saved-style 非空)
已经没有内部分歧，可以进 ADR。M7 的**建造顺序**上锁在 Wave B 之后，这不是设计分歧，是依赖事实。

## Owner dispositions — RESOLVED 2026-07-12

| # | 决策 | 裁决 | 采纳理由 |
|---|------|------|---------|
| 1 | **M7 version 模型 = 外观预设三存储**(非快照)；切外观 = 投影到当前扫描；version 不存图标清单 | **APPROVED** | UX 的 reframe 从根上消解了"图标集变了"的困惑(该困惑只在快照模型里存在)；三存储正交划清"怎么还原/现在按什么样/以前按过哪些样"的职责边界，增量 auto-format 只写①一行、不碰②③，天然不新建 version，零特判解决"选系统默认 → saved-style=null → M7 停手"这个例外场景。 |
| 2 | **重置 = trust-first**(用户改过的跳过 + 告知，非字面 clobber)；重置耦合(清源 + 关自动整理 + 还原箭头覆盖一次 UAC) | **APPROVED** | 架构师 round-2 最强反驳成立：字面 revert 销毁用户数据，违反产品头号承诺(README + spec07 §5 + `driver.rs:239` CAS 语义)；UX 的双向歧义补充堵死"重置完新图标又被套旧风格"的洞。现有 CAS 已结构性保证不覆盖用户改动，新增的只是"跳过时报告 + 三件套同步动作"。 |
| 3 | **自动 format 信任模型 = 批量提议 + 超时自动生效**(非纯静默)；原生 Toast 反馈；意图新鲜度检查 | **APPROVED** | PM+UX 一致，且是 Owner 自己的逻辑要求——"没改 = 想要"的 revealed-preference 前提是用户必须先感知到改动，纯静默会摧毁这个前提；批量提议 + 超时在零点击价值和信任信号之间取得平衡；原生 Toast 白送无障碍/勿扰遵从/操作中心留痕；意图新鲜度(60 天 / 部分回退信号)防止执行已经过期的意图。 |
| 4 | **常驻开启前置 = 先成功全局 Apply 过一次**(saved-style 非空) | **APPROVED** | 架构师 + PM 一致：让"上次 style 一定存在"成为不变量，消灭一整类空指针边缘；同时天然防止"从未表达过风格意图"时被常驻抢跑。 |

## Implementation status

设计已收敛，落地为 ADR `docs/decisions/0022-m7-appearance-model-and-consent.md` + spec
`docs/specs/07-background-resident.md`(整合 §1-§22 全部细节：三存储/状态机耦合表/切换算法/排除集/
信任模型/意图新鲜度/常驻前置/首次同意流/托盘状态机/可逆触点/重置确认文案/性能/活跃检测/生命周期/
UAC/依赖清单/托盘图标资产/10-cap 处理/指标)+ 构建计划 `docs/plans/2026-07-12-m7-resident.md`。

**构建门禁未开** —— 见 `docs/STATE.md` 的 Wave B → M7 依赖关系；M7 排在 M6-WIRE Wave B(saved-style
存储/`FsAssetStore`/`TxnDriver::apply` 接线/`LookHistoryStore`/source fingerprint/`watcher.rs`)
之后。本记录不重复 spec07 已经写入的实现细节，只留裁决与反驳链的可追溯来源。
