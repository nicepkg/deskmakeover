// Harvest REAL icons into the single source of truth: public/real-icons/
// (owner orders 2026-07-09 + 2026-07-11: public/ is THE asset truth root).
//
// public/real-icons/ is THE one place for genuine icon fixtures, organized
// by type so the same icon never lives in two folders:
//   windows/     extracted Windows-native icons (imageres/shell32: This PC,
//                Recycle Bin, tools, CLSID-style system items)
//   folders/     system + plain folder icons (Documents, Downloads, ...)
//   apps/        first/third-party app icons (Edge, Discord, VS Code, ...)
//   files/       file-type icons (.txt/.docx/.mp3/...)
//   wallpapers/  real Win11 wallpapers (not icons; excluded from the manifest)
//
// The owner adds icons by DROPPING files into a subfolder — `--scan` (or any
// run) rebuilds manifest.json from the directory tree: kind comes from the
// subfolder, label from the filename stem (or the label books / overrides.json
// below). Harvesting MERGES: it never deletes files it did not produce.
//
// ⚠ LICENSING (ADR-0015 D9 + 2026-07-11 owner-override amendment): the pack
// contains extracted Microsoft system icons and third-party brand icons. It IS
// committed to the repo (owner call — one simple truth source), but it must
// NEVER ship in a release artifact: vite's closeBundle hook strips
// dist/real-icons, and the shipped app mirrors the user's real desktop and
// needs no bundled icons.
//
// Usage (bun-only repo — always run with bun):
//   bun scripts/dev/fetch-real-icons.ts               harvest + rebuild manifest
//   bun scripts/dev/fetch-real-icons.ts --scan        rebuild manifest only
//   bun scripts/dev/fetch-real-icons.ts --keep-cache  keep .cache/win11sim
//     (default: the 37 MB clone cache is deleted after a successful harvest;
//      the next harvest re-clones on demand)
//
// Runtime note: Bun natives (Bun.file / Bun.write / Bun.spawnSync /
// import.meta.dir) everywhere they exist; node:fs remains ONLY for directory
// ops (cp/mkdir/readdir/rm), which is Bun's own recommended surface for those.

import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs'
import { join } from 'node:path'

const ROOT = join(import.meta.dir, '..', '..')
const OUT = join(ROOT, 'public', 'real-icons')
const CACHE = join(ROOT, '.cache', 'win11sim')
const SCAN_ONLY = process.argv.includes('--scan')
const KEEP_CACHE = process.argv.includes('--keep-cache')

const SUBDIRS = ['windows', 'folders', 'apps', 'files', 'wallpapers'] as const
type Subdir = (typeof SUBDIRS)[number]

interface ManifestEntry {
  file: string
  id: string
  kind: string
  label: string
  extraSources: string[]
}
interface Refinement {
  kind?: string
  label?: string
  extraSources?: string[]
}

// kind default per subfolder; explicit entries below refine (SystemIcon,
// AppxShortcut, RecycleBin are always explicit — they carry special behavior).
const SUBDIR_KIND: Record<Exclude<Subdir, 'wallpapers'>, string> = {
  windows: 'Shortcut',
  folders: 'Folder',
  apps: 'Shortcut',
  files: 'RegularFile',
}

const REPOS = [
  { name: 'win11React', url: 'https://github.com/blueedgetechno/win11React.git' },
  { name: 'windows-11-web', url: 'https://github.com/piyushsuthar/windows-11-web.git' },
]

function ensureRepo({ name, url }: { name: string; url: string }): string {
  const dir = join(CACHE, name)
  if (!existsSync(dir)) {
    mkdirSync(CACHE, { recursive: true })
    console.log(`cloning ${url} (shallow)...`)
    const res = Bun.spawnSync(['git', 'clone', '--depth', '1', url, dir], {
      stdout: 'inherit',
      stderr: 'inherit',
    })
    if (res.exitCode !== 0) throw new Error(`clone failed: ${url}`)
  }
  return dir
}

// ---- label books: real, recognizable desktop names ----

