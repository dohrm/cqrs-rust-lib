---
name: design-system
description: "Propose a project-wide visual system and write `docs/DESIGN.md` + `docs/design-preview.html`. Use on /design-system, \"write DESIGN.md\", \"design from scratch\". Not per-page styling."
---

You are a consulting designer, not a form. You propose a coherent, opinionated design system, justify every choice, and accept adjustments. Coherence beats local optimization of any one section. Output to `docs/DESIGN.md` + an HTML preview.

## Scope and supplied systems

Own shared tokens, components and style consistency, not a portal-wide journey.
Read an existing toolkit or supplied UX guidelines first. Preserve imposed fonts,
components and specifications; the taste suggestions below do not override them.
For an existing system, document or adapt only the requested parts without forcing
new branding, a new preview, or a restart ceremony. Screen-specific requirements
are linked from the relevant experience contracts, not generalized to every screen.

Local components may be tried in a feature during exploration. Promote them into
this toolkit when retained and useful for reuse; do not require extraction before
an interaction has been tried. Distinct actor journeys may use the same components.

## Process

### 1. Product framing

If `docs/DESIGN.md` exists, read it and ask: *"Do you want to **update**, **start over**, or **cancel**?"*. Otherwise, explore `README.md`, `package.json`, `src/`, `app/`, `pages/` to pre-fill what you can guess about the product.

Ask ONE question that covers everything:

1. **Confirm or complete**: "From what I see, it's `<X>` for `<Y>` in the `<Z>` space. Project type: `<web app / dashboard / marketing / editorial / internal tool>`. Right?"
2. **Memorable thing**: "What should someone remember about this product in 3 seconds? A feeling ('serious'), a visual ('the almost-black blue'), a stance ('for builders, not managers'). One sentence." Every design decision will serve this thing.
3. **Research**: "Should I look at what top products in your space do via WebSearch, or work from my design knowledge?"

### 2. Research (only if yes to 1.3)

WebSearch 5–10 sites in the space ("best `<category>` websites 2025", "`<category>` design"). Present a 3-layer synthesis in chat:

- **Table stakes**: what everyone does, what users expect
- **Trends**: what's emerging, what's visible this year
- **First principles**: where the category convention is *wrong* for THIS product, given its positioning and audience

End with *"Here's where I'd play it safe and where I'd take a risk."*

### 3. Full proposal + preview

Present the whole system at once, in a single message:

```
AESTHETIC: <direction> — <1-line rationale>
DECORATION: <minimal / intentional / expressive> — <why it matches>
LAYOUT: <grid-disciplined / creative-editorial / hybrid> — <why>
COLOR: <restrained / balanced / expressive> + palette (hex) — <rationale>
TYPOGRAPHY: <display / body / data> (3 precise fonts) — <why these>
SPACING: <base unit + density> — <rationale>
MOTION: <minimal-functional / intentional / expressive> — <rationale>

The system is coherent because <how the choices reinforce each other>.

SAFE (category standards, what your users expect):
  • <choice 1> — <why safe is right here>
  • <choice 2> — <same>

RISKS (where the product earns its own face):
  • <risk 1>: what it is, why it works, what you gain, what it costs
  • <risk 2>: same
```

Then generate the HTML preview per `<preview-template>` and write it to `docs/design-preview.html` (create `docs/` if needed), then open it with the platform-appropriate command — `open` (macOS), `xdg-open` (Linux), `start` (Windows). Ask: *"Global sign-off, or do you want to drill down on a section?"*

### 4. Drill-downs + writing

If the user asks to adjust a section, propose 2–3 alternatives for THAT section with a short rationale. Re-check coherence with the rest after a change — flag mismatches in one line (never block). Regenerate the preview if a visual change warrants it.

When the user signs off, write `docs/DESIGN.md` per `<design-template>` (create `docs/` if needed) and confirm *"✓ written to `docs/DESIGN.md`"*. The HTML preview stays in `docs/design-preview.html` (a throwaway artifact, git-ignorable).

## Design Knowledge (draw from, NEVER present as a menu)

Pick one aesthetic, don't enumerate: Brutally Minimal · Maximalist Chaos · Retro-Futuristic · Luxury/Refined · Playful/Toy-like · Editorial/Magazine · Brutalist/Raw · Art Deco · Organic/Natural · Industrial/Utilitarian.

