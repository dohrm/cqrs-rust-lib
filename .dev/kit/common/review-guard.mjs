#!/usr/bin/env node

// review-guard — the deterministic half of a code review.
//
// A review is an LLM's judgment: it can be persuaded, and it can be re-run until
// it says something nicer. So the review itself is never the gate. What IS a gate
// is this: a report on disk, two machine-readable markers at its end, and one rule
// no prose can talk its way around — a CRITICAL blocks the push until a NEW review
// says otherwise. See rules/agent/autonomy.md.
//
// The report (.work/review-report.md, gitignored) must end with:
//   <!-- CI_VERDICT: CRITICAL|WARNINGS|CLEAN -->
//   <!-- REVIEWED: <full sha of HEAD at review time> -->
//
// | report state                        | verdict | why                                     |
// |-------------------------------------|---------|-----------------------------------------|
// | absent                              | pass +  | same contract as mutate-diff: a missing |
// |                                     | notice  | step is DECLARED, never simulated       |
// | CLEAN/WARNINGS, sha = HEAD          | pass    |                                         |
// | CLEAN/WARNINGS, sha ≠ HEAD          | pass +  | trivial commits after a review must not |
// |                                     | stale   | cost a whole new review                 |
// | CRITICAL, any sha                   | BLOCK   | otherwise one more commit expires a     |
// |                                     |         | CRITICAL — the hole this closes         |
// | markers unreadable                  | BLOCK   | a malformed report is a falsifiable one |
// `--expect-files <list>` is the recipe's completeness check on the TEMP report, before
// it is promoted — see "Completeness" below. The pre-push hook never passes it.
// The markers are read LAST-wins, so a contract quoted mid-report is prose, not a
// second verdict — and a report that signs off with a few lines of prose after them
// still parses. A fixed-size tail window did neither reliably.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'

const VERDICTS = ['CLEAN', 'WARNINGS', 'CRITICAL']
const VERDICT_MARKER = /^<!--\s*CI_VERDICT:\s*(.*?)\s*-->\s*$/gm
const REVIEWED_MARKER = /^<!--\s*REVIEWED:\s*(.*?)\s*-->\s*$/gm
const SHA = /^[0-9a-f]{7,40}$/

const args = process.argv.slice(2)
const flagValue = (name) => {
  const i = args.indexOf(name)
  return i === -1 ? null : (args[i + 1] ?? '')
}
// `--expect-files <list>` takes a value, so the report is the first positional that is
// neither a flag nor that value.
const expectFiles = flagValue('--expect-files')
const reportPath = args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--expect-files')
  ?? '.work/review-report.md'
// `code-review` passes it; the pre-push hook must NOT. "No report" is a legitimate
// answer to "may I push?" (declared, never simulated) but never to "did the review
// I just ran produce a verdict?" — and that second question is what advances the
// `.latest_review` marker, so a CLI that exits 0 having written an empty file would
// otherwise mark code nobody read as cleared.
const requireReport = args.includes('--require-report') || args.includes('--expect-files')

const RERUN = 'Run `just code-review` to produce a fresh one.'

