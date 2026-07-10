#!/usr/bin/env bun
// Spike 4 (ADR-0019 M1 gate) — tri-target pixel-slice pipeline, one command:
//
//   bun tests/icon-parity/spike4/run.ts
//
// 1. cargo test (dm-icon-core + dm-icon-wasm unit/property tests)
// 2. TS oracle side  → target/spike4/{sources,ts}/ + cells.tsv + fixtures.tsv
// 3. Rust native side → target/spike4/native/
// 4. Rust WASM side   → target/spike4/wasm/ (wasm32-unknown-unknown via Bun)
// 5. xtask spike4-compare → summary table + PASS/FAIL (process exit code)
//
// Requires: bun, cargo with the wasm32-unknown-unknown target installed.

import { join, resolve } from 'node:path'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const SPIKE_DIR = join(REPO_ROOT, 'target/spike4')

function run(label: string, cmd: string, args: string[], cwd: string): void {
  console.log(`\n== ${label}: ${cmd} ${args.join(' ')}`)
  const r = Bun.spawnSync([cmd, ...args], { cwd, stdout: 'inherit', stderr: 'inherit' })
  if (r.exitCode !== 0) {
    console.error(`${label} failed (exit ${r.exitCode})`)
    process.exit(r.exitCode ?? 1)
  }
}

run('cargo test', 'cargo', ['test', '-p', 'dm-icon-core', '-p', 'dm-icon-wasm'], REPO_ROOT)
run('TS slice', 'bun', ['scripts/spike4-slice.ts'], REPO_ROOT)
run('native build+render', 'cargo', ['run', '--release', '-p', 'xtask', '--', 'spike4-native', SPIKE_DIR], REPO_ROOT)
run('wasm build', 'cargo', ['build', '--release', '-p', 'dm-icon-wasm', '--target', 'wasm32-unknown-unknown'], REPO_ROOT)
run('wasm render', 'bun', [join(import.meta.dir, 'wasm-render.ts'), SPIKE_DIR], REPO_ROOT)
run('compare', 'cargo', ['run', '--release', '-p', 'xtask', '--', 'spike4-compare', SPIKE_DIR], REPO_ROOT)
