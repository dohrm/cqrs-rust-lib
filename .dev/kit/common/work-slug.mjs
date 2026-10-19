// Which `.work/<slug>/` directory the current branch works in — one answer for every gate
// script, so a gate's marker and the RFC it reviews against land in the same directory.
//
// The bug it closes was measured in a consuming repo: branch `rfc/19-deliver-composition`,
// RFC in `.work/19-deliver-composition/`, and the review marker in
// `.work/rfc-19-deliver-composition/` — because the gates slugged the branch name while
// `/rfc` slugged the request. A naming convention cannot fix that (a phase branch keeps its
// own name, and one tree may hold several committed RFCs, more than one `implementing`), so
// the RFC says which branch it is on, in its `**Branch**` header, and this reads it.
//
// Order, first answer wins:
//   1. an explicit slug (the justfile's `work_slug`);
//   2. the `.work/*/rfc.md` (or `/defect`'s `defect.md`) whose `**Branch**` header names the
//      current branch;
//   3. an L4 phase branch `phase/<slug>/<n>` whose `.work/<slug>/rfc.md` exists — the
//      header names the integration branch `rfc/<slug>`, and git cannot nest a phase
//      under it (rules/agent/autonomy.md § L4);
//   4. the branch name, sanitized into one path segment (the gates' original behavior).
//
// A module, not a command: `diff-since.mjs` and `review-context.mjs` import it, and its
// `ensureWorkIgnored` keeps their per-developer files out of a committed `.work/`.

import { execFileSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

// A slug becomes a PATH. Anything outside the safe set collapses to `-`, which also
// flattens the `/` of `feat/thing` and makes `..` unwritable — a branch cannot contain
// `..`, an overridden `work_slug` can.
export const sanitize = (s) => s.replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^[-.]+/, '') || 'detached'

// Header fields are read from the header list only — everything before the first `## ` —
// so a step that quotes the format does not count, and `[ \t]*` keeps an empty field
// from capturing the next line.
export const headerOf = (text) => text.split(/^## /m)[0]
const BRANCH_HEADER = /^[ \t]*-[ \t]*\*\*Branch\*\*:[ \t]*`?([^`\s]+)`?/m

export function currentBranch(cwd = '.') {
  try {
    return execFileSync('git', ['symbolic-ref', '--quiet', '--short', 'HEAD'],
      { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim() || null
  } catch {
    return null
  }
}

// The state files that carry a `**Branch**` header: `/rfc`'s and `/defect`'s.
const BRANCHED = ['rfc.md', 'defect.md']

/** The state files under `.work/` (`<dir>/rfc.md`, `<dir>/defect.md`) whose `**Branch**`
 *  header is `branch`. */
function rfcsOn(branch, cwd) {
  const root = join(cwd, '.work')
  let entries = []
  try {
    entries = readdirSync(root, { withFileTypes: true })
  } catch {
    return []
  }
  return entries
    .filter((e) => e.isDirectory())
    // Within one directory the RFC wins: a bug escalated to `/rfc` keeps its `defect.md` as
    // the RFC's input, and the RFC is the frame. Two directories claiming one branch stay
    // ambiguous.
    .map((e) => BRANCHED.map((file) => `${e.name}/${file}`).find((rel) => {
      // An unreadable state file (a directory, no permission) belongs to some other work:
      // it must not crash the gate of this one.
      try {
        return headerOf(readFileSync(join(root, rel), 'utf8')).match(BRANCH_HEADER)?.[1] === branch
      } catch {
        return false
      }
    }))
    .filter(Boolean)
    .sort()
}

/** `{ slug, from, file, ambiguous }` — `from` says which rule answered, for the
 *  diagnostics; `file` is the state file whose header answered (`rfc.md` or `defect.md`);
 *  `ambiguous` lists the state files that all claim this branch, when more than one does. */
export function resolveSlug({ slug = '', cwd = '.' } = {}) {
  if (slug.trim()) return { slug: sanitize(slug.trim()), from: 'work_slug' }
  const branch = currentBranch(cwd)
  if (!branch) return { slug: 'detached', from: 'detached HEAD' }
  const matches = rfcsOn(branch, cwd)
  if (matches.length === 1) {
    const [dir, file] = matches[0].split('/')
    return { slug: sanitize(dir), from: `.work/${matches[0]} **Branch**`, file }
  }
  if (matches.length > 1) {
    // Two RFCs claiming one branch is a tree that contradicts its state files — not ours
    // to arbitrate. The branch name is the answer that hides nothing.
    return { slug: sanitize(branch), from: 'branch name', ambiguous: matches }
  }
  const phaseOf = branch.match(/^phase\/([^/]+)\/[^/]+$/)?.[1]
  if (phaseOf && existsSync(join(cwd, '.work', phaseOf, 'rfc.md'))) {
    return { slug: sanitize(phaseOf), from: `phase branch of .work/${phaseOf}/rfc.md` }
  }
  return { slug: sanitize(branch), from: 'branch name' }
}

// `.work/` is gitignored in most installs and committed in some (that is why
// `review_exclude` drops it from the diff). The markers must be private either way, and
// so must a ticket RFC's cached `review-context.md` (it holds the ticket's comments), so
// the patterns are written where they hold in both cases, and only what is missing is
// appended — a `.gitignore` the repo already owns is never rewritten.
const PATTERNS = ['*/.latest_review', '*/.latest_mutate', '*/review-context.md']
export function ensureWorkIgnored(cwd = '.') {
  const file = join(cwd, '.work', '.gitignore')
  const have = existsSync(file) ? readFileSync(file, 'utf8') : null
  const lines = have === null ? [] : have.split(/\r?\n/)
  const missing = PATTERNS.filter((p) => !lines.includes(p))
  if (missing.length === 0) return
  const header = have === null
    ? '# Per-developer gate state: what THIS clone has already reviewed / mutated, and the\n'
      + '# ticket context it reviewed against.\n'
      + '# Never shared — a teammate\'s HEAD is not yours, and neither is their verdict.\n'
    : ''
  writeFileSync(file, (have === null ? '' : have.replace(/\n*$/, '\n')) + header + missing.join('\n') + '\n')
}
