#!/usr/bin/env node

// worktree-status — the aggregator over parallel work trees. NOT a gate.
//
// The loop's proof is local to the tree: `.work/review-report.md` is ONE file per
// worktree, `review-guard` reads the one in the tree it runs in, and
// `.work/<slug>/` is one work unit. That is why parallel sessions get
// parallel trees (rules/agent/autonomy.md, "One tree, one writer") — and why, once
// they do, nothing shows you all of them at once. This does.
//
// One line per worktree: branch · commits ahead of base · dirty files · worklist ·
// review verdict + staleness. Then, indented under it, whatever that worklist's
// `## Blocked on the human` section holds — the escalation channel `/rfc` and
// `/loop-setup` already define, which is worth nothing if nobody walks the trees to read it.
//
// It ALWAYS exits 0, and it is read-only. A dashboard that fails is a dashboard
// nobody runs, and the verdict printed here is a REPORT of a gate, never the gate:
// that stays `just review-guard`, in each tree, at push time. review-guard is the
// authority on a report — this echoes what it would say and prints `malformed`
// rather than guessing when it cannot parse one.
//
// Usage: just status   (directly: node .dev/kit/common/worktree-status.mjs [base-ref]; default origin/main)

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

const base = process.argv.slice(2).find((a) => !a.startsWith('--')) ?? 'origin/main'

/** Runs git in `cwd`, returning its stdout, or null when it exits non-zero. */
function git(cwd, ...argv) {
  try {
    return execFileSync('git', argv, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).replace(/\s+$/, '')
  } catch {
    return null
  }
}

// The report contract (kit/common/review-guard.mjs): two markers at the very end,
// read LAST-wins so a contract quoted mid-report is prose. Duplicated here on
// purpose — the guard must stay a standalone gate with no import surface, and this
// file must stay a reporter with no verdict of its own.
const VERDICTS = ['CLEAN', 'WARNINGS', 'CRITICAL']
const VERDICT_MARKER = /^<!--\s*CI_VERDICT:\s*(.*?)\s*-->\s*$/gm
const REVIEWED_MARKER = /^<!--\s*REVIEWED:\s*(.*?)\s*-->\s*$/gm
const SHA = /^[0-9a-f]{7,40}$/

const lastMatch = (re, text) => {
  re.lastIndex = 0
  let m, out = null
  while ((m = re.exec(text)) !== null) out = m[1]
  return out
}

/** What `just review-guard` would say in this tree, as a string for the eye. */
function reviewState(tree) {
  const file = join(tree.path, '.work', 'review-report.md')
  if (!existsSync(file)) return 'no report'
  let text
  try {
    text = readFileSync(file, 'utf8')
  } catch {
    return 'unreadable'
  }
  const verdict = lastMatch(VERDICT_MARKER, text)
  const sha = lastMatch(REVIEWED_MARKER, text)
  if (!VERDICTS.includes(verdict) || !sha || !SHA.test(sha)) return 'malformed → blocks'
  // A CRITICAL blocks whatever the sha, so its staleness is not worth printing.
  if (verdict === 'CRITICAL') return 'CRITICAL → blocks'
  const describesHead = tree.head && (tree.head.startsWith(sha) || sha.startsWith(tree.head))
  if (describesHead) return verdict
  const since = git(tree.path, 'rev-list', '--count', `${sha}..HEAD`)
  return `${verdict} (stale${since ? `, +${since}` : ''})`
}

/** The lines of a worklist's `## Blocked on the human` section that are really there:
 *  no HTML comments, no `- <placeholder>` left over from the template. */
