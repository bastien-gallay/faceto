# F-region-collapse — shaping plan

**Status:** shaping — **superseded in part by the 2026-07-03 feature-torture
(✂️ reshape).** v1 is column-fold only; the crossing-edge summarisation
described below (see "What 'folded band' means" step 3, and the reroute
tests) is **spun out to F-region-edge-fold** and is NOT in the first slice.
Read this plan for the column-fold mechanism; read
`.personal/feature-torture/reports/F-region-collapse.md` for the scope cut.
Horizon: Later. Register: UI · legibility.
**Author pass:** 2026-07-03 (Claude, from a cleared-context `F-region-collapse` prompt; user away).

## One-liner (from ROADMAP)

> Collapse / hide a region to concentrate readability: fold its stickies **and the edges
> that cross it** into a summarised band. Pure **view-state** — no model / event change —
> so it is orthogonal to F-region-frontiers and works under either border model.

## The core tension (and its resolution)

"Pure view-state" and "fold the layout into a summarised band" pull against each other,
because **layout is server-side** (`render::render_svg(&Model)` owns every x/y). You cannot
fold columns with client CSS alone — hiding stickies leaves their empty columns, so the board
does not actually get *shorter*, which is the whole point ("concentrate readability").

**Resolution — reuse the diff seam.** The server already re-renders on a *client-held view
parameter* that never touches the log: `GET /board.svg?base=<hash>` (serve.rs:335). Collapse is
the same shape:

- **View-state lives client-side** (localStorage, beside the offline-move stash — `LS_KEY`,
  template.html:254), a set of collapsed region ids: `{ collapsed: ["R2","R5"] }`.
- The client appends it to the SVG request: `GET /board.svg?collapse=R2,R5` (composes with
  `?base=` for diff-while-collapsed — same `query_get` parser, serve.rs:477).
- **The server re-lays-out** with those phases folded. No event, no log append, no `Model`
  mutation. `render_svg` gains a view argument (see "Signature" below); `replay`/`from_json`
  are untouched. This keeps the invariant the ROADMAP asserts: orthogonal to F-region-frontiers,
  works under either border model, because it operates on the *already-normalized* phase
  partition, not on how phases got their bounds.

This is the CUPID-idiomatic move: collapse reads exactly like the diff overlay the codebase
already trusts — a pure function of `(Model, ViewState) → SVG`, re-derived per request, never
persisted.

## What "folded band" means, concretely

A collapsed phase spans `[from_col, to_col]`. Folding it:

1. **Columns collapse to one summary column.** The timeline coordinate is global (`col`), so
   every lane compresses in lockstep across that span — the band stays a vertical slice, lanes
   still flow through it (DESIGN §4). Columns to the *right* of the band shift left by
   `(to_col − from_col)` so the board actually shortens. Elements keep their `id`/`type`/`col`
   in the `Model`; only the *rendered* x is remapped. (A pure `col → x` remap table, computed
   once before the draw loop at render.rs:496 `col_left`.)
2. **Stickies inside the band are hidden**, replaced by a count chip on the band tab
   (e.g. "▸ Payments · 12"). The region tab (render.rs ~673) becomes the collapse/expand toggle.
3. **Edges that cross the band** reroute to the band's two frontiers with a small count badge,
   instead of drawing through hidden nodes. Edges *wholly inside* the band vanish with their
   nodes. Edges with one end inside → anchor that end to the near frontier.

## Gesture

- **Toggle:** click a triangle disclosure on the region label tab (▸ collapsed / ▾ expanded),
  or `z` on a focused region tab. Mirrors the existing region-tab gesture cluster
  (rename/resize/remove already live there — template.html:627+). No modal.
- **Affordance:** collapsed bands read as a thin filled slice with the count chip; the calm
  instrument stays calm (no animation beyond a plain swap, matching the diff SVG swap).
- **Persistence:** collapsed-set survives reload (localStorage) but is **never** shared/committed
  — it is one viewer's reading lens, exactly like which diff base you are looking at.

## Signature / seam changes (minimal)

- `render.rs`: introduce a tiny `struct View { collapsed: Vec<String> }` (or `&[String]`) and
  thread it: `render_svg(&Model, &View)`. Default `View::none()` for `render`/`genesis` static
  output and the plain `GET /` page. Only `GET /board.svg` reads `?collapse=`.
- The `col → x` remap is the one real new piece of layout logic. Everything else (frontier
  positions, tab, rail) already keys off `col_left`, so remapping `col_left` propagates for free.
- `serve.rs`: one `query_get(query, "collapse")` → split on `,` → pass into `View`. Compose with
  the existing `base` branch (collapse the baseline model with the *same* view before diffing, so
  the overlay lines up).
- `template.html`: collapsed-set in localStorage; disclosure glyph on `.region-tab`; append
  `collapse=` to `svgUrl()` (template.html:250).

## Explicitly out of scope (v1)

- **No per-lane collapse** (only whole regions) — regions are the grouping primitive; lane
  collapse is F-lane-flow (b)'s merge-adjacent-lanes territory.
- **No stored/shared collapse** — resist the pull to make it an event. The moment it is shared
  it stops being view-state and needs conflict rules; that is a different (unrequested) feature.
- **No animation / smooth reflow** — plain swap. Calm instrument.
- **No collapse of the diff itself** — collapse composes with diff but does not change diff rules.

## Risks / things to pin before coding

1. **Edge-crossing count correctness.** The reroute+badge is the subtle part. Needs a pure unit
   test: given a model + collapsed set, the set of edges crossing each frontier is exact
   (endpoints inside vs outside the `[from_col,to_col]` span, in `col` space, before remap).
2. **Remap must stay a pure, deterministic function** of `(phases, collapsed)` — no clocks, same
   determinism bar as `replay`/`normalize`. Idempotent, order-independent over the collapsed set.
3. **Interaction with an out-of-range / clamped band.** Collapse uses the *clamped* bounds the
   region already renders with (render.rs:643 note), not raw stored bounds, so a band past the
   last element column collapses to nothing visible rather than desyncing.
4. **Nested/adjacent collapsed bands** must compose (two adjacent collapsed regions → two chips,
   not a merged one) since regions are now a contiguous partition (F-region-frontiers).

## Tests to done (red first)

- UT (render, pure): `crossing_edges(model, "R2")` returns exactly the edges with one endpoint
  in-span and one out-of-span; wholly-inside and wholly-outside excluded.
- UT (render, pure): `col_remap(phases, collapsed)` shifts right-of-band columns left by the
  folded width; is idempotent and order-independent; empty collapsed set = identity.
- UT (render): a collapsed region emits a count chip = number of in-span elements; no hidden
  sticky `<g>` is rendered inside the band.
- Integration (serve): `GET /board.svg?collapse=R2` renders shorter than plain; `?collapse=`
  empty = identity; `?collapse=R2&base=<hash>` still produces a diff overlay.
- Non-regression: static `render`/`genesis` output byte-identical (View::none default path).

## Sequencing note

Independent of everything Now/Next except that it *reads* the F-region-frontiers partition, which
shipped (PR #25). Safe to build any time. Small, self-contained (render.rs + serve.rs + template.html;
no model/event/replay change) — a good "Later" slice to pull forward when board legibility next
bites in a dogfood session. Recommend **feature-torture before coding** only if the edge-reroute
band turns out contentious in review; otherwise this plan is buildable as-is.
