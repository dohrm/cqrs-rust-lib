---
paths:
  - "**/*.rs"
title: "Rust Code Style"
---

Format and clippy (`-D warnings`, including `await_holding_lock`): `just rust-lint`.

What clippy does not decide:

- Exhaustive `match` — no `_` for meaningful variants. `matches!` / `if let` when only one arm matters.
- `Option<&T>` over `&Option<T>`; `impl Trait` over `Box<dyn Trait>` when the type is known; `Cow` when it sometimes owns.
- **≤ 50 lines** per function, hard 100 — extract a named helper past that.
- API types: `#[serde(rename_all = "camelCase", deny_unknown_fields)]`. Domain types keep field names. Tagged enums, `skip_serializing_if = "Option::is_none"`, `default` on new optional fields.
- Tokio: timeouts on every external I/O. Bounded `mpsc` unless you need `broadcast` / `watch` / `oneshot`. `JoinSet` for fan-out. Never `std::sync::Mutex` across `.await` (clippy already).
