# The kit — a just library, not a template

The gates live here, under `.dev/kit/`, and the repo's root `justfile` **imports**
them:

```just
set allow-duplicate-recipes := true
set allow-duplicate-variables := true

import '.dev/kit/common/gate.just'
import '.dev/kit/rust/rust.just'
```

`claude-rules init` writes that file for you. Everything else in it is the
**composition** — where each technology lives (`rust_dir`…), what `check` runs, and
whatever this repo overrides. The recipes underneath are refreshed by
`claude-rules update`.

That split is the point. A snippet merged by hand could never be updated again: a
fix upstream stayed upstream, and every installed repo drifted from the day it was
installed. An import can be updated. So:

- **The library is upstream's.** Never edit a `.just` under `.dev/kit/` — the next
  `update` overwrites it.
- **The root justfile is yours.** A recipe or variable defined there wins over the
  imported one, which is what the two `set allow-duplicate-*` lines buy. That is how
  you adapt a gate without forking it.

Needs **just >= 1.27** (`import` landed in 1.18; the duplicate-override settings in
1.27).

## Requirements

| Tool | macOS | Linux | Windows |
|---|---|---|---|
| `just` >= 1.27 | `brew install just` | official script: `curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh \| bash -s -- --to ~/.local/bin` (a distro package may be older than 1.27 — check) | `winget install --id Casey.Just.Just` (or `scoop install just`) |
| `lefthook` | `brew install lefthook` | `brew install lefthook` (Linuxbrew), or `npm install -g lefthook` | `scoop install lefthook`, or `npm install -g lefthook` |
| Node.js >= 18 | `brew install node` | your distro's package manager, or [nvm](https://github.com/nvm-sh/nvm) | `winget install OpenJS.NodeJS.LTS` (or `scoop install nodejs`) |

Node is not optional even in a pure Rust/Go/Python repo: `adr-check.mjs`,
`docs-check.mjs`, `review-guard.mjs`, `worktree-status.mjs`, `diff-since.mjs` and
`publish-summary.mjs` all run on it (that's the reason they're Node rather
than bash — see the parent `kit/README.md`), and `dup-check` / `rules-check`
shell out to `npx`.

## What is where

| File | Holds |
|---|---|
| `common/gate.just` | `code-review`, `review-with`, `review-guard`, `mutate-from`, `mutate-mark`, `tree`, `tree-rm`, `status`, `publish-summary`, `dup-check`, `adr-check`, `docs-check`, `rules-check`, `base`, `worktree_root` |
| `rust/rust.just` · `ts/ts.just` · `go/go.just` · `python/python.just` | `<tech>-lint`, `<tech>-check`, and that tech's Tier-3 recipe |
| `godot/godot.just` | `godot-lint`, `godot-check` (+ the three variables you must override) |
| your `justfile` | the imports, `*_dir`, `check`, `mutate-diff`, `base` if the trunk is not `origin/main` |

The scripts the recipes call (`adr-check.mjs`, `docs-check.mjs`, `review-guard.mjs`,
`worktree-status.mjs`, `publish-summary.mjs`, `diff-since.mjs`, `review-prompt.md`) ship **in this directory** and are called
from here. There is nothing to move into `scripts/`: gate and implementation are
updated together, which is the whole reason they are not copied out.

To point `just code-review` at a repo-specific prompt, override one variable in the
root justfile:

```just
review_prompt := "docs/review-prompt.md"
```

## Tier 3 measures the block, not the branch

`code-review` used to read `git diff <base>...HEAD` on every run, so a branch grown
over several loops re-reviewed everything the previous runs had already cleared — the
tenth block paying for the nine before it, until `review_max_bytes` failed one for the
size of its own history. Each gate now records the commit it last **passed** on and
diffs from there:

| | |
|---|---|
| `.work/<slug>/.latest_review` | what `code-review` cleared |
| `.work/<slug>/.latest_mutate` | what `mutate-diff` cleared, once you wire it |

