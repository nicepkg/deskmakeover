<div align="center">

<img src=".github/assets/logo.png" width="96" alt="DeskMakeover logo" />

# DeskMakeover

<img src=".github/assets/name-zh-pill.png" height="22" alt="桌面美颜" />

**给你的 Windows 桌面换个好看的样子。不喜欢，一键变回原样。**

[![状态](https://img.shields.io/badge/beta-v0.1.0-FF6F5E?labelColor=2f363d)](https://github.com/nicepkg/deskmakeover/releases)
[![Windows](https://img.shields.io/badge/Windows-10%20%C2%B7%2011-464f58?labelColor=2f363d)](#-怎么安装)
[![许可证](https://img.shields.io/badge/License-MIT-464f58?labelColor=2f363d)](LICENSE)
[![Tauri](https://img.shields.io/badge/Built%20with-Tauri%202-464f58?labelColor=2f363d)](https://v2.tauri.app/)

[English](README.md) · **中文**

<br/>

<a href="https://github.com/nicepkg/deskmakeover/releases">
  <img src=".github/assets/hero-beforeafter.svg" width="880" alt="杂乱的 Windows 默认桌面一键美颜成整齐的方圆图标，然后完整还原" />
</a>

</div>

<br/>

DeskMakeover（桌面美颜）帮你把乱糟糟的 Windows 桌面，收拾成看着顺眼、像被认真布置过的样子。它给桌面图标换上你选的外观，还能在壁纸上圈出分区，把图标归归类。动手之前你先看到效果，满意了才落地；不满意，一键就回到原来的样子。

<details>
<summary>📑 目录</summary>

- [你的桌面，可以是这样的](#%EF%B8%8F-你的桌面可以是这样的)
- [先说最重要的：随时能变回来](#%EF%B8%8F-先说最重要的随时能变回来)
- [九套现成外观，一键上身](#-九套现成外观一键上身)
- [每一处都能自己调](#-每一处都能自己调)
- [存成自己的风格，还能分享](#-存成自己的风格还能分享)
- [在壁纸上圈出分区](#%EF%B8%8F-在壁纸上圈出分区图标不再乱堆)
- [怎么安装](#-怎么安装) · [常见问题](#-常见问题) · [工作原理](#%EF%B8%8F-工作原理)

</details>

## 🖥️ 你的桌面，可以是这样的

上面这幅画面，前一半是很多人桌面的日常：图标挤成一片，一堆小箭头，看着就累。点一下美化，同一台电脑，同一批图标，同一张壁纸，就换了个样子。中间没有魔法，是你自己一步步挑、一步步调，看着它慢慢变成这样。

## ↩️ 先说最重要的：随时能变回来

很多人想动桌面又不敢，怕弄坏了回不去。这个「回不去」，恰恰是我们最先解决的事。

动任何东西之前，它会先把你现在的桌面完整备份一份，就像先给桌面拍张照片存起来：图标、箭头、壁纸，原封不动地留好。之后你怎么调都行，想回到最初，点一下还原，就和你没动过时一模一样。就算调得不满意，也不会把自己困在里面。

它只在你这台电脑上跑。不用注册账号，不联网上传任何东西。你的桌面长什么样，只有你自己看得到。

你也不用自己敲命令，不用碰任何技术设置。你要做的就三件事：挑、预览、满意了点一下。

<div align="center"><br/><img src=".github/assets/rule-gradient.svg" width="880" alt="" /><br/><br/></div>

## ✨ 九套现成外观，一键上身

我们内置了九套外观，每一套都在真实桌面上一点点手工调过。挑一套点一下，所有图标就换上这个样子。同一个文件夹，九种穿法：

<div align="center">
<img src=".github/assets/specimen-nine-styles.webp" width="880" alt="同一个文件夹图标在九套风格下的样子" />
<br/>
<sub>方圆 · 圆窗 · 像素纪元 · 溪石 · 拼贴手帐 · 浮光 · 随形贴 · 蓝图 · 釉光</sub>
</div>

<br/>

<table>
<tr>
  <td align="center"><img src=".github/assets/preset-squircle.webp" width="250" alt="方圆" /><br/><b>方圆</b><br/><sub>一枚方圆，各有其形</sub></td>
  <td align="center"><img src=".github/assets/preset-blueprint.webp" width="250" alt="蓝图" /><br/><b>蓝图</b><br/><sub>一整套工程蓝图</sub></td>
  <td align="center"><img src=".github/assets/preset-pixel-era.webp" width="250" alt="像素纪元" /><br/><b>像素纪元</b><br/><sub>回到八比特的下午</sub></td>
</tr>
<tr>
  <td align="center"><img src=".github/assets/preset-gleam.webp" width="250" alt="浮光" /><br/><b>浮光</b><br/><sub>原样的图标，掠过一层光</sub></td>
  <td align="center"><img src=".github/assets/preset-glaze.webp" width="250" alt="釉光" /><br/><b>釉光</b><br/><sub>上过釉的冷瓷面</sub></td>
  <td align="center"><img src=".github/assets/preset-die-cut.webp" width="250" alt="随形贴" /><br/><b>随形贴</b><br/><sub>沿轮廓裁开的贴纸</sub></td>
</tr>
<tr>
  <td align="center"><img src=".github/assets/preset-porthole.webp" width="250" alt="圆窗" /><br/><b>圆窗</b><br/><sub>程序是圆窗</sub></td>
  <td align="center"><img src=".github/assets/preset-scrapbook.webp" width="250" alt="拼贴手帐" /><br/><b>拼贴手帐</b><br/><sub>随手拼贴的一页</sub></td>
  <td align="center"><img src=".github/assets/preset-creekstone.webp" width="250" alt="溪石" /><br/><b>溪石</b><br/><sub>溪水磨圆的石头</sub></td>
</tr>
</table>

## 🎨 每一处都能自己调

<table>
<tr>
<td width="55%" valign="middle">

九套只是起点，不是全部。外观的每一部分你都能自己搭：图标形状有十一种可选，配色、图标底板的颜色、表面质感、快捷方式那个小标记的样式，都能单独换。程序、文件夹、普通文件还能各用各的样子。

这样组合下来是上千种搭配，够你调出一套只属于自己的桌面。

预览时按住不放，就能和原样对比；哪个图标想单独改，右键点它就行；调乱了可以一步步撤销、重做。

</td>
<td width="45%" align="center">
  <img src=".github/assets/feature-combine.webp" width="380" alt="外观控制面板：形状、配色、底板、质感、快捷方式标记都能单独调" />
</td>
</tr>
</table>

## 🧩 存成自己的风格，还能分享

<table>
<tr>
<td width="45%" align="center">
  <img src=".github/assets/feature-stylepack.webp" width="360" alt="风格库：九套内置外观，支持存为我的风格、导出和导入" />
</td>
<td width="55%" valign="middle">

调出一套顺眼的搭配，别让它溜走。你可以把它存成自己的一套风格，下次一键就能再用上。

你也可以把它导出成一个文件，发给朋友。朋友把文件导进去，就用上了你调的这一套。反过来，别人做好的风格包，你拿过来也能直接用。一套好看的桌面，可以这样传来传去。

</td>
</tr>
</table>

## 🗂️ 在壁纸上圈出分区，图标不再乱堆

图标越攒越多，最后堆成一片，找个东西得扫半天。你可以直接在壁纸上圈出几块半透明的区域，比如一块放工作，一块放娱乐，把相关的图标归到一起，看着清爽多了。

每块区域放在哪、多大，你自己拖着调。有五种材质、四种标题样式可选，还能让壁纸稍微暗一点，让分区更清楚。原来的壁纸照样备份好，想撤掉分区，一键换回。

<div align="center">
<img src=".github/assets/feature-zones.webp" width="880" alt="壁纸分区编辑：亮暗两种材质的分区摆在壁纸上，位置大小随意拖" />
</div>

## ✂️ 快捷方式的小箭头，可换可去

快捷方式左下角那个小箭头，很多人嫌它丑。你可以把它换成更精致的小标记，或者干脆去掉。

这件事由应用替你完成，你不用自己动手做任何技术操作。去掉箭头的时候，Windows 会弹一次确认框，问你允不允许，你点「允许」就行。改之前它一样先备份好，想让箭头回来，一键就装回去。

> 顺带一提：内置的「清爽」小助手还能帮你把 Windows 一些吵闹的默认设置安静下来。改哪条都先问你，也都能改回来。

<div align="center"><br/><img src=".github/assets/rule-gradient.svg" width="880" alt="" /><br/><br/></div>

## 🎛️ 走进工作台

<div align="center">
<img src=".github/assets/app-studio.webp" width="880" alt="DeskMakeover 主窗口：左边是桌面实时镜像，右边是设计控制面板" />
<br/>
<sub>右边挑一套外观，左边看着自己的真实桌面实时换装，满意了再按下美化。</sub>
</div>

## 📦 怎么安装

第一个正式安装包还在准备中，做好后会放到 [Releases](https://github.com/nicepkg/deskmakeover/releases) 页面。到时候步骤很简单：下载那个安装文件，双击打开。它只给你自己这个账户安装，装完打开就能用。

如果 Windows 弹出一个蓝色的「Windows 已保护你的电脑」，别慌。这不是说软件有病毒，只是 Windows 对下载人数还不多的新软件比较谨慎。点一下「更多信息」，再点「仍要运行」就好。等装的人多了，这个提示就不再出现了。

它支持 Windows 10（1809 及以上版本）和 Windows 11，64 位。

## 💬 常见问题

**会不会把电脑弄坏，回不去？**
它动手之前先把你的桌面完整备份，任何改动都能一键还原，回到和你没动过时一模一样。这是我们最优先保证的事，所以就算你随便试，也能随时收回来。

**会不会拖慢电脑？**
不会让你察觉到。费劲的活只在你点「应用」的那一下干一次，平时它只是安安静静待着。万一 Windows 自己把桌面刷回了默认，它会帮你把外观补回来。

**关机重启之后还在吗？**
在。换好的图标是实实在在的图片文件，重启不会丢。如果哪天 Windows 大更新把一些设置重置了，你一键重新应用就好，或者干脆还原。

**要花钱吗？我的东西会被上传吗？**
完全免费，而且是开源的。它只在你这台电脑上跑，不用注册账号，也不会把你的桌面或任何东西传到网上。

**我不太懂电脑，能用明白吗？**
能。你要做的就是挑一套外观、看预览、满意了点一下。全程不用敲命令，不用碰任何技术设置。挑错了、看腻了，随时一键还原。

> **说句实在话：** 它现在还是 Beta，第一个安装包还在准备，用起来会有些粗糙的地方。它做的是把桌面图标和壁纸分区变好看，不负责给整个 Windows 换主题。别指望它一键就完美，但你调出来的样子随时能一键收回，所以尽管放心试。

<div align="center"><br/><img src=".github/assets/rule-gradient.svg" width="880" alt="" /><br/><br/></div>

## 🏗️ 工作原理

DeskMakeover 是一个 **Tauri 2 + Rust** 桌面应用，UI 是渲染在系统 WebView（Windows 上是 WebView2）里的 **React**。像素由一个 Rust 图标内核统一拥有：

```
React UI  ──(生成式 bridge，tauri-specta)──▶  Rust host
  │                                              │
  │  实时预览 + 设计控件                          ├─ dm-icon-core   唯一像素真理(WASM 预览 + 原生烘焙)
  │  WYSIWYG 画布(Pixi 壁纸)                     ├─ dm-windows     shell / 注册表 / 桌面几何
  └─ 浏览器开发用 mock 后端                        ├─ dm-operations  快照 · 应用 · 还原
                                                  ├─ dm-resident    后台托盘 + reconciler
                                                  └─ dm-elevated    极小的白名单提权助手
```

bridge 契约由 `dm-contracts` crate 生成，并由 CI 里的 bindings 测试锁住，TypeScript 与 Rust 两侧保持同步。预览像素和最终落地像素来自同一套 `dm-icon-core` 渲染代码（预览走 WASM，落地走原生），所见即所得是构造出来的，不是碰运气。隐藏原生快捷方式箭头会通过白名单提权助手改一个 Shell Icons 注册表值；和其他所有改动一样，改之前先快照，还原时原样放回。web 半边可独立对着 mock 后端运行，这也是大部分 UI 在浏览器循环里构建和测试的原因。全貌见 [`docs/development.md`](docs/development.md) 与 [`docs/specs/`](docs/specs) 下的设计规格。

## 🛠️ 面向开发者

[![CI](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml/badge.svg)](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml)

需要 [**Bun**](https://bun.sh) ≥ 1.1 与 [`rust-toolchain.toml`](rust-toolchain.toml) 里锁定的 Rust 工具链（`rustup` 会自动装）。本仓库只用 Bun 这一个 JS 工具链。

```bash
bun install
bun run dev          # 用 mock 后端跑 web UI：任意 OS，浏览器 + 热更新
bun run tauri:dev    # 完整桌面应用(Windows)：编译 Rust host，打开窗口
```

完整开发手册（开发模式、测试、Tauri 循环、打包、签名）见 [`docs/development.md`](docs/development.md)；本地构建永远是未签名的，在哪都能跑。

## 🤝 参与贡献

React UI、Rust 内核、文档、本地化、Windows 兼容性测试都欢迎参与；大部分 UI 在浏览器里就能开发测试，不需要 Windows 机器。先看 [`CONTRIBUTING.md`](CONTRIBUTING.md) 了解环境搭建和家规，再看看 [good first issue](https://github.com/nicepkg/deskmakeover/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)。安全问题走 [`SECURITY.md`](SECURITY.md)。

## 📄 许可证

[MIT](LICENSE) © 2026 [Jinming Yang](https://github.com/2214962083)。免费开源。

<div align="center">

<br/>

**桌面变好看了？给个 ⭐ 让更多人看到它。**

<a href="https://github.com/nicepkg/deskmakeover"><img src="https://img.shields.io/github/stars/nicepkg/deskmakeover?style=social" alt="Star DeskMakeover" /></a>

<br/><br/>

<img src=".github/assets/rule-gradient.svg" width="880" alt="" />

<img src=".github/assets/logo.png" width="40" alt="" />

**DeskMakeover · 桌面美颜**

[Releases](https://github.com/nicepkg/deskmakeover/releases) · [Issues](https://github.com/nicepkg/deskmakeover/issues) · [English](README.md) · **中文**

</div>
