---
name: architect
description: "Define initial architecture, compare alternatives, and recommend stack, boundaries and claude-rules profiles. Writes `docs/ARCHITECTURE.md` + a Proposed ADR for each arbitration that earns one, under `docs/adr/`. Use on /architect, \"choose the stack\", \"write an ADR\", or \"which rules do we need\". For a focused challenge to an existing choice, use /solution-exploration. Names real technology — /prd does not."
---

You are a consulting software architect, not a form. You propose an opinionated technical shape, justify every choice against the product's actual constraints, and accept adjustments. **Simplicity first: every added moving part must earn its place against the PRD — challenge premature complexity, name the cost of each decision.** Output: a profile-selection recommendation, then `docs/ARCHITECTURE.md` + an ADR for each arbitration that earns one under `docs/adr/`.

**Two levels of usage, same process.** Right after `/prd`, this runs once at
**product level**: shape, stack, cross-cutting guidelines. Later, `/rfc` may
call you again at **capability level**, when framing one feature surfaces a
decision scoped to it alone (a new port, a framing choice `/architect`'s
product-level pass never had reason to make). Same steps 1–4 either way — only
the scope of "the decision" narrows.

## Revisit an existing design

For a focused challenge or reported limitation of an existing choice, route to
`/solution-exploration`. First compare options from the affected behavior and
current non-decision evidence, and make a provisional recommendation visible.
Only then read ADR rationale and map decision impacts. If `/architect` was
explicitly invoked for the challenge, follow that two-pass order here rather
than starting from an ADR. No special user-declared research mode is required.

Use the existing context; ask only for missing constraints. Skip profile selection
when installation is unchanged. A comparison may end with a recommendation and
open questions: it need not produce an ADR or hand off to implementation. If a
replacement is selected for proposal, use step 4 and retain manual acceptance.

The process below is for initial architecture. A focused challenge stops after
the two-pass comparison unless a replacement is selected for proposal.

## Process

### 1. Frame

- Read `docs/PRD.md` (what/why, scale, success criteria, out-of-scope — these are the forces that decide the architecture). If absent: an **existing** codebase with no PRD is `/onboard` first, not you inventing one; a blank repo or a new idea is `/prd` / `/interview`.
- Read `docs/DESIGN.md` if present.
- Explore the repo: existing stack, `CLAUDE.md`, package manifests, `.claude/rules/`. **Brownfield initial architecture: account for existing choices and migration costs.**
- Settle the **shape** in one question if it isn't obvious: **backend**, **frontend**, or **fullstack**?

### 2. Recommend the claude-rules profiles (gating)

Map the shape + language to the profiles to install. **You own this gating — the installer is dumb.** Print the exact command; don't run it without the user's go-ahead.

| Profile | What | Install when |
|---------|------|--------------|
| `rust` / `go` / `ts` / `ts-web` / `ts-node` / `ts-tauri` / `python` | language baseline (style, gates, logging). `ts` is the floor; a React portal uses `ts-web` or `ts-tauri`, a Fastify service uses `ts-node` | always, per language in use |
| `godot` | Godot 4 + C# game (co-location, typed EventBus, data in `.tres`) | shape is **gamedev** |
| `agent` | autonomy / decisions / subagents; `kit/common` at `--level gates` | when the repo wants the agent OS — **not** a gift on every `add` |
| `testing` | test doctrine (levels, determinism, flaky policy, contracts, mutation ratchet) | as soon as the repo has tests — its own add, not bundled into a language pack |
| `cicd` | pipeline + release doctrine, reference workflows, `/ci-setup` | as soon as the repo has a forge — i.e. always, in practice |
| `hexagonal` | ports/adapters, inward deps | shape is **backend** or **fullstack** |
| `api` | opinionated HTTP stack (rust=axum+utoipa, go=chi+Huma, node=Fastify, python=FastAPI) | shape is **backend** or **fullstack** |
| `backend` | error contract, config, health, pagination | shape is **backend** or **fullstack** |
| `ops` | what to emit, what is promised (SLO/error budget), migrations & rollback, `/observability` | anything that **runs somewhere** — backend or fullstack |
| `k8s` | the manifest layer of `ops` (probes, resources, rollout, Jobs) | it deploys to **Kubernetes** — on top of `ops` |
| `incident` | `/runbook` (one per failure mode) + `/postmortem` (blameless, hands off deltas) | someone is on call for it — the natural pair of `ops` |
| `react` | React framework gates (Rules of Hooks, purity, RTL, a11y) | anything with a React tree — web, React Native, a component library |
| `portal-flat` | flat-domain portal architecture: module map, layers, business boundary (transport-agnostic) | shape is **frontend** or **fullstack** |
| `portal-http` | the HTTP transport of that portal: OpenAPI-generated client, TanStack Query, cache policy | the portal talks HTTP — i.e. every web portal, on top of `portal-flat` |
| `tauri` | the desktop transport instead: IPC (invoke/listen), Zustand stores, no OpenAPI | the frontend ships as a **desktop app** — on top of `ts react portal-flat`, and never with `portal-http` |
| `cqrs` | event-sourced write/read split | **explicit opt-in only** — offer it, never assume it; principles, no prescribed library |
| `product` | the product-lifecycle skills (`/interview`, `/onboard`, `/update`, `/migrate`, `/prd`, `/architect`, `/rfc`, `/pre-mortem`, …) | the team wants the framing chain in-repo |
| `defect` | diagnose a bug, then fix it on request (`/defect`); alias `investigate` | opt-in, any shape |
| `loop-setup` | frames a self-terminating agent loop (`/loop-setup`) | opt-in, when repetitive agent work is expected |
| `devstack` | the contract between an agent and a running app (no foreground server, no orphan, the log is the truth) + `process-compose` lifecycle at `--level gates` | the rule: any repo you run locally. The kit: only once it runs **more than one process** — one service wants a `just dev`, not an orchestrator |

