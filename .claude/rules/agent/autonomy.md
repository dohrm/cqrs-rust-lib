---
title: "Agent Autonomy"
---

An agent that writes code closes its OWN loop against the gates. The machine
enforces correctness; the human judges design.

## The loop

1. Write the code AND its tests.
2. Run **`just check`**. No justfile → `lefthook run pre-commit --all-files && lefthook run pre-push --all-files`.
3. Read the failure, fix the ROOT CAUSE, re-run until green.
4. Once the block stands alone — **before pushing** — run `just code-review` and
   fix `CRITICAL`, then loop to 2. **Mutation is not in this loop**: it is a gate on
   the pull request. Run `just mutate-diff` yourself only to avoid a
   round trip on code you have reason to doubt — it is an optimisation, never a step
   you owe.
5. Hand back only after a **fresh green run**. Belief is not permission. Do not
   trust a prior run's claim — re-run and read the exit code.

Cadence — none of these is "wait for the human" or "wait for the PR":

| When | Command | What it answers |
|---|---|---|
| Per iteration | `just check` (Tier 1-2) | fmt, lint, tests, deny — seconds |
| Per RFC phase, before its push | `just code-review` (Tier 3) | judgment a gate cannot make — minutes |
| Per push | CI | a **witness**, same tools on the PR diff |
| Per pull request | `mutate-diff` in CI (Tier 4) | do the tests *assert*? — **the gate**, not a witness |

**The coherent block is the RFC phase** — one PR — not the step. Say it plainly because the
drift is one-way: a loop that commits per task starts running Tier 3 per task, and
then Tier 3 is a tax somebody eventually removes.

Tier 3 is code review; Tier 4 is mutation (or the weaker Go coverage ratchet).
Review is minutes and its feedback can redesign a block, so it stays close. Mutation
was measured at tens of minutes, and its remaining lever is `-j` — cores a laptop
cannot spend while the editor and the agent are using them. It moved to the PR,
where it **blocks the merge once calibrated and ratcheted**. Shipped snippets
start non-blocking to measure the baseline; declare that state rather than claiming
enforcement. One round trip per survivor is accepted as the price.

`just code-review` writes `.work/review-report.md`. `just review-guard` (pre-push,
no LLM) reads it. Marker rules: `.dev/kit/common/review-guard.mjs`.

**Detect, never assume.** Tier 3 is sometimes absent (`just --list`):

- recipe present → running it is part of "done";
- recipe absent → say so in the hand-back. Never silently skip, never assume green.
  No report → `review-guard` passes and tells you to declare it.

## Authority

A green gate is permission for **code**. An agent or review opinion is a proposal.
A green gate never settles a **decision** — that line is `agent/decisions.md`.

## Levels

How often the agent stops to ask is a per-run choice, declared once in the state
file header — `.work/<slug>/rfc.md` or `loop.md`:
`**Autonomy**: L1 | L2 | L3 | L4`.
The human picks it at launch, from the model *and* the risk of the work; a level
names a delegation, never a model. **Absent or unreadable → L1.**
A level passed at launch is written into that header before the first turn: the
state file is the source of truth, so a level that lives only in the prompt is lost
at the next one. A skill that runs another passes its resolved level as the argument
(`/rfc L3`) — the callee's own lookup would fall back to an older header or to L1.

| Level | The human validates | The agent settles alone |
|---|---|---|
| **L1 — guided** | every checkpoint a skill marks, before the next step | nothing a skill marks as a checkpoint |
| **L2 — delegated** | each skill's written output, once | the open questions the code answers, each one recorded |
| **L3 — autonomous** | only the hard checkpoints below | the RFC's local decisions and steps, the loop's caps, launching once `ready` (a Codex `/goal` included), sub-agents |
| **L4 — orchestrated** | the hard checkpoints the RFC's delegation board does not delegate, and the final merge | L3, plus the checkpoints on the board, inside a domain checked at launch (§ L4) |

**Hard checkpoints — identical at every level:** an ADR status (`agent/decisions.md`);
a new acceptance criterion, User Story or scope change; an RFC's `ready`, a
capability's `Shipped` and the merge; a hard bypass (below); a tree that contradicts the state file.
At L4 only, an RFC's delegation board may take a few of them off this list (§ L4).

A level moves *when* the human is asked, never *what* counts as done. The gate,
escalation channel and one-tree-one-writer hold at L3 exactly as at L1. A loop
retains its caps and divergence guard; at L3 the agent sets their values itself
and writes them before the first turn. An RFC retains its stopping condition and
the proof of each step at every level. Under L2+, every question settled without asking is written
where the human will read it — the document's `## Assumptions`, else `## Log`. An
unrecorded assumption is a silent scope change.

**Only the human raises a level.** An agent may always drop one: an L3 run facing an
ambiguity the code cannot settle asks, which is escalation, not failure.

## L4 — delegation inside a checked domain

L4 is L3 plus the checkpoints an RFC's `## Delegation board` delegates. No
board, or an empty list, means L3. A loop has no delegable checkpoint: `loop.md` reads
L4 as L3.

| The board may delegate | Never delegable |
|---|---|
| a phase merge into the RFC's integration branch `rfc/<slug>` | an ADR status; a new acceptance criterion, User Story or scope change |
| a hold whose check is a command (a date included) | a hold that needs production, or someone else's sign-off |
| a `changed:` local decision | a hard bypass; the merge of `rfc/<slug>` into the base branch; `Shipped` |

