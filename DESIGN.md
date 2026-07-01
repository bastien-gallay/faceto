---
name: faceto
description: A typed JSON model rendered as a calm, instrument-grade event-storming board.
colors:
  # Domain grammar — one type → one colour → one lane. Fixed; not decoration.
  lane-actor: "#FCEFA1"
  lane-command: "#1A6FAE"
  lane-aggregate: "#FFD23F"
  lane-event: "#FF9F1C"
  lane-policy: "#C39BD3"
  lane-readmodel: "#6FCF97"
  lane-external: "#F2A0C9"
  lane-hotspot: "#C0392B"
  # Chrome — instrument greys, the table the specimen sits on.
  bench-bg: "#fbfbfd"
  surface: "#ffffff"
  ink: "#222222"
  muted: "#777777"
  muted-strong: "#555555"
  lane-label: "#90a4ae"
  border-chrome: "#e6e6ee"
  border-control: "#cfcfda"
  rule-line: "#e0e0e6"
  # Accent — the live pen. Same hue as the command lane, by design.
  accent: "#1A6FAE"
  chip-bg: "#eef1f4"
  status-idle-bg: "#eeeeff"
  status-idle-ink: "#555577"
  status-live-bg: "#e6f7ec"
  status-live-ink: "#1b7a3d"
  resolved-fill: "#D9DEE3"
  edge-flow: "#9AA7B0"
  edge-hotspot: "#C39086"
  # Diff overlay — the focusing pass that says what moved.
  diff-added: "#27ae60"
  diff-removed: "#EB5757"
  diff-changed: "#E59500"
typography:
  # Nameplate — the one serif, board title only (HTML header + SVG). All system fonts; nothing downloaded.
  nameplate:
    fontFamily: "'Iowan Old Style', 'Palatino Linotype', Palatino, 'Book Antiqua', Georgia, serif"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "0.2px"
  headline:
    fontFamily: "-apple-system, Segoe UI, Roboto, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: "-apple-system, Segoe UI, Roboto, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  caption:
    fontFamily: "-apple-system, Segoe UI, Roboto, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: 1.3
  lane-label:
    fontFamily: "-apple-system, Segoe UI, Roboto, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: 1.3
  micro:
    fontFamily: "-apple-system, Segoe UI, Roboto, sans-serif"
    fontSize: "9px"
    fontWeight: 700
    lineHeight: 1.2
rounded:
  hotspot: "2px"
  xs: "3px"
  control: "7px"
  card: "8px"
  modal: "12px"
  pill: "999px"
spacing:
  xs: "8px"
  sm: "12px"
  md: "14px"
  lg: "16px"
components:
  sticky-card:
    backgroundColor: "{colors.lane-event}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    height: "74px"
    width: "176px"
  sticky-hotspot:
    backgroundColor: "{colors.lane-hotspot}"
    textColor: "{colors.surface}"
    rounded: "{rounded.hotspot}"
    height: "74px"
    width: "176px"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "5px 11px"
  button-secondary-hover:
    backgroundColor: "#f2f2f7"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "5px 11px"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.surface}"
    rounded: "{rounded.control}"
    padding: "5px 11px"
  status-pill-idle:
    backgroundColor: "{colors.status-idle-bg}"
    textColor: "{colors.status-idle-ink}"
    rounded: "{rounded.pill}"
    padding: "3px 9px"
  status-pill-live:
    backgroundColor: "{colors.status-live-bg}"
    textColor: "{colors.status-live-ink}"
    rounded: "{rounded.pill}"
    padding: "3px 9px"
  input-field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "8px"
  chip:
    backgroundColor: "{colors.chip-bg}"
    textColor: "#334455"
    rounded: "6px"
    padding: "1px 7px"
---

<!-- markdownlint-disable MD025 MD036 -- Stitch spec mandates the H1 + bold marker lines. -->

# Design System: faceto

## 1. Overview

**Creative North Star: "The Optical Bench"**

`faceto` is an instrument, and an instrument lights the specimen and gets out of the way.
The specimen here is the model — the stickies, the lanes, the edges between them. Everything
else (the header bar, the status pill, the comment modal, the diff overlay) is the bench: the
stage, the lamp, and the focusing knob. The bench is machined grey and near-white so the only
saturated colour on screen belongs to the domain itself. When you hover an element the rest of
the board recedes — connectors fade to 7% opacity, unrelated stickies to 32% — exactly the way a
microscope dims the field to bring one cell into focus. You are never decorating; you are
focusing.

