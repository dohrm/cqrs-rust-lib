---
paths:
  - "**/*.rs"
title: "Rust Test Layout"
---

Each kind of test has one place, and no test lives anywhere else.

| Kind | Where | Declared by | Sees |
|------|-------|-------------|------|
| Unit: the module's own behaviour, private items included | `foo/tests.rs`: inside a `foo/` directory next to `foo.rs`, or beside `foo/mod.rs`; `src/tests.rs` for the crate root | `#[cfg(test)] mod tests;` as the last item of `foo.rs` (a convention: clippy's `items_after_test_module` only sees inline modules) | `super::*`, private included |
| Integration: the crate's public API, end to end | one test crate, `tests/it/main.rs`, with one module per subject (`tests/it/<subject>.rs`) | nothing: Cargo builds it as its own crate | the crate's `pub` API only |
| Doctest: a usage example of a public item | its `///` doc comment | nothing | the crate's `pub` API only |

- **No inline `mod tests { … }` block.** Code and tests are read separately, whatever their size.
- **No `#[path = "…"]`.** Use the standard module path (`foo/tests.rs`), as `std` and `rustc` do.
- **In a production file, `#[cfg(test)]` marks only a module declaration**: `mod tests;`, or `mod testkit;`
  for fixtures shared by the crate's unit tests — declared in `lib.rs` (or `main.rs`), so it lives in
  `src/testkit.rs` and is reached as `crate::testkit`. Never a helper function, an impl or a field.
- **`cfg(test)` does not reach `tests/` or other crates.** They link the library built without it. Fixtures
  for integration tests go in `tests/it/` (a `common.rs` module). Fixtures shared across crates of a
  workspace go behind a `test-support` feature, enabled only from `[dev-dependencies]`, or in a dedicated
  dev-only crate. The feature gates the same module, still as a declaration only:
  `#[cfg(any(test, feature = "test-support"))] pub mod testkit;`.
- **One integration crate, not one file per test.** Every `tests/*.rs` file is a separate crate, linked
  separately. Many files multiply link time.
- **A test that needs private items is a unit test.** Never widen a production item's visibility for a test;
  a `testkit` behind `test-support` is the one `pub` surface made for tests. Its `pub` items take and
  return public types only, and never re-export or wrap a private item.
- **A test that only uses the crate's `pub` API belongs in `tests/`.** It documents the contract and
  survives any internal refactor. A `pub` item of a private module is not public API: its test is a
  unit test.
- **A binary keeps its logic in `lib.rs`.** `tests/` cannot import from `main.rs`, so `main.rs` only parses
  input and calls the library.

Why it is fixed this way: an agent editing logic reads `foo.rs` without paying for its tests in context,
finds the tests at a predictable path, and a diff keeps code and test changes apart.