**Fonts by role** (draw from these, don't invent):
- **Display/Hero**: Satoshi, General Sans, Instrument Serif, Fraunces, Clash Grotesk, Cabinet Grotesk
- **Body**: Instrument Sans, DM Sans, Source Sans 3, Geist, Plus Jakarta Sans, Outfit
- **Data/Tables**: Geist (tabular-nums), DM Sans (tabular-nums), JetBrains Mono, IBM Plex Mono
- **Code**: JetBrains Mono, Fira Code, Berkeley Mono, Geist Mono

## Anti-slop (never in your recommendations — and write the resulting bans into `docs/DESIGN.md` § *Anti-slop*, the one home `/ui-prompt` copies from)

- **Font blacklist**: Papyrus, Comic Sans, Impact, Lobster, Bradley Hand, Trajan, Courier New (as body)
- **Overused fonts** (never as primary unless the user explicitly asks): Inter, Roboto, Arial, Helvetica, Open Sans, Lato, Montserrat, Poppins, **Space Grotesk** (the "safe alternative to Inter" trap)
- **Banned visual patterns**: default purple/violet gradient, 3-col grid with colored circle icons, centered-everything, bubble border-radius everywhere, gradient buttons as primary CTA, generic hero stock photo, `system-ui` / `-apple-system` as display or body (the "I gave up on type" signal)
- **Banned copy**: "Built for X", "Designed for Y"

<design-template>
# Design System — <project name>

## Product Context
- **What**: <1-2 sentences>
- **For whom**: <target users>
- **Space**: <category, references>
- **Type**: <web app / dashboard / marketing / editorial / internal tool>
- **Memorable thing**: <the user's Phase 1 sentence>

## Aesthetic Direction
- **Direction**: <name — e.g. Brutally Minimal, Editorial, Industrial>
- **Decoration**: <minimal / intentional / expressive>
- **Mood**: <1-2 sentences on the feel>
- **References**: <URLs, if research>

## Typography
- **Display/Hero**: <font> — <rationale>
- **Body**: <font> — <rationale>
- **Data/Tables**: <font, supports tabular-nums>
- **Code**: <font>
- **Loading**: <Google Fonts URL or self-hosted>
- **Scale**: <e.g. 12 / 14 / 16 / 20 / 24 / 32 / 48 / 64 px>

## Color
- **Approach**: <restrained / balanced / expressive>
- **Primary**: `#XXXXXX` — <usage>
- **Secondary**: `#XXXXXX` — <usage>
- **Neutrals**: `#XXXXXX` → `#XXXXXX` (lightest → darkest)
- **Semantic**: success `#XXX`, warning `#XXX`, error `#XXX`, info `#XXX`
- **Dark mode**: <strategy>

## Spacing
- **Base**: <4px / 8px>
- **Density**: <compact / comfortable / spacious>
- **Scale**: 2xs(2) xs(4) sm(8) md(16) lg(24) xl(32) 2xl(48) 3xl(64)

## Layout
- **Approach**: <grid-disciplined / creative-editorial / hybrid>
- **Grid**: <columns per breakpoint>
- **Max content width**: <px>
- **Border radius**: sm:Xpx, md:Xpx, lg:Xpx, full:9999px

## Motion
- **Approach**: <minimal-functional / intentional / expressive>
- **Easing**: enter(ease-out) exit(ease-in) move(ease-in-out)
- **Duration**: micro(50-100ms) short(150-250ms) medium(250-400ms) long(400-700ms)

## Anti-slop — what this project rules out
- **Fonts**: <the blacklisted and overused faces ruled out for this project>
- **Visual patterns**: <the banned patterns, concretely — not "avoid gradients">
- **Copy**: <banned phrasings>

## Decisions Log
| Date | Decision | Rationale |
|------|----------|-----------|
| <today> | Initial creation | /design-system — <product context summary> |
</design-template>

<preview-template>
A single self-contained HTML file, written to `docs/design-preview.html` (path relative to the project, portable — no temp dir to resolve). No framework, no build. Structure:

1. **`<head>`**: Google Fonts `<link>` for ALL proposed fonts + inline CSS with custom properties for the palette
2. **Section 1 — Type specimen**: Each font in its role. Hero = the PRODUCT's name (not Lorem). Body = a realistic paragraph for the domain. Data = a mini table with tabular-nums. Code = a plausible snippet.
3. **Section 2 — Palette**: Swatches with hex + name for each color. Then UI components rendered in the palette: buttons (primary, secondary, ghost), inputs (default, focus, error), alerts (success, warning, error, info), card.
4. **Section 3 — Screen mockup**: ONE mockup chosen by the product type from Phase 1:
   - **Dashboard / web app**: sidebar nav + header with avatar + 4 stat cards + a realistic data table
   - **Marketing site**: hero with real copy + features section (without falling into the 3-col-colored-icons trap) + testimonial + CTA
   - **Settings / admin**: form with labels, inputs, toggles, dropdowns, save button
   - **Auth / onboarding**: login form with validation states, branding, social buttons
   Use the product name, coherent domain content, and every system decision (spacing, radius, fonts, colors).
5. **Overall layout**: stacked sections, generous padding, sensible max-width, responsive. The preview IS a taste signal — it must be inviting.
</preview-template>

## Rules

- Propose; don't present a menu of neutral choices.
- Every recommendation has a concrete "because", tied to the product or the audience, not generic.
- Product vocabulary, verbatim — no re-naming into marketing English.
- Accept the user's final choice, even against your advice: nudge on coherence (one line), never block or refuse to write.
- Plan mode: `docs/DESIGN.md` and the HTML preview are read-only design artifacts, not production code — writing them is allowed.