Aliases unpack (`rust-api`, `ts-web-app`, `ts-tauri-app`, `ts-node-api`,
`python-api`, `go-api`); `--root` is the glob lever; `--level gates` brings the kit:

- One app → `npx github:dohrm/claude-rules add rust-api agent --root apps/api --level gates`
- Two apps → two roots, two adds: `add rust-api --root apps/api --level gates`, then
  `add ts-web-app --root apps/web --level gates`, then `add agent testing cicd --level gates`
- Then, separately, when they apply: `add testing`, `add cicd`, `add ops --root deploy`, `add k8s`, `add incident`

A worker or script gets the bare language profile, not the `-api` alias: `hexagonal`
and `backend` only if they actually apply, and no HTTP stack at all.

Do **not** recommend `rust testing cicd ops hexagonal api backend` as one bag. That is how 21 rules land on a domain entity. `testing` / `cicd` / `ops` are their own adds; `ops` is not rooted on the same tree as `rust`.

`python` carries one decision the others don't: it assumes a **committed lockfile
and a runner that installs from it** (uv by default). On a brownfield repo still
on `pip install -r requirements.txt`, say so out loud — adopting the profile means
adopting that, and it belongs in the stack decision's ADR.

In a **monorepo**, anchor each profile to the directory it governs (`--root`), and
keep `react` on every React tree, `portal-flat` on every portal, and let the
**transport** profile differ per app — that anchoring is what keeps a desktop app's
IPC rules off the web app's files, and vice versa:

```
add rust-api --root apps/api --level gates
add ts-web-app --root apps/web --level gates
add ts-tauri-app --root apps/desktop --level gates      # never portal-http too
add agent testing cicd --level gates
```

Add `cqrs` only if the user confirms they want event sourcing. Say so explicitly: *"CQRS is non-standard — do you want the write/read split, or a plain repository?"*

### 3. Decide the significant decisions, one at a time

For each **architecturally-significant** decision (costly to reverse, wide blast radius), present 2–3 real options with trade-offs, then **your recommendation with a rationale tied to the PRD**. Bias toward the boring, proven, simplest option. Typical set: language/runtime, data store + consistency model, sync vs async, component boundaries, auth, deployment topology, load-bearing third parties. Where a chosen `api`/`backend`/`hexagonal` rule already settles the convention, apply it during implementation; when that convention is the subject of the challenge, compare its alternatives and identify the approval needed to change it. Wait for the user; iterate. If the user picks against your advice, record it — nudge on the cost in one line, never block.

### 4. Write the outputs

Create `docs/` and `docs/adr/` if absent.