/** Runs git, returning its stdout, or null when it exits non-zero. */
function git(...argv) {
  try {
    return execFileSync('git', argv, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()
  } catch {
    return null
  }
}

const short = (sha) => sha.slice(0, 7)
const samesha = (a, b) => a.startsWith(b) || b.startsWith(a)

/** The LAST marker of its kind, or null when there is none. The report contract says
 *  the markers go at the VERY END, so anything earlier is prose ABOUT markers — a fix
 *  suggestion quoting the contract, which happens exactly when the diff touches the
 *  prompt or the reviewer agent. Reading a fixed-size TAIL was the first attempt at
 *  that rule and it broke both ways: a review that signs off with a few lines of prose
 *  pushed its own markers out of the window and blocked every push as "malformed",
 *  while a quote sitting next to the real markers stayed inside it and still counted.
 *  Last-wins is what the contract promises, so it is what runs. */
function lastMarker(text, marker) {
  const all = [...text.matchAll(marker)].map((m) => m[1])
  return all.length === 0 ? null : all[all.length - 1]
}

/** Every reason the markers cannot be trusted — empty when the report is readable. */
function malformations(verdict, sha) {
  const out = []
  if (verdict === null) out.push('no `<!-- CI_VERDICT: ... -->` marker.')
  else if (!VERDICTS.includes(verdict))
    out.push(`CI_VERDICT is "${verdict}" — expected one of ${VERDICTS.join(', ')}.`)

  if (sha === null) out.push('no `<!-- REVIEWED: <sha> -->` marker — the report does not say what it reviewed.')
  else if (!SHA.test(sha)) out.push(`REVIEWED is "${sha}" — expected a commit sha.`)
  return out
}

// ── Completeness (`--expect-files`) ─────────────────────────────────────────
// A verdict over files the reviewer never looked at is the failure an agent produces on
// a large diff: it reviews some files, skips others, and still signs CLEAN. So the
// report names every file it reviewed (`### Files reviewed`), and each finding points at
// a `path:line` that exists. Checked by the recipe on the TEMP report, before it is
// promoted — on the promoted one, a pre-push run would already have let an incomplete
// CLEAN through. Exit codes, for the recipe:
//   0  complete — promote it
//   1  incomplete — do not promote: the previous report stands, the marker stays
//   3  incomplete, but CRITICAL or with unreadable markers — promote it anyway, then
//      fail: a CRITICAL is never hidden, and a truncated report still BLOCKS as
//      malformed instead of leaving an older CLEAN standing
// `--expect-files` implies `--require-report`: an empty temp report is never promoted.
//
// A Location is `path:line` or `path:start-end`; a trailing `:col` is ignored, `**Location:**`
// and an en-dash range are accepted, `\` separators are read as `/`. A Location that does
// not parse is not checked — a floor, not a parser of every way a model writes a place.
const LOCATION = /\*\*Location:?\*\*:?[ \t]*`?([^\s`:]+?):(\d+)(?:[-–](\d+))?(?::\d+)?`?(?=[\s·,.;)]|$)/g

/** The text of the `### Files reviewed` section, or null when there is none. */
function filesReviewed(text) {
  const m = text.match(/^###\s+Files reviewed\s*$([\s\S]*?)(?=^#{2,3}\s|^<!--|(?![\s\S]))/m)
  return m ? m[1] : null
}

/** Whether `section` names `path` as a whole path: `src/lib.rs` must not be "named" by
 *  `crates/x/src/lib.rs`, nor `a.rs` by `data.rs`. */
function names(section, path) {
  const escaped = path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return new RegExp(`(?<![\\p{L}\\p{N}_./-])(?:\\./)?${escaped}(?![\\p{L}\\p{N}_/-]|\\.[\\p{L}\\p{N}_])`, 'u')
    .test(section)
}

/** The number of lines of `path` as of HEAD, or null when it does not exist there.
 *  Untrimmed and unbounded: blank edges count, and a large file is not "missing". */
function linesAtHead(path) {
  let body
  try {
    body = execFileSync('git', ['cat-file', '-p', `HEAD:${path}`],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], maxBuffer: 1 << 30 })
  } catch {
    return null
  }
  if (body === '') return 0
  return body.split('\n').length - (body.endsWith('\n') ? 1 : 0)
}

/** Why a `path:line` does not exist as of HEAD, or null when it does. */
function badLocation(path, start, end) {
  const lines = linesAtHead(path)
  if (lines === null) return `${path} does not exist as of HEAD`
  const last = Number(end ?? start)
  if (Number(start) < 1 || last > lines || last < Number(start))
    return `${path}:${start}${end ? `-${end}` : ''} is outside the file (${lines} lines)`
  return null
}

function completeness(text, listPath) {
  if (!existsSync(listPath)) {
    console.error(`review-guard: no file list at ${listPath} — nothing to check the report against.`)
    return 1
  }
  // NUL-separated (`git diff --name-only -z`): a path may hold any byte but NUL.
  const expected = readFileSync(listPath, 'utf8').split('\0').filter(Boolean)
  const problems = []
  const section = filesReviewed(text)
  if (section === null) problems.push('no `### Files reviewed` section.')
  else for (const f of expected) if (!names(section, f)) problems.push(`Files reviewed does not name ${f}.`)
  for (const [, path, start, end] of text.matchAll(LOCATION)) {
    const why = badLocation(path.replaceAll('\\', '/'), start, end)
    if (why) problems.push(`Location ${why}.`)
  }
  if (problems.length === 0) return 0

  const verdict = lastMarker(text, VERDICT_MARKER)
  const blocking = verdict === 'CRITICAL'
    || malformations(verdict, lastMarker(text, REVIEWED_MARKER)).length > 0
  console.error(`review-guard: ${reportPath} is incomplete — a verdict over files nobody read clears nothing.`)
  for (const p of problems) console.error(`  ${p}`)
  console.error(blocking
    ? '  It still blocks (CRITICAL, or markers nobody can read), so it replaces the standing report.'
    : '  It is not promoted: the previous report stands and the marker does not move. Its\n'
      + '  findings still stand — `code-review` keeps it as .work/review-incomplete.md: fix what\n'
      + '  it found, then re-run the review.')
  return blocking ? 3 : 1
}

