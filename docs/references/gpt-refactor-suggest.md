---
ctxport: v2
source: chatgpt
url: "https://chatgpt.com/c/6a50b1c5-e814-83ec-a8a9-01e93fd4e49f"
title: Windows桌面美化方案分析
date: "2026-07-10T10:58:15.037Z"
nodes: 21
format: full
---

## User

这是我正在制作的一个Windows桌面美化应用。它通过Visual Studio提供用户操作界面，然后走.NET路径来替换桌面壁纸和图标。未来我想做成右下角的托盘长驻进程，用户拖入新图标后自动处理。请问当前方案能否实现？因为我的逻辑都在前端。如果不行，我考虑用 WASM 编写核心算法，以便在前端和 .NET 中复用。另外，虽然前端处理速度不错，但我想知道 OpenCV WASM 是否会更快。当然，我不确定 .NET 能否调用 WASM，也不确定这些想法是否可行。请帮我分析一下。

## Assistant

I’ll separate architecture, tray automation boundaries, WASM reuse, and OpenCV performance, focusing on ownership, maintainability, deployment, and real performance costs rather than mere feasibility.

## Assistant

The current architecture supports manual foreground processing, but reliable tray automation cannot depend on WebView2. Splitting TypeScript and C# renderers would inevitably cause feature drift.

## Assistant

## 结论

**能实现，而且你现在没必要为了托盘功能立刻重写成 WASM。**

准确地说，要分成两种情况：

1. **窗口隐藏到托盘，但 WebView2 仍然活着**
   你现有的 TypeScript 渲染器可以继续工作。.NET 负责监听桌面变化，把任务发给 WebView2 里的 Worker，前端生成 256px PNG，.NET 再组装 ICO、修改快捷方式或注册表、记录还原历史。这个方案现在就能做，不需要 WASM。

2. **窗口关闭后连 WebView2 也卸载，只保留一个很轻的托盘进程**
   那么当前纯前端渲染器确实没法直接运行。此时需要一个独立运行时，或者把核心算法抽成同时支持浏览器和原生 Windows 的模块。

所以真正的选择不是前端还是 .NET，而是：

> 你愿不愿意为了百分之百复用现有算法，让 WebView2 在托盘状态下继续占用内存。

我把你现有的路线图、产品架构、视觉与壳层、壁纸、WebView2 桥以及图标模块对齐看了一遍。整体方向其实已经很清晰，就是 Web 负责像素，C# 负责系统操作。

---

## 一、你当前方案本身是成立的

严格地说，Visual Studio 并没有提供你的用户界面。它只是开发工具。

你的真实运行架构是：

```text
WPF 窗口
  └ WebView2
      └ React 用户界面
          ├ TypeScript 图标渲染器
          └ Pixi 壁纸渲染器

.NET Host
  ├ 扫描桌面
  ├ 提取源图标
  ├ 写入 ICO
  ├ 修改快捷方式和系统图标
  ├ 替换壁纸
  ├ 快照、历史、恢复
  └ 托盘、监听器、UAC
```

这套职责划分是对的。

WebView2 官方就支持网页通过 `window.chrome.webview.postMessage` 向宿主发消息，宿主通过 `WebMessageReceived` 接收，再反向向网页发送 JSON 消息。你的 JSON RPC bridge 并不是什么邪门玩法，而是 WebView2 的标准通信路径。

你目前不能在真实宿主里完整运行，不是因为算法都在前端，而是因为文档里已经明确写了：

- Web 端已经使用 schema 3
- C# Host 仍然是 schema 1
- 目前只有浏览器加 mock 链路完整
- F8 才是把真实 Windows Host 接上的阶段

所以眼下最大的坑不是 WASM，而是你还没把桥接层收口。

现在为了未来托盘去重写渲染器，就像房子水电还没接通，你突然决定重做地基。不是不能做，是顺序有点蠢。

---

## 二、托盘自动美化完全可以沿用当前前端算法

我建议托盘版先采用下面这条链路：

```text
DesktopWatcher
      ↓
变化合并与延迟确认
      ↓
重新扫描并判断新增或变化项目
      ↓
IconJobCoordinator
      ↓
IIconRenderBackend
      ↓
WebView2 内的 TypeScript Worker
      ↓
256px PNG master
      ↓
.NET 组装 ICO、写入桌面、记录历史
```

也就是说：

- .NET 是眼睛和手
- 前端 Worker 是画师
- 托盘只是 WPF 窗口隐藏，不是进程退出

你甚至可以先不做新的渲染协议，继续使用现在的 `applyBakedBegin / Chunk / Commit` 思路，只增加一个后台任务入口。

例如：

```text
icons.backgroundRender
{
  revision,
  items,
  config,
  typeOverrides,
  kindPolicy
}
```

返回：

```text
{
  items: [
    {
      id,
      sourceIndex,
      masterPng
    }
  ]
}
```

对于一次只新增一个或几个桌面项目的场景，跨桥传输量很小，根本不值得为了省几毫秒引入一个新的 WASM 宿主。

### 监听器不要把 Created 事件当真相

这里需要特别小心。

`FileSystemWatcher` 确实可以监听创建、修改、删除和重命名，但官方文档明确提醒：

- 一次文件操作可能产生多个事件
- 杀毒软件也可能额外触发事件
- 短时间大量事件可能导致内部缓冲区溢出
- 缓冲区溢出后会丢失具体变化，只剩一个笼统错误通知

所以文件事件只能表示桌面可能变了，不能直接表示立刻处理这个文件。

正确做法是：

1. 同时监听用户桌面和公共桌面。
2. 收到事件后延迟几秒，合并安装器产生的一串事件。
3. 等待文件大小、时间戳、快捷方式目标稳定。
4. 重新执行一次增量扫描。
5. 用 source hash 和 ledger 判断是不是真的新增或变化。
6. 忽略自己刚刚写回的项目，避免死循环。
7. 监听器溢出时执行一次全量 reconciliation。
8. 应用启动时再做一次 catch-up，弥补关机期间错过的变化。

你自己的规格里已经写了 2 到 10 秒 debounce、默认 4 秒、poll fallback、active makeover ledger、首次提案、默认关闭和每次写入历史。这个方向非常靠谱，别删。

---

## 三、你当前文档里有一个真正需要改掉的架构矛盾

你现在一方面规定：

> Web 渲染用户看到的一切，并生成最终 256px master。

另一方面又规定：

> 后台自动美化使用冻结的 C# TileRenderer，不使用隐藏 WebView2。

而且文档还明确说，新样式只在 TypeScript 实现，C# renderer 只是 oracle 和未来后台 renderer。

这会产生一个迟早爆炸的问题：

```text
前台预览效果 A
前台手动应用效果 A
托盘自动处理效果 B
```

每增加一个形状、滤镜、颜色规则、分区策略，都会面对三个选择：

- 同时实现 TS 和 C#
- 后台不支持新效果
- 后台生成的效果与前台不一致

这就是经典的双渲染器陷阱。

**我建议把冻结的 C# TileRenderer 永久降级为测试 oracle，不要再让它承担正式的后台生产渲染。**

近期开启 WebView2 后台渲染。

长期再用真正共享的跨平台核心替代它。

这才符合你的 WYSIWYG 铁律。

---

## 四、托盘常驻 WebView2 的代价

方案可行，不代表完全免费。

WebView2 使用 Edge 的多进程架构，一个控制器通常会涉及浏览器、renderer、GPU 等多个进程，因此会带来额外启动时间和内存占用。微软的性能文档也明确建议减少 WebView2 实例数量，并复用同一个环境。

所以短期方案的代价主要是：

- 托盘状态下仍有若干 WebView2 进程
- React、Worker、WASM 或 Pixi 的内存仍然存在
- 低配置机器上，用户可能觉得一个桌面美化工具不该占这么多内存

但这里不要凭感觉。

先实测：

- 窗口显示时工作集
- 窗口隐藏 5 分钟后的工作集
- `TrySuspendAsync` 后的工作集
- 恢复后完成一次单图标渲染的延迟
- 空闲状态是否有持续 CPU 占用

WebView2 提供 `TrySuspendAsync` 来减少内存消耗，也提供 `Resume` 恢复网页活动。

一个现实的折中方案是：

```text
窗口隐藏
  ↓
空闲一段时间后 suspend WebView2
  ↓
桌面监听器发现新项目
  ↓
resume WebView2
  ↓
执行后台渲染
  ↓
再次进入空闲
```

这个方案需要在真实 Win10、Win11、不同 WebView2 Runtime 上验证，尤其要检查 Worker 恢复和字体、Canvas、资源缓存状态。

---

## 五、.NET 能不能调用 WASM

**可以，但不是 .NET 原生直接把 `.wasm` 当 DLL 调。一般需要嵌入一个 WebAssembly Runtime。**

最典型的是 Wasmtime。它有官方 .NET embedding，能够在 C# 中：

- 创建 Engine
- 加载 Wasm Module
- 实例化 Module
- 提供 host function
- 调用 Wasm 导出的函数

官方示例就是通过 NuGet 的 `wasmtime` 包，在 C# 中实例化模块并调用导出函数。

调用模型大概是：

```text
.NET
  ↓
Wasmtime
  ↓
icon_core.wasm
  ├ analyze
  ├ segment
  └ render
```

但这条路有三个问题。

### 1. 浏览器里的 Wasm 不一定能直接放进 Wasmtime

很多通过 Emscripten 构建的 Wasm 并不是一个独立模块，而是：

```text
JavaScript loader
  + Emscripten runtime glue
  + WebAssembly binary
```

模块可能依赖：

- JS 提供的内存管理
- 文件系统模拟
- Canvas
- Worker
- 回调表
- Emscripten 特有 imports

OpenCV.js 官方构建也会生成 JS loader，并根据能力加载 wasm、threads、simd、threadsSimd 等不同版本。

所以不能简单理解成：

```text
浏览器能加载 opencv.wasm
因此 Wasmtime 也能直接加载
```

理论上可以补齐 imports，实际上很容易变成自己实现半套 Emscripten runtime。

### 2. 你要自己设计内存 ABI

Wasm 导出函数适合处理数字和线性内存，不适合直接传复杂 C# 对象。

你需要设计类似：

```c
int dm_render_icon(
    const uint8_t* input_rgba,
    uint32_t width,
    uint32_t height,
    const uint8_t* config,
    uint32_t config_length,
    uint8_t* output_rgba,
    uint32_t output_length
);
```

然后自己管理：

- 分配
- 释放
- 输入复制
- 输出复制
- 错误码
- ABI 版本
- 批处理
- 并发 Store

如果每个小步骤都跨一次边界，性能会很难看。应该把一次图标渲染作为一个粗粒度调用。

### 3. 你自己的受信任算法并不特别需要沙箱

Wasmtime 最大的价值之一是运行隔离的第三方 Wasm。

但这个 Wasm 是你自己编写、自己签名、自己随应用发布的。为了运行自己的可信算法，多带一套 Wasmtime Runtime，收益未必划算。

---

## 六、比 .NET 调 WASM 更合适的两种复用方式

### 方案 A：同一份 Rust 或 C++ 源码，分别编译 WASM 和原生 DLL

```text
icon-core 源码
   ├ browser-wasm
   │    └ React Worker 调用
   └ windows-x64 / windows-arm64 DLL
        └ .NET P/Invoke 调用
```

这通常是最务实的长期方案。

优点：

- 浏览器使用 WASM
- 托盘使用原生机器码
- 不需要在 .NET 中嵌入 Wasmtime
- 同一份算法源码
- 原生端性能通常更好
- WebView2 可以在托盘空闲时彻底释放
- DLL 接口可以非常小

代价：

- 要维护两套构建目标
- 需要处理 ABI
- 原生 DLL 要分别发布 x64 和 arm64
- 原生和 Wasm 浮点结果仍需 parity fixture 验证

如果以后确实要用 OpenCV，我会偏向 C++。

如果主要是你自己的色彩、分割、蒙版和组合算法，我会偏向 Rust。

### 方案 B：核心算法写成纯 C#，桌面原生运行，React 通过 .NET WASM Worker 调用

这个方案现在也确实可行。

微软已经提供了 .NET 10 及以上版本中，让 React 前端通过 Web Worker 运行 C# WebAssembly 的官方示例。算法可以用 `[JSExport]` 暴露给 Worker，React 通过消息调用。微软也特别建议减少数据传输、使用批处理、保持 Worker 常驻，避免反复支付初始化成本。

架构会变成：

```text
纯 C# IconKernel
   ├ DeskMakeover.App 直接引用
   └ 编译成 .NET WebAssembly
        └ React Worker 调用
```

这个方案对你有个特殊优势：

你已经有一套 C# oracle，理论上可以把它改造成不依赖 WPF、Win32、System.Drawing 的纯 `Span<byte>` 图像算法库。

但它也有明显成本：

- Web 端需要额外加载 .NET Wasm Runtime
- React 集成的工具链仍然偏重
- 现有 TS renderer 已经完成，迁移是一次大重写
- 纯 C# 核心必须完全摆脱 WPF 图像类型
- 浏览器 AOT、包体积、启动时间都需要验证

