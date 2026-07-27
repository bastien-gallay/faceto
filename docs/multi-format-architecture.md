# Multi-format architecture — exploration & decision record

**Status:** partly built. Captures the design direction for evolving `faceto` from a single-format
tool (event storming) into a **kernel + pluggable diagram/workshop formats** (e.g. C4, User Story
Mapping, BPMN — the roadmap's parked `F-new-diagrams`). It is the shared reasoning so the seams are
chosen deliberately, not discovered late.

**One section is now code.** The data-Scene decision shipped 2026-07-26 as `src/scene.rs`
(`F-scene-ir` #116, PR #136). Everything else here is still exploration. Where a section describes
the Scene IR in the future tense, read it as the design it was built from — and see the *as built*
notes, which record the three places reality diverged.

**Ethos guardrails (unchanged):** zero external dependencies, pure std, hand-written JSON, calm
instrument. These *constrain* the design (no serde, no autolayout crate) rather than relax it.

## Where this stands — 2026-07-27

**Settled.** Two decisions are built. The data-Scene decision: `src/scene.rs` holds the geometric
primitives and the one `render_scene` serializer, and `render::board_scene` turns a board into a
`Scene` (`F-scene-ir` #116, PR #136). And the board/overlay split, one day later: `render::diff.rs`
holds `diff_boards -> (Model, Overlay)` and the closed verdict enums, so a `Model` is now always a
board (`F-board-vs-diff` #119, PR #138). Nothing else in this note is code.

**One line of the type-discipline section is now stale by success** — the third bullet ("Separate
`Board` from a diff/overlay type") describes work that is done. It is left standing because the
*reasoning* under it is what the next format will need; read it as the argument, not the backlog.

**Open, ranked.** Each item is tracked or explicitly is not:

1. **#121 `F-format-tag`** — the correctness precondition the canvas spike found: a foreign-format
   log currently replays as an empty board, silently.
2. **#115 `F-spike-wardley`** — still worth running, but its job changed: it can no longer
   constrain `Shape`'s introduction, only a revision of it.
3. **Compose the overlay out of two Scenes** — what #119's row promised and did not deliver. The
   split made a `Scene`-level diff *possible* (the overlay is now its own type, off the domain
   model) but the code still annotates one board while it is built. **Not ticketed**: it wants a
   second format to say what composing two scenes even means for a format whose layout is not a
   column grid.
4. **Ship positions to the client** — the half of the Scene IR that did not land, and the whole
   justification for #128. **Deliberately not ticketed**: nobody has designed what the client
   would receive, and a ticket for an undesigned interface is a ticket that rots. It is named in
   the #128 row instead.

**What will look like a contradiction and is not.**

- This note describes the Scene IR in the future tense throughout. That is deliberate — it is the
  design the code was built *from*. The *as built* notes mark each place reality diverged; the
  prose around them was not rewritten, so a reader arriving with the old design in their head can
  still recognise it.
- The staged path carries **two** superseding blockquotes that disagree with each other. Both are
  correct as records: the family was re-ordered twice in one week and the code followed neither
  ordering. Read them as history, not as instructions.
- `docs/notes/f-spike-canvas.md` forecasts that the Scene IR deletes 766 lines of client geometry.
  It did not — the note is pinned to a commit and never rebased, by design.
- The data-Scene decision's second driver names `diff_meta`, and `git grep diff_meta` now returns
  nothing. The field was deleted by #119, which is the driver's own goal arriving one step early:
  the diff stopped being a `Model` field before it started composing two `Scene`s. The driver is
  the record of why the IR was built, not a description of today's code.

**Resuming here:**

```text
Read docs/multi-format-architecture.md (this section), then the F-scene-ir and F-board-vs-diff
rows in ROADMAP.md for what shipped versus what the rows promised. The Scene IR is
src/scene.rs; the ES scene builder is render::board_scene; the diff overlay is
src/render/diff.rs (diff_boards -> (Model, Overlay), passed to render_svg beside the board).
Next action: issue #121 (F-format-tag) — a foreign-format log still replays as an empty board,
silently.
```

## Read this first — triage of 2026-07-26

This note is now **tracked**, and one of its premises changed. Read the two together.

**Every section below is still the design** — except the data-Scene decision, which is now code
(see *Status*, above). The kernel/format boundary, the sealed `enum Board`, the format tag, ADR-1
and the staged path are all unchanged, and they are now 15 rows in `ROADMAP.md` (issues #114–#128,
under the de-parked `F-new-diagrams` umbrella #126).

**What changed is which second format.** This note uses **C4** throughout as its pressure test —
`enum Board { EventStorming, C4 }`, `formats/c4/`, the container/component views. C4 is now a
**paper adversary, not a plan**: its pressure test did its job (it is what forced nested
`Shape::Group`, the format-owned lens, per-format diff verdicts, and the singular-board break), and
it stays unbuilt because it is the most expensive of the candidates — stored per-view coordinates
*and* multi-view *and* nested containment.

The formats actually queued:

| | Format | Shape family | Role |
| --- | --- | --- | --- |
| spike, **reported** 2026-07-26 | Bounded Context Canvas (#114) | slot template — *no* coordinates | kill `col`/`lane`/`y`/`phase` at once |
| spike, throwaway | Wardley map / Core Domain Chart (#115) | continuous 2D plane | replace discrete `col` with named axes |
| shipped format #2 | DDD Context Map (#124) | free-form graph, **typed** relationships | stress the `Edge` seam, which ES barely exercises |

Also dropped outright: **user story mapping** and **event modeling** — both are timeline ×
swimlane, i.e. structurally the board that already ships, so they would validate no abstraction.

**So when you read `c4::Model` below, read it as "a second format that is maximally distant".** The
C4 sketch is still the sharpest pressure test in the file; it is just not the next thing built.
Publication of this note into `docs/src/architecture/` is #127, and is deliberately sequenced
*after* the two spikes report — publishing a decision a spike is about to contradict is worse than
publishing nothing.

### What the canvas spike settled (2026-07-26)

The first spike reported: [`docs/notes/f-spike-canvas.md`](notes/f-spike-canvas.md), 11
constraints, code on `spike/f-spike-canvas` (never merged, never rebased). **The sections below are
still the design — but two of its judgements moved, and both are about *order*, not content.**

- **The format tag moves to the front.** This note treats it as part of step 2 of the staged path.
  It is a *precondition*: without it, `events::parse_log` reads a foreign-format log as `Ok` with
  an empty `Model`, because skipping unknown kinds is precisely the forward-compatibility rule.
  Forward compatibility and format discrimination are one mechanism aimed in opposite directions.
- **The Scene IR moves behind the second spike.** Step 1 below calls it "validated by ES alone".
  The canvas contradicted the *reason*, not the conclusion: a from-scratch canvas renderer was
  easy, so "render N formats" is a weak driver. The strong one is that the Scene IR **deletes**
  the Rust↔JS geometry mirror. And the canvas exercised zero coordinates and zero edges, so it
  constrains `Shape` not at all — #115 is the probe that still can.

  > **Not honoured — 2026-07-26.** #116 shipped *before* #115 ran, on the ROADMAP row's reading
  > ("correct with one format") rather than this bullet's. Two consequences the next reader needs.
  > `Shape` was designed with no second format pressing on it, so the primitive set is a bet: #115
  > can now only constrain a *revision*. And the strong driver named here — deleting the geometry
  > mirror — **did not land with it**: the IR builds a `Scene` server-side and serializes it, but
  > ships no positions to the client, which is still 1597 lines. The bullet's reasoning stands;
  > only its sequencing was overtaken.

Two of the open questions at the foot of this note now have evidence; see there. The kernel /
format boundary, the sealed `enum Board`, ADR-1 and the C4 pressure test are **unchanged** — the
spike found no coordinate concept leaking into the kernel at all. (The data-Scene decision was in
this list too; it has since been built.)

## The core realization

Almost everything in `faceto` today is coupled to **one domain: event storming**. The `Model` is
not a generic diagram — it *is* an ES board: 8 fixed lanes, `col` = a global timeline, `phases`
= regions, `is_pivotal`, frontiers. A second format like C4 shares almost none of that: nested
systems/containers/components, relationships, boundaries, **no timeline, no lanes-as-ES-lanes**.

So multi-diagram support is **not primarily a rendering problem — it is a domain/kernel split.**
The render layer (a Scene IR, below) is how the kernel and formats talk about drawing, but the
larger move is separating *the generic instrument* from *the event-storming format that happens
to be the only one today*.

The clean line: **`col` / `lane` / `phase` / `pivotal` must never leak into the kernel.** That
they are purely ES is exactly what makes the boundary real.

## Kernel vs format boundary

| Current code | Generic kernel | ES-specific format |
| --- | --- | --- |
| `json.rs` | all of it | — |
| `events.rs` | log machinery: `parse_log` / `read_log` / `jsonl_records`, compact *scaffold*, `upcast` seam, unknown-kind skip, `to_jsonl` | the `Event` enum, `replay`, `from_model`, `comment_to_events`, `region_watermark`, compact bodies |
| `model.rs` | the stable-id **diff pattern** (`join_by_id`) | `Model` / `Element` / `Edge` / `Phase`, `normalize`, `is_pivotal`, `y_key`, `resolve_region_id`, `lane_left_col`, `diff_models` |
| `render/` | `svg` **primitives** + the Scene serializer | `style` (8 lanes/colours), `geometry` (col = x timeline), `svg` `draw_*` (sticky/region/frontier) |
| *as built* | the primitives and the serializer went **further out** than this row: they are `src/scene.rs`, a sibling of `render/`, not a kernel half of it | `render/` kept every ES word — lane, col, sticky, region, frontier — and gained `board_scene`, the `(Model, View) -> Scene` builder |
| *as built* | the stable-id join is **not** in `model.rs` any more: F-board-vs-diff (#119, PR #138) moved it to `render/diff.rs` as `diff_boards -> (Model, Overlay)`. The row's instinct was right — the *pattern* is the generic half — but it generalises from `render/`, not from the domain type | `model.rs` kept `Model` / `Element` / `Edge` / `Phase`, `normalize`, `is_pivotal`, `y_key`, `resolve_region_id`, `lane_left_col` — and nothing that knows what a diff is |
| `serve.rs` | transport: listener/threads, request parse + caps, `send`, `fnv12`, cache ring, appends mutex, mint *mechanism* | route command→event mapping, mint *prefix table* |
| `lint.rs` | a `Finding` shape + the serve-sidecar merge | every rule (ES grammar) |

## The render contract: a Scene IR as data

The single most consequential technical choice. Two ways to serialize a board to SVG:

- **String-Scene (immediate mode):** each format's renderer writes SVG strings directly (via
  shared primitives). Enough to render N formats, but the knowledge stays inline per format.
- **Data-Scene (retained mode):** every format produces a `Scene` value; **one** kernel
  serializer turns it into SVG. The domain layer becomes inspectable data.

```rust
// kernel/scene.rs
enum Shape {
    Rect { x: f64, y: f64, w: f64, h: f64, /* class, fill, stroke, data-* */ },
    Line { .. }, Text { .. }, Circle { .. }, Path { .. },
    Group { attrs: Attrs, children: Vec<Shape> },   // NESTED — see C4 pressure test
}
type Scene = Vec<Shape>;
fn render_scene(scene: &Scene) -> String;           // written ONCE
```

**As built (`src/scene.rs`, PR #136), three divergences from this sketch.** `Scene` is a struct
carrying `width` / `height` beside the shapes, not a bare `Vec<Shape>` — the canvas size is
geometry too, and leaving it out meant the serializer could not write its own `<svg>` root.
`Attrs` is an *ordered* `Vec<(String, Val)>`, not a map: deterministic output is what makes a
rendered board diffable in git. And `Val` keeps numbers as numbers (`Num(f64)` / `Int(i64)`),
which is what lets a test — or later, the diff overlay — read geometry back off a scene instead
of re-parsing strings. The comment on `Rect` here (`/* class, fill, stroke, data-* */`) is exactly
that `Attrs`.

**Decision: data-Scene.** Across `render`, `serve`, and especially the client, string-Scene
forces every downstream layer to re-implement per format. The drivers, strongest last:

1. The SVG serializer is written once (weak — string-Scene also gives this).
2. The **diff overlay** composes two Scenes into one (today's `diff_meta` / dashed-badge logic
   becomes format-agnostic).
3. **Scene-level tests** replace brittle SVG-substring assertions.
4. **The client.** `template.html` is the strongest argument: with string-Scene it **cannot
   split** — geometry stays inline, so each format needs its own ~1500-line untested client
   monolith. With data-Scene (shapes carrying `id` / `data-*` / position + a draggable-axis
   hint), a *generic* client renderer + hit-test + drag-preview operate on shapes without
   knowing "sticky" vs "container."

The Scene IR must be **geometric/primitive** (`Rect`/`Line`/`Text`/`Group`), never semantic
(`Sticky`/`Region`) — sticky/region are ES-only; the semantic shapes stay inside each format's
`scene()` builder. Modelling `Shape` as data mirroring SVG attributes 1:1 would be the "SVG with
extra steps" trap: the value is the *nesting + tags + positions the kernel can reason over*, not
attribute storage.

## The Format seam

- **Dispatch: a sealed `enum Board`, not a `dyn Format` trait.**

  ```rust
  enum Board { EventStorming(es::Model), C4(c4::Model) }
  ```

Fits the zero-dep / legible / sealed ethos; one `match`, compiler-checked exhaustiveness. A
`Format` trait with `type Model` / `type Event` is not object-safe, so `dyn` would type-erase to
`String` / `Vec<u8>` anyway.

- **The log needs a format tag.** `event-log.jsonl` is implicitly ES today; a genesis header
  (`{"event":"BoardFormat","format":"event-storming"}`) or a top-level `"format"` in
  `model.json` selects the projector. Absent → `event-storming` (the same additive default rule
  `level` already uses).

- **One model → many views.** C4's context/container/component diagrams are all projections of
  one graph at different scopes. So the seam carries `fn views(&Model) -> Vec<ViewId>` (ES
  returns exactly one), and render is `(Model, ViewId, Lens) -> Scene`. The **lens is
  format-owned** (ES = collapsed-set; C4 = view-id).

- **Mint:** the kernel offers the "highest-suffix-under-lock" mechanism; the **prefix-for-kind
  table** is the format's (ES `X`/`C`/`A`…; C4 `p`/`s`/`c`…).

## Per-file consequences

- **`events.rs` →** `kernel/log.rs` (generic journal; simplest tolerant form: the log is
  `Vec<Json>` lines and each format supplies `replay(&[Json]) -> its Model`) +
  `formats/event_storming/events.rs` (the `Event` enum, `replay`, `comment_to_events`, ES
  `compact`).

- **`model.rs` →** a small `kernel` `join_by_id` helper **only** +
  `formats/event_storming/model.rs` (everything else). Do **not** extract the diff *verdicts* —
  "changed vs moved vs resized vs y-key" is ES semantics; C4's verdicts differ (re-parented,
  technology-changed). Rule of two: extract the diff engine once C4's diff exists.

- **`render/` →** `kernel/scene.rs` (primitives + `render_scene`) +
  `formats/event_storming/render.rs` (`Model -> Scene`: lanes, colours, col→x layout, the ES
  shapes). **Half done 2026-07-26**: the split happened, at `src/scene.rs` + `src/render/`
  (`board_scene`). The `kernel/` and `formats/` trees are still F-formats-move #122's to create;
  this is a rename away, not a re-design.

- **`serve.rs` →** transport stays kernel; the command→event mapping, the mint prefix table,
  and the view selector are per-format. `?base=` diff-overlay composition moves to Scene-level
  (kernel).

- **`template.html` →** `client/shell.{html,css,js}` (kernel: version-poll → board swap, diff
  overlay, comment sidebar, offline queue, modal, raw pointer-drag primitives) +
  `formats/<fmt>/client.{css,js}` (glue: gestures → event kinds, format CSS). Assembled at
  compile time via `include_str!` + `concat!` (no build step). **Highest-regression-risk split
  in the repo** (zero automated coverage; live-verified only), and strictly downstream of the
  Scene-IR choice — sequence it **last**. Bonus: data-Scene lets the server ship positions, so
  the Rust↔JS geometry mirror (`edgePath` / `fanOffsets` / `computeGrid`, the "hard sync
  constraint") is **deleted**, not copied per format.

## C4 pressure test

Sketching `formats/c4/model.rs` (Person / SoftwareSystem / Container / Component with `parent`
containment; Relationships with `technology`; `views` with per-view manual coordinates) against
the boundary:

- **Held:** stable-id identity (elements *and* relationships) validates the shared `join_by_id`
  diff pattern; the generic log folds C4 events unchanged; every C4 mark decomposes into
  geometric primitives.
- **Bent (forced refinements):** `Shape::Group` must be a **nested tree** (boundary boxes
  contain and sit behind children — ES draws flat today and would have under-specified this);
  the **lens is format-owned**; **diff verdicts are per-format**.
- **Broke (the deep one):** **"one file = one board = one diagram" → "one model = many
  views."** This is baked into `serve` (renders *the* board), `board.svg` (singular), and
  `render(&Model, &View)`. Cheap to accommodate now (ES always has one view); expensive to
  retrofit once serve/CLI hard-code singularity.

The sleeper cost of C4 is **not** the model — it is **layout** (zero-dep ⇒ stored per-view
coordinates, not an autolayout engine) and the singular-board assumption.

## Type discipline (make illegal states unrepresentable)

`faceto` is already half-DMMF (append-only log, pure `replay`, immutable projections). The gap
is type discipline at the *edges*. Apply, as the modelling posture of the redesign — not a
separate refactor:

- **`enum Lane`** replaces `kind: String`. The off-grammar-element panic class (currently
  patched by *filtering* unknown kinds, with `_ =>` fallbacks in `colour`/`lane_index`) becomes
  **unrepresentable**; the fallbacks vanish; `colour`/`lane_prefix`/`LANES` become total.
  Highest-leverage, lowest-cost win.
- **`UnitFraction(f64)`** (smart constructor, `[0,1]`) replaces `y: Option<f64>` clamped in
  *two* places (`clamp_y` on write, `y_key` on read) precisely because the type admits illegal
  values. One boundary clamp, none downstream — parse, don't validate.
- **Separate `Board` from a diff/overlay type.** ~~Today `Model` carries `diff` / `was` /
  `status` optionals, so one type encodes both "the board" and "a diff overlay" — two states
  crammed into one product type.~~ **Done 2026-07-27 (#119, PR #138):** `render::diff_boards`
  returns the union board and an `Overlay` of closed verdict enums, which `render_svg` takes beside
  the board.
  The argument still holds for the next format — the diff is a render concern, not a domain
  fact — which is why the bullet stays.
- **Parse, don't validate, at the command boundary.** `serve`'s `v.get_str("kind")` matched
  against string literals (the double-dispatch + silent-drop review findings) → parse the
  request into a typed `Command` enum once, then match exhaustively.

**Honest tensions.** Zero-dep hand-written JSON means every newtype/enum needs explicit
`from_json`/ `to_json` (no serde derive) — so apply MISI *selectively* (the four above), not to
every `String` (an `ElementId`/`RegionId` split is marginal unless id-confusion is a real bug —
it has not been). And **forward-compat bounds the purity:** the log must skip unknown kinds and
ignore unknown fields, so the ADT boundary is a *tolerant* parser; the `upcast` seam is the DMMF
anti-corruption layer and is where the tolerance lives.

## ADR-1: rename the `external` lane to `system`

**Decision:** rename the pink lane's canonical value `external → system`.

**Why:** the pink sticky represents any software system outside the aggregate/domain logic
(internal or external), so `external` is the narrower reading; `system` is the accurate general
name and **aligns with C4's `SoftwareSystem`** — a shared "system" notion across formats is a
feature.

**Consequence — this is a data migration, not a code rename.** `type:"external"` is persisted in
`event-log.jsonl` (git-tracked) and both example `model.json` files. It is the textbook case for
the anti-corruption seam: the read path maps legacy `external → system` (a renamed *field
value*, cousin of the `upcast` kind-rename `CommentAdded → ElementAnnotated`). Old logs stay
replayable; new writes emit `system`.

**Do it as part of** the `Lane` enum (the value maps to the `System` variant, and the upcast
rule lives at the parse boundary), not as a standalone string swap. **Keep the prefix letter
`G`** (ids `G1…` stay valid) even though `S` is free — changing it would force an id migration
for no functional gain.

## Staged path — isolate first, abstract on the second example

The way to get ready for C4 is **not** to build the abstraction now (that bakes ES assumptions
C4 breaks), but to make event storming stop being "the whole app" and become "format #1 behind a
thin seam."

1. ~~**Now (low regret, validated by ES alone):**~~ **Shipped 2026-07-26 (PR #136)** — the Scene IR,
   as `src/scene.rs` rather than `kernel/scene.rs` (the `kernel/` tree is step 2's to create), with
   `render::board_scene` producing the `Scene`. **The riders did not come with it**: the `Lane` enum
   (#117) and `UnitFraction` (#118) are both still open, so step 1 shipped as its head only.
2. **Next:** move ES into `formats/event_storming/`; leave `json` / `log` / `scene` /
   serve-transport as `kernel/`; introduce `enum Board` with **one** variant + the format tag.
   No C4 yet — just the boundary.

   > **Superseded 2026-07-26 by the canvas spike (#114) — steps 1 and 2 re-order.** The **format
   > tag (#121)** leaves step 2 and becomes the first thing built: it is a correctness
   > precondition, not a boundary detail (see *What the canvas spike settled*, above). The
   > **Scene IR (#116)** leaves step 1 and waits on the Wardley spike (#115), the only remaining
   > probe that can still constrain `Shape`. What lands first instead is the trio that pays off
   > with a single format — **#119** (split the board type from the overlay type), **#117** (the
   > `Lane` enum) and **#118** (`UnitFraction`), which step 1 already folded in as riders. The
   > *content* of both steps is unchanged; only their order is.
   >
   > **This re-ordering was itself overtaken — 2026-07-26.** #116 shipped first after all, and
   > none of the trio named above has. Read the note as a record of what was decided, not of what
   > happened: two orderings were written down for this family in one week, and the code followed
   > neither. The *content* of the steps is still unchanged — that part has now survived twice.
3. **When C4 is actually designed:** add `formats/c4/`. Two real examples let the `Format` /
   diff / lint abstractions extract correctly (rule of two) instead of guessed.
4. **Defer:** generic diff and lint frameworks until C4 shows its shape; autolayout (documented
   non-goal territory) — prefer stored per-view coordinates.

Every step is independently green and shippable.

## Risks & non-goals

- **Premature abstraction** is the main risk — mitigated by isolate-before-abstract (rule of
  two).
- **Workspace of internal crates** (`faceto-kernel`, `faceto-eventstorming`, `faceto-cli`, all
  path deps, still zero *external* deps) is an option for *hard* boundary enforcement + parallel
  compile. Prefer `mod kernel; mod formats;` first; promote to crates only if the boundary needs
  enforcing or compile times bite.
- **Not a plugin marketplace.** Formats are sealed (`enum Board`), added by recompiling —
  intentional.
- **Autolayout stays out** under zero-dep; C4 uses stored coordinates.

## Open questions

- ~~Does the generic log stay `Vec<Json>` (tolerant, simplest) or gain a `trait LogEvent`?~~
  **Answered by the canvas spike (#114): `Vec<Json>`, as leaned.** `CanvasEvent` shares *no*
  variant with `Event` — not one — so there is no vocabulary for a trait to abstract over. What
  the kernel keeps is the **journal**: `parse_log`'s policy, `jsonl_records`, the `upcast` seam,
  all of which are already format-agnostic and merely typed on `Event`. The spike had to copy
  them verbatim; parameterising them on a parse closure is the one clearly-earned extraction.
- ~~Where does the diff overlay compose — Scene-level in the kernel (preferred) or per-format?~~
  **Split, per the canvas spike:** the *join* (`join_by_id`) is kernel and transferred verbatim;
  the *verdicts* are per-format and must not be extracted. `moved` has no canvas meaning, and its
  replacement `reslotted` is not `moved` renamed — it reports a category, so it reads closer to
  `changed`. The rule of two resolved **against** a generic verdict engine: the second example
  disagreed with the first rather than generalising it.
- Still open — a second-format question the canvas could not touch: it has no edges and no
  coordinates, so `Shape` and the `Edge` seam are unconstrained by it. #115 and #124 own those.
- View selection UX in `serve`/CLI (`--view` / `?view=`) — settle when the first multi-view
  format lands.
