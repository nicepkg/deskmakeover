# WebView2 consumer-machine pitfalls — research report (2026-07-08)

Compiled by a dedicated research agent for the v3 redesign hardening phase (plan F6).
Scope: WPF host + Evergreen WebView2 + local React SPA + frameless custom chrome +
SharedBuffer pixel streams + non-technical users on messy machines. Every claim
carries a source URL; the two high-stakes items (font CORS, occlusion throttling)
were verified across ≥2 sources.

> **DeskMakeover reconciliation notes (commander):**
> 1. Fonts: this report suggests `font-display: swap` (web-general advice). Our
>    typography decision (ADR-0013 D2) uses `font-display: block` + a
>    `document.fonts.ready` first-render gate — correct for a LOCAL desktop app
>    (fonts come off disk in milliseconds; zero FOUT beats fast-fallback). Keep D2.
> 2. Virtual-host mapping / kiosk settings / DPI / ProcessFailed items must be
>    AUDITED against the existing C# host (`Host/WebShellWindow.cs`, spec 05) on the
>    Windows side — some may already be in place; verify, don't assume, in F6.

---

## TL;DR 高危 Top 10（按「杂机器翻车概率 × 伤害」排序）

1. **Evergreen Runtime 缺失/被企业策略拦装 → 整个 app 白屏起不来。** 启动前必须 `GetAvailableCoreWebView2BrowserVersionString` 探测，缺了引导装 Bootstrapper 或退回 Fixed Version。非技术用户 + 老 Win10 极常见。
2. **杀软/EDR/DLP 掐掉 `msedgewebview2.exe` 子进程 → 白屏、卡死、初始化 `E_FAIL`。** 消费级机器上「激进杀软」是头号杀手，只能靠签名 + 引导 + 崩溃兜底 UI。
3. **125%/150% 分数 DPI → 文字发虚、1px 描边/发丝线糊成一团。** 必须 PerMonitorV2 + 跟踪 `RasterizationScale`。
4. **首帧白闪（white flash）。** `DefaultBackgroundColor` + `WEBVIEW2_DEFAULT_BACKGROUND_COLOR` 环境变量双保险。
5. **GPU 崩溃/被禁/老显卡/RDP → 软件渲染，`backdrop-filter`/blur/canvas/动画卡成 PPT 甚至花屏。** 重度依赖 blur 的 UI 要有降级。
6. **`ProcessFailed` 不处理 → renderer/browser 进程崩了以后停在错误页。** 必须挂事件自动 Reload/重建。
7. **中文 IME 候选框错位**（无边框 / 多屏 / DPI 变化 / RDP 下尤甚）。
8. **浏览器快捷键/右键菜单/F12/拖文件进窗口没锁死** → kiosk 感全无，拖文件可把整个 SPA 导航走。
9. **用户名含中文/非 ASCII、UDF 落在 OneDrive 同步目录/网络盘 → 初始化失败或数据损坏。**
10. **无边框窗口拖拽/缩放/最大化后最小化多屏白屏。** `app-region: drag` 需较新 Runtime（老 Runtime 静默失效）；`WM_NCHITTEST` 不认触摸；最大化多屏最小化有官方 bug (#2549)。

## 分类详单

### 1. CSS / 渲染差异

**坑：分数 DPI（125%/150%/175%）文字发虚、发丝线糊。**
修复：① `PerMonitorV2`（Win32 `SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`；WPF `app.manifest` `<dpiAwareness>PerMonitorV2</dpiAwareness>`）。② `CoreWebView2Controller.RasterizationScale` = 显示器 scale × 文字缩放；默认 `ShouldDetectMonitorScaleChanges=true` 自动跟随，手动管窗口则设 false 并监听 `RasterizationScaleChanged`。③ 发丝线在 1.25/1.5 落半像素 → 用 `box-shadow inset`/`transform` 对齐设备像素，别裸 `border:1px`。
来源：github.com/MicrosoftEdge/WebView2Feedback/issues/571 · github.com/tauri-apps/tauri/issues/1074 · learn.microsoft.com corewebview2controller.rasterizationscale · WebView2Feedback specs/RasterizationScale.md

**坑：文字 greyscale AA 而非 ClearType → 比原生略糊略细。** 合成层/非不透明背景下禁子像素 AA。
修复：无法强制 ClearType。缓解：文字背后垫不透明背景色；PerMonitorV2 + 尽量整数缩放；选 hinting 好的字体，正文字重略加。
来源：github.com/microsoft/vscode/issues/24957 · WebView2Feedback/issues/5205

**坑：滚动条默认样式不搭；部分 Runtime `::-webkit-scrollbar` 被 overlay 覆盖。**
修复：`CoreWebView2EnvironmentOptions.ScrollBarStyle = FluentOverlay`，或自绘 CSS + `scrollbar-gutter: stable`（在最低目标 Runtime 实测）。
来源：learn.microsoft.com corewebview2scrollbarstyle · WebView2Feedback/issues/4131

**坑：无边框拖拽 — WebView2 不认 Electron 的 `-webkit-app-region`。**
修复：`CoreWebView2Settings.IsNonClientRegionSupportEnabled = true`（ICoreWebView2Settings9，默认 false，下次导航生效）→ CSS `app-region: drag/no-drag` 生效。老 Runtime 属性缺失静默失效 → 保留 `WM_NCHITTEST` 子类化兜底；该方案只认鼠标不认触摸。
来源：learn.microsoft.com icorewebview2settings9 · WebView2Feedback/issues/200 · /issues/2243

**坑：GPU 关闭/老显卡/RDP/VM → 软件渲染，blur/canvas/动画极卡甚至花屏。**
修复：别关 GPU（`--disable-gpu` 不受支持且全局变慢）。做能力降级：检测软件渲染 → blur→半透明纯色、停非必要动画、尊重 `prefers-reduced-motion`。花屏引导更新驱动/Runtime。
来源：learn.microsoft.com webview2/concepts/performance · WebView2Feedback/issues/725 · /issues/2421

**坑：Ctrl+滚轮 / Ctrl± / 捏合意外缩放整个 SPA。**
修复：`IsZoomControlEnabled = false` 且 `IsPinchZoomEnabled = false`（二者独立）。
来源：learn.microsoft.com iszoomcontrolenabled · WebView2Feedback/issues/459

### 2. @font-face / bundled 本地字体

**坑：字体从 `file://` 加载 → CORS 静默失败掉回 fallback。**
修复：不要用 `file://`。`SetVirtualHostNameToFolderMapping("appassets.local", <dist>, DenyCors)` 映射整站 → 导航 `https://appassets.local/index.html`。字体与 HTML 同一虚拟 host = 同源，DenyCors 也正常加载；虚拟 host 顺带给 secure context。
来源：learn.microsoft.com webview2/concepts/working-with-local-content

**坑：字体跨源引用 → CORS 拦截静默不显示。** 字体是强 CORS 资源，浏览器通用行为。
修复：全部资源塞同一虚拟 host（首选）；确需跨源用 `Allow` + `crossorigin`。
来源：working-with-local-content · hirehop.com cross-domain-fonts-cors

**坑：bundled 字体首帧 FOUT/FOIT。**
修复（web 通用建议）：`font-display: swap` + 度量接近的 fallback + `<link rel=preload as=font crossorigin>`。**DeskMakeover 取 block + fonts.ready 门（见顶部 reconciliation）。**
来源：debugbear.com web-font-layout-shift

**坑：`src: local("字体名")` 在杂机器上行为不一致。**
修复：bundled 场景直接 `url()` 走打包文件；VF 用 woff2 + `font-weight: 100 900` 区间声明。
来源：runebook.dev @font-face/src

### 3. Runtime / 环境地雷

**坑：Evergreen Runtime 未装/损坏 → 初始化失败白屏。**
修复：启动探测 `GetAvailableCoreWebView2BrowserVersionString(null)`；缺→引导装 ~2MB Evergreen Bootstrapper（按架构自动拉）或 Fixed Version（封闭机器）。友好引导 UI，绝不裸崩。
来源：webview2/concepts/distribution · evergreen-vs-fixed-version

**坑：杀软/EDR/WDAC/AppLocker 破坏多进程架构 → 白屏/卡死/E_FAIL。**
修复：① host exe + `msedgewebview2.exe` 数字签名；② 文档给 IT publisher-rule allowlist 指引；③ 保留 Runtime 目录默认 ACL/LowIL；④ 允许 Renderer/GPU/Network/Crashpad 子进程、不注入 DLL；⑤ `ProcessFailed` + init try/catch → 「被安全软件拦截」兜底页。排查：Task Manager 子进程 / Event Viewer / Reliability Monitor。
来源：webview2/concepts/measures · /concepts/enterprise

**坑：UDF 落 OneDrive/网络盘/只读区，或非 ASCII 用户名 → Access Denied、数据损坏。**
修复：UDF 显式设 `%LOCALAPPDATA%\<App>\WebView2`（Local 非 Roaming）；宽字符 API 处理路径。
来源：webview2/concepts/user-data-folder · WebView2Feedback/issues/2594 · /issues/1410

**坑：进程崩溃不兜底。**
修复：挂 `ProcessFailed` 按 `ProcessFailedKind` 分流：GPU/Utility → 自恢复记日志；RenderProcessExited → `Reload()`；BrowserProcessExited → 重建控件（与 `BrowserProcessExited` 事件协调防 race）；RenderProcessUnresponsive（~15s 重复）→ 阈值后 Reload。dump 在 `UDF\EBWebView\Crashpad\reports\`。
来源：webview2/concepts/process-related-events · WebView2Feedback/issues/4452

### 4. 输入 & IME

**坑：中文 IME 候选框错位（无边框/多屏/DPI 变化/RDP）。**
修复：桌面场景用默认 windowed hosting（避开 composition hosting 的定位问题）；DPI 变化后刷新窗口位置；RDP 已知官方 bug #5570。中文输入是核心用户群 → 真机逐输入框实测。
来源：WebView2Feedback/issues/5570 · microsoft-ui-xaml/issues/9867 · WebView2Feedback/issues/869

**坑：浏览器快捷键暴露「这是浏览器」。**
修复：`AreBrowserAcceleratorKeysEnabled = false`（关 Ctrl+F/P/D/G/F3/F5/F7…；不关 Ctrl+C/V/X/A/Z 和 Home/End/PgUp/PgDn——通常正是想要的）。细粒度走 `AcceleratorKeyPressed`。
来源：learn.microsoft.com arebrowseracceleratorkeysenabled

**坑：右键菜单/F12/状态栏/自动填充/存密码弹窗露馅。**
修复：`AreDefaultContextMenusEnabled=false` · `AreDevToolsEnabled=false`(发布) · `IsStatusBarEnabled=false` · `IsGeneralAutofillEnabled=false` · `IsPasswordAutosaveEnabled=false`。
来源：learn.microsoft.com corewebview2settings

**坑：拖文件进窗口 → SPA 被导航走。**
修复：`CoreWebView2Controller.AllowExternalDrop = false`（默认 true）；需要拖入则 JS `dragover/drop preventDefault` 自行处理。
来源：WebView2Feedback/issues/4859 · specs/APIReview_AllowExternalDrop.md

**坑：滑动手势触发后退/前进。**
修复：`IsSwipeNavigationEnabled = false`；已知 bug #4502 部分版本仍穿透 → CSS `overscroll-behavior: none` + JS 拦截兜底。
来源：learn.microsoft.com isswipenavigationenabled · WebView2Feedback/issues/4502

### 5. 无边框窗口专项

**坑：首帧白闪。**
修复：`DefaultBackgroundColor` = app 底色（只支持 alpha 0 或 255）+ 环境变量 `WEBVIEW2_DEFAULT_BACKGROUND_COLOR`（如 `FFF5F5F3`）进程级兜底 + WPF host 窗口同色。
来源：west-wind.com WebView2-Flashing · specs/BackgroundColor.md · WebView2Feedback/issues/3384

**坑：WPF airspace — windowed 托管的 webview 永远盖在 WPF 内容上。**
修复：chrome 全画进 web（DeskMakeover 已如此）；确需覆盖才考虑 composition hosting（自带模糊 bug #5205 + 输入路由复杂度）。

**坑：最大化+多屏+最小化还原 → 白屏（官方 bug #2549）。**
修复：还原时强制 resize/重排 webview；上线前多屏混合 DPI 回归最小化/还原/最大化循环。

**坑：无边框 resize 边框（6px 白条等）。**
修复：`WM_NCCALCSIZE` 自绘 + `WM_NCHITTEST` 命中（不认触摸）。
来源：WebView2Feedback/issues/704 · /issues/1515

**坑：`window.open`/`target=_blank` 弹内嵌窗或跳 Edge。**
修复：`NewWindowRequested` → `Handled=true` + 外链 `Process.Start` 丢系统默认浏览器；`NavigationStarting` 白名单只允许 `https://appassets.local/*`。
来源：WebView2Feedback/issues/2587 · /issues/881

### 6. 性能陷阱

**坑：SharedBuffer 生命周期泄漏。** 每帧新建不释放 → 内存暴涨；上限 <2GB。
修复：复用一个（或双缓冲两个）SharedBuffer；native 侧用完 `Close()`；大数据永远 SharedBuffer 不走 postMessage JSON。
来源：specs/SharedBuffer.md · learn.microsoft.com postsharedbuffertoscript

**坑：最小化/被遮挡时 Chromium 节流 rAF/timer；`CalculateNativeWinOcclusion` 误判遮挡 → 可见窗口也被节流/空白。** 对实时预览类 app 是隐形杀手。
修复：`AdditionalBrowserArguments` 加 `--disable-features=CalculateNativeWinOcclusion`；按需 `--disable-background-timer-throttling` `--disable-renderer-backgrounding`（牺牲功耗，只在需要时）；配合 Page Visibility API 主动降级。
来源：WebView2Feedback/issues/1172 · rostacik.net webview2-setup · tauri/issues/5250

### 7. 主题 / 系统集成

**坑：`prefers-color-scheme` 与 Windows 主题不同步（滚动条/弹窗尤其）。**
修复：`CoreWebView2Profile.PreferredColorScheme = Auto`（同时决定 WebView2 自身 UI 明暗）；已知 #3696 滚动条有时不尊重 → CSS 显式配色兜底。host 与前端双向一致。
来源：learn.microsoft.com preferredcolorscheme · WebView2Feedback/issues/3696

**坑：高对比度 `forced-colors: active` 覆盖品牌色，图标/边框消失。**
修复：`@media (forced-colors: active)` 适配（`-ms-high-contrast` 已弃用），系统色关键字 Canvas/CanvasText/ButtonFace，必要处 `forced-color-adjust`。至少可读不崩。
来源：blogs.windows.com styling-for-windows-high-contrast · deprecating-ms-high-contrast

**坑：`prefers-reduced-motion` 不降级。**
修复：前端 `@media (prefers-reduced-motion: reduce)` 全量降级（spec 02 v3 已定为无洞覆盖）。

---

## DeskMakeover 补丁清单（build plan F6 的输入）

### 🔴 必打（发布前必须）
- [ ] Runtime 探测 + Bootstrapper 引导 + 友好兜底 UI（绝不裸崩）
- [ ] 数字签名 + IT allowlist 指引文档（杀软白屏头号问题）
- [ ] `ProcessFailed`/`BrowserProcessExited` 全分支兜底（Reload/重建/错误页）
- [ ] UDF 显式 `%LOCALAPPDATA%\DeskMakeover\WebView2`；非 ASCII 用户名宽字符处理
- [ ] PerMonitorV2 + `RasterizationScale` 跟踪；发丝线对齐设备像素；125%/150% 实测
- [ ] 首帧白闪三保险（DefaultBackgroundColor + 环境变量 + host 同色）
- [ ] 全资源单一虚拟 host（含字体，禁 file://）；字体 preload + block + fonts.ready 门（D2）
- [ ] `IsNonClientRegionSupportEnabled` + `app-region: drag`，保留 `WM_NCHITTEST` 兜底；多屏最小化/还原回归
- [ ] kiosk 化全套 settings 关闭（快捷键/右键/F12/状态栏/缩放/捏合/滑动导航/自动填充/AllowExternalDrop）
- [ ] `NewWindowRequested` + `NavigationStarting` 白名单，外链走系统浏览器
- [ ] 中文 IME 真机逐输入框实测（windowed hosting；多屏 + DPI 切换）
- [ ] SharedBuffer 复用 + Close()；`--disable-features=CalculateNativeWinOcclusion`

### 🟡 建议打
- [ ] `ScrollBarStyle = FluentOverlay` 或自绘滚动条（最低 Runtime 实测）
- [ ] `PreferredColorScheme = Auto` 与前端主题双向一致；滚动条配色兜底
- [ ] reduced-motion 全量降级（已在 spec 02 v3）
- [ ] 软件渲染/老显卡降级：blur→纯色、砍动画、花屏引导更新驱动
- [ ] ClearType 缓解：文字垫不透明底 + 字重微调

### 🟢 可暂缓
- [ ] `forced-colors` 高对比度完整适配（先保可读不崩）
- [ ] composition hosting 迁移（无需求不动）
- [ ] RDP/VM IME 定位精修（官方 bug 待修）