所以它是一个真实可行的长期选项，但我不建议拿它阻塞第一版。

---

## 七、几个方案放在一起看

| 方案 | 当前复用程度 | 托盘空闲开销 | 复杂度 | 我的判断 |
|---|---:|---:|---:|---|
| 托盘常驻 WebView2 + TS Worker | 最高 | 较高 | 最低 | **近期首选** |
| .NET 通过 Wasmtime 调同一个 Wasm | 高 | 中等 | 高 | 只有必须同一二进制时考虑 |
| C# 原生 + C# .NET Wasm Worker | 重写后很高 | 低 | 中高 | C# 团队可考虑 |
| Rust/C++ 编译 Wasm + 原生 DLL | 重写后很高 | 最低 | 中高 | **长期首选** |
| 继续维护 TS 和 C# 两套 renderer | 表面最高 | 低 | 长期最高 | **建议砍掉** |

这里有个很重要的区别：

> 共享同一个二进制，不一定比共享同一份源码更值钱。

为了让桌面端和浏览器都执行完全相同的 `.wasm`，你可能引入 Wasmtime、内存 ABI 和运行时分发。

而编译同一份 Rust 或 C++ 源码，浏览器生成 Wasm，Windows 生成原生 DLL，通常更简单、更快，也更符合桌面软件的常识。

---

## 八、OpenCV WASM 会不会更快

**可能会，但绝不等于一定会。**

这里最容易犯的错误，就是把 WASM 和 OpenCV 当成更快的同义词。

WASM 首先是一种可移植执行格式。

OpenCV 首先是一套通用计算机视觉库。

性能只是可能的副产品。

OpenCV.js 官方构建支持普通 Wasm、线程、SIMD 以及线程加 SIMD等变体，也提供了针对 `cvtColor`、`resize`、`threshold` 等操作的性能测试入口。Emscripten 本身也支持 Wasm SIMD，以及基于 SharedArrayBuffer 的 pthreads。

### OpenCV 可能明显更快的部分

你的图标管线里，下面这些属于 OpenCV 的强项：

- resize
- Gaussian blur
- morphology
- flood fill
- connected components
- threshold
- distance transform
- 颜色空间转换
- 边缘和轮廓运算

这些都是大块、规则、可批量、容易 SIMD 化的运算。

### OpenCV 不会自动帮你解决的部分

你的大量核心价值是定制规则：

- Apple/Figma 特殊形状
- per-icon profile
- own background 判断
- rim law
- OKLab 配色规则
- plate 和 subject 组合
- 跨图标 hue spread
- 快捷方式 mark
- 自定义滤镜
- 各类 exception 和 type override

这些并不是调用几个 OpenCV API 就结束的。

最终仍然要自己写大量循环、分支和组合逻辑。

### 256×256 小图像不一定适合套重库

你的单张图标 master 是 256×256，像素量并不大。

对于这种小图：

- JS TypedArray 循环可能已经很快
- V8 JIT 能优化热循环
- OpenCV Mat 创建有成本
- JS 内存与 Wasm 内存之间可能需要复制
- 每次调用 Embind 也有边界成本
- 频繁分配和销毁 Mat 可能吃掉计算收益

OpenCV.js 的官方示例就是通过 `ImageData` 创建 `cv.Mat`，输出时再从 `dst.data` 构造新的 `ImageData`，并要求显式删除 `Mat`。这意味着数据移动、对象生命周期和缓冲区复用必须纳入性能测试，不能只测某一个 OpenCV 内核。

### 对你最合理的使用方式

不要把整个 icon compositor 替换成 OpenCV。

只在 profiler 证明某个热点真的慢之后，把那个热点换掉，例如：

```text
segmentSubject
distance field
large blur
connected components
resize ladder
```

甚至可以做一个最小 OpenCV.js 构建，只保留需要的 core 和 imgproc 功能。

你现在的规格已经规定离散操作在 300 个图标下应该控制在约 50ms 内，而且现有 TS renderer 使用了 staged caching 和 Worker pool。既然你主观感觉已经不错，就先拿这个指标做真实基准，不要因为技术焦虑把一套已经跑得不错的东西重写掉。

建议基准至少测四组：

| 测试组 | 内容 |
|---|---|
| A | 当前 TypeScript TypedArray + Worker |
| B | 自定义 Wasm，单线程 |
| C | 自定义 Wasm + SIMD |
| D | 最小 OpenCV.js Wasm + SIMD |

每组记录：

- 冷启动时间
- 第一张图标时间
- 单图 p50 和 p95
- 50 张总时间
- 300 张总时间
- 峰值内存
- Wasm 或 JS 包体积
- PNG 编码时间
- 与 golden fixture 的差异
- 连续运行后的内存是否回落

我很怀疑 OpenCV.js 在你的整个流水线上会碾压现有 TS。

更可能的结果是：

> 个别大算子更快，但完整端到端并没有快多少，甚至因为复制和初始化更慢。

---

## 九、我建议你按这个顺序做

### 第一阶段：不要动渲染核心

先完成 F8：

- C# Host 升级到 schema 3
- 真实 `icons.scan` sourceUrls
- `applyBakedBegin / Chunk / Commit`
- wallpaper source 和 applyBaked
- 包装、ElevatedHelper、真实发布产物
- golden parity
- 实机应用与恢复

这一步完成后，你的首版架构就是成立的。

### 第二阶段：加入一个渲染后端抽象

不要拆出一百个接口，三个边界足够：

```csharp
public interface IDesktopChangeSource
{
    IAsyncEnumerable<DesktopChangeHint> WatchAsync(
        CancellationToken cancellationToken);
}

public interface IIconRenderBackend
{
    Task<RenderBatchResult> RenderAsync(
        RenderBatchRequest request,
        CancellationToken cancellationToken);
}

public interface IDesktopApplyService
{
    Task<ApplyResult> ApplyAsync(
        RenderBatchResult result,
        CancellationToken cancellationToken);
}
```

当前实现：

```text
IIconRenderBackend
  └ WebView2IconRenderBackend
```

未来实现：

```text
IIconRenderBackend
  ├ WebView2IconRenderBackend
  └ NativeIconRenderBackend
```

这样你可以先交付托盘功能，以后再换核心，监听、历史、恢复和 shell 写入完全不动。

### 第三阶段：托盘 MVP 继续使用 WebView2

- X 关闭窗口时隐藏，不退出进程
- 托盘菜单提供打开、暂停自动美化、退出
- 监听器只产生 change hint
- 4 秒 debounce
- 增量 scan
- 首次只提案，不静默写入
- 后续按 consent 设置执行
- 每次自动变化进入历史
- 绝不在后台弹 UAC
- 特权项目留待用户打开窗口确认

这是最短路径，也最符合你当前文档里的信任模型。

### 第四阶段：用数据决定是否迁移

只有出现以下任一情况，才值得抽 WASM 或原生核心：

- 托盘隐藏后 WebView2 稳定占用明显过高
- 300 图标实时预览达不到你的响应指标
- 用户确实大量使用自动美化
- 新样式越来越多，C# oracle 已经严重阻碍迭代
- 你准备把渲染能力复用到 CLI、Explorer 模块或其他平台

届时优先选择：

```text
纯 IconKernel
  ├ Web Worker 中运行 Wasm
  └ .NET 中运行原生 DLL
```

不要迁移 React 状态、桥接、历史、Shell 操作，也不要迁移 Pixi 壁纸 compositor。壁纸本来就是一次交互式编辑加一次 apply，不需要为了托盘自动化一起被拖下水。

---

## 最终判断

你的当前方案不是不可行，而是**首版宿主尚未接完，后台渲染策略又提前留下了一套双实现债务**。

最合理的路线是：

> 首版继续 WebView2 加 TypeScript。
> 托盘 MVP 也继续复用 WebView2 Worker。
> 测出真实内存和性能问题后，再把图标纯算法抽成 Wasm 加原生双目标。
> .NET 不必调用 Wasm，优先调用同源编译出来的原生 DLL。
> OpenCV 只打热点，不要接管整个渲染器。

一句话概括：

> 前端不是问题，重复实现才是问题。WASM 也不是答案，它只是你确定需要第二个运行环境之后的一种运输工具。

## User

从性能和代码复用率的角度来看，你认为哪个方案最优？请给出一个最优方案。

## Assistant

I’ll choose one architecture based on end-to-end performance, reuse, and migration cost: one shared core, uncompromised frontend preview, and no permanently resident WebView2.

## Assistant

The key is eliminating parallel TS and C# renderers so foreground previews and tray processing execute identical algorithmic source.

## Assistant

## 我拍板的最优方案

**把图标渲染算法重构成一个纯 Rust 核心，然后编译成两份产物：**

```text
同一份 Rust 源码
        │
        ├── deskmakeover_core.wasm
        │       └── React Web Worker 调用
        │           负责实时预览和手动应用
        │
        └── deskmakeover_core.dll
                └── .NET 通过 LibraryImport / PInvoke 调用
                    负责托盘后台自动美化
```

一句话概括：

> **共享源码，不强求共享二进制。前端跑 WASM，桌面后台跑原生 DLL。**

并且：

- **不让 .NET 调 WASM**
- **不让隐藏 WebView2 承担后台渲染**
- **不继续维护 TypeScript 和 C# 两套算法**
- **不把 OpenCV WASM 作为核心依赖**
- **壁纸模块继续保留 Pixi，不迁移到 Rust**

这是性能、代码复用率、长期维护成本三者之间最优的交点。

---

## 为什么这是最优，而不是 .NET 调 WASM

.NET 当然可以通过 Wasmtime 加载和调用 WASM。Wasmtime 的 .NET SDK 可以创建 Engine、加载 Module、实例化并调用导出函数。

但你的算法是自己编写、自己签名、自己随软件发布的可信代码。此时把链路做成：

```text
.NET
  → Wasmtime Runtime
    → WebAssembly
      → 图像算法
```

相比：

```text
.NET
  → PInvoke
    → 原生 DLL
      → 图像算法
```

前者平白增加了：

- Wasmtime Runtime 的体积
- 模块实例化和运行时管理
- Wasm 线性内存管理
- C# 到 Wasm 的数据复制
- Wasm ABI 适配
- 额外的发布和兼容性测试

而 Rust 本身可以从同一个 crate 编译出供其他语言加载的 `cdylib`，Windows 上就是 DLL。

.NET 通过 `LibraryImport` 或 PInvoke 调用这种原生库是标准路径；`LibraryImport` 还能在编译期生成互操作代码，避免运行时生成传统 PInvoke IL stub。

所以最合理的是：

```text
浏览器环境需要 WASM，所以编译 WASM
Windows 原生环境不需要 WASM，所以编译 DLL
```

不要为了所谓百分之百二进制复用，搞出一层运行时套娃。

---

## 性能为什么最好

### 前端路径

```text
React
  → Web Worker
    → Rust WASM
      → RGBA 输出
```

Rust 可以通过 `wasm-bindgen` 和 JavaScript、TypedArray、Web Worker 交互。

这里的关键不是把代码编译成 WASM 就自动起飞，而是下面四件事：

1. 源图像只上传一次
2. 分析结果永久缓存
3. 配置变化只传几十或几百字节
4. 一次调用完成一张或一批图标，而不是每个算子跨一次边界

推荐做成持久化会话：

```text
createSession
registerSources
analyzeDesktop
setLook
renderPreview
renderBake
destroySession
```

而不是：

```text
JS 调 resize
JS 调 segment
JS 调 blur
JS 调 compose
JS 调 filter
```

后者会被边界调用和内存复制活活拖死。

Web 端不要急着启用 Wasm 多线程。直接维持你现在的 Worker Pool 思路：

```text
Worker 1 → 一个 WASM 实例
Worker 2 → 一个 WASM 实例
Worker 3 → 一个 WASM 实例
Worker 4 → 一个 WASM 实例
```

每个 Worker 单线程处理自己的图标分片。这样不需要 SharedArrayBuffer、共享内存和复杂的线程初始化。浏览器 Wasm 线程本质上依赖 Worker 和共享内存，部署与同步复杂度明显更高。

你这里图标天然可并行，没必要先上 Wasm 内部线程。

### .NET 托盘路径

```text
FileSystemWatcher
  → 增量扫描
    → WIC 解码为 RGBA
      → Rust Native DLL
        → 256px RGBA
          → C# ICO Writer
            → Shell 写入
```

这条链路不需要启动或唤醒 WebView2，也不需要 PNG 在多个运行时之间来回编码、传输和解码。

.NET 可以在图标级别并行：

```text
图标 A → ThreadLocal RenderContext
图标 B → ThreadLocal RenderContext
图标 C → ThreadLocal RenderContext
```

每张图标只进入一次 DLL，原生代码内部完成完整流水线。

因此它的性能上限是这几个候选方案里最高的。

---

## 真正能复用多少代码

这里要把一个容易想歪的东西说清楚：

