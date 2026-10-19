---
paths:
  - "**/docs/EXPERIENCE.md"
  - "docs/experience/**/*.md"
  - "**/*.tsx"
title: "Experience Contracts — One Journey, One Actor"
---

Stabilize one screen or workflow for one actor profile. A portal shares a toolkit,
not a universal journey: an expert and an occasional user may reach the same
service through different interactions. Never unify them merely for consistency.

## Two independent dimensions

- **Status**: `exploring` (the developer is trying alternatives) or `stable`
  (the developer explicitly selected the behavior to preserve).
- **Visual policy**: `toolkit` (shared tokens/components, composition remains free)
  or `specified` (the supplied references constrain the named screen/states).
  Either policy can apply to either status; never infer it from team size.

API contracts, authorization, data protection, typing and business ownership apply
in both states. Exploration permits local UI experiments, not bypassing those
invariants or weakening an existing behavioral test to get green. Reuse suitable
shared components; promote a local experiment only when retained and reuse warrants
it. Do not demand a generic component before trying an interaction.

## One home and a small contract

`docs/EXPERIENCE.md` indexes `docs/experience/<journey>-<actor>.md` from the first
new contract. Keep the index to links and a short purpose; status lives in the unit.
`docs/DESIGN.md` holds the shared toolkit. Read only the contracts affecting the
change. Existing single-file experience documents remain readable: migrate the
selected journey when requested, preserving other content and existing constraints.
Absence of the new format never cancels a developer's previous requirements.

Use the following Markdown fields and sections (one-line fields, no YAML parser).
IDs are unique lowercase slugs. Local links are relative to their containing file.

```markdown
# Experience — <journey and actor>

- **ID**: <journey-actor>
- **Actor**: <one profile and relevant context>
- **Scope**: <screen or workflow, entry and exit boundary>
- **Status**: exploring
- **Visual policy**: toolkit

## Outcome
<What the actor accomplishes.>

## Flow
<Entry → actions → feedback → completion; alternatives for this actor.>

## Invariants
<Observable properties that must hold; business rules link to their source.>

## Recovery
<Loading, empty, failure, cancellation and retry behavior where relevant.>

## Freedom
<What may vary: composition, wording, assistance, local components, etc.>

## Evidence
<Property → test, observed walkthrough, or human review; say unverified where so.>

## Visual references
None — toolkit only.
```

For `specified`, replace None with Markdown links to the supplied specifications
(local files or HTTPS URLs), identifying the applicable screens/states and revision.
For `stable`, add `- **Validation source**: <date + developer instruction or review
reference>` to the fields. Never invent that source or infer approval from green
tests. Evidence may still be unverified: acceptance and verification are different.

## Changing a contract

The developer chooses when an experience is conclusive, with stakeholder reviews
where applicable. On that explicit instruction, an agent may record `stable` and
its source. No ADR or manual status commit ceremony is required for a journey.

A developer's explicit correction authorizes updating behavior, contract and its
checks together; record the reason under `## Decisions Log`. It does not authorize
changing an Accepted ADR. An agent must not silently rewrite an invariant, downgrade
`stable`, or remove a visual requirement to make its own implementation conform.
When authorization is missing, propose the change and surface the decision.

## Verification

Protect transitions and outcomes with integration/E2E tests where appropriate;
assert screen geometry only when specified. Test shared invariants across affected
profiles, not identical paths. Mutation diagnoses missing assertions, dead code or
equivalent mutants; it does not prove usability or visual conformity.

T3 review distinguishes a demonstrated contract violation from a suggestion and
from an unverified property. Cite contract ID and property for a violation; report
actual evidence (code read, test result, walkthrough or visual comparison). Static
review never claims to have exercised a journey. Unavailable UI/spec access is a gap,
not a fabricated success or defect. Human usability judgment stays named as such.

`just docs-check` checks structure, fields and local reference existence, not human
authorship, remote reference contents, behavior, or visual conformity.
