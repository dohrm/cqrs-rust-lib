---
name: experience
description: "Explore or stabilize one screen/workflow for one actor (Web UX, CLI/TUI, DX). Writes a contract under docs/experience/ indexed by docs/EXPERIENCE.md. Use on /experience, \"explore this flow\", \"stabilize this experience\", \"empty/loading/error states\". Shared visual tokens belong to /design-system."
---

Help the developer discover an experience by trying it, then preserve the behavior
they retain. Work on one journey and actor at a time, not a portal-wide sign-off.
An expert's direct path and an occasional user's assisted path can share the same
service and toolkit without sharing their sequence.

## Frame the selected journey

Read `product/experience.md` for the contract format and lifecycle before writing.
Read `docs/EXPERIENCE.md` and the relevant unit if present; `docs/DESIGN.md` for
shared components and any supplied visual constraints; the relevant PRD capability
for the actor's job. Inspect the existing screen/command/API and nearby patterns.
A missing PRD or visual guideline does not block an experiment when the developer
has supplied enough context. Ask only for the missing actor, outcome or boundary.

Infer the requested mode from the user's instruction: explore by default;
stabilize only when the developer explicitly selects the experience to preserve.
Do not restart an existing document or ask for approval already given. Legacy
single-file docs remain constraints; migrate only the selected journey, preserving
unrelated sections and links. Do not label historical behavior stable without a
validation source supplied by the developer.

## Explore

- Establish the actor's job and the moment that should be effortless. Separate
  profiles when they need different assistance or entry points.
- Propose concrete alternatives for the uncertain interaction, with the cost and
  benefit for this actor. Work from a prototype or running product when available.
  Implement a scoped experiment when requested; otherwise describe the alternatives.
- Cover entry, action, feedback, completion and relevant unhappy paths: loading,
  empty, partial, failure, cancellation, retry. Include keyboard/accessibility needs.
  CLI/DX contracts also cover exit codes, output channels and recovery as applicable.
- Keep the contract `exploring`. Record the current hypothesis and what remains
  free, without converting every sketch into a permanent constraint. A local UI
  experiment need not become a shared component before it has proved useful.
- Consume the existing toolkit. If visual specifications are supplied as requirements,
  use `specified` and link them; otherwise use `toolkit`. Do not invent a visual guide.

## Stabilize

Start from the experience actually retained and the developer's review feedback.
Inspect it where tools/access allow; clearly state when only source or descriptions
were available. Stakeholder approval is supplied by the developer, never inferred.

Extract a short contract: actor, scope, outcome, flow, invariants, recovery and the
remaining freedom. Name observable behavior, such as "retry preserves the draft",
not a vague claim such as "intuitive". Keep layout free unless explicitly constrained.
Show unresolved ambiguities before treating them as guarantees; preserve the rest
of the authorized work. Record `stable` and the genuine Validation source on explicit
instruction; no extra ADR or manual status edit is needed.

For each retained property, map suitable evidence: existing or needed unit,
integration/E2E tests, a walkthrough, visual comparison for specified screens, or
human judgment. Wire/run the checks within the authorized implementation scope;
otherwise record them as unverified follow-up work. Never call the flow verified
because a document gate passed. Do not claim tests ran when only reading them.

## Write and hand back

Use the contract template in `product/experience.md`. Write one unit under
`docs/experience/` and link it from the compact `docs/EXPERIENCE.md` index. Keep
shared toolkit decisions in `docs/DESIGN.md`, business rules in their existing home,
and links rather than copies in the contract. A developer's later correction updates
the unit and affected checks together, with a short Decisions Log entry.

Run `just docs-check` if wired. Report the selected journey/profile, status, visual
policy, properties now protected, and remaining verification gaps. A request about
one journey never stabilizes another. No automatic new ADR for a screen or component.