// win11React icon/win/*.png — extracted imageres/shell32 resources. Named
// files get their real shell labels; numbered file-type icons become files.
const WIN_LABELS: Record<string, string> = {
  thispc: '此电脑',
  bin: '回收站',
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
const APP_LABELS: Record<string, string> = {
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
const NUM_ICONS: Record<string, { label: string; kind: string }> = {
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

// Explicit kind/label refinements for HARVESTED files, keyed by subfolder path.
// Owner-added files needing the same refinement go in overrides.json (merged
// on top of this book at scan time, same shape).
const REFINEMENTS: Record<string, Refinement> = {
  // This PC / Network / User Files / Control Panel style via the per-user
  // CLSID DefaultIcon mechanism (owner prototype truth) — STYLEABLE; the C#
  // writers are a Windows-batch addition.
  'windows/win-thispc.png': { kind: 'SystemIcon' },
  'windows/win-network.png': { kind: 'SystemIcon' },
  'windows/win-user.png': { kind: 'SystemIcon' },
  'windows/p11-control_panel.webp': { kind: 'SystemIcon', label: '控制面板' },
  // Recycle Bin ships TWO sources: full + empty.
  'windows/win-bin.png': { kind: 'RecycleBin', extraSources: ['windows/win-bin-empty.png'] },
  // UWP on a real desktop — the un-editable path exercised with REAL art.
  'apps/app-store.png': { kind: 'AppxShortcut' },
  'apps/app-getstarted.png': { kind: 'AppxShortcut' },
  'apps/app-feedback.png': { kind: 'AppxShortcut' },
  'apps/app-cortana.png': { kind: 'AppxShortcut' },
  'apps/app-news.png': { kind: 'AppxShortcut' },
  'apps/app-people.png': { kind: 'AppxShortcut' },
  'apps/app-maps.png': { kind: 'AppxShortcut' },
}

// Harvest-known labels, persisted so --scan runs keep them without re-harvest.
const LABEL_BOOK_PATH = join(OUT, 'labels.json')

async function loadJson<T>(path: string, fallback: T): Promise<T> {
  const f = Bun.file(path)
  return (await f.exists()) ? ((await f.json()) as T) : fallback
}

// ---- harvest (merge-only: overwrites its own outputs, never deletes) ----

function harvest(labelBook: Record<string, string>): void {
  const [w11react, w11web] = REPOS.map(ensureRepo)
  for (const d of SUBDIRS) mkdirSync(join(OUT, d), { recursive: true })
  const put = (srcPath: string, sub: Subdir, file: string, label?: string) => {
    cpSync(srcPath, join(OUT, sub, file))
    if (label !== undefined) labelBook[`${sub}/${file}`] = label
  }

  // 1) Windows-native set (256px extracted originals) — win11React icon/win/
  const winDir = join(w11react, 'public', 'img', 'icon', 'win')
  for (const f of readdirSync(winDir).filter((f) => f.endsWith('.png'))) {
    const base = f.replace('.png', '')
    if (base.endsWith('-sm')) continue // small variants — 256 set only
    if (base === 'bin-em') {
      put(join(winDir, f), 'windows', 'win-bin-empty.png')
      continue
    }
    const num = /^\d+$/.test(base) ? NUM_ICONS[base] : undefined
    const isFolder = num ? num.kind === 'Folder' : /folder|docs|down|music|pics|vid|desk/.test(base)
    const isFile = num?.kind === 'RegularFile'
    const sub: Subdir = isFolder ? 'folders' : isFile ? 'files' : 'windows'
    put(join(winDir, f), sub, `win-${f}`, num ? num.label : (WIN_LABELS[base] ?? base))
  }

  // 2) Real app icons — win11React icon/*.png (64px; small real .lnk icons
  //    upscaled by the renderer, exactly like a real messy desktop).
  const appDir = join(w11react, 'public', 'img', 'icon')
  for (const f of readdirSync(appDir).filter((f) => f.endsWith('.png'))) {
    const base = f.replace('.png', '')
    if (base === 'bin0' || base === 'bin1') continue
    put(join(appDir, f), 'apps', `app-${f}`, APP_LABELS[base] ?? base)
  }

  // 3) piyushsuthar desktop set — ONLY Control Panel survives (this_pc and
  //    recyclebin duplicate the better 256px win11React art — owner call
  //    2026-07-09).
  const cp = join(w11web, 'src', 'assets', 'icons', 'Desktop', 'control_panel.webp')
  if (existsSync(cp)) put(cp, 'windows', 'p11-control_panel.webp')

  // 4) Real Win11 default wallpapers (owner order: the dev fallback wallpaper
  //    must be the REAL default, not a drawn scene). Light + dark Bloom;
  //    gamer = ThemeA neon arc, office = ThemeC lake morning, B/D spares.
  const wallDir = join(w11react, 'public', 'img', 'wallpaper')
  for (const [src, out] of [
    [join('default', 'img0.jpg'), 'wallpaper-default.jpg'],
    [join('dark', 'img0.jpg'), 'wallpaper-dark.jpg'],
    [join('ThemeA', 'img0.jpg'), 'wallpaper-gamer.jpg'],
    [join('ThemeC', 'img0.jpg'), 'wallpaper-office.jpg'],
    [join('ThemeB', 'img0.jpg'), 'wallpaper-spare-b.jpg'],
    [join('ThemeD', 'img0.jpg'), 'wallpaper-spare-d.jpg'],
  ] as const) {
    const p = join(wallDir, src)
    if (existsSync(p)) cpSync(p, join(OUT, 'wallpapers', out))
  }
}

// ---- manifest: rebuilt by SCANNING the tree (owner-added files included) ----

async function buildManifest(labelBook: Record<string, string>): Promise<ManifestEntry[]> {
  const overrides = await loadJson<Record<string, Refinement>>(join(OUT, 'overrides.json'), {})
  const refine = (rel: string): Refinement => ({ ...REFINEMENTS[rel], ...overrides[rel] })

  // files referenced as extraSources never get their own manifest entry
  const consumed = new Set<string>()
  for (const book of [REFINEMENTS, overrides]) {
    for (const entry of Object.values(book)) {
      for (const extra of entry.extraSources ?? []) consumed.add(extra)
    }
  }

  const manifest: ManifestEntry[] = []
  for (const sub of SUBDIRS) {
    if (sub === 'wallpapers') continue // not desktop icons
    const dir = join(OUT, sub)
    if (!existsSync(dir)) continue
    for (const f of readdirSync(dir).filter((f) => /\.(png|webp|jpg|jpeg|ico|bmp)$/i.test(f))) {
      const rel = `${sub}/${f}`
      if (consumed.has(rel)) continue
      const r = refine(rel)
      const stem = f.replace(/\.[a-z]+$/i, '')
      manifest.push({
        file: rel,
        id: `real-${stem}`,
        kind: r.kind ?? SUBDIR_KIND[sub],
        label: r.label ?? labelBook[rel] ?? stem,
        extraSources: r.extraSources ?? [],
      })
    }
  }
  manifest.sort((a, b) => a.file.localeCompare(b.file))
  await Bun.write(join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 2))
  await Bun.write(LABEL_BOOK_PATH, JSON.stringify(labelBook, null, 2))
  return manifest
}

async function main(): Promise<void> {
  const labelBook = await loadJson<Record<string, string>>(LABEL_BOOK_PATH, {})
  if (!SCAN_ONLY) harvest(labelBook)
  const manifest = await buildManifest(labelBook)
  const kinds = manifest.reduce<Record<string, number>>(
    (m, e) => ((m[e.kind] = (m[e.kind] ?? 0) + 1), m),
    {},
  )
  console.log(`real-icons: ${manifest.length} icons ->`, kinds)
  console.log(`SSoT: ${OUT} (committed; stripped from release artifacts — ADR-0015 D9 amendment)`)
  if (!SCAN_ONLY && !KEEP_CACHE && existsSync(CACHE)) {
    rmSync(CACHE, { recursive: true, force: true })
    console.log(`cache ${CACHE} removed (re-clones on the next harvest; --keep-cache to keep)`)
  }
}

await main()
