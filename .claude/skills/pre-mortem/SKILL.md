---
name: pre-mortem
description: "Assume the project is already dead at a chosen horizon; work backward to causes. Writes `docs/premortem/<target>-<horizon>.md` and hands off deltas — never edits PRD/ADR/PLAN. Use on /pre-mortem, \"why will this fail\", \"attack this PRD\". Unlike /architect, it does not weigh a decision."
---

You run a pre-mortem: assume the project has **already failed** at a chosen horizon and reason backward. This is not `/architect` weighing a decision — it is a prospective autopsy. Drive residual risk to **acceptable** by iterating mitigations with the user.

Output: `docs/premortem/<target>-<horizon>.md`. One file per **(target, horizon)**; personas are lenses inside it. Mitigations that touch other documents are **deltas**, handed to `/prd`, `/architect`, or `/rfc`. You own `docs/premortem/` and nothing else.

## Process

### 1. Frame

- Identify the **target**: `docs/PRD.md`, `docs/ARCHITECTURE.md` + `docs/adr/*`, an open RFC's `.work/<slug>/rfc.md` (only while it is open — it's ephemeral), or the repo itself. Read what exists. If there is nothing concrete to attack, say so and point at `/prd` or `/architect` first — a pre-mortem needs a design to kill.
- Read `CLAUDE.md`, adjacent code, and the project's vocabulary. Reuse it verbatim.
- Settle the **horizon** in one question if the user hasn't given one: *"Assume it's dead — how far out? (launch, 6 months, 2 years?)"*. The horizon decides the failure classes: near-term = launch/adoption failure; long-term = maintenance rot, scaling, team turnover, cost drift.
- Propose an **adversarial persona panel** (2–4) deduced from the target — each sees a different death. Typical: the on-call engineer at 6 months, the security/compliance auditor, the new hire at 12 months, the person paying the bill, a hostile user. The user adds, removes, or renames. Each persona is an attack angle, not a stakeholder to please.

### 2. Run the pre-mortem

For each persona, state the failure as **already true** at the horizon (*"It's `<horizon>`. The project is dead. As the `<persona>`, here is what killed it…"*), then work backward. Merge and de-duplicate across personas into a single set, ordered by **lethality** — most lethal first. The quiet, invisible failures rank above the loud ones.

Group the failures by class, most lethal class first:

1. **Conceptual** — the premise was wrong; the machine reliably produced the wrong thing. These make no noise and are the most lethal.
2. **Structural** — the design turns on itself under real load; an invariant held only by discipline, not by the machine.
3. **Prosaic** — the failures that actually kill projects: bus factor, velocity paralysis, cost overrun, adoption friction, never reaching an end-to-end loop.

For every failure, write three things:

- **Scenario** — the concrete path to death, in the present tense of the horizon.
- **Why it's invisible now** — what makes this easy to miss today (an unstated assumption, a deferred hard part, a "someone will handle it").
- **Leading indicator** — the observable signal that this pre-mortem is coming true, so it can be killed early. This is mandatory: a failure with no early signal is unmanageable.

### 3. Score lethality

Rate each failure on two axes, each **L / M / H**:

- **Likelihood** — how probable, given the design as it stands.
- **Impact** — how fatal if it happens.

Lethality is the combination as a single band: **High** (kills or maims the project), **Medium**, **Low**. Bias toward High for plausible-and-fatal; a slick "unlikely" is worth less than an honest "likely". No 9-cell ceremony — name both axes, state the band.

### 4. Iterate to acceptable risk

Walk the register **High lethality first**. For each risk, propose a **concrete, minimal** mitigation, then let the user set a disposition:

- **Mitigated** — a design change removes or defuses it. Emit the precise delta (see step 5).
- **Accepted** — the user knowingly takes the risk. Record the **owner** and the one-line rationale. An accepted risk is a decision, not an omission.
- **Transferred** — pushed to another party/system (SLA, insurance, upstream). Name where it went.
- **Open** — not yet resolved. Stays on the board.

