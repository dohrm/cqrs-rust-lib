// Structural half of experience contracts, called by docs-check. No browser,
// network or authorship inference: a valid document is not a verified experience.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'

const read = file => readFileSync(file, 'utf8')
const isFile = file => existsSync(file) && statSync(file).isFile()
const withoutCode = text => text.replace(/^(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\1\s*$/gm, '')
const slug = /^[a-z0-9]+(?:-[a-z0-9]+)*$/

function contractFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(dir, entry.name)
    if (entry.isDirectory()) return contractFiles(file)
    return entry.name.endsWith('.md') && entry.name.toLowerCase() !== 'readme.md' ? [file] : []
  })
}

function section(text, heading) {
  for (const part of text.split(/^## /m).slice(1)) {
    const [title, ...body] = part.split('\n')
    if (title.trim() === heading) return body.join('\n').trim()
  }
  return ''
}

function links(text) {
  return [...text.matchAll(/\[[^\]]*\]\((<[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)/g)]
    .map(m => m[1].replace(/^<|>$/g, ''))
}

function target(file, href, problems) {
  if (/^https?:\/\//i.test(href)) {
    try { new URL(href); return null } catch { /* report below */ }
  } else if (!/^[a-z][a-z0-9+.-]*:/i.test(href)) {
    try {
      const local = decodeURIComponent(href.split('#')[0])
      if (!local) return path.resolve(file) // in-document anchor; content not checked
      const resolved = path.resolve(path.dirname(file), local)
      if (isFile(resolved)) return resolved
    } catch { /* invalid escape or filesystem target */ }
  }
  problems.push(`${file}: invalid or missing reference ${href}.`)
  return null
}

export function checkExperience(docsDir, problems) {
  const dir = path.join(docsDir, 'experience')
  const index = path.join(docsDir, 'EXPERIENCE.md')
  // Existing single-file experience docs keep working. But a link to a missing
  // contract must fail even before the first directory is created.
  const indexed = new Set()
  if (isFile(index)) {
    for (const href of links(withoutCode(read(index)))) {
      if (/^[a-z][a-z0-9+.-]*:/i.test(href)) continue
      let resolved
      try { resolved = path.resolve(path.dirname(index), decodeURIComponent(href.split('#')[0])) }
      catch { problems.push(`${index}: invalid reference ${href}.`); continue }
      if (resolved.startsWith(path.resolve(dir) + path.sep)) {
        const found = target(index, href, problems)
        if (found) indexed.add(found)
      }
    }
  }
  if (!existsSync(dir)) return
  if (!statSync(dir).isDirectory()) {
    problems.push(`${dir}: expected a directory of experience contracts.`)
    return
  }
  const files = contractFiles(dir)
  if (!isFile(index)) problems.push(`${dir}: contracts require ${index}.`)
  const ids = new Map()
  for (const file of files) {
    if (!isFile(file)) { problems.push(`${file}: expected a contract file.`); continue }
    if (!indexed.has(path.resolve(file))) problems.push(`${file}: no link from ${index}.`)
    const text = withoutCode(read(file))
    // Metadata belongs before the sections; a quoted template/example is not it.
    const header = text.split(/^## /m)[0]
    const fields = {}
    for (const [, key, value] of header.matchAll(/^- \*\*([^*]+)\*\*:[ \t]*(.*)$/gm)) {
      if (Object.hasOwn(fields, key)) problems.push(`${file}: duplicate field ${key}.`)
      fields[key] = value.trim()
    }
    for (const key of ['ID', 'Actor', 'Scope', 'Status', 'Visual policy']) {
      if (!fields[key]) problems.push(`${file}: missing ${key}.`)
    }
    if (fields.ID) {
      if (!slug.test(fields.ID)) problems.push(`${file}: ID must be a lowercase slug.`)
      if (ids.has(fields.ID)) problems.push(`${file}: duplicate ID ${fields.ID} (also ${ids.get(fields.ID)}).`)
      ids.set(fields.ID, file)
    }
    if (fields.Status && !['exploring', 'stable'].includes(fields.Status))
      problems.push(`${file}: Status must be exploring or stable.`)
    if (fields['Visual policy'] && !['toolkit', 'specified'].includes(fields['Visual policy']))
      problems.push(`${file}: Visual policy must be toolkit or specified.`)
    if (fields.Status === 'stable' && !fields['Validation source'])
      problems.push(`${file}: stable requires Validation source (developer instruction, not inferred approval).`)
    for (const heading of ['Outcome', 'Flow', 'Invariants', 'Recovery', 'Freedom', 'Evidence']) {
      if (!section(text, heading)) problems.push(`${file}: missing or empty ${heading} section.`)
    }
    const visual = links(section(text, 'Visual references'))
    if (fields['Visual policy'] === 'specified' && visual.length === 0)
      problems.push(`${file}: specified requires links under Visual references.`)
    for (const href of links(text)) target(file, href, problems)
  }
}
