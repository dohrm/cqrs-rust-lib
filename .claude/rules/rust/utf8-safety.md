---
paths:
  - "**/*.rs"
title: "UTF-8 String Safety"
---

Indexing a `&str` by byte position panics or corrupts on non-ASCII. `&s[i..j]` on lib and bins: `just rust-lint` (`clippy::string_slice`).

- Never `s[i..j]` or `s.as_bytes()[i]` to process text. Use `find`, `split`, `chars`, `char_indices`, or a regex.
- Never `bytes[i] as char`. Never assume 1 byte = 1 character.
- A boundary from `str::find` / `rfind`: `s.get(start..)` or `split_at_checked` — no raw slice.
- Character offsets for an editor or range API: map from bytes — do not hand over raw indices.
