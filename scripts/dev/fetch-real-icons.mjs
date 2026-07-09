// Fetch REAL icons for the dev mock desktop (owner order 2026-07-09).
//
// Harvests genuine icon assets from two open-source Win11 simulator repos
// (blueedgetechno/win11React, piyushsuthar/windows-11-web) into
// src/DeskMakeover.Web/public/mock-icons-real/ + manifest.json.
//
// ⚠ LICENSE GATE: the harvested images include extracted Microsoft system
// icons and third-party brand icons. They are LOCAL DEV FIXTURES ONLY —
// the output directory is gitignored and must NEVER ship in a release or
// enter the repo. The shipped app mirrors the user's real desktop and needs
// no bundled icons (ADR-0015 D9).
//
// Usage: node scripts/dev/fetch-real-icons.mjs [repoCacheDir]
//   repoCacheDir: a dir that contains (or will receive) the two clones.
//   Defaults to .cache/win11sim under the repo root.

import { execFileSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const OUT = join(ROOT, 'src', 'DeskMakeover.Web', 'public', 'mock-icons-real')
const CACHE = process.argv[2] ?? join(ROOT, '.cache', 'win11sim')

const REPOS = [
  { name: 'win11React', url: 'https://github.com/blueedgetechno/win11React.git' },
  { name: 'windows-11-web', url: 'https://github.com/piyushsuthar/windows-11-web.git' },
]

function ensureRepo({ name, url }) {
  const dir = join(CACHE, name)
  if (!existsSync(dir)) {
    mkdirSync(CACHE, { recursive: true })
    console.log(`cloning ${url} (shallow)...`)
    execFileSync('git', ['clone', '--depth', '1', url, dir], { stdio: 'inherit' })
  }
  return dir
}

// ---- label books: real, recognizable desktop names ----

// win11React icon/win/*.png — extracted imageres/shell32 resources. Named
// files get their real shell labels; numbered file-type icons become files.
const WIN_LABELS = {
  thispc: ['此电脑', 'RecycleBinNo'], // kind override marker unused; see below
  bin: '回收站',
  'bin-em': null, // second recycle-bin state — folded into the bin entry
  folder: '新建文件夹',
  folder3d: '3D 对象',
  docs: '文档',
  down: '下载',
  music: '音乐',
  pics: '图片',
  vid: '视频',
  desk: '桌面',
  onedrive: 'OneDrive',
  disc: 'DVD 驱动器',
  disk: '本地磁盘 (C:)',
  user: '用户文件',
  store: 'Microsoft Store',
  shield: 'Windows 安全中心',
  network: '网络',
}

// win11React icon/*.png — real first/third-party app icons.
const APP_LABELS = {
  edge: 'Microsoft Edge',
  excel: 'Excel',
  powerpoint: 'PowerPoint',
  outlook: 'Outlook',
  onenote: 'OneNote',
  word: 'Word',
  groove: 'Groove 音乐',
  paint: '画图',
  calculator: '计算器',
  camera: '相机',
  notepad: '记事本',
  photos: '照片',
  maps: '地图',
  mail: '邮件',
  people: '人脉',
  alarm: '闹钟和时钟',
  calendar: '日历',
  board: '白板',
  code: 'Visual Studio Code',
  cortana: 'Cortana',
  defender: 'Windows 安全中心',
  explorer: '文件资源管理器',
  discord: 'Discord',
  github: 'GitHub Desktop',
  pinterest: 'Pinterest',
  spotify: 'Spotify',
  minecraft: 'Minecraft',
  news: '资讯',
  getstarted: '使用技巧',
  feedback: '反馈中枢',
  settings: '设置',
  store: 'Microsoft Store',
  terminal: '终端',
  twitter: 'Twitter',
  whiteboard: 'Whiteboard',
  yourphone: '手机连接',
}


// Numbered imageres icons, identified by eye from the contact sheet
// (2026-07-09): every label MUST match the art — a PDF name on a printer icon
// is exactly the confusion the owner banned. kind: doc-like art = RegularFile,
// tool-like art = Shortcut.
const NUM_ICONS = {
  3: { label: '新建文本文档.txt', kind: 'RegularFile' },
  50: { label: '归档', kind: 'Folder' },
  58: { label: '资源', kind: 'Folder' },
  67: { label: '项目计划.docx', kind: 'RegularFile' },
  90: { label: '待办事项.txt', kind: 'RegularFile' },
  98: { label: '未读邮件.eml', kind: 'RegularFile' },
  106: { label: '产品截图.png', kind: 'RegularFile' },
  114: { label: '录音备忘.mp3', kind: 'RegularFile' },
  122: { label: '宣传片素材.mp4', kind: 'RegularFile' },
  130: { label: '合同定稿.docx', kind: 'RegularFile' },
  170: { label: '季度汇报.pptx', kind: 'RegularFile' },
  175: { label: 'Internet', kind: 'Shortcut' },
  183: { label: '网络位置', kind: 'Shortcut' },
  199: { label: '打印机', kind: 'Shortcut' },
  255: { label: '帮助文档', kind: 'Shortcut' },
  797: { label: '卸载工具', kind: 'Shortcut' },
  805: { label: '年度总结草稿.docx', kind: 'RegularFile' },
  837: { label: '页面设置', kind: 'Shortcut' },
  877: { label: '快速启动', kind: 'Shortcut' },
  893: { label: '会议纪要_0708.txt', kind: 'RegularFile' },
  1077: { label: '主打歌demo.mp3', kind: 'RegularFile' },
  1085: { label: '海报终稿.jpg', kind: 'RegularFile' },
  1093: { label: '发布会回放.mp4', kind: 'RegularFile' },
  1101: { label: '播客节目.m4a', kind: 'RegularFile' },
  1154: { label: '相机导入', kind: 'Shortcut' },
  1247: { label: '显示设置', kind: 'Shortcut' },
  1278: { label: '合同_副本.docx', kind: 'RegularFile' },
  1286: { label: '截图工具', kind: 'Shortcut' },
  1294: { label: '入职清单.docx', kind: 'RegularFile' },
  1437: { label: '搜索', kind: 'Shortcut' },
  1479: { label: '屏幕键盘', kind: 'Shortcut' },
  1569: { label: '公司资料.docx', kind: 'RegularFile' },
  1577: { label: '设计稿.png', kind: 'RegularFile' },
  1585: { label: '音乐库', kind: 'Shortcut' },
  1593: { label: '击杀集锦.mp4', kind: 'RegularFile' },
  1609: { label: '共享文件', kind: 'Shortcut' },
  1669: { label: '铃声剪辑.mp3', kind: 'RegularFile' },
  1677: { label: '已禁用项', kind: 'Shortcut' },
  1693: { label: '系统镜像.iso', kind: 'RegularFile' },
  1736: { label: '固定便签', kind: 'Shortcut' },
  1780: { label: '剪贴板历史', kind: 'Shortcut' },
  1788: { label: '命令提示符', kind: 'Shortcut' },
  1836: { label: '窗口布局', kind: 'Shortcut' },
  2000: { label: '数据备份_0630', kind: 'RegularFile' },
}

function main() {
  const [w11react, w11web] = REPOS.map(ensureRepo)
  rmSync(OUT, { recursive: true, force: true })
  mkdirSync(OUT, { recursive: true })

  const manifest = []
  const used = new Set()
  const add = (srcPath, file, kind, label, extraSources = []) => {
    if (used.has(file)) return
    used.add(file)
    cpSync(srcPath, join(OUT, file))
    manifest.push({ file, id: `real-${file.replace(/\.[a-z]+$/, '')}`, kind, label, extraSources })
  }

  // 1) System icons (256px, extracted originals) — win11React icon/win/
  const winDir = join(w11react, 'public', 'img', 'icon', 'win')
  for (const f of readdirSync(winDir).filter((f) => f.endsWith('.png'))) {
    const base = f.replace('.png', '')
    if (base.endsWith('-sm')) continue // small variants — 256 set only
    if (base === 'bin-em') continue // folded into the bin entry below
    if (base === 'bin') {
      // Recycle Bin ships TWO sources: full (bin) + empty (bin-em).
      cpSync(join(winDir, 'bin-em.png'), join(OUT, 'win-bin-empty.png'))
      add(join(winDir, f), 'win-bin.png', 'RecycleBin', '回收站', ['win-bin-empty.png'])
      continue
    }
    const label = WIN_LABELS[base]
    const isFolder = /folder|docs|down|music|pics|vid|desk/.test(base)
    const num = /^\d+$/.test(base) ? NUM_ICONS[base] : null
    add(
      join(winDir, f),
      `win-${f}`,
      // This PC / Network / User Files style via the SAME per-user CLSID
      // DefaultIcon mechanism as the Recycle Bin (owner prototype truth) —
      // STYLEABLE; the C# writers are a Windows-batch addition.
      base === 'thispc' || base === 'network' || base === 'user' ? 'SystemIcon'
        : num ? num.kind
        : isFolder ? 'Folder'
        : 'Shortcut',
      num ? num.label : typeof label === 'string' ? label : base,
    )
  }

  // 2) Real app icons — win11React icon/*.png (64px; small real .lnk icons
  //    upscaled by the renderer, exactly like a real messy desktop).
  const appDir = join(w11react, 'public', 'img', 'icon')
  for (const f of readdirSync(appDir).filter((f) => f.endsWith('.png'))) {
    const base = f.replace('.png', '')
    if (base === 'bin0' || base === 'bin1') continue
    const label = APP_LABELS[base] ?? base
    // Store & friends are UWP on a real desktop — mark a few AppxShortcut so
    // the un-editable path gets exercised with REAL art.
    const uwp = ['store', 'getstarted', 'feedback', 'cortana', 'news', 'people', 'maps'].includes(base)
    add(join(appDir, f), `app-${f}`, uwp ? 'AppxShortcut' : 'Shortcut', label)
  }

  // 3) piyushsuthar desktop set — ONLY Control Panel survives (this_pc and
  //    recyclebin duplicate the better 256px win11React art; the「旧」suffix
  //    the dupes forced looked terrible — owner call 2026-07-09).
  const deskDir = join(w11web, 'src', 'assets', 'icons', 'Desktop')
  const cp = join(deskDir, 'control_panel.webp')
  if (existsSync(cp)) add(cp, 'p11-control_panel.webp', 'SystemIcon', '控制面板')

  // 4) Real Win11 default wallpapers (owner order: the dev fallback wallpaper
  //    must be the REAL default, not a drawn scene). Light + dark Bloom.
  const wallDir = join(w11react, 'public', 'img', 'wallpaper')
  for (const [src, out] of [
    [join('default', 'img0.jpg'), 'wallpaper-default.jpg'],
    [join('dark', 'img0.jpg'), 'wallpaper-dark.jpg'],
    // Scenario wallpapers (dev menu): gamer = ThemeA neon arc, office = ThemeC
    // lake morning; ThemeB/D ride along as spares for future scenarios.
    [join('ThemeA', 'img0.jpg'), 'wallpaper-gamer.jpg'],
    [join('ThemeC', 'img0.jpg'), 'wallpaper-office.jpg'],
    [join('ThemeB', 'img0.jpg'), 'wallpaper-spare-b.jpg'],
    [join('ThemeD', 'img0.jpg'), 'wallpaper-spare-d.jpg'],
  ]) {
    const p = join(wallDir, src)
    if (existsSync(p)) cpSync(p, join(OUT, out))
  }

  manifest.sort((a, b) => a.file.localeCompare(b.file))
  writeFileSync(join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 2))
  const total = manifest.length
  const kinds = manifest.reduce((m, e) => ((m[e.kind] = (m[e.kind] ?? 0) + 1), m), {})
  console.log(`mock-icons-real: ${total} icons ->`, kinds)
  console.log(`output: ${OUT} (gitignored — dev fixtures only, never ship)`)
}

main()
