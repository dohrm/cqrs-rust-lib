---
title: "Decisions"
---

An ADR is where a decision stops being an opinion. That transition is the
**human/machine validation boundary**: an agent can research a decision, argue it,
and write it down — it cannot be the thing that declares it settled.

This is the counterpart to `autonomy.md`. There, the agent closes its own loop and
a green gate is authority. Here it does not: a green gate is permission for
**code**, never for a **decision**. The two rules do not conflict; they draw the
line between what a machine may settle and what it may not.

## The rule

- **An agent writes `Proposed`, and nothing else.** `Accepted`, `Rejected`,
  `Superseded by ADR-XXXX` and `Deprecated` are set by a human, in a commit — not on
  the strength of the agent's own reasoning, not because the code that goes with it
  is already written and green, not because the human discussed it at length in the
  conversation. Discussing is not accepting, and nothing in the repository
  distinguishes the two afterwards.
- **Say it in the hand-back**: what is proposed, what the alternatives were, and
  what changes if the answer is no. An ADR that lands silently has skipped the
  boundary even if its status line is honest.
- **Amending an accepted record's prose is fine** — a consequence learnt in
  practice, an argument that turned out to be wrong. Moving its status line is not.
- **Never fake a mandate.** An ADR is a record of a human decision; writing one to
  make a choice already implemented look authorised is the documentation equivalent
  of `--no-verify`.

## Before you write one

Everything else — the status vocabulary, the one-record-one-decision test, the
section budgets, the `Implemented` section, amendments, and what `just adr-check`
enforces — is in **`agent/decision-records.md`**. It is path-scoped to `docs/adr/`,
so it loads on its own the moment you open a record.

**Creating the first record of a session may not open one**, and then it will not
have loaded. Read it explicitly before writing rather than guessing the format: a
record that is the wrong shape is a record a human postpones, which is a decision
that does not happen.