> **你不应该追求整个项目的代码复用率，而应该追求所有影响像素结果的代码只有一份。**

合理的目标大约是：

| 部分 | 是否复用 |
|---|---|
| 图标分析和分类 | 100% |
| 背景检测 | 100% |
| 主体分割 | 100% |
| 形状与蒙版 | 100% |
| OKLab 与配色 | 100% |
| RIM 法则 | 100% |
| 跨图标 hue spread | 100% |
| 阴影、光环、标识 | 100% |
| 滤镜 | 100% |
| 图像组合 | 100% |
| React 状态和交互 | 不复用 |
| Windows 扫描与写入 | 不复用 |
| ICO 封装 | 不必复用 |
| 历史、恢复、UAC | 不复用 |

也就是：

- **图标渲染内核复用率可以达到约 90% 到 95%**
- 剩下约 5% 到 10% 是 WASM 和 Native 的薄适配层
- 整个应用当然达不到 90%，也根本没必要

UI 与 Windows Shell 是两个完全不同的问题。强行共享，只会把纯净的算法核心污染成一坨跨平台胶水。

---

## Rust 核心应该怎么切

我建议只有三个 crate，不要拆成二十个微型包。

```text
render-core/
├── crates/
│   ├── dm-core/
│   │   ├── profile.rs
│   │   ├── background.rs
│   │   ├── segment.rs
│   │   ├── shapes.rs
│   │   ├── color.rs
│   │   ├── field.rs
│   │   ├── marks.rs
│   │   ├── filters.rs
│   │   ├── compositor.rs
│   │   └── session.rs
│   │
│   ├── dm-wasm/
│   │   └── wasm-bindgen 适配
│   │
│   └── dm-native/
│       └── C ABI DLL 适配
```

### `dm-core`

必须保持纯净：

- 不访问 DOM
- 不访问文件系统
- 不访问 Windows API
- 不依赖 WPF
- 不解析快捷方式
- 不编码 ICO
- 不弹 UAC
- 不知道 WebView2 存在
- 不知道 React 存在

输入和输出只认：

```text
RGBA8
宽高
stride
RenderConfig
DesktopContext
```

### `dm-wasm`

只负责：

- TypedArray 与 Rust 内存映射
- Source Handle
- Session Handle
- Worker 消息
- 输出 RGBA 视图
- 错误转换

### `dm-native`

只提供稳定、版本化的 C ABI：

```c
dm_session_create
dm_session_destroy
dm_register_source
dm_remove_source
dm_set_desktop_context
dm_set_look
dm_render_preview
dm_render_batch
dm_get_last_error
```

.NET 端使用 `LibraryImport`。

---

## 你的核心 API 不能只按单图标设计

这是非常重要的一点。

你的算法已经存在跨图标 hue spread、type override、Field 色板去重等桌面级规则。文档里也明确规定，跨图标色相扩散会参与预览和 bake。

所以不能做成：

```text
renderIcon(icon, config)
```

否则每张图标不知道其他图标的色相分布，前台和后台很容易出现不同结果。

应该是：

```text
RenderSession
  ├── register 300 个源图标
  ├── 分析全部 metadata
  ├── 执行跨图标色相调整
  ├── 解析 typeOverrides
  ├── 生成每个图标的 resolved config
  └── 渲染指定图标集合
```

推荐接口模型：

```rust
pub struct RenderSession {
    sources: SourceStore,
    profiles: ProfileCache,
    masks: MaskCache,
    field_seeds: FieldSeedMap,
    look: ResolvedLook,
}

impl RenderSession {
    pub fn register_source(&mut self, id: SourceId, image: ImageView);
    pub fn set_desktop(&mut self, desktop: DesktopDescriptor);
    pub fn set_look(&mut self, look: Look);
    pub fn render(&mut self, request: RenderRequest) -> RenderOutput;
}
```

前端调色时，不再重复做：

- 背景识别
- 主体分割
- rim 分析
- 主色提取

这些都应存在 Session 缓存里。

这件事带来的性能收益，通常比单纯把 TypeScript 换成 WASM 更大。

---

## 像素协议必须先钉死

为了保证 Native 和 WASM 结果一致，先定义统一输入：

```text
颜色空间：sRGB
格式：RGBA8
Alpha：非预乘
内存布局：row-major
通道顺序：R G B A
stride：显式传递
尺寸：宽高显式传递
```

所有影响结果的逻辑都进入 Rust：

- `resolveTypeConfig`
- own background 判断
- subject mask
- RIM plate
- shadow polarity
- hue spread
- mark geometry
- filter 参数
- shape geometry
- 缩放与采样规则

尤其不要出现这种情况：

```text
TypeScript 决定 plate color
Rust 负责绘制
C# 又重新决定一次 type override
```

凡是影响像素的判断，只能有一个最终裁判。

---

## 如何保持 WYSIWYG

你的现有铁律是预览和 bake 使用同一套代码。当前规格里，Web 负责所有可见像素和 256px master，C# 只负责 Shell、ICO 和应用。

建议迁移后继续遵守：

### 用户打开应用时

```text
预览：Rust WASM
手动 bake：同一个 Rust WASM
```

所以用户看到什么，手动应用就是什么。

### 托盘后台时

```text
自动 bake：同源编译的 Rust Native DLL
```

WASM 和 Native 是同一份 Rust 算法，但底层浮点和 SIMD 可能存在微小差异。因此：

- 颜色转换尽量使用 LUT
- Alpha 和 resize 尽量使用整数或固定点
- 不依赖无序并行归约
- 不依赖平台随机数
- 关闭会造成明显差异的浮点快速数学
- Native 与 WASM 跑同一批 golden fixtures

要求可以设成：

```text
普通形状与配色：逐像素一致，或极小容差
模糊、阴影与复杂滤镜：SSIM 容差
分类和分支选择：必须完全一致
```

最重要的不是每个模糊边缘是否差一个灰度，而是绝不能出现：

```text
WASM 判定为 own background
Native 判定为 irregular artwork
```

分类分支必须百分之百一致。

---

## 为什么不选择 OpenCV WASM

**不把 OpenCV 放进核心。**

OpenCV.js 确实可以构建普通 Wasm、threads、SIMD、threads + SIMD 等不同版本，也提供 `resize`、`threshold` 等算子的性能测试。

但它不适合成为你整个项目的渲染架构。

你的核心竞争力不是：

```text
resize
blur
threshold
```

而是：

```text
own background detector
corner symmetry
subject segmentation
RIM law
Field plate
type distinction
cross-icon hue spread
silhouette marks
形状与视觉规则
```

这些绝大多数还是要自己写。

OpenCV.js 还引入 `cv.Mat` 生命周期、显式释放和数据适配。官方文档也要求不再使用时手动删除 `Mat`，图像通常需要从 `ImageData` 构造成 Mat。

对于大量 256×256 小图标，最终性能很容易被下面这些东西吃掉：

- Mat 创建和销毁
- 内存复制
- JS 到 Embind 边界
- OpenCV 通用类型分发
- 未使用模块的包体积
- 初始化成本

所以最优策略不是 OpenCV WASM，而是：

> **自己实现一套小而确定的专用像素内核。**

只有 profiler 证明某个算子，例如 distance transform 或 connected components，占据了绝大部分总时间，才考虑替换那一个算子。不要为了一个锤子，把整个房子改成五金店。

---

## 为什么不选择 C# 作为共享核心

理论上也可以：

```text
C# Core
  ├── .NET 原生执行
  └── .NET WebAssembly 给 React 调用
```

但你的前端只需要一个图像计算模块，却要为此带上 .NET Wasm Runtime、启动逻辑和 JS 互操作层。

这更适合本来就在 Blazor 或 .NET WebAssembly 体系里的产品。

你现在是：

```text
React + Bun + Vite + Web Worker
```

Rust WASM 更像一个真正独立的计算库，不会把前端运行时绑进另一套应用框架。

性能上限、包体积控制和运行时独立性都会更好。

---

## 壁纸模块不要一起迁移

你的壁纸模块现在是 Pixi v8 + WebGL2，并且预览与 bake 已经使用同一个 compositor。

它没有托盘后台自动处理的刚性需求，所以不要为了架构统一把它强塞进 Rust。

保留：

```text
图标：
Rust Core
  ├ WASM
  └ Native DLL

壁纸：
Pixi WebGL2
  └ Web 前端
```

这不是不统一，这是在正确的边界上统一。

图标是 CPU 图像算法，并且需要前台和后台两种运行环境。

壁纸是 GPU 交互式 compositor，只需要 Web 环境。

拿同一把锤子敲所有钉子不叫架构统一，叫强迫症。

---

## 对你现有架构的直接修正

你当前文档存在一个长期一定会炸的矛盾：

- Web renderer 是用户可见结果和手动 bake 的权威
- C# TileRenderer 被冻结
- 新样式只在 TypeScript 实现
- 未来后台自动美化却准备让 C# TileRenderer 负责

这意味着未来自动美化和用户预览迟早不一致。

Rust 双目标方案直接把这个矛盾消掉：

```text
旧：
TS Renderer        C# Renderer
前台规则 A          后台规则 B

新：
               Rust Core
              ╱         ╲
         WASM             DLL
        前台               后台
```

现有 C# TileRenderer 可以在迁移期间继续做 oracle。

迁移完成后：

- 删除 C# TileRenderer
- 删除 TypeScript 像素实现
- 保留 TypeScript Worker adapter
- 保留 golden fixtures
- Rust Core 成为唯一算法真相

---

## 最合理的迁移顺序

你现在 Web 侧已经完成，剩余主要是 F8 Host 集成。

所以不要大爆炸重写。

### 第一步

先增加渲染后端接口：

```csharp
public interface IIconRenderBackend
{
    Task<RenderBatchResult> RenderAsync(
        RenderBatchRequest request,
        CancellationToken cancellationToken);
}
```

当前：

```text
WebIconRenderBackend
```

未来：

```text
NativeIconRenderBackend
```

### 第二步

按纯度从高到低迁移：

```text
颜色与数学
→ shape geometry
→ mask 和距离计算
→ profile/background detector
→ segmentation
→ plate 和 field
→ marks
→ filters
→ 完整 compositor
```

每迁移一个模块：

```text
TypeScript 输出
vs
Rust WASM 输出
vs
Rust Native 输出
```

同时跑 fixture。

### 第三步

Web 端切换到 Rust WASM，但保留旧 TypeScript renderer 作为一段时间的开发期对照。

### 第四步

实现 Native DLL 后台后端和托盘 watcher。

### 第五步

确认 fixtures、性能和实机结果后，删除两套旧 renderer。

---

## 最终结论

**最终方案就选这个：**

```text
Rust 单一图标渲染核心
  ├── 编译为 WASM，供 React Web Worker 调用
  └── 编译为原生 DLL，供 .NET 托盘进程通过 LibraryImport 调用
```

外围职责保持不变：

```text
React
  负责交互、状态、预览调度

Rust
  负责一切影响像素的算法

.NET
  负责监听、扫描、文件、ICO、Shell、历史、恢复、UAC

Pixi
  继续负责壁纸
```

这是我认为唯一同时满足下面四件事的方案：

- 原生后台性能上限最高
- 前端仍然能高效实时预览
- 图标算法只维护一份
- 不引入 Wasmtime 和 OpenCV 这种不必要的重量级中间层

最核心的一句话是：

> **把规则写一遍，把它编译两遍。不要把同一个二进制硬塞进两个不一样的世界。**

## User

非常不错，我很认可你的观点。如果我选择这套方案，基于我现有的业务和方向，有没有推荐的社区生态库可以减少我的工作量？比如图标处理、像素计算、主体识别等，有没有很好的方案？或者任何其他能减少工作量的建议，也请大胆告诉我。我不希望别人已经做了大部分工作，我却还要重新实现，既浪费时间又要踩同样的坑。

## Assistant

I’ll evaluate reusable libraries across the real pipeline, prioritizing WASM/native compatibility, licensing, footprint, and complexity, then distinguish commodity components from product-specific algorithms.

## Assistant

The likely foundation is tiny-skia, imageproc, fast_image_resize, and palette—not full OpenCV—with classical segmentation first and lightweight models only for low-confidence cases.

## Assistant

Icon subjects differ from photographic foregrounds: document sheets, folders, badges, baked corners, and glows can fool generic background-removal models, so they shouldn’t be authoritative.

## Assistant

## 我给你的最终选型

有，而且你完全没必要从像素循环开始造轮子。

我建议生产环境只用这一组核心依赖：

```text
dm-core
├── tiny-skia             路径、蒙版、描边、渐变、图层合成
├── imageproc             阈值、泛洪、形态学、连通域、距离变换、模糊
├── palette               sRGB、线性 RGB、OKLab、OKLCH 转换
└── fast_image_resize     高质量缩放与 SIMD

dm-wasm
├── wasm-bindgen
└── serde-wasm-bindgen

dm-native
├── 稳定 C ABI
├── cbindgen              生成 C 头文件，仅构建期使用
└── .NET LibraryImport

按需启用
├── quantette             主色、调色板、颜色聚类
├── resvg/usvg            将来处理 SVG 图标
└── rten                  低置信度时的 ONNX 模型兜底
```

