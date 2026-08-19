---
paths:
  - "**/*.rs"
title: "UTF-8 String Safety"
---

Any project handling non-ASCII text — French accents (é, è, ê, ç, à…), and most
human languages — must treat every string operation as UTF-8-aware. This is a
recurring, easily-missed source of panics and silent corruption.

## Rules

- **NEVER index a `&str` by byte position** (`s[i..j]`). Use `str::find()`,
  `str::split()`, `str::chars()`, or a regex.
- **NEVER iterate bytes** (`s.as_bytes()[i]`) to process text — use `str`
  methods or `char` iterators.
- **NEVER cast bytes to char** (`bytes[i] as char`) — multi-byte UTF-8 chars
  produce wrong results or panics.
- **NEVER assume 1 byte = 1 character.** A French accented char is 2 bytes.
- Slice `&content[start..]` only when `start` is a guaranteed char boundary
  (e.g. an offset returned by `str::find()` / `str::rfind()`).
- When computing character offsets for an external consumer that counts by
  *character* (an editor, a text-range API), convert from byte offsets using a
  proper UTF-8 mapping — do not hand it raw byte indices.

## Safe patterns

```rust
// ✅ str::find returns a char boundary
if let Some(pos) = content.find("[DOC:") {
    let (before, after) = (&content[..pos], &content[pos..]);
}

// ✅ split/replace operate on char boundaries
let cleaned = content.replace("[DOC: ...]", "");

// ✅ char iterator with byte offsets that ARE valid boundaries
for (i, ch) in content.char_indices() { /* … */ }

// ❌ byte indexing — panics / wrong char on multi-byte text
let ch = content.as_bytes()[i] as char;
let slice = &content[i..i + 5]; // may split a char → panic
```

## Other languages (same class of bug, different shape)

- **TypeScript/JS**: strings are UTF-16. `str.length` and indexing count code
  *units*, not code *points* — astral chars (emoji) are surrogate pairs. Prefer
  `[...str]`, `Array.from(str)`, or `Intl.Segmenter` for character-correct work.
- **Go**: strings are byte slices; `len(s)` is bytes and `s[i]` is a byte.
  Iterate with `for _, r := range s` (yields runes) or use `utf8`/`unicode/utf8`.
- **Python 3**: `str` is code points (safe by default); the trap is `bytes` ↔
  `str` boundaries — decode/encode explicitly with a known encoding.