This system explicitly rejects the things that pull a tool's attention onto itself. It is **not**
a generic SaaS dashboard (no card grids, no hero-metric tiles, no gradient accents, no
tracked-uppercase eyebrows). It is **not** heavy branded chrome (no logos competing with the
board, no marketing gradients, no glassmorphism). It is **not** Miro/FigJam maximalism (no
floating toolbars, no ambient motion, no collaboration overload). And it is **not** toy or
childish — the sticky-note palette is a *fixed domain grammar* inherited from event storming, not
a candy decoration, and the chrome around it stays sober to prove the point.

**Key Characteristics:**

- Instrument-grey, near-white bench (`#fbfbfd` / `#ffffff`); colour is reserved for the model.
- A single accent — ink-blue `#1A6FAE`, the "live pen" — used only where you are acting.
- One working sans at fixed (not fluid) sizes, plus a serif nameplate for the title alone; density
  without noise.
- Hover-to-focus as the primary interaction metaphor; motion conveys state, never spectacle.
- Colour is never the only signal: lane position, label, and corner shape carry meaning too.

## 2. Colors

A grey-on-near-white bench carrying the saturated 8-colour event-storming grammar; a single
ink-blue accent for live action, and a three-colour diff vocabulary for change.

### Primary

- **Live Pen Ink-Blue** (`#1A6FAE`): the one accent. Save button, the active comment, the
  `has-note` ring on annotated stickies, and the focus signal. It is deliberately the same hue as
  the `command` lane — acting on the board speaks the board's own language. Used on ≤10% of any
  screen.

### Secondary — the Domain Grammar (fixed; never re-skin)

The eight lane colours are not a palette choice; they are the typed vocabulary of the board. One
`type` → one colour → one lane, top to bottom.

- **Actor Straw** (`#FCEFA1`): people / roles. Dark text.
- **Command Deep-Blue** (`#1A6FAE`): commands. White text — deepened from the classic event-storm
  blue so white clears 4.5:1.
- **Aggregate Amber** (`#FFD23F`): aggregates. Dark text.
- **Event Orange** (`#FF9F1C`): domain events — the spine of the board. Dark text.
- **Policy Lilac** (`#C39BD3`): policies / reactions. Dark text.
- **Read-Model Green** (`#6FCF97`): read models / views. Dark text.
- **External Pink** (`#F2A0C9`): external systems. Dark text.
- **Hotspot Deep-Red** (`#C0392B`): open questions / pain. White text, and the *only* squared
  sticky (2px corners) — a non-colour tell that this one is loud on purpose.

### Tertiary — the Diff Vocabulary

Shown only in the change overlay; each carries a non-colour badge so the signal survives
colour-blindness.

- **Added Green** (`#27ae60`), badge `+`: newly present elements/edges (dashed-outline ring).
- **Removed Red** (`#EB5757`), badge `–`: gone elements (ghosted to 40% opacity).
- **Changed/Moved Amber** (`#E59500`), badge `≠` (reworded) / `→` (relocated).

### Neutral — the Bench

- **Bench White** (`#fbfbfd`): the board canvas and SVG ground.
- **Surface White** (`#ffffff`): header bar, modal, controls, sticky-stroke contrast.
- **Ink** (`#222222`): primary text and dark sticky labels.
- **Muted** (`#777777`) / **Muted-Strong** (`#555555`): subtitles, detail lines, prior comments.
- **Lane Label** (`#90a4ae`): lane and phase captions — quiet enough to read as scaffolding.
- **Borders**: chrome `#e6e6ee` (header), control `#cfcfda` (buttons/inputs), rule `#e0e0e6`
  (lane dividers). Edges: flow `#9AA7B0`, hotspot-link `#C39086`.

### Named Rules

**The Bench-Is-Grey Rule.** No saturated colour in the chrome — ever. Every saturated pixel on
screen must belong to the domain grammar (a lane) or the live pen (`#1A6FAE`). If a control,
panel, or banner wants colour, the answer is a grey or a tint of the ink. The specimen is lit; the
bench is not.

**The Three-Signal Rule.** A lane is identified by colour **and** vertical position **and** label —
never colour alone. The hotspot reinforces this with a squared corner. Any new state must carry a
redundant non-colour cue (badge, shape, position, opacity) so a colour-blind user loses nothing.

