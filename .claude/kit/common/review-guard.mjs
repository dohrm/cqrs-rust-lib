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
const reportPath = args.find((a) => !a.startsWith('--')) ?? '.work/review-report.md'

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
  if (text === null || text.trim() === '') {
    console.log(`review-guard: ${text === null ? `no ${reportPath}` : `${reportPath} is empty`} — code review not run.`)
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
