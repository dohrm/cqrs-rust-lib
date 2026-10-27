---
name: update
description: "Bring a current claude-rules install up to date: run `update`, read what the CHANGELOG added since the last install, apply its hand-merge items, re-green `just check`. Use on /update, \"update claude-rules\", \"pull the latest rules\", \"what changed in the kit\". Not /migrate (restructure an old harness), not /onboard (first wire)."
---

Bring **this repo's claude-rules install** up to date. The installer refreshes
what it owns (rules, skills, agents, `.dev/kit/`); the configs the repo copied
once (`.golangci.yml`, `eslint.config.js`, `tsconfig.json`, `pyproject.toml`,
`rustfmt.toml`, `deny.toml`, `lefthook.yml`, CI) never move on their own. The
CHANGELOG says which entry needs which. You do not change product code, except
to fix findings of a gate the human accepted.

Route first:

- No `.claude-rules.lock` → `/onboard`.
- A lock without `modules`, or a root `justfile` that does not `import`
  `.dev/kit/**/*.just`, or leftover `.claude/kit/` → `/migrate`. A bare
  `update` on that install replays unscoped globs.
- A dirty work tree → stop: the diff after `update` must be the installer's
  alone.

## Never

- Overwrite a repo-owned config with the kit's copy. Add the new keys; keep
  the repo's own.
- Accept a stricter gate without a human yes — the same rule as `/migrate`.
- Mass-fix the codebase to green a gate nobody accepted, or weaken a test,
  loosen a config, or skip a hook to green one that was.
- Commit unless asked. Push.

## Process

### 1. Update

Same `--agent` set and ref as the lock (`npx github:dohrm/claude-rules#<ref>`;
ask once for `<ref>` if the lock says `main` and they did not name one). The
`#<ref>` picks the CLI; `--ref` picks the assets and is what the lock records —
without it, `update` falls back to `main`:

```
npx github:dohrm/claude-rules#<ref> update --ref <ref>
```

The lock may list several sources: `--ref <ref>` moves `core` only, and every other
source replays its locked ref unless the human names one (`--ref <name>=<ref>`).

Two refusals stop `update` before it changes anything. Both are the human's to
settle — never pass `--force` on your own:

- **Adoption** — the lock records no file hashes (an install from before hashes).
  `update` lists every installed destination it would overwrite once, local edits
  included, and needs `--force` without a terminal. Show the list; the human saves
  any edit worth keeping (into `.dev/overrides/`, same layout as the source) and
  then says whether to re-run with `--force`.
- **Corruption** — a listed path is edited, unrecorded, or foreign inside a skill.
  Report each one with `git log -- <path>`. The fix is the human's: move the edit to
  `.dev/overrides/`, delete the stray file, or re-run with `--force`.

An `⚠ <path> changed in <source> (overridden by <layer>)` line means an upstream
fix no longer reaches this repo. Add a row for it in step 3 — the human reads the
upstream diff and decides whether the override still holds.

### 2. Read what changed

- `git diff -- .dev/kit/CHANGELOG.md` — the added lines are every entry since
  the last install. That copy is the baseline; no tag is needed.
- No previous copy (the install predates it): the whole `[Unreleased]` section
  is a candidate, and `git diff --stat -- .dev/kit` says which entries actually
  reached this repo — keep only the entries whose kit files changed.
- `git diff --stat -- .claude .agents .cursor .dev/kit` — what the installer
  rewrote, for the hand-back.

### 3. Triage — write `.work/update.md`, then stop

One row per new entry that concerns an installed profile:

```
| Entry | Delivered by update | Hand merge (file → into) | Stricter gate? | Findings now |
```

"Findings now" is measured, not guessed: run the gate with the new setting on a
scratch copy of the config, or the recipe itself when `update` delivered it, and
count. **Stop.** Each stricter gate needs a yes (accept, fix the findings) or a
no (keep today's behavior as an override, with the reason on the line).

### 4. Apply

- Hand merges: from `.dev/kit/<tech>/` into the repo-owned file the kit README
  maps it to (`.dev/kit/<tech>/README.md`). One commit-sized change per entry; an
  entry the CHANGELOG says to merge alone stays alone.
- A refused stricter gate delivered by `update`: override the recipe in the root
  justfile (a root recipe wins over the import), with a comment saying why.
- An accepted one: fix its findings. Past a handful, ask before going on.

### 5. Done

- `npx github:dohrm/claude-rules#<ref> doctor`
- `just check` green.

Hand back (and append to `.work/update.md`): entries applied, overrides kept
with their reason, findings fixed, anything still waiting on a yes. Then stop.