/** How far HEAD has drifted from the reviewed commit, in prose. */
function drift(sha, head) {
  const count = git('rev-list', '--count', `${sha}..${head}`)
  return count && count !== '0' ? `${count} commit(s) since` : `HEAD is ${short(head)}`
}

function main() {
  // An EMPTY report is "not run", never "malformed". The recipe redirects into the
  // report, and `>` truncates before the CLI runs, so a CLI that dies — absent, no
  // key, rate-limited, Ctrl-C — leaves a 0-byte file. Reading that as malformed
  // blocks every push on the machine (no glob on the hook), with no way out but the
  // successful review that just failed, and the file's existence hiding the
  // absent-report escape. The recipe writes to a temp file and moves it into place for
  // the same reason; this arm is the belt to that braces.
  const text = existsSync(reportPath) ? readFileSync(reportPath, 'utf8') : null
  if (expectFiles !== null && text !== null && text.trim() !== '') return completeness(text, expectFiles)
  if (text === null || text.trim() === '') {
    const what = text === null ? `no ${reportPath}` : `${reportPath} is empty`
    if (requireReport) {
      console.error(`review-guard: ${what} — the reviewer produced nothing, so there is no verdict.`)
      console.error('  Reported as "code review not run", never as green. Check the agent CLI (absent,')
      console.error('  unauthenticated, rate-limited?) and re-run.')
      return 1
    }
    console.log(`review-guard: ${what} — code review not run.`)
    console.log('  Not a pass and not a failure: hand back with "code review not run", never as green.')
    console.log(`  ${RERUN}`)
    return 0
  }

  // The LAST marker of each kind wins, because the format spec puts them "at the very
  // end" — and a review that QUOTES the output contract in a fix suggestion (which
  // happens exactly when the diff touches review-prompt.md or the code-reviewer agent)
  // emits a second CI_VERDICT line at column 0 and would otherwise be rejected as
  // having two verdicts. A fixed-size TAIL window was the first attempt and it broke
  // both ways — see lastMarker().
  const verdict = lastMarker(text, VERDICT_MARKER)
  const sha = lastMarker(text, REVIEWED_MARKER)

  const broken = malformations(verdict, sha)
  if (broken.length > 0) {
    console.error(`review-guard: ${reportPath} is malformed — a report nobody can parse cannot clear a push.`)
    for (const problem of broken) console.error(`  ${problem}`)
    console.error(`  ${RERUN}`)
    return 1
  }

  const head = git('rev-parse', 'HEAD')

  if (verdict === 'CRITICAL') {
    console.error(`review-guard: the review found CRITICAL issues (reviewed ${short(sha)}).`)
    console.error('  A CRITICAL does not expire. Committing on top of it does not answer it, and')
    console.error('  neither does deleting the report — fix what the report names, then re-review.')
    if (head && !samesha(sha, head)) console.error(`  HEAD has moved (${drift(sha, head)}), which changes nothing here.`)
    console.error(`  Report: ${reportPath}. ${RERUN}`)
    return 1
  }

  if (!head) {
    console.log(`review-guard: ${verdict} at ${short(sha)} — no commit yet, so nothing to compare it against.`)
    return 0
  }
  if (samesha(sha, head)) {
    console.log(`review-guard: ${verdict} at ${short(sha)} — the review describes HEAD.`)
    return 0
  }
  console.log(`review-guard: ${verdict}, but the report reviewed ${short(sha)} and not HEAD (${drift(sha, head)}).`)
  console.log('  Stale, not blocking: a trivial commit must not cost a whole review. If the code')
  console.log(`  moved in a way a reviewer would care about, re-review — \`just code-review\`.`)
  return 0
}

process.exit(main())
