---
paths:
  - "**/*.rs"
title: "Rust Quality Gates"
---

```bash
just rust-check     # rust-lint → tests → deny + machete
```

`rust-lint` is pre-commit; `rust-check` is pre-push and `just check`. Tier 4 is `just rust-mutate` — never a hook. Wiring: `.dev/kit/rust/README.md`.

| Recipe | Command | Config |
|---|---|---|
| `rust-lint` | `cargo fmt --all --check` | `rustfmt.toml` → `<rust_dir>/` |
| `rust-lint` | `cargo clippy --workspace --all-targets -- -D warnings` + `dbg_macro` `todo` `unimplemented` `undocumented_unsafe_blocks` | — |
| `rust-lint` | `cargo clippy --workspace --lib --bins` + `unwrap_used` `expect_used` `panic` `string_slice` | — |
| `rust-check` | `cargo test --workspace` | — |
| `rust-check` | `cargo deny check licenses advisories sources` | `deny.toml` → `<rust_dir>/` |
| `rust-check` | `cargo machete --skip-target-dir` | — |
| `rust-mutate` | `cargo mutants --in-diff` | `mutants.toml` → `<rust_dir>/.cargo/` |

No `#[allow(clippy::…)]` without a reason on the same line. Never loosen the configs or clippy flags to pass.

Sibling rules cover what clippy does not: match/`_`, function size, serde, timeouts, thiserror placement, tracing fields, UTF-8 indexing.
