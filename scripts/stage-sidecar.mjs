#!/usr/bin/env bun
// Build the dm-elevated privileged helper (release) and stage it as the Tauri
// sidecar (externalBin) so `tauri build` bundles it next to the main exe. At
// runtime the app resolves `dm-elevated.exe` from current_exe().parent()
// (src-tauri/src/lib.rs), which is exactly where Tauri drops a sidecar after
// stripping the target-triple suffix. Bun-only (owner rule: never node/npm);
// cross-platform via node:* builtins Bun implements.
//
// Two ship-only hardening steps run HERE, not in cargo, so `cargo test` stays a
// manifest-free, non-elevated, dynamic-CRT build:
//
//  1. SECURITY / self-containment (development.md §6.1): the app installs per-user
//     to a user-writable dir (%LOCALAPPDATA%) and launches this helper elevated via
//     ShellExecuteExW "runas". A dynamically-imported non-KnownDLL (the default
//     VCRUNTIME140.dll) beside the elevated exe is a DLL-hijack LPE (CWE-427). So
//     the sidecar is built with a STATIC CRT (+crt-static) and the build hard-fails
//     if any hijackable CRT module name survives in the import table.
//  2. ELEVATION: the requireAdministrator manifest is embedded into the STAGED exe
//     with mt.exe. Doing this at build (a linker arg) would also mark the crate's
//     unit-test harness requireAdministrator → `cargo test -p dm-elevated` fails to
//     launch it with os error 740. Packaging-time embedding is the crate's original
//     design (see crates/dm-elevated/src/main.rs) and keeps the test suite green.

import { execFileSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync } from 'node:fs'
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
const isWin = host.includes('windows')
const exe = isWin ? '.exe' : ''

// +crt-static statically links the CRT so the shipped elevated helper imports no
// hijackable CRT DLL. Scoped to THIS cargo invocation only — the main app keeps the
// dynamic CRT WebView2/tauri expect, and it runs unelevated so a hijack there is no
// escalation.
const env = { ...process.env }
if (isWin) {
  env.RUSTFLAGS = `${env.RUSTFLAGS ? env.RUSTFLAGS + ' ' : ''}-C target-feature=+crt-static`
}

execFileSync('cargo', ['build', '--release', '-p', 'dm-elevated'], {
  cwd: ROOT,
  stdio: 'inherit',
  env,
})

const built = join(ROOT, 'target', 'release', `dm-elevated${exe}`)

const outDir = join(ROOT, 'src-tauri', 'binaries')
const staged = join(outDir, `dm-elevated-${host}${exe}`)
mkdirSync(outDir, { recursive: true })
copyFileSync(built, staged)

if (isWin) {
  // (1) Self-contained guard: the import table stores each dependency DLL name as an
  // ASCII string, so a surviving VCRUNTIME140/ucrt apiset means +crt-static did not
  // take — refuse to stage a hijackable elevated helper.
  const bytes = readFileSync(staged)
  const hijackable = ['VCRUNTIME140.dll', 'msvcp140.dll', 'api-ms-win-crt-']
  const hit = hijackable.find((name) => bytes.includes(Buffer.from(name, 'ascii')))
  if (hit) {
    throw new Error(
      `dm-elevated still imports a hijackable CRT module (${hit}) — +crt-static did not apply. ` +
        `Refusing to stage a non-self-contained elevated helper (DLL-hijack LPE, development.md §6.1).`,
    )
  }

  // (2) Embed the requireAdministrator manifest into the staged exe with mt.exe.
  const mt = findMt()
  const manifest = join(ROOT, 'crates', 'dm-elevated', 'dm-elevated.exe.manifest')
  execFileSync(mt, ['-nologo', '-manifest', manifest, `-outputresource:${staged};#1`], {
    stdio: 'inherit',
  })
  // Confirm the embed took and requests admin — never ship a helper that silently
  // lost its elevation request.
  const embedded = readFileSync(staged)
  if (!embedded.includes(Buffer.from('requireAdministrator', 'ascii'))) {
    throw new Error('manifest embed failed: staged dm-elevated does not request requireAdministrator')
  }
}

console.log(`dm-elevated → ${staged}${isWin ? ' (static CRT + requireAdministrator manifest)' : ''}`)

// Locate mt.exe in the newest installed Windows 10/11 SDK (a Tauri MSVC prerequisite).
function findMt() {
  const roots = [
    join(process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)', 'Windows Kits', '10', 'bin'),
    join(process.env['ProgramFiles'] ?? 'C:\\Program Files', 'Windows Kits', '10', 'bin'),
  ]
  for (const bin of roots) {
    if (!existsSync(bin)) continue
    const versions = readdirSync(bin)
      .filter((v) => /^\d+\.\d+/.test(v))
      .sort()
      .reverse()
    for (const v of versions) {
      const p = join(bin, v, 'x64', 'mt.exe')
      if (existsSync(p)) return p
    }
  }
  throw new Error('mt.exe not found in the Windows SDK — install the Windows 10/11 SDK (Tauri prerequisite)')
}
