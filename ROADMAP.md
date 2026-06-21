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
| F-inline-edit | UI · direct edit | ☐ | **Now** | Rename / move / remove elements directly on the board; the comment box becomes optional, not the only path. Wires client gestures to the existing `ElementRenamed/Moved/Removed` events + server-side minting — high impact, low effort. |
| F-inline-add | UI · direct edit | ☐ | **Now** | Finish the inline add-element affordance → `ElementAdded` + server mint. Same low-effort substrate as F-inline-edit. |
| F-edge-routing | UI · legibility | ☐ | **Next** | Reduce edge crossings via a layout heuristic in `render.rs`. Self-contained, no model-spine change. Lower ceiling than grouping but cheap and immediate. |
| F-container | model · grouping | ☐ | Later | Add the missing bounded-context / container primitive as a readability device. The single brick that also unlocks F-model-smells and F-ddd-process. Build when grouping-legibility or linting becomes the felt pain. |
| F-mcp-narrative | AI · participant | ☐ | Later | MCP server (stdio JSON-RPC, std-only) + a reverse-narrative / discovery skill so an LLM reads the log and proposes events. On product-thesis; the real answer to "solo & stuck". Revisit when momentum, not legibility, ends sessions. |
| F-multiplayer | collab · network | ☐ | Parked | Multi-collaborator over network + event reconciliation + user naming. Heaviest std-only lift; fixes *crowded*, not *solo* — out of slice until a real multi-user need appears. |
| F-format-interop | interop | ☐ | Parked | Import/export to known event-storming formats and visual tools (Excalidraw, Miro). Not felt pain today. |
| F-es-vocabulary | modelling fidelity | ☐ | Parked | Deeper pure event-storming vocabulary — parallel / recurrent events, out-of-lane elements. Open when the model can't express something a real session needs. |
| F-ddd-process | DDD process | ☐ | Parked | Adjacent capabilities from the ddd-crew starter modelling process. Depends on F-container; open after it lands. |
| F-new-diagrams | new formats | ☐ | Parked | New diagram types: C4, User Story Mapping, BPMN. The long-term PRODUCT.md ambition; deferred until the event-storming board is excellent. |
| F-model-smells | linting | ☐ | Parked | Detect model smells — orphans, loops, heavy bounded-contexts. Needs the F-container primitive and a graph pass; open once grouping exists. |

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
