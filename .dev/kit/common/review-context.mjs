#!/usr/bin/env node

// The `=== RFC ===` section of a review prompt: the frame the change was cut against,
// handed to the reviewer instead of left for it to find.
//
// The reviewer is read-only on purpose — no writes, no network — so it can read a local
// RFC but never follow a ticket RFC's pointer: there, `.work/<slug>/rfc.md` holds only
// `**Ticket**: <URL>`, and the review ran without its frame while saying nothing about
// it. In ticket mode the implementing agent caches the ticket, raw, in
// `.work/<slug>/review-context.md` before each phase's review (skills/rfc/ticket.md);
// this script only reads what is on disk.
//
// | on disk                                  | section                                     |
// |------------------------------------------|---------------------------------------------|
// | no `rfc.md` nor `defect.md` for the slug | "No RFC for this branch." — not a finding:  |
// |                                          | loops and ad hoc work have none             |
// | `defect.md` naming the branch while its  | the diagnosis, minus `## Log` — always      |
// | `rfc.md` does not, or no `rfc.md` at all | local, its `**Ticket**` is only a link      |
// | local `rfc.md`                           | its body, minus `## Revisions` and `## Log` |
// |                                          | (history), keeping Log's `changed:` entries |
// | ticket pointer + `review-context.md`     | the pointer header, then the cache, with    |
// |                                          | the time it was written                     |
// | ticket pointer, no cache; several RFCs   | "RFC context not available: …" — declared,  |
// | claiming the branch; a read error        | for the reviewer to report as a 🟡          |
//
//   node review-context.mjs [--slug <s>]                prints the section on stdout.
//   node review-context.mjs [--slug <s>] --brief <n>    prints at most <n> characters of
//       it for OCR's `--background-file`: the RFC's title, Goal, Model and Local decisions
//       (Scope is left out: the diff OCR reviews already shows it). OCR warns past 2,000 characters and aborts past 8,000 (1.12.13,
//       cmd/opencodereview/background_file.go) — the full section, a ticket's comments
//       included, ran 27,530 on the first real RFC.

import { existsSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { ensureWorkIgnored, headerOf, resolveSlug } from './work-slug.mjs'

const HISTORY = new Set(['revisions', 'log'])
const TICKET_HEADER = /^[ \t]*-[ \t]*\*\*Ticket\*\*:[ \t]*(\S+)/m

// A recorded change, in the form `/rfc` writes it: `changed: <decision> — <why>` in
// backticks. The bare word is not enough — a log line ABOUT `changed:` entries is not one.
const CHANGED = /`changed: [^`\s][^`]*`/
const BULLET = /^([-*+]|\d+[.)])\s/
const HEADING = /^#{1,6}\s/

/** The document without its history sections: a `## ` heading named in HISTORY runs to
 *  the next `## ` heading or the end. A `changed:` entry of `## Log` is history that
 *  still binds — it is how a local decision moved — so those top-level bullets are kept
 *  whole (continuations and sub-bullets included, a loose paragraph after a blank line
 *  too) under a heading of their own: without them the reviewer flags a recorded change
 *  as a departure. */
