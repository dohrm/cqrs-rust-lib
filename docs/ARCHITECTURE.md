# Architecture — cqrs-rust-lib

> Shared decisions: none
> RFC store: GitHub Issues — see § RFC store

Minimal on purpose: the header and the RFC store adapter. The system shape lives in
`CLAUDE.md` § Architecture until `/architect` writes the full overview.

## Decisions

| ADR | Decision | Status |
|---|---|---|
| [0001](adr/0001-reject-malformed-codex-query-params.md) | A malformed codex query parameter is rejected, not dropped | Accepted |
| [0002](adr/0002-declare-the-queryable-field-surface-on-the-view.md) | `_q` and the typed query params are the same set | Accepted |
| [0003](adr/0003-sorting-declares-its-own-field-list.md) | Sorting is offered field by field, and nothing is offered by default | Accepted |

## RFC store

- Tracker: GitHub Issues, `dohrm/cqrs-rust-lib`
- Access: `gh` (`gh issue view|edit|comment`, `gh api` for the timeline) — else REST `https://api.github.com`, token in `$GH_TOKEN`
- Agent accounts: none
- Status: draft = label `rfc:draft`, ready = label `rfc:ready`, implementing = label `rfc:implementing`, done = label `rfc:done` (and the issue closed by the PR)
- Who set ready: the actor of the `labeled` event for `rfc:ready` in the issue timeline (`gh api repos/dohrm/cqrs-rust-lib/issues/<n>/timeline`)
