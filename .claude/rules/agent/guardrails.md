---
title: "AI Development Guardrails"
---

This code is developed mostly by AI agents and reviewed by humans. Optimize for
code a tired human can understand six months later.

## Non-negotiables

- Implement the smallest milestone slice that satisfies the request. No abstraction
  before two concrete call sites prove it, and no framework pattern this milestone
  does not need.
- Business rules never live in UI, CLI, gateway, or connector code.
- A TODO needs a concrete issue, milestone, or stated deferred scope. Nothing else
  earns one.
- Never silently change user-visible behavior, filesystem layout, security policy,
  or milestone scope.
- **A dependency is a decision, not a convenience.** Say why in the change summary,
  and check it: needed by *this* milestone, maintained enough for the risk, clean
  through the supply-chain gate (e.g. `cargo deny`), not a duplicate of one already
  there.
- **Behavior change ⇒ doc change** — milestone scope and exit criteria, module
  boundaries, runtime responsibilities, filesystem layout, trust levels,
  sandbox/approval behavior, CLI commands, user-visible configuration.

## Before coding

Which milestone is this? Which module owns the behavior, and which boundary does it
cross? What is the smallest useful vertical slice? How will a test catch the failure?
Any answer unclear → update the docs or ask, before writing code.

## During coding

- Keep the change close to the owning module.
- Plain data and explicit functions over generic machinery; typed commands/events
  over strings and raw JSON; deterministic tests over mocked complexity.
- Keep public APIs narrow; make invalid states hard to represent where it is cheap.
- Error messages useful to the user or the operator, not just to the compiler.
- Leave existing comments alone unless the change made them false.

## After coding — before calling it done

Closing the loop is `agent/autonomy.md`. This rule adds three reads: the diff **as a
reviewer, not as the author**; dead code, unused dependencies, placeholder
abstractions and vague comments removed; docs still matching behavior.

## AI slop indicators (review blockers unless justified)

- Large generic modules named `manager`, `service`, `handler`, `utils`, `common`.
- New abstractions (traits/interfaces) with a single implementation and no clear
  testing or boundary value.
- Untyped blobs outside transport/protocol edges (e.g. Rust `serde_json::Value`,
  `any` in TS) used to model domain data.
- Optional fields used to model many different command shapes.
- Catch-all errors (e.g. `Other(String)`) in domain code.
- Tests that only assert construction, or mocks returning mocks.
- Comments explaining obvious code while omitting the real invariant.
- Rewriting, reformatting, or adding a comment the code change did not make
  false. **A comment-only hunk is a failed edit.** Update a comment only when
  the change made it untrue.
- New dependencies where the stdlib or an existing dependency is enough.
- Architecture changes without documentation updates.
- The same block pasted into three places, each copy drifting on its own.

Every indicator above is a judgment except the last one. Measure that one —
`just dup-check` if the repo wires it — then ratchet (`testing/ratchet.md`). A
green number is not permission to stop reading the diff.
