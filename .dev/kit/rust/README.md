# kit/rust — the Rust validation chain

This is the **jalon**: a language module whose toolchain already sees
format, lint, types, tests, supply-chain, unused deps, and (opt-in)
mutation. The recipes own the commands. The files below are the configs
those commands read. The installer copies this directory to `.dev/kit/rust/`
and **never merges** the configs into the workspace — you copy them once.

`claude-rules init` writes `import '.dev/kit/rust/rust.just'` and derives
`rust_dir` from the lock. Lefthook is a thin trigger: merge
`lefthook.snippet.yml` so pre-commit runs `just rust-lint` and pre-push
runs `just rust-check`.

## The chain

| Recipe | Tier | When | What it runs |
|---|---|---|---|
| `just rust-lint` | 1 | pre-commit | `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used` |
| `just rust-check` | 2 | pre-push, `just check` | rust-lint · `cargo test --workspace` · `cargo deny check licenses advisories sources` · `cargo machete --skip-target-dir` |
| `just rust-mutate` | 3 | coherent block, never a hook | `cargo mutants --in-diff -j {{mutate_jobs}} {{mutate_args}}` against `{{base}}...HEAD` |

`cargo build` is not a separate line: clippy and `cargo test` already
compile. `unwrap` / `expect` are denied on lib and bins only — tests stay
free. That is why the second clippy pass is `--lib --bins`, not
`--all-targets`.

## Requirements

| Tool | macOS | Linux | Windows |
|---|---|---|---|
| Rust toolchain | `brew install rustup-init && rustup-init` | official script: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` | `winget install Rustlang.Rustup` |

Once `rustup`/`cargo` are on the `PATH`, the rest is one command everywhere:

```bash
rustup component add rustfmt clippy
cargo install cargo-deny cargo-machete
cargo install cargo-mutants   # Tier 3 only
```

Optional, and every one of them is about Tier 3 being fast enough to stay in
the loop — read **Tier 3 economics** below before installing any of them:

```bash
cargo install sccache          # mutualizes the dependency build across trees
cargo install cargo-nextest    # per-test timeouts, faster startup
apt install mold               # or: brew install mold
```

## Configs — copy once, then they are yours

| File | Destination | Read by | Adapt |
|---|---|---|---|
| `rustfmt.toml` | `<rust_dir>/rustfmt.toml` | `cargo fmt` (rust-lint) | `edition` |
| `deny.toml` | `<rust_dir>/deny.toml` | `cargo deny` (rust-check) | `ignore`, private registry, license allow-list |
| `mutants.toml` | `<rust_dir>/.cargo/mutants.toml` | `cargo mutants` (rust-mutate) | `exclude_re`, `exclude_globs` — generated code ships pre-filled |
| `cargo-profile.snippet.toml` | merge into `<rust_dir>/Cargo.toml` | `cargo mutants --profile mutants` | nothing; opt in via `mutate_args` |
| `cargo-config.snippet.toml` | merge into `<rust_dir>/.cargo/config.toml` | every `cargo` invocation | pick your linker target; sccache as-is |
| `lefthook.snippet.yml` | merge into root `lefthook.yml` | lefthook | nothing if `just rust-*` exists |
| `mutation-ci.yaml` | `.gitea/workflows/` or `.github/workflows/` | CI — **the gate** | runner, `working-directory` |

`rust-fmt.sh` is a **special case**: only when a generated member crate
must be skipped (`cargo fmt --all` would fight the generator). Swap the
fmt line in the root justfile and add `#![allow(clippy::all)]` on that
crate. Most repos never need it.

## Tier 3 economics — what actually costs the time

Measured on one real workspace, because the intuition is wrong in a way that
changes which levers are worth pulling:

| Phase | Cost | Reading |
|---|---|---|
| Baseline build | 972 s | cargo-mutants copies the tree to a scratch dir and builds it **cold** — your warm `target/` buys nothing |
| Baseline test | 363 s | paid again **in full by every surviving mutant** |
| Per mutant | ~60 s | an incremental **rebuild**, not test execution |
| A 32-mutant sprint diff | ~50 min | of which 22 min is the baseline, before the first mutant runs |

Two consequences worth stating out loud:

- **Mutation in Rust is compile-bound.** Shrinking the mutant count or the test
  suite helps far less than linking and codegen do. The unit of recompilation is
  the *crate*, which makes "keep the domain in a small pure crate" a performance
  rule as well as an architecture one — the hexagonal rule pays here too.
- **A killed mutant exits on the first red test; a survivor pays the whole
  suite.** So the cost spikes exactly when the gate has something to say. A gate
  that gets slower the more it finds is a gate on its way out
  (`rules/testing/ratchet.md`).

Levers, by yield:

| Lever | Term it attacks | Where |
|---|---|---|
| `--in-place` in a dedicated worktree + `--baseline=skip` | the 1335 s baseline | `mutate_args` |
| mold + `[profile.mutants] debug = "none"` | the per-mutant rebuild | the two snippets |
| `-j` | the mutant phase (multiplies disk too) | `mutate_jobs` |
| sccache | cold dependency builds in fresh trees | `cargo-config.snippet.toml` |
| generated-code exclusions | the variance, not the average | `mutants.toml` |

`--baseline=skip` is **fail-open on its own**: against a red tree every mutant
"fails" and so reads as killed. It is safe only when something just proved the
tree green *in that directory* — a property an orchestrator can guarantee and a
human's memory cannot. Leave it off until you have one.

### Measure it, do not believe it

cargo-mutants already recorded the answer, so there is nothing to re-run and no
need to time your suite while you are working in the tree:

```bash
jq '.outcomes[0]' mutants.out/outcomes.json   # inspect the shape; it moves by version
```

Then aggregate `Build` against `Test` duration per phase. If build dominates, the
two snippets are the whole subject. Record before and after: a claim about a
gate's cost is a measurement, not a belief.

## What this chain cannot see

Match `/_`, function size, serde, I/O timeouts, thiserror placement,
tracing fields, UTF-8 indexing. Those stay in `rules/rust/` as a mention
for the reviewer. Do not invent a fourth clippy pass to "translate" them.
