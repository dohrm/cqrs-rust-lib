---
paths:
  - "**/*.rs"
title: "UTF-8 String Safety"
---

Not a clippy pass. Indexing a `&str` by byte position panics or corrupts on non-ASCII.

- Never `s[i..j]` or `s.as_bytes()[i]` to process text. Use `find`, `split`, `chars`, `char_indices`, or a regex.
- Never `bytes[i] as char`. Never assume 1 byte = 1 character.
- Slice `&s[start..]` only when `start` is a char boundary (`str::find` / `rfind`).
- Character offsets for an editor or range API: map from bytes — do not hand over raw indices.
