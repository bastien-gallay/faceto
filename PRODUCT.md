# Product

## Register

product

## Users

Software designers, architects, and developers running domain-modelling sessions —
event storming first, with C4 / story mapping / impact mapping on the roadmap. Their
context: mid-thought, often pairing with an LLM, working from a single typed JSON model
they iterate on between sessions. They are technically fluent (the tool installs as a
zero-dependency Rust binary and is driven from the CLI), comfortable editing a typed
file by hand, and they reach for `faceto` to *see* the model at a glance and to leave
notes that the next session acts on.

The job to be done: turn a typed model into a board you can read instantly, click any
element to attach a short note, and on reload see exactly what changed since you last
looked — so the loop "think → annotate → let the next session adjust the model" stays
tight and never loses the thread.

## Product Purpose

`faceto` renders a typed JSON model into an interactive HTML+SVG workshop board, either
static (`render`) or live with a sync comment sidecar and an in-page diff (`serve`). The
premise is "a simple typed file you think through with an LLM": identity, timeline, and
lane all live in the file; the board is a faithful, deterministic view of it.

Success looks like: the board disappears into the task. A user opens it, understands the
model in seconds, annotates without friction, and trusts the diff overlay to tell the
truth about what moved. The interface never competes with the model for attention, and
it never makes the user pause at a subtly-wrong control.

## Brand Personality

A calm instrument. Quiet, precise, and out of the way — a thinking tool you reach for
mid-thought. The model is always the subject; the UI is glass. Three words: **calm,
precise, faithful.** The emotional goal is quiet confidence — the feeling of a sharp,
dependable instrument, not a busy app demanding interaction.

Voice in UI copy is plain, lowercase-leaning, and direct ("showing what changed since
you last looked — Plain to clear"). It explains the mechanism in one breath and trusts
the user to be technical.

## Anti-references

The board should explicitly NOT look like any of these:

- **Generic SaaS dashboard.** No card grids, hero-metric tiles, gradient accents, or
  tracked-uppercase section eyebrows. The default AI-product look is the thing to avoid.
- **Heavy / branded chrome.** No big logos, marketing gradients, decorative illustration,
  or glassmorphism. Nothing that competes with the board for attention.
- **Miro / FigJam maximalism.** No busy toolbars, floating panels everywhere, ambient
  motion, or collaboration-canvas overload. Density of *controls* is the enemy; density
  of *model* is fine.
- **Toy / childish.** No bubbly rounded everything, comic fonts, or oversaturated candy
  palettes. This is a serious thinking instrument. (Note: the event-storming sticky
  colours are a fixed domain grammar, not decoration — they are not the toy tell.)

## Design Principles

1. **The model is the subject; the UI is glass.** Every pixel of chrome must justify
   itself against the board. When in doubt, recede.
2. **Faithful to the file.** The board is a deterministic, honest view of a typed model.
   `id` is identity, `col` is the timeline, `type` is the lane — the UI never invents or
   obscures these. The diff tells the truth about what changed.
3. **Reach-for-it-mid-thought.** Trivial to install (zero dependencies, offline) and
   trivial to act in — click to note, ←/→ to nudge, reload to see the change. Friction
   anywhere breaks the thinking loop.
4. **Earned familiarity over flair.** Standard affordances (native `<dialog>`, real
   focus states, keyboard paths) so a user fluent in good tools trusts it on sight. No
   invented controls for standard tasks.
5. **Calm under density.** A board can hold many stickies and edges; the interface stays
   legible through hierarchy, hover spotlighting, and restraint — never through removing
   information the user needs.

## Accessibility & Inclusion

- **WCAG 2.1 AA** as the floor: body text ≥4.5:1, large/bold text ≥3:1, against its
  actual background. Verify the muted greys used for labels, counts, and detail lines.
- **Full keyboard path.** The board already supports ←/→ to nudge a hovered sticky and a
  native `<dialog>` for comments; keep every action reachable without a mouse, with
  visible focus states throughout.
- **Reduced motion is honored.** Transitions already gate on
  `prefers-reduced-motion: reduce`; any new motion must ship a reduced-motion
  alternative (crossfade or instant).
- **Colorblind-safe lane grammar.** The 8-lane colour grammar (`actor`, `command`,
  `aggregate`, `event`, `policy`, `readmodel`, `external`, `hotspot`) must never be the
  *only* signal. Lane position and the element label already carry meaning; preserve a
  non-colour cue (lane, label, and/or shape) so colour-blind users are never lost. Diff
  states (added / removed / changed-moved) likewise need a non-colour tell.
