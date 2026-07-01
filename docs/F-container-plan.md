<!-- markdownlint-disable MD013 -->

# F-container — build plan

Status: **all seven stages done — F-container ships on branch `feat/F-container-client-gestures`
(PR #11), flipping the feature to ✅ in `ROADMAP.md`** · Companion:
[`F-container-scope.md`](./F-container-scope.md)

Build order is **model-first**: the pure brick (replay + diff) is testable with zero UI, and the
gestures are meaningless until replay and render carry regions. v1 ships model **and** on-board UI
in one PR, commit-staged in the order below. Tidy First: structural commits (add field, evolve
event) land separately from behavioural ones.

Membership and pivotal are **derived from geometry** (scope D2/D3) — so they cost **zero new
events** and **zero new fields**. That is the cheap part the plan leans on hard.

## Session handoff — resume at Stage 4

**Done & merged to `main` (PR #8):** Stages 1–3, the pure model brick.

- **S1** `events.rs` — `Phase.id` + additive `PhaseResized`/`PhaseRenamed`/`PhaseRemoved`, wired
  through `parse_event`/`replay`/`from_model`/`to_json`. Legacy id-less bands replay to a synthetic
  `K<n>`.
- **S2** `model.rs` — `region_of(col)` (spatial membership, innermost wins) and `is_pivotal(el)`
  (event on a band edge). Both `pub`, both `#[allow(dead_code)]` **until Stage 4 consumes them**.
- **S3** `model.rs` — `Phase.diff` field + `diff_phases` (added/removed/renamed/resized by id).
- **Review (medium) ran on the brick — all 4 findings resolved:** id-mint now reserves the
  highest-ever `K` suffix via `model::resolve_region_id` (single source of truth, shared by replay +
  `from_json`); 78 tests green; `fmt`/`clippy` clean.

**Resume here — Stage 4 (render). First moves, in order:**

1. Branch fresh off `main` (e.g. `feat/F-container-render`); the old `feat/F-container` is merged.
2. **Read `DESIGN.md` + `PRODUCT.md` first** — calm-instrument register; a region is a *thin
   labelled outline*, never a filled block.
3. Touch `render.rs` ≈`:629` (the phase-band block). Consume the ready-made helpers `region_of` /
   `is_pivotal` and the `Phase.diff` field; **drop the two `#[allow(dead_code)]` attributes** in
   `model.rs` once a non-test caller exists.

**Two carry-over review items (already pinned below):**

- **#3** Stage 5's server mint must share the `K<n>` namespace with replay's synthetic ids
  (reserve removed/synthetic suffixes) — see `F-container-scope.md`.
- **#4** Stage 4 must read `Phase.diff` or a removed region renders as a phantom unstyled band —
  pinned in Stage 4 below.

## Naming sub-decision (resolve at Stage 1)

D1 collapses *temporal phase* and *bounded context* into one band. The code already has a `Phase`
type. Recommendation: **keep the `Phase` type name internally, add an `id`, surface "region /
context" only in UI copy** — avoids a large mechanical rename diff while honouring D1. Flag for the
reviewer; flip to a `Region` rename only if the team prefers it.

## Stage 1 — Event spine (`events.rs`) · structural · ✅ merged (PR #8)

- Add `id: String` to `PhaseAdded`. On read, `id` is **optional** (additive field path): replay
  assigns a synthetic stable `K<index>` when a legacy `PhaseAdded` carries none — old logs stay
  replayable, no `upcast` needed (not a renamed kind).
- Add three additive kinds: `PhaseResized { id, from_col, to_col }`, `PhaseRenamed { id, label }`,
  `PhaseRemoved { id }`.
- Wire each through `parse_event` / `to_json` / `replay` / `from_model` in lockstep — the match is
  compiler-enforced. Unknown kinds keep being skipped on read.
- Tests: log → replay → `to_json` round-trip carries region id; resize/rename/remove fold correctly;
  legacy `PhaseAdded` (no id) replays with a stable synthetic id.

## Stage 2 — Region model + derivation (`model.rs`) · structural + behavioural · ✅ merged (PR #8)

- `Phase` gains `id: String` (and the diff annotations it will need: `diff: Option<String>`).
- Two pure helpers (no clocks/IO):
  - `region_of(model, col) -> Option<&Phase>` — the band whose `[from_col, to_col]` contains `col`;
    **innermost (smallest span) wins** on overlap (scope D2).
  - `is_pivotal(model, el) -> bool` — `el.kind == "event"` **and** `el.col` equals a region boundary
    col (scope D3). Events-only, derived, no stored flag.
- Tests: membership by col incl. overlap tie-break; pivotal true only for boundary-col events;
  non-event on a boundary is not pivotal.

## Stage 3 — Region diff (`model.rs::diff_models`) · behavioural · ✅ merged (PR #8)

- Diff regions by stable `id`: `added` / `removed` / `renamed` / `resized` (bounds differ). Layout
  follows the **new** side, mirroring the element diff. Removed regions keep their old slot.
- Tests: each verdict pinned by id; a bounds-only change reads `resized`, a label-only change
  `renamed`.

## Stage 4 — Render outline (`render.rs`) · behavioural · ✅ done (branch `feat/F-container-render`)

- Evolve the decorative phase-band block (≈`render.rs:627`) into a **thin labelled region outline**
  with a label tab — *not* a filled block competing with the 8-lane colour grammar (calm
  instrument; anti-reference: Miro maximalism).
- Draw **pivotal events on the border line** (derived via `is_pivotal`).
- Emit a grabbable border affordance (class / hit-region) for the resize handle — the *visual* half
  of D5; the interaction is Stage 6.
- Diff styling for `added` / `removed` / `resized` regions, consistent with element diff styling.
  - ⚠️ **Review #4 (pinned):** `render.rs` (≈`:629`) currently reads only `label`/`from_col`/`to_col`
    and ignores `Phase.diff`. Since Stage 3 now feeds *removed* regions into `model.phases`, a removed
    band would render as a **phantom unstyled band** in a diff overlay until this stage styles it.
    Latent today (no version produces region-differing models until Stage 5 UI), but this stage must
    read `Phase.diff` and style/omit removed bands explicitly.

**As built (S4):** the band is an open "⊓" — a top rule + two grabbable vertical edges + the lone
DESIGN-sanctioned 2% tonal wash, with a quiet folder-tab carrying the name (Bench-Is-Grey: no domain
colour). Region geometry now reads the region's own `[from_col, to_col]` (clamped into the visible
column range), *not* element positions, so empty/removed regions still render. Each edge emits a wide
transparent `class="region-edge" data-region data-edge` hit-line for the Stage-6 resize grab (removed
regions omit it). Pivotal nodes are `●` on the edge line at the event-lane centre, gated by
`is_pivotal`. Diff maps onto the element vocab via `phase_diff_kind` (renamed→`≠`, resized→`→`);
removed regions render inside a `<g opacity="0.45">` ghost. `is_pivotal` lost its `#[allow(dead_code)]`
(now consumed); **`region_of` keeps its `#[allow(dead_code)]`** — render derives membership from
geometry directly, so `region_of`'s first real caller is Stage 5/6 (serve/client), not render.

## Stage 5 — Mint + append (`serve.rs`) · behavioural · ✅ done (branch `feat/F-container-render`)

- Region id **mint namespace** `K<N>`, computed under the `appends` lock like `mint_id` — scan
  `PhaseAdded` history for the next free suffix (removed-but-not-compacted ids stay reserved).
  Generalise `mint_id`/`append_add` to take a prefix, or add a sibling `mint_region_id`.
- Extend `comment_to_events` + the `POST /comment` mapping with region commands: `region-add` →
  `PhaseAdded` (minted id), `region-resize` → `PhaseResized`, `region-rename` → `PhaseRenamed`,
  `region-remove` → `PhaseRemoved`.
- **Membership needs no route** — moving an element across a border is the *existing* `ElementMoved`
  (col change). Pivotal needs no route — both fall out of geometry on the next render (D2/D3).
- Tests: region mint picks next free `K` suffix; concurrent region adds never collide;
  `comment_to_events` maps each region command.

**As built (S5):** added a sibling `mint_region_id` (review #3) rather than generalising `mint_id` —
it folds the log's `PhaseAdded` history through `model::resolve_region_id`, the exact function
`replay` uses to synthesize legacy ids, so a fresh mint shares that namespace by construction (a
region id can never collide with one replay would later synthesize for the same log). `append_region_add`
mirrors `append_add`: mint + write under the `appends` lock. `region-add` is special-cased in the
`POST /comment` dispatch (like element `add`) since it needs the server-minted id; `region-resize`/
`region-rename`/`region-remove` go through `comment_to_events`, keyed by a `regionId` field (a region
is not an element, so it doesn't share `elemId`). 6 new tests; 86 total, gate clean.

## Stage 6 — Client gestures (`template.html`) · behavioural · ✅ done (branch `feat/F-container-client-gestures`)

Layered on the F-inline-edit / F-inline-add drag substrate:

- **Create region**: split on the timeline axis (`+` / divider) → `region-add`.
- **Resize**: drag the **region edge handle** (grab target = the band border, *not* an element →
  disambiguated from pivotal per D5) → `region-resize`.
- **Rename**: in-place edit of the label tab → `region-rename`.
- **Element across border**: the existing move gesture; membership + pivotal update is just a
  re-render (derived — no extra post).
- Offline `localStorage` fallback parity with existing structural ops (local-only, not resynced).

**As built (S6):** `render.rs` grew three pieces of markup to give the client something to hang
gestures on, since Stage 4 drew regions decoratively only: (1) a **region rail** — one invisible
per-column hit-rect, painted *before* the regions so a live region's own rect/edges/tab paint over
it and stay clickable, and it only "shows through" (hoverable) in the gaps between/around regions —
exactly the create-region affordance, with no client-side membership math; (2) each region wrapped
in a `<g class="region" data-region data-from-col data-to-col>`, so a resize drag can read the
*fixed* other edge straight off the DOM instead of inverting screen pixels back to a column; (3)
the label tab wrapped in a focusable `<g class="region-tab" role="button" tabindex="0"
data-label>` (mirrors the sticky pattern) as one rename hit-target.

`template.html`: `region-add`/`region-rename` reuse the *existing* `adding`/`renaming` state objects
and the same floating `#rename-edit` input as element add/rename (tagged `region: true`/keyed by
`regionId`) rather than duplicating the editor — one inline-edit substrate for both element and
region text entry. Resize is a real mouse drag: `mousedown` on a `region-edge` starts it, a thin
`#region-drag-guide` line follows the cursor snapped to the *exact* rail-cell boundary the server
itself renders (`readRegionRail` reads `x`/`width` straight off the `.region-rail` DOM — no
pixel-to-column guessing), and the drag clamps so the moving edge can never cross the fixed other
edge (client-side `events::valid_span` parity — never even offers an invalid target). `STRUCTURAL_KINDS`
and a new `NOT_APPLIED_OFFLINE` set give the four region kinds the same offline-fallback treatment as
`add`/`drop` (stashed locally, not applied to the board, `Export` to keep). 89 tests green (2 new
render tests: region-group/tab/edge markup, and the region-rail covering every visible column even
with zero regions); the drag/hover/rename/create gestures themselves have no Rust test surface and
were verified by hand — `faceto serve` against a copy of `examples/event-log.jsonl`, driving
rename (K1 → "Kickoff"), create (`region-add` at the rail, minted `K3`), and resize (`region-resize`
K2 via a simulated drag) end to end through the real HTTP server, each producing the correct
`PhaseRenamed`/`PhaseAdded`/`PhaseResized` log line and re-rendering the diff overlay correctly.
No client-side gesture exists yet for `region-remove` (not required by this stage's bullet list;
the server route has existed since Stage 5).

## Stage 7 — Example + roadmap · housekeeping · ✅ done (branch `feat/F-container-client-gestures`)

- Add a region (and a pivotal boundary event) to `examples/sample.model.json`; verify `genesis →
  render → serve` carries it end to end.
- Flip `F-container` status to ✅ in `ROADMAP.md` **inside this PR** (not a follow-up docs PR).

**As built (S7):** `examples/sample.model.json` already carried both a region *and* a pivotal
boundary event from before F-container existed — `begin`[0,1]/`work`[2,4], with `E1` (DayStarted)
sitting on `begin`'s `to_col` and `E2` (ItemAdded) on `work`'s `from_col` — so this stage added no
contrived third region purely to tick the checklist; genesis→render→serve already carried it (the
board's own screenshots throughout Stages 4–6 are proof). What the two phases lacked was an
explicit `id` — every element in the file already carries one, but the phases relied on
`resolve_region_id`'s synthetic-id fallback. Gave them `"id": "K1"`/`"K2"` so the canonical example
matches the "id is the stable identity, never derive from position" invariant the same way every
element does, and re-verified `render`/`genesis`/`serve` all carry the explicit ids through
unchanged (`data-region="K1"`/`"K2"` in the rendered SVG, `PhaseAdded{id:Some("K1"),...}` in the
genesis batch). `examples/event-log.jsonl` — the independently-evolving, already-diverged tracked
log — is untouched: it isn't re-derived from `model.json` on each change (that log has its own
history since genesis, per the event-sourcing spine), and it already renders/serves K1/K2 correctly
(verified by hand in Stage 5/6). `ROADMAP.md` flipped to ✅ in this same PR.

## Test gate (every stage)

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Behaviour not covered by tests (region outline, drag, pivotal placement) is verified by
`render`-ing the updated `examples/sample.model.json` and interacting via `serve`.
