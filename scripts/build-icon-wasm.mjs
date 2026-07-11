#!/usr/bin/env node
// Build dm-icon-wasm for wasm32 and copy the release artifact into public/,
// where vite serves it at /dm_icon_wasm.wasm (dev + dist). Run when the Rust
// icon core changes and the WASM preview path is in use. The copied .wasm is a
// build artifact (gitignored). Cross-platform (node only, no shell).

import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const ARTIFACT = join(ROOT, 'target', 'wasm32-unknown-unknown', 'release', 'dm_icon_wasm.wasm')
const OUT = join(ROOT, 'public', 'dm_icon_wasm.wasm')

execFileSync('cargo', ['build', '--target', 'wasm32-unknown-unknown', '--release', '-p', 'dm-icon-wasm'], {
  cwd: ROOT,
  stdio: 'inherit',
})
mkdirSync(dirname(OUT), { recursive: true })
copyFileSync(ARTIFACT, OUT)
console.log(`icon wasm → public/dm_icon_wasm.wasm`)