这套东西不会替你做产品判断，但能吃掉绝大部分机械劳动。你真正应该自己维护的，是 DeskMakeover 独有的审美和决策层，而不是高斯模糊、连通域、颜色空间转换这些工业零件。

---

# 一、四个必须采用的核心库

## 1. `tiny-skia`：作为统一的 CPU 光栅与合成底座

这是我最推荐的基础库。

它适合负责：

- 贝塞尔路径光栅化
- Apple、Samsung、书签、花瓣等形状蒙版
- 填充和描边
- 线性、径向渐变
- clip
- 图像变换
- 图层混合
- 阴影和光环最终合成
- 抗锯齿边缘

`tiny-skia` 是一个纯 CPU、低层级的 Rust 2D 光栅库，支持路径填充、描边、渐变、图案、裁剪和图像混合，不带文本系统，依赖和二进制体积都相对克制。对于你这种固定尺寸、无文字排版的图标合成器，它比完整 Skia 更合适。

它不应该负责：

- 判断图标有没有自带背景
- 决定 RIM plate 颜色
- 主体识别策略
- 跨图标 hue spread
- 类型差异规则

也就是说：

> 你把形状和审美规则告诉它，它负责稳定地把像素画出来。

你现有的 Apple cubic、Figma corner smoothing、OEM mask 路径已经是产品规范，不要再找一个所谓 squircle 库替代。直接把现有路径数学迁移到 Rust，再交给 `tiny-skia` 光栅化。

### 最大的坑：预乘 Alpha

`tiny-skia::Pixmap` 使用的是 **premultiplied RGBA**。而你的分析算法更适合读取 straight-alpha RGBA。

所以不要让 `Pixmap` 成为整个项目通用的图像类型。建议明确分成：

```rust
/// 输入、分析、FFI 使用
struct SourceImage {
    pixels: Vec<u8>, // straight-alpha sRGB RGBA8
    width: u32,
    height: u32,
    stride: u32,
}

/// 二值或灰度分析蒙版
struct Mask8 {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

/// 仅合成阶段使用
struct RenderSurface {
    pixmap: tiny_skia::Pixmap, // premultiplied RGBA
}
```

边界是：

```text
SourceImage
    ↓ 分析、分割、颜色提取
IconProfile + Mask8
    ↓
转换为预乘 Alpha
    ↓
tiny-skia 合成
    ↓
转回 straight-alpha RGBA
    ↓
PNG 或 .NET
```

这个协议第一天就钉死，不然后面会遇到一堆黑边、半透明像素变色、模糊边缘发灰之类的鬼故事。

---

## 2. `imageproc`：主体识别的传统视觉工具箱

`imageproc` 能直接替你完成大部分基础视觉算法：

- Otsu 阈值
- 自适应阈值
- 边界泛洪
- 形态学膨胀和腐蚀
- 开运算和闭运算
- 连通域标记
- 轮廓提取
- 距离变换
- Gaussian blur
- Sobel 等梯度运算

这些正好覆盖你当前主体分割、极致单色、silhouette mark、halo、rim band 所需的机械算子。

例如你现有的主体分割，可以组合成：

```text
Alpha silhouette
    ↓
边界颜色建模
    ↓
从四边和四角 flood fill
    ↓
OKLab 色差阈值
    ↓
Otsu 分割
    ↓
形态学 closing
    ↓
去掉微小连通域
    ↓
填洞
    ↓
主体 mask
```

自由形状的光环和贴边标识，则可以直接走：

```text
alpha mask
    ↓
distance transform
    ↓
取指定距离区间
    ↓
得到外侧 ring
```

这比自己手写每一个两遍扫描、队列泛洪和 connected-component 算法靠谱得多。

### 第二个大坑：线性颜色空间

`imageproc` 文档明确提醒，它的图像运算通常假定输入位于线性颜色空间，而不是直接对 sRGB 数值进行视觉上正确的处理。

因此：

- 二值 mask、Alpha mask、形态学可以直接算
- 颜色插值、缩放、模糊应在线性 RGB 下算
- 色差判断应转 OKLab
- 不要直接拿 sRGB 的 R、G、B 做欧氏距离
- 阴影通常只模糊 Alpha mask，再着色合成，不必模糊完整 RGBA

另外，`imageproc` 的默认特性可能带入 Rayon、文字和 FFT 等你目前用不上的东西。建议关闭默认特性，只开启实际需要的模块，特别是 WASM 构建不要一开始把所有东西打进去。

---

## 3. `palette`：颜色决策的正确性底座

你有大量颜色规则：

- sRGB 与线性 RGB
- OKLab、OKLCH
- 色相合并
- chroma fitting
- 深板和浅板
- 感知灰度
- ΔE 测试
- gamut clamp
- hue spread

这部分最容易出现看起来差不多，实际上到处埋雷的手写公式。

`palette` 提供类型化的 sRGB、线性 RGB、OKLab、OKLCH 转换，适合作为你颜色系统的正确性基线。

我建议：

```text
第一阶段
所有颜色转换都走 palette

性能分析后
sRGB → linear 改成 256 项 LUT
部分热点循环做专用实现

但 fixtures 始终拿 palette 版本当 reference
```

不要一开始为了省几纳秒重写 OKLab。

你每张图才 256×256，颜色转换通常不会是最先爆掉的地方。缓存做对了，比手撸 SIMD 重要一百倍。

---

## 4. `fast_image_resize`：统一缩放算法

它适合处理：

- 源图标准化
- 预览尺寸生成
- 256px 内部缩放
- plate 中主体重排
- 未来希望统一 WASM 和 Native 缩放逻辑时使用

它支持 RGBA 像素、Alpha 处理以及多种 CPU SIMD 路径，包括 WASM SIMD128。文档也提供了线性化和 Alpha 乘除相关的处理方式。

但当前先别动你已经稳定的 C# ICO ladder。

你的现有职责是：

```text
Rust
  输出 256px master

C#
  从 256 下采样到 48/32/24/20/16
  组装 ICO
```

这部分已经属于成熟的 Windows 输出基础设施，没必要为了看起来更统一就迁移。你的规格也已经明确把 ICO ladder 和包装留在 C#。

`fast_image_resize` 先用于 Rust 内部和浏览器预览。等整个 Rust 迁移结束，再决定是否连 ICO ladder 一起统一。

---

# 二、主体识别的最优方案

我不建议第一版直接加入神经网络。

不是因为 AI 没用，而是因为你的问题不是普通照片去背景。

你的输入里有：

- 带透明边缘的 logo
- 自带纯色板的应用图标
- 白纸文档图标
- 文件夹
- 四角 badge
- 发光边缘
- 阴影
- 照片填充
- 预先烘焙圆角
- 全屏不透明方形图
- 不规则主体
- 单色线稿
- 深色主体配透明边缘

通用去背景模型看到一张白纸文档，可能把纸当主体，也可能只留下纸上的字。看到文件夹，可能把整个文件夹留下，也可能把标签和主体拆成几块。它根本不知道你后面要做的是 plate split、RIM law 还是保留原背景。

你的规格已经不是一般的分割问题，而是一个带有产品语义的分类加分割系统。

## 推荐四层管线

### 第一层：确定性快速路径

先处理最容易、覆盖率最高的情况：

```text
coverage ≥ 0.98
    → full bleed

四角和外环颜色稳定
且角落 inset 对称
    → own background

Alpha 边界明显
    → transparent irregular artwork
```

这一步主要用：

- Alpha coverage
- 边界跨度
- 四角采样
- 多深度 outer ring
- corner symmetry
- bbox 和轮廓几何

这些逻辑必须由你维护，因为它们正是 DeskMakeover 的领域知识。

### 第二层：传统视觉分割

对于需要真正切主体的情况：

```text
边界像素建立背景颜色模型
    ↓
OKLab 距离
    ↓
border flood fill
    ↓
Otsu 或自适应阈值
    ↓
morphology
    ↓
connected components
    ↓
fragmentation / coherence 检查
    ↓
subject mask
```

这里绝大多数机械工作交给 `imageproc`，你只负责：

- 使用什么颜色距离
- 阈值如何计算
- 哪些连通域保留
- 什么情况算分割失败
- 哪个算法分支优先

### 第三层：置信度系统

不要让算法只返回一个 mask。应该返回：

```rust
struct SegmentationResult {
    mask: Mask8,
    strategy: SegmentationStrategy,
    confidence: f32,
    diagnostics: SegmentationDiagnostics,
}
```

置信度可以综合：

```text
+ 边界颜色一致性
+ 角落对称性
+ 主体连通性
+ 主体面积处于合理范围
+ 前景与背景色差

- 边界泄漏
- 连通域过多
- 大量孤立噪点
- 主体贴满四边
- mask 过小或过大
- 结果对阈值变化极度敏感
```

有置信度之后，你才能知道哪些图标值得交给模型，哪些结果需要保留原样。

### 第四层：模型只做低置信度兜底

真到了传统算法无法覆盖的阶段，我推荐 `RTen`，而不是在原生端塞 ONNX Runtime、在浏览器端再搞另一套 Runtime。

RTen 是纯 Rust 的 ONNX 推理运行时，支持 native 和 WASM，也支持 SIMD 以及量化模型，因此更符合你同源双目标的架构。

建议结构：

```text
confidence ≥ 0.78
    → 使用传统算法结果

0.45 ≤ confidence < 0.78
    → 传统结果和模型结果进行一致性比较

confidence < 0.45
    → 运行模型兜底
```

模型只在以下时机运行一次：

```text
新 source hash
    ↓
分析并缓存
    ↓
以后调整形状、颜色、滤镜不再推理
```

绝对不要每次用户拖动调色盘都跑模型，那不是 AI，是拿电锯削铅笔。

可以拿 U²-Net 这类 salient-object 模型作为研究基线，也可以用 `rembg` 在开发阶段批量生成候选蒙版，但它们只适合当老师和对照组，不应该未经图标领域测试就成为最终裁判。

RTen 遇到不支持的 ONNX 算子时，再评估 `tract`。不要同时引入两个推理 Runtime。

---

# 三、两个值得保留为可选项的库

## `quantette`：颜色聚类和主色提取

它提供基于 Oklab 的颜色量化，包括 Wu quantizer 和可选的 k-means。它适合：

- 照片类图标的主色提取
- 生成有限调色板
- 对主题色候选做聚类
- 开发阶段验证你的 hue histogram



但我不建议一开始就让它决定你的 RIM 颜色。

你的 RIM law 明确要求：

- 只看最外层 fully-solid band
- 多数色相必须压过细高光
- lightness 决定深板还是浅板
- neutral rim 使用 neutral board
- 黄色深板有特殊 gamut 处理
- 跨图标还需要 hue spread

这不是通用调色板提取能代替的。

所以 `quantette` 的位置是：

```text
候选颜色提取器
而不是审美规则引擎
```

先不用也没问题。等你的自定义 histogram 在照片图标上暴露问题，再加。

## `resvg/usvg`：未来的 SVG 输入

`resvg` 是纯 Rust SVG 渲染器，底层同样使用 tiny-skia，并支持 WASM。对于未来的：

- SVG 图标包
- 用户导入 SVG
- 某些矢量应用资源
- AI 或设计工具导出的矢量图标

它会非常省事。

但你现在的 11 个目标形状别拿 SVG renderer 绕一圈。它们是你自己掌控的几条路径，直接 tiny-skia 最简单。

---

# 四、WASM 和 .NET 互操作也别自己硬写太多

## 浏览器侧

用：

```text
wasm-bindgen
serde-wasm-bindgen
```

`wasm-bindgen` 负责 JS 和 WASM 之间的绑定，`serde-wasm-bindgen` 可以把 Rust 数据结构转成原生 JS 值，而不是先转 JSON 字符串再解析。

推荐边界：

```text
配置、metadata、诊断信息
    → serde-wasm-bindgen

RGBA 大缓冲区
    → Uint8Array / WASM memory

Source
    → 上传一次，返回 source handle

Render
    → 只传 handle + config

结果
    → RGBA 或 PNG
```

千万不要：

```text
每次 render
→ RGBA 转数组
→ JSON.stringify
→ postMessage
→ JSON.parse
→ 再复制进 WASM
```

那等于亲手给性能挖坟。

## .NET 侧

用一个非常小的 C ABI，然后通过 `.NET LibraryImport` 调用。`LibraryImport` 会在编译期生成 marshalling 代码，是当前 .NET 原生互操作的标准方式之一。`cbindgen` 可以根据 Rust 公开 ABI 生成 C 头文件，避免 Rust 声明和 C 声明手工漂移。

我建议总共不超过十个 ABI 函数：

