#!/usr/bin/env node

// The `=== OCR CANDIDATES ===` section of a review prompt: what an Open Code Review pass
// (alibaba/open-code-review) found on the same diff, handed to OUR reviewer as leads.
//
// OCR is the opposite trade-off to ours: a deterministic pipeline (file grouping, one
// sub-agent per group, comment re-location, a fact-check filter) tuned for precision,
// where our reviewer is one strict pass that also judges ADRs, experience contracts and
// AI slop. Its findings never become ours by being printed here: the reviewer confirms
// each against the code or drops it, and stays the only verdict.
//
// It never blocks either. OCR is opt-in, needs its own LLM provider, and can fail for a
// dozen reasons (no key, rate limit, network): every failure is declared in the section —
// "OCR pass not run: …" — and the review goes on without it.
//
//   node ocr-candidates.mjs <ocr.json> <ocr exit code> <ocr log>

import { existsSync, readFileSync } from 'node:fs'

const [jsonPath, code = '', logPath = ''] = process.argv.slice(2)
const SEVERITY = ['critical', 'high', 'medium', 'low']
// Bounded in bytes, not only in count: the section counts toward `review_max_bytes`, and
// OCR must never be what makes a review too large to run. Most severe first, each text
// field truncated, warnings capped, and candidates stop once the budget is spent.
const MAX_BYTES = 32_000
const MAX_FIELD = 1_500
const MAX_WARNINGS = 10
const head = '=== OCR CANDIDATES ===\n\n'
const end = '=== END OCR CANDIDATES ===\n'
// OCR's text is another model's output about a diff that may be hostile: every line break
// a tokenizer might honour becomes a newline, and every continuation line is indented, so
// nothing in it can pose as a section marker.
const flat = (text) => {
  const t = String(text ?? '').trim().replace(/\r\n?|[\u2028\u2029\u0085\v\f]/g, '\n')
  return (t.length > MAX_FIELD ? `${t.slice(0, MAX_FIELD)}…` : t).replace(/\n/g, '\n  ')
}
const line = (n) => (Number.isInteger(n) && n > 0 ? n : 0)

/** The last lines of OCR's own log: why it failed, in its words — flattened like the rest,
 *  and never a reason to throw. */
function tail(path, n = 5) {
  try {
    if (!path || !existsSync(path)) return ''
    const lines = readFileSync(path, 'utf8').trim().split('\n').filter(Boolean)
    return lines.slice(-n).map((l) => `    ${flat(l)}`).join('\n')
  } catch {
    return ''
  }
}

function notRun(why) {
  const log = tail(logPath)
  return `${head}OCR pass not run: ${flat(why)}.${log ? `\n\n${log}` : ''}\n${end}`
}

function section() {
  if (code !== '0') return notRun(`exit ${code || '(none)'}`)
  if (!jsonPath || !existsSync(jsonPath)) return notRun(`no ${jsonPath || 'output file'}`)
  let out
  try {
    out = JSON.parse(readFileSync(jsonPath, 'utf8'))
  } catch (err) {
    return notRun(`unreadable output (${err.message})`)
  }
  if (out === null || typeof out !== 'object') return notRun('output is not an object')
  const comments = (Array.isArray(out.comments) ? out.comments : []).filter((c) => c && typeof c === 'object')
  const rank = (c) => {
    const i = SEVERITY.indexOf(String(c.severity ?? '').toLowerCase())
    return i === -1 ? SEVERITY.length : i
  }
  const where = (c) => {
    const path = flat(c.path) || '(no path)'
    const [start, stop] = [line(c.start_line), line(c.end_line)]
    if (!start) return `${path} (OCR could not place it)`
    return stop > start ? `${path}:${start}-${stop}` : `${path}:${start}`
  }
  const model = out.llm?.model ? `, model ${flat(out.llm.model)}` : ''
  const warnings = Array.isArray(out.warnings) ? out.warnings : []
  const notes = []
  if (out.message) notes.push(`OCR says: ${flat(out.message)}`)
  for (const w of warnings.slice(0, MAX_WARNINGS))
    notes.push(`Warning from OCR: ${flat(typeof w === 'string' ? w : JSON.stringify(w))}`)
  if (warnings.length > MAX_WARNINGS) notes.push(`(${warnings.length - MAX_WARNINGS} more OCR warning(s) omitted)`)

  const shown = []
  let bytes = notes.join('\n').length
  for (const c of [...comments].sort((a, b) => rank(a) - rank(b))) {
    const tag = [c.severity, c.category].filter(Boolean).map(flat).join('/')
    const entry = `- [${tag || 'unrated'}] ${where(c)} — ${flat(c.content)}`
    if (bytes + entry.length > MAX_BYTES) break
    bytes += entry.length + 1
    shown.push(entry)
  }
  const lines = [
    `${comments.length} candidate(s) from OCR, ${shown.length} shown (status ${flat(out.status) || 'unknown'}${model}). `
      + 'Leads from another reviewer, not findings: confirm each against the code before you report '
      + 'it, in your own words and severity, or drop it.',
    ...notes,
    ...shown,
  ]
  if (comments.length > shown.length) lines.push(`(${comments.length - shown.length} less severe candidate(s) omitted)`)
  return `${head}${lines.join('\n')}\n${end}`
}

// Never the reason a review does not run: any surprise in OCR's output is declared.
let text
try {
  text = section()
} catch (err) {
  text = notRun(`unusable output (${err.message})`)
}
process.stdout.write(text)
