---
name: diagram
description: "Write valid `.excalidraw` JSON that argues a concept (relationships, causality), not a decoration. Use on /diagram, \"flowchart\", \"illustrate this\". Export happens in Excalidraw — no rendering pipeline."
allowed-tools: Read, Write, Bash, Glob, AskUserQuestion
---

You create visual diagrams. A diagram must **ARGUE, not DISPLAY** — every diagram should advance the reader's understanding in a way text alone cannot: it shows the relationships, transformations, and causalities that prose can't carry.

This skill is **dependency-free**: it produces a valid `.excalidraw` JSON file. To view or export a PNG/SVG, open the file in Excalidraw ([excalidraw.com](https://excalidraw.com), the desktop app, or the "Excalidraw" VS Code extension) and use its built-in export. Do not add a rendering pipeline.

---

## 6-Step Design Process

### 1. Clarify the concept and context

If the context isn't obvious from the conversation, ask:
- "What concept do you want to illustrate?"
- "In what context? (video, lesson, article, slide…)"
- "Simple/conceptual or technical/detailed?"

**Levels:**
- **Simple/Conceptual** — abstract shapes, high-level relationships, little text
- **Technical/Detailed** — concrete examples, real tool names, real data flows

### 2. Understand the relationships

Identify in the concept:
- **Transformations**: A → B (what changes and how)
- **Relationships**: hierarchy, dependency, sequence, parallelism
- **Comparisons**: Before/After, With/Without, Option A vs B
- **Flows**: Input → Process → Output

### 3. Map to a visual pattern

| Pattern | Use | When |
|---------|-----|------|
| **Fan-Out** | 1 → N | one element decomposing into several |
| **Convergence** | N → 1 | several elements merging |
| **Pipeline** | A → B → C | a sequential process |
| **Tree** | hierarchy | parent-children, categorization |
| **Cycle** | loop | iterative process, feedback loop |
| **Side-by-Side** | comparison | Before/After, A vs B |
| **Matrix** | 2 axes | 2D classification |
| **Timeline** | chronology | steps over time |
| **Layers** | stack | abstraction layers |

### 4. Ensure variety

If several diagrams share one output: vary the visual patterns, vary the dominant colors, alternate complexity (simple → complex → simple).

### 5. Sketch the layout

**Reference sizes:** Hero (main element) 300×150px · Primary 180×90px · Secondary 120×60px · Small marker 60×40px.

**Spacing:** 200px+ around main elements · 80–100px between secondary elements · 40px minimum between any two elements.

### 6. Generate the Excalidraw JSON

Write a valid `.excalidraw` file (JSON). Build large diagrams section by section (see below).

---

## Container discipline

**Rule: less than 30% of the text belongs inside shapes.** Typography creates the hierarchy; containers delimit, they don't hold all the text.

| Concept | Shape | Reason |
|---------|-------|--------|
| Labels/descriptions | none (free text) | type creates the hierarchy |
| Timeline markers | small circle (10–20px) | visual anchor without containment |
| Trigger/Input | ellipse | origin quality |
| Decision/Condition | diamond | classic decision symbol |
| Process/Action | rectangle | contained action |
| Abstract state | overlapping ellipses | fuzzy, cloud-like |
| Hierarchy node | lines + text | structure by connection |

---

## Aesthetic standards

```json
{
  "roughness": 0,
  "opacity": 100,
  "fontSize": 24,
  "textAlign": "center",
  "strokeWidth": 2
}
```

- **roughness: 0** — always clean and crisp (no "sketch" style)
- **fontSize: 24** minimum by default; you may go to 18 for a static article/doc, never below (legibility)
- **strokeWidth: 2** standard, 3 for emphasis
- **Color** — pick a restrained, consistent palette up front (one accent + neutrals, plus semantic colors only if the diagram needs them) and reuse it across every diagram in the batch. Consistency over per-diagram novelty. Match the project's design system if one exists (`docs/DESIGN.md`).

---

## Large diagrams

For complex diagrams (>15 elements), build section by section:

1. **Descriptive IDs**: use meaningful string IDs (`"input-webhook"`, `"process-filter"`)
2. **Namespaces per section**: group related elements
3. **Progressive construction**: generate section by section, verify, then assemble
4. **Never** attempt a whole diagram in a single block

---

## Output workflow

### A. Design

1. Decide where the visual should be saved — infer from context or ask: an in-progress lesson/article (`<project>/assets/diagrams/` or `<project>/images/`), otherwise `./diagrams/` in the current directory.
2. Identify every visual moment needed.
3. Propose the diagrams to create (count, type, placement).
4. Wait for validation.

### B. Generation

Two modes, by context:

- **Grouped (recommended for videos / slide series)**: all diagrams of one delivery stacked vertically in a single `diagrams.excalidraw`. One file to open, scroll, and keep a consistent style.
  - **Vertical gap between slides: 800px minimum for video** (lets a video editor zoom one slide without the neighbors bleeding into frame); 200px is fine for doc/article use.
  - **Per-slide label**: `── Slide N — Title ──` top-left, discreet.
  - **Separator**: a dotted horizontal line (`strokeStyle: "dotted"`, `strokeColor: "#CCCCCC"`, `strokeWidth: 2`) across the full width, mid-gap.
  - Stacking: `y_offset(slide N) = N * (SLIDE_H + GAP)`, with `SLIDE_H = 1080`, `CANVAS_W = 1920`.
- **Separate files (recommended for lessons / articles)**: each diagram in its own `diagram-<name>.excalidraw`, inserted one by one into a doc or slide deck.

If the mode is ambiguous, ask.

### C. Export

Tell the user how to get the image: open the `.excalidraw` file in Excalidraw (web/desktop/VS Code extension) and export to PNG (raster, for immediate use) or SVG (vector, for scaling without loss). The `.excalidraw` file is the source of truth and stays editable.

### D. Validation

Present each diagram and check:
- [ ] Legible at the target format (mobile video, lesson screen, article size)?
- [ ] Enough contrast?
- [ ] No truncated or overflowing text?
- [ ] Clear relationships/arrows?
- [ ] Understandable in 3–5 seconds?

### E. Presentation

List the generated files with path and a short description.

---

## Rules

- ALWAYS `roughness: 0` — never the sketch/rough style.
- Match the project's design system palette if one exists; otherwise a restrained, consistent palette.
- NEVER a diagram that needs more than 5 seconds to understand.
- NEVER more than 7 main elements per diagram — split if necessary.
- 1920×1080 (16:9) is the versatile default canvas; relax only when the context clearly justifies it (static article, custom web embed).
- Dependency-free: never introduce a Python/headless-browser renderer — export happens in Excalidraw.