```c
uint32_t dm_api_version(void);

DmSession* dm_session_create(const DmSessionOptions* options);
void dm_session_destroy(DmSession* session);

DmResult dm_register_source(
    DmSession* session,
    uint64_t source_id,
    const uint8_t* rgba,
    uint32_t width,
    uint32_t height,
    uint32_t stride
);

DmResult dm_remove_source(DmSession* session, uint64_t source_id);

DmResult dm_set_desktop_context(
    DmSession* session,
    const uint8_t* data,
    size_t length
);

DmResult dm_render_batch(
    DmSession* session,
    const DmRenderRequest* request,
    DmRenderOutput* output
);

void dm_buffer_free(DmBuffer buffer);
const char* dm_last_error(DmSession* session);
```

边界规则：

- 不允许 Rust panic 跨越 FFI
- 不把 Rust `Vec` 直接交给 C# 释放
- 所有结构体带 `struct_size` 或 ABI 版本
- 大任务以 batch 为单位
- 不做回调
- C# 管理并发
- 每个 native worker 使用独立 session 或明确同步

这能避免以后 DLL 一升级，C# 莫名其妙读到错误结构体。

---

# 五、真正能替你省掉大量时间的，不只是库

## 1. 做持久化 `IconProfile` 缓存

这是我认为收益最大的优化，甚至比 WASM 和 SIMD 更重要。

你现在已经有 WeakMap memoization 和 staged caching，但它主要是当前前端会话内缓存。Rust 化后应该提升为跨进程缓存。当前规范也已经存在 source hash 和后台 ledger 的方向。

```rust
struct IconProfile {
    schema_version: u32,
    source_hash: [u8; 32],

    alpha_bbox: Rect,
    opaque_coverage: f32,
    silhouette_kind: SilhouetteKind,

    own_background: Option<DetectedBackground>,
    subject_mask: Mask8,

    subject_color_stats: ColorStats,
    rim_stats: RimStats,

    segmentation_strategy: SegmentationStrategy,
    segmentation_confidence: f32,
    diagnostics: Diagnostics,
}
```

缓存键：

```text
source_hash + analysis_schema_version
```

以后用户修改：

- 形状
- plate color
- foreground mode
- mark
- filter
- type override

都不重新做主体识别、背景识别、连通域和颜色统计。

托盘进程看到同一个 source hash，也直接复用结果。

### 简单文件布局就够了

别上数据库：

```text
%LocalAppData%/DeskMakeover/cache/profiles/
├── abcd1234.meta
├── abcd1234.mask.png
└── ...
```

写临时文件，再原子替换。缓存坏了就删掉重算。

KISS。

---

## 2. 做一个开发专用的分析检查器

这比再引入十个库更能救命。

给每张图标展示：

```text
原图
Alpha silhouette
背景 flood 区域
主体 mask
连通域
outer rim band
主题色直方图
最终 plate seed
分类结果
置信度
为什么走了这个分支
```

例如：

```text
classification: OwnBackground
confidence: 0.91

evidence:
  edge uniformity       0.94
  corner symmetry       0.97
  border leakage        0.03
  component count       1
```

这样某张文件图标识别错了，你不是盯着最终图标猜发生了什么，而是一眼看到是 corner symmetry 错了，还是 flood fill 泄漏了。

你的现有 React 应用特别适合做这个 dev panel，Rust 只需要返回 diagnostics。

---

## 3. Python 和 OpenCV 当实验室，Rust 当生产线

别把不使用 OpenCV 理解成完全不碰 OpenCV。

正确工作流是：

```text
Python/OpenCV
  快速实验算法和阈值
  批量跑真实图标
  输出候选 mask
  画 histogram 和分布图

确认算法有效
  ↓

用 imageproc + tiny-skia 实现生产版本
  ↓

把 Python 结果作为测试 oracle
```

这样你同时获得：

- OpenCV 的试验速度
- Rust 的双目标复用
- 不把 OpenCV Runtime 塞给用户
- 不必在 Rust 里慢吞吞地探索每一种算法

`rembg` 也一样。开发时批量生成候选蒙版，再人工挑错例，转成你的回归语料。不要把它直接焊进生产链路。

---

## 4. 只允许一层拥有并行权

浏览器：

```text
Worker pool
  ├ Worker 1 → 一个 WASM session
  ├ Worker 2 → 一个 WASM session
  ├ Worker 3 → 一个 WASM session
  └ Worker 4 → 一个 WASM session
```

Native：

```text
.NET Job Coordinator
  ├ session 1
  ├ session 2
  ├ session 3
  └ session 4
```

Rust core 内部第一版不要再开 Rayon。

否则会变成：

```text
.NET 并行 4 个任务
  × 每个 Rust 任务再并行 8 个线程
  = 32 个线程争抢 4 到 12 个 CPU 核心
```

这种优化通常能把电脑优化成暖手宝。

等 profiler 证明单图标内部某个算子非常重，再考虑 native 专用 Rayon。浏览器仍然由 Worker pool 并行，避免 SharedArrayBuffer 和 Wasm 内部线程复杂度。

---

## 5. 把源图规范化放在核心之外

你当前 Host 已经负责 WIC、HEIC、JPEG XR、Shell 图标提取和 cover crop。继续保持。

Rust 核心只接收：

```text
256×256
sRGB
RGBA8
straight alpha
明确 stride
```

不要在 Rust 里重新解决：

- HEIC
- JPEG XR
- Windows ICO 提取
- AppxManifest 资源选择
- Shell COM
- 色彩配置文件兼容
- Windows 缩略图

那些坑你已经通过 .NET 和 WIC 绕过去了，没必要因为用了 Rust，又傻乎乎地走回来一遍。

最理想的是 Host 先把所有来源归一化到同一种 sRGB PNG 或 RGBA，WASM 和 Native 都吃同样的输入。

---

## 6. 中间阶段也要做差分测试

不要只比最终 PNG。

迁移时每一阶段都输出 fixture：

```text
TypeScript profile
vs
Rust WASM profile
vs
Rust Native profile

TypeScript subject mask
vs
Rust WASM subject mask
vs
Rust Native subject mask

TypeScript rim stats
vs
Rust WASM rim stats
vs
Rust Native rim stats

最终 PNG
再做 SSIM / ΔE
```

只比最终图时，一张图错了，你不知道问题在：

- 分类
- 分割
- 色彩
- 形状
- 采样
- Alpha
- 合成
- 滤镜

中间结果一对比，问题会缩小到一个函数。

你已经有约 120 张合成 mock 图标、gitignored 真实图标场景以及 golden fixture 规划，这些资产非常值钱，千万不要重做测试语料。

---

## 7. 第一天就自动化许可证和供应链检查

建议加入：

```text
cargo-deny
cargo-about
Criterion
cargo-fuzz
```

- `cargo-deny`：许可证、来源、重复依赖和安全公告检查
- `cargo-about`：生成第三方许可证清单
- Criterion：native 微基准
- cargo-fuzz：图片尺寸、stride、FFI、配置和异常输入 fuzz



模型文件的许可证不属于 Cargo 依赖许可证，所以还应单独维护：

```text
THIRD_PARTY_MODELS.toml
```

记录：

```text
模型名称
来源
commit 或版本
文件哈希
代码许可证
权重许可证
训练数据说明
是否允许再分发
```

你的产品强调本地运行、无网络，所以未来模型也应该作为本地资源随应用发布，而不是运行时偷偷下载。

---

# 六、哪些东西必须自己写

社区已经完成了底层数学，但下面这些是你的产品护城河，没有现成库能正确替你做：

| 必须自己掌握 | 原因 |
|---|---|
| full-bleed、own-background、irregular 分支 | 这是图标领域分类，不是通用 CV |
| corner symmetry gate | 为拒绝折角文档等特殊设计 |
| RIM law | 完全是你的视觉规则 |
| cross-icon hue spread | 需要整个桌面上下文 |
| typeOverrides 合并 | 产品配置语义 |
| 现有 11 个形状比例 | 已经过设计验收 |
| mark 的审美参数 | Shadow、Halo、Fold 等是产品语言 |
| segmentation confidence | 决定何时信任算法、何时兜底 |
| WYSIWYG pipeline orchestration | 必须确保预览、手动 bake、托盘一致 |

库应该负责：

| 交给社区 | 推荐库 |
|---|---|
| 路径光栅化、抗锯齿、合成 | tiny-skia |
| 阈值、泛洪、形态学 | imageproc |
| 连通域、轮廓、距离变换 | imageproc |
| 颜色空间转换 | palette |
| 高质量缩放 | fast_image_resize |
| 主色聚类 | quantette，可选 |
| SVG 解析和渲染 | resvg，可选 |
| ONNX 推理 | RTen，可选 |
| JS/WASM 绑定 | wasm-bindgen |
| C ABI 头文件 | cbindgen |
| C# 调原生 | LibraryImport |

边界分得很清楚：

> 社区负责怎么计算，你负责为什么这样算。

---

# 七、明确不推荐的东西

## 不要把完整 OpenCV 放进生产核心

你只需要它几十个模块中的少量基础算子，却会连带接受：

- 更复杂的构建
- Emscripten 和原生两套适配
- Mat 生命周期
- 通用类型系统
- 更大的包体
- 更难追踪的内存和复制

OpenCV 非常适合实验室，不适合成为你这个专用 256px 图标内核的地基。

## 不要第一版引入模型

先让传统管线跑过真实语料，记录低置信度比例。

例如：

```text
真实 1000 个图标
传统算法高置信度覆盖 96%
剩余 4% 用户可以保留原样
```

这种情况下，为了 4% 引入数 MB 模型、推理 Runtime、模型许可证和测试矩阵，未必值。

只有失败样本达到足以影响产品体验的程度，才引入 RTen。

## 不要迁移壁纸模块

壁纸当前的 Pixi v8/WebGL2 compositor 已经具有同源预览和 bake，也不需要托盘后台执行。

Rust 化图标，不代表所有像素都要 Rust 化。那不是统一，是技术洁癖。

## 不要暂停 F8 去大爆炸重写

你的 Web 侧已经完成并有 297 个测试，目前离首发真正的阻塞是 F8 Windows Host、schema 3、打包和实机验证。

正确顺序是：

```text
先完成 F8
让真实产品跑通

再建立 Rust core
保持旧 TS renderer 可运行

逐层迁移
颜色和形状
→ profile
→ segmentation
→ marks 和 filters
→ 完整 compositor

每一步做差分
最后切 WASM
再接 Native DLL
最后删除旧 TS/C# renderer
```

否则你会在产品还没有真实跑通过一次的时候，同时调：

- Windows Host
- WebView2 bridge
- Rust
- WASM
- C ABI
- C#
- 分割算法
- 发布打包

那不叫勇敢，那叫主动制造无法定位的问题。

---

# 最终采用清单

## 立即引入

```text
tiny-skia
imageproc
palette
fast_image_resize

serde
wasm-bindgen
serde-wasm-bindgen

cbindgen
.NET LibraryImport
```

## 暂缓，等真实数据证明需要

```text
quantette
resvg/usvg
RTen
tract
```

## 开发环境使用，不进入核心发布链路

```text
Python
OpenCV
rembg
Criterion
cargo-fuzz
cargo-deny
cargo-about
```

最终你大概可以让社区承担大部分底层像素工程，而把自己的代码集中在少量真正有价值的地方：

```text
IconProfile
图标领域分类
主体策略编排
RIM 视觉法则
桌面级配色
类型差异
最终组合语义
```

最蠢的不是自己写算法。

最蠢的是别人已经把锤子、锯子和水泥都造好了，你却重新炼铁。另一种同样蠢的做法，是别人卖了一整座工厂，你只需要钉一颗钉子，却把整座工厂搬回家。

**用社区的砖，保留自己的图纸。**

## User

对了，我想做的这些功能应用，可以切换到 Tauri 技术吗？.NET 是必要的吗？如果用 Tauri 加 Rust，Rust 能帮我设置或获取桌面图标，或者去掉快捷方式的角标吗？设置和获取壁纸这些 Rust 也能做吗？还是需要 .NET 参与？如果需要，.NET 参与的部分有多大？你觉得是更适合用 Tauri，还是按照之前说的架构——以 .NET 为主体，Rust 只负责算法，View 负责界面——哪个方案更优？

## Assistant

I’ll evaluate UI hosting, Rust’s Windows Shell coverage, and the real maintenance cost of Tauri versus a .NET-centered architecture.

## Assistant

A key finding is that Rust can replace nearly all .NET platform code; the real issue is rewrite value, while Windows lacks a supported per-shortcut arrow-removal API.

## Assistant

按你 **2026 年 7 月 10 日**的项目现状，我的拍板是：

> **技术上完全可以切到 Tauri + Rust，.NET 的必要参与是 0。**
> **但对你这个已经做到当前阶段的项目，我仍然选择 .NET 宿主 + Rust 核心 + React/Pixi 界面。**

从空仓库重新开始，我会认真考虑 Tauri。可你不是空仓库：Web 侧已经完成并有 297 个测试，C# 侧已经存在 Shell、Operations、ICO、恢复、ElevatedHelper 等模块和约 277 个旧测试，现在剩的是 F8 宿主接线、打包与真实环境验证。此时换 Tauri，不是换个壳，是重写整个风险最高的 Windows 控制层。

