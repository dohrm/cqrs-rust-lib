---
name: interview
description: "Frame a product or ONE new feature as a decision tree: work the frontier a round at a time, always with a justified recommendation. Writes `.work/<slug>/intent.md` — what is still open. Use on /interview, \"help me plan\", \"I have an app idea\", \"let's dig into this feature\". Upstream of /prd. Wiring an existing repo onto the workflow is /onboard, not this."
---

Interview me relentlessly about every aspect of this plan until we reach a shared understanding.

## What is being framed — the axis this skill is about

Two questions get confused here. **What are we framing** is this skill's axis.
**Does the repo have code** is not.

| | The repo is empty | The repo has code |
|---|---|---|
| Frame the product | **this skill** | **this skill** |
| Frame ONE new feature | n/a | **this skill** |
| Wire the repo onto this workflow | `claude-rules add` | `/onboard` |

So a new feature on a living codebase is **this skill**. `/onboard` inventories an
existing repo to install the harness and frames no product at all — send them there
only when the job is the wiring, and to `/migrate` when a `.claude-rules.lock` is
already in place.

What a living codebase changes is not the process, it is the tree: the code has
already settled branches you would otherwise ask about. Read them, do not ask them
(*Finding facts is your job*, below), and keep the tree to the feature — a question
that reopens the whole product is a signal you are framing the wrong thing.

## Map the decision tree

Every decision branches into the decisions that hang off it. Sketch this tree
before asking anything — even roughly, even if it keeps growing as we go.

## Work it in rounds, on the frontier

The **frontier** is every decision whose prerequisites are already settled — the
questions askable *now*, without guessing at an answer I haven't given yet. Ask
the **whole frontier in one round**, not one question at a time: batching what's
independently answerable is faster for both of us, and asking them one by one
pretends an ordering exists between decisions that don't actually depend on each
other.

Format a round like this:

<round-template>
**Q1 — <question title>**

<question body — may run multiple paragraphs, may offer multiple choices>

**Recommendation:** <your recommended answer, with its rationale — you are a
thinking partner, not a form>

---

**Q2 — <question title>**
...
</round-template>

Wait for the answers to the **whole round** before continuing.

## Recompute and repeat

Each answer reshapes the tree: a settled decision pushes the frontier outward
and can unblock questions that depended on it. Recompute the frontier and ask
the next round. A question whose prerequisite isn't settled yet does not belong
in the current round, however tempting.

**Finding facts is your job, never mine.** When a frontier question needs a fact
from the environment — the codebase, a config file, an existing convention —
rather than a decision from me, go find it yourself (explore the repo, dispatch
the Explore subagent for a wider search) instead of spending a round asking me
something you could have looked up.

## Done

The session is done when the frontier is empty: every branch of the tree
visited, nothing left silently assumed. Say so, and don't act on it — don't hand
off to `/prd` — until I confirm we've reached a shared understanding.

**An empty frontier is not the same as nothing left open.** The frontier is what a
round can still ask *me*. A question no round can settle — it needs a technical
decision (`/architect`), a measurement, or a real user — was never a frontier
question. It is an **open question**, and it is the reason the next section exists.

## Write it down

Write `.work/<slug>/intent.md` from `<intent-template>` — the brief `/prd` reads.

It is **working memory, the same nature as an RFC**: committed
so a PR shows what was being framed, rewritable any turn, and deleted with
`.work/<slug>/` once the capability ships (`product/documents.md`). Nothing durable
reads it afterwards: what was promised ends up in the PRD, what was decided in an
ADR — including the options that lost, which every ADR carries under *Alternatives
considered* (`agent/decision-records.md`). So there is nothing here to archive.

- **Slug**: provisional. `/prd` names the capability and renames the directory if
  they differ. Do not stall the interview inventing a durable name.
- **Skip the file** when the tree collapsed in one round and `/prd` runs next in
  this session — the conversation is still the brief. A file written to be deleted
  twenty minutes later is ceremony.
- **The intent asks; the PRD and the ADRs answer.** An answered question leaves the
  open list and becomes one `## Settled` line pointing at its new home. Never copy
  the answer back — a second copy of the PRD is exactly the failure
  `product/documents.md` exists to prevent.

<intent-template>
<!-- `.work/<slug>/intent.md`. Working memory: rewritable every turn, deleted with
     the directory when the capability ships. Committed until then. Never under docs/. -->
# <slug> — intent

- **Capability**: <`docs/PRD.md` § NN — or `not framed yet`>
- **Framed**: <date> · **Slug**: <provisional until `/prd`, or confirmed>

## Problem

<What someone cannot do today, in the originator's own words. Who lives with it.
 No solution, no technology.>

## Proposed outcome

<What becomes true. One paragraph — success criteria are the PRD's job, not this file's.>

## Affected users and systems

<Who it serves, and what it touches in the code that already exists — named
 anchors where the interview found them. Not a design: a blast radius.>

## Constraints

<What is not negotiable, and where it comes from — an ADR, a regulation, a date.
 Cite the source; never restate an ADR's reasoning.>

## Open questions

<!-- The reason this file exists. An empty list means this is a capability, not an
     intent — hand it to `/prd` and stop maintaining this file. -->
- [ ] <question> — <what it blocks, or what it changes downstream>

## Settled

<!-- Append-only, one line per closed question: what was decided and WHERE the answer
     now lives. The answer itself never stays here. -->
- <date> — <question> → <`docs/adr/NNNN-<slug>.md` | PRD § NN | dropped, because …>
</intent-template>
