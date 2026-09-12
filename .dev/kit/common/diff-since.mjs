#!/usr/bin/env node

// How much of this branch a Tier-3 gate has ALREADY cleared, and from where the next
// run should therefore diff.
//
// The problem it solves is measured, not theoretical: `code-review` and `mutate-diff`
// both read `git diff <base>...HEAD`, so on a branch built by successive loops every
// run re-reviews and re-mutates everything the previous ones already cleared. The
// tenth block pays for the nine before it — minutes of mutants, and a review prompt
// that eventually trips `review_max_bytes` for a reason that has nothing to do with
// the block under review.
//
// So each gate keeps ONE line of state: `.work/<slug>/.latest_<kind>`, the commit it
// last cleared. Per developer, per worktree, never shared — a teammate's HEAD is not
// yours, and neither is their verdict (rules/agent/autonomy.md, "One tree, one
// writer"). It is written into `.work/` because that is already the gates' scratch
// area, and this script keeps `.work/.gitignore` carrying the two patterns, so the
// marker stays private even in a repo that COMMITS its `.work/` plans.
//
//   node diff-since.mjs <review|mutate> --base <ref> [--slug <s>] [--incremental 0|1]
//       prints, on stdout and alone, the ref to diff FROM. Everything else is stderr,
//       because the caller reads this inside `$( )`.
//   node diff-since.mjs <review|mutate> --mark <sha> [--slug <s>]
//       records <sha> as cleared. Callers run it AFTER the gate passed, never before.
//
// It never fails closed on its own state: every way the marker can be wrong — absent,
// unparseable, a commit that no longer exists, a commit that is not an ancestor of
// HEAD (a rebase, an amend, a branch switch in the same tree) — falls back to `base`,
// i.e. to today's behaviour. A stale marker costs one full pass; a marker trusted
// blindly hides a rewritten commit from every future review, which is the failure
// this gate exists to prevent.

import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const KINDS = ['review', 'mutate']
const SHA = /^[0-9a-f]{7,40}$/
const OFF = new Set(['0', 'false', 'no', 'off'])

const argv = process.argv.slice(2)
const flag = (name, dflt = '') => {
  const i = argv.indexOf(`--${name}`)
  return i === -1 ? dflt : (argv[i + 1] ?? dflt)
}
const given = (name) => argv.includes(`--${name}`)

/** Runs git, returning its stdout, or null when it exits non-zero. */
function git(...args) {
  try {
    return execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()
  } catch {
    return null
  }
}
/** Runs git for its exit code alone (`merge-base --is-ancestor` answers that way). */
function gitOk(...args) {
  try {
    execFileSync('git', args, { stdio: 'ignore' })
    return true
  } catch {
    return false
  }
}

const note = (msg) => console.error(`diff-since: ${msg}`)
const short = (sha) => sha.slice(0, 7)

// A slug reaches this script from a justfile variable or a branch name, and it becomes
// a PATH. Anything outside the safe set collapses to `-`, which also flattens the `/`
// of `feat/thing` and makes `..` unwritable — a branch cannot contain `..`, an
// overridden `work_slug` can.
const sanitize = (s) => s.replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^[-.]+/, '') || 'detached'

const kind = argv[0]
if (!KINDS.includes(kind)) {
  console.error(`usage: diff-since.mjs <${KINDS.join('|')}> --base <ref> [--slug <s>] [--incremental 0|1]`)
  console.error(`       diff-since.mjs <${KINDS.join('|')}> --mark <sha> [--slug <s>]`)
  process.exit(2)
}

const slug = sanitize(flag('slug').trim() || git('symbolic-ref', '--quiet', '--short', 'HEAD') || 'detached')
const dir = join('.work', slug)
const marker = join(dir, `.latest_${kind}`)

// `.work/` is gitignored in most installs and committed in some (that is why
// `review_exclude` drops it from the diff). The marker must be private either way, so
// the patterns are written where they hold in both cases, and only what is missing is
// appended — a `.gitignore` the repo already owns is never rewritten.
const PATTERNS = ['*/.latest_review', '*/.latest_mutate']
function ensureIgnored() {
  const file = join('.work', '.gitignore')
  const have = existsSync(file) ? readFileSync(file, 'utf8') : null
  const lines = have === null ? [] : have.split(/\r?\n/)
  const missing = PATTERNS.filter((p) => !lines.includes(p))
  if (missing.length === 0) return
  const header = have === null
    ? '# Per-developer gate markers: what THIS clone has already reviewed / mutated.\n'
      + '# Never shared — a teammate\'s HEAD is not yours, and neither is their verdict.\n'
    : ''
  writeFileSync(file, (have === null ? '' : have.replace(/\n*$/, '\n')) + header + missing.join('\n') + '\n')
}

if (given('mark')) {
  const sha = git('rev-parse', '--verify', `${flag('mark')}^{commit}`)
  if (!sha) {
    note(`not a commit: ${flag('mark') || '(missing)'} — nothing recorded.`)
    process.exit(2)
  }
  mkdirSync(dir, { recursive: true })
  writeFileSync(marker, `${sha}\n`)
  ensureIgnored()
  console.log(`${kind}: cleared through ${short(sha)} — recorded in ${marker}`)
  process.exit(0)
}

const base = flag('base', 'origin/main')
/** Today's behaviour: the whole branch. Printed alone, on stdout. */
function whole(why) {
  if (why) note(`${why} — diffing the whole branch (${base}...HEAD).`)
  console.log(base)
  process.exit(0)
}

if (OFF.has(flag('incremental', '1'))) whole('incremental turned off')
if (!existsSync(marker)) whole(`no ${marker} yet`)

const stored = readFileSync(marker, 'utf8').trim()
if (!SHA.test(stored)) whole(`${marker} does not hold a commit sha`)
if (!git('rev-parse', '--verify', `${stored}^{commit}`)) whole(`${short(stored)} is gone (rebased? amended?)`)
if (!gitOk('merge-base', '--is-ancestor', stored, 'HEAD')) whole(`${short(stored)} is not an ancestor of HEAD (rebase, or another branch)`)

note(`${kind}: already cleared through ${short(stored)} — diffing ${short(stored)}...HEAD.`)
console.log(stored)