---

# 一、Tauri 加 Rust 能不能独立完成这些功能

能。

Tauri 在 Windows 上依然使用 WebView2，可以继续使用你现在的 React、Vite、Bun、Pixi 和 Web Worker；它有 Rust command、系统托盘、自定义标题栏、Windows installer 和本地资源协议。换句话说，界面基本可以保留，主要重写的是宿主。

微软官方的 `windows-rs` 能把 Win32、COM 和 WinRT API 投射到 Rust，因此语言能力层面，Rust 和 C# 最终都能调用同一套 Windows API。

具体来看：

| 功能 | Rust 实现路径 | 是否需要 .NET |
|---|---|---:|
| 托盘常驻、隐藏与恢复窗口 | Tauri Tray + Rust 后台状态 | 否 |
| 枚举桌面文件与系统对象 | `SHGetDesktopFolder`、`IShellFolder::EnumObjects` | 否 |
| 获取桌面图标位置 | `IFolderView::GetItemPosition` | 否 |
| 提取文件、快捷方式、系统对象图标 | `IShellItemImageFactory::GetImage`、`SHGetFileInfo` | 否 |
| 获取或修改 `.lnk` 图标 | `IShellLink::GetIconLocation/SetIconLocation` + `IPersistFile` | 否 |
| UWP 桌面快捷方式 | 桌面项仍是 `.lnk`，包资源解析走 WinRT 或 Manifest | 否 |
| 修改文件夹图标 | `desktop.ini` + 文件属性 + `SHChangeNotify` | 否 |
| 修改回收站、此电脑等系统图标 | 注册表 CLSID `DefaultIcon` + Shell 刷新 | 否，但兼容性敏感 |
| 获取和设置壁纸 | `IDesktopWallpaper::GetWallpaper/SetWallpaper` | 否 |
| 多显示器壁纸 | `IDesktopWallpaper` 的 monitor ID 系列接口 | 否 |
| WIC 解码、HEIC/JPEG XR、缩放 | 通过 `windows-rs` 调 WIC | 否 |
| 监听新桌面项目 | `notify` 或 `ReadDirectoryChangesW` + 定期校对 | 否 |
| UAC 提权 | 独立 Rust helper EXE + `runas` 或管理员 manifest | 否 |
| 快照、历史、事务和恢复 | Rust 文件系统与序列化 | 否 |

桌面 Shell 命名空间、内容枚举和图标位置都有公开 COM 接口；图标提取也有 `IShellItemImageFactory` 和 `SHGetFileInfo`。

修改快捷方式图标同样是标准接口：`IShellLink::SetIconLocation` 修改图标来源，随后通过 `IPersistFile` 保存 `.lnk`。你当前规格里的 AppxShortcut 也是按普通 `.lnk` 修改，并保存完整原始字节以便恢复。

文件夹图标可以用微软公开说明的 `desktop.ini` 机制，变更后通过 `SHChangeNotify` 通知 Shell。

壁纸则更直接。`IDesktopWallpaper` 原生提供获取、设置、显示方式、显示器枚举和幻灯片状态等能力。Rust 只是调用它，不需要 .NET 中转。

所以从纯技术能力来说：

> **.NET 没有任何不可替代的魔法。它和 Rust 都是在开同一扇 Win32 大门。**

---

# 二、但有一个语言无关的硬限制：快捷方式箭头

这里把你说的角标理解为 Windows 快捷方式左下角的系统箭头。

微软文档明确描述了它的工作方式：

1. Shell 先取得快捷方式的基础图标。
2. 然后额外叠加一个系统定义的 link overlay。
3. `IShellLink::SetIconLocation` 只负责基础图标的位置，不控制 overlay。

因此：

> **没有公开、可靠的逐快捷方式 API，可以让 A 快捷方式有箭头、B 快捷方式没有箭头。**

这不是 Rust 做不到、C# 能做到，而是 Windows 就没给你这个粒度的正式接口。

网上常见的做法是：

- 修改全局 Shell overlay 图标
- 将 overlay 29 替换成透明图标
- 删除或修改 `IsShortcut`
- 注册 Shell overlay handler
- 强制刷新或重启 Explorer

这些方案本质上都是全局操作，而且不同 Windows 版本和更新后的表现可能变化。有些还会影响固定、跳转列表、快捷方式识别或者第三方 Shell 扩展。我不建议把它当成普通、稳定、逐图标功能。

## 这会撞上你当前规格里的一个真实矛盾

你的规格要求：

- 美化后的快捷方式可以有自定义标识
- 可以选择无标识
- 保留原样的快捷方式显示经典 Windows 箭头



假设你为了实现无标识而全局关掉 Windows 箭头，那么保留原样的快捷方式也会一起失去箭头。

除非你连保留原样的快捷方式也重新生成一份图标，把经典箭头烘焙进去。但这样它就不再是真的保留原样了，只是视觉上模拟原样。

这件事最好现在就把产品语义钉死。我建议采用安全方案：

### 推荐方案

**始终保留 Windows 系统箭头。**

你的投影、光环、缎光、描边等效果继续作为额外视觉语言，但不要承诺移除系统箭头。把无标识改成：

```text
不添加效果
```

而不是：

```text
移除 Windows 箭头
```

你的那些标识大部分是轮廓、阴影、卷角和光效，并不一定要占据左下角，因此可以与 Windows 箭头共存。

### 激进方案

提供一个明确标注为兼容性功能的全局开关：

```text
隐藏 Windows 快捷方式箭头
```

开启时：

- 先保存相关注册表完整快照
- 全局隐藏系统箭头
- 给所有参加美化的快捷方式烘焙自定义标识
- 给需要经典箭头的快捷方式烘焙一枚模拟箭头
- 每次写入历史
- 完整关闭时恢复注册表和全部快捷方式
- Windows 大版本更新后重新验证

但不要伪装成逐图标能力。

---

# 三、普通文件也有一个类似限制

`.lnk` 有自己的图标位置。

文件夹有 `desktop.ini`。

但是任意普通文件，比如：

```text
report.docx
photo.jpg
archive.zip
```

没有一个通用的逐文件图标属性。通常只能：

1. 修改整个文件类型的图标，例如所有 `.docx`
2. 创建一个指向原文件的快捷方式或包装项
3. 写 Shell extension
4. 不修改

你的路线图已经明确永久禁止全局文件类型关联图标，因此普通文件要参与美化，只能继续走 wrapper 或快捷方式替代机制，并严格保存位置、名称和恢复信息。

Rust 可以实现这个机制，C# 也可以。还是那句话，语言不是障碍，Windows 对象模型才是边界。

---

# 四、纯 Tauri 方案应该怎么设计

真切到 Tauri，我建议这么做：

```text
DeskMakeover
├── frontend
│   ├── React
│   ├── Pixi 壁纸编辑器
│   └── dm-core.wasm
│       └── Web Worker 中实时渲染图标
│
├── crates
│   ├── dm-core
│   │   └── 图标分析、分割、配色、合成
│   │
│   ├── dm-wasm
│   │   └── WASM 适配
│   │
│   ├── dm-windows
│   │   ├── shell_sta.rs
│   │   ├── desktop_scan.rs
│   │   ├── icon_extract.rs
│   │   ├── shortcut.rs
│   │   ├── folder_icon.rs
│   │   ├── system_icon.rs
│   │   ├── wallpaper.rs
│   │   ├── watcher.rs
│   │   ├── journal.rs
│   │   └── restore.rs
│   │
│   └── dm-app
│       └── Tauri commands、tray、window、settings
│
└── helper
    └── 独立的 requireAdministrator Rust EXE
```

## `dm-core` 仍然编译两遍

```text
dm-core → WASM
  前端实时预览和手动 bake

dm-core → Native Rust
  托盘后台自动处理
```

在 Tauri 后端中，Native Rust 可以直接调用 `dm-core`，不需要 DLL、C ABI 或 `LibraryImport`。这是 Tauri 最大的架构优势。

## Windows Shell 操作放进一个 STA Actor

你当前 C# 规格已经要求 COM/STA 工作封送到 STA 线程。切 Rust 后这条规则不能丢。

不要假定某个 `#[tauri::command]` 执行线程天然满足 Shell COM 要求。正确方式是创建一个专用线程：

```text
ShellStaActor
  ├ CoInitializeEx(COINIT_APARTMENTTHREADED)
  ├ Windows message loop
  ├ IShellLink
  ├ IShellFolder
  ├ IFolderView
  ├ IDesktopWallpaper
  ├ WIC
  └ channel 收发任务
```

COM 的 STA 模型依赖线程归属和消息分派，不能随便把 COM 对象跨 Tokio worker 线程移动。

例如：

```rust
enum ShellCommand {
    ScanDesktop,
    ExtractIcon { item_id: ItemId },
    ApplyShortcutIcon { path: PathBuf, ico: PathBuf },
    SetWallpaper { path: PathBuf },
    Restore { snapshot_id: SnapshotId },
}
```

Tauri command 只向 Actor 发请求，不直接到处写 `unsafe` COM 调用。

## 大图片不要走普通 command JSON

保持你当前 bridge 的正确设计：

```text
小数据
  配置、状态、任务、结果
  → Tauri command

大数据
  原始图标、壁纸、bake 文件
  → asset URL 或文件路径
```

Tauri 可以通过受权限范围限制的 asset protocol 把本地文件暴露给 WebView。它的事件系统也明确更适合少量数据，而不是持续传输大块像素。

也就是说，你现在的 `sourceUrls` 思路可以原封不动迁移：

```text
Rust 提取 icon-source.png
        ↓
asset://...
        ↓
React / WASM 加载一次
```

---

# 五、Tauri 与 .NET 主体的真正比较

| 维度 | Tauri + Rust | .NET Host + Rust Core |
|---|---|---|
| 图像算法性能 | 几乎相同 | 几乎相同 |
| 后台原生渲染 | Rust 直接调用 | C# 通过一次批量 FFI |
| 算法复用率 | 最高 | 几乎同样高 |
| Windows Shell 开发体验 | 更底层，更多 `unsafe` 和 COM 生命周期 | 更成熟，现有代码已经在 |
| 前端复用 | 基本完整 | 已经完整 |
| WebView 运行时 | WebView2 | WebView2 |
| 托盘 | Tauri 原生支持 | .NET 也容易实现 |
| 包体和运行时 | Tauri 通常更克制 | 自包含 .NET 发布较大 |
| 当前迁移成本 | 很高 | 最低 |
| 已有 Windows 测试复用 | 大量重写 | 直接保留 |
| UAC、恢复、Shell 风险 | 全部重新证明 | 沿用现有边界 |
| 长期语言数量 | TS + Rust | TS + Rust + 少量 C# |
| 当前项目综合结果 | 次优 | **最优** |

Tauri 在 Windows 上仍然是 WebView2，所以它不会让 WebView2 进程和网页内存消失；它的轻量主要来自不再随应用携带另一个托管运行时。

性能上也不要被少一层 FFI 迷惑。

只要 .NET 调 Rust 时采用：

```text
一次调用处理一批图标
```

而不是：

```text
每个像素或每个算子调用一次
```

那点 P/Invoke 成本和图像分割、缩放、PNG、ICO、磁盘写入、Explorer 刷新相比，基本没有产品意义。

Tauri 确实可以省掉 C ABI，但你为此要重写：

- 桌面 Shell 扫描
- COM/STA 调度
- 快捷方式读取和保存
- WIC 解码
- ICO 输出
- 注册表操作
- Explorer 刷新
- 事务和快照
- 恢复
- ElevatedHelper
- 打包
- 原有 .NET 测试

而你当前的 F8 虽然还没完成 schema 3 接线，但底层 C# 平台模块并不是不存在。你的文档已经列出了 `DeskMakeover.Shell`、`DeskMakeover.Operations`、`DeskMakeover.IconRendering` 和 `DeskMakeover.ElevatedHelper`，C# 侧也已有大量测试。

换句话说：

> F8 是把已经存在的手接到新的大脑上。
> 切 Tauri 是把整条胳膊锯掉，再用 Rust 重新长一条。

---

# 六、我建议的最终架构

```text
React + Pixi
  负责界面、交互、状态、壁纸实时编辑

Rust dm-core
  ├ WASM：图标实时预览、手动 bake
  └ Native DLL：托盘后台渲染

.NET Windows Host
  ├ WPF + WebView2
  ├ Bridge
  ├ Shell STA
  ├ 桌面扫描与图标提取
  ├ 快捷方式、文件夹、系统图标应用
  ├ WIC 与 ICO 封装
  ├ 壁纸获取、应用与恢复
  ├ 快照、journal、历史
  ├ 托盘与 watcher
  └ ElevatedHelper
```

这里的 .NET 不是主体业务，更不是渲染引擎。

它应该被压缩成：

> **Windows 平台驱动层。**

合理目标是：