## 3. Typography

**Nameplate Font (board title only):** `'Iowan Old Style', 'Palatino Linotype', Palatino,
'Book Antiqua', Georgia, serif` — a refined **system serif**. **Working Font (everything else):**
`-apple-system, Segoe UI, Roboto, sans-serif`, identical in the HTML chrome and the SVG board.
Both are system stacks: nothing is downloaded, so the board still installs and runs offline.

**Character:** The instrument carries exactly one engraved mark — its **nameplate**, the board
title, set in a precise system serif in the header bar *and* the SVG. Like the maker's plate on an
optical bench, it is the one place a second face appears; it gives the page a true typographic
hierarchy without dressing the chrome. Everything that does work — lane captions, sticky labels,
controls, modal copy — stays in one well-tuned native sans, because the *working surface* earns
trust through consistency. The pairing is deliberately lopsided: one serif word, a whole sans
instrument. Sizes are **fixed px**, not fluid clamps: users read at a consistent DPI and a sticky
label that shrank with the viewport would read as broken, not elegant.

### Hierarchy

A four-step scale (sans) with the serif nameplate above it; steps step by roughly 1.25 so the
levels read as distinct, not as near-duplicate sizes.

- **Nameplate** (serif, 700, 20px, 0.2px tracking): the board name, in the SVG and the header bar.
  The single piece of emphatic type and the only serif; everything else stays calm beneath it.
- **Headline** (sans, 600, 16px): the focused element's label in the comment modal.
- **Body** (sans, 400, 13px): controls, modal prose, comment text — the page base size.
- **Caption** (sans, 600, 11px): the status pill, notes, prior comments, relationship chips, the
  modal `id`, and detail lines — quiet supporting copy.
- **Lane label** (sans, 600, 12px, `#90a4ae`): lane and phase captions on the board; quiet enough
  to read as scaffolding.
- **Micro** (sans, 700, 9px, 0.6 opacity): the element `id` watermark in the corner of each
  sticky, and the 9.5px detail second line. Present for reference, never shouting.

### Named Rules

**The One-Serif Rule.** The serif nameplate is for the **board title and nothing else** — it is the
engraved maker's mark, not a heading style. Section labels, modal headlines, captions, and every
control stay in the working sans. A second serif word anywhere on the working surface is wrong; the
hierarchy belongs to size and weight, not to spreading the display face.

**The Fixed-Scale Rule.** No `clamp()`, no fluid headings. The board is an instrument read at
1:1; type sizes are pixel-exact and stable across viewports. Responsiveness is structural (the
board scrolls in its frame), never typographic.

## 4. Elevation

Flat by default. The bench is a near-white plane with hairline rules (lane dividers `#e0e0e6`) and
the faintest tonal banding for phases (`#000` at **0.02** opacity) — depth is conveyed by tonal
layering and opacity, not by shadow. Stickies sit on the plane with only a 1px translucent stroke
(`#0003`), no drop shadow. The single exception is the comment modal: a real lifted surface,
because it is a temporary instrument that floats above the bench.

### Shadow Vocabulary

- **Modal lift** (`box-shadow: 0 18px 50px #0003`): the only shadow in the system. Used solely on
  the `<dialog>`, paired with a `#0006` backdrop, to read as "above the work" while it's open.

### Named Rules

**The Flat-Bench Rule.** Surfaces are flat at rest. The only lift in the system is the modal, and
only because it is transient. If a card, panel, or button has a resting drop shadow, it is wrong —
convey grouping with the hairline rules and tonal bands instead.

## 5. Components

### Buttons

- **Shape:** gently rounded (7px control radius).
- **Secondary (default — Reload / Export / Plain):** surface-white (`#ffffff`) on a `#cfcfda`
  border, ink text, 5px 11px padding. The everyday controls; quiet.
- **Primary (Save comment):** ink-blue (`#1A6FAE`) fill, white text, same shape. Exactly one
  primary action per context — the live pen committing a note.
- **Hover / Focus:** secondary buttons shift to `#f2f2f7` on hover (150ms). Every control needs a
  visible `:focus-visible` ring; never rely on hover alone.

### Status Pill

- **Idle** (`checking…` / offline): `#eeeeff` bg, `#555577` text, fully rounded (999px).
- **Live** (server connected): `#e6f7ec` bg, `#1b7a3d` text. A reassurance signal, not a CTA —
  it states the connection, it isn't clickable.

