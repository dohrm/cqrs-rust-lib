# hooks — the harness tier of the gate layer (opt-in, wired by hand)

`rules/agent/autonomy.md` states in prose what an agent must never do: bypass a
hook, disable the gate layer, forge the review report. Prose can be persuaded.
This directory is the part that cannot.

## Two layers, and they do not replace each other

| Layer | Mechanism | Catches | Portable? |
|-------|-----------|---------|-----------|
| **git floor** | `lefthook` (`../lefthook.snippet.yml`) | what git can SEE: a commit on the trunk, a push with an unanswered `CRITICAL` | yes — every agent, every machine, CI included |
| **harness** | per-tool hooks (this directory) | what git never gets to see: the `--no-verify`, the `lefthook uninstall`, the `rm` on the report | no — one dialect per tool |

The floor is the same for both targets, so it is where a guarantee belongs. The
harness layer is depth, per tool. **Never the only rampart.**

(Not to be confused with the kit's `Tier 1-3`, which grade gates by *cost* —
lint / check / mutation. These two layers are about *where* a bypass is caught.)

## What it does NOT do

Be honest about the boundary or the whole thing becomes theatre:

- `.claude/settings.json` protects itself through `edit-guard`, and `bash-guard`
  escalates the obvious shell rewrites of it — but the set of ways to write a file
  from a shell is not enumerable. A determined agent gets through.
- Both guards **fail open**: an unreadable payload is reported on stderr and the
  command proceeds. A guard that bricks a session on its own bug is worse than one
  that misses a case.
- A hook can be unwired by whoever can edit `settings.json` outside the agent.

So: these hooks raise the cost of drift and make it **loud**. *Impossibility* lives
somewhere else — server-side branch protection, and the orchestrator that owns the
merge. Do not sell more than that.

## The two guards

**`bash-guard.mjs`** — `PreToolUse` on `Bash`. Two verdicts:

- **deny** (exit 2, reason on stderr → back to the agent): commands with no
  legitimate form — `--no-verify`, `git commit -n`, `core.hooksPath`,
  `lefthook uninstall`, `LEFTHOOK=0`/`SKIP=`, force-pushing or deleting the trunk,
  and any *write* aimed at `.work/review-report.md` or at `.git/hooks/` — a
  mutating command (`rm`, `mv`, `cp`, `tee`, `ln`, `install`, `touch`, `chmod`,
  `chown`, `truncate`, `dd`, `sed -i`) or a redirect. Reading them is not a write:
  `cat`, `grep` and `sed -n` pass, because a guard that blocks its own diagnostic
  invites being unwired.
- **ask** (JSON): the command *writes* to the gate layer — a `lefthook.yml`, a
  workflow, the `justfile`, a ratchet baseline, `mutants.toml`. Sometimes that is
  the task, so a human decides.

Quoted text is scrubbed before matching, so `git commit -m "drop the -n flag"` is
not a bypass. Matching stays inside the current `|;&` segment, and the write and the
gate file must be *correlated* — the redirect has to target the gate file, or the
mutating command has to carry it. Tested independently, `just check > /dev/null;
node .dev/kit/common/docs-check.mjs` escalates, and that command RUNS the gate rather
than writing it.

The gate layer includes `*.just`: the recipes live in the imported library, so guarding
only the `justfile` that holds the `import` lines would leave the commands unguarded.

**`edit-guard.mjs`** — `PreToolUse` on `Edit|Write`. Two verdicts, like its sibling:

- **deny** for the paths with no legitimate hand-edit: `.work/review-report.md`
  (hand-writing it forges the verdict `review-guard` reads) and `.git/hooks/`
  (`lefthook install` owns those files). `autonomy.md` calls both a HARD bypass, and
  `bash-guard` denies the shell spelling — an ask here would leave the strongest rule
  in the doctrine enforced by the weaker half.
- **ask** for everything else on the list, because editing it is sometimes the task:
  `/ci-setup`'s whole job is editing a workflow. The list is short on purpose — the
  hook layer itself (including these two files: the meta-bypass), the `justfile`
  (`check` is what "green" means), the ratchets, the review prompt.

Interactively, "ask" is one keypress. **Headless, "ask" blocks** — which is the
intended answer for an unattended agent rewriting its own gates.

Too noisy for your repo? Drop `justfile` from `PROTECTED` first — it is the entry
most likely to be edited for honest reasons.

## Wiring, per tool

The installer never writes any of these files (same contract as `lefthook.yml` and
the `justfile`): it ships the snippet, you merge it.

| Tool | Mechanism | Merge | Confidence |
|------|-----------|-------|------------|
| **Claude Code** | `PreToolUse` hooks | `settings.snippet.json` → `.claude/settings.json` | verified against the hook API |
| **Cursor** | agent hooks (`.cursor/hooks.json`, `version: 1`) | `cursor-hooks.snippet.json` | shell tier verified; **no pre-edit hook exists** (`afterFileEdit` is post-hoc), so tier 2 is shell-only |

One script serves both. Cursor's `beforeShellExecution` payload
carries `command` at the top level (Claude nests it under `tool_input`) and expects
`{"permission": …}` (Claude expects `hookSpecificOutput`); `bash-guard.mjs` detects
the dialect from the payload and answers in kind. Exit 2 means "blocked" to both.

## Verifying it works

The guards are plain Node with no dependencies, so they are testable — and tested,
in `test/hooks.test.mjs` of this library. In a consuming repo:

```bash
echo '{"tool_input":{"command":"git commit --no-verify -m x"}}' \
  | node .dev/kit/common/hooks/bash-guard.mjs; echo "exit=$?"   # → exit=2
echo '{"tool_input":{"command":"git commit -m \"drop the -n flag\""}}' \
  | node .dev/kit/common/hooks/bash-guard.mjs; echo "exit=$?"   # → exit=0, silent
```

A hook that is wired but never fires is indistinguishable from no hook at all —
run the two lines above once after merging the snippet.
