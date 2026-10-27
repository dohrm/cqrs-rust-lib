---
title: "Decisions"
---

An ADR is where a decision stops being an opinion. That transition is the
**human/machine validation boundary**: an agent can research a decision, argue it,
and write it down — it cannot be the thing that declares it settled. `autonomy.md`
draws the other half of the same line: a green gate is permission for **code**,
never for a **decision**.

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

## Challenge a decision before changing it

A request to challenge a design, a reported limitation, or dissatisfaction with
its behavior is enough to explore alternatives. No special research mode is
required. An accepted ADR records the current arbitration; its status is not
evidence that the design still meets today's needs.

For a focused challenge, use `/solution-exploration` when installed. If the skill
is absent (for example in an agent-only installation), follow this procedure
directly: compare credible options from the problem and current non-decision
evidence first, and make a provisional recommendation visible before reading
ADRs or decision-log sections of
`docs/ARCHITECTURE.md`. Then read the original rationale, including rejected
options, check which constraints still hold, and map impacts and migration costs.
Recommend against change when the evidence supports that conclusion. Keep the
exploration scoped to the reported problem; do not reopen unrelated decisions.

Exploration does not authorize implementation. Complete the comparison before
handing back; the manual status change and guard apply when adopting a different
decision, not when examining it. Keep alternatives as proposals and the accepted
record's decision intact until the human arbitrates.

## Which choices need an ADR

A project ADR records an arbitration that passes **all three** tests: it is hard to
reverse, it constrains more than one feature, and it refused real alternatives at a
real cost. Changing an Accepted decision is the fourth case. Everything else has
another home:

| Choice | Home |
|---|---|
| Enterprise policy — security, secrets, identity, data protection, observability, deployment | the shared decision repository the project references, if any; propose changes there |
| Stack conventions — language, libraries, style, profile selection | the installed rules and the lock file; recorded once with the stack decision |
| Feature-local choices — a field, a format, a validation rule, an event, a layout | the feature's work document under `.work/` and its PR |
| Product and design rules | the PRD, `DESIGN.md`, `EXPERIENCE.md`, domain documents |

A deviation from a rule is an **in-place exception with its reason**, next to the
code it excuses, like a `nolint` with a justification. It is not an ADR. When the
same deviation recurs, the rule is wrong: propose the change to the rule.

Local, reversible choices belong to the implementer. Explain relevant dependency
trade-offs in the change summary and run the supply-chain checks. Novelty alone is
not architectural significance, and neither is the wish to stop an agent from
changing something later.

## Shared decisions

An organization's enterprise policy lives in its **shared decision repository**,
declared once per project in the header of `docs/ARCHITECTURE.md`:
`> Shared decisions: <repository URL>` (or `none`). Ask for it when a project starts
(`/architect`, `/onboard`) and whenever it is missing; record the answer.

- Read the shared records that bear on the change — through the forge CLI (`gh`,
  `tea`) or a local checkout, read-only. An accepted shared record binds like an
  Accepted project ADR; a project change that contradicts it is an escalation.
- When the project shows something with impact on the organization — a practice
  worth generalizing, a shared decision that hurts here, a policy gap, a repeated
  project ADR that is really enterprise policy — **propose a PR on the shared
  repository**: a `Proposed` record, with this project's evidence. Prepare it, then
  ask before opening it: a PR is outward-facing. Never merge it, never set a status.
- Write it for the shared repository's audience: no detail the project would not
  share with every other project of the organization.

## The ceremony stops at the ADR

An ADR is the **only** document with a ceremony. Generalising it to the rest is how a
repository loses the ability to change its mind.

| Document | Regime |
|---|---|
| `docs/adr/**` | **Ceremony** — the rule above, and `agent/decision-records.md`. |
| `EXPERIENCE.md`, `docs/experience/*`, `DESIGN.md`, `DATA-MODEL.md`, `ARCHITECTURE.md`, the PRD, `.work/*` | **Amended in place** — a human's feedback on the built artifact outranks prose written before that artifact existed. |

The asymmetry is about what the document holds, not about how much it matters. The
others are **descriptive**: they say what the product does, so the running product is
a better source than the guess that preceded it. An ADR is an **arbitration** — what
was refused and at what cost — and `Alternatives considered` is not something you can
observe on a screen. Only a new arbitration replaces an arbitration.

Experience contracts can be `stable` on the developer's explicit instruction
(`product/experience.md`). This protects retained behavior, not an ADR-like
ceremony: agents may record the instruction and subsequent authorized corrections.
Amendable does not mean disposable — never change a retained invariant or its
status just to justify an implementation. Review may flag a demonstrated regression
against that contract; visual preferences with no supplied requirement remain advice.

So when a human corrects the thing they have just used:

- **Apply it, and amend the document in the same commit** — one line in its
  `Decisions Log`. Behavior change ⇒ doc change still holds (`agent/guardrails.md`):
  what is banned is the silence, not the change.
- **Never send them back to the framing skill.** `/experience`, `/design-system` and
  `/prd` frame a product; they do not ratify a correction their own author has
  already made. Re-running one to record a fix is ceremony where there is none.
- **If the correction contradicts an `Accepted` ADR, stop short of applying it**:
  name the record, write the superseding one as `Proposed`, hand back. That costs the
  human one status line — the whole ceremony, and not a new framing session.

## Before you write one

The shape — statuses, the one-record-one-decision test, section budgets,
`Implemented`, amendments, what `just adr-check` enforces — is
**`agent/decision-records.md`**, path-scoped to `docs/adr/`. Creating the first
record of a session may not open one, so **read it before writing rather than
guessing the format**: a record of the wrong shape is one a human postpones, which
is a decision that does not happen.
