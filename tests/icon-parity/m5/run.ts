#!/usr/bin/env bun
// M5 icon-core certification — one command runs every parity gate end to end:
//
//   bun tests/icon-parity/m5/run.ts
//
// 1. cargo test (dm-icon-core + dm-icon-codec unit/property tests)
// 2. shape-mask bit parity  (module 2: geometry)
// 3. profile + mask parity   (modules 4/5: analysis + segment + profile)
// 4. hue-spread parity       (module 6)
// 5. full pixel differential (modules 8/9/10: compose + marks + filters)
//
// Every gate runs over the REAL icon corpus (public/real-icons via testdata/icons),
// the only pack. Each TS dumper writes to target/m5/<stage>; the matching `xtask m5-*`
// reruns the stage through dm-icon-core and compares byte-for-byte. Any failure exits
// non-zero. Spike-4 (tests/icon-parity/spike4/run.ts) remains the tri-target
// (TS↔native↔wasm) gate for the shared slice.

import { join, resolve } from 'node:path'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const M5_DIR = join(REPO_ROOT, 'target/m5')

function run(label: string, cmd: string, args: string[]): void {
  console.log(`\n== ${label}: ${cmd} ${args.join(' ')}`)
  const r = Bun.spawnSync([cmd, ...args], { cwd: REPO_ROOT, stdout: 'inherit', stderr: 'inherit' })
  if (r.exitCode !== 0) {
    console.error(`\n${label} FAILED (exit ${r.exitCode})`)
    process.exit(r.exitCode ?? 1)
  }
}

run('cargo test', 'cargo', ['test', '-p', 'dm-icon-core', '-p', 'dm-icon-codec'])

run('shape-mask dump', 'bun', ['tests/icon-parity/m5/shape-masks.ts'])
run('shape-mask compare', 'cargo', ['run', '--release', '-q', '-p', 'xtask', '--', 'm5-shape-masks', M5_DIR])

run('profile dump', 'bun', ['tests/icon-parity/m5/profiles.ts'])
run('profile compare', 'cargo', ['run', '--release', '-q', '-p', 'xtask', '--', 'm5-profiles', M5_DIR])

run('hue-spread dump', 'bun', ['tests/icon-parity/m5/hue-spread.ts'])
run('hue-spread compare', 'cargo', ['run', '--release', '-q', '-p', 'xtask', '--', 'm5-hue', M5_DIR])

run('pixel dump', 'bun', ['tests/icon-parity/m5/cells.ts'])
run('pixel compare', 'cargo', ['run', '--release', '-q', '-p', 'xtask', '--', 'm5-pixels', M5_DIR])

console.log('\nM5 icon-core certification: ALL GATES PASS')