`<slug>` is the branch name unless you set `work_slug` (a `/loop-setup` capability slug
parks the marker next to that loop's `loop.md`). Both files are per developer —
`diff-since.mjs` keeps `.work/.gitignore` carrying the patterns, so they stay private
even in a repo that commits its `.work/` plans.

Three properties, none of them optional:

- **It advances only after the gate passed.** A `CRITICAL` leaves the marker where it
  was, so the fixes come back for review *with* the block they fix.
- **It fails back, never closed.** Absent, unparseable, rewritten, rebased, a branch
  switched under it — anything but a commit that is still an ancestor of `HEAD` means
  the full `<base>...HEAD`. A stale marker costs one whole pass; a marker trusted
  blindly hides a rewritten commit from every review that follows.
- **It is local.** CI always measures the whole PR. The marker is a loop's economy,
  never the branch's proof — and `just incremental=0 code-review` is the whole-branch
  read, worth one run before you open one.

Mutation is wired in **your** justfile, because `mutate-diff` is where you name the
mutators this repo has:

```just
mutate-diff:
    just rust-mutate "$(just mutate-from)"
    just mutate-mark
```

`mutate-mark` runs last and only on success (just stops at the first failing line), so
a surviving mutant leaves the marker put. Stryker (`--incremental`) and mutmut (its
cache) already scope themselves and take no range — leave them on their own line.

`review_in` / `review_out` are **not** configuration: they are how `code-review` (the
gate) and `review-with` (ad hoc) drive the same `review-<agent>` recipes over separate
scratch files, so an interrupted ad-hoc review can never leave a temp file the gate
would promote to a verdict. Leave them alone.

## Migrating a justfile that predates this

A justfile from before the library holds the recipes inline. Only a human (or an
agent reading the diff) can tell which of them drifted **on purpose**, so this is not
something the installer rewrites — it reports, and you do the four steps below.

The reason it is safe to do at all: `just --summary` and `just --evaluate` **flatten
imports**, so a before/after comparison of both is a deterministic check that nothing
was lost — mechanical, not a judgement call. (`just --dump` does *not* flatten: it
keeps the `import` line, so it proves nothing here.)

1. **Record the baseline**, from the repo root:

   ```sh
   just --summary                      > /tmp/before.recipes
   just --evaluate | sed 's/ *:= /=/'  > /tmp/before.vars   # padding varies; normalise it
   ```

2. **Add the header** `claude-rules init` printed: the two `set allow-duplicate-*`
   lines, then one `import` line per `.just` under `.dev/kit/`.

3. **Delete every inline recipe the library now provides**, and every variable it
   defines with the same value. Three cases, and the third is the one that matters:

   - *Identical to the library* → delete it. This is most of the file.
   - *Only its `*_dir`/`base` value differs* → delete the recipe, keep the variable
     assignment. It now overrides the library's default.
   - *The commands themselves differ* → **keep it, above the imports.** It wins over
     the library version. Leave a comment saying why it diverged, or the next reader
     deletes it as leftover.

   Recipes that were never the kit's stay untouched, wherever they are in the file.

4. **Prove it.** Not a raw `diff` — the comparison is one-directional, because things
   *appearing* is normal (the library ships more than the old snippet did, e.g.
   `status`, and adds its own variables `kit` and `review_prompt`). What must not
   happen is something being **lost or changed**:

   ```sh
   # recipes that no longer resolve
   comm -23 <(tr ' ' '\n' < /tmp/before.recipes | sort -u) \
            <(just --summary | tr ' ' '\n' | sort -u)

   # variables that vanished, or whose value moved
   comm -23 <(sort /tmp/before.vars) \
            <(just --evaluate | sed 's/ *:= /=/' | sort)
   ```

   Read both lists against the three cases below. Anything not covered by them is a
   real loss — put it back.

   - **`ts-*`, `go-*`, `python-*` (and their `*_dir`) disappeared.** Expected, and the
     most common result: the old snippet shipped every technology inline, while the
     library only imports the profiles this repo actually locked. Two right answers —
     `claude-rules add <profile>` if the repo really has that technology (the recipe
     comes back, from the library), or accept the drop, because a `ts-check` that no
     repo here can run was never a gate. Check `check`'s dependency list either way.
   - **A recipe you deliberately changed disappeared.** You deleted it in step 3 when
     it belonged in case 3. Put it back above the imports.
   - **A `*_dir` or `base` value moved.** A forgotten override from case 2: re-add the
     assignment (the library's default is not your layout).

   Then run `just check` once and read the exit code. `--summary` proves the file
   parses and every dependency resolves; only running it proves the commands still
   work.

Two things the proof does not cover, so check them by hand:

- **CI**, if a workflow calls a recipe by name (`rules/cicd/pipeline.md`: CI calls
  `just check`, never its own copy of the commands). `--summary` shows the names, so
  a disappeared name is caught by step 4 — a *renamed* one is not.
- **`.gitignore`**, if you enable Tier 3 or code review: `pr.diff`, `coverage.out`,
  `.work/`, `mutants/`, `reports/`.

If the old justfile referenced `scripts/adr-check.mjs` (or the other gate scripts),
those copies under `scripts/` are now dead — the library calls them in
`.dev/kit/common/`. Delete them after step 4 passes, not before: they are what the
baseline ran with.
