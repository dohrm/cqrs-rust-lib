---
paths:
  - "**/*.rs"
title: "Rust Logging (tracing)"
---

Clippy does not see this.

| Level | Usage |
|-------|-------|
| `error` | Handling boundary — not at every `?` |
| `warn` | Degraded but recoverable |
| `info` | Program flow |
| `debug` | Decision paths |
| `trace` | Heavy diagnostic, scripts only |

One log where propagation stops. Log before `let _ = …`. Never log a secret — the key name only.

```rust
info!(user_id = %id, "creating user");
#[instrument(skip_all, fields(user_id = %cmd.user_id))]
```

`skip_all` + explicit `fields` — never auto-log params. Instrument entry points, not hot inner functions. I/O crates declare `tracing`.
