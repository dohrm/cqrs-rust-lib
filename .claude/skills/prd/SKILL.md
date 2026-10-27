---
name: prd
description: "Frame or extend `docs/PRD.md` (stable spine + capability units). Use on /prd, \"write the PRD\", \"frame this project\". Names no technology. Upstream of /architect and /rfc."
---

You question the user to produce or extend `docs/PRD.md` using the template below.

## Process

1. Explore the repo if needed to understand existing context (`CLAUDE.md`, ADRs, domain glossary, adjacent code). Reuse the project's vocabulary in the PRD and respect architectural decisions already made. If the answer to a question is in the repo, explore instead of asking.

2. **The brief is `.work/<slug>/intent.md`** when `/interview` left one — read it first, and treat its `## Open questions` as your agenda: those are the questions an interview could not settle, so they are yours to close or to declare out of scope. A brief may also just be what the user typed. If `docs/PRD.md` (or `docs/prd/`) exists, read the spine and the capabilities the brief touches. Cross-check against the incoming brief and only question the deltas. Confront contradictions: *"You'd settled on X, the brief suggests Y — which do we keep?"*. No PRD and no brief → your first question is *"What do you want to frame?"*.

   **The PRD is meant to grow — the file is not.** Spine stays stable and one screen; growth is a capability unit (`product/documents.md` — read it for split thresholds and budgets). If the file is past the split, propose the migration before adding:

   > *"The PRD is at N capabilities / L lines. I'd split it: one file per capability in `docs/prd/`, `docs/PRD.md` keeps the spine and gains the capability table. Content moves unchanged. Go?"*

3. Ask one question at a time, with your justified recommendation. Follow the dependencies: Problem → Target User → Solution → Success Criteria → Out of Scope. User Stories and Implementation Decisions emerge across the others. Before moving on, ask *"Anything for Additional Notes: risks, dependencies, assumptions?"*.

4. When every section can be written with no gaps, say so in one sentence and write the full PRD in the chat using the template. User Stories are synthesized at this point from the other answers and presented for validation.

5. The user validates or corrects section by section. On a correction, re-post only the affected section. Once everything is validated, write it (create `docs/` if needed) and confirm what was written.

   **Two shapes** (`product/documents.md`): below the split, everything in `docs/PRD.md`; beyond it, spine + capability table in the index, one file per capability in `docs/prd/NN-<slug>.md`.

6. **Then settle the intent, if there was one.** The capability is now the home of the problem, the outcome and the success criteria, so they leave `.work/<slug>/intent.md`: every question you closed becomes one `## Settled` line pointing at the PRD section or the ADR that now answers it, and the `## Capability` line at the top gets its number. What stays is what is still open.

   If the capability came out under a different name, `git mv` the directory to the capability's slug — `/rfc` will start from it, and the first RFC of the capability can take the same directory. Nothing else references it yet, so the rename is free now and expensive later.

   An intent whose open list is empty has done its job; it is not deleted here — the whole directory dies when the capability ships (`product/documents.md`).

<prd-template>
<!-- The SPINE. ~1000 words, each section ~200 max. It is the stable part: new scope
     lands in a capability, not here. -->

## Problem

What the user lives through: frustration, context, "why now". Prose in the third person, not in *I*.

## Solution

Product direction from the user's point of view: what the product lets them do, not how it's built.

## Target User

Profile + usage context, precise enough to picture a real person.

## Capabilities

One line per capability — the actor, the job it does for them, its status, and
where its detail lives. This table IS the compaction: it must be enough to see
the whole product, and, once work starts, where the project actually stands.

| # | Capability | For whom, and the job it does | Status | Stories |
|---|-----------|-------------------------------|--------|---------|
| [01](./prd/01-<slug>.md) | <name> | <one line> | Not started | US-1…4 |

**Status**: `Not started` / `In progress` / `Shipped <date>`. Set by re-running
`/prd`, which reconciles it against `.work/<slug>/` and the git log rather than
asking — the plan and the code already know. This is the one place that keeps
saying "done" after `.work/<slug>/` is gone (`product/documents.md`).

## Success Criteria

Directly verifiable criteria: an observable event (a click, a produced file, a received email) or an objective measure (duration, count, threshold). No judgment of inner behavior ("understands X", "identifies Y"). Product-level only — a criterion that belongs to one capability lives with it.

## Out of Scope

What we explicitly refuse, one line each. Exhaustive — this is what guards against over-engineering.

## Additional Notes

Risks, external dependencies, assumptions. Keep it short; *"Nothing to report."* if empty. Not a place for deferred scope — that goes in Out of Scope.

</prd-template>

<capability-unit>
<!-- ~500 words. One capability. As `docs/prd/NN-<slug>.md`, or as a `###` block of
     PRD.md below the split threshold. -->
# <NN> — <capability name>

**For** <actor>, **so that** <the job it does for them>. <One or two sentences of context, only if the spine does not already carry it.>

## User Stories

*"As a `<actor>`, I want `<capability>`, so that `<benefit>`"* — numbered **US-n, unique across the whole PRD**.

At `/prd` time this section may stay a **stub**: the one or two stories obvious
enough to cadre the capability, no more. Writing the full, edge-case-covering set
this early is speculative — the capability may not be worked for months, and
`/rfc` will elaborate it against the real code right before it is. Full
coverage (main interactions, empty states, errors, alternative paths, edge
cases) is `/rfc`'s job when it frames this capability, not `/prd`'s upfront.

## Behavior decisions

User-visible product behavior for this capability: numeric limits, empty states, display format, UX choices, errors. No internal technical detail (algorithms, env vars, library names). Mental test: if the user cannot observe the difference in use, it does not belong here.

**Anything architecturally significant is an ADR, not a line here** — link it rather than arguing it (`agent/decisions.md`).

## Out of scope for this capability

What this capability deliberately does not do, one line each — distinct from the product-level Out of Scope in the spine.
</capability-unit>

## Rules

- Vocabulary = the user's, verbatim.
- No named technology, no gaps, no file path, no code snippet in the PRD.
- User Stories must be numbered: US-1, US-2 … and the numbering is **global**, never restarted per capability.
- The spine stays one screen, and stays stable. Growth is a new capability unit — never a longer section, never a `(continued)` heading.
- **One home per fact** (`product/documents.md`): why-this-technical-choice is an ADR, what-it-looks-like is `EXPERIENCE.md`/`DATA-MODEL.md`, the delivery order is the (ephemeral) RFC's phases. The PRD carries what, why-anyone-cares, and whether it has shipped — and links to the rest.
