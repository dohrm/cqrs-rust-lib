---
title: "AI Development Guardrails"
---

This code is developed mostly by AI agents and reviewed by humans. Optimize for
code a tired human can understand six months later.

## Non-negotiables

- Implement the smallest milestone slice that satisfies the request.
- Do not add abstractions before two concrete call sites prove they are useful.
- Do not add dependencies for convenience wrappers, tiny helpers, or speculative work.
- Do not introduce framework patterns unless the current milestone needs them.
- Do not hide business rules in UI, CLI, gateway, or connector code.
- No TODO-driven architecture. A TODO is acceptable only when paired with a
  concrete issue, milestone, or explicit deferred scope.
- Do not silently change user-visible behavior, filesystem layout, security
  policy, or milestone scope.

## Before coding

Answer internally: Which milestone does this belong to? What is the smallest
useful vertical slice? Which module/crate owns the behavior? Which boundary does
it cross? What can go wrong and how will tests catch it? If unclear, update docs
or ask before writing code.

## During coding

- Keep changes close to the owning module.
- Prefer plain data structures and explicit functions over generic machinery.
- Prefer typed commands/events over strings and raw JSON.
- Prefer deterministic tests over mocked complexity.
- Keep public APIs narrow; make invalid states hard to represent where cheap.
- Keep error messages useful for the user or operator, not just the compiler.

## After coding — before calling it done

- Run the quality gates unless impossible (`just check` / the repo's gate).
- Read the diff as a reviewer, not as the author.
- Remove dead code, unused dependencies, placeholder abstractions, vague comments.
- Check docs still match behavior.
- Mention any skipped gate or known risk in the final response.

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
- New dependencies where the stdlib or an existing dependency is enough.
- Architecture changes without documentation updates.
- The same block pasted into three places, each copy drifting on its own.

Every indicator above is a judgment except the last one, and that one is an
agent's native failure mode: generating a near-copy is cheaper than finding the
existing helper. So measure it — `just dup-check` (jscpd) if the repo wires it.
Baseline the number first and ratchet it down (`testing/ratchet.md`); a
duplication gate switched on at a round number on day one gets switched off on
day two. A green number is not permission to stop reading the diff — the other
indicators still need eyes.

## Dependency rule

When adding a dependency, state the reason in the change summary and verify: it
is needed by the current milestone; actively maintained enough for the risk; its
license passes the supply-chain gate (e.g. `cargo deny`); it does not duplicate
an existing dependency.

## Documentation rule

Documentation is part of the implementation when behavior changes. Update docs
when changing: milestone scope/exit criteria, module boundaries, runtime
responsibilities, filesystem layout, trust levels, sandbox/approval behavior,
CLI commands, or user-visible configuration.
