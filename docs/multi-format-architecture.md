# Multi-format architecture — exploration & decision record

**Status:** exploration (no code yet). Captures the design direction for evolving `faceto` from
a single-format tool (event storming) into a **kernel + pluggable diagram/workshop formats**
(e.g. C4, User Story Mapping, BPMN — the roadmap's parked `F-new-diagrams`). Nothing here is
committed to `main`; it is the shared reasoning so the seams are chosen deliberately, not
discovered late.

**Ethos guardrails (unchanged):** zero external dependencies, pure std, hand-written JSON, calm
instrument. These *constrain* the design (no serde, no autolayout crate) rather than relax it.

## Read this first — triage of 2026-07-26

This note is now **tracked**, and one of its premises changed. Read the two together.

**Every section below is still the design.** The kernel/format boundary, the data-Scene decision,
the sealed `enum Board`, the format tag, ADR-1 and the staged path are all unchanged, and they are
now 15 rows in `ROADMAP.md` (issues #114–#128, under the de-parked `F-new-diagrams` umbrella #126).

**What changed is which second format.** This note uses **C4** throughout as its pressure test —
`enum Board { EventStorming, C4 }`, `formats/c4/`, the container/component views. C4 is now a
**paper adversary, not a plan**: its pressure test did its job (it is what forced nested
`Shape::Group`, the format-owned lens, per-format diff verdicts, and the singular-board break), and
it stays unbuilt because it is the most expensive of the candidates — stored per-view coordinates
*and* multi-view *and* nested containment.

The formats actually queued:

| | Format | Shape family | Role |
| --- | --- | --- | --- |
| spike, throwaway | Bounded Context Canvas (#114) | slot template — *no* coordinates | kill `col`/`lane`/`y`/`phase` at once |
| spike, throwaway | Wardley map / Core Domain Chart (#115) | continuous 2D plane | replace discrete `col` with named axes |
| shipped format #2 | DDD Context Map (#124) | free-form graph, **typed** relationships | stress the `Edge` seam, which ES barely exercises |

Also dropped outright: **user story mapping** and **event modeling** — both are timeline ×
swimlane, i.e. structurally the board that already ships, so they would validate no abstraction.

**So when you read `c4::Model` below, read it as "a second format that is maximally distant".** The
C4 sketch is still the sharpest pressure test in the file; it is just not the next thing built.
Publication of this note into `docs/src/architecture/` is #127, and is deliberately sequenced
*after* the two spikes report — publishing a decision a spike is about to contradict is worse than
publishing nothing.

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
  shapes).

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
- **Separate `Board` from a diff/overlay type.** Today `Model` carries `diff` / `was` /
  `status` optionals, so one type encodes both "the board" and "a diff overlay" — two states
  crammed into one product type. Split them. This is the **same** boundary the Scene IR wants
  (the diff is a render concern, not a domain fact).
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

1. **Now (low regret, validated by ES alone):** the Scene IR — `kernel/scene.rs` (primitives +
   `render_scene`); ES `render.rs` produces a `Scene`. Correct with one format; the render
   contract later. Fold in the `Lane` enum and `UnitFraction` (net-negative-line wins regardless
   of C4).
2. **Next:** move ES into `formats/event_storming/`; leave `json` / `log` / `scene` /
   serve-transport as `kernel/`; introduce `enum Board` with **one** variant + the format tag.
   No C4 yet — just the boundary.
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

- Does the generic log stay `Vec<Json>` (tolerant, simplest) or gain a `trait LogEvent`? Lean
  `Vec<Json>`.
- Where does the diff overlay compose — Scene-level in the kernel (preferred) or per-format?
- View selection UX in `serve`/CLI (`--view` / `?view=`) — settle when the first multi-view
  format lands.
