# The COMPOSITION half of the gate layer. The recipes themselves are a `just`
# library under .dev/kit/, imported below and refreshed by `claude-rules update`,
# so an upstream fix reaches this repo instead of staying upstream. What lives
# HERE is what only this repo knows: where the code is, what `check` runs, the
# test databases, and the two recipes that diverge on purpose.
#
# Anything defined here WINS over the imported version — that is what the two
# `set allow-duplicate-*` lines buy. Never edit a `.just` under .dev/kit/: an
# update overwrites it. Doctrine: .dev/kit/common/README.md.
#
# `just` >= 1.27 (install: cargo/brew/scoop install just) + Node >= 18 for the
# Node gates (adr-check, docs-check, review-guard, diff-since).

set allow-duplicate-recipes := true
set allow-duplicate-variables := true

import '.dev/kit/common/gate.just'
import '.dev/kit/rust/rust.just'

# Between the markers: `claude-rules init` derives these from the `modules` map in
# .claude-rules.lock, so the layout has ONE home. Edit them by hand if you prefer —
# but the next `init` rewrites the block, and nothing outside it is ever touched.
# claude-rules:start (managed — derived from .claude-rules.lock)
rust_dir   := "."    # e.g. "api"                       for a backend under api/
# claude-rules:end

# Full local self-verify (Tiers 1-2) — the command an agent closes its loop on
# before handing back. Tier 3 is deliberately NOT in here, and the two halves of it
# now live in different places: `just code-review` runs per sprint before the push,
# mutation is a gate on the PR (.github/workflows/mutation.yml). Doctrine:
# rules/agent/autonomy.md.
#
# adr-check and docs-check joined the gate with docs/adr/0001: a decision record
# whose shape nothing verifies is a record that drifts. Both are no-ops when their
# directory is absent, and both are Node, so `check` stays cross-platform.
check: rust-check adr-check docs-check

# ── Rust ─────────────────────────────────────────────────────────────────
# OVERRIDE of .dev/kit/rust/rust.just, and it is measured, not preference: the
# library lints ONE feature set, and this workspace has two that do not share a
# build. The two clippy lines cover both sets .github/workflows/ci.yml's lint job
# builds, plus the examples (CI lints the root package only). Both are needed —
# `--all-features` unifies the features across the workspace, and the examples pull
# in postgres+mongodb+surrealdb, so a helper used only by a feature-gated backend
# looks alive under a workspace lint and is `dead_code` in the narrower set. Two
# such functions passed `just check` and failed CI's legacy-path clippy.
#
# The lint lists are the library's, kept verbatim. The third line is its production
# pass (unwrap/expect/panic/string_slice), kept verbatim except for
# `--all-features` (same reason: the feature-gated backends are most of the I/O
# code, and they are where a lock or an Option is unwrapped). `--lib --bins`, never
# `--all-targets`: tests stay free to unwrap.
#
# The cost model, measured on an M-series Mac, because it is not the usual one: the
# four feature sets here (three clippy passes + `cargo test --workspace`) do not share
# artifacts, so ONE source edit is four rebuilds of the workspace crates — 70s + 157s +
# 51s + 133s, i.e. 4 to 7 min for `just check` depending on load. Re-run it without
# touching anything and it is ~20s, all four no-ops. So the gate is cheap to repeat and
# expensive per edit, which is the opposite of the "seconds" Tier 1-2 assumes, and it
# is the price of one command everywhere instead of two that drift. Drop a clippy line
# and CI's lint job is the only thing left watching that feature set. lefthook's
# pre-commit runs this recipe now, so a commit pays the edit price once.
rust-lint:
    cd {{rust_dir}} && cargo fmt --all --check
    cd {{rust_dir}} && cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::dbg_macro -D clippy::todo -D clippy::unimplemented -D clippy::undocumented_unsafe_blocks
    cd {{rust_dir}} && cargo clippy --features rest --all-targets -- -D warnings -D clippy::dbg_macro -D clippy::todo -D clippy::unimplemented -D clippy::undocumented_unsafe_blocks
    cd {{rust_dir}} && cargo clippy --workspace --lib --bins --all-features -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::string_slice

