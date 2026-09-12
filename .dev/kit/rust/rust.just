# Rust gates — a just LIBRARY, imported by the repo's root justfile:
#
#     import '.dev/kit/rust/rust.just'
#
# `claude-rules init` writes that line for you, and `claude-rules update` refreshes
# THIS file — so a fix here reaches a repo that installed months ago. Do not edit it:
# override instead. A recipe or variable in the root justfile wins over an imported
# one, which is what the `set allow-duplicate-*` lines at the top of that file buy.
#
# Recipes run with the working directory of the ROOT justfile, never of this file, so
# every path below is repo-relative.

# Where the Rust workspace lives. `init` derives it from the lock's `modules` into the
# root justfile's managed block, which overrides this default.
rust_dir := "."

# Merge-base for Tier 3. kit/common/gate.just defines the same default when `agent
# --level gates` is installed; allow-duplicate-variables on the root justfile
# lets either win. Defined here so `just rust-mutate` parses without common.
base := "origin/main"

# Each line runs in its own shell, so `cd {{dir}} &&` per line (portable: works
# in sh/cmd/pwsh). Split: *-lint is the fast pre-commit tier, *-check the full.
#
# Configs the commands read — copy from this directory, the installer never merges
# them: rustfmt.toml → {{rust_dir}}/, deny.toml → {{rust_dir}}/, mutants.toml →
# {{rust_dir}}/.cargo/. See README.md.
#
# unwrap/expect: a second clippy pass on --lib --bins only. --all-targets would
# deny them in tests; the crate-attribute form in the old rule is no longer needed.
rust-lint:
    cd {{rust_dir}} && cargo fmt --all --check
    cd {{rust_dir}} && cargo clippy --workspace --all-targets -- -D warnings
    cd {{rust_dir}} && cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used
rust-check: rust-lint
    cd {{rust_dir}} && cargo test --workspace
    cd {{rust_dir}} && cargo deny check licenses advisories sources
    cd {{rust_dir}} && cargo machete --skip-target-dir

# ── Tier 3 economics ───────────────────────────────────────────────────────
# Measured on one real workspace, and the numbers decide which levers are worth
# having: a 32-mutant diff took ~50 minutes, of which **22 were the BASELINE** —
# 972 s of build plus 363 s of test BEFORE the first mutant. cargo-mutants copies
# the tree into a scratch directory and builds it cold, so a warm `target/` in the
# checkout buys nothing. The remaining ~60 s per mutant is an incremental REBUILD,
# not test execution.
#
# Two consequences, both counter-intuitive enough to write down:
#
#   • Mutation in Rust is COMPILE-bound. Shrinking the mutant count (exclusions) or
#     the suite (test selection) helps far less than linking and codegen do. The
#     unit of recompilation is the CRATE — which makes "keep the domain in a small
#     pure crate" a performance rule as well as an architecture one.
#   • A KILLED mutant exits on the first red test; a SURVIVOR pays the whole suite.
#     So the cost spikes exactly when the gate has something to say. Watch that,
#     because a gate that gets slower the more it finds is a gate on its way out
#     (rules/testing/ratchet.md).
#
# `mutate_jobs` matches the `-j 2` mutation-ci.yaml already runs with. Each job gets
# its OWN copy of the build directory, so raising it multiplies DISK, not just CPU:
# a 10 GB target dir at `-j 8` is 80 GB mid-run. Raise it in the root justfile once
# you know what your workspace weighs.
mutate_jobs := "2"

# Everything else is ONE escape hatch rather than five knobs, because every useful
# flag here needs something copied or installed first, and an `update` must never
# change what a repo that installed months ago actually runs. All off by default:
#
#   --profile mutants     needs cargo-profile.snippet.toml merged into Cargo.toml.
#                         `debug = "none"`: debuginfo is expensive to generate and
#                         nobody reads a mutant's.
#   --test-tool=nextest   needs cargo-nextest. Faster startup and per-test timeouts,
#                         so a mutant that hangs dies before `minimum_test_timeout`.
#   --baseline=skip       removes the 1335 s baseline — and is FAIL-OPEN alone: if
#                         the tree is really red, every mutant "fails" and so reads
#                         as killed. Pass it only when something just proved the tree
#                         green in THIS directory (an orchestrator that ran
#                         `just check`, never a human's memory of having run it).
#   --in-place            mutates the real tree instead of copying it, so the warm
#                         target dir applies and the 972 s cold build disappears. An
#                         interrupted run leaves the tree mutated: never in your main
#                         checkout, fine in a dedicated mutation worktree.
#
# It interpolates RAW into the command line, the same convention as `review_exclude`
# in kit/common/gate.just: one flag per word, space-separated, quote anything that
# contains a shell metacharacter yourself.
mutate_args := ""

# `git diff <base>...HEAD` = changes since the merge-base, i.e. the same set the
# PR job computes. `--relative` inside {{rust_dir}} rewrites the paths so they
# are workspace-relative, which is what cargo-mutants expects when the Rust
# workspace is NOT at the repo root. Needs: cargo install cargo-mutants.
#
# `from` is where that range STARTS, and it defaults to the whole branch — the
# behaviour this recipe always had, and the only one available to a repo that
# installed rust without kit/common. Pass it the commit mutation last cleared to
# re-mutate only what is new: `just rust-mutate "$(just mutate-from)"`, which is
# what common/gate.just's `mutate-from` prints. Mutants cost minutes each, so on a
# branch built by successive loops that argument is most of the runtime.
rust-mutate from=base:
    cd {{rust_dir}} && git diff {{from}}...HEAD --relative -- . > pr.diff
    cd {{rust_dir}} && cargo mutants --in-diff pr.diff --no-shuffle -j {{mutate_jobs}} {{mutate_args}}
