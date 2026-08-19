# Quality-gate task layer — MERGE into your repo's root justfile (or let
# `claude-rules init` assemble it). This is the ONE place that knows where each
# technology lives: set the `*_dir` variables below. Both the git hooks
# (lefthook) and you/the agent (`just check`) call these recipes, so NO path is
# hardcoded anywhere else in the kit.
#
# `just` is the cross-platform task runner (install: cargo/brew/scoop install just).
# Adapt the dirs to your layout (default: repo root). Enable the techs you have.

# Between the markers: `claude-rules init` derives these from the `modules` map in
# .claude-rules.lock, so the layout has ONE home. Edit them by hand if you prefer —
# but the next `init` rewrites the block, and nothing outside it is ever touched.
# claude-rules:start (managed — derived from .claude-rules.lock)
rust_dir   := "."    # e.g. "api"                       for a backend under api/
ts_dir     := "."    # e.g. "apps/web"
go_dir     := "."    # e.g. "workflows/orchestration"
python_dir := "."    # e.g. "services/ingest"
# claude-rules:end
base     := "origin/main"   # branch a feature is measured against (Tier 3)

# Full local self-verify (Tiers 1-2) across the techs you have — the command an
# agent closes its loop on before handing back, in seconds. Tier 3 (mutation)
# is deliberately NOT in here: it costs minutes, so it runs per coherent block
# (`just mutate-diff`), not per iteration.
# `claude-rules init` derives this line from the locked language profiles when it
# CREATES the justfile — after that the recipe is yours, and init only reports a
# disagreement with the lock. Add the opt-in gates below as you enable them:
check: rust-check
# check: rust-check ts-check go-check python-check adr-check docs-check rules-check

# ── Rust ─────────────────────────────────────────────────────────────────
# Each line runs in its own shell, so `cd {{dir}} &&` per line (portable: works
# in sh/cmd/pwsh). Split: *-lint is the fast pre-commit tier, *-check the full.
rust-lint:
    cd {{rust_dir}} && cargo fmt --all --check
    cd {{rust_dir}} && cargo clippy --workspace --all-targets -- -D warnings
rust-check: rust-lint
    cd {{rust_dir}} && cargo test --workspace
    cd {{rust_dir}} && cargo deny check licenses advisories sources
    cd {{rust_dir}} && cargo machete --skip-target-dir

# ── TypeScript ─────────────────────────────────────────────────────────────
ts-lint:
    cd {{ts_dir}} && npm run lint
ts-check: ts-lint
    cd {{ts_dir}} && npm run test
    cd {{ts_dir}} && npm run build

# ── Go ─────────────────────────────────────────────────────────────────────
go-lint:
    cd {{go_dir}} && golangci-lint run ./...
go-check: go-lint
    cd {{go_dir}} && go test -race ./...
    cd {{go_dir}} && go build ./...
    cd {{go_dir}} && govulncheck ./...

# ── Python ─────────────────────────────────────────────────────────────────
# Every command goes through `uv run`, so the gate runs against the LOCKED
# environment and not whatever is on this machine (rules/python/environment.md).
#
# `--locked` on EVERY line, and it is not decoration: a bare `uv run` silently
# re-locks when pyproject.toml has drifted from uv.lock, so the gate would repair
# the very thing it is supposed to catch and still go green. `--locked` makes the
# drift an error instead. (`uv lock --check` is the same check, standalone.)
#
# `mypy` takes no path: it reads `files` from [tool.mypy] in pyproject.toml, so
# the scope has one home (and `mypy .` would walk .venv). Same for pytest.
# On poetry/pdm, swap the runner and keep the rest — the invariant is the
# lockfile, not the tool.
python-lint:
    cd {{python_dir}} && uv run --locked ruff format --check .
    cd {{python_dir}} && uv run --locked ruff check .
python-check: python-lint
    cd {{python_dir}} && uv run --locked mypy
    cd {{python_dir}} && uv run --locked pytest
    cd {{python_dir}} && uv run --locked pip-audit
    cd {{python_dir}} && uv run --locked deptry .

# ── Tier 3 — do the tests ASSERT, or do they merely execute? ───────────────
# Coverage cannot answer that; mutation can. Minutes, not seconds — so this is
# NEVER a git hook and never part of `check`. Run it when a coherent block is
# finished, BEFORE pushing: the survivors come back while the code is still in
# your head, instead of at PR time. CI runs the same tool on the PR diff as a
# witness (kit/*/mutation-ci.yaml). Doctrine: rules/testing/ratchet.md.
#
# Opt-in per tech — uncomment the ones whose tool is installed. A recipe that
# is absent is a valid answer: the agent reports mutation as not-run rather
# than pretending. Add `pr.diff` and `coverage.out` to .gitignore.
mutate-diff: rust-mutate
# mutate-diff: rust-mutate ts-mutate go-cover python-mutate

