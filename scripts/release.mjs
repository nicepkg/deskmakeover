#!/usr/bin/env bun
// Cut a release in one command. Single-sources the version across the three places it
// lives — package.json, src-tauri/tauri.conf.json, and the workspace Cargo.toml — then
// optionally commits, tags, and pushes. Pushing a `v*` tag triggers the signed release
// workflow (.github/workflows/release.yml → docs/signing-setup.md).
//
// Cross-platform (Bun, per the repo rule — no bash/ps1 duplication).
//
//   bun scripts/release.mjs 0.2.0                     # set exact version in all three files
//   bun scripts/release.mjs patch | minor | major     # bump from the current version
//   bun scripts/release.mjs 0.2.0 --commit            # ...and `git commit` the bump
//   bun scripts/release.mjs 0.2.0 --commit --tag      # ...and create the v0.2.0 tag
//   bun scripts/release.mjs 0.2.0 --commit --tag --push   # ...and push main + the tag (fires CI release)
//
// With no flags it only rewrites the files, so you can review the diff before committing.

import { readFileSync, writeFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const PKG = join(ROOT, 'package.json')
const TAURI = join(ROOT, 'src-tauri', 'tauri.conf.json')
const CARGO = join(ROOT, 'Cargo.toml')

const SEMVER = /^\d+\.\d+\.\d+$/

function die(msg) {
  console.error(`\x1b[31m✗ ${msg}\x1b[0m`)
  process.exit(1)
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function currentVersion() {
  return readJson(PKG).version
}

function bump(current, kind) {
  const [maj, min, pat] = current.split('.').map(Number)
  if (kind === 'major') return `${maj + 1}.0.0`
  if (kind === 'minor') return `${maj}.${min + 1}.0`
  if (kind === 'patch') return `${maj}.${min}.${pat + 1}`
  return null
}

// Targeted replace of the top-level "version" only — a full parse/stringify would reformat
// the file's hand-tuned inline arrays/objects (tauri.conf.json). Matches the first semver
// "version" value; both files have exactly one.
function setJsonVersion(path, version) {
  const raw = readFileSync(path, 'utf8')
  let from
  const next = raw.replace(/"version":(\s*)"(\d+\.\d+\.\d+[^"]*)"/, (_, ws, v) => {
    from = v
    return `"version":${ws}"${version}"`
  })
  if (from === undefined) die(`no semver "version" key in ${path}`)
  writeFileSync(path, next)
  return from
}

// The version lives under [workspace.package]; replace only that table's `version = "..."`.
function setCargoWorkspaceVersion(path, version) {
  const raw = readFileSync(path, 'utf8')
  const anchor = '[workspace.package]'
  const at = raw.indexOf(anchor)
  if (at < 0) die(`no [workspace.package] table in ${path}`)
  const head = raw.slice(0, at)
  const tail = raw.slice(at)
  let from
  const next = tail.replace(/version\s*=\s*"([^"]*)"/, (_, v) => {
    from = v
    return `version = "${version}"`
  })
  if (from === undefined) die(`no version key under [workspace.package] in ${path}`)
  writeFileSync(path, head + next)
  return from
}

function git(...args) {
  return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim()
}

// ── main ──
const args = process.argv.slice(2)
const flags = new Set(args.filter((a) => a.startsWith('--')))
const positional = args.filter((a) => !a.startsWith('--'))
const target = positional[0]

if (!target) die('usage: bun scripts/release.mjs <version | patch | minor | major> [--commit] [--tag] [--push]')

const current = currentVersion()
let version
if (SEMVER.test(target)) version = target
else if (['patch', 'minor', 'major'].includes(target)) version = bump(current, target)
else die(`not a version or bump kind: "${target}" (expected X.Y.Z or patch|minor|major)`)

if (version === current) die(`version is already ${version}`)

const froms = {
  'package.json': setJsonVersion(PKG, version),
  'src-tauri/tauri.conf.json': setJsonVersion(TAURI, version),
  'Cargo.toml [workspace.package]': setCargoWorkspaceVersion(CARGO, version),
}

console.log(`\x1b[36mDeskMakeover → v${version}\x1b[0m  (was ${current})`)
for (const [file, from] of Object.entries(froms)) {
  const note = from === current ? '' : `  \x1b[33m(was ${from} — drift corrected)\x1b[0m`
  console.log(`  ${file}: ${from} → ${version}${note}`)
}

const tag = `v${version}`

if (flags.has('--commit')) {
  git('add', 'package.json', 'src-tauri/tauri.conf.json', 'Cargo.toml', 'Cargo.lock', 'bun.lock')
  git('commit', '-m', `chore(release): ${tag}`)
  console.log(`\x1b[32m✓ committed\x1b[0m chore(release): ${tag}`)
}

if (flags.has('--tag')) {
  if (!flags.has('--commit')) {
    console.log('\x1b[33m! --tag without --commit: the tag will point at HEAD, which does not yet contain the version bump. Commit first.\x1b[0m')
  }
  git('tag', tag)
  console.log(`\x1b[32m✓ tagged\x1b[0m ${tag}`)
}

if (flags.has('--push')) {
  if (!flags.has('--tag')) die('--push needs --tag (nothing to push but the branch)')
  const branch = git('rev-parse', '--abbrev-ref', 'HEAD')
  git('push', 'origin', branch, tag)
  console.log(`\x1b[32m✓ pushed\x1b[0m ${branch} + ${tag} → the signed release workflow will run`)
}

if (!flags.has('--commit') && !flags.has('--tag') && !flags.has('--push')) {
  console.log('\nNext:')
  console.log(`  git add -A && git commit -m "chore(release): ${tag}"`)
  console.log(`  git tag ${tag} && git push origin main ${tag}`)
  console.log('  (or re-run with --commit --tag --push to do it in one shot)')
}