- **0% 的审美算法留在 .NET**
- **0% 的主体识别留在 .NET**
- **0% 的颜色、形状、滤镜逻辑留在 .NET**
- .NET 只拥有 Windows API、事务和恢复
- Rust ABI 保持 8 到 10 个粗粒度函数
- 整个 .NET 层代码量最终尽量控制在仓库的三分之一以内

但代码量不是最关键的。

.NET 可能只占二三成代码，却负责七八成系统风险。因为用户不关心你用了几种语言，用户只关心重启之后桌面还能不能完整还原。

---

# 七、最终判断

## 从零开始的新项目

我会选：

```text
Tauri + React/Pixi + Rust Core + windows-rs
```

理由是：

- 后端只有 Rust
- Native 与 WASM 直接共享 crate
- 没有 C ABI
- 托盘天然
- 包装更统一
- 长期结构更漂亮

## 你当前这个项目

我会选：

```text
.NET Windows Host + Rust Core + React/Pixi View
```

理由是：

- 性能几乎没有损失
- 算法复用率仍然接近完整
- 现有 Windows 平台代码和测试全部保留
- 你的产品又是纯 Windows，不会享受到多少跨平台收益
- 不需要重写最危险的应用、恢复、UAC 和 Shell 部分
- 可以更快进入真实机器验证，而不是再次重构宿主

另外，**不要采用 Tauri + .NET sidecar** 作为长期架构。那会同时保留 Tauri、Rust、.NET、WebView2 和进程通信，成功集齐两套方案的复杂度，却没有完整获得任何一套方案的简洁。

最后就一句：

> **Tauri 能替代 .NET，但现在替代它不划算。Rust 应该吃掉计算，.NET 应该守住 Windows。技术统一是手段，不是信仰；为了少一种语言重写一套已经验证过的恢复系统，是架构洁癖，不是工程理性。**

## User

原来用 .NET 和 AI 写的代码里，单独实现了一个 UAC 提权模块，因为有些操作需要权限。我还去淘宝买了一个软件签名证书，这个证书还用得上吗？另外，UAC 提权原本是 .NET 写的，现在我切换到了 TAU I 等技术栈，新的方案能提权吗？也就是说，我不再用原来的提权方式，新的能实现提权吗？还有，我讨厌原生的小箭头角标，希望把它隐藏掉（设为透明），整个图标，包括新的区别快捷方式，全部都是重新绘制的。我只保留一个进程在后台，监听 events，监听用户在桌面创建的新快捷方式或图标，然后增量更新，按照他的设定进行 format，这不就行了吗？请帮我检查一下这些内容，看看还有什么需要补充的？

## Assistant

I’ll verify certificate reuse, Tauri/Rust elevation, global shortcut-arrow suppression, and whether the single-process incremental watcher has adequate safety and recovery mechanisms.

## Assistant

The certificate remains usable, and Tauri/Rust can elevate via a signed temporary helper. Arrow suppression is system-wide, and elevation briefly requires a second process.

## Assistant

## 先拍板

你的思路基本成立，但要把几个概念分开：

1. **软件签名证书继续有用，而且提权助手比主程序更需要签名。**
2. **切到 Tauri + Rust 后，.NET 可以完全退出，UAC 提权照样能做。**
3. **可以只保留一个长期常驻的 Rust 主进程，但提权时必须临时启动第二个管理员进程。**
4. **全局隐藏 Windows 原生小箭头，再把快捷方式标识烘焙进你生成的 ICO，这套视觉模型是闭环的。**
5. **监听事件后增量处理是主路径，但监听事件不能当真相，必须加扫描校对、状态账本和恢复机制。**

最终结构应该是：

```text
DeskMakeover.exe
签名 · asInvoker · 唯一长期常驻进程
├── Tauri 托盘
├── 单实例
├── 开机启动
├── 桌面监听
├── 增量校对
├── Rust 原生图标算法
├── Shell COM / 注册表 / 壁纸
├── 历史与恢复
└── 按需创建 WebView 界面
          │
          │ 只有需要管理员权限时
          │ ShellExecuteExW + runas
          ▼
DeskMakeover.Elevated.exe
签名 · requireAdministrator · 临时进程
├── 固定白名单操作
├── 修改机器级快捷方式箭头设置
├── 修改公共桌面或机器级系统图标
└── 完成后立刻退出
```

**这里不再需要任何 .NET。**

---

# 一、你买的软件签名证书继续有用

代码签名证书签的是 Windows 可执行文件和安装包，不是签 .NET。

所以无论你的程序是：

- C#
- Rust
- C++
- Tauri
- 原生 Win32

只要最终产物是 Windows 的 EXE、DLL、MSI 或安装器，就可以继续使用 Authenticode 代码签名。Tauri 本身也提供 Windows 签名配置，可以通过证书存储、PFX 或云签名命令完成签名。

你应该用同一个发布者身份签署：

```text
DeskMakeover.exe
DeskMakeover.Elevated.exe
安装程序
卸载程序
紧急恢复程序
所有原生 DLL
所有会被执行的 sidecar
```

签名对提权助手尤其重要。Windows 弹出 UAC 时会检查可执行文件的发布者，已验证签名的程序显示已验证发布者，未签名或者不可信的程序会显示未知发布者并使用更具警告意味的提示。

你的路线图本来就把签名后的 EXE 和 SmartScreen 验证列为公开发布门槛，这条不要删，只需要把原来的 C# ElevatedHelper 替换成 Rust ElevatedHelper。

## 证书必须带时间戳

每次签名都要同时使用 RFC 3161 时间戳，并使用 SHA-256。

时间戳的价值是：证书以后过期了，用户仍然可以验证这个文件是在证书有效期内签署的。没有时间戳，证书一过期，旧版本的签名体验也可能变差。

推荐发布流水线：

```text
构建 Elevated.exe
→ 签名并加时间戳
→ 验证签名

构建 DeskMakeover.exe
→ 签名并加时间戳
→ 验证签名

打包安装程序
→ 再签安装程序并加时间戳
→ 在干净虚拟机验证
```

注意签名后不要再修改二进制，否则签名会失效。

## 淘宝买的证书要认真检查一次

淘宝只是购买渠道，不能单凭渠道判断证书真假。关键看四件事：

### 1. 发布者是不是你

签名详情中的 Subject 应该是：

- 你的真实姓名
- 你的公司主体
- 你认可的发布主体

不能是淘宝商家，也不能是一家完全不认识的公司。

否则 UAC 弹窗显示的是卖家的名字，用户看到的不是桌面美颜的真实发布者。

### 2. 私钥是否只由你控制

最危险的情况是：

```text
很多客户共用一张证书
或者
你把软件发给卖家，由卖家代签
```

这种证书即使技术上有效，也不适合作为正式产品的长期身份。

别人用同一张证书签了恶意软件，可能连累证书信誉，甚至导致证书被吊销。

### 3. 2023 年以后签发却给了普通可复制 PFX，要高度警惕

CA/Browser Forum 从 **2023 年 6 月 1 日**起要求公开信任的代码签名私钥在合格硬件密码模块或者符合要求的云 HSM 中生成、保存和使用。若你的证书是在此日期以后签发，而商家只是给了你一个可以随便复制、导出的含私钥 PFX，需要向真正的 CA 核实来源和合规性。

正常形态通常会是：

- USB 硬件 Token
- 云签名服务
- Azure Key Vault 一类 HSM 服务
- 其他受保护的签名设备

### 4. 不要相信 EV 证书天然免 SmartScreen

微软目前明确说明，新签名的二进制即使使用有效 OV 或 EV 证书，也仍可能在早期出现 SmartScreen 未知应用提示，直到文件哈希或发布者积累足够信誉。EV 已经不再自动绕过 SmartScreen。稳定地使用同一个发布者身份签署所有版本，才有利于信誉延续。

你可以先执行：

```powershell
Get-PfxCertificate .\codesign.pfx |
  Format-List Subject, Issuer, NotBefore, NotAfter, Thumbprint, EnhancedKeyUsageList

Get-AuthenticodeSignature .\DeskMakeover.exe |
  Format-List Status, StatusMessage, SignerCertificate

signtool verify /pa /all /v .\DeskMakeover.exe
```

使用硬件 Token 或云签名时不一定存在可导出的 PFX，但最终的 `signtool verify` 仍然应该通过。

另外，Tauri Updater 使用的是另一套更新包签名密钥，和 Windows Authenticode 证书不是一回事。开启 Tauri 自动更新后，两套都需要：

```text
Windows Authenticode
验证 EXE、helper、安装包的发布者

Tauri updater signature
验证下载到的更新包没有被替换
```

Tauri 的更新签名不能关闭。

---

# 二、Tauri 加 Rust 完全可以重新实现 UAC 提权

不但可以，而且原来的安全架构思路应该保留。

你的产品规格本来就要求：

- 主程序不持续提权
- 先解释
- 一次 UAC 批量完成
- 用户拒绝后继续处理不需要权限的部分
- 提权助手是独立 EXE
- 助手只能执行固定白名单动作

这个设计非常正确。

只是实现语言从：

```text
C# 主程序
→ C# ElevatedHelper
```

变成：

```text
Rust/Tauri 主程序
→ Rust ElevatedHelper
```

## 不要把整个 Tauri 主程序设为管理员运行

Tauri 确实支持给程序嵌入：

```xml
<requestedExecutionLevel
  level="requireAdministrator"
  uiAccess="false" />
```

这样每次启动都会要求 UAC。

但主程序不应该这么做。

主程序应该继续：

```text
asInvoker
普通用户权限
长期驻留
```

只有真正需要权限的操作才启动 helper。

Windows 的 `ShellExecuteExW` 支持 `runas` verb。使用它启动 helper 时，Windows 会显示 UAC 同意框，标准用户也可以输入管理员账户凭据。

## Rust 提权助手建议这样实现

```text
crates/
├── dm-core
├── dm-windows
├── dm-app
└── dm-elevated
```

`dm-elevated` 是普通 Rust Windows EXE，不需要带 Tauri，也不需要 WebView。

它只链接：

```text
windows-rs
serde
日志和错误处理
共享的 PrivilegedOperation DTO
```

Tauri 可以通过 `externalBin` 将外部二进制打进安装包，但普通 sidecar spawn 本身不等于提权。提权启动仍然应该从 Rust 后端调用 `ShellExecuteExW` 的 `runas`，不要让前端 JavaScript 直接拥有任意进程启动权限。

## helper 只允许固定操作

绝对不要设计这种接口：

```text
runElevated(command: string)
runPowerShell(script: string)
writeRegistry(path, name, value)
```

这相当于给 WebView 和任何潜在前端漏洞一把管理员万能钥匙。

应该是：

```rust
enum PrivilegedOperation {
    InstallTransparentShortcutOverlay,
    RestoreShortcutOverlay,
    ApplyPublicDesktopIcons,
    RestorePublicDesktopItems,
    ApplyMachineSystemIcons,
    RestoreMachineSystemIcons,
}
```

helper 内部自己决定：

- 可以修改哪些注册表项
- 可以访问哪些目录
- 允许处理多少文件
- 文件必须是什么扩展名
- 路径是否位于允许范围
- 请求版本是否兼容

前端只应该表达用户意图，不能传任意注册表路径、PowerShell 或命令行。

## helper 必须安装在受保护目录

Tauri 的 NSIS 安装默认是 `currentUser`，也就是当前用户安装模式。它也支持 all-users 安装模式。

对你这个带管理员 helper 的产品，我建议：

```text
程序和 helper
→ Program Files\DeskMakeover

机器级透明箭头文件
→ ProgramData\DeskMakeover\Shell

用户数据、缓存、历史
→ LocalAppData\DeskMakeover
```

不要把即将以管理员权限运行的 helper 放在普通用户可以随意覆盖的 LocalAppData 目录里。否则恶意程序可能先替换 helper，再诱导用户同意你的 UAC 提示。

主 EXE、helper、安装程序应使用同一发布者证书签名。

## 标准用户有一个容易踩的坑

当标准用户在 UAC 中输入另一个管理员账户的密码时，helper 可能运行在那个管理员账户身份下。

因此 helper 里不要依赖：

```text
当前进程的 HKCU
当前进程的 USERPROFILE
当前进程的 Desktop
```

它们可能属于管理员账户，而不是正在使用桌面的原用户。

正确边界是：

```text
主程序处理
当前用户 HKCU
当前用户桌面
当前用户壁纸
当前用户 LocalAppData

helper 处理
HKLM
Program Files
ProgramData
Public Desktop
真正的机器级系统操作
```

需要处理原用户对象时，由主程序传入明确、经过验证的路径或用户 SID，helper 不能自己猜。

---

# 三、只保留一个后台进程是可以的，但要换个准确说法

你可以做到：

> **只有一个长期常驻进程。**

但不能承诺：

> **系统中任何时候都只有一个进程。**

因为：

- 提权时会短暂启动 `DeskMakeover.Elevated.exe`
- 打开 Tauri 界面时会使用系统 WebView
- 安装、更新和卸载时也会启动对应进程

