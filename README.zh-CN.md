<div align="center">

<img src=".github/assets/logo.png" width="96" alt="DeskMakeover logo" />

# DeskMakeover

<img src=".github/assets/name-zh-pill.png" height="22" alt="桌面美颜" />

**一键美颜你的 Windows 桌面，一键完整还原。**

[![状态](https://img.shields.io/badge/beta-v0.1.0-FF6F5E?labelColor=2f363d)](https://github.com/nicepkg/deskmakeover/releases)
[![Windows](https://img.shields.io/badge/Windows-10%20%C2%B7%2011-464f58?labelColor=2f363d)](#安装)
[![许可证](https://img.shields.io/badge/License-MIT-464f58?labelColor=2f363d)](LICENSE)
[![Tauri](https://img.shields.io/badge/Built%20with-Tauri%202-464f58?labelColor=2f363d)](https://v2.tauri.app/)

[English](README.md) · **中文**

<br/>

<a href="https://github.com/nicepkg/deskmakeover/releases">
  <img src=".github/assets/hero-beforeafter.svg" width="880" alt="杂乱的 Windows 默认桌面一键美颜成整齐的方圆图标，然后完整还原" />
</a>

</div>

<br/>

DeskMakeover（桌面美颜）把杂乱的 Windows 桌面变成干净、像被认真设计过的样子，并且随时能还原到分毫不差的原样。它重绘你的桌面图标，在图标背后铺半透明的壁纸分区；每一个像素都先实时预览，确认之后才写入任何文件。不用 PowerShell，不用改注册表，不用手动换图标。

> **状态：Beta。** 桌面壳已在真实 Windows 10/11 上运行；写入面正在完成真机验证，首个公开安装包正在准备中。在它发布前，请[从源码构建](#面向开发者)。细节见[常见问题](#常见问题)。

## 可还原是产品本身

- **动任何东西之前先快照。** 每次应用前都会快照你当前的图标、箭头和壁纸。还原只需一键，带回分毫不差的原样。
- **先看到，再发生。** 实时预览和最终写入用的是同一套渲染代码，你看到什么，落地就是什么。
- **纯本地。** 无账号，无上传，无遥测。一切都在你的机器上运行、留存。
- **主程序不要管理员权限。** 应用以普通用户身份运行；极少数特权操作走一个只认固定动作白名单的小助手。
- **签名发布。** 每个公开发布的安装包都会带 Authenticode 签名，Windows 可以验证它出厂后未被篡改；本地构建保持未签名。

<div align="center"><br/><img src=".github/assets/rule-sparkle.svg" width="80" alt="" /><br/><br/></div>

## 九套外观，一键上身

同一个文件夹，九种穿法；每一套都是在真实桌面上手工调出来的：

<div align="center">
<img src=".github/assets/specimen-nine-styles.webp" width="880" alt="同一个文件夹图标在九套 DeskMakeover 风格下的样子" />
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

预设是起点，不是牢笼。它下面是一套真正的图标设计系统：基于地道 iOS 连续圆角 squircle 几何的 11 形状目录、共享调色盘的配色处理、精修的快捷方式标识、质感滤镜，以及按类型覆写（文件夹和文件可以各有自己的形状）。图标主体像素永不被重新上色；外观靠底板、剪影和背景来区分。

## 它能做什么

- **一键美颜：**在你真实桌面的实时镜像上（真壁纸、真图标位置）重绘每一个图标。按住对比原样，右键单独覆盖任意图标，支持完整撤销重做与版本历史。
- **壁纸分区：**直接在壁纸上画半透明面板给图标分组：五种材质、四种标题样式、可选烘焙阴影、网格吸附。原壁纸自动备份，一键换回。
- **清爽（Calm Windows）：**引导式、fail-closed 的系统安静助手。任何一条调整在配方被认证前都不会真正写入；在那之前它教你真正的 Windows 设置在哪，并直接带你过去。
- **还原永远在手边：**任何改动前都先快照；一键带回你原本的图标、箭头和壁纸，分毫不差。

<div align="center"><br/><img src=".github/assets/rule-sparkle.svg" width="80" alt="" /><br/><br/></div>

## 走进工作台

<div align="center">
<img src=".github/assets/app-studio.webp" width="880" alt="DeskMakeover 主窗口：左边是桌面实时镜像，右边是设计控制面板" />
<br/>
<sub>右边挑一套外观，左边看着自己的真实桌面实时换装，满意了再按下美化。</sub>
</div>

## 安装

1. 打开 [Releases](https://github.com/nicepkg/deskmakeover/releases) 页面，下载最新的 `DeskMakeover_x.y.z_x64-setup.exe`。（首个公开版本正在路上；在它出现之前，请[从源码构建](#面向开发者)。）
2. 双击运行。它按用户安装，不弹管理员提示；如果系统缺 WebView2 运行时会自动补上。
3. 打开 DeskMakeover 开始。桌面上的一切都先预览再落地，每个改动都能一键撤销。

支持 Windows 10（1809 及以上）和 Windows 11，64 位。

> **如果 Windows 弹出蓝色的「Windows 已保护你的电脑」：** 这是 SmartScreen 对还不常见的发布者的谨慎提示，不是病毒警告。安装包做了 Authenticode 签名，你可以右键 `.exe`，选「属性 → 数字签名」核对签署者。要继续安装，点「更多信息」，再点「仍要运行」。随着装签名版的人越来越多，Windows 就不再弹这个提示了。

## 常见问题

**它会拖慢电脑吗？**
重活只在你应用一套外观时干一次。之后有一个小托盘助手盯着 Windows 重置桌面的时机（比如资源管理器重启）并把你的外观补回来。它是一个校对器，不是常驻渲染器。

**重启或 Windows 更新后还在吗？**
美化后的图标是烘焙成真实图片文件的，重启不丢；Windows 重建图标缓存时托盘助手会把外观校对回来。大版本的 Windows 功能更新可能重置部分系统默认项；遇到了就一键重新应用，或者干脆还原。

**怎么变回原样？**
一键。DeskMakeover 在任何改动前都快照了你原本的图标、箭头和壁纸，还原就是分毫不差地放回去。你也可以逐步撤销重做，设置里还有完整的「恢复系统原始外观」。

**它动注册表吗？**
你不需要碰注册表，你原本的图标文件也永远不会被修改。美化图标是存在应用自己数据目录里的新图片文件；文件夹走标准的 `desktop.ini` 机制。应用确实会改的少数 shell 设置都先快照，还原时原样放回。

**支持 Windows 10 吗？**
支持，Windows 10 需 1809 及以上，64 位；也支持 64 位 Windows 11。

## 工作原理

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

bridge 契约由 `dm-contracts` crate 生成，并由 CI 里的 bindings 测试锁住，TypeScript 与 Rust 两侧保持同步。web 半边可独立对着 mock 后端运行，这也是大部分 UI 在浏览器循环里构建和测试的原因。全貌见 [`docs/development.md`](docs/development.md) 与 [`docs/specs/`](docs/specs) 下的设计规格。

## 面向开发者

[![CI](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml/badge.svg)](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml)

需要 [**Bun**](https://bun.sh) ≥ 1.1 与 [`rust-toolchain.toml`](rust-toolchain.toml) 里锁定的 Rust 工具链（`rustup` 会自动装）。本仓库只用 Bun 这一个 JS 工具链。

```bash
bun install
bun run dev          # 用 mock 后端跑 web UI：任意 OS，浏览器 + 热更新
bun run tauri:dev    # 完整桌面应用(Windows)：编译 Rust host，打开窗口
```

完整开发手册（开发模式、测试、Tauri 循环、打包、签名）见 [`docs/development.md`](docs/development.md)；本地构建永远是未签名的，在哪都能跑。

## 参与贡献

React UI、Rust 内核、文档、本地化、Windows 兼容性测试都欢迎参与；大部分 UI 在浏览器里就能开发测试，不需要 Windows 机器。先看 [`CONTRIBUTING.md`](CONTRIBUTING.md) 了解环境搭建和家规，再看看 [good first issue](https://github.com/nicepkg/deskmakeover/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)。安全问题走 [`SECURITY.md`](SECURITY.md)。

## 许可证

[MIT](LICENSE) © 2026 [Jinming Yang](https://github.com/2214962083)。免费开源。
