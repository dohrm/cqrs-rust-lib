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
`docs-check.mjs`, `review-guard.mjs`, `worktree-status.mjs`, `diff-since.mjs`,
`review-context.mjs`, `ocr-candidates.mjs` and `publish-summary.mjs` all run on it (that's the reason they're Node rather
than bash — see the parent `kit/README.md`), and `dup-check` / `rules-check`
shell out to `npx`.

## What is where

| File | Holds |
|---|---|
| `common/gate.just` | `code-review`, `review-with`, `review-guard`, `mutate-from`, `mutate-mark`, `tree`, `tree-rm`, `status`, `publish-summary`, `dup-check`, `adr-check`, `docs-check`, `rules-check`, `base`, `worktree_root` |
| `rust/rust.just` · `ts/ts.just` · `go/go.just` · `python/python.just` | `<tech>-lint`, `<tech>-check`, and that tech's Tier-4 recipe |
| `godot/godot.just` | `godot-lint`, `godot-check` (+ the three variables you must override) |
| your `justfile` | the imports, `*_dir`, `check`, `mutate-diff`, `base` if the trunk is not `origin/main` |

The scripts the recipes call (`adr-check.mjs`, `docs-check.mjs`, `review-guard.mjs`,
`worktree-status.mjs`, `publish-summary.mjs`, `diff-since.mjs`, `review-context.mjs`,
`work-slug.mjs`, `ocr-candidates.mjs`, `review-prompt.md`) ship **in this directory** and are called
from here. There is nothing to move into `scripts/`: gate and implementation are
updated together, which is the whole reason they are not copied out.

To point `just code-review` at a repo-specific prompt, override one variable in the
root justfile:

```just
review_prompt := "docs/review-prompt.md"
```

## Experience contracts in docs-check

When `docs/experience/` exists, `docs-check` also checks its index, unique IDs,
actor/scope, status, visual policy, behavior/evidence sections and local links.
`stable` requires a declared developer validation source; `specified` requires
links to supplied visual references. HTTP(S) references are syntax-checked offline,
not fetched. Local targets must exist; anchors and their contents are not checked.
The helper `experience-check.mjs` ships beside `docs-check.mjs`; update both together
(the installer copies the common kit directory). No new recipe or configuration.

Legacy single-file `EXPERIENCE.md` remains valid. The checker does not authenticate
approval or verify a UI: evidence may explicitly remain unverified. Format and
lifecycle are in `rules/product/experience.md` in the source library.

## Local T3/T4 runs measure the block, not the branch

`code-review` used to read `git diff <base>...HEAD` on every run, so a branch grown
over several loops re-reviewed everything the previous runs had already cleared — the
tenth block paying for the nine before it, until `review_max_bytes` failed one for the
size of its own history. Each gate now records the commit it last **passed** on and
diffs from there:

| | |
|---|---|
| `.work/<slug>/.latest_review` | what `code-review` cleared |
| `.work/<slug>/.latest_mutate` | what `mutate-diff` cleared, once you wire it |

`<slug>` is, first answer wins: `work_slug` if you set it; else the `.work/*/rfc.md`
whose `**Branch**` header is the current branch (`/rfc` writes it per phase), so the
marker lands next to its RFC; else, for an L4 phase branch `phase/<slug>/<n>`, that
`<slug>` when `.work/<slug>/rfc.md` exists; else the branch name (`work-slug.mjs`). Both files are per
developer — `.work/.gitignore` is kept carrying the patterns, so they stay private even
in a repo that commits its `.work/` RFCs.

## What the reviewer never sees

`review_exclude` drops what a human reviewer skips too — lockfiles, the installed
agent-rules tree, `.work/`, build outputs, vendored dependencies, generated code — and
the reviewer is told each omission is deliberate (the `--stat` inventory stays whole).
`review_secrets` withholds credential files (`.env*`, `.ssh/`, `id_*`, `.netrc`, `.npmrc`,
`.pypirc`, `.dockercfg`) from the model whatever `review_exclude` says: override it only
to add. Every `.env.*` is withheld, templates included — a git pathspec cannot re-include
`.env.example` once `.env.*` matched it. Both lists come from Open Code Review's defaults;
its default exclusion of tests was not taken, on purpose.

## The review proves what it read

The report names every file of the diff under `### Files reviewed`, and each finding
points at a `path:line` (or `path:start-end`) as of the commit under review.
`code-review` checks both on the temp report before promoting it (`review-guard.mjs
--expect-files .work/review-files.txt`): an incomplete report is not promoted — the
previous one stands, the marker stays, the recipe fails, and the report is kept as
`.work/review-incomplete.md`, because its findings are real even when its verdict is
not trusted — unless it still blocks (a
`CRITICAL`, or markers nobody can read: a truncated report), which is promoted anyway,
so that it still blocks the push. The check is a
floor, not a reading: it proves a file was named and a line exists, not that either was
understood.

