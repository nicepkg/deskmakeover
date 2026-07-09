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
    const isNumbered = /^\d+$/.test(base)
    add(
      join(winDir, f),
      `win-${f}`,
      // This PC / Network / User Files style via the SAME per-user CLSID
      // DefaultIcon mechanism as the Recycle Bin (owner prototype truth) —
      // STYLEABLE; the C# writers are a Windows-batch addition.
      base === 'thispc' || base === 'network' || base === 'user' ? 'SystemIcon'
        : isFolder ? 'Folder'
        : isNumbered ? 'RegularFile'
        : 'Shortcut',
      typeof label === 'string' ? label : isNumbered ? `文件_${base}` : base,
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
