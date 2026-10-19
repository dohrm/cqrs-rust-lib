---
name: domain-modeling
description: "Build and sharpen the project's ubiquitous language in a living `CONTEXT.md`, and propose an ADR when a decision earns one. Use on /domain-modeling, \"what do we call this\", \"define the glossary\", \"audit CONTEXT.md\". For when the model changes — reading CONTEXT.md is everyone's habit."
---

Actively build and sharpen the project's domain model as you design: challenge
terms, invent edge-case scenarios, write the glossary and the decisions down the
moment they crystallise. Run this alongside `/interview`, `/architect`, or
`/onboard` — it is not a phase of its own, it is what keeps the vocabulary and
the decision log honest while those skills talk to the human.

Merely *reading* `CONTEXT.md` for vocabulary is not this skill — that is a
one-line habit any skill already has (`product/vocabulary.md`). This skill is
for when you are changing the model, not just consuming it.

## File structure

Most repos have a single context: one `CONTEXT.md` at the repo root, one
`docs/adr/` next to it.

If `CONTEXT-MAP.md` exists at the root, the repo has multiple bounded contexts —
the map points to where each one lives, its own `CONTEXT.md` and `docs/adr/`
alongside it. Infer which context the current topic belongs to; ask if unclear.

Create files **lazily**: `CONTEXT.md` when the first term resolves, `docs/adr/`
when the first ADR is needed. A missing file means nobody has needed one yet, not
that something is broken.

## During the session

- **Challenge against the glossary.** A term that conflicts with `CONTEXT.md` gets
  called out immediately: *"Your glossary defines 'cancellation' as X, but you
  mean Y here — which is it?"*
- **Sharpen fuzzy language.** A vague or overloaded term gets a precise canonical
  replacement proposed on the spot: *"'Account' — the Customer, or the User?
  Those are different things."*
- **Discuss concrete scenarios.** Stress-test a relationship with an edge case
  invented on the spot, and force precision about the boundary.
- **Cross-reference with code.** If what the user just said contradicts what the
  code does, surface it — don't silently trust the newer statement.
- **Update `CONTEXT.md` inline, the moment a term resolves.** Don't batch. Use
  `<context-template>`.

`CONTEXT.md` is a glossary and nothing else — never a spec, a scratchpad, or a
place for implementation decisions. Rules for the entries:

- **Be opinionated.** Several words for one concept → pick the best, list the
  rest under `_Avoid_`.
- **Tight definitions.** One or two sentences. What the thing **is**, not what it
  does.
- **Project-specific only.** A general programming concept (timeouts, error
  types) doesn't belong even if the project uses it heavily — ask "unique to this
  context, or general programming?" before adding a term.
- **Subheadings for natural clusters**, a flat list otherwise.

## Offering an ADR

Offer one only when **all three** hold — skip it if any is missing:

1. **Hard to reverse** — the cost of changing your mind later is real.
2. **Surprising without context** — a future reader will wonder why.
3. **A real trade-off** — genuine alternatives existed and one was picked for
   specific reasons.

If it qualifies it is a normal decision record, not a lighter format born of the
conversation having already happened: **write it `Proposed`**, same skeleton as
`/architect`'s `<adr-template>` (`agent/decision-records.md`), and hand back what
you propose and what changes if the answer is no (`agent/decisions.md`). Create
`docs/` and `docs/adr/` if absent; if `docs/ARCHITECTURE.md` doesn't exist yet,
create a minimal one from `/architect`'s `<architecture-template>` and add this ADR
to its decision index — otherwise append to the existing index.

## Consolidation pass

The during-the-session behavior above is reactive: it fires when a term comes up
in conversation. This is the other mode — a full, deliberate audit, for when the
glossary needs to be trustworthy on its own, not just accurate in the moment it
was last touched: before handing `CONTEXT.md` to someone non-technical, or
whenever it's been a while since anyone actively challenged it.

1. Read `CONTEXT.md` (or every context under `CONTEXT-MAP.md`) in full, and
   `docs/PRD.md`+`docs/prd/*`, `docs/ARCHITECTURE.md`+`docs/adr/*`.
2. Cross-reference every term against the code: does it still name something
   that exists, the way the glossary says it works?
3. Surface, don't silently fix: a term the docs use but the glossary doesn't
   define; a defined term nothing in the code or docs still uses; two terms
   drifting toward the same meaning; a definition the code now contradicts.
4. Propose the edits as a batch, get the human's validation, then write —
   same inline-update rule as during a session, just applied all at once
   instead of term by term.

<context-template>
# <Context name>

<One or two sentences: what this context is and why it exists.>

## Language

**<Term>**:
<What it IS, one or two sentences. Not what it does.>
_Avoid_: <synonym>, <synonym>

<!-- group under ### subheadings when natural clusters emerge -->
</context-template>

<context-map-template>
# Context Map

## Contexts

- [<Context>](./src/<context>/CONTEXT.md): <one line>

## Relationships

- **<Context> → <Context>**: <how they connect — events, shared types, calls>
</context-map-template>

## Rules

- `CONTEXT.md` is never implementation detail — that goes in `ARCHITECTURE.md`,
  `DATA-MODEL.md`, or the code itself (`product/documents.md`).
- An ADR born here follows the same status discipline as one born in `/architect`
  (`agent/decisions.md`). No format skips the gate because the conversation felt
  conclusive.
