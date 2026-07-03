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
| F-narrative-skill | AI · participant | ☐ | **Now** | *(was F-mcp-narrative — reshaped by feature-torture 2026-07-02.)* Reverse-narrative / discovery skill: an LLM agent reads `event-log.jsonl` directly and proposes events through the existing `POST /comment` seam (server-side minting + guards + append mutex, all shipped). Prompt-ware only — **no new Rust**; the participation seams already exist (`serve.rs` re-reads the log per request, so agent appends show live). The on-thesis answer to "solo & stuck". Spec: [`docs/F-narrative-skill-spec.md`](docs/F-narrative-skill-spec.md); torture report: `.personal/feature-torture/reports/F-mcp-narrative.md`. |
| F-auto-genesis | CLI · migration | ☐ | **Now** | **Kill legacy mode.** `faceto serve model.json` **auto-runs genesis** — creates `event-log.jsonl` beside the model (folding a sibling `comments.jsonl`, counts reported: the shipped genesis path, `main.rs`) — then serves in **log mode only**; if the sibling log already exists, the log wins (it is truth, the model is derived) and the `.json` argument just redirects to it. Removes `serve.rs`'s entire legacy branch (`log_mode = false`: `comments.jsonl` appends, structural gestures stored as dead comments) — the "gesture lies" defect dies **by construction**, not behind a warning. `model.json` stays as a **read-only retrieval / bootstrap format**: `render` and `genesis` keep reading it purely; only serving (mutation) forces the log. Supersedes the legacy-mode-guard idea (F-region-frontiers torture + working note, 2026-07-03). Sequence **before** F-region-frontiers so the frontier client work never grows a legacy branch. |
| F-mcp-server | AI · interop | ☐ | Parked | std-only stdio JSON-RPC MCP server exposing read-log / propose-event tools. Spawned from the F-mcp-narrative torture (2026-07-02): redundant while the dogfood agent has file + shell tools. Revisit when a shell-less client (claude.ai, Claude Desktop) becomes a real usage context, or a second agent platform needs typed tool discovery. |
| F-multiplayer | collab · network | ☐ | Parked | Multi-collaborator over network + event reconciliation + user naming. Heaviest std-only lift; fixes *crowded*, not *solo* — out of slice until a real multi-user need appears. |
| F-format-interop | interop | ☐ | Parked | Import/export to known event-storming formats and visual tools (Excalidraw, Miro). Not felt pain today. |
| F-es-vocabulary | modelling fidelity | ☐ | Parked | Deeper pure event-storming vocabulary — parallel / recurrent events, out-of-lane elements, and two sticky types a real board reached for: **`timer` / `temporal`** (time-triggered policies) and **`process`** (stateful, longer-running workflows). Each is an additive lane in `LANES` + `colour` + `lane_prefix`. Open when the model can't express something a real session needs. Field feedback #13 §2. |
| F-ddd-process | DDD process | ☐ | Parked | Adjacent capabilities from the ddd-crew starter modelling process. Depends on F-container; open after it lands. |
| F-new-diagrams | new formats | ☐ | Parked | New diagram types: C4, User Story Mapping, BPMN. The long-term PRODUCT.md ambition; deferred until the event-storming board is excellent. |
| F-model-smells | linting | ☐ | Parked | Detect model smells — orphans, loops, heavy bounded-contexts. Needs the F-container primitive and a graph pass; open once grouping exists. |
| F-board-gestures | UI · direct edit | ✅ | **Now** | Richer on-element gestures layered over F-inline-add: **chromeless** bare ghost glyphs (`+` add · `×` remove · comment), not a floating toolbar (DESIGN §6); single-click focuses only (select-then-edit), double-click / F2 rename, drag left/right moves, `c` / comment glyph opens the modal. The modal then carries only prose actions, and `resolve` shows only on hotspots / open questions. Working note below; shipped 2026-07-01. |
| F-region-frontiers | model · grouping | ☐ | Next | *(reshaped by feature-torture 2026-07-03 — frontier core only.)* Regions as a **contiguous partition defined by shared *frontiers***, not independent `[fromCol, toCol]` spans. One primitive: resize = move a frontier (both neighbours re-border atomically, one `FrontierMoved` event), add = *split* a phase, remove = *merge* two, the outermost frontiers resize the **whole board**. Kills by construction the hole / overlap / unreachable-edge confusions of the independent-span model. New **additive** kinds (`FrontierMoved` / `PhaseSplit` / `PhaseMerged` — never repurpose `PhaseResized`); `replay` normalizes any log — legacy spans included — to a contiguous partition via one pure, deterministic rule. **Cut from v1:** the pivot / interstice column (a layout seam co-owned with F-lane-flow (c) and F-floating-hotspots — shape jointly; until then the frontier draws on the column boundary) and move-region-as-reorder (deferred to a candidate F-region-reorder until a real session needs it). Model-spine change (`events.rs` frontier semantics + client rebind). Follows F-container; surfaced by dogfood 2026-07-02. Working note below; torture report: `.personal/feature-torture/reports/F-region-frontiers.md`. |
| F-region-collapse | UI · legibility | ☐ | Later | Collapse / hide a region to concentrate readability: fold its stickies **and the edges that cross it** into a summarised band. Pure **view-state** — no model / event change — so it is orthogonal to F-region-frontiers and works under either border model. Surfaced alongside F-region-frontiers (dogfood 2026-07-02). |
| F-2d-placement | model · layout | ✅ | **Now** | Replace the rows / columns / grid **packing** (and its dark grey group box — a poor 2D representation) with a **stored 2D sub-position**: keep `x = col` (global timeline) and `type = lane` — both invariants — but give each element a free **Y within its lane band** instead of auto-packing. Removes the packing control entirely and fixes two dogfood bugs: moving within a stacked group force-**swaps** (can't re-insert without displacing the survivor), and moving from / into a group **superposes**. Model change — `ElementMoved` gains the sub-position. Absorbs feedback #1 / #3 / #4 / #10. Shipped 2026-07-02 (PR #17); as-built note below. |
| F-lane-flow | UI · legibility | ☐ | Next | Reorder the 8 lanes to the **canonical event-storming flow** (actor → command → aggregate / system → event → policy → … → read-model → UI → actor) so system and policy sit *near* events / commands, not at the bottom. Forks to shape: (a) reorder `LANES`; (b) **merge** adjacent lanes (aggregate+external, readmodel+policy) as an expandable *display grouping* — `type` still selects a pure lane, so the 8-colour grammar invariant holds; (c) alternate event / non-event **column cadence** — recoups the pivot / interstice column of F-region-frontiers, so shape together. Also shares the `LANES` / `colour` / `lane_prefix` seam with **F-floating-hotspots** (removes the hotspot lane) and **F-es-vocabulary** (adds `timer` / `process` lanes) — touch the lane set once, not three times. Feedback #2. |
| F-floating-hotspots | model · ES fidelity | ☐ | Later | Hotspots become **floating annotations attached to an element** (placed beside it, ES-canonical) rather than a bottom lane — removing `hotspot` from `LANES` (the shared lane-set seam with **F-lane-flow** and **F-es-vocabulary** — sequence together). Split the modal into two direct gestures: **`c` = comment**, **`h` = hotspot / open question**; drop **split** (add / rename / remove already cover it). Feedback #5 / #6. |
| F-frozen-headers | UI · legibility | ☐ | Later | Pin the **lane titles** to the left through horizontal scroll (condensable to initial + colour) and the **phase tabs** to the top through vertical scroll — frozen row / column headers. The board is one scrolling SVG, so this needs an overlay layer (or a split render), not plain `position: sticky`. Feedback #7 / #8. |
| F-commit-flow | UI · server flow | ☐ | Later | Replace the counterintuitive **Export comments / Reload** header actions with a single **Commit / Save** that re-baselines and clears the since-you-last-looked diff overlay. Framing: event-sourcing has **no server-side uncommitted state** (the log is truth, every edit is already appended), so "commit" = **re-baseline the diff view** (today's "Plain" button, reframed), not a write. The **Export** rethink shares the comments-representation seam with **F-comment-lifecycle** (which collapses the exported-array vs `comments.jsonl` duality) — reconcile the two. Feedback #11. |
| F-es-lint | linting | ☐ | Next | **ES-grammar linter** — `faceto lint` over the replayed `Model`, a pure graph pass, zero-dep. Rules validated by a real 147-element workshop (all 6 review comments were mechanical grammar defects): event with no producer, policy with no output, policy with no input, non-terminal event with no outbound edge. **Warn-only** (a big-picture board is legitimately incomplete — never a gate that breaks the calm loop), with an optional `level: big-picture \| design` strictness knob, and findings that can flow into the comment sidecar as resolvable entries (reuses serve→review→resolve). Distinct from **F-model-smells** (orphans / loops / heavy bounded-contexts) which needs F-container; this one needs only the graph. Field feedback #13 §3 — the headline item. |
| F-comment-lifecycle | comment · identity | ☐ | Next | Close the sidecar identity gaps surfaced at scale: deleting an element **orphans** its comments (needs cascade/tombstone in `replay`); resolving a comment needs a **gesture**, not hand-edited JSONL (likely a small serve endpoint + client button over the existing `HotspotResolved` / comments-as-events); collapse the **two comment representations** (exported array vs `comments.jsonl`) toward the log-is-truth spine; ID-rename sidecar migration is **reframed as guardrails/docs** (`id` is defined-stable — "never renumber, only add"), not tooling. The two-representations collapse meets **F-commit-flow**'s Export rethink — same seam. Field feedback #13 §5. |
| F-output-naming | CLI · output | ☐ | Next | Derive `board.svg` / `index.html` names from the **model basename** so sibling boards in one directory don't clobber each other. Small correctness win. Field feedback #13 §1. |
| F-cli-help | CLI · ergonomics | ☐ | Next | `--help` / `-h` per subcommand (`faceto render --help` currently treats `--help` as a file path); **plus `faceto <file>` defaulting to `serve`** — the primary action — while `render` / `genesis` / `compact` stay explicit. Small CLI-dispatch ergonomics in `main.rs`. Field feedback #13 §1 + dogfood #12 (reconciled). |
| F-png-docs | docs | ☐ | Later | **Document** the sanctioned SVG→PNG paths (`rsvg-convert` / `resvg` / headless Chromium) rather than build a rasterizer — PNG encoding + font rasterization is not feasible in pure std, so raster export stays a **deliberate non-goal** under the zero-dep constraint. A good idea, kept out of the binary. Field feedback #13 §1. |
| F-status-tracking | model · fidelity | ☐ | Later | Optional as-is / to-be **status field** on `Element` (additive) for mixed implemented/target boards, rendered as a visual state (e.g. dashed = target). Field feedback #13 §2. |
| F-typed-edges | model · fidelity | ☐ | Later | Give edges an optional **`type` / label** so connection kinds stop rendering identically. Additive; shape **with F-edge-routing** (which owns edge geometry) to avoid touching `edge_path` twice. Field feedback #13 §2. |
| F-tech-names | model · fidelity | ☐ | Parked | Optional **technical-name layer** distinct from the human label — before building, confirm it isn't `id` misuse. Field feedback #13 §2. |

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

## Working note — F-board-gestures (2026-07-01, branch `feat/F-board-gestures`)

**Scope.** Close the "Now" slice by making the box itself the edit surface, layered on the existing
F-inline-add / F-inline-edit / region-resize gesture engine. Client-only — every event
(`move` / `drop` / `rename` / `comment` / `resolve`) already exists end to end, so `render.rs` and
the server are untouched; the whole slice is `src/template.html`. No new Rust behaviour, hence no
new Rust tests — the gate stays green and the gestures are hand-verified on a live `serve` (the
F-container Stage 6 pattern).

**Two forks ratified before building.**

- **Affordance style (D1).** The roadmap line said "hover opens a small tool-button set," but
  `DESIGN.md §6` forbids floating toolbars. Chosen: **chromeless — individual bare ghost glyphs**
  (`+` add on the right edge, `×` remove top-right, a speech-bubble comment top-left), never a
  button row. DESIGN wins over the literal wording; the glyphs stay in the live-pen accent, no chrome
  at rest, and hide together when a drag starts.
- **Single-click (D2).** Chosen **focus / spotlight only** (select-then-edit, the calm gesture);
  comment relocates off the click to the **`c`** key + the comment glyph — the user's redirect,
  cleaner than either option first offered. So the click is benign, which let the disambiguation
  timer and the drag's `suppressClick` guard both go away.

**Gesture map (the contract).** single-click → focus · double-click / **F2** → rename in place ·
drag left/right (or ← / →) → move along the lane · **`c`** / comment glyph → the prose modal ·
**`×`** glyph / Delete → remove · `+` → add. The modal is now **prose-only**
(comment / split / open question / resolve); `resolve` is gated in `openModal` to a hotspot or an
element carrying an open `question`.

**As built.** `moveTo(id, targetCol)` was extracted from `doMove` first (tidy) so the arrow nudge
and the new drag share one move contract. Drag reuses the region-resize pattern — Pointer Events +
`setPointerCapture`, a 4px threshold below which a press is just a click, snap-to-column via the
rendered centres, and an occupied same-lane target swaps (both `ElementMoved` lines confirmed on the
wire). A small `graceGlyph` helper factors the fade / grace-travel / stashed-target plumbing so `×`
and comment are one line of wiring each. Hand-verification surfaced one fix — the comment glyph moved
from the left-centre edge (where it landed on incoming arrowheads) to the top-left corner.

**Out of scope.** Lane change via vertical drag (breaks `type` = lane); server-side enforcement of
the resolve-gating (a UI concern); any new event kind or model-spine change.

## Working note — F-region-frontiers (2026-07-02, design surfaced by dogfood)

**Root cause.** F-container shipped regions as **independent `[fromCol, toCol]` spans**. Dogfooding
the CISAC model surfaced the confusions that model allows: dragging one region's edge past a
neighbour's opens a **hole** or an **overlap**, and on overlap the underneath edge becomes
**unreachable** to grab. Minor as method, but very disorienting.

**The real fork (name it before coding).** The confusion is a symptom of an unnamed question — *what
is a region?*

- **Phase** (pivotal-event model): a **contiguous partition of the timeline**; holes / overlaps are
  impossible by construction. A *pivotal event is literally the frontier between two phases* — so a
  region boundary and a pivotal event are the same object.
- **Bounded context**: a semantic grouping where overlap is legitimate (two contexts can share
  stickies).

The dogfood instinct — "a region should always be present, holes shouldn't exist" — plus the pivotal
= frontier identity point to the **Phase** reading. Chosen: **Option A — contiguous partition,
frontier-based.** If overlapping bounded contexts are ever needed, that is a *second primitive*, not
a bent phase model.

**The unifying primitive — the frontier.** Defining a region by its shared frontiers (not
independent spans) collapses four gestures into one:

- **resize** = move a frontier → the two neighbouring phases re-border atomically (like a
  table-column boundary);
- **add** = *split* a phase at a column (the `+` glyphs left / right of a frontier in the interstice);
- **remove** = *merge* two phases (delete the frontier between them);
- **board ends** = the outermost frontiers have only one neighbour, so dragging them **grows /
  shrinks the whole board** (fixes "can't resize at the extremes").

**The pivot / interstice column.** To keep "one element per column", a frontier gets its **own
dedicated column** between the element columns. That interstice hosts, in one place: the frontier
itself, the region-operation glyphs, and — canonically — the **pivotal event** that marks the phase
boundary (materialises F-container's "derived pivotal"). Under A the frontier runs through the middle
of that column.

**The one geste that gets harder.** In a contiguous partition, **move-region = reorder** (this phase
now happens before / after that one), and it should **carry its content** — the stickies whose `col`
falls inside the region *at move time* (membership = spatial containment; regions never *own*
elements, they are `col` ranges). A compound operation, unlike the simple delta a span-move would be.

**Separable, do not bundle.**

- **F-region-collapse** is pure view-state (fold a region + hide its crossing edges); it rides on
  top of whichever border model and belongs to its own slice.
- **Legacy-mode guard (superseded 2026-07-03 → F-auto-genesis).** Region structural ops only apply
  in **log mode**; in legacy `model.json` mode `POST /comment` stores them as dead comments yet the
  gesture still reports success ("region resized"). The gesture *lies*. The guard idea (client
  learns the mode, refuses / warns) is superseded by the stronger call: **kill the legacy serve
  mode** — `serve` auto-runs genesis on a `model.json` and always operates on the log, so the lying
  state is unrepresentable. See the F-auto-genesis row (Now).

**Architecture note.** This is a **model-spine change** — `events.rs` needs frontier semantics
(evolved additively; a frontier move re-borders two phases atomically), `render.rs` needs the
interstice-column layout, and the client gestures rebind to frontiers. Not a template patch: shape it
(`/impeccable shape` or `feature-torture F-region-frontiers`) before any code.

**Shaped (feature-torture, 2026-07-03) — verdict ✂️, frontier core only.** v1 keeps the partition
semantics and drops two bundles from this note: the **interstice column** waits for joint shaping
with F-lane-flow (c) / F-floating-hotspots (until then the frontier draws on the column boundary —
the pointer-capture edge drag is already proven), and **move-region-as-reorder** is deferred until a
real session needs it. Top open question: the deterministic normalization rule `replay` applies to
legacy span logs with holes / overlaps. Full ADR + spec stub:
`.personal/feature-torture/reports/F-region-frontiers.md`.

## Working note — dogfood batch (2026-07-02): layout, lanes, hotspots, headers, commit

A second dogfood pass on the CISAC model produced twelve retours; they cluster into five slices
(above) plus two quick fixes. Recorded here so the reasoning and the invariant tensions survive.

**The through-line — three slices converge on one column.** The event / non-event **column cadence**
(F-lane-flow option c), the **pivot / interstice column** of F-region-frontiers, and **floating
hotspots** beside their element all want to place non-element material *between* the element columns.
Shape them together or they will repeatedly rework the same layout seam.

**F-2d-placement — the invariant guard.** "True 2D" must **not** become free-float: `col` is the
global timeline (x) and `type` is the lane (y) — domain invariants. The target is *stored Y within
the lane band* replacing *derived packing*, not position-anywhere. Keeping that line is what lets the
grey group box and the move-swap / superpose bugs go away without breaking the diff join (still keyed
on `id`) or the timeline.

**F-lane-flow — merge without breaking the grammar.** A merged lane (aggregate+external,
readmodel+policy) is a **display grouping**, not a new `type`: an element's `type` still resolves to
one of the eight pure lanes and keeps its colour; the merge only stacks two bands into one row that
can expand back. That preserves the "type selects the lane and colour" invariant while giving the
denser default the user wants.

**F-commit-flow — there is nothing to "save".** The event log is append-only truth; every gesture is
already persisted server-side the instant it posts. So a Commit / Save button cannot mean "flush
pending writes" (there are none) — it can only mean **re-baseline the client's since-you-last-looked
diff overlay**. That is today's "Plain" button with an intent-revealing name. Worth a rename +
rethink of Export (a power-user escape hatch, not a primary action), not a new persistence path.

**Two quick fixes (each carries a small fork).**

- **Duplicate title (#9).** The header `<b>` and the in-SVG serif nameplate (DESIGN.md §3, "the
  engraved maker's mark") both print the model title. Keep one. The header is always-visible and
  functional; the SVG nameplate is the treasured brand mark that scrolls away — DESIGN has a stake,
  so decide before cutting.
- **Serve by default (#12).** `faceto <file>` should launch `serve` (the primary action) instead of
  requiring the subcommand. A CLI-contract change in `main.rs` dispatch — small, but it changes the
  bare-argument meaning, so keep `render` / `genesis` / `compact` explicit. **Reconciled** into the
  CLI cluster → tracked on **F-cli-help** (with `--help` / F-output-naming, one `main.rs` pass).

## Working note — F-2d-placement (2026-07-02, branch `feat/F-2d-placement`, as built)

**The stored form.** `y` is an optional **fraction of the lane-band interior in `[0, 1]`** —
band-relative on purpose (the first shaping lock): it survives a lane merge (F-lane-flow b), a
region collapse, or any band-height change without remapping. It rides `ElementMoved` *and*
`ElementAdded` (both additive — an old log simply has no `y` and replays identically);
`ElementAdded` must carry it or `compact`/genesis would silently flatten a placed board. A
col-only move never resets a stored `y`. The fraction is clamped + rounded at the comment seam
(`events::clamp_y`) and clamped again at render, so an out-of-range log value can't draw off-band.

**Reshaped mid-dogfood: grid, not free canvas.** The first cut rendered `y` as a literal free
position; testing showed free vertical placement carries little meaning. As built, `y` is an
**ordering key**: a cell's members sort by it (unplaced = the neutral 0.5, barycenter tie-break)
and *everyone* renders on **row-slot centres** — a lone box sits on the classic mid-line whatever
its `y`, two sharing a cell split top / bottom and the lane grows a row to hold them. Same log
schema, same replay; only the render interpretation changed.

**Default without `y` = the old Rows stack.** Auto-stacked elements keep the barycenter ordering
(F-edge-routing Lever A) and the lane-height rule is unchanged (deepest cell) — which sidesteps
the fraction/band-height circularity a "grow to fit stored Ys" rule would create, and renders an
un-migrated log byte-identically.

**Packing is gone everywhere.** The `Packing` enum, `--pack`/`-k`, `?pack=`, the Rows/Columns/Grid
header control, the grey time-slot tray, and the sub-column machinery (`SUBCOL_W`, per-column
widths). **One col = one x slot** now holds unconditionally — the second shaping lock (zero
intra-cell X spread), and exactly the ground the F-region-frontiers interstice column assumes.
This also buries the stale "packing buttons don't switch" note under F-edge-routing.

**Gesture.** Drag is 2D: x snaps to columns as before, and the pointer's y (clamped to the lane
band — `type` = lane is untouchable) becomes an ordering key whose **preview snaps to the same
grid slots the commit will produce**: the client mirrors the renderer's cell-stack placement
(`computeGrid`, fed by the `data-y` keys render.rs emits), so a drop never "jumps" on the
authoritative re-render and legacy/offline replays land on the grid too. A drop posts the `y`
key **only when the target cell is shared**; into an empty cell it posts col-only, so the box
stays auto-placed. While the drag hovers a cell that would deepen the lane, a horizontal
**lane-growth guide** (`#lane-grow-guide`, the region-resize live-pen blue) marks where the
lane's bottom rule will land on release. ←/→ still posts col-only. The **force-swap is removed**
(dogfood bug #1): nothing is displaced, stickies sharing a cell are simultaneous and stack on
the grid. The server keeps *parsing* `swapId` so old logs and stashed offline moves replay
faithfully. Undo of a placement restores the prior key — the neutral `0.5` for a
previously-unplaced box, which `model::y_key` makes indistinguishable from "no y" — and a
y-only change diffs as `moved` through that same key, so a neutralised placement never reads
as a phantom move.

## Why this slice

Chosen by filtering all eight directions through felt dogfood pain. The three live
pains were clunky editing, an unreadable board, and losing momentum solo — **not**
thin modelling vocabulary, which dropped F-es-vocabulary, F-ddd-process,
F-new-diagrams, and F-model-smells out of the slice automatically.

Two deferred items are named on purpose:

- **F-container** is a hidden hub — UI bounded-context editing, F-model-smells, and
  F-ddd-process all silently depend on it, and the model has no container concept
  today. Cheap to add now, expensive to retrofit; build it when grouping is the pain.
- **F-narrative-skill** (né F-mcp-narrative) is the on-thesis answer to "solo & stuck"
  (faceto is "a simple typed file you think through with an LLM"). Reshaped 2026-07-02:
  the write seam an MCP server would expose already ships (`POST /comment` + per-request
  log re-read), so the slice is a skill, not a server — the server is parked as
  **F-mcp-server**. F-multiplayer stays parked because it solves a different problem —
  crowded, not solo.

## Working note — Field feedback triage (issue #13, 2026-07-02)

Source: field feedback from authoring + workshop-reviewing a **147-element / 186-edge /
48-column** two-bounded-context board through a full author → serve review → fix → resolve
loop ([issue #13](https://github.com/bastien-gallay/faceto/issues/13)). Highest-signal input
to date — the whole loop ran on a real board. Mapping of every item to a feature:

- **§1 CLI / Output** → **F-output-naming** (sibling clobber), **F-cli-help** (`--help`),
  **F-png-docs** (raster export).
- **§2 Model format** → **F-status-tracking** (as-is/to-be), **F-typed-edges** (untyped edges),
  **F-tech-names** (technical-name layer), **F-es-vocabulary** (timer / process sticky types).
  Bounded contexts → see pushback below.
- **§3 ES-grammar lint** → **F-es-lint** (the headline; warn-only + `level` + sidecar flow).
- **§4 Timeline at scale** → no new feature. Single-row-breaks-past-~20-cols is the concurrent-
  lifecycle problem already owned by **F-2d-placement** (free Y within a lane) and the region
  work (**F-region-frontiers** / **F-region-collapse**); wide-board back-edge readability is
  **F-edge-routing** + **F-region-collapse**. Re-scope those with the §4 evidence rather than
  add an ID.
- **§5 Comment lifecycle / identity** → **F-comment-lifecycle**.
- **§6 What worked well** → protect, don't build: the serve→review→fix→resolve loop, LLM-safe
  `model.json` transforms, the hotspot lane. Constrains **F-es-lint** to stay warn-only.

**Two pushbacks resolved (author call):**

- **Raster/PNG export is a genuinely good idea, but the zero-dep constraint holds.** Ship it as
  documentation of the sanctioned external paths (`rsvg-convert` / `resvg` / headless Chromium),
  not a built-in rasterizer → **F-png-docs**. Raster-in-binary is a deliberate non-goal.
- **Bounded contexts already shipped (F-container, PR #8–11) but not in the form this board
  needed** — the in-flight region rework (**F-region-frontiers**) improves the usable model, and
  a future MVP walkthrough + clear tool-usage docs are the real fix so this class of "I invented
  a convention because I didn't know it existed" comment stops surfacing. No new build; treat as
  a **discoverability / docs** gap, not a missing primitive.

New catalog rows are tagged *Field feedback #13*. Suggested first slice (value-to-effort, no
design debt): **F-es-lint** + **F-output-naming** + **F-cli-help**.

## Working note — Batch reconciliation (2026-07-02)

Two feedback batches landed the same day — the **dogfood batch** (#1–12, from re-reviewing the CISAC
board: five slices + two quick fixes) and the **field-feedback batch** (issue #13, from the
147-element workshop loop: eight rows). Reconciled into one set — no feature is a duplicate; the
overlaps are cross-referenced, not merged away:

- **CLI cluster.** Dogfood #12 (`faceto <file>` → `serve`) folded into **F-cli-help**, alongside
  **F-output-naming** — one `main.rs` dispatch pass.
- **Lane-set seam.** **F-lane-flow** (reorder / merge), **F-floating-hotspots** (removes the hotspot
  lane) and **F-es-vocabulary** (adds `timer` / `process` lanes) all mutate `LANES` / `colour` /
  `lane_prefix` — sequence them so the lane set is touched **once**.
- **Comment / export seam.** **F-commit-flow** (Export → commit re-baseline) and
  **F-comment-lifecycle** (collapse the exported-array vs `comments.jsonl` duality) meet at the same
  representation — reconcile together.
- **Already cross-mapped by #13 §4:** timeline-at-scale points at **F-2d-placement** (shipped),
  **F-region-frontiers** / **F-region-collapse**, and **F-edge-routing** — shared ground, no new ID.
- **Lint stays split:** **F-es-lint** (graph-only) is distinct from **F-model-smells** (needs
  F-container).
- **No #13 sibling:** dogfood #9 (duplicate title) stays a standalone quick fix.
