---
name: adr-review
description: "Review an existing ADR corpus for active, superseded, redundant, contradictory, or stale decisions and propose a compact consolidation. Use on /adr-review or requests to clean up a large decision log. For one challenged architecture choice, use /solution-exploration."
---

# Review the decision corpus

Inventory `docs/adr/` and its index (usually the decision log in
`docs/ARCHITECTURE.md`). Read the project's decision rules if installed; the
review and human status boundary here still apply with a product-only install.
This is a corpus-wide review: inspect every record,
its status, decision, references, and claimed successor. Use code, current product
documents, and history selectively to verify whether an apparent conflict or
obsolete premise is real. Do not infer that a decision is obsolete merely because
its date is old or implementation differs; distinguish evidence from suspicion.

Map which decisions are in force, proposed, rejected, superseded, or deprecated
from their explicit status lines (`Rejected`, `Rejeté`, or `Refusé` where used).
Distinguish status from a title about rejected techniques, and link
relevant refusals recorded as alternatives inside accepted ADRs to their current
decision.
Identify broken or stale links and index entries, duplicated rationale, overlapping
scope, incompatible active decisions, and decisions whose premises may no longer
hold. Trace dependencies before recommending a replacement. One `Proposed` ADR
may consolidate several older records when it expresses **one coherent decision**
and makes each affected record's fate explicit. Use separate proposals when the
choices can be accepted independently. Preserve the old records as history; do
not rewrite them to make the current state look inevitable.

Read the shared decision repository named in `docs/ARCHITECTURE.md` (ask for it
if absent; `none` is an answer). Flag project records that are enterprise policy,
stack convention or feature-local detail (`agent/decisions.md` § Which choices need
an ADR), and records that duplicate or contradict a shared one. For each enterprise
candidate, propose a PR on the shared repository (§ Shared decisions) and, once
merged there, a pointer in place of the project record.

Return a compact source-linked map of the current decision set and a prioritized
action table: finding, evidence, affected ADRs, proposed action, and whether a
human decision is needed. Recommend a slimmer index or clearer cross-links where
that reduces the context agents must load. Summarize active constraints for
readers without copying whole ADRs. Mark uncertain findings for verification.

The review itself changes no ADR status. Correcting a factual link or index may
be done as an ordinary documentation edit when requested, while preserving the
record's meaning. If the human chooses a replacement, write only `Proposed` ADRs
following the installed `agent/decision-records.md` rule when available, or the
repository's ADR format otherwise; state what remains binding
until acceptance. `Accepted`, `Rejected`, `Superseded`, and `Deprecated` status changes are
made and committed by a human. Use `/solution-exploration` for a focused challenge
to one design choice, and `/architect` for initial architecture authoring.
