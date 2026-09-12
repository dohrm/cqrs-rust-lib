#!/usr/bin/env bash
# SPECIAL CASE — most repos do NOT need this. The portable default fmt gate is
# plain `cargo fmt --all --check` (see lefthook.snippet.yml). Use this script
# ONLY when the workspace has a generated crate that is a path-dep of a member
# (e.g. an OpenAPI client): `cargo fmt --all` would reformat it (churn that
# fights the generator). `rustfmt.toml ignore` needs nightly; `[workspace]
# exclude` can't drop a path-dep of a member. So we enumerate the real crates
# and skip the excluded ones (auto-adapts via cargo metadata).
#
# ⚠️ PORTABILITY: this is bash + python (mac/linux). It is the one non-Windows
# piece of the kit. If a repo needs BOTH generated-crate exclusion AND Windows
# dev, port this logic to a cross-platform runner (a `just` recipe with a
# `#!/usr/bin/env node` shebang, or a subcommand of the npx installer). Until a
# repo actually hits that intersection, don't build it.
#
# Usage: scripts/rust-fmt.sh [--check]   (no arg = format in place)
set -euo pipefail

# ── ADAPT PER REPO ──────────────────────────────────────────────────────────
# Workspace root, relative to this script. Default assumes scripts/ at repo root
# and a single `api/` workspace; change if your layout differs.
WORKSPACE_DIR="${RUST_WORKSPACE_DIR:-../api}"
# Package names to NEVER format (generated code). Empty = format everything.
EXCLUDE=(gitea-generated)
# ────────────────────────────────────────────────────────────────────────────

cd "$(dirname "$0")/$WORKSPACE_DIR"

# Build a python set literal of excluded names for the filter below.
excl_py="{$(printf '"%s",' "${EXCLUDE[@]}")}"

# Space-separated "-p name -p name ..." for every member except the excluded
# ones. Crate names are [a-z0-9-], so the word-splitting below is safe.
# (No `mapfile`: it is bash 4+, absent from the bash 3.2 shipped on macOS.)
pkgs="$(cargo metadata --no-deps --format-version 1 \
  | python3 -c "import json,sys; excl=$excl_py; print(' '.join('-p '+p['name'] for p in json.load(sys.stdin)['packages'] if p['name'] not in excl))")"

# shellcheck disable=SC2086  # intentional word-splitting of the -p flags
exec cargo fmt $pkgs "$@"
