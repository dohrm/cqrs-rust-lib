#!/usr/bin/env node

// publish-summary — the per-loop counterpart to `worktree-status.mjs`: that one is
// read-only across every worktree at a glance, this one is one worktree, once, at the
// moment a `/loop-setup` run stops (skills/loop-setup/SKILL.md).
//
// Mechanical, not LLM-authored: everything below is either parsed out of the state
// file the loop was already maintaining (`.work/<slug>/loop.md`, or the newest
// `.work/<slug>/tasks/NN-*.md`) or computed from git — branch, commits ahead of
// `base`, a one-line diffstat. `status` is the one thing only the loop knows, because
// it is why the loop stopped, not something derivable from the file.
//
// Report, not a gate: always exits 0 once there is something to summarize. Fails
// (exit 2) only on usage problems — bad args, no `.work/<slug>/`, no state file to
// read — the same two-layer validation `code-review`'s `reviewer` gets: the justfile's
// `case` guard fails fast for a human typo, this script's own check is what actually
// runs when a test calls it directly, past the justfile.
//
// Usage: just publish-summary slug=<slug> status=<COMPLETED|BLOCKED|BUDGET_EXHAUSTED>
//        (directly: node .dev/kit/common/publish-summary.mjs <slug> <status> [base]; default base origin/main)

import { execFileSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const STATUSES = ['COMPLETED', 'BLOCKED', 'BUDGET_EXHAUSTED']

function bail(msg) {
  console.error(`publish-summary: ${msg}`)
  process.exit(2)
}

const [, , slug, status, base = 'origin/main'] = process.argv
if (!slug) bail(`missing slug — usage: publish-summary <slug> <${STATUSES.join('|')}> [base]`)
if (!STATUSES.includes(status)) bail(`unknown status: ${status ?? '(missing)'} — one of ${STATUSES.join('|')}`)

/** Runs git, returning its stdout, or null when it exits non-zero (no repo, no base,
 *  no history) — a missing git fact is a `—` in the report, never a crash. */
function git(...argv) {
  try {
    return execFileSync('git', argv, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).replace(/\s+$/, '')
  } catch {
    return null
  }
}

/** The body of one `## Heading` section (case-insensitive), HTML comments stripped —
 *  same extractor `worktree-status.mjs`'s `blockedIn()` uses, re-implemented here
 *  rather than imported: gate scripts stay standalone, no import surface between them. */
function section(text, heading) {
  const re = new RegExp(`^##\\s+${heading}\\s*$`, 'mi')
  const start = text.search(re)
  if (start === -1) return null
  const rest = text.slice(start).replace(/^##[^\n]*\n/, '')
  return rest.split(/^##\s/m)[0].replace(/<!--[\s\S]*?-->/g, '').trim()
}

/** A template placeholder line (`- <turn>: <win or dead end>`, `- <blocker>`, …) is
 *  nothing but angle-bracket tokens and punctuation once the tokens are stripped. */
function isPlaceholder(line) {
  return line.replace(/<[^>]*>/g, '').replace(/[-*:\s]/g, '') === ''
}

function nonPlaceholderLines(body) {
  if (!body) return []
  return body.split('\n').map((l) => l.trim()).filter((l) => l && !isPlaceholder(l))
}

/** `.work/<slug>/loop.md` if present, else the newest `.work/<slug>/tasks/NN-*.md` —
 *  the same resolution order `/loop-setup` itself uses and `worktree-status.mjs`'s
 *  `worklists()` already sorts by. */
function findStateFile(slug) {
  const dir = join('.work', slug)
  if (!existsSync(dir)) return { dir, file: null }
  const loopFile = join(dir, 'loop.md')
  if (existsSync(loopFile)) return { dir, file: loopFile }
  let entries = []
  try {
    entries = readdirSync(join(dir, 'tasks')).filter((n) => n.endsWith('.md')).sort()
  } catch {
    entries = []
  }
  if (!entries.length) return { dir, file: null }
  return { dir, file: join(dir, 'tasks', entries[entries.length - 1]) }
}

const { dir, file } = findStateFile(slug)
if (!existsSync(dir)) bail(`no .work/${slug}/ directory`)
if (!file) bail(`no loop.md or tasks/*.md under .work/${slug}/`)

const text = readFileSync(file, 'utf8')

const objectiveMatch = text.match(/^-\s*\*\*Objective \(bounded\)\*\*:\s*(.+)$/m)
const h1Match = text.match(/^#\s+(.+)$/m)
const objective = (objectiveMatch?.[1] ?? h1Match?.[1] ?? '—').trim()

const guardrails = section(text, 'Guardrails')
const iterationCap = guardrails?.match(/\*\*Iteration cap\*\*:\s*(.+)/)?.[1]?.trim() ?? 'n/a'
const tokenBudget = guardrails?.match(/\*\*Token budget\*\*:\s*(.+)/)?.[1]?.trim() ?? 'n/a'

const checklistBody = section(text, 'Remaining work') ?? section(text, 'Tasks')
const items = (checklistBody ?? '')
  .split('\n')
  .map((l) => l.match(/^-\s*\[([ xX])\]\s*(.+)$/))
  .filter(Boolean)
  .map((m) => ({ done: m[1].trim() !== '', text: m[2].trim() }))
const doneCount = items.filter((i) => i.done).length

const blockedLines = nonPlaceholderLines(section(text, 'Blocked on the human'))
const logLines = nonPlaceholderLines(section(text, 'Log'))

const ahead = git('rev-list', '--count', `${base}..HEAD`)
const branch = git('rev-parse', '--abbrev-ref', 'HEAD')
const diffOut = git('diff', `${base}...HEAD`, '--stat')
const diffLine = diffOut ? diffOut.split('\n').pop().trim() : null

const body = [
  '<!-- Generated by `just publish-summary` — regenerated whole each run, never hand-edit. -->',
  `# Run summary — ${slug}`,
  '',
  `- **Status**: ${status}`,
  `- **Objective**: ${objective}`,
  `- **Branch**: ${branch ?? '—'} · **Ahead of ${base}**: ${ahead !== null ? `+${ahead}` : '—'}`,
  `- **Diff**: ${diffLine ?? '—'}`,
  `- **Turns logged**: ${logLines.length}`,
  `- **Generated**: ${new Date().toISOString()}`,
  '',
  '## Guardrails',
  '',
  `- **Iteration cap**: ${iterationCap}`,
  `- **Token budget**: ${tokenBudget}`,
  '',
  `## Remaining work (${doneCount}/${items.length} done)`,
  '',
  ...(items.length ? items.map((i) => `- [${i.done ? 'x' : ' '}] ${i.text}`) : ['— none —']),
  '',
  '## Blocked on the human',
  '',
  ...(blockedLines.length ? blockedLines.map((l) => (l.startsWith('-') ? l : `- ${l}`)) : ['— none —']),
  '',
].join('\n')

// Atomic write: `SUMMARY.tmp`, not `SUMMARY.md.tmp` — a temp name containing the final
// filename as a substring already tripped bash-guard's forge-detection once
// (review-report.md.part, see gate.just); named to avoid that from the start.
const tmp = join(dir, 'SUMMARY.tmp')
const final = join(dir, 'SUMMARY.md')
writeFileSync(tmp, body)
renameSync(tmp, final)
