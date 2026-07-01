# faceto — Roadmap

First direction, chosen 2026-06-20: **the board as a single-player instrument.**
Make editing direct and the board legible before adding any new participant (human
or AI) or new format. Horizons are dogfood-driven, not date-pinned — they shift as
real sessions surface the next felt pain. Source: `.personal/brainstorm/20260620-roadmap-first-direction.md`.

**Status:** ☐ todo · 🚧 in progress · ✅ done
**Horizon:** Now (this slice) · Next · Later (named, deferred) · Parked (not felt yet)

## Feature catalog

| ID | Direction | Status | Horizon | Summary |
| --- | --- | --- | --- | --- |
| F-inline-edit | UI · direct edit | ✅ | **Now** | Rename / move / remove elements directly on the board; the comment box becomes optional, not the only path. Wires client gestures to the existing `ElementRenamed/Moved/Removed` events + server-side minting — high impact, low effort. Shipped PR #4. |
| F-inline-add | UI · direct edit | ✅ | **Now** | Direct on-board element creation (the `add` substrate already exists end-to-end via the modal). Hover-element `+` and an empty-board affordance replace the modal dropdown's `add` option. Lane-only, client-only. Shipped PR #5. |
| F-edge-routing | UI · legibility | ✅ | **Now** | Reduce edge crossings via a layout heuristic in `render.rs`. Self-contained, no model-spine change. Two levers: barycenter within-cell ordering + fan-out edge anchoring (both ports kept in lockstep). Shipped PR #6. |
| F-container | model · grouping | ✅ | **Now** | The missing bounded-context / region primitive (vertical bands; spatial membership; derived pivotal). Model brick (PR #8), render (PR #9), serve mint/append (PR #10), client gestures (PR #11) — create/resize/rename a region directly on the board. Decisions + plan in [`docs/F-container-scope.md`](docs/F-container-scope.md) / [`docs/F-container-plan.md`](docs/F-container-plan.md). Unlocks F-model-smells and F-ddd-process. |
| F-mcp-narrative | AI · participant | ☐ | Later | MCP server (stdio JSON-RPC, std-only) + a reverse-narrative / discovery skill so an LLM reads the log and proposes events. On product-thesis; the real answer to "solo & stuck". Revisit when momentum, not legibility, ends sessions. |
| F-multiplayer | collab · network | ☐ | Parked | Multi-collaborator over network + event reconciliation + user naming. Heaviest std-only lift; fixes *crowded*, not *solo* — out of slice until a real multi-user need appears. |
| F-format-interop | interop | ☐ | Parked | Import/export to known event-storming formats and visual tools (Excalidraw, Miro). Not felt pain today. |
| F-es-vocabulary | modelling fidelity | ☐ | Parked | Deeper pure event-storming vocabulary — parallel / recurrent events, out-of-lane elements. Open when the model can't express something a real session needs. |
| F-ddd-process | DDD process | ☐ | Parked | Adjacent capabilities from the ddd-crew starter modelling process. Depends on F-container; open after it lands. |
| F-new-diagrams | new formats | ☐ | Parked | New diagram types: C4, User Story Mapping, BPMN. The long-term PRODUCT.md ambition; deferred until the event-storming board is excellent. |
| F-model-smells | linting | ☐ | Parked | Detect model smells — orphans, loops, heavy bounded-contexts. Needs the F-container primitive and a graph pass; open once grouping exists. |
| F-board-gestures | UI · direct edit | 🚧 | **Now** | Richer on-element gestures layered over F-inline-add: **chromeless** bare ghost glyphs (`+` add · `×` remove · comment), not a floating toolbar (DESIGN §6); single-click focuses only (select-then-edit), double-click / F2 rename, drag left/right moves, `c` / comment glyph opens the modal. The modal then carries only prose actions, and `resolve` shows only on hotspots / open questions. Working note below; decisions ratified 2026-07-01. |

## Working note — F-inline-edit (2026-06-20, branch `feat/F-inline-edit`)

**Root cause / scope.** Editing is *modal-only*: every rename/remove routes through the
comment dropdown. Move is already direct (← / →, Move ←/→). So this slice adds **direct
rename + direct remove** gestures and demotes the modal to "optional, not the only path".
Wiring a direct rename surfaces a latent defect: the `rename` arm of `comment_to_events`
(and `replay`) accepts a **blank label**, persisting a never-renumbered empty box — the exact
failure the `add` path already guards. Select-all → delete → Enter would trip it in one gesture.
The fix keeps the *non-blank-label* invariant in the Rust domain seam (not only in JS).

**Tests to done** (red first, then green):

- UT: `rename` rejects a blank/whitespace label (→ nothing to persist); trims surrounding space.
- PBT (std-only, hand-rolled): over random comment sequences, no element ever ends with a
  blank label via the comment seam; move/annotate preserve element cardinality & identity.
- Integration: a blank rename appends nothing to the log; a real one persists one `ElementRenamed`.
- Non-regression: move/swap, server-side mint, and `add`'s blank-guard stay green.

## Working note — F-inline-add (2026-06-21, paired)

**Scope (hardened, ratified).** `add` already works end-to-end through the comment modal's
dropdown: `serve.rs` `append_add` + server-side `mint_id` + the non-blank-label guard, all
tested. This slice makes add a **direct on-board gesture** and strips the modal's `add` option.
**Lane-only, client-only** — domains / bounded-contexts are explicitly *not* in scope (that is
**F-container**, which stays parked at Later; F-inline-add must not touch the model spine).

**Gesture (ratified, two affordances — `+left` dropped).**

- *Add after:* hover an element → a `+` appears on its **right** edge → mints in the **same lane**
  (`type`) at `anchorCol + 1`. This is byte-identical to today's modal `add` payload, so the whole
  server path is already written and tested.
- *Prepend / first element / empty board:* hover a **lane title** → a `+` → mints at the **left of
  that lane** via `model::lane_left_col(model, kind)`: a **first element of an empty lane** aligns
  to the board's existing first column (no rightward shift of the other lanes); a **prepend into a
  non-empty lane** marches one column further left (the renderer draws negative/sparse `col`
  on-board). Because the lane title is always present (see the render change below), this one
  affordance covers prepend-into-a-lane, the first element of a lane, **and** the empty-board
  bootstrap the modal cannot reach. (The non-empty-lane prepend feel is deferred for later.)
- *Modal:* remove the `add` option (and its now-dead `<select id="m-type">` lane picker). Modal
  stays prose-only — comment, hotspot resolve, rename, open question. No reshape.

**Render change (R, deliberate, accepted).** `render_svg_packed` currently builds `present` by
filtering `LANES` to lanes that *have* an element, so empty lanes — and the whole empty board —
draw no row or title. R makes all 8 lanes always render, so an empty board shows the lane scaffold
(onboarding for an event-storming beginner) and every lane title is hoverable. This is the one
non-client change in the slice. Regression surface: absolute-y render tests on sparse models
(e.g. `a_lone_sticky_stays_on_the_lane_mid_line`) shift and must be re-pinned; the dead lane-picker
test (`the_add_element_picker_offers_every_lane`) is removed with the `<select>`.

**`col` wrinkle — resolved by design, not by code.** Dropping `+left` removes the file-order
tie-break problem entirely; prepend uses a strict lane-minimum − 1, which sorts left unambiguously.

**Out of scope / parked:** F-container (domains) and the F-board-gestures future set (hover
tool-buttons, click-centre rename, drag-to-move).

## Working note — F-edge-routing (2026-06-27, branch `feat/F-edge-routing`)

**The locked-node constraint (the whole shape of this slice).** Node positions are *not*
free here: `col` is the global timeline (x) and `type` is the lane (y) — both are domain
invariants we must not break. So the textbook crossing-reducer (permute node positions) is
off the table. The only genuinely free levers inside `render.rs` are **(a) the order of
*simultaneous* stickies within a single `(lane, col)` cell** (`sub_ord`, today just file
order) and **(b) how edges anchor and route between fixed centres** (`edge_path`). This slice
spends both, and touches nothing in the event/model spine.

**Lever A — barycenter within-cell ordering.** For each crowded cell, sort its members by the
mean position of their edge neighbours, then assign `sub_ord` from that order (stable, file-order
tiebreak). Because a neighbour's *lane* is fixed, its vertical band is essentially fixed, so the
barycenter is computable in a **single deterministic pass** — no Sugiyama iteration, no clocks,
no randomness. Rows packing sorts sub-rows by neighbour **lane index**; Columns packing sorts
sub-columns by neighbour **col**. A lone sticky in a cell is unaffected (its classic mid-lane spot
holds). This is the part that removes *topological* crossings.

**Lever B — fan-out anchoring.** When several edges meet a box on the same side, they all anchor
at the box centre today and read as one fat bundle (e.g. `X1`→`C1` and `X1`→`C2` in the sample).
Generalise `edge_path` to take a small per-edge anchor offset so siblings spread along the facing
side. This is legibility polish (reduces visual *overlap*, not crossings), kept **subtle** per the
calm-instrument register in DESIGN.md — a few px of spread, never a starburst.

**Hard sync constraint (R).** `src/template.html` carries a JS port of `edge_path` (`edgePath`,
~line 211) used for the in-page move-nudge. Any change to the `edge_path` *signature/geometry* must
be mirrored there or the client nudge diverges from the authoritative server render. (In log mode
the server re-render lands moments later and corrects it; in legacy `model.json` mode the nudge is
the only feedback, so the ports must match.) Lever A changes only `sub_ord`/centres, which the
client already reads from the DOM — no JS change. Lever B changes `edge_path`'s signature — **must**
update `edgePath` too.

**Out of scope (deliberately).** Obstacle-avoidance routing (bowing a cross-lane edge around an
intervening sticky) is *not* in this slice: it needs every box's geometry on both server and client,
risks a busy non-calm look, and has a poor effort/payoff ratio. Park it; reopen only if dogfood
shows cross-lane edges genuinely getting lost under boxes.

**Known regression (accepted, not fixed).** Dogfooding this branch surfaced that the header
Rows / Columns / Grid packing buttons no longer switch the board. Cause unconfirmed — the server
renders each packing correctly (`packing_chooses_its_growth_axis` is green) and the client re-render
path rebuilds its position maps (`renderPack → bindStickies → readLayout`), so code inspection
didn't pin it on this slice. Left unfixed on purpose: packing is likely to be replaced soon by a
thin-positioning model, so investing in the three-mode control now would be wasted. Revisit only if
packing survives that change.

**Tests to done** (red first, then green):

- UT (Lever A): a two-member cell whose neighbours sit in opposite lane-bands orders so the
  upper-neighbour member takes the upper sub-row; a lone sticky keeps its mid-lane centre
  (re-pin / preserve `a_lone_sticky_stays_on_the_lane_mid_line`).
- UT (Lever A): ordering is deterministic and stable — equal barycenters fall back to file order.
- UT (Lever B): `edge_path` with offset 0 is byte-identical to today's path (no-regression on the
  common single-edge case); a non-zero offset shifts the anchor along the facing side only.
- Non-regression: absolute-y render tests on sparse models re-pinned; `diff` styling, hotspot
  dotted connector, and the JS `edgePath` port stay in lockstep (manual board check).

## Why this slice

Chosen by filtering all eight directions through felt dogfood pain. The three live
pains were clunky editing, an unreadable board, and losing momentum solo — **not**
thin modelling vocabulary, which dropped F-es-vocabulary, F-ddd-process,
F-new-diagrams, and F-model-smells out of the slice automatically.

Two deferred items are named on purpose:

- **F-container** is a hidden hub — UI bounded-context editing, F-model-smells, and
  F-ddd-process all silently depend on it, and the model has no container concept
  today. Cheap to add now, expensive to retrofit; build it when grouping is the pain.
- **F-mcp-narrative** is the on-thesis answer to "solo & stuck" (faceto is "a simple
  typed file you think through with an LLM"). F-multiplayer is parked because it
  solves a different problem — crowded, not solo.
