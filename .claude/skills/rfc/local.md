# RFC — local mode

The RFC is **one file, `.work/<slug>/rfc.md`**: its current state *and* its whole
audit trail. Nothing else records what happened, so the file keeps it all. Committed
working memory (`product/documents.md`); deleted once the PR merges, with the local
decisions that matter copied into the PR description.

## The file

`<rfc-body>` from `SKILL.md`, followed by these sections:

```markdown
## Assumptions

- <question settled without asking> — <the answer taken, and why>

## Blocked on the human

- <what you need, from whom, and what stays blocked meanwhile>

## Revisions

- <round N>: <what changed in the RFC, one line per change>

## Log

- <step or event>: <outcome, proof result> — `changed: <decision> — <why>` when a
  discovery changes a local decision; `human: <what and why>` when the human edited
  the tree
```

## A round

Revise the RFC in place, then append one line per change under `## Revisions`.
Answer a comment where it was made, and in the file — not only in the chat. A
question settled without asking (L2+) goes under `## Assumptions`.

## Implementation

Tick each step as its proof passes, and append its outcome to `## Log`. The status
line is the state; `just status` and `just publish-summary` read this file.

At L4, `## Log` also takes the `ODD:` lines (or `dropped to L3: <why>`), the dependency
graph before any parallel phases, and each delegated decision with its proof. The final
report is the `rfc/<slug>` PR's description, because this file is deleted at the merge.

## `ready` and escalation

- `ready` is the `**Status**` line, changed and committed by the human. Its commit
  author is the record of who set it.
- Escalate under `## Blocked on the human` — the section `just status` surfaces.