### Inputs / Fields (comment modal)

- **Style:** surface-white, `#cfcfda` 1px border, 8px (card) radius, 8px padding. The `<select>`
  for comment kind and the `<textarea>` for the note share one vocabulary.
- **Focus:** border shifts toward the accent; a focus ring is mandatory. Textarea is vertically
  resizable only.

### Chips (relationship pills)

- **Style:** `#eef1f4` bg, `#334455` text, 6px radius, 1px 7px padding. The bold `id` inside a chip
  is ink-blue. They list a focused element's connectors — the keyboard-readable way to see edges.

### Signature Component — the Sticky

- **Shape:** 176×74px, 8px corners — **except hotspots, which are squared (2px)** as a non-colour
  type tell.
- **Fill:** the element's lane colour (domain grammar). Text is dark or white per lane for AA.
- **Anatomy:** a 9px/700 `id` watermark (0.6 opacity) top-left; a 12px/600 wrapped label; an
  optional 9.5px detail second line. A `resolved` hotspot desaturates (grayscale .85, .6 opacity)
  with a check, instead of shouting red.
- **States:** `:hover` strokes the card `#1a1a1a` and dims the rest of the board (the focus
  metaphor); `has-note` gives a 2.5px ink-blue ring; diff states add a dashed coloured outline and
  a corner badge. All transitions are 150ms and gated behind `prefers-reduced-motion`.

### Signature Behaviour — Hover-to-Focus

Hovering any sticky adds `.dim` to the board: its edges highlight (`#2f3c45`, full opacity, 2.4px),
its neighbours stay lit (`.adj`), and everything else recedes (edges → .07, stickies → .32). This
is the optical bench's focusing knob and the system's defining interaction. ←/→ while hovering
nudges the sticky one column — the keyboard-fast path.

Hovering also reveals the box's edit affordances as **individual bare ghost glyphs**, never a
floating toolbar (§6): a `+` (add) on the right edge, a `×` (remove) at the top-right corner, and a
speech-bubble comment at the top-left corner — each a single live-pen accent glyph, no chrome at
rest, anchored apart so it reads as three affordances, not a control cluster. The box itself carries
the rest: single-click focuses, double-click / F2 renames in place, a left/right drag (or ←/→) moves
it along its lane, and **`c`** opens its comment. This is the anti-Miro reading of "gestures on the
element" — the chrome stays calm while every edit stays direct.

## 6. Do's and Don'ts

### Do

- **Do** keep every saturated pixel in the domain grammar or the single accent `#1A6FAE`. The
  bench stays grey (`#fbfbfd` / `#ffffff` / instrument greys).
- **Do** give every lane/state a redundant non-colour signal — position, label, corner shape, or
  badge (the Three-Signal Rule). The hotspot's squared corner and the diff badges (`+ – ≠ →`) are
  the model to follow.
- **Do** use fixed px type sizes on a ~1.25 scale, the system sans for the whole working surface,
  and the serif nameplate for the board title alone (across HTML and SVG).
- **Do** convey depth with hairline rules (`#e0e0e6`) and 2% tonal bands; reserve the lone shadow
  (`0 18px 50px #0003`) for the modal.
- **Do** keep exactly one primary action (the ink-blue Save) per context; everything else is a
  quiet secondary control.
- **Do** ship a `prefers-reduced-motion` alternative for every transition, and a visible
  `:focus-visible` ring on every control.

### Don't

- **Don't** build a generic SaaS dashboard: no card grids, hero-metric tiles, gradient accents, or
  tracked-uppercase section eyebrows.
- **Don't** add heavy or branded chrome: no logos competing with the board, marketing gradients,
  decorative illustration, or glassmorphism.
- **Don't** drift toward Miro/FigJam maximalism: no floating toolbars, no panels everywhere, no
  ambient motion or collaboration-canvas overload. Density of *model* is welcome; density of
  *controls* is the enemy.
- **Don't** make it toy or childish: no bubbly oversized radii, comic fonts, or candy
  oversaturation. The sticky palette is a fixed domain grammar — don't read it as license to
  decorate the chrome.
- **Don't** put colour in the chrome or a resting shadow on any surface but the modal. If a control
  wants colour or lift to feel important, the hierarchy is wrong — fix the hierarchy.
- **Don't** use `clamp()`/fluid type or let colour be a lane's only signal.
