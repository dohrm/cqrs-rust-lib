#!/usr/bin/env node

// bash-guard — the harness tier of the gate layer, for shell commands.
//
// rules/agent/autonomy.md says in prose what an agent must never do: bypass a
// hook, disable the gate layer, forge the review report. Prose persuades; this
// file does not. It is the second of TWO tiers, and it is the weaker one:
//
//   1. git (lefthook) — portable across every agent, the universal floor. It
//      catches what git can see: a commit on the trunk, an unanswered CRITICAL.
//   2. harness (this) — catches what git CANNOT see, because it never gets that
//      far: the `--no-verify`, the `lefthook uninstall`, the `rm` on the report.
//
// Neither tier makes drift impossible. They make it expensive and LOUD. The
// impossibility lives on the server (branch protection) and in the orchestrator.
// See ./README.md — do not sell this as more than it is.
//
// Two verdicts, on purpose:
//   • DENY (exit 2) — the command has no legitimate form. stderr comes back to
//     the agent as the reason, so it can fix the cause instead of the gate.
//   • ASK (json)    — the command WRITES to the gate layer itself. Sometimes
//     that is the task (`/ci-setup` edits a workflow), so a human decides.
//     Headless, "ask" blocks — which is the intended answer for an unattended run.
//
// Hosts: Claude Code (PreToolUse, matcher "Bash") and Cursor
// (beforeShellExecution). One script, two payload dialects — see ask().
//
// It FAILS OPEN. A crash here must not brick a session, so an unreadable payload
// is reported on stderr and the command proceeds. A guard is a cost multiplier,
// never a proof.

import { readFileSync } from 'node:fs'

/** Quoted text is data, not flags: `git commit -m "drop the -n flag"` is innocent. */
const scrub = (s) => s.replace(/'[^']*'/g, "''").replace(/"[^"]*"/g, '""')

// Where a token ENDS. `(\s|$)` and `(\s|:|$)` read as "space, or the end of the
// command" and are not that: `;`, `&` and `|` are none of the alternatives, so ONE
// trailing separator walked straight past the rule — `git commit --no-verify; git push`
// and `git push -f origin main; echo ok` were both allowed, which is the flagship rule
// of this file defeated by one character. A negated class ends the token on anything
// that cannot be part of it, separators and end-of-string alike.
const REF_END = String.raw`(?![\w/.-])`
const FLAG_END = String.raw`(?![\w-])`

// The trunk as a WHOLE ref — preceded by a space or a colon (`HEAD:master`), or
// spelled in full (`refs/heads/main`), and ending there. `\b(main)\b` was matching
// the word inside an ordinary branch name: `feat/fix-main-nav` is not the trunk, and
// denying a push to it teaches everyone that the guard cries wolf.
const TRUNK = String.raw`(main|master|trunk)`
const TRUNK_REF = String.raw`((\s|:)${TRUNK}|refs/heads/${TRUNK})${REF_END}`

// Every command that MUTATES a file, in ONE list. There were two — this and a shorter
// `.git/hooks/` list — and the short one let `cp /dev/null`, `ln -sf`, `install -m755`
// and `chmod 000` empty the git floor with no prompt at all. `sed` counts only when it
// writes: `-i`, or the long spelling `--in-place`. Readers (`cat`, `grep`, `sed -n`) are
// deliberately absent — a guard that blocks its own diagnostic invites being unwired.
const MUTATORS = String.raw`(?:rm|mv|cp|tee|truncate|dd|chmod|chown|ln|install|touch|sed\s+(?:-i|--in-place))`
const MUTATOR = String.raw`(^|[\s;|&])${MUTATORS}\b[^|;&]*`

// A redirect that opens a FILE. `>&2` and `2>&1` duplicate an fd and open nothing, so
// the `&` AFTER the `>` is what excludes them. A lookbehind BEFORE the `>` was reading
// exactly where the fd number sits, so `1>`, `2>` and `&>` were exempted instead — and
// each of those truncates a real file on open. `>|` overrides noclobber.
const REDIRECT = String.raw`>>?(?!&)\|?\s*[^\s|;&]*`

