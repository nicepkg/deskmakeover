#!/usr/bin/env bun
// Cut a release in one command. Single-sources the BUNDLE version across the two files that carry
// it — package.json and src-tauri/tauri.conf.json — then optionally commits, tags, and pushes.
// Pushing a `v*` tag triggers the signed release workflow (.github/workflows/release.yml →
// docs/signing-setup.md). The Cargo workspace version is deliberately NOT touched: crates are
// publish=false and the bundle version is owned by tauri.conf.json (see docs/STATE.md).
//
// Cross-platform (Bun, per the repo rule — no bash/ps1 duplication).
//
//   bun scripts/release.mjs 0.2.0                     # set exact version in the two files (review the diff)
//   bun scripts/release.mjs patch | minor | major     # bump from the current version
//   bun scripts/release.mjs 0.2.0 --commit            # ...and `git commit` the bump
//   bun scripts/release.mjs 0.2.0 --commit --tag      # ...and create the v0.2.0 tag
//   bun scripts/release.mjs 0.2.0 --commit --tag --push   # ...and push main + the tag (fires CI release)
//   bun scripts/release.mjs 0.1.0 --tag --push        # version already 0.1.0 → tag the current clean HEAD
//
// Any git action (--commit/--tag/--push) first runs preflight: clean worktree, branch main, and the
// target tag must not already exist. It commits ONLY the version files, never unrelated staged work.

import { readFileSync, writeFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const PKG = join(ROOT, 'package.json')
const TAURI = join(ROOT, 'src-tauri', 'tauri.conf.json')

const SEMVER = /^\d+\.\d+\.\d+$/

function die(msg) {
  console.error(`\x1b[31m✗ ${msg}\x1b[0m`)
  process.exit(1)
}

function currentVersion() {
  return JSON.parse(readFileSync(PKG, 'utf8')).version
}

function bump(current, kind) {
  const [maj, min, pat] = current.split('.').map(Number)
  if (kind === 'major') return `${maj + 1}.0.0`
  if (kind === 'minor') return `${maj}.${min + 1}.0`
  if (kind === 'patch') return `${maj}.${min}.${pat + 1}`
  return null
}

// Targeted replace of the top-level "version" only — a full parse/stringify would reformat the
// file's hand-tuned inline arrays/objects (tauri.conf.json). Both files have exactly one semver.
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

function git(...args) {
  return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8' }).trim()
}
function gitQuiet(...args) {
  return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim()
}

// ── main ──
const args = process.argv.slice(2)
const flags = new Set(args.filter((a) => a.startsWith('--')))
const positional = args.filter((a) => !a.startsWith('--'))
const target = positional[0]
const wantCommit = flags.has('--commit')
const wantTag = flags.has('--tag')
const wantPush = flags.has('--push')

if (!target) die('usage: bun scripts/release.mjs <version | patch | minor | major> [--commit] [--tag] [--push]')
if (wantPush && !wantTag) die('--push needs --tag (nothing to push but the branch)')

const current = currentVersion()
let version
if (SEMVER.test(target)) version = target
else if (['patch', 'minor', 'major'].includes(target)) version = bump(current, target)
else die(`not a version or bump kind: "${target}" (expected X.Y.Z or patch|minor|major)`)

const bumping = version !== current
const gitAction = wantCommit || wantTag || wantPush
const tag = `v${version}`

// Preflight BEFORE writing anything, whenever a git action is requested (finding A6/A2).
if (gitAction) {
  const branch = git('rev-parse', '--abbrev-ref', 'HEAD')
  if (branch !== 'main') die(`refusing to release from branch "${branch}" — switch to main`)
  if (git('status', '--porcelain') !== '') die('working tree is not clean — commit or stash first')
  if (git('tag', '--list', tag) !== '') die(`tag ${tag} already exists locally`)
  let remoteTag = ''
  try {
    remoteTag = git('ls-remote', '--tags', 'origin', tag)
  } catch {
    /* offline is fine — the local check already ran; the push would fail loudly on a dup */
  }
  if (remoteTag !== '') die(`tag ${tag} already exists on origin`)
}

if (!bumping) {
  // No file change: only meaningful for tagging an already-versioned, clean HEAD (the first release
  // at 0.1.0 lands here — finding A5). A commit would be empty, so skip it even if --commit was passed.
  if (!wantTag) die(`version is already ${version} — pass --tag to tag the current commit, or give a new version`)
  console.log(`\x1b[36mDeskMakeover\x1b[0m already at v${version} — tagging the current HEAD (no version bump)`)
  if (wantCommit) console.log('  (--commit ignored: nothing to commit)')
} else {
  if (wantTag && !wantCommit) die('--tag needs --commit when bumping (the tag must point at the version-bump commit)')
  const fromPkg = setJsonVersion(PKG, version)
  const fromTauri = setJsonVersion(TAURI, version)
  console.log(`\x1b[36mDeskMakeover → v${version}\x1b[0m  (was ${current})`)
  console.log(`  package.json: ${fromPkg} → ${version}`)
  console.log(`  src-tauri/tauri.conf.json: ${fromTauri} → ${version}`)
  if (wantCommit) {
    // Stage ONLY the version files (+ bun.lock if the bump touched it) — never sweep unrelated work.
    const staged = ['package.json', 'src-tauri/tauri.conf.json']
    if (git('status', '--porcelain', 'bun.lock') !== '') staged.push('bun.lock')
    git('add', ...staged)
    gitQuiet('commit', '-m', `chore(release): ${tag}`)
    console.log(`\x1b[32m✓ committed\x1b[0m chore(release): ${tag}`)
  }
}

if (wantTag) {
  gitQuiet('tag', tag)
  console.log(`\x1b[32m✓ tagged\x1b[0m ${tag}`)
}

if (wantPush) {
  gitQuiet('push', 'origin', 'main', tag)
  console.log(`\x1b[32m✓ pushed\x1b[0m main + ${tag} → the signed release workflow will run`)
}

if (!gitAction) {
  console.log('\nNext:')
  console.log(`  git add package.json src-tauri/tauri.conf.json && git commit -m "chore(release): ${tag}"`)
  console.log(`  git tag ${tag} && git push origin main ${tag}`)
  console.log('  (or re-run with --commit --tag --push to do it in one shot)')
}