function withoutHistory(text) {
  const out = []
  const changed = []
  let section = null
  let bullet = null
  let blank = false
  const flush = () => {
    if (bullet && CHANGED.test(bullet.join('\n'))) changed.push(...bullet)
    bullet = null
    blank = false
  }
  for (const line of text.split('\n')) {
    const h2 = line.match(/^##\s+(.+?)\s*$/)
    if (h2) {
      flush()
      section = h2[1].toLowerCase()
    }
    if (!HISTORY.has(section)) {
      out.push(line)
      continue
    }
    if (section !== 'log' || h2) continue
    if (BULLET.test(line) || HEADING.test(line)) {
      flush()
      if (BULLET.test(line)) bullet = [line]
    } else if (line.trim() === '') {
      blank = true
    } else if (bullet && (!blank || /^\s/.test(line))) {
      if (blank) bullet.push('')
      bullet.push(line)
      blank = false
    } else {
      flush()
    }
  }
  flush()
  const body = out.join('\n').replace(/\n+$/, '\n')
  return changed.length ? `${body}\n## Changed decisions (from ## Log)\n\n${changed.join('\n')}\n` : body
}

function reviewContext({ slug = '', cwd = '.' } = {}) {
  const resolved = resolveSlug({ slug, cwd })
  const dir = join('.work', resolved.slug)
  const rfcPath = join(dir, 'rfc.md')
  const head = `=== RFC ===\n\n`

  if (resolved.ambiguous) {
    return `${head}RFC context not available: ${resolved.ambiguous.map((f) => `.work/${f}`).join(', ')} `
      + 'all claim this branch in their **Branch** header — the tree contradicts its state files.\n'
  }
  // The file whose header named the branch; with a slug given or found by name, the RFC
  // first, else the diagnosis.
  const defectPath = join(dir, 'defect.md')
  const isDefect = resolved.file === 'defect.md'
    || (!resolved.file && !existsSync(join(cwd, rfcPath)) && existsSync(join(cwd, defectPath)))
  if (isDefect) {
    return `${head}Source: ${defectPath} (defect diagnosis)\n\n${withoutHistory(readFileSync(join(cwd, defectPath), 'utf8'))}`
  }
  if (!existsSync(join(cwd, rfcPath))) {
    return `${head}No RFC for this branch (slug \`${resolved.slug}\`, from the ${resolved.from}).\n`
  }

  const rfc = readFileSync(join(cwd, rfcPath), 'utf8')
  const ticket = headerOf(rfc).match(TICKET_HEADER)?.[1]
  if (!ticket) return `${head}Source: ${rfcPath} (local RFC)\n\n${withoutHistory(rfc)}`

  const cachePath = join(dir, 'review-context.md')
  if (!existsSync(join(cwd, cachePath))) {
    return `${head}RFC context not available: ${rfcPath} points to ${ticket} and no ${cachePath} `
      + 'was cached — the implementing agent writes it before each phase\'s review '
      + '(skills/rfc/ticket.md § Implementation). Report the review as run without its frame.\n\n'
      + `Pointer:\n\n${withoutHistory(rfc)}`
  }
  // A cache can sit on disk because a run wrote it; it is private either way.
  ensureWorkIgnored(cwd)
  const written = statSync(join(cwd, cachePath)).mtime.toISOString()
  return `${head}Source: ${ticket}, cached in ${cachePath} at ${written}\n\n`
    + `Pointer:\n\n${withoutHistory(rfc)}\nTicket:\n\n${readFileSync(join(cwd, cachePath), 'utf8')}`
}

const BRIEF_SECTIONS = /^##\s+(goal|model|local decisions)\s*$/i

/** What a second reviewer needs of the frame, in at most `max` characters: the title and
 *  the first Goal, Model and Local decisions sections found (the ticket body carries them
 *  in ticket mode); else the start of the text. Cut, never rejected. */
function brief(text, max) {
  const lines = text.split('\n')
  const out = []
  const title = lines.find((l) => /^#\s+RFC\b/.test(l))
  if (title) out.push(title, '')
  const seen = new Set()
  let keep = false
  for (const l of lines) {
    const h2 = l.match(/^##\s+(.+?)\s*$/)
    if (h2) {
      const name = h2[1].toLowerCase()
      keep = BRIEF_SECTIONS.test(l) && !seen.has(name)
      if (keep) seen.add(name)
    }
    if (keep) out.push(l)
  }
  const chars = Array.from((seen.size ? out.join('\n') : text).trim())
  return chars.length > max ? `${chars.slice(0, max - 1).join('')}…\n` : `${chars.join('')}\n`
}

// Context is the reviewer's input, never a gate: whatever goes wrong here is declared in
// the section and the review still runs.
const argv = process.argv.slice(2)
const flag = (name) => (argv.includes(name) ? (argv[argv.indexOf(name) + 1] ?? '') : null)
let section
try {
  section = reviewContext({ slug: flag('--slug') ?? '' })
} catch (err) {
  section = `=== RFC ===\n\nRFC context not available: ${err.message}\n`
}
const max = Number(flag('--brief'))
process.stdout.write(flag('--brief') !== null && max > 0 ? brief(section, max) : section)