function blockedIn(file) {
  let text
  try {
    text = readFileSync(file, 'utf8')
  } catch {
    return []
  }
  const start = text.search(/^##\s+Blocked on the human\s*$/mi)
  if (start === -1) return []
  const rest = text.slice(start).replace(/^##[^\n]*\n/, '')
  const section = rest.split(/^##\s/m)[0].replace(/<!--[\s\S]*?-->/g, '')
  return section
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l && !/^[-*]?\s*<[^>]*>$/.test(l))
}

const RANK = ['defect.md', 'loop.md', 'rfc.md']

/** Every state file an agent escalates in, under `.work/<slug>/`, ranked:
 *  `defect.md` from `/defect`, then `loop.md` from `/loop-setup`, then `rfc.md` from `/rfc`. Within one
 *  slug the highest rank comes last and is the one shown: an RFC takes over the bug or
 *  loop it came from. */
function worklists(root) {
  const found = []
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue
    const slug = entry.name
    const add = (rel) =>
      found.push({ slug, file: rel, label: `${slug}/${rel.replace(/\.md$/, '')}`, path: join(root, slug, ...rel.split('/')) })
    for (const name of RANK) if (existsSync(join(root, slug, name))) add(name)
  }
  // By slug, then by rank — never by the spelling of the file name.
  return found.sort((a, b) => a.slug.localeCompare(b.slug) || RANK.indexOf(a.file) - RANK.indexOf(b.file))
}

/** The newest worklist in a tree, plus what every slug's winning worklist says is
 *  blocked: two works in one tree each keep their escalation visible. */
function worklistState(tree) {
  let found = []
  try {
    found = worklists(join(tree.path, '.work'))
  } catch {
    return { label: '—', blocked: [] }
  }
  if (!found.length) return { label: '—', blocked: [] }
  const newest = found[found.length - 1]
  const label = newest.label + (found.length > 1 ? ` (+${found.length - 1})` : '')
  const winners = new Map(found.map((w) => [w.slug, w]))
  const blocked = [...winners.values()].flatMap((w) =>
    blockedIn(w.path).map((line) => (w === newest ? line : `${w.label}: ${line.replace(/^[-*]\s*/, '')}`)))
  return { label, blocked }
}

const listed = git(process.cwd(), 'worktree', 'list', '--porcelain')
if (listed === null) {
  console.log('worktree-status: not a git repository (or git is not on PATH) — nothing to aggregate.')
  process.exit(0)
}

const trees = []
for (const block of listed.split(/\n{2,}/)) {
  const tree = {}
  for (const line of block.split('\n')) {
    const sep = line.indexOf(' ')
    const key = sep === -1 ? line : line.slice(0, sep)
    const value = sep === -1 ? '' : line.slice(sep + 1)
    if (key === 'worktree') tree.path = value
    else if (key === 'HEAD') tree.head = value
    else if (key === 'branch') tree.branch = value.replace(/^refs\/heads\//, '')
    else if (key === 'detached') tree.detached = true
    else if (key === 'bare') tree.bare = true
    else if (key === 'prunable') tree.prunable = true
    else if (key === 'locked') tree.locked = true
  }
  // A bare repo is not a working tree: nobody writes in it, so it has nothing to report.
  if (tree.path && !tree.bare) trees.push(tree)
}

/** How to name a tree so the reader can act on it: relative when it is below the
 *  current directory, absolute the moment it is not. */
function shownPath(tree) {
  const rel = relative(process.cwd(), tree.path) || '.'
  return rel.startsWith('..') ? resolve(tree.path) : rel
}

const here = git(process.cwd(), 'rev-parse', '--show-toplevel')
const rows = trees.map((tree) => {
  const ahead = git(tree.path, 'rev-list', '--count', `${base}..HEAD`)
  const porcelain = git(tree.path, 'status', '--porcelain')
  const dirty = porcelain === null ? null : porcelain.split('\n').filter((l) => l.trim()).length
  const worklist = worklistState(tree)
  return {
    mine: here !== null && resolve(tree.path) === resolve(here),
    // Relative while the tree is under the cwd, ABSOLUTE once it is not. Worktrees
    // live under `worktree_root` now (gate.just), so the relative form was a
    // `../../.worktrees/<repo>/<slug>` nobody can paste into an IDE or an orchestrator.
    path: shownPath(tree) + (tree.prunable ? ' (prunable)' : tree.locked ? ' (locked)' : ''),
    branch: tree.branch ?? `(detached ${(tree.head ?? '').slice(0, 7)})`,
    ahead: ahead === null ? '—' : `+${ahead}`,
    dirty: dirty === null ? '?' : dirty === 0 ? 'clean' : `${dirty} dirty`,
    worklist: worklist.label,
    review: reviewState(tree),
    blocked: worklist.blocked,
  }
})

const pad = (s, w) => s + ' '.repeat(Math.max(0, w - s.length))
const width = (key) => Math.max(...rows.map((r) => r[key].length))
const [wPath, wBranch, wAhead, wDirty, wWorklist] = ['path', 'branch', 'ahead', 'dirty', 'worklist'].map(width)

console.log(
  `${rows.length} worktree${rows.length === 1 ? '' : 's'} · base ${base} · ` +
  'this REPORTS; `just review-guard` is what gates, in each tree',
)
for (const r of rows) {
  console.log(
    `${r.mine ? '*' : ' '} ${pad(r.path, wPath)}  ${pad(r.branch, wBranch)}  ` +
    `${pad(r.ahead, wAhead)}  ${pad(r.dirty, wDirty)}  ${pad(r.worklist, wWorklist)}  ${r.review}`,
  )
  for (const line of r.blocked) console.log(`    BLOCKED: ${line.replace(/^[-*]\s*/, '')}`)
}

const waiting = rows.filter((r) => r.blocked.length).length
if (waiting) console.log(`\n${waiting} tree(s) waiting on you — answer in the tree, not here.`)