## An optional second reviewer: OCR

`just ocr=1 code-review` first runs [Open Code Review](https://github.com/alibaba/open-code-review)
(`ocr_cmd`, pinned by `ocr_version`; `ocr_cmd := "ocr"` uses a global install instead) on the same range, with a brief of the RFC (title, Goal, Local decisions) as background
(`ocr_background_chars`, 4,000: OCR warns past 2,000 and aborts past 8,000), and hands its findings to the reviewer as `=== OCR CANDIDATES ===`: leads
it confirms against the code or drops, never a verdict of their own. OCR trades recall
for precision with a deterministic pipeline; ours is one strict pass that also judges
ADRs, experience contracts and AI slop — the two complement each other.

### How to use it

1. **Once per machine**, pick OCR's provider and model (stored in OCR's own config, not
   in the repo), with the `ocr_version` that `gate.just` pins:

   ```sh
   npx @alibaba-group/open-code-review@<ocr_version> config provider   # interactive
   npx @alibaba-group/open-code-review@<ocr_version> config model
   ```

   Non-interactive (CI): `config set provider anthropic`, `config set model <id>`,
   `config set providers.anthropic.api_key "$ANTHROPIC_API_KEY"`.
2. **Per review**: `just ocr=1 code-review` (`1`, or any value but empty, `0`,
   `false`, `no`, `off`).
3. **Or from the root justfile**, which overrides `gate.just`'s variables (the
   `set allow-duplicate-variables` header) — every knob is optional:

   ```just
   ocr                  := "1"            # every `code-review` runs OCR; `just ocr=0 code-review` skips it once
   ocr_cmd              := "ocr"          # a global install instead of the pinned npx
   ocr_args             := "--max-tokens-budget 300000"  # cap OCR's spend; a budget stop keeps what it found
   ocr_background_chars := "4000"         # RFC brief size; OCR aborts past 8,000
   ocr_exclude          := "**/*.lock,…"  # extend together with review_exclude
   ```

4. **Read the result**: the reviewer's report ends with `OCR: <n> shown, <k> confirmed`
   (or `OCR: not run — <reason>`). OCR's raw output stays in `.work/ocr.json`, its log
   in `.work/ocr.log`.

**Running OCR by hand instead** (outside `code-review`) loses what the recipe pins:
give it the branch's base, not an older ref, or it reviews other changes than yours
(`--from` is resolved by merge-base). The recipe runs:

```sh
ocr review --audience agent --format json --output .work/ocr.json \
  --from <base> --to HEAD --background-file .work/ocr-background.md --exclude '<ocr_exclude>'
```

Its candidates are unchecked there: nothing confirms them against the current code.

- **Its own tokens**: OCR calls its own LLM and spends that provider's tokens, on top of
  the reviewer's.
- **Failure never blocks**: no key, rate limit, network — the prompt says "OCR pass not
  run" with the tail of `.work/ocr.log`, and the review runs without it.
- **Read-only**: on 1.12.9 and 1.12.13 its tools only read git (`ls-files`, `grep`, `show`). An MCP
  server in OCR's own config would add tools: keep that config empty where it gates.
- **Same scope, bounded cost**: `ocr_exclude` mirrors `review_exclude` in OCR's
  doublestar globs (`**/*.lock`: its `*` never crosses `/`; extend both together);
  `ocr_args` takes extra flags, e.g. `--max-tokens-budget 300000`. The section is
  capped at 32 KB — most severe first, each text truncated, warnings capped — so OCR
  never makes a review too large to run.
- **Its text is data**: OCR's comments are another model's output about a diff that may
  be hostile — flattened (continuation lines indented, every line break normalized),
  closed by an explicit `=== END OCR CANDIDATES ===`, and the contract tells the
  reviewer never to follow them.
- `review-with` does not run it: an ad hoc question does not need a second pipeline.

## The review reads its RFC

`code-review` and `review-with` put an `=== RFC ===` section before the diff
(`review-context.mjs`): the local RFC of that slug without its `## Revisions` and
`## Log`, or, for a ticket RFC, the ticket the implementing agent cached raw in
`.work/<slug>/review-context.md` before the review (`skills/rfc/ticket.md`). A ticket RFC
with no cache is declared as "RFC context not available", and the reviewer reports it —
it never reviews without its frame silently. The cache holds ticket comments, so it is
gitignored with the markers.

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
- **`.gitignore`**, if you enable mutation or code review: `pr.diff`, `coverage.out`,
  `.work/`, `mutants/`, `reports/`.

If the old justfile referenced `scripts/adr-check.mjs` (or the other gate scripts),
those copies under `scripts/` are now dead — the library calls them in
`.dev/kit/common/`. Delete them after step 4 passes, not before: they are what the
baseline ran with.
