---
name: migrate
description: "Upgrade an existing claude-rules install to the current harness: gap table, then lock --root, imported just library, lefthook/CI callers. Use on /migrate, \"migrate the harness\", \"justfile imports\". Not /update (routine upgrade of a current install), not /onboard (first wire), not /ci-setup (pipeline only)."
---

Upgrade **this repo's claude-rules install**. There is already a lock. You
measure the gap against the current factory, get a yes on the lock map and
any severity jump, then compose the install. You do not change product code.

`/onboard` is first wire on a brownfield with no lock. `/ci-setup` is the
pipeline after the justfile is true. If there is no `.claude-rules.lock`,
stop and send them to `/onboard`.

If this skill is missing on disk: `npx github:dohrm/claude-rules#<ref> add product`
lands it. Do **not** `update` until step 3 has `--root` queued — a bare update
replays unscoped globs.

## Never

- Bare `update` before `add … --root`.
- Replace a custom just recipe with the stock one when the **commands** differ.
  Override **above** the imports, comment why.
- Raise gate severity (advisories, machete, fmt on generated crates, eslint
  `--max-warnings`, deptry layout) without a human yes. Keep today's severity
  as an override.
- A second definition of gates in CI. Workflows call `just` recipes. Rename a
  recipe → grep every caller (`justfile` `check`, `lefthook.yml`,
  `.github/` / `.gitea/workflows/`).
- Delete repo-owned skills, nested `CLAUDE.md`, a crate-root `deny.toml`, or
  scripts that are not kit clones.
- Commit unless asked. Do not push.
- Weaken tests or skip hooks to green `just check`.

Installer doctrine: it never merges justfile / lefthook / CI. `update`
refreshes `.dev/kit/`. You compose. Justfile procedure and the equivalence
proof: `.dev/kit/common/README.md` after the kit lands, else
`kit/common/README.md` in the factory. Do not restate it — follow it.

`ts-web` recipes are `ts-web-lint` / `ts-web-check`. Prefer a root-justfile
**alias** (`ts-check: ts-web-check`) over a silent rename that 404s CI.

Pin: `npx github:dohrm/claude-rules#<ref>`. Ask once for `<ref>` if the lock
says `main` and they did not name a tag. Same `--agent` set as the lock.

## Process

### 1. Snapshot — no installer command yet

- git status; `.claude-rules.lock`; root `justfile`; `lefthook.yml`;
  `CLAUDE.md` / `AGENTS.md`; workflows under `.github/` or `.gitea/`.
- `just --summary > /tmp/before.recipes`
- `just --evaluate | sed 's/ *:= /=/' > /tmp/before.vars`
- Leftover trees: `.claude/kit/`, `.dev/rules/`, `.opencode/`,
  `.agents/rules/`, kit clones under `scripts/` (`adr-check.mjs`,
  `docs-check.mjs`, `review-guard.mjs`, the review prompt)
- Custom `check` deps (openapi, spa, helm, boundary, e2e, mobile-bundle, …)
- Languages from the **tree** (`Cargo.toml`, workspaces, `go.mod`,
  `pyproject.toml`), not from the old lock alone
- Whether `justfile` already `import`s `.dev/kit/**/*.just`

### 2. Gap table — write `.work/migrate.md`, then stop

Create `.work/` if needed (gitignored working memory). Shape:

```
# Migrate — <repo>

## As-installed
## Proposed lock (modules + levels + aliases)
## Overrides to keep (just / scripts / deny / hooks)
## Recipe-name map (old → new, or alias)
## Severity jumps (need yes / keep override)
## Leftover trees to purge
## Commands to run
## Hand-off
```

Map languages to `--root` from the tree. Typical:

- HTTP API → `rust-api` / `go-api` / `python-api` / `ts-node-api` `--root <dir> --level gates`
- React HTTP portal → `ts-web-app --root <portal> --level gates` (`portal-http` is in the alias; old locks have `portal-flat` only)
- `react` on **every** React tree (web **and** React Native) — not bundled into `portal-flat`
- Never `portal-http` and `tauri` together
- `agent testing cicd --level gates` (agent is not a gift; a legacy `update` injects `agent@gates` if `levels` is missing — still name it)
- `product defect loop-setup` if they already had them (the `goal-setup` profile is gone: `/rfc`, in `product`, replaces it and the former plan and tasks skills; `investigate` is now `defect`, and the old name still resolves)
- `ops` `k8s` `incident` only if this repo owns run / manifests

A kit `*_dir` is one directory. Two Rust workspaces → keep the second as a
**just override**, do not invent a second `rust_dir`.

Print the `npx … add` lines. **Stop.** Do not run them until the human
accepts the modules table and each severity jump (yes = accept stock, no =
override). If they already said “execute” and named the ref, still stop on
a severity jump that would turn CI red overnight.

### 3. Lock, then update, then init

```
npx github:dohrm/claude-rules#<ref> add <alias-or-profiles> --root <dir> --level gates
# repeat per module; add is additive
npx github:dohrm/claude-rules#<ref> add agent testing cicd --level gates
npx github:dohrm/claude-rules#<ref> update --ref <ref>
npx github:dohrm/claude-rules#<ref> init
```

`init` does not rewrite an existing justfile body. It reports missing
imports. It only rewrites a `# claude-rules:start` … `end` `*_dir` block
once you wrap one.

### 4. Justfile — import the library

Needs just >= 1.27. Follow `.dev/kit/common/README.md` (header + imports,
managed `*_dir` block, three cases for inline recipes, one-directional
`comm -23` proof). Keep every recipe the kit never owned. Point `check` at
the same extras as the snapshot.

After the proof: recipes call `.dev/kit/common/*.mjs`. Delete `scripts/`
**kit clones** only then. Keep local scripts. Move a patched docs ceiling
into `.docs-budgets.json` at the repo root, never a forked `docs-check.mjs`.

### 5. lefthook + CI

Merge kit snippets as thin triggers → `just <tech>-lint` / `<tech>-check`.
Keep local extra hooks (helm, boundaries, trunk guard, review-guard). Do
not add a language hook the repo omitted unless the human wants that tax.

CI (GitHub or Gitea): same just names as local. Split jobs per toolchain
are fine. Aggregator (`ci-ok`) stays the required check. Tier 4 stays out
of `check` and out of hooks. After recipe aliases, grep workflows.

If the justfile is true and CI still invents commands, `/ci-setup` audit
mode — do not bootstrap a second pipeline.

### 6. Agent OS + leftover trees

- Root `CLAUDE.md` missing and Claude is a target → scaffold a **project
  map** only (`init` writes one iff the file is absent). Do not dump rules
  into it.
- Preserve `AGENTS.md` and `.agents/rules/`, including manual bridges. To select
  Codex explicitly, repeat an installed profile with `add <profile> --agent codex`.
  The installer adopts identical assets and a single valid historical block,
  preserving outside text. Reconcile conflicting assets or malformed markers;
  do not delete local guidance to make the installation pass.
- Preserve locked targets unless the user requests another. Normalize aliases to
  elementary profiles; `modules["."]` is the complement of non-root assignments.
- Retired cleanup covers `.dev/rules/` and `.opencode/`; legacy kit cleanup covers
  known `.claude/kit/` assets, preserving unrelated neighbors.
- Preserve nested `CLAUDE.md`, local rules/skills/agents not in the
  registry, `.claude/settings.json`.

### 7. Done

- `npx github:dohrm/claude-rules#<ref> doctor`
- `npx github:dohrm/claude-rules#<ref> budget <one file per module>` —
  portal rules must **not** load on a mobile / non-portal TS file
- `just check` — if red because stock gates are stricter, restore the
  override; do not mass-fix the codebase in this pass

Hand back (and append to `.work/migrate.md`): modules table as locked,
overrides kept, recipe aliases, doctor leftovers, CI grep hits, severity
jumps still waiting on a yes. Then stop.