**The domain is checked at launch, never assumed.** Before the first phase, prove each
prerequisite and log one `ODD:` line per check:

- the stopping condition runs and fails for the reason the RFC gives. When the RFC
  writes its own witness, the condition already names it and fails on its absence: a
  runner that skips a missing file passes, which is not failing first. A change whose
  condition cannot fail first, pure prose for instance, cannot run at L4;
- every delegated hold has a check command;
- every executor on the board is installed and answers;
- `.work/<slug>/rfc.md` is committed on `rfc/<slug>`, so every phase tree has it;
- the caps are written.

One failure → rewrite `**Autonomy**` to L3, log `dropped to L3: <why>`, and go on at L3.

**Outside the domain, reach the minimal-risk condition**: a case the board does not
cover, a cap reached, a phase rejected past its cap. Merge nothing half-done into
`rfc/<slug>`, leave the phase branches as they are, escalate under
`## Blocked on the human`, and stop.

**The orchestrator is the executor's human.** It owns the RFC and the integration tree,
and is the only writer of `rfc.md`. Each phase goes to an executor — a coding agent and
a model the board lists, as an in-harness sub-agent or a detached process (`claude -p`,
`codex exec`, `opencode run`) — in its own tree on `phase/<slug>/<n>`.

- **It delegates only what it received.** An executor runs at L3 at most, picked for
  its model, and merges nothing. Its escalation goes to the orchestrator, which settles
  it if the board covers it and escalates to the human otherwise.
- **The brief is the RFC and the phase number**, never a summary rewritten from it.
- **It judges artifacts, not accounts.** It re-runs `just check` on the phase branch,
  reads the diff and the review report, and merges on gates green, Tier 3 clean and
  Tier 4 green in CI. The executor's report is a claim.
- **It never writes in an executor's tree** while that executor runs.

Reviewers are unrestricted: a stronger or different model reviewing is where
independence is cheapest.

**The human merges the RFC.** The `rfc/<slug>` → base PR carries the final report:
every delegated decision with its proof and reversibility, the assumptions, the
`changed:` lines, each hold with its check output, and where the run diverged from the
plan. A hold that needs production is a merge point: the human merges what precedes it,
and the run resumes from the new base once the hold's check passes.

## One tree, one writer

`.work/` (review report, RFC) is per-tree. Two sessions in one checkout
share one verdict: A's `CLEAN` authorises B's push. **One tree, one writer, one
branch, one RFC.**

Parallel work gets a parallel tree, at a path you do not have to remember:

```bash
just tree <slug>     # prints the path; one tree per RFC
just tree-rm <slug>  # tree AND branch, at the merge, in one act
```

Every tree on the machine lives under one root (`$CR_WORKTREES`, default
`~/.worktrees/<repo>/<slug>`) — outside every repo, because a checkout nested in a
repo is one every tool scans. **One tree, one branch, one RFC**: `.work/<slug>/` is per-tree and holds that
RFC, so any other split puts one work unit in two trees.

**Parallelise across RFCs that touch disjoint code, never across phases.** The
phases of one RFC traverse the same layers and collide by construction: they run in
sequence, in its tree.

**At L4, the orchestrator decides.** Each phase gets its own tree
(`just tree <slug>-p<n> phase/<slug>/<n>`, run from the integration tree). Before
running phases in parallel, it writes in the RFC the dependency graph it relies on.
A merge conflict goes back to that phase's executor to rebase. The orchestrator never
resolves it in the executor's tree. In a phase tree, the executor writes nothing under
`.work/<slug>/` except the gates' files and the review's ticket cache. The RFC lives in
the integration tree.

The tree dies with the work. `tree-rm` removes nothing unless the tree is clean and
the branch is merged, so a forest of detached branches is something you have to
build on purpose. `just status` reports every tree; it never gates.

The human is a writer too. When they edit the tree a loop or an RFC owns — legitimate, and
declared as a `human:` line in `## Log` (`skills/loop-setup` or `skills/rfc`) — that line is a
**fact about the tree**, not an item to redo or undo. So: re-read the code you are
about to touch, every turn; your memory of the previous turn is not the tree. And
**never revert a change you cannot explain** — a tree that contradicts the
RFC or loop is an escalation, not a merge conflict to resolve on your own.

## Never fake green

- **Hard — never bypass; escalate.** Correctness, security, `secret-scan`,
  behavioral tests. No `--no-verify`, no disabling a gate, no `#[ignore]` /
  `.skip`, no weakening an assertion. Same for excluding a mutant you could
  kill, lowering a ratchet baseline, or hand-editing `.work/review-report.md`.
  Cannot fix the cause → **STOP and ask**. The green must be true.
- **Soft — traced bypass allowed.** Minor style/lint only, with a justification
  comment on the same line. No bare `#[allow]` / `eslint-disable` / `# noqa` /
  `# type: ignore`.

The git floor (`lefthook`) and the harness layer (`kit/common/hooks/`, opt-in)
make bypass expensive and loud, not impossible. Wiring:
`.dev/kit/common/hooks/README.md`. This rule is still the rule.

**Declare every bypass.** No silent TODO, skipped test, placeholder, or stub.

**Escalate in that tree's `.work/<slug>/rfc.md` or `loop.md` → `## Blocked on the human`** —
what `just status` surfaces. Never in the review report.
