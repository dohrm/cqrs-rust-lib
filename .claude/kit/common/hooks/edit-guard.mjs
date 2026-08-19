#!/usr/bin/env node

// edit-guard — the harness tier of the gate layer, for file edits.
//
// The companion of ./bash-guard.mjs, and it asks rather than denies. Editing a
// workflow, a lefthook.yml or the `check` recipe is sometimes exactly the task
// (`/ci-setup` does nothing else), so the answer is not "forbidden" but "a human
// confirms". Interactively that costs one keypress; headless, "ask" blocks —
// which is the intended answer for an unattended agent rewriting its own gates.
//
// The list is deliberately short. Every path on it is a file whose content
// decides whether a gate EXISTS, or what "green" means:
//   • the hook layer itself (lefthook.yml, the settings that wire these guards,
//     and these guards' own source — the meta-bypass);
//   • the gate definition (justfile: `check` is the command an agent closes its
//     loop on, so weakening it is the cheapest fake green);
//   • the ratchets (a coverage baseline, mutants.toml exclusions) — autonomy.md
//     calls lowering one a HARD bypass;
//   • the review report, which review-guard reads and nothing else writes.
//
// Host: Claude Code (PreToolUse, matcher "Edit|Write"). Cursor has NO
// pre-edit hook — `afterFileEdit` runs after the write — so on Cursor this tier
// simply does not exist. See ./README.md.
//
// It FAILS OPEN, for the same reason bash-guard does.

import { readFileSync } from 'node:fs'

// DENY, not ask: hand-writing the report forges the verdict `review-guard` reads, and
// `bash-guard` already denies the shell spelling of the same act. A guard whose two
// halves disagree about the hardest rule is a guard nobody trusts. The git hook files
// join it — `lefthook install` owns them, an agent never writes them by hand.
const DENIED = [
  /(^|\/)\.work\/review-report\.md$/,
  /(^|\/)\.git\/hooks\//,
]

const PROTECTED = [
  /(^|\/)lefthook\.ya?ml$/,
  /(^|\/)[Jj]ustfile$/,
  /(^|\/)\.git(hub|ea)\/workflows\//,
  /(^|\/)\.claude\/settings(\.local)?\.json$/,
  /(^|\/)\.cursor\/hooks\.json$/,
  /(^|\/)\.codex\/config\.toml$/,
  /(^|\/)opencode\.jsonc?$/,
  /(^|\/)(bash|edit)-guard\.mjs$/,
  /(^|\/)(adr-check|docs-check|review-guard)\.mjs$/,
  /(^|\/)review-prompt\.md$/,
  /(^|\/)\.coverage-baseline$/,
  /(^|\/)mutants\.toml$/,
  /(^|\/)deny\.toml$/,
  /(^|\/)\.docs-budgets\.json$/,
]

const WHY = {
  'review-report.md': 'the report is written by a review and by nothing else — hand-editing it forges the verdict `review-guard` reads',
  baseline: 'lowering a ratchet to turn a run green is a HARD bypass (rules/agent/autonomy.md)',
  guard: 'this file IS the guard — editing it is the meta-bypass',
  hooks: '`lefthook install` owns .git/hooks/ — hand-writing one replaces the git floor with whatever you put there',
}
function reason(path) {
  if (/review-report\.md$/.test(path)) return WHY['review-report.md']
  if (/(\.coverage-baseline|mutants\.toml)$/.test(path)) return WHY.baseline
  if (/(bash|edit)-guard\.mjs$/.test(path)) return WHY.guard
  if (/\.git\/hooks\//.test(path)) return WHY.hooks
  return 'it is part of the gate layer, and a green gate is only worth its exit code if an agent cannot redefine it'
}

function main() {
  let input
  try {
    input = JSON.parse(readFileSync(0, 'utf8'))
  } catch (e) {
    console.error(`edit-guard: unreadable hook payload (${e.message}) — failing open.`)
    return 0
  }

  const path = input.tool_input?.file_path ?? input.tool_input?.path ?? ''
  if (!path) return 0

  if (DENIED.some((re) => re.test(path))) {
    console.error(`Blocked: ${path} — ${reason(path)}`)
    console.error('  If the gate is genuinely wrong, say so and escalate — do not route around it.')
    return 2
  }

  if (!PROTECTED.some((re) => re.test(path))) return 0

  console.log(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'ask',
      permissionDecisionReason: `${path}: ${reason(path)}. A human confirms edits to the gates themselves.`,
    },
  }))
  return 0
}

process.exit(main())
