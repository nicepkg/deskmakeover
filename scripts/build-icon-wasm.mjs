#!/usr/bin/env node
// Build dm-icon-wasm for wasm32 and copy the release artifact into public/,
// where vite serves it at /dm_icon_wasm.wasm (dev + dist). Run when the Rust
// icon core changes and the WASM preview path is in use. The copied .wasm is a
// build artifact (gitignored). Cross-platform (node only, no shell).
//
// Built with `--features fast`: the shipped preview artifact carries the M6
// geometry-mask cache (byte-identical to the scalar reference — the four-way
// 1487-cell cert enforces it). Without this flag the crate's default (`[]`) is
// the scalar recompute kernel, so the artifact would ship WITHOUT any Phase 1
// speedup even though the fast kernel exists.

import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const ARTIFACT = join(ROOT, 'target', 'wasm32-unknown-unknown', 'release', 'dm_icon_wasm.wasm')
const OUT = join(ROOT, 'public', 'dm_icon_wasm.wasm')

execFileSync('cargo', ['build', '--target', 'wasm32-unknown-unknown', '--release', '-p', 'dm-icon-wasm', '--features', 'fast'], {
  cwd: ROOT,
  stdio: 'inherit',
})
mkdirSync(dirname(OUT), { recursive: true })
copyFileSync(ARTIFACT, OUT)
console.log(`icon wasm → public/dm_icon_wasm.wasm`)
