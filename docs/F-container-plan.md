<!-- markdownlint-disable MD013 -->

# F-container — build plan

Status: **plan (not started)** · Branch: `feat/F-container` · Companion:
[`F-container-scope.md`](./F-container-scope.md)

Build order is **model-first**: the pure brick (replay + diff) is testable with zero UI, and the
gestures are meaningless until replay and render carry regions. v1 ships model **and** on-board UI
in one PR, commit-staged in the order below. Tidy First: structural commits (add field, evolve
event) land separately from behavioural ones.

Membership and pivotal are **derived from geometry** (scope D2/D3) — so they cost **zero new
events** and **zero new fields**. That is the cheap part the plan leans on hard.

## Naming sub-decision (resolve at Stage 1)

D1 collapses *temporal phase* and *bounded context* into one band. The code already has a `Phase`
type. Recommendation: **keep the `Phase` type name internally, add an `id`, surface "region /
context" only in UI copy** — avoids a large mechanical rename diff while honouring D1. Flag for the
reviewer; flip to a `Region` rename only if the team prefers it.

## Stage 1 — Event spine (`events.rs`) · structural

- Add `id: String` to `PhaseAdded`. On read, `id` is **optional** (additive field path): replay
  assigns a synthetic stable `K<index>` when a legacy `PhaseAdded` carries none — old logs stay
  replayable, no `upcast` needed (not a renamed kind).
- Add three additive kinds: `PhaseResized { id, from_col, to_col }`, `PhaseRenamed { id, label }`,
  `PhaseRemoved { id }`.
- Wire each through `parse_event` / `to_json` / `replay` / `from_model` in lockstep — the match is
  compiler-enforced. Unknown kinds keep being skipped on read.
- Tests: log → replay → `to_json` round-trip carries region id; resize/rename/remove fold correctly;
  legacy `PhaseAdded` (no id) replays with a stable synthetic id.

## Stage 2 — Region model + derivation (`model.rs`) · structural + behavioural

- `Phase` gains `id: String` (and the diff annotations it will need: `diff: Option<String>`).
- Two pure helpers (no clocks/IO):
  - `region_of(model, col) -> Option<&Phase>` — the band whose `[from_col, to_col]` contains `col`;
    **innermost (smallest span) wins** on overlap (scope D2).
  - `is_pivotal(model, el) -> bool` — `el.kind == "event"` **and** `el.col` equals a region boundary
    col (scope D3). Events-only, derived, no stored flag.
- Tests: membership by col incl. overlap tie-break; pivotal true only for boundary-col events;
  non-event on a boundary is not pivotal.

## Stage 3 — Region diff (`model.rs::diff_models`) · behavioural

- Diff regions by stable `id`: `added` / `removed` / `renamed` / `resized` (bounds differ). Layout
  follows the **new** side, mirroring the element diff. Removed regions keep their old slot.
- Tests: each verdict pinned by id; a bounds-only change reads `resized`, a label-only change
  `renamed`.

## Stage 4 — Render outline (`render.rs`) · behavioural · ⚠️ read DESIGN.md + PRODUCT.md first

- Evolve the decorative phase-band block (≈`render.rs:627`) into a **thin labelled region outline**
  with a label tab — *not* a filled block competing with the 8-lane colour grammar (calm
  instrument; anti-reference: Miro maximalism).
- Draw **pivotal events on the border line** (derived via `is_pivotal`).
- Emit a grabbable border affordance (class / hit-region) for the resize handle — the *visual* half
  of D5; the interaction is Stage 6.
- Diff styling for `added` / `removed` / `resized` regions, consistent with element diff styling.

## Stage 5 — Mint + append (`serve.rs`) · behavioural

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

## Stage 6 — Client gestures (`template.html`) · behavioural · ⚠️ DESIGN.md register

Layered on the F-inline-edit / F-inline-add drag substrate:

- **Create region**: split on the timeline axis (`+` / divider) → `region-add`.
- **Resize**: drag the **region edge handle** (grab target = the band border, *not* an element →
  disambiguated from pivotal per D5) → `region-resize`.
- **Rename**: in-place edit of the label tab → `region-rename`.
- **Element across border**: the existing move gesture; membership + pivotal update is just a
  re-render (derived — no extra post).
- Offline `localStorage` fallback parity with existing structural ops (local-only, not resynced).

## Stage 7 — Example + roadmap · housekeeping

- Add a region (and a pivotal boundary event) to `examples/sample.model.json`; verify `genesis →
  render → serve` carries it end to end.
- Flip `F-container` status to ✅ in `ROADMAP.md` **inside this PR** (not a follow-up docs PR).

## Test gate (every stage)

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Behaviour not covered by tests (region outline, drag, pivotal placement) is verified by
`render`-ing the updated `examples/sample.model.json` and interacting via `serve`.