# ── Tier 3 review ──────────────────────────────────────────────────────────
# OVERRIDE of .dev/kit/common/gate.just: `claude -p` inherits the user's `language`
# setting, and a reviewer answering in French writes `### Fichiers revus`, which
# review-guard (rightly) rejects as a report with no `### Files reviewed`. Flag
# settings outrank user settings, so the report is pinned to English — the language
# of every artifact here (rules/common/language.md). Same command otherwise.
review-claude:
    claude -p --settings '{"language":"english"}' --disallowedTools '{{review_deny}}' --allowedTools 'Read,Glob,Grep' < {{review_in}} > {{review_out}}

# ── Test databases ─────────────────────────────────────────────────────────
# The integration suites and the examples need a real server. `compose.yaml` starts one
# of each on a shifted port, so this stack can never be confused with — or collide with —
# a database already running on the machine. Nothing persists: `db-down` takes the data.
#
# Both suites skip when their variable is unset, so `just check` stays green without any
# of this; `just test-db` is what opts in.
pg_test_uri    := "postgres://cqrs:cqrs@127.0.0.1:55432/cqrs_test"
mongo_test_uri := "mongodb://127.0.0.1:37017/?directConnection=true"
surreal_uri    := "ws://127.0.0.1:18000"

# `directConnection=true` on the Mongo URI is required, not decoration: the event store
# opens a transaction, which needs a replica set, and a single-node set would otherwise
# hand the driver its own announced host to re-dial.
db-up:
    podman compose -f compose.yaml up -d
    @until [ "$(podman ps --filter name=cqrs-rust-lib-test --format '{{{{.Status}}' | grep -c healthy)" -ge 3 ]; do sleep 1; done
    @echo "postgres {{pg_test_uri}}"
    @echo "mongodb  {{mongo_test_uri}}"
    @echo "surreal  {{surreal_uri}}"

db-down:
    podman compose -f compose.yaml down -v

# The suites that need a server. Everything else already runs under `just check`.
#
# The first line is the root package's own integration tests — `tests/snapshot_read_path.rs`
# among them. Without it those files return at their first line under every recipe in this
# justfile, which is not a passing test, it is an absent one.
test-db:
    cd {{rust_dir}} && PG_TEST_URI="{{pg_test_uri}}" MONGODB_TEST_URI="{{mongo_test_uri}}" cargo test --all-features
    cd {{rust_dir}} && PG_TEST_URI="{{pg_test_uri}}" cargo test -p todolist
    cd {{rust_dir}} && MONGODB_TEST_URI="{{mongo_test_uri}}" cargo test -p bank

# ── Tier 3 — do the tests ASSERT, or do they merely execute? ───────────────
# Coverage cannot answer that; mutation can. NOT a step you owe before pushing and
# never a git hook: the gate is .github/workflows/mutation.yml, on the PR, where a
# runner can spend the cores this is compile-bound on. Run this recipe yourself only
# to buy back a round trip on code you already doubt — an optimisation, not a duty.
# Doctrine: rules/agent/autonomy.md, and the cost model in .dev/kit/rust/README.md
# ("Tier 3 economics").
#
# `--profile mutants` is opt-in in the library because it FAILS OUTRIGHT when the
# profile is absent; [profile.mutants] is merged into Cargo.toml here, so it is on.
# The CI job passes the same flag — one profile, both callers.
mutate_args := "--profile mutants"

# This recipe lives here because it names the mutators THIS repo has — Rust only.
# `mutate-from` prints the commit mutation last cleared, so a branch grown over
# several loops re-mutates only what is new; `mutate-mark` runs LAST and only on
# success (just stops at the first failing line), so a surviving mutant leaves the
# marker where it was and the next run re-mutates that block WITH its fix.
mutate-diff:
    just rust-mutate "$(just mutate-from)"
    just mutate-mark