# `git diff <base>...HEAD` = changes since the merge-base, i.e. the same set the
# PR job computes. `--relative` inside {{rust_dir}} rewrites the paths so they
# are workspace-relative, which is what cargo-mutants expects when the Rust
# workspace is NOT at the repo root. Needs: cargo install cargo-mutants.
rust-mutate:
    cd {{rust_dir}} && git diff {{base}}...HEAD --relative -- . > pr.diff
    cd {{rust_dir}} && cargo mutants --in-diff pr.diff --no-shuffle

# StrykerJS has no `--since`: it has `--incremental`, which reuses the previous
# report and re-tests only what changed — same outcome, one flag, no file list
# to compute. The first run is a full one; every run after it is minutes.
# Needs: npm i -D @stryker-mutator/core + a runner, then npx stryker init.
ts-mutate:
    cd {{ts_dir}} && npx stryker run --incremental

# Go has no production-grade mutation tool, so this is the weaker signal: run
# the suite with coverage and read which statements in the code you just touched
# are still at 0%. The numeric ratchet against .coverage-baseline stays in CI
# (kit/go/coverage-ci.yaml) — here you want the map, not the verdict.
go-cover:
    cd {{go_dir}} && go test -race -count=1 -covermode=atomic -coverprofile=coverage.out ./...
    cd {{go_dir}} && go tool cover -func=coverage.out

# mutmut has no diff mode (no `--in-diff`, no `--since`) — it is path-scoped. It
# does cache its results, so a re-run only re-tests what actually changed, which
# is the same outcome as Stryker's `--incremental`: first run long, every one
# after it minutes. Scope comes from [tool.mutmut] source_paths in pyproject.toml;
# narrow it with `--paths-to-mutate src/billing` when the package gets big. CI
# scopes to the PR's changed files instead (kit/python/mutation-ci.yaml).
# Needs: uv add --dev mutmut. Gitignore `mutants/`.
python-mutate:
    cd {{python_dir}} && uv run mutmut run
    cd {{python_dir}} && uv run mutmut results

# ── Code review (Tier 3 — judgment, not correctness) ───────────────────────
# Same cadence as mutate-diff: once per coherent block, BEFORE the push. One
# reviewer, one process, one prompt (scripts/review-prompt.md) — any agent CLI runs
# it. stdout IS the report, and the recipe parks it in .work/review-report.md, so
# gitignore `.work/`: a report is working memory, never a committed artifact.
#
# Permissions are the point: a reviewer that can edit the code becomes its author,
# and then nobody reviewed it. Getting that right took two corrections, both measured:
#
#   1. `--allowedTools` ADDS to the permission rules the subprocess inherits from
#      .claude/settings.json and settings.local.json — which in a worked-in repo means
#      a hundred-odd allow rules, `chmod`, `curl` and `just:*` among them. It does not
#      replace them. With `--allowedTools 'Read,Glob,Grep'` a reviewer still ran
#      `just --version`. Deny beats allow, so the DENY list is the half that binds.
#   2. `Bash` is not the only door to a shell. Denied it, a reviewer reached for
#      `Monitor` (runs a command, streams its stdout) and said so in its report. Same
#      door in `Workflow`, `Skill`, `CronCreate`, `SendMessage`, `EnterWorktree`.
#
# So review_deny is a DENYLIST OVER AN ENUMERATED SURFACE, not a proof: it is every
# tool the CLI reported having, minus Read/Glob/Grep. A release that adds an executor
# re-opens the door silently — re-enumerate on a CLI bump with:
#   echo 'List every tool available to you, one per line.' | claude -p ...
# The half that does hold is structural: the recipe hands the reviewer the diff and the
# sha, so it needs no shell for the job at all.
review_deny := "Bash,Write,Edit,NotebookEdit,Task,Agent,Artifact,WebFetch,WebSearch,Monitor,Workflow,Skill,ToolSearch,CronCreate,CronDelete,CronList,EnterWorktree,ExitWorktree,LSP,PushNotification,RemoteTrigger,SendMessage,ScheduleWakeup,TaskOutput,TaskStop,ListAgents,ListMcpResourcesTool,ReadMcpResourceTool,WaitForMcpServers,mcp__*"
review_cmd  := "claude -p --disallowedTools '" + review_deny + "' --allowedTools 'Read,Glob,Grep'"
#   codex:      codex exec -s read-only          # sandboxed by the flag, no list needed
#   opencode:   opencode run --auto
#   cursor:     cursor-agent -p
#
# `{{{{base}}` is how a justfile writes the literal string `{{base}}` (an opening `{{`
# is escaped by doubling it; the closing `}}` needs nothing): sed swaps that placeholder
# and `{{{{sha}}` for this repo's base branch and today's HEAD. The diff follows a plain
# marker rather than a fence, because a fence gets closed early by the first ``` in any
# markdown file's diff.
#
# The prompt goes in on STDIN, never as an argument: `--allowedTools` is variadic, so a
# trailing positional is eaten as more tool rules and the run dies with "Input must be
# provided either through stdin or as a prompt argument" after printing one "Ignoring
# --allowedTools rule" per word of the prompt. A pipe has no argv to mis-parse.
#
# The report is written to `.work/review.tmp` and moved into place, because `>` truncates
# its target BEFORE the CLI runs: a CLI that dies (absent, no key, rate-limited, Ctrl-C)
# left a 0-byte file, review-guard read that as MALFORMED, and since the hook has no
# glob that blocked every push on the machine — with the "no report → passes" escape
# unreachable, because the file existed. Observed in the wild, not theorised.
#
# The temp file is NOT named `review-report.md.part`: that string contains
# `review-report.md`, so bash-guard denied the recipe's own two lines to anyone
# replaying them by hand to debug a review — blocked by the forge-the-report rule while
# doing the opposite of forging.
#
# A dead CLI therefore leaves the PREVIOUS report standing, and that is deliberate: the
# alternative (delete it first) means a rate-limited review CLEARS a CRITICAL, which is
# the hard bypass autonomy.md forbids. The signal that nothing ran is this recipe's
# non-zero exit — read it and hand back "code review not run", never as green. The `mv`
# line stays deny-listed for a hand-typed spelling, on purpose: writing the report by
# hand is a forged verdict, and the recipe is trusted because editing the justfile is
# itself gated.
#
# The guard runs at the end so the verdict comes back as an exit code, not as something
# you have to remember to go and read. Needs a POSIX shell.
code-review:
    mkdir -p .work
    sed -e 's|{{{{base}}|{{base}}|g' -e "s|{{{{sha}}|$(git rev-parse HEAD)|g" scripts/review-prompt.md > .work/prompt.md
    printf '\n\n=== DIFF UNDER REVIEW ===\n\n' >> .work/prompt.md
    git diff {{base}}...HEAD >> .work/prompt.md
    {{review_cmd}} < .work/prompt.md > .work/review.tmp
    mv .work/review.tmp .work/review-report.md
    node scripts/review-guard.mjs

