# F-spike-canvas — where event storming is welded into the kernel

Throwaway spike for [#114], dated 2026-07-26. **This note is the deliverable. The code is not** —
it lives only on [`spike/f-spike-canvas`][branch] at `ceda0cb`, which is **never merged and never
rebased**: it is pinned to the state of the kernel these findings were measured against, and
rebasing it onto a later `main` would quietly invalidate every claim below.

So every `src/spike_canvas/…` path in this note refers to **that branch**, not to this tree. The
commands below only run there too.

**What was built.** A second board format — the DDD-crew [Bounded Context Canvas][bcc] — in
`src/spike_canvas/` (`model` / `events` / `diff` / `render`, ~1200 lines with tests), wired into
`faceto render` and `faceto genesis` by filename convention (`*.canvas.json`,
`*.canvas.event-log.jsonl`). It parses, replays from a log, genesis-migrates, diffs against a
baseline, and renders SVG. `fmt` / `clippy -D warnings` / 214 tests green on that branch. Try it:

```bash
faceto render  examples/orders.canvas.json
faceto genesis examples/orders.canvas.json     # → examples/orders.canvas.event-log.jsonl
faceto render  examples/orders.canvas.event-log.jsonl
faceto render  after.canvas.json --base before.canvas.json
```

**What was not built,** per the timebox: `serve` routes, the client, lint. Those questions are
answered on paper below and flagged as such.

**Why this format.** The BCC is a *slot template*: ten fixed named sections holding short lists.
No `col`, no lane, no `y`, no phase, no edges, no timeline. Layout is a constant table, so the
spike measures the seam and not the drawing.

[#114]: https://github.com/bastien-gallay/faceto/issues/114
[branch]: https://github.com/bastien-gallay/faceto/tree/spike/f-spike-canvas
[bcc]: https://github.com/ddd-crew/bounded-context-canvas

## The four questions, answered

### 1. Which of `col` / `lane` / `y_key` / `phase` / `is_pivotal` refuse to stay out of the kernel?

**None of them.** All five stayed out without a fight, and that is the spike's most reassuring
result: the ES coordinate vocabulary is already confined to `model.rs` + `render/`, and a format
that needs none of it simply never imports them. `spike_canvas::model` stores **no coordinate of
any kind** — an `Item` is `{ id, slot, text, via }`, and `Slot` is a closed enum, not a position.

The one `col` in the spike is a *render-local* grid column index in `render.rs`, read out of the
constant `GRID` table and multiplied into an `x`. It is never stored, never authored, never in the
log, and never diffed — which is exactly the distinction ES loses: there, `col` is simultaneously
the layout coordinate, a model field, an event payload, a diff key and a lint input.

What *did* leak is smaller and different in kind:

| leak | where | severity |
| --- | --- | --- |
| `model::Model` is the only board type every `main.rs` signature names | `cmd_render`, `warn_if_empty`, `render_diff`, `render::render_svg`, `render::render_html` | **structural** — forced a fork, not a branch |
| `render::text::{esc, wrap}` are pure utilities locked inside `render` | `src/render/mod.rs` | trivial — one-line visibility widening |
| `render::style::diff_colour` / `diff_badge` are `pub(crate)` to `render` | `src/render/style.rs` | trivial — copied 8 lines |
| the log framing is generic policy typed on `Event` | `src/events/log.rs` | **the one real extraction** — see Q2 |

The `main.rs` leak is the sealed-`enum Board` argument made concrete. Nothing between the CLI and
the SVG string is polymorphic, so the canvas path is a `return cmd_render_canvas(args)` at the top
of `cmd_render`. That is *acceptable* for two formats and untenable for three.

### 2. Does `replay` generalise, or does the `Event` enum fork per format?

**It forks — completely.** `CanvasEvent` shares **no variant** with `events::Event`. Not one.
`PhaseAdded` / `FrontierMoved` / `PhaseSplit` / `HotspotResolved` are pure ES;
`ElementMoved { col, kind, y }` has no canvas counterpart; `ItemReslotted { slot }` has no ES one.
`LogCompacted` is the sole *coincidence* (both formats want a provenance marker), and it is a
kernel concern, not shared vocabulary.

But the split is clean, and it is not where the note guessed:

- **Vocabulary is format-owned** — the `Event` enum, `parse_event`, `to_json`, `replay`.
- **Journal policy is kernel** — and is currently unreachable. `events::log` already implements
  "blank lines skip / bad JSON is fatal / *unknown kind* skips / *known* kind with a bad field is
  fatal", plus the `upcast` seam and `jsonl_records`. Every line of that is format-agnostic, and
  every line of it is typed on `Event` and `pub(crate)` to `events`. The spike had to **copy it**
  (`spike_canvas::events::parse_log`), and two copies drift. Extraction is cheap: the policy needs
  only `parse: impl Fn(&Json) -> Option<E>` and `is_known_kind: impl Fn(&str) -> bool`.
- **`replay`'s *shape* transfers, its *closing pass* does not.** ES's `replay` ends in
  `normalize(&mut phases)` — an invariant-restoring sweep over a coordinate space. The canvas
  replay has no post-pass at all, because a slot template has no invariant a single event can
  break. Any generic `replay` must therefore be a fold *plus a format-supplied closing pass*, not
  a fold alone.
- **`compact` is genuinely generic**: it is `marker + from_<board>(replay(log))` for any format.

### 3. Does the diff survive when "moved" has no meaning?

**The join survives; the verdicts do not.**

| ES verdict | canvas | why |
| --- | --- | --- |
| `added` / `removed` | same | pure set membership on the stable `id` |
| `changed` (label differs) | same | one text field per item |
| `moved` (col / kind / `y_key`) | **gone** | there is no coordinate to differ |
| — | **new: `reslotted`** | an item changed section — categorical, not spatial |
| region `resized` / `renamed` | gone | no regions, no bounds |
| edge `added` / `removed` | gone | no edges at all |

`join_by_id` extracted verbatim and is in `src/spike_canvas/diff.rs` as the kernel helper the
architecture note predicted; its test joins bare `&str`s with no canvas type in sight.

**`reslotted` is not `moved` renamed.** `moved` reports a position the viewer can compare on the
board ("col 4 → col 7"); `reslotted` reports a *category* and reads as a semantic
re-classification ("PlaceOrder was inbound, is now outbound") — closer to ES's `changed` than to
its `moved`. It even earns a different badge (`⇄`, a swap, not `→`). This is the rule of two
resolving *against* extraction: the second example disagrees with the first rather than
generalising it. **Do not build a generic verdict engine.**

One thing the ES diff hides, which the canvas inherited by copying the mistake on purpose:
`diff_models` returns a `Model`, so one type means both "the board" and "an overlay". Doing the
same here means the canvas renderer branches on "board or overlay?" at every mark, exactly as
`render::svg` does. **Splitting board from overlay is a pre-requisite for either format, not a
multi-format concern** — it would pay for itself with one format.

### 4. What does the client need that `template.html` cannot give without the Scene IR?

Not built; argued from the code. The spike writes **no `.html`** — `render::render_html` wraps an
SVG in `template.html` plus the nine ES client modules, and reusing it would ship a client that
can only mis-handle a canvas. Concretely:

- **`__CONFIG__` is ES geometry.** `colW`, `stickyW/H`, `rowPitch`, `laneVpad`, `regionTabH/CharW`
  — every value is a timeline-and-lane measurement. A canvas has section boxes of computed height
  and no pitch at all. The config is the seam and it is 100% format-specific.
- **Every gesture is a coordinate gesture.** Drag→`col`/`y`, lane-title `+`→`lane_left_col`,
  region frontier drag, connect→`EdgeAdded`. The canvas's *entire* interaction vocabulary is
  "type text into a section" and "drag an item to a different section". `drag.js`, `connect.js`,
  `region.js` and `layout.js` have nothing to contribute; `sync.js` (fetch, offline queue, version
  swap) and the modal have everything.
- **The Rust↔JS geometry mirror is the thing the Scene IR deletes.** `edgePath` / `fanOffsets` /
  `computeGrid` exist in JS because the client must recompute positions the server already knew.
  With a data `Scene` carrying positions and `data-*`, the canvas needs *no* geometry JS — and
  neither does ES.

So the split the architecture note proposes is confirmed, with one sharpening. Counting the nine
modules by line (1597 total): `layout` (222) + `connect` (207) + `region` (199) + `drag` (138) =
**766 lines, 48% of the client, is coordinate and gesture code** a canvas would need none of —
and most of it exists only to mirror geometry the server already computed. `core` (55) + `sync`
(192) + `graph` (133) are the portable third; `edit` (246) and `main` (205) are mixed.

**Do not plan to *split* `layout.js`/`drag.js` per format; plan to delete them.** They are not the
portable-vs-format question — they are the Scene IR question.

## The constraints list (what this spike locks in)

1. **The format tag is a safety mechanism, not a convenience.** Hand a canvas log to
   `events::parse_log` and it returns **`Ok`** with an empty `Model` — every line is an "unknown
   kind", and skipping unknown kinds *is* the forward-compatibility rule. The reverse holds too;
   both are pinned by tests in `spike_canvas::events`. Forward compatibility and format
   discrimination are the same mechanism aimed in opposite directions, so **nothing today
   distinguishes "a log from a newer faceto" from "a log from a different format"**, and
   `warn_if_empty` would report the wrong-format board as merely empty. Ship the genesis
   `BoardFormat` header *before* the second format, and make an absent tag mean `event-storming`.
2. **Dispatch must read the log's contents, not its filename.** The spike used
   `*.canvas.*` because it is throwaway. A rename must not change how a log replays.
3. **Extract the journal, not the vocabulary.** `parse_log`'s policy + `jsonl_records` + the
   `upcast` seam go to the kernel, parameterised by a parse closure. The `Event` enum, `replay`'s
   body and `comment_to_events` stay in the format. This is the single highest-value extraction
   the spike found and it is validated by two real examples.
4. **A generic `replay` is fold + format-supplied closing pass.** ES needs `normalize`; the canvas
   needs nothing. A fold-only abstraction would silently drop ES's partition invariant.
5. **`join_by_id` is kernel; diff verdicts are not.** Rule of two, resolved: extract the join,
   leave the verdicts. `reslotted` is not `moved`.
6. **Split the board type from the overlay type first.** `Model.diff` / `Element.was` /
   `Edge.status` (and the canvas's copy of them) are the reason every renderer branches per mark.
   Worth doing with one format; mandatory with two.
7. **Type the closed vocabularies as enums.** `Slot` is an enum, and `slot_from_str` / `slot.key()`
   / `slot.title()` / `slot.prefix()` are total with no fallback arm. Writing format #2 enum-first
   cost nothing and produced no `_ =>` escape hatch — direct evidence for the `enum Lane` item in
   `docs/multi-format-architecture.md` §Type discipline.
8. **`esc` / `wrap` / `diff_colour` / `diff_badge` belong in the kernel.** Four pure functions,
   currently `pub(crate)` to `render`. Trivial, but they are exactly what a second format reaches
   for first.
9. **Mint mechanism is kernel; the prefix table is format.** `serve::ids::mint_id`'s
   highest-suffix-under-lock rule needed no change; only `Slot::prefix` is format-owned. Half of
   `serve` is already portable.
10. **Genesis write policy is kernel.** `write_canvas_genesis` differs from `write_genesis` in
    three calls (`load`, `from_*`, `to_jsonl`); the exclusive-create clobber refusal, the
    `<stem>.event-log.jsonl` naming and the summary are identical.
11. **Do not plan a per-format client.** Nearly half the current client (766 of 1597 lines) is
    coordinate/gesture code the Scene IR removes rather than makes portable. Sequencing stands:
    Scene IR first, client last.

## What this changes for the queued work

- **#121 F-format-tag** — **promote it above everything else in the family**. Constraint 1 makes it
  a correctness requirement ahead of any second format, not a bookkeeping step alongside one.
- **#116 F-scene-ir** — confirmed, and its strongest justification moved. It is not primarily about
  rendering N formats (the canvas renderer was easy to write from scratch); it is about *deleting*
  the Rust↔JS geometry mirror so the client does not fork.
- **#119 F-board-vs-diff** — constraint 6 promotes this too: it pays for itself with one format.
- **#117 F-lane-enum** — constraint 7 is direct evidence, not analogy: format #2 was written
  enum-first and produced no `_ =>` fallback anywhere.
- **#122 F-formats-move** — the boundary the note drew is right, with the journal/vocabulary split
  (constraint 3) as the first concrete move and #119 as its prerequisite.
- **#128 F-client-shell-split** — retitle the intent: plan to *delete* the geometry modules, not to
  split them per format.
- **#124 (DDD Context Map)** — the canvas exercised *no* edges at all, so it says nothing about the
  `Edge` seam. That remains entirely #124's job, and the two spikes are genuinely complementary:
  this one killed the coordinates, that one will stress the relationships.

## Honest limits of this spike

- `serve`, the client and `lint` were not built; Q4 and constraint 11 are reasoned from the code,
  not measured. The client split is the repo's highest-regression-risk area and this spike did not
  reduce that risk.
- The canvas is the *easiest* possible second format (no coordinates, no edges, no views, fixed
  layout). It proves the kernel does not *force* ES concepts on a format that wants none. It does
  **not** prove the kernel can serve a format with *different* coordinates — that is #115 (Wardley)
  — nor one with real relationships (#124).
- Filename dispatch and the `spike_canvas` module name are throwaway scaffolding, called out as
  such in the code.