// Only the current segment matters: `[^|;&]*` keeps a match from leaking across
// a `&&` into an unrelated command.
const DENY = [
  [new RegExp(String.raw`\bgit\b[^|;&]*\s--no-verify${FLAG_END}`),
    'hook bypass (--no-verify) is forbidden: fix the cause, never the gate'],
  [new RegExp(String.raw`\bgit\s+commit\b[^|;&]*\s-[a-zA-Z]*n[a-zA-Z]*${FLAG_END}`),
    '`git commit -n` bypasses the hooks — same rule as --no-verify'],
  [new RegExp(String.raw`\bgit\s+push\b[^|;&]*(--force\b|--force-with-lease\b|\s-f\b)[^|;&]*` + TRUNK_REF),
    'force-push to the trunk is a human act, never an agent one'],
  [new RegExp(String.raw`\bgit\s+push\b[^|;&]*\s\+${TRUNK}${REF_END}`),
    'a `+ref` refspec IS a force-push, and the trunk is a human act'],
  [new RegExp(String.raw`\bgit\s+push\b[^|;&]*(--delete\s+|:)\s*${TRUNK}${REF_END}`),
    'deleting the trunk is a human act, never an agent one'],
  [/core\.hooksPath\s*=|\bgit\s+config\b[^|;&]*core\.hooksPath\s+\S/,
    'moving core.hooksPath disables the hook layer — reading it back is fine, setting it is not'],
  [/\blefthook\s+uninstall\b/,
    'uninstalling lefthook removes the gate layer — escalate instead'],
  [new RegExp(MUTATOR + String.raw`\.git/hooks/`),
    'emptying .git/hooks/ unwires the git floor — the cheapest bypass there is, and the one git itself cannot see'],
  [new RegExp(REDIRECT + String.raw`\.git/hooks/`),
    'writing .git/hooks/ by hand replaces the floor with whatever you put there — `lefthook install` owns those files'],
  [/\b(LEFTHOOK\s*=\s*(0|false)|LEFTHOOK_EXCLUDE\s*=|SKIP\s*=|HUSKY\s*=\s*0)/,
    'skipping the hooks through the environment is a hook bypass'],
  [new RegExp(MUTATOR + String.raw`review-report\.md`),
    'the review report is written by a review and by nothing else — a CRITICAL is answered by a fix, not by deleting the file'],
  [new RegExp(REDIRECT + String.raw`review-report\.md`),
    'writing the review report by hand forges the verdict review-guard reads'],
]

// The gate layer's own files. Reading them is normal; rewriting them is a
// decision. The list is deliberately short — every entry is a file whose content
// decides whether a gate exists or what "green" means.
//
// The gate file and the write used to be matched INDEPENDENTLY over the whole
// command, which escalates anything holding both a gate file and any redirect — so
// `cat justfile 2>/dev/null` asked, and headless an "ask" blocks. Two mistakes were
// folded into one: an fd redirect (`2>`, `1>&2`) is not a write to what is being
// read, and `sed -n` does not write at all. The pair below correlates the halves:
// the redirect must TARGET a gate file, or a genuinely mutating command must carry
// one among its arguments, in the same segment.
//
// `*.just` is in the list because the recipes live in an IMPORTED library now, not in
// the justfile: guarding only the file that holds the `import` lines would leave the
// commands the gate actually runs unguarded.
const GATE_SRC = String.raw`(?<![\w.-])(?:lefthook\.ya?ml|[Jj]ustfile|\S*\.just|\.coverage-baseline|mutants\.toml|deny\.toml|\.docs-budgets\.json|\.claude/settings(?:\.local)?\.json|\.cursor/hooks\.json|\.git(?:hub|ea)/workflows/\S*|(?:adr-check|docs-check|review-guard|bash-guard|edit-guard)\.mjs|review-prompt\.md)`

/** A redirect whose target is a gate file, and a mutating command carrying one in the
 *  SAME segment. Both halves come from the shared REDIRECT/MUTATOR sources above, so
 *  there is one definition of "this writes" for the whole file. */
const REDIRECT_TO_GATE = new RegExp(REDIRECT + GATE_SRC)
const MUTATOR_ON_GATE = new RegExp(MUTATOR + GATE_SRC)

const cmd = (input) => input.tool_input?.command ?? input.command ?? ''

/** Cursor payloads always carry a conversation_id; Claude Code's never do. */
const isCursor = (input) => 'conversation_id' in input

/** Same decision, two dialects. Exit 2 means "blocked" to both, so deny is shared. */
function ask(input, reason) {
  console.log(JSON.stringify(isCursor(input)
    ? { permission: 'ask', agent_message: reason }
    : { hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'ask', permissionDecisionReason: reason } }))
}

function main() {
  let input
  try {
    input = JSON.parse(readFileSync(0, 'utf8'))
  } catch (e) {
    console.error(`bash-guard: unreadable hook payload (${e.message}) — failing open.`)
    return 0
  }

  const raw = cmd(input)
  if (!raw) return 0
  const command = scrub(raw)

  for (const [re, why] of DENY) {
    if (re.test(command)) {
      console.error(`Blocked: ${why}`)
      console.error('  If the gate is genuinely wrong, say so and escalate — do not route around it.')
      return 2
    }
  }

  if (REDIRECT_TO_GATE.test(command) || MUTATOR_ON_GATE.test(command)) {
    ask(input, 'This command writes to the gate layer (a hook, a workflow, a justfile, a baseline). '
      + 'Editing the gates is sometimes the task — a human confirms it.')
  }
  return 0
}

process.exit(main())
