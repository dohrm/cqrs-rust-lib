---
paths:
  - "**/*.rs"
title: "Rust Error Handling"
---

`unwrap` / `expect` / `panic!` on lib and bins: `just rust-lint`. Tests are free. A real invariant: `#[allow(clippy::panic, reason = "…")]`.

| Context | Library |
|---------|---------|
| Domain / library crates | `thiserror` — typed, matchable |
| Infrastructure / adapters | `thiserror` or `anyhow` with context |
| CLI / `main.rs` | `anyhow` or `miette` |

- Never `Box<dyn Error>` in domain crates, never `anyhow` in port signatures.
- One error enum per boundary; variants named after *what* failed, not *where*. No `Other(String)`.
- Propagate with `?`. Log before `let _ = …`.
