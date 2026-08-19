---
paths:
  - "docs/adr/**/*.md"
  - "**/docs/adr/**/*.md"
title: "Writing a Decision Record"
---

The boundary this serves — an agent proposes, a human decides — is in
`agent/decisions.md`, and it is always in context. This is the shape of the record
itself, and it loads when you open one.

## Statuses

| Status | Meaning | Who may set it |
|--------|---------|----------------|
| `Proposed` | Written and argued, not yet decided. | **Agent or human** |
| `Accepted` | Decided. Binding on the code, cited as settled. | **Human only** |
| `Rejected` | Considered and refused. Kept, because the reasoning is worth keeping. | **Human only** |
| `Superseded by ADR-XXXX` | Replaced by a later decision. | **Human only** |
| `Deprecated` | No longer applies, with nothing replacing it. | **Human only** |

A status may carry a qualifier after the first word — `Accepted — provisional,
review scheduled` — as long as the first word is one of the five.

## One record, one decision

An ADR records **one** decision. The test is mechanical: if a sentence outside the
`Decision` section says *"we will"*, it is a second decision and it needs its own
record. `Consequences` is where costs are named, never where a new choice is
smuggled in.

Records that decide three things at once cannot be accepted separately, cannot be
superseded separately, and are read by nobody — which defeats the point of writing
them.

## Size — an ADR is one screen

The reader is a human who has to decide. A record they postpone is a decision that
does not happen, so length is not a neutral property of the document.

| Section | Budget |
|---|---|
| Context | ~120 words — the forces, as facts. Not a history of the discussion. |
| Decision | ~150 words — *"we will …"*, precise enough to act on. Bullets, not paragraphs. |
| Consequences | ~100 words — what gets harder, the costs knowingly accepted. |
| Alternatives considered | 2–4 bullets, **one line each**: the option, then the trade-off that killed it. |
| Implemented *(optional, post-hoc)* | ~150 words — see below. |

**~400 words for the whole record, 600 as a hard ceiling.** Past that, one of three
things is true, and the fix is never to compress the prose:

- it decides more than one thing → split it;
- it describes *what the thing looks like* rather than *why it was chosen* → the
  description belongs in another document (below), and the ADR links to it;
- it is arguing with an objection nobody raised → cut it.

**Where the overflow goes instead:**

| Content | Home |
|---|---|
| Field lists, schemas, types, table shapes | `docs/DATA-MODEL.md` |
| Screen behavior, states, wording, display rules | `docs/EXPERIENCE.md` |
| Component boundaries, stack overview, the decision index | `docs/ARCHITECTURE.md` |
| Sequencing, phases, what ships when | `docs/PLAN.md` |
| The argument for the decision, and its cost | **the ADR** |

**No section outside the canonical set** — Context, Decision, Consequences,
Alternatives considered, Implemented. An invented heading is an overflow valve: it
is how a 400-word record becomes a 2,000-word one.

## Implemented — closing the loop

Once the decision is built, the record may gain an `Implemented` section, written
**after** the code so the repository and its rationale agree. It is not a changelog
and not a summary of the implementation. Only three things earn a line:

- **a claim the code now proves** — the central justification, no longer an argument
  but a test that fails without it (name the test);
- **where reality diverged** — cheaper, more expensive, or different from what the
  decision predicted. Say which, in one sentence;
- **what exists but is not load-bearing yet** — a column nothing reads, a hook
  nothing calls. A reader must not assume it carries weight.

Writing it is not accepting anything: it is prose, it never touches the status line,
and an agent may write it on its own. If implementing revealed the decision was
**wrong**, that is not an amendment — it is a new ADR that supersedes this one.

## Amendments

Amending an accepted record's prose is allowed (a reason that turned out false, a
consequence learnt in practice). An amendment is a **dated note of five lines or
fewer**, appended under the section it corrects. Anything longer is a new decision
wearing an amendment's clothes: write the new ADR and let it supersede.

## Accepting one

The human reads it, changes the status line, and commits that change:

```
- **Status**: Proposed          →      - **Status**: Accepted
```

The commit is what makes the acceptance real — and it is the one signal an agent
does not produce on its own, which is why the gate keys on it.

## The gate

`kit/common/adr-check.mjs`, wired as `just adr-check`, fails when an ADR has no
status or an unknown one, when a **new** ADR carries anything but `Proposed`, and
when the **status line** of an existing ADR differs from the committed version.
Amending prose stays green; so does moving a status *down* to `Proposed`, because
withdrawing a claim is not making one.

It **warns**, without failing, on a record over the 600-word ceiling or carrying a
section outside the canonical five. Those are judgments a human makes, and a gate
that fakes one is worse than no gate; the warning exists so the drift is visible
while writing instead of at review. A repo that wants the budget enforced passes
`--strict`, which turns both into failures.

600 is a **default**. A repo moves it in `.docs-budgets.json` at its root —
`{ "adr": { "unitCeiling": 900 } }`, or `null` for no ceiling — the same file the
document budgets live in (`../product/documents.md`), and one the installer never
writes, so an update cannot reset it. The canonical section list is not a setting.

Like every gate: it is a file, so it can be edited. It is not a wall. It is there so
that skipping the step has to be a deliberate, visible act rather than an oversight.