# The deterministic half of the review — pure Node, no LLM, milliseconds. THAT is
# why it can be a hook (pre-push; never pre-commit, the cadence is per push). It
# reads the two markers at the END of the report (the LAST of each, so a verdict quoted
# mid-report is data): CRITICAL blocks whatever the sha, an absent OR EMPTY report
# passes with a notice (declared, never simulated), a stale CLEAN/WARNINGS passes, a
# malformed one blocks. One test per arm in test/gates.test.mjs; doctrine:
# rules/agent/autonomy.md.
review-guard:
    node scripts/review-guard.mjs

# ── Duplication (opt-in) ───────────────────────────────────────────────────
# Copy-paste is an agent's native failure mode, and it is the one "AI slop
# indicator" from rules/agent/guardrails.md a machine can measure. jscpd is
# npx-only (no install, no server) and language-agnostic. Add `dup-check` to
# `check` once the reported number is at a level you accept — the same
# baseline-then-ratchet sequence as any other metric (rules/testing/ratchet.md).
# Tune --threshold (max % duplication tolerated) and the ignore globs.
dup-check:
    npx --yes jscpd . --min-lines 12 --threshold 3 --reporters consoleFull \
        --ignore "**/node_modules/**,**/target/**,**/dist/**,**/*generated*/**"

# ── Decisions ──────────────────────────────────────────────────────────────
# Opt-in: add `adr-check` to `check` above if the repo keeps ADRs under docs/adr/.
# An agent proposes a decision, a human accepts it — and accepting means committing
# the status line, which is the one signal an agent does not produce on its own.
# Doctrine: rules/agent/decisions.md. Node (>=18) rather than bash, so `just check`
# stays cross-platform; the script is a no-op when there are no ADRs.
adr-check:
    node scripts/adr-check.mjs
    # Size/section budgets are advisory above. Enforce them with:
    #   node scripts/adr-check.mjs docs/adr --strict
    # Move the ceiling for THIS repo in .docs-budgets.json (never in the script — an
    # update overwrites it):  { "adr": { "unitCeiling": 900 } }

# ── Living documents ───────────────────────────────────────────────────────
# Opt-in: add `docs-check` to `check` above if the repo keeps a PRD/PLAN under docs/.
# A document meant to grow is a directory of units plus a compacted index — this fails
# on an index and its units disagreeing (dangling link, unreferenced unit) and warns on
# the budgets. Doctrine: rules/product/documents.md. No-op with no docs/.
# Enforce the budgets too with: node scripts/docs-check.mjs docs --strict
# Budgets are defaults — move them for THIS repo in .docs-budgets.json (never in the
# script, which an update overwrites):  { "prd": { "indexCeiling": 1500 } }
docs-check:
    node scripts/docs-check.mjs

# ── The agent install itself ───────────────────────────────────────────────
# Opt-in: add `rules-check` to `check` above once the install has settled. It audits
# .claude-rules.lock against what is actually on disk and in this repo — an asset the
# lock does not explain, a rule whose globs match no file here, the always-on context
# budget. Offline and deterministic (npx fetches the CLI once, then npm caches it).
# Fails on facts, warns on judgments; `--strict` promotes the warnings.
rules-check:
    npx --yes github:dohrm/claude-rules doctor
