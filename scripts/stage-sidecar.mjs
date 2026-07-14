#!/usr/bin/env bun
// Build the dm-elevated privileged helper (release) and stage it as the Tauri
// sidecar (externalBin) so `tauri build` bundles it next to the main exe. At
// runtime the app resolves `dm-elevated.exe` from current_exe().parent()
// (src-tauri/src/lib.rs), which is exactly where Tauri drops a sidecar after
// stripping the target-triple suffix. Bun-only (owner rule: never node/npm);
// cross-platform via node:* builtins Bun implements.

import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')

// Tauri names a sidecar `<name>-<target-triple>[.exe]`; the triple must match the
// build host (rustc's `host:`), so read it rather than hard-coding one platform.
const vv = execFileSync('rustc', ['-vV'], { encoding: 'utf8' })
const host = vv
  .split('\n')
  .find((l) => l.startsWith('host:'))
  ?.slice('host:'.length)
  .trim()
if (!host) throw new Error('could not read the host triple from `rustc -vV`')
const exe = host.includes('windows') ? '.exe' : ''

execFileSync('cargo', ['build', '--release', '-p', 'dm-elevated'], {
  cwd: ROOT,
  stdio: 'inherit',
})

const built = join(ROOT, 'target', 'release', `dm-elevated${exe}`)
const outDir = join(ROOT, 'src-tauri', 'binaries')
const staged = join(outDir, `dm-elevated-${host}${exe}`)
mkdirSync(outDir, { recursive: true })
copyFileSync(built, staged)
console.log(`dm-elevated → ${staged}`)
