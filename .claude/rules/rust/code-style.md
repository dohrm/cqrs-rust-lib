---
paths:
  - "**/*.rs"
title: "Rust Code Style"
---

Format and clippy (`-D warnings`, `await_holding_lock`, no `dbg!` / `todo!` / `unimplemented!`, `// SAFETY:` on every `unsafe`): `just rust-lint`.

What clippy does not decide:

- Exhaustive `match` — no `_` for meaningful variants. `matches!` / `if let` when only one arm matters.
- `Option<&T>` over `&Option<T>`; `impl Trait` over `Box<dyn Trait>` when the type is known; `Cow` when it sometimes owns.
- **≤ 50 lines** per function, hard 100 — extract a named helper past that.
- API types: `#[serde(rename_all = "camelCase", deny_unknown_fields)]`. Domain types keep field names. Tagged enums, `skip_serializing_if = "Option::is_none"`, `default` on new optional fields.
- Tokio: timeouts on every external I/O. Bounded `mpsc` unless you need `broadcast` / `watch` / `oneshot`. `JoinSet` for fan-out. Never `std::sync::Mutex` across `.await` (clippy already).
- Every spawned task has an owner that awaits or aborts it and sees its error — never a dropped `JoinHandle`.
- Blocking or CPU-heavy work in async goes through `spawn_blocking`. A `select!` branch is cancellation-safe, or the partial work survives the drop.
- Retries are bounded, with backoff and a timeout.