Tauri 本来就支持托盘、单实例和开机启动。你可以让程序以 `--background` 模式启动，不创建 WebView 窗口，只启动托盘、监听器和 Rust 后台；用户点击托盘后再创建界面窗口，关闭界面时销毁窗口，Rust 主进程继续运行。

所以产品层应该描述成：

```text
一个后台常驻程序
需要提权时临时启动安全助手
```

而不是纠结任务管理器里永远只能看到一个 PID。

---

# 四、你隐藏原生箭头，再自绘所有快捷方式标识，逻辑是成立的

Windows 快捷方式的原生箭头不是 ICO 文件本身的一部分。

Windows 会先读取快捷方式的基础图标，然后再由 Shell 叠加系统定义的 link overlay。`IShellLink::SetIconLocation` 只能设置基础图标，不能关闭这个 overlay。微软公开文档还说明，`IsShortcut` 标记会让 Shell 自动附加箭头，而且更改相关设置后需要重新启动 Shell 才会生效。

因此你现在的模型应该是：

```text
Windows 原生快捷方式箭头
→ 全局变透明

所有桌面快捷方式
→ 都生成新的 ICO

快捷方式区别标识
→ 直接烘焙进 ICO
```

这在视觉上是最干净的。

你的标识状态可以正式定义为：

| 状态 | 最终行为 |
|---|---|
| 经典箭头 | 烘焙你自己绘制的经典箭头 |
| 投影、光环、卷角等 | 烘焙对应自定义标识 |
| 无标识 | 不烘焙任何标识 |
| 保留原样 | 保留原始主体像素，但仍按用户设置烘焙快捷方式标识 |

这样预览、手动应用和后台自动处理都只认你的渲染结果，不再依赖 Windows 原生箭头。

## 但透明箭头必须被定义为全局兼容功能

我没有找到微软公开支持的逐快捷方式隐藏系统 overlay API。公开接口只说明系统叠加箭头以及快捷方式自己的基础图标。因此，把系统 overlay 替换成透明资源应该明确视为一个**兼容性方案**，不能包装成稳定的逐图标 API。

它的影响范围也不是只针对你处理过的桌面快捷方式。

至少要向用户说明：

```text
开启后，Windows 的原生快捷方式箭头会在当前 Shell 范围内隐藏。
桌面上的快捷方式由桌面美颜重新绘制标识。
其他文件夹中的快捷方式也可能不再显示 Windows 原生箭头。
```

若使用机器级注册表设置，还可能影响这台电脑上的其他用户。

这意味着：

- 桌面上的快捷方式会很好看，因为你全部重新绘制
- 桌面之外未经过你处理的快捷方式，可能完全没有箭头
- 应用关闭期间新出现的快捷方式，在启动补处理前可能暂时没有标识
- 公共桌面快捷方式如果需要 UAC，会在用户批准前暂时没有自绘标识

这是你必须接受的产品取舍。

## 不要删除 `IsShortcut`

不要通过删除注册表中的 `IsShortcut` 来去掉箭头。

它不只是一个图像选项，而是 Shell 识别快捷方式类型的标记之一。你的策略应该是：

```text
保留快捷方式语义
保留 IsShortcut
仅让系统 overlay 使用透明图像
```

## 透明 overlay 本身也要进入恢复系统

建议机器级资源使用：

```text
%ProgramData%\DeskMakeover\Shell\transparent-overlay.ico
```

这个 ICO 应包含完整尺寸和真正透明的 32 位位图，例如：

```text
16
20
24
32
48
256
```

第一次修改前，记录：

```text
原注册表项是否存在
原值是否存在
原值类型
原始数据
使用的透明 ICO 路径
修改时间
应用版本
```

关闭功能、卸载程序、紧急恢复时，必须精确恢复原状态，而不是简单删除一个固定值。

更改 overlay 后需要提供受控的 Explorer 刷新或重启流程。官方文档明确说相关 overlay 变化需要重启 Shell 才会生效，所以不要假装一次 `SHChangeNotify` 一定足够，也不要静默杀掉 Explorer。

Windows 更新后还要执行健康检查：

```text
设置仍然生效
→ 什么都不做

设置被系统恢复
→ 显示待修复状态

用户明确允许自动维护
→ 再重新应用
```

不要每次启动都偷偷和 Windows 对着干。

---

# 五、你的现有规格里有三处需要同步修改

当前文档仍然规定：

- 保留原样的快捷方式显示 Windows 经典箭头
- 经典箭头是系统原生箭头
- C# 负责未来的后台自动美化



切换后应该修改为：

### 1. 经典箭头变成自绘样式

```text
旧：
经典箭头 = Windows overlay

新：
经典箭头 = dm-core 烘焙的 mark style
```

### 2. 保留原样变成保留主体，而不是保留最终系统像素

```text
保留原样
= 原始主体不改色、不改形
+ 根据快捷方式设置烘焙标识
```

否则全局箭头隐藏后，保留原样的快捷方式就完全没有类型区别了。

### 3. 后台渲染由 native Rust 负责

```text
旧：
C# TileRenderer

新：
Rust dm-core native
```

WASM 只用于前端交互预览，后台常驻进程直接调用 native Rust，不需要启动 WebView。

---

# 六、监听事件加增量更新是对的，但事件只能是提示

你说的：

```text
监听用户在桌面创建的新快捷方式或图标
→ 增量更新
→ 按用户设置格式化
```

这是正确的主流程。

但真正可靠的实现必须是：

```text
收到变化提示
→ 合并事件
→ 等待文件稳定
→ 重新扫描变化范围
→ 对比 ledger
→ 提取源图标
→ Rust 渲染
→ 事务式应用
→ 刷新 Shell
→ 再次验证
```

而不是：

```text
收到 Created
→ 立即修改这个文件
```

Windows 的目录变化缓冲区可能溢出；一旦溢出，详细事件会被丢弃，微软要求调用方重新枚举目录计算变化。所以监听器永远不能作为唯一真相。

## 建议同时使用四个变化来源

### 1. 用户桌面目录

使用 `SHGetKnownFolderPath` 获取，不要硬编码：

```text
C:\Users\xxx\Desktop
```

用户桌面可能被 OneDrive、组策略或其他机制重定向。官方将 `FOLDERID_Desktop` 定义为每用户 Desktop。

### 2. 公共桌面目录

`FOLDERID_PublicDesktop` 是 COMMON 文件夹，默认位于 `%PUBLIC%\Desktop`。它与用户桌面不是一回事。

### 3. Shell 级通知

回收站、此电脑、网络等虚拟桌面项目不一定对应一个普通桌面文件。`SHChangeNotifyRegister` 可以接收文件系统级和 Shell 级通知。

### 4. 启动和恢复后的完整校对

以下场景必须全量或半全量校对：

```text
应用刚启动
电脑从睡眠恢复
监听器报告溢出
Explorer 重启
应用升级
Windows 大版本更新
应用异常退出后恢复
```

你的规格里原本就已经规定了用户桌面加公共桌面、默认 4 秒 debounce、source hash、增量 ledger、首次提案和可撤销历史。这个方向是对的，迁移到 Rust 后继续保留。

---

# 七、监听器还要补上这些关键机制

## 1. 监听的不只是 Created

要处理：

```text
Created
Renamed
Changed
Deleted
```

很多安装器会：

```text
创建临时文件
→ 写入
→ 重命名成最终 .lnk
→ 再修改属性
```

单看 Created 很容易处理到半成品。

## 2. 等待文件稳定

处理 `.lnk` 前至少检查：

```text
文件可以正常打开
长度和修改时间连续两次不变
IShellLink 可以正常解析
目标和 IconLocation 已经写完
```

单个新快捷方式可以使用较短稳定窗口，安装器批量创建时再使用约 4 秒合并窗口。

## 3. 防止处理自己的写入

你的程序修改 `.lnk` 后，监听器也会收到 Changed。

必须通过下面的信息识别自己的操作：

```text
operationId
修改前 source hash
应用后的 expected hash
时间窗口
目标路径
```

否则会产生：

```text
修改快捷方式
→ 监听到修改
→ 再次渲染
→ 再次修改
→ 无限循环
```

## 4. source hash 不能只算 `.lnk` 文件

快捷方式文件没变，不代表源图标没变。

例如目标应用更新后：

```text
foo.lnk 不变
foo.exe 的图标变了
```

建议 source fingerprint 包含：

```text
.lnk 内容或关键字段
目标路径
目标文件版本或修改时间
IconLocation
IconLocation 对应文件状态
AUMID / PackageFamilyName
包版本
选中的资源变体
```

应用启动时和定期校对时检查这些指纹。

## 5. generated ICO 使用内容寻址路径

不要永远覆盖同一个：

```text
foo.ico
```

建议：

```text
icons/
  <item-id>/
    <source-hash>-<style-hash>.ico
```

应用新版本时先写新文件，再更新快捷方式 IconLocation。

这样可以：

- 避免读取到半写文件
- 降低旧图标缓存干扰
- 更容易回滚
- 历史版本可以直接引用旧产物
- 确认不再被使用后再垃圾回收

## 6. 用户手工修改优先

如果当前状态和你的最后一次 applied hash 不一致，不能直接覆盖。

应该标记：

```text
用户或其他程序已修改
```

然后：

- 默认不覆盖
- 在应用中显示冲突
- 让用户选择重新接管或保留当前状态

否则你的软件会像个控制狂，用户每次手工改完，它立刻改回去。

---

# 八、公共桌面是你现在最容易漏掉的大坑

公共桌面的快捷方式对所有用户共享。

假设你这样做：

```text
修改 Public Desktop\App.lnk

IconLocation 指向：
C:\Users\小明\AppData\Local\DeskMakeover\icons\app.ico
```

那么其他 Windows 用户登录后，很可能无法正确访问这个用户私有路径。

公共桌面项目只能选择两种策略：

## 安全默认

```text
自动模式默认不修改公共桌面
发现后加入待处理列表
用户打开应用后一次 UAC 批量处理
```

## 机器级美化模式

```text
公共桌面生成的 ICO
→ %ProgramData%\DeskMakeover\Icons

修改公共桌面快捷方式
→ ElevatedHelper

所有机器用户共享同一套机器级样式
```

不要让公共快捷方式引用某个用户的 LocalAppData。

另外，后台进程不应在用户没有操作时突然弹 UAC。因此：

```text
用户桌面新快捷方式
→ 已授权自动模式后可静默处理

公共桌面或机器级项目
→ 进入 privileged queue
→ 用户打开应用或点击处理时
→ 一次 UAC 批量完成
```

若你要求公共桌面项目也必须完全无人值守处理，那就需要系统服务或长期管理员进程，这会打破你只保留一个普通常驻进程的目标，也不值得。

---

# 九、最终建议的后台模块

不要把它写成一个巨大的 watcher 回调。分成这五个模块就够了：

```text
BackgroundCoordinator
├── DesktopChangeSource
│   ├── 用户桌面文件事件
│   ├── 公共桌面文件事件
│   └── Shell 通知
│
├── DesktopReconciler
│   ├── 扫描
│   ├── source fingerprint
│   ├── 新增、变化、删除判断
│   └── 冲突判断
│
├── IconJobProcessor
│   ├── 提取 source
│   ├── dm-core native 渲染
│   ├── ICO 生成
│   └── 应用与验证
│
├── PrivilegedQueue
│   ├── 公共桌面
│   ├── 全局箭头
│   └── 机器级系统对象
│
└── ActiveMakeoverLedger
    ├── 原始状态
    ├── source hash
    ├── style hash
    ├── 生成文件
    ├── 最后应用状态
    └── 恢复信息
```

完整状态流：

```text
event
→ debounce
→ stability probe
→ reconcile
→ diff
→ render
→ journal
→ atomic apply
→ Shell refresh
→ verify
→ commit ledger
```

任何一步失败，都不能把原始状态和恢复锚点弄丢。

---

# 十、我对你最终方案的判断

你现在可以正式采用：

```text
Tauri + React/Pixi
负责界面

Rust dm-core
负责图标分析、识别和渲染
WASM 用于前端
Native 用于后台

Rust dm-windows
负责 Shell、COM、壁纸、注册表、ICO、监听、恢复

Rust dm-elevated
负责极少数管理员操作

.NET
完全退出
```

你的代码签名证书继续保留，并用于所有可执行产物。

原生快捷方式箭头可以全局透明化，但必须承认它是兼容性方案，并把下面四件事做成一个完整事务：

```text
隐藏系统 overlay
桌面快捷方式全部重新绘制
完整快照与恢复
卸载时零残留还原
```

后台则采用：

```text
一个长期常驻的非提权 Rust 进程
加一个按需出现、执行完就退出的签名 helper
```

这才是准确的单进程架构。

你现在真正需要防的，不是 Rust 能不能做，而是三个工程幻觉：

> 监听到事件不等于掌握了真实状态。
> 隐藏一个箭头不等于只影响桌面。
> 一个常驻进程不等于永远不允许临时 helper。

把这三件事处理好，这套 Tauri + Rust 架构已经足够完整，而且不需要再把 .NET 请回来。