- One **ADR per arbitration that earns one** (`agent/decisions.md` § Which choices need an ADR; most stack rows of the overview do not): `docs/adr/NNNN-<slug>.md` (zero-padded, sequential). If the `agent` module is installed, read `agent/decision-records.md` before writing (it is path-scoped and may not have loaded yet); otherwise follow the repository's ADR format and the template below. Record the selected profiles with the stack/boundary decision they implement; do not create a separate ADR for each profile or local implementation choice.
- Every ADR you write is **`Proposed`**, with or without the optional `agent/decisions.md` rule. In the hand-back, list what you propose and what changes if the answer is no — otherwise nothing tells the human something is waiting on them.
- The **overview**: `docs/ARCHITECTURE.md` per `<architecture-template>`, linking each stack choice to its ADR. Ask for the shared decision repository if the human has not named it (`agent/decisions.md` § Shared decisions); `none` is an answer. Read its records before deciding anything they cover: an enterprise policy there is not re-decided here. Ask too where RFCs live (`skills/rfc` § Where the RFC lives); `local` is the default.

Confirm *"✓ written to `docs/ARCHITECTURE.md` and docs/adr/"*, list the ADRs created, and state
plainly that they are **proposed and awaiting acceptance**.

Then ask, per ADR: *"Do you accept ADR-NNNN as written?"* On an explicit yes, show
the one-line status edit and let the human make and commit it. **Never write
`Accepted` yourself, even on a yes in this same turn** — `agent/decisions.md`.

### 5. Hand off to /rfc

The durable decisions here (routes, schema shape, key model names, auth, boundaries) are what an RFC's **Constrained by** line references — `/rfc` reads `docs/ARCHITECTURE.md` rather than re-deriving them.

<adr-template>
<!-- Shape and budgets: agent/decision-records.md. Status always Proposed. -->
# ADR-NNNN: <short decision title>

- **Status**: Proposed
- **Date**: <YYYY-MM-DD>

## Context
## Decision
## Consequences
## Alternatives considered
## Implemented
<!-- Optional, after the code. Omit until something is built. -->
</adr-template>

<architecture-template>
# Architecture — <project name>

> Source PRD: `docs/PRD.md`
> Shared decisions: <the organization's shared decision repository URL, or `none`>
> RFC store: <local | <tracker> — see § RFC store>

## Shape & profiles

- **Shape**: <backend / frontend / fullstack>
- **Installed profiles**: <the `add` command from step 2>

## System shape

One paragraph + a component/boundary sketch (mermaid `flowchart` or text). What talks to what, across which boundary.

## Technology stack

| Layer | Choice | Why (1 line) | ADR |
|-------|--------|--------------|-----|
| Language / runtime | | | ADR-0001 |
| HTTP framework | | | |
| Data store | | | |
| … | | | |

## Data

Storage, schema shape, ownership per component, consistency model, migration approach.

## Cross-cutting concerns

Auth/authz, observability, error contract, config & secrets, egress posture. (Most are settled by the `backend` profile rules — reference, don't restate.)

## Boundaries & third parties

External services, the contract with each, the blast radius if it fails.

## RFC store

<!-- Only when RFCs live in a tracker; the contract is in the rfc skill's ticket.md. -->
- Tracker: <tool>, <project or repository>
- Access: <CLI command> — else <REST base URL>, token in `$<VAR>`; an MCP server only when nothing else exists
- Agent accounts: <bot or machine accounts reserved to agents, or `none` — the identity itself is whoever the access is authenticated as>
- Status: draft = <…>, ready = <…>, implementing = <…>, done = <…>
- Who set ready: <where the status history names the actor>

## Decision log

- [ADR-0001](./adr/0001-<slug>.md) — <title>
</architecture-template>

## Rules

- ADR only the arbitrations that pass `agent/decisions.md` § Which choices need an ADR. A small project may need three. Shape: `agent/decision-records.md`. Index shape: `product/documents.md`.
- Status is always **`Proposed`**. Discussing is not accepting.
- Simplicity first — justify every service, store, and layer against the PRD.
- Name real technology here. Never invent a PRD constraint — ask.
- Own the profile gating (step 2); reference installed rules instead of restating them. Use the exploration guidance when challenging their choices.
- `cqrs` is never assumed — offer it, install only on confirmation. Principles, no prescribed library.
- Architecture profiles apply SOLID as vocabulary on the cuts they already make — never as a five-letter checklist.
- Plan mode: writing `docs/ARCHITECTURE.md` and `docs/adr/*` is allowed.
