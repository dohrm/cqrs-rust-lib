# Changelog

What changed for a repo that **installs** these assets — not a record of the work.
Newest first, grouped by impact, breaking changes and their migration first.

> This file starts with the entries below. Nothing before them was ever tagged, so
> there is no release to reconstruct; that history lives in the git log.

**Versioning.** SemVer on the *asset contract*: an installed rule, skill, kit file or
CLI flag a consuming repo depends on. **We are on `0.x`, where MINOR is the breaking
slot** — pin a ref (`--ref <tag>`) if you need the guarantee `0.x` does not give.

## [Unreleased]

### Breaking — `/investigate` is now `/defect`

`/investigate` was a debug method that stopped after every phase. `/defect` keeps the same
method and adds a lifecycle around it ([#44](https://github.com/dohrm/claude-rules/issues/44)):

- **Two commands.** `/defect <symptom or ticket>` diagnoses: it reproduces the bug, ranks
  3 hypotheses and confirms the root cause, then stops. `/defect fix` lands the minimal fix
  as a PR. It refuses to start without a recorded diagnosis, and its reproduction must
  still fail on the base before any fix.
- **Autonomy levels.** L1 stops after each phase, as `/investigate` did. L2 stops after
  the diagnosis. L3 runs through to the PR. The human gate is the merge.
- **A state file on its own branch.** The state lives in `.work/<slug>/defect.md`, committed
  on the bug's branch, which is cut before the diagnosis. The file's `**Branch**` header
  places the gate markers, and `just code-review` reads the file as the frame.
- **`just status` shows one escalation per slug.** Before, a tree showed only its newest
  state file's escalation. Now each slug reports its own `## Blocked on the human`,
  prefixed with its label. Within one slug the RFC still wins, so an `rfc.md` replaces
  the escalation of the `defect.md` it came from.
- **Restore, never change.** `/defect` only restores established behavior. A change request,
  a contested expected behavior, three refuted hypotheses, a touched ADR, public API or
  schema, or a fix larger than one PR all go to `/rfc`.

Migration:

- `update` replaces `investigate` with `defect` in the lock, installs `skills/defect` and
  deletes the old `investigate` skill directories for Claude and Codex.
- `add investigate` and `remove investigate` still work: `investigate` is a profile alias.
  So is `bug`, transitionally, for installs taken from `next` while the skill carried that
  name: their `update` installs `defect` and removes the `bug` skill directories. Unlike
  `investigate`, the `bug` alias is removed before the first tagged release: update
  from `next` before pinning a tag.
- Why not `/bug`: Claude Code has a built-in `/bug` (report a bug), and a project skill
  of the same name replaces it in a terminal session, here and in every repo that
  installs the profile.
- The typed `/investigate` command is gone. The `defect` description keeps "bug",
  "investigate" and "debug" as triggers, so asking in plain words still finds it.
- Wired the kit by hand? Update `kit/common` whole: `work-slug.mjs`, `review-context.mjs`
  and `worktree-status.mjs` changed.

### Breaking — the installer writes only over what it owns

The lock now records every file the installer writes, for every target, with its
hash, and `add`, `update` and `remove` refuse a tree they cannot explain
([ADR-0010](docs/adr/0010-installer-refuses-unexplained-tree.md), [#33](https://github.com/dohrm/claude-rules/issues/33)).
Four behaviours change:

- **A hand edit or an unrecorded copy at an installed path stops `add`/`update`.**
  Claude and Cursor copies used to be overwritten silently. Now the command lists
  every such path (an edited rule, a file the lock does not record even when
  byte-identical, a foreign file in an installed skill directory), exits 1 and
  changes nothing. Move the edit to `.dev/overrides/` (below), or re-run with
  `--force` to overwrite the listed paths.
- **The first `update` from a lock without file hashes overwrites once.** Any lock
  written before this change is adopted: the run lists every installed destination,
  overwrites it whole (local edits included) and records it. Without a terminal it
  needs `--force`. Save your edits under the listed paths first, then run
  `update --force` once and review `git diff`.
- **Legacy trees are purged only by that adoption.** `.dev/rules/`, `.opencode/`,
  the old `.claude/kit/` directories and retired skills are deleted by the adopting
  `update`, never by a later one. One left behind after that is yours to delete;
  `doctor` reports the retired-target trees.
- **`remove` deletes only the files it recorded.** It reads the lock's `installed`
  inventory, not a registry, and refuses a lock without hashes: run `update` once
  first. A file it did not write survives. An edited file it would delete stops it;
  `--force` deletes it anyway. An edited Codex rule that `remove` must keep and
  rescope stops it even under `--force`: restore it with `update --force` first.

### Added — layered sources and `.dev/overrides/`

A team can add its private rules, skills and kit as a second pinned source, and a
repo can keep its own edits; one `update` replays every layer
([ADR-0008](docs/adr/0008-layered-sources.md), [#33](https://github.com/dohrm/claude-rules/issues/33)).
See [sources, layers and overrides](docs/reference.md#sources-layers-and-overrides).

- **`sources` in the lock.** An ordered list of `{ name, url, ref }`, `core` first.
  An old `{ repo, ref }` lock reads as `core` and is rewritten in the new shape by
  the next write. Sources are fetched with `git` on `PATH` and your own credentials
  (`github:owner/repo` or any git URL); `giget` is gone.
- **`add --source <name>=<url>#<ref>` / `remove --source <name>`** append and drop a
  source. **`--ref <name>=<ref>`** moves one source; a bare `--ref` still moves `core`.
- **Each source's `registry.json` decides** what it installs; registries merge in
  order (profiles append, aliases and `retired` merge, later wins), and a source may
  define profiles only it has. Its **`exclude`** removes files or whole skills of the
  sources before it, and `update` deletes their installed copies.
- **`.dev/overrides/`**, if present, is the last layer: same layout as a source, never
  written by the installer. A file there replaces the installed one and survives
  `update`.
- **Overrides are listed.** The lock records `overrides: [{ path, by, over,
  baseHash }]`, and `add`/`update` print each one. When a replaced file changed in the
  layer under it, `update` warns
  `⚠ <path> changed in <over> (overridden by <by>)` (`removed from <over>` when it is
  gone there); **`update --strict`** fails before writing anything.
- **The lock records the merged `aliases`**, so an offline `remove` unpacks an alias
  only an overlay defined, and **`doctor` audits the hashed inventory** offline: an
  overlay-only profile passes, an edited or deleted installed file fails.
- **Three new refusals:** two registry entries that install the same source path to
  two destinations, two entries whose source paths nest, and a skill directory that an `exclude`, a dropped entry or a
  `remove` empties while it still holds a file the installer did not write
  (`--force` removes it).

### Breaking — stricter language gates

The language rules' high-stakes lines (taken from Open Code Review's per-language
checklists) are now **checked**, not just written: a gate that was green can go red.
Fix the finding, or keep an exception with its reason on the same line. Each language
says whether `update` delivers it or you merge it by hand — the configs are copies
the installer never merges, so `update` refreshes `.dev/kit/<lang>/` and nothing else.

- **Rust — delivered by `update`** (`rust.just`). `rust-lint` adds clippy
  `dbg_macro`, `todo`, `unimplemented`, `undocumented_unsafe_blocks` on all targets,
  and `panic`, `string_slice` on `--lib --bins` (tests stay free). An invariant keeps
  `#[allow(clippy::panic, reason = "…")]`; a byte slice becomes `s.get(a..b)`. To
  adopt later, override `rust-lint` in the root justfile with the old two lines.
- **TypeScript — merge by hand**, from `.dev/kit/ts/eslint.base.js` (or the
  `ts-web` / `ts-node` / `ts-tauri` overlay you copied) into `eslint.config.js`:
  `parserOptions.projectService` (type-aware linting — slower), then
  `no-floating-promises`, `no-misused-promises`, `switch-exhaustiveness-check`,
  `no-implied-eval`, `no-eval`, `eqeqeq: smart`; the React overlays also ban
  `dangerouslySetInnerHTML`. From the matching `tsconfig.*.json` into `tsconfig.json`:
  `noUncheckedIndexedAccess` — the largest churn (`xs[0]` becomes `T | undefined`);
  merge it in its own commit.
- **Go — merge by hand**, from `.dev/kit/go/golangci.base.yml` into `.golangci.yml`:
  `bodyclose`, `sqlclosecheck`, `rowserrcheck`, `noctx`, `contextcheck`,
  `containedctx`, `errorlint`, `nilerr`, `exhaustive` (a `default` does not make an
  enum switch exhaustive).
- **Python — merge by hand**, from `.dev/kit/python/pyproject.snippet.toml` into
  `[tool.ruff.lint]`: `ASYNC`, `BLE`, `DTZ`, `T20`, `PERF` in `select`, and `T20` in
  the `tests/**` per-file-ignores.
- **Rules — delivered by `update`.** New `rules/go/concurrency.md` (goroutine
  ownership, channel closing, `defer` in loops, writer `Close` errors, typed nil).
  Rust gains task ownership, `spawn_blocking`, cancellation-safe `select!` and bounded
  retries; TypeScript a deadline on every `fetch` and `Promise.all` for independent
  awaits; Python `asyncio.to_thread` and no shared module-level state.

### Added

- **Autonomy L4 — orchestrated** (ADR-0009). An RFC carrying a `## Delegation board`
  can run at L4. The orchestrator checks its domain at launch (or drops to L3), gives
  each phase to an executor the board lists on `phase/<slug>/<n>`, merges phases into
  `rfc/<slug>` on proof, and hands back one PR with a final report. The human merges
  it. Doctrine in `agent/autonomy.md` § L4. **Hand merge:** add `'rfc/**'` to the
  `pull_request` branches of your CI and mutation workflows (the templates now have
  it), or delegated merges run without Tier 4. `work-slug.mjs` resolves a phase branch
  to its RFC.

- **`/update`, and the CHANGELOG ships with the install.** Every install now writes
  this file to `.dev/kit/CHANGELOG.md`; after `update`, `git diff` on it is exactly
  what changed since the repo last installed — no tag needed. `/update` (in
  `product`) runs `update`, reads that diff, triages each entry (delivered by
  `update`, hand merge, stricter gate) in `.work/update.md` and stops for a yes on
  every stricter gate before merging and re-greening `just check`. `/migrate` stays
  for an older harness (no `modules`, no imported just library).
  **Get it:** `add product`, then `update` — the first `update` writes the baseline,
  so this first time `/update` falls back to the kit diff.

- **The review reads its RFC.** `code-review` and `review-with` put an `=== RFC ===`
  section before the diff: the local RFC without its history, or the ticket the
  implementing agent cached raw in `.work/<slug>/review-context.md` (gitignored) —
  a ticket RFC without a cache is declared, and reported as a 🟡, never reviewed
  silently without its frame. The gates now find the RFC by its new `**Branch**`
  header, so the review marker lands beside it instead of in a directory named after
  the branch. Install with `update`; existing RFCs gain the header at their next phase.

- **Credential files never reach the reviewer's model.** `.env*`, `.ssh/`, private keys
  (`id_*`), `.netrc`, `.npmrc`, `.pypirc` and `.dockercfg` are withheld from the review
  diff by `review_secrets` — kept even when a repo overrides `review_exclude` — and the
  contract makes a committed private key a 🔴, any other such file a 🟡 to confirm.
  `review_exclude` also drops build outputs, vendored dependencies and generated code
  (`node_modules/`, `vendor/`, `dist/`, `target/`, `*.min.*`, `*.pb.go`, snapshots).
  Both lists come from Open Code Review's defaults; its test exclusion was not taken.

- **An optional OCR pass.** `just ocr=1 code-review` runs Open Code Review
  (alibaba/open-code-review, pinned `ocr_version := "1.12.13"`) on the same range first, with a 4,000-character brief of the RFC (title, Goal, Local decisions);
  its findings reach the reviewer as candidates to confirm or drop, never as a verdict.
  It needs its own LLM provider and costs its tokens; a failed pass is declared and the
  review runs without it. Off by default.

- **The review proves what it read.** A report must name every file of the diff under
  `### Files reviewed`, and its `Location`s must exist (`path:line` as of the reviewed
  commit). `code-review` rejects an incomplete report before promoting it: the previous
  one stands and the marker stays, and the report is kept as `.work/review-incomplete.md`
  — its findings are real even when its verdict is not trusted — except one that still blocks (a `CRITICAL`, or
  unreadable markers), which is promoted anyway. An oversized prompt now says to cut
  the RFC phase smaller; `/rfc` sizes phases to fit.

- **The review contract follows ADR-0005.** An accepted shared decision binds like an
  Accepted ADR (🔴). An in-place rule exception with its reason is compliant; without
  one it is a 🟡, and the same exception repeated is a 🔵 against the rule. Departing
  from an RFC local decision is a 🟡 at most, unless the RFC records the change.

- **`/rfc`, the execution path** (ADR-0006, ADR-0007). One request for change is
  framed in one RFC: verdict, local decisions, steps with proofs and a stopping
  condition. A large change is decomposed into phases (one PR each) and holds (a
  migration to finish, a deploy) inside the same RFC. You iterate on it with the
  agent, mark it `ready`, and the agent implements it under the declared autonomy
  level — at L3, every phase, stopping only at holds and hard checkpoints. The RFC lives where `> RFC store:` in `docs/ARCHITECTURE.md` says:
  `local` (default) or a ticket in any tracker. The project's `## RFC store` section
  is the adapter: tracker, access (CLI first, then REST, an MCP server last), bot
  account, status mapping. A webhook dispatcher may pre-fetch the ticket into
  `.work/<slug>/ticket-context.md` to save the agent the reads. Install with `update`
  on a `product` profile.

- **`/goal-setup` for Codex.** It frames one durable `/goal` with a bounded
  objective, stopping condition, checkpoints, progress proof and pause causes.
  It writes `goal.md` only when no `/tasks` worklist exists; status and summary
  tooling recognize that state file. `/loop-setup` remains for repeated loops.

- **Codex installation and module guidance.** New installs select Claude/Codex;
  existing locks retain their targets and Cursor remains optional. Add Codex
  explicitly with `add <installed-profile> --agent codex`.
- Root/module `AGENTS.md` blocks request scoped rule reads after alias expansion.
  `modules["."]` records root membership. Owned Codex files are tracked in the lock;
  updates/removal preserve local instruction text and unknown files, and reject
  conflicts before writes. `doctor` audits this output; `budget --agent codex`
  estimates requested reading without claiming native dynamic loading.


- **Experience contracts per journey and actor.** `/experience` now explores or
  stabilizes a selected screen/workflow under `docs/experience/`, indexed by
  `docs/EXPERIENCE.md`. `exploring`/`stable` and `toolkit`/`specified` are independent:
  retain a behavior without freezing its layout, or honor supplied UX specifications.
  Developer instruction is required for stabilization; agents may record it directly.
  Existing single-file documents keep working; migrate only the selected journey.
- **Frontend experiments may keep components local** until retained and useful for
  reuse. `/design-system`, `/ui-prompt` and `/tasks` carry the toolkit, actor scope
  and relevant experience constraints without requiring a new ADR per component.
- **T3 review distinguishes contract violations, suggestions and verification gaps.**
  A demonstrated regression against a stable experience or supplied visual requirement
  can block; differing actor journeys and exploratory composition do not inherently
  conflict. Runtime/visual checks are never claimed from source inspection alone.
  `.work/review-report.md` and the pre-push guard's behavior are unchanged.
- **docs-check validates experience structure.** Unique IDs, required metadata and
  sections, index links and local evidence/spec references. Stable units require a
  validation source; specified visuals require references. No browser, network fetch
  or proof of human authorship. Update the whole common kit, including its new
  `experience-check.mjs` helper; no additional wiring beyond `docs-check`.

### Changed

- **Rust tests have one layout: `rules/rust/test-layout.md`.** Unit tests in
  `foo/tests.rs` declared by `#[cfg(test)] mod tests;` (no inline `mod tests { … }`, no
  `#[path]`), integration tests in one `tests/it/` crate, fixtures shared across crates
  behind a `test-support` feature. `rules/testing/strategy.md` now points to it instead
  of suggesting an inline module. It is a convention, not a gate: existing inline test
  modules still pass `just rust-lint`. Move them when you next touch the file.
- **`/rfc` asks for a model when a step rewrites existing state.** A new optional
  `## Model` section lists the invariants the change keeps and the states it refuses
  instead of handling; a review finding asking to handle a refused state is answered by
  the refusal. The OCR brief (`review-context.mjs --brief`) now carries it with Goal and
  Local decisions. Learned on #33: without a model, each review round found one more
  combination of old and new state.
- **L4 launch check: a missing witness is not failing first.** `agent/autonomy.md` § L4
  now says that when the RFC writes its own witness, the stopping condition names it and
  fails on its absence: `node --test a b` drops a missing `a` when `b` exists.

- **RFC store: `Act as` becomes `Agent accounts`.** The identity is whoever the
  tracker access is authenticated as (whoami at run time); the adapter now lists only
  the accounts reserved to agents, or `none`. Any other whoami is a human's, so the
  agent signs `on behalf of <whoami>`. Rename the line in your `docs/ARCHITECTURE.md`
  § RFC store by hand: `Agent accounts: <your bot>`, or `none` if the agent ran under
  your account.

- **Fewer, broader ADRs** (ADR-0005). A project ADR now needs an arbitration that is
  hard to reverse, constrains more than one feature, and refused real alternatives.
  Enterprise policy goes to a shared decision repository, stack conventions to the
  installed rules, feature-local choices to the work document and PR. A rule deviation
  is an in-place exception with its reason, not an ADR. Records no longer carry lock
  clauses, and the "we will outside Decision" split test is gone. `/architect` stops
  writing one ADR per stack row; `/observability` keeps SLO targets in
  `docs/OBSERVABILITY.md` instead of one ADR each. Existing ADRs stay valid; consolidate
  them with `/adr-review` when convenient.

- **Four validation depths:** T1 lint, T2 technical checks, T3 independent review,
  T4 mutation/Go coverage ratchet. Generated justfile guidance now matches PR mutation
  cadence; local mutation stays optional and shipped CI remains non-blocking during
  calibration. Existing justfile recipes are not automatically rewritten.
- **ADR scope clarified:** durable architectural/security constraints and mandated
  structural libraries need records; reversible implementation choices within the
  selected profiles do not require an ADR merely because they are new.

### Breaking

- **A custom `review_prompt` must ask for `### Files reviewed`.** `code-review` now
  rejects a report that does not name every file of the diff there, or whose
  `Location`s do not exist. A repo that overrides `review_prompt` fails every review
  until its prompt asks for the section and for `path:line` Locations: copy both from
  `.dev/kit/common/review-prompt.md` § Output format.

- **`/plan`, `/tasks` and `/goal-setup` are removed** (ADR-0007), and so is the
  `goal-setup` profile. `update` drops `goal-setup` from an existing lock (it says
  so) and deletes the `plan`, `tasks` and `goal-setup` skill directories; review them
  with `git status`.
  `/rfc` replaces all three. The coherent block of `agent/autonomy.md` is the RFC,
  not the sprint. `just status` and `just publish-summary` read `.work/<slug>/rfc.md`
  and `loop.md` only. Finish open `.work/<slug>/PLAN.md` / `tasks/` work first, or
  rewrite what remains as RFCs. `/loop-setup` stays, for repeated chores only; it no
  longer extends a worklist.

- **`cqrs` drops `cqrs-rust-lib`.** The profile is the write/read +
  event-sourcing principles. `rules/cqrs/rust.md` is gone (it was a
  cookbook for that library). A generator that owns HTTP is still an
  exception in `api/rust.md`, unnamed. Pick any crate; the rule will
  not name one.

- **`agent` is a profile, not shared.** `add rust` no longer installs
  autonomy, subagents, or `kit/common`. Shared is `rules/common` only.
  `add agent --level gates` is the previous gift. A lock written before
  this, on `update`, is migrated at `--level gates` (so existing installs
  keep their kit) and gains `agent`; `remove agent` drops it.
- **`--level rules|gates|ratchet`.** Default on a new `add` is `rules`
  (prose, no kit). `--level gates` copies the language kit. `--level
  ratchet` is never the default; `init` then writes `mutate-diff` live.
- **`--root` is the glob lever** (`--module` still works). `/architect`
  recommends aliases (`rust-api`, `go-api`, `python-api`, `ts-web-app`,
  `ts-tauri-app`, `ts-node-api`) plus `--root` and `--level gates`, not the seven-profile
  bag that loaded 21 rules on a domain entity.

### Fixed

- **`reviewer=codex` reviewed the diff blind.** The review prompt told every reviewer
  "you have no shell — Read/Glob/Grep is all you need", which is true for Claude only.
  Codex reads through its read-only sandbox shell, took the sentence literally, and
  reported it could not read the repo, so `CLAUDE.md`, the ADRs and any file outside the
  diff went unchecked. The prompt now names each CLI's read-only mechanism and asks every
  reviewer to read the repo. Delivered by `update` (`.dev/kit/common/review-prompt.md`);
  a repo that overrides `review_prompt` merges the "Read-only, and not by promise"
  bullet by hand.

- **`mutate-diff` never advanced its marker, so mutation re-ran the whole branch
  every time.** `init` wrote it as a dependency list — `mutate-diff: rust-mutate` —
  which runs each mutator with its *default* range (the whole branch since `base`)
  and never calls `mutate-mark`. `.work/<slug>/.latest_mutate` was therefore never
  written, and the incremental economy `diff-since.mjs` exists for was dead on
  arrival for mutation. `code-review` was unaffected because it is one
  self-contained recipe in `gate.just` that marks at its own end, which is exactly
  why the bug survived: the half that was watched worked.
  `init` now emits the body `kit/common/gate.just` always documented —
  `just rust-mutate "$(just mutate-from)"` then `just mutate-mark` last, so a
  surviving mutant leaves the marker where it was and the next run re-mutates that
  block together with its fix. Only `rust-mutate` takes a range (Stryker and mutmut
  keep their own caches, `go-cover` measures the whole tree), so the other mutators
  are emitted as plain lines. Repos without `kit/common` keep the dependency form —
  the marker recipes are not there to call.
  **Re-run `claude-rules init` to pick it up**: the justfile is yours, so `update`
  does not rewrite it.

### Added

- **`kit/rust`: Tier 3 made affordable enough to stay in the loop.** Measured on
  one real workspace, a 32-mutant sprint diff cost ~50 minutes — **22 of them the
  baseline** (972 s build + 363 s test) before the first mutant ran. So mutation in
  Rust is *compile*-bound, not test-bound, and the levers are linking and codegen,
  not test selection. Two consequences are now written down in
  `kit/rust/README.md` under **Tier 3 economics**: the unit of recompilation is the
  crate (so the hexagonal rule is also a performance rule), and a killed mutant
  exits on the first red test while a **survivor pays the whole suite** — the cost
  spikes exactly when the gate has something to say, which is how a gate ends up
  switched off (`rules/testing/ratchet.md`).
  - `mutants.toml` now ships `exclude_globs` **filled** with generated-code
    patterns instead of a commented example. `review_exclude` in `gate.just`
    already kept generated files out of code review; the same doctrine now reaches
    the other gate. This protects the *variance*: a normal sprint is a few dozen
    mutants, the sprint that regenerates an OpenAPI or protobuf client is
    thousands. A test asserts the list cannot be quietly emptied again.
  - **`cargo-profile.snippet.toml`** (`[profile.mutants] debug = "none"`) and
    **`cargo-config.snippet.toml`** (sccache, mold/lld) — both opt-in, because each
    needs merging into a file the installer never touches. `cargo-config` also
    documents why a shared `CARGO_TARGET_DIR` is the trap it looks like the
    solution to: cargo locks it (so parallel worktrees serialize), branches thrash
    each other's fingerprints, and mutation artifacts land in the tree
    rust-analyzer reads. Mutualize with a content-addressed cache instead.
  - **`mutate_args`** — one escape hatch rather than five knobs, shipped empty:
    `--profile mutants`, `--test-tool=nextest`, `--baseline=skip`, `--in-place`.
    `--baseline=skip` removes the whole 1335 s baseline and is **fail-open alone**
    (against a red tree every mutant "fails" and reads as killed) — it is safe only
    where something just proved the tree green in that directory, so it waits for
    an orchestrator. `--in-place` wants a dedicated mutation worktree.
  - **`mutate_jobs`, default `2`** — matches the `-j 2` `mutation-ci.yaml` already
    used, so `rust-mutate` now runs two jobs where it ran one. Each job copies the
    build directory, so raising it multiplies **disk**, not just CPU.
- **`just tree <slug>` / `just tree-rm <slug>` — worktrees you can find, and that
  do not accumulate.** The doctrine used to say `git worktree add ../<repo>-<slug>`,
  which scatters checkouts among the sibling repos of your dev directory; finding one
  later was an archaeological dig. Every tree now lives under **one root outside every
  repo** — `$CR_WORKTREES`, default `~/.worktrees/<repo>/<slug>` — at a path that is
  *derivable* rather than remembered, so an agent computes it instead of inventing one,
  and `just status` prints it absolute (it used to print `../../..` forms nothing
  could paste). Not `<repo>/.worktrees/`, tempting as that is: a checkout nested in
  the repo is one every tool scans — cargo workspace globs, `cargo mutants` copying
  the workspace to scratch, rust-analyzer, ripgrep, `just check`.
  - **One tree per capability, one branch per sprint inside it.** `.work/<slug>/` is
    per-tree and holds that capability's plan and all its worklists, so a tree per
    sprint splits one work unit across trees. This also settles a contradiction:
    `autonomy.md` said per capability and `/tasks` said per sprint.
  - **Parallelise across capabilities, never across the sprints of one.** A sprint is
    a vertical slice, so two sprints of one capability traverse the same layers and
    collide by construction — they are independent as promises, not as files.
  - `tree-rm` removes the worktree **and** the branch in one act, and removes neither
    unless the tree is clean and the branch is merged into `base`. Both checks run
    *before* anything is removed: letting git enforce them in passing looked
    equivalent and was not — `worktree remove` succeeded, `branch -d` then refused,
    and the recipe had deleted the only checkout of work it declined to clean up.


- **`devstack` — the contract between an agent and a running app, plus the
  `process-compose` lifecycle that makes it cheap.** An agent that needs the app
  running had three ways to lose a turn: holding a dev server in the foreground
  (the turn blocks, the output truncates, nothing is learned), leaving an orphan
  (it holds its port, so the next start fails with a diagnostic about the port and
  not the code — and the human inherits it), and asserting green against a process
  that never reloaded the edit.

  `rules/devstack/running.md` is the rule and holds with or without the kit.
  `kit/devstack` at `--level gates` is the lifecycle: `just up` (detached,
  headless, returns in ~0.5s) / `down` / `ps` / `restart <svc>` / `logs <svc>` /
  `follow` / `ports` / `stack` (attach the TUI; detaching leaves the stack up) /
  `stack-check` (`--dry-run`). NOT a gate — running the app is how you observe
  behaviour, `just check` is what proves the code.

  **Never tail `.logs/<svc>.log` to find out what just happened**, and this is
  measured rather than assumed: process-compose writes each per-process log file
  in 4 KB blocks and flushes it in full only at shutdown. A 400-line service had
  40960 bytes on disk while running; a one-line service had **zero**, while
  `process logs` returned that line immediately. A chatty service crosses the
  block in seconds and looks fine, which is why the trap survives casual testing —
  so `just logs` asks the supervisor, and the file is for history and post-mortem.

  **A unix socket per work tree, and it is not optional.** process-compose serves
  its control API on TCP `:8080`, so two worktrees of one repo fight over it — and
  the second `up` reports success with no port error at all, after which a
  `process list` returns one view that does not say which tree it describes. An
  agent then debugs its own code while the interference comes from a sibling
  checkout. `devstack.just` passes `-U -u /tmp/pc-<tree>.sock`, derived from the
  tree directory; with it, `just restart` in one tree provably left the other
  worktree PID untouched. It does NOT isolate your app own ports: one tree runs
  the stack at a time, the others run `just check`, which needs no server.

  **Wiring**: install only once the repo runs more than one process — in practice
  almost any repo with a UI, since a front/back monorepo is already two. Copy
  `process-compose.snippet.yaml` → `process-compose.yaml` and adapt it (a shape,
  not a stack), gitignore `.logs/`, and `mkdir -p ~/.config/process-compose` —
  without that directory process-compose prints two JSON debug lines on stderr for
  every invocation, in front of every recipe output an agent reads.

- **Tier 3 measures the block, not the branch.** `just code-review` read
  `git diff <base>...HEAD` on every run, so a branch built by successive
  loops re-reviewed everything the previous runs had already cleared — the
  cost grew with the branch, not with the block, until `review_max_bytes`
  failed a two-file change for the size of its own history. Each gate now
  records the commit it last **passed** on and diffs from there:
  `.work/<slug>/.latest_review` and `.work/<slug>/.latest_mutate`, resolved by
  the new `kit/common/diff-since.mjs`. `<slug>` is the branch name unless you
  set `work_slug` (point it at a `/loop-setup` capability to park the marker
  next to that loop's `loop.md`).

  Three properties make it safe rather than merely cheaper. The marker
  advances **only after the gate passed**, so a `CRITICAL` leaves it where it
  was and the fixes come back for review with the block they fix. Every way it
  can be wrong — absent, unparseable, a commit that was rewritten or rebased
  away, a branch switched under it — falls back to the full `<base>...HEAD`; a
  stale marker costs one whole pass, a marker trusted blindly hides a
  rewritten commit from every review that follows. And it is local: CI still
  measures the whole PR, `just incremental=0 code-review` is the whole-branch
  read, worth one run before opening one. The `--stat` inventory in the prompt
  stays whole-branch too, so an incremental diff can never read to the
  reviewer as "the earlier commits do not exist" (`review-prompt.md` gained
  the `=== REVIEWED THROUGH ===` section that says which of the two it is
  looking at). Running it twice with nothing new in between is refused before
  dispatch — the standing report is the verdict — rather than spending an LLM
  call on an empty diff.

  **Wiring**: nothing to do for review — `claude-rules update` refreshes
  `kit/common` and the next `just code-review` starts recording. Mutation is
  yours, because `mutate-diff` names the mutators this repo has:
  `just rust-mutate "$(just mutate-from)"` then `just mutate-mark` (last, and
  only on success). `rust-mutate` takes the range start as an argument now,
  defaulting to `base`, so a repo without `kit/common` is unaffected. Stryker
  and mutmut already scope themselves and take no range. The markers are per
  developer: `diff-since.mjs` keeps `.work/.gitignore` carrying the two
  patterns, so they stay private even where `.work/` is committed.

  One behaviour change to know: `code-review` now calls `review-guard
  --require-report`, so a reviewer CLI that exits 0 having written nothing
  **fails the recipe** instead of reporting "not run" and exiting 0. Without
  it the marker would advance over code no reviewer ever read. The pre-push
  hook keeps the old, lenient contract — a missing report is still declared,
  never simulated.

- **`just publish-summary`** (shared `kit/common`) — a per-loop catch-up file
  for someone running several `/loop-setup` sessions in parallel. Run as the
  last step of the loop prompt, it reads whichever state file the loop was
  using (`.work/<slug>/loop.md`, or the newest `.work/<slug>/tasks/NN-*.md`),
  adds what only git knows — branch, commits ahead of `base`, a one-line
  diffstat — and writes `.work/<slug>/SUMMARY.md`: objective, guardrails, the
  remaining-work checklist, and `## Blocked on the human`. Report, not a gate:
  always exits 0 once there is something to summarize, and `status`
  (`COMPLETED`/`BLOCKED`/`BUDGET_EXHAUSTED` — the same three-way outcome the
  loop prompt already ends on) is validated both in the recipe and in the
  script, since a test calls the script directly.

  **Wiring**: opt-in, like `status` — nothing to move, the script ships with
  the library. `skills/loop-setup/SKILL.md`'s loop prompt calls it best-effort
  on either terminal path; a repo without `kit/common` wired just skips that
  line. `.work/<slug>/` is already committed working memory
  (`rules/product/documents.md`), and `SUMMARY.md` is now part of it — nothing
  new to gitignore.

- **Python HTTP API matches the other languages.** `rules/api/python.md` is
  FastAPI + Pydantic v2 (types generate OpenAPI); `rules/hexagonal/python.md`
  is the import graph (`domain/` must not import FastAPI/SQLAlchemy/httpx).
  Alias `python-api` unpacks `python hexagonal api backend`, same shape as
  `rust-api`. FastAPI's default `{"detail": …}` is not the error contract —
  problem+json stays in `backend/errors.md`. A worker/script is still
  `add python` without `api`.

- **`/onboard`** — brownfield twin of `/interview`. Inventory, CLAUDE.md
  scaffold, `.work/onboard.md`. No ADR, no PRD. Hands off to `/prd` /
  `/architect` / the `add` command.

### Changed

- **Shipped assets no longer cite this library's own decision records.** A rule or
  kit file that said "(`ADR-0002`)" was read, once installed, against the consuming
  repo's `docs/adr/` — where `ADR-0002` is a *different* decision, or none. The
  citation did not dangle, it resolved to the wrong thing, which is worse. The
  doctrine each rule states stands on its own authority; only the attribution was
  lost, and attribution is exactly what does not travel between repos.
  - Concrete numbers used as format examples in prose (`ADR-0007 § Decision`,
    "supersede ADR-0003") become `ADR-NNNN`, the placeholder the rest of the
    library already uses. They read as real references, and one day one of them is.
  - `/architect`'s `ADR-0001` stays: it is inside `<architecture-template>`, so the
    number belongs to the repo the skill is writing for.
  - A test enforces it — a shipped asset may cite a concrete record only inside a
    template block.


- **Mutation moved from the agent's pre-push loop to a pull-request gate**
  (`docs/adr/0002-mutation-is-a-pr-gate.md`, Accepted). Measured on one Rust
  workspace: 19 minutes for a 65-mutant diff at `-j 2` — 31 caught, 0 missed, and
  34 **unviable** (mutants that do not compile, carry no signal, and cost a full
  build each). Excluding those lands near 9 minutes, so the case is not wall-clock
  alone: the remaining lever is `-j`, and that is what a laptop cannot spend while
  the editor and the agent are using the cores. CI can.
  - `just mutate-diff` leaves the cadence table in `agent/autonomy.md`. It stays
    available as an **optimisation** — run it to buy back a round trip on code you
    doubt, never as a step owed before pushing. `just code-review` is unchanged and
    still local, per sprint: its feedback can redesign a block, so it stays close.
  - The three `mutation-ci.yaml` snippets say **the gate**, not "the witness". The
    Rust one now treats a persistent compiler cache on the runner as part of the
    gate rather than a commented tuning option — the same workspace took 972 s of
    baseline build cold against 113 s warm, and a cold runner turns this job from
    minutes into a quarter of an hour.
  - `CONTEXT.md` redefines **Tier** as a *depth, not a venue*: Tier 3's two halves
    now run in different places, so the old "run before push" wording could not
    survive as part of the definition.
  - Applied to TS and Python too, since the decision carries no language
    qualifier — but their mutation cost was never measured. They inherit a cadence
    justified on Rust numbers; say so rather than imply otherwise.


- **The cadence table splits `code-review` from `mutate-diff`, and names the
  coherent block.** They were one row, and they are not one thing: review is minutes,
  mutation is tens of them. Review now runs **first** — it can send a block back to
  the drawing board, and mutation would otherwise have measured code the redesign
  deletes. And "per coherent block" is now stated as **per sprint, not per task**,
  because the drift is one-way: a loop that commits per task starts running Tier 3
  per task, and then Tier 3 is a tax somebody removes.
- **`rules/testing/ratchet.md` says where mutation time actually goes.** A killed
  mutant exits on the first red test while a survivor pays the whole suite, so the
  run gets slower the more it finds — the shape of a gate on its way to being
  switched off. And in a compiled language the cost is the rebuild, whose unit is the
  crate or package: the architecture rule that keeps I/O out of the domain is also
  what makes this gate affordable.
- **`/tasks` acceptance criteria must name a command**, not "a command *or
  observation*". Step 1 already refuses a criterion no machine can check; the
  template still offered the escape hatch.


- **`just code-review` stops paying for the diff it cannot use.** The reviewer
  has no shell, so the prompt is its entire view of the change — and on one
  real repo two thirds of a 693 KB diff was `Cargo.lock`, the installed
  agent-rules tree (twice: `.claude/` *and* `.agents/`) and codegen, so it
  reviewed this library instead of the feature. `review_exclude` drops those
  bodies (lockfiles, `.claude`/`.agents`/`.cursor`/`.dev/kit`, `.work`,
  `*.gen.*`, `openapi.json`); a complete `git diff --stat` inventory now sits
  above the diff and the prompt tells the reviewer that a file listed there
  with no hunk was omitted, not unchanged. `review_max_bytes` (400 KB) fails
  the gate on a diff too big to review instead of letting the CLI truncate one
  and return a verdict over code it never received. Append your own generated
  paths to `review_exclude` in the root justfile — one single-quoted pathspec
  per word.

- **Architecture principles carry SOLID.** `hexagonal/principle.md`,
  `cqrs/principle.md`, and `portal-flat/principle.md` map S/O/L/I/D
  onto the cuts they already make, and say when *not* to add a type
  or interface. Not a scorecard.

- **Cross-cutting rules that restated a kit gate shrink to a pointer.**
  `product/documents.md`, `agent/decision-records.md`, `testing/ratchet.md`,
  and `cicd/pipeline.md` name the recipe and keep what the gate cannot see
  (one home per fact, `Implemented`, survivor triage, pinning / skip-as-pass).
  `api/rust.md` and `api/go.md` get the same does-not-see line as
  `api/node.md`. `autonomy.md` and `guardrails.md` drop the paragraph the
  scripts already enforce.

- **Rust is the first language jalon.** `just rust-lint` now denies
  `clippy::unwrap_used` / `expect_used` on `--lib --bins` (tests stay free).
  `rules/rust/quality-gates.md` is a pointer: the recipes, the configs they
  read, suppressions. Copy `rustfmt.toml` next to `deny.toml` and
  `mutants.toml` — the map is `.dev/kit/rust/README.md`. Style, errors,
  tracing, UTF-8 stay prose; they are not translated into a fourth clippy
  pass.

- **Python is the second language jalon.** `rules/python/quality-gates.md`
  is a pointer: the recipes, the `pyproject.toml` tables they read,
  suppressions. Merge `pyproject.snippet.toml` — the map is
  `.dev/kit/python/README.md`. `S` / `TRY` / `G` stay in ruff; layout,
  domain errors, `extra=` log fields stay prose. Every recipe line is
  `uv run --locked`.

- **Rust and Python sibling rules shrink to a mention** where the jalon
  already gates (clippy / ruff / mypy / `--locked`). What remains is only
  what the toolchain cannot see.

- **Go is the third language jalon.** `rules/go/quality-gates.md` is a
  pointer: `just go-lint` / `go-check` / `go-cover`. Copy
  `golangci.base.yml` → `<go_dir>/.golangci.yml` — the map is
  `.dev/kit/go/README.md`. The shipped file is **golangci-lint v2**
  (`gofmt` under `formatters`, `gosimple` folded into `staticcheck`);
  `golangci-lint migrate` if you still own a v1 file. slog, proto /
  `pb.*`, `internal/`, `%w` stay mentions.

- **TypeScript is the fourth language jalon, with three derivatives.**
  Recipes own the tools (`npm exec --no-install -- eslint / tsc / vitest`)
  — they no longer delegate to `npm run lint`. `ts` is the floor
  (`no-explicit-any`, `no-non-null-assertion`, `strict`). `ts-web` and
  `ts-tauri` add react-hooks + jsx-a11y + DOM; `ts-node` adds Node
  globals and drops DOM. Fastify schemas, Zustand / invoke, kebab-case
  filenames stay mentions. Maps: `.dev/kit/ts/README.md` and the
  derivative READMEs.

- **Godot C# is the fifth language jalon.** `just godot-lint` is
  `dotnet build` (`TreatWarningsAsErrors` on the game `.csproj`, GODOT001–003 in-process) plus
  `check-no-new-gd.sh`. `just godot-check` adds `dotnet test` and
  headless import/export. `godot-check` stays off default `check`
  until `godot_bin` and `godot_export_preset` are real. Co-location,
  composition, typed `.tres`, and migrate-together stay mentions.
  Map: `.dev/kit/godot/README.md`.

### Breaking

- **Codex, OpenCode and Antigravity are no longer targets.** The installer emits
  for Claude Code and Cursor only (`--agent claude,cursor`; default: both).
  `--agent codex` / `opencode` / `antigravity` fails. An old lock that still lists
  them is rewritten on `add`/`update`; leftover `.dev/rules/`, `.opencode/`,
  `.agents/rules/` and the managed `AGENTS.md` block are purged. `doctor` fails
  until that `update` has run. Nested `CLAUDE.md` / `AGENTS.md` is now possible
  (the Codex-concatenates / OpenCode-replaces conflict is gone) — the installer
  does not emit those files yet.

- **`/rust-add-domain` is gone.** It was a transformation recipe for one AppState DI
  scaffold, not a reusable methodology — the `hexagonal` profile keeps the rules
  (`rust-di-container.md` and friends), not the skill. After `update`, delete the
  leftover copy (`doctor` will flag it): `.claude/skills/rust-add-domain`,
  `.agents/skills/rust-add-domain`, `.opencode/skills/rust-add-domain`.

- **The kit has one home: `.dev/kit/`, for every agent.** It was `.claude/kit/` for
  Claude and `.dev/kit/` for the other four. The kit is the only asset kind no emitter
  transforms — executable gates, copied verbatim — and nothing reads it automatically
  the way Claude reads `.claude/rules/`: its only consumers name the path themselves
  (the justfile, `lefthook.yml`, `settings.json`). So the per-agent destination bought
  nothing and cost two things: a second identical tree in a multi-agent install, and a
  justfile whose import path depended on which agent happened to be first in the lock.

  **Migration** — `add`/`update` move it for you and purge the old tree (by name, one
  registry-known directory at a time; anything you put under `.claude/kit/` yourself
  survives). What they cannot rewrite is the paths you wired by hand:

  ```bash
  npx github:dohrm/claude-rules update
  grep -rn '\.claude/kit' justfile lefthook.yml .claude/settings.json .github .gitea 2>/dev/null
  ```

  Anything that turns up becomes `.dev/kit/…`. `claude-rules doctor` reports a wired
  guard whose script is not on disk, so a missed path shows up as a problem rather
  than as a hook that silently never fires.

- **The gates are a `just` library you import, not a snippet you merge.**
  `kit/common/justfile.snippet` is gone. In its place: `kit/common/gate.just` plus one
  file per technology (`kit/rust/rust.just`, `ts`, `go`, `python`, `godot/godot.just`),
  which the repo's root justfile `import`s. The reason is the one that mattered all
  along — a merged snippet could never be updated again, so every fix stayed upstream
  and every installed repo drifted from the day it was installed. An import can be
  updated. Needs **just >= 1.27** (`import` landed in 1.18; the duplicate-override
  settings in 1.27).

  What the root justfile holds now is the *composition*: the imports, the `*_dir`
  variables, `check`, `mutate-diff`, and any recipe or variable it overrides — a
  definition there wins over the imported one, which is how you adapt a gate without
  forking it. `claude-rules init` writes that file; it is ~40 lines instead of ~260.

  Two wiring steps disappear with it. The gate scripts (`adr-check.mjs`,
  `docs-check.mjs`, `review-guard.mjs`, `worktree-status.mjs`, `review-prompt.md`) are
  no longer moved into `scripts/` — the recipes call them where they ship, so an
  update refreshes gate and implementation together. And enabling an opt-in gate is
  now one word in the `check` recipe.

  **Migration** — the installer reports, it never rewrites your justfile:

  ```bash
  npx github:dohrm/claude-rules update && npx github:dohrm/claude-rules init
  ```

  `init` prints the `import` lines and the two `set allow-duplicate-*` lines to add;
  `doctor` warns for as long as a justfile redefines a library recipe instead of
  importing it, naming which ones. The full procedure — including the deterministic
  equivalence proof, since `just --summary` and `just --evaluate` flatten imports
  while `just --dump` does not — is in `.dev/kit/common/README.md`.

  Both guards now treat `*.just` as gate-layer files: the recipes moved, so guarding
  only the file holding the `import` lines would have left the commands unguarded.

- **`portal-flat` no longer ships the HTTP transport — add `portal-http` for it.**
  The profile bundled two independent axes: the flat-domain *architecture* (module
  map, layer boundaries, business boundary) and the *HTTP idioms* (OpenAPI codegen,
  TanStack Query, cache policy). `tauri` needs the first and explicitly forbids the
  second, so a Tauri repo was loading 12.9 KB of rules prescribing TanStack Query on
  its Zustand stores.

  **Migration** — a web portal adds one profile; nothing else changes:

  ```bash
  npx github:dohrm/claude-rules add portal-http        # same globs, same --module anchoring
  ```

  `rules/portal-flat/react.md` → `rules/portal-http/react.md`, and
  `kit/portal-flat/openapi-ts.config.ts` → `kit/portal-http/`. A Tauri app installs
  `portal-flat` + `tauri` and **never** `portal-http`.

### Changed

- **Always-on rules are shorter.** `agent/autonomy.md` is a procedure, not an
  essay. `agent/guardrails.md` and the review contract now treat a comment-only
  hunk as a failed edit: rewrite a comment only when the code change made it
  false.
- **Skill prompts: less restatement, same contracts.** Descriptions dropped the
  8-synonym trigger lists. `/prd` description no longer names a stale 8-section
  template. Skills defer to `product/documents.md` / `agent/decision-records.md`
  instead of reciting budgets. `/experience` dropped the DX/Web/CLI knowledge
  book (anti-slop stays). `/investigate` is agent-agnostic and may instrument
  with a debugger or a failing test, not only tagged logs.

### Added

- **`portal-http/state.md` — server-state ownership and cache-clean policy.** The
  generated TanStack Query code invalidates nothing: the plugin emits option
  factories (`xxxOptions()`, `xxxMutation()`, `xxxQueryKey()`), and the generated
  mutation is a bare `mutationFn`. The rule states who owns server state, where the
  invalidation set is written (`features/{domain}/api/`, not the component), what a
  logout must clear, and what the browser may not recompute.
- **`backend/api-surface.md` — the API models the domain, not the screens.** An admin
  screen and a consultation screen touching the same data are one route and two
  authorizations, not two routes. Includes the "name the difference" table
  (who / which fields / which subset / a real domain operation) and the line between
  a legitimate domain command and a screen-shaped endpoint.
- **`backend/authorization.md` — habilitation, which no rule covered.** Deny by
  default, one permission model in the domain's vocabulary, row-level scope applied
  in the query (never post-filtered — it breaks `total` and pagination), 403 vs 404,
  and the policy/implementation split that `hexagonal` implies.

### Fixed

- **`rules/portal-flat/react.md` claimed generated mutation hooks invalidate related
  queries "by convention". They do not** — verified against two independently
  generated clients: zero occurrences of `invalidate` or `queryClient` in the emitted
  code. The rule promised a behaviour that does not exist, which is worse than saying
  nothing. Corrected, along with the same wrong vocabulary in the plugin table, the
  code example, and the `openapi-ts.config.ts` header comment.
- **`rules/tauri/app.md` was scoped `**/*.rs` while being entirely TypeScript.** It
  loaded on the Rust side, where none of it applies, and never on the stores and IPC
  wrappers it governs. Now scoped `**/*.ts`/`**/*.tsx`. Its state table also moved
  server state from `features/{domain}/logic/` to `features/{domain}/api/`, so a
  Tauri app and a web portal share one module map instead of contradicting it.
- **`features/{domain}/logic/` was chartered as "validation, state machines, derived
  values"** — an invitation to put business rules in the browser. It is now screen
  behaviour only: UI state machines, display derivations, form drafts. A UI
  approximation the product asked for is allowed, labelled as an estimate.

### Fixed

- **`just code-review` could not run, and the gate it feeds could lock a repo out of
  pushing.** Everything here was found by wiring the review into a real repo and using
  it; each one is one line of evidence, not a review of the code.

  - **The prompt never reached the CLI.** It was passed as a trailing positional after
    `--allowedTools`, which is variadic — so the prompt's words were eaten as tool
    rules and the run died with "Input must be provided either through stdin or as a
    prompt argument". It goes in on stdin now.
  - **A failed review blocked every push.** `>` truncates the report before the CLI
    runs, so a CLI that dies (absent, no key, rate-limited, Ctrl-C) left a 0-byte file
    — which `review-guard` read as *malformed*, not *absent*. The hook has no glob, so
    that state blocked every push on the machine, and the documented "no report →
    passes" escape was unreachable because the file existed. The report is now written
    to a temp file and moved into place, and an empty report reads as "not run".
  - **A report that QUOTES the verdict markers was read as malformed.** Any review
    whose fix suggestion shows the output contract — which is exactly what happens when
    the diff touches `review-prompt.md` or the reviewer agent — emitted a second
    `CI_VERDICT` line and was rejected for having two verdicts, reproducibly. The
    markers are now read LAST-wins over the report; earlier ones are prose about the
    contract.
  - **The "read-only reviewer" was not read-only.** `--allowedTools` ADDS to the rules
    the subprocess inherits from `settings.json` and `settings.local.json`; it does not
    replace them. Measured in a worked-in repo: with `--allowedTools 'Read,Glob,Grep'`
    the reviewer ran `just --version`. Denied `Bash`, it reached for `Monitor` — which
    runs a command — and said so in its answer. The recipe now hands the reviewer its
    diff and its sha, so it needs no shell at all, and denies the enumerated executor
    surface. **This is a denylist over a surface a CLI release can grow** — the recipe
    carries the re-enumeration command and says so; the structural half is what holds.

- **`bash-guard` blocked five commands an agent runs on an ordinary day**, and two of
  them were denies, where the only move left is to escalate over a non-issue:
  `git merge --no-verify-signatures` (a GPG flag, nothing to do with hooks),
  `git push --force-with-lease origin feat/fix-main-nav` (the trunk is a whole ref, not
  the word "main" inside a branch name), `cat justfile 2>/dev/null` (an fd redirect is
  not a write to what is being read), `sed -n '1,20p' lefthook.yml` (`sed` writes only
  with `-i` — and the same bug denied reading the review report, so an agent could not
  look at the verdict blocking its push), and `git config --get core.hooksPath` (the
  diagnostic the hooks README tells you to run). The write test is now correlated with
  the gate file instead of matched independently over the whole command.

- **Tightening those patterns opened three ways past the guard's own rules** — found by
  measuring the guard against the commands it exists to refuse, not by reading it. A ref
  and a flag do not END on `(\s|$)`: `;`, `&` and `|` are none of the alternatives, so
  one trailing separator was enough. `git commit --no-verify; git push` and
  `git push -f origin main; echo ok` were both allowed, where the looser patterns before
  them denied both. And the lookbehind meant to spare `2>&1` sat BEFORE the `>`, which is
  exactly where an fd number goes — so `1>`, `2>` and `&>` were exempted while each of
  them truncates a real file on open, and `echo x 2> .git/hooks/pre-commit` emptied a
  hook silently. Both are anchored on what cannot FOLLOW the token now, which still
  refuses to see the trunk in `feat/fix-main-nav`.
- **Half the mutating commands could empty `.git/hooks/` without a prompt.** The
  `.git/hooks/` rule carried its own short list (`rm`, `mv`, `truncate`, `dd`, `sed -i`,
  `chmod -x`) while the gate-file rule thirty lines below also listed `cp`, `tee`, `ln`,
  `install`, `chown` and `chmod` — so `cp /dev/null`, `ln -sf /bin/true`, `install
  -m755` and `chmod 000` all went through, each one neutering the git floor. One list
  feeds both rules now, plus `sed --in-place`. Readers (`cat`, `grep`, `sed -n`) stay
  out of it, on purpose.
- **Editing a document and then running the gate that checks it escalated.**
  `just check > /dev/null 2>&1; node scripts/docs-check.mjs` holds a gate filename and a
  redirect for entirely unrelated reasons — it RUNS the gate, it does not write it — and
  independent matching asked for confirmation on it. That is a prompt on every
  documentation change, interrupting exactly the loop `rules/agent/autonomy.md` asks an
  agent to close by itself.
- **`review-guard` read a fixed 6-line tail, which blocked ordinary reports and missed
  the case it was added for.** A review that signs off with a few lines of prose pushed
  its own markers out of the window, and the push was refused as *malformed*; a contract
  quoted next to the real markers stayed inside the window and still counted as a second
  verdict. The report contract already promises the reviewer that only the last marker
  counts, so that is what runs: the LAST of each marker, over the whole report. Several
  markers is no longer a malformation; a last one that is not a verdict still is.
- **The temp report was named `review-report.md.part`**, which contains
  `review-report.md` — so `bash-guard` denied the recipe's own two lines to anyone
  replaying them by hand to debug a review, blocked by the forge-the-report rule while
  doing the opposite of forging. It is `.work/review.tmp` now. A dead CLI still leaves
  the previous report standing, deliberately: deleting it first would let a rate-limited
  review clear a `CRITICAL`, and the signal that nothing ran is the recipe's exit code.
- **`kit/common/hooks/README.md` described guards that had changed under it** —
  `edit-guard` documented as "always ask, never deny" after two paths started denying,
  and a deny list still advertising bare `sed` while omitting `.git/hooks/`. That file
  ships into consuming repos as the explanation of the layer.

### Added

- **The git hook files are part of the gate layer.** `lefthook uninstall` was covered;
  emptying the files it installs was not — and that is both the cheaper move and the
  one git cannot see. `bash-guard` denies `rm`/`sed -i`/redirects aimed at
  `.git/hooks/`, and `edit-guard` denies writing one.
- **`edit-guard` denies what has no legitimate hand-edit.** It was an ask-only guard,
  so `Write` to `.work/review-report.md` escalated while `bash-guard` denied the shell
  spelling of the same act — the hardest rule in the doctrine enforced by the weaker
  half. The report and `.git/hooks/` now deny; the ask list keeps the files whose edit
  is sometimes the task (justfile, workflows, baselines).

### Changed

- **The review prompt's sha instruction moved out of the shared contract block.** The
  headless reviewer has no shell and is handed `{{sha}}`; the subagent runs
  `git rev-parse HEAD` itself. Keeping both inside the block would have made the two
  copies drift, which is the one thing `registry.test.mjs` exists to prevent — so the
  sourcing is per-variant and the block stays byte-identical.

- **BREAKING — `/loop-setup` keeps its state in one file under `.work/`.** It used to
  write `PLAN.md` and `MEMORY.md` at the repo root; it now writes a single
  `.work/loop.md` (bounded objective, guardrails, remaining work, log, escalations).
  The old name collided head-on with the durable `docs/PLAN.md` that `/plan` owns —
  two files, one name, opposite lifetimes: one is a promise kept for the life of the
  project, the other is scaffolding that dies with the branch.

  **Migration** — only if a loop is in flight. Nothing breaks silently: the skill
  simply will not find the old files, and a restarted loop would begin with an empty
  log, losing the dead ends it must not retry.

  1. Create `.work/loop.md` from the skill's template.
  2. Move `PLAN.md`'s checklist under `## Remaining work`, `MEMORY.md`'s entries under
     `## Log`, and anything that was waiting on you under `## Blocked on the human`.
  3. Delete the two old files and add `.work/` to `.gitignore`.

  With no loop running, delete the two files and move on.

### Added

- **The gate layer got teeth, in two layers** (shared `kit/common`). `rules/agent/autonomy.md`
  said in prose what an agent must never do; now something enforces it.

  **The git floor** — `common/lefthook.snippet.yml` gains a pre-commit
  `no-commit-on-trunk` (a no-op off `main`/`master` via `only: ref`). Portable across
  every agent and CI, which is why the guarantee lives here. `claude-rules init` now
  writes it into a generated `lefthook.yml` — and writes that file even when no
  language is locked, since the floor holds in a docs-only repo too. **Opt-out:**
  a solo repo that really commits on `main` deletes the one command; the snippet says so.

  **The harness layer** — new `common/hooks/`, opt-in, one wiring snippet per tool.
  `bash-guard.mjs` **denies** `--no-verify`, `git commit -n`, `core.hooksPath`,
  `lefthook uninstall`, `LEFTHOOK=0`/`SKIP=`, force-pushing or deleting the trunk, and
  any `rm`/`sed`/redirect aimed at `.work/review-report.md`; it **asks** when a command
  writes to the gate layer itself. `edit-guard.mjs` always asks, never denies —
  `/ci-setup`'s whole job is editing a workflow. Merge
  `settings.snippet.json` into `.claude/settings.json` (the installer still never
  writes it), or the `opencode`/`cursor`/`codex` snippet beside it.

  **`doctor` audits the wiring.** New `Gate layer` section: it **fails** on a
  `lefthook.yml` git was never told about (`lefthook install` was never run — the file
  looks installed and every hook in it is inert) and on a hook wired to a guard script
  that is not on disk; it **warns** on a host config that is not valid JSON; and it
  reports an unwired harness layer as a **notice that never scores**, because opt-in
  absence is a decision, not drift. Per agent, so it names `.dev/kit` for Cursor and
  `.claude/kit` for Claude, and says plainly that Codex has no hooks and Antigravity
  no mechanism.

  **What it is not:** both guards fail open, `settings.json` is not fully
  self-protecting, Cursor has no pre-edit hook, Codex has no hooks at all (a sandbox
  is a different promise), and Antigravity degrades to the git floor. This makes drift
  expensive and loud, never impossible — impossibility is branch protection.
  `common/hooks/README.md` is the honest version.

- **`just code-review` + `just review-guard`** (shared `kit/common`) — a local code
  review with teeth. `just code-review` runs a reviewer as its own **read-only**
  process over `git diff <base>...HEAD` (any agent CLI — set `review_cmd`: `claude -p`,
  `codex exec`, `opencode run`, `cursor-agent -p`) and parks its report in
  `.work/review-report.md`. The report ends with two machine-readable markers,
  `CI_VERDICT` and `REVIEWED: <sha>`, and `just review-guard` — pure Node, no LLM,
  milliseconds, so it can be a **pre-push hook** — reads them: a `CRITICAL` blocks the
  push **whatever sha it was written against**, a malformed report blocks, an absent
  one passes with a "not run" notice (declared, never simulated), and a stale
  `CLEAN`/`WARNINGS` passes with a notice.

  The hole this closes: a verdict that expires with HEAD is a verdict you defeat by
  committing once more. The only way out of a `CRITICAL` is a fix and a NEW review —
  and hand-editing or deleting the report is now a hard "never fake green" bypass
  (`rules/agent/autonomy.md`, where the review joins `mutate-diff` at the second
  speed of the loop).

  **Wiring** (opt-in, like `adr-check`): move `review-guard.mjs` **and**
  `review-prompt.md` to `scripts/`, merge `common/lefthook.snippet.yml` (pre-push,
  no glob) into `lefthook.yml`, gitignore `.work/`. The prompt is the same review
  contract as the `code-reviewer` subagent — one block, duplicated on purpose,
  test-enforced identical.

- **`/tasks`** (profile `product`) — the missing step between a plan and code. It takes
  **one** phase of `docs/PLAN.md` and cuts it into tasks an agent loop can execute:
  the anchors in your existing code (where each layer attaches, and the nearest
  neighbour to imitate), a first task that freezes the phase's contract **as code**,
  then tasks sized to the *green boundary* — the smallest change that leaves the gate
  green, which is what makes one-commit-per-task true and the branch bisectable.

  Output is `.work/phase-NN-<slug>.md`: gitignored working memory, never a document
  under `docs/`. The phase's promise stays in `docs/plan/`, the account of what
  happened is the git log. Run it when a phase *starts*, not once for every phase —
  the code will have moved.

  `/loop-setup` composes with it: given a phase worklist it adds only its guardrails
  section to that file rather than writing a second plan.