**Verify ≠ repair.** To sharpen a finding you may — and should — **verify** a leading indicator with read-only or idempotent probes: run the quality gate, `grep`, `cargo deny`, list a matrix. Turning a hypothesis into a proven-already-true finding is the whole point. But **applying** a mitigation that mutates code or repo config is not verification — it follows the same discipline as a document change (step 5): propose it, apply only on the user's explicit go-ahead, else leave it as a delta. A risk-analysis pass does not silently rewrite the repo it is analysing.

After each disposition, restate the **residual**: what's left of this risk once the mitigation lands. A mitigation that spawns a new risk gets its own row. Loop.

**Acceptable** = no **High-lethality** risk remains **Open**, and every remaining risk is explicitly Accepted, Transferred, or Mitigated with the user's sign-off. State when you reach it in one sentence. Never declare acceptable on your own authority — the user signs off.

### 5. Hand off mitigations as deltas (never edit the target yourself)

You own `docs/premortem/`. When a mitigation changes the design, emit a **precise delta** against the owning artifact and offer to apply it — do not touch it yourself. This holds for documents **and** for code/config:

- PRD change → *"Delta for `docs/PRD.md`: add to Out of Scope — `<text>`. Run `/prd` to apply?"*
- Architecture change → *"Delta: supersede ADR-NNNN with a new ADR — `<decision>`. Run `/architect` to apply?"*
- RFC change → *"Delta: insert a hardening step before step 2, or a hardening phase first — `<slice>`. Run `/rfc` to apply?"*
- Code/config change → *"Delta: wire `ts-check` into the `check` target; create `deny.toml`. Want me to apply these now, or leave them as tickets?"* — a bundle of code changes lands only on an explicit go-ahead, never as a silent side effect of the analysis.

Track every pending delta in the register so nothing is silently lost.

### 6. Write the register

Create `docs/premortem/` if absent. Write `docs/premortem/<target>-<horizon>.md`, where `<target>` ∈ {`prd`, `architecture`, `plan`, `repo`} and `<horizon>` is a short slug (`launch`, `6mo`, `2yr`). On re-run of the same (target, horizon), read the existing file: keep validated dispositions, re-score against the moved design, add new failures, and append a dated round to the log rather than overwriting history. A new target or horizon is a new file. Confirm *"✓ written to `docs/premortem/<target>-<horizon>.md`"* and list the pending deltas with the command to apply each.

<premortem-template>
# Premortem — <target> @ <horizon>

> **Status:** risk analysis · **Target:** <PRD / architecture / plan / repo> · **Horizon:** <the death date>
> **Method:** assume the project is dead at the horizon; work backward to the causes.
> **Personas:** <the validated panel>

## Starting observation

One paragraph: the central imbalance or assumption that orients every failure below. What is this project's real bet, and where is it most exposed?

## 1. Conceptual failures — quiet, most lethal

### 1.1 <short title> — <lethality band> (likelihood <L/M/H> × impact <L/M/H>)

- **Scenario:** …
- **Why it's invisible now:** …
- **Leading indicator:** …

## 2. Structural failures — the design turns on itself

### 2.1 <short title> — <lethality band> …

## 3. Prosaic failures — what actually kills projects

### 3.1 <short title> — <lethality band> …

## Residual-risk register

| ID | Risk (1 line) | Persona | Lethality | Disposition | Mitigation / owner | Pending delta |
|----|---------------|---------|-----------|-------------|--------------------|---------------|
| R-1 | … | … | High | Mitigated | … | `/architect`: supersede ADR-NNNN |
| R-2 | … | … | Medium | Accepted (owner: …) | rationale … | — |

## Rounds

- **<YYYY-MM-DD>** — <N> failures found, <n> High. Acceptable: <yes/no>. Open High: <list or none>.
</premortem-template>

## Rules

- Findings start from "it's dead". Pros-and-cons is `/architect`.
- You own `docs/premortem/` only. Emit deltas; never edit the target.
- The user signs off on "acceptable". No invented severity band.
- Plan mode: writing `docs/premortem/` is allowed.
