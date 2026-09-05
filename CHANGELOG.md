# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **The `external` lane is now `system`** (ADR-1, #117). The pink sticky never meant "outside the
  company" — it means *a software system this board does not open up*, which is as often one of
  your own services as a third party's, and it lines up with C4's software system. Boards written
  with `external` keep working: the old spelling is **read** as `system` and never written back,
  and the lane still mints `G1`, `G2`, … so no id — and therefore no comment and no diff verdict —
  moves.

- **A sticky's `type` is a closed set, enforced at the edges** (F-lane-enum, #117). It was a
  string that every reader had to re-check, each with its own fallback for a lane that cannot
  exist; it is now a type, so the fallbacks are gone. Two things a user can notice: an element
  whose `type` is not one of the eight lanes is **dropped when the file or log is read** rather
  than carried into the board and filtered out again at draw time (the drawn board is unchanged —
  it was never drawn), and `faceto extract --type` now **refuses a misspelled lane by name**
  instead of cutting an empty board. The rendered SVG, HTML and context pack are byte-identical.

- **`faceto compact` refuses a log it cannot fully read**, instead of deleting the parts it could
  not project. Compaction rewrites the log from the replayed board, so a sticky whose `type` names
  a lane this build does not know — skipped on read, which is harmless everywhere else — was folded
  straight out of the append-only truth, silently and with exit 0. It now reports how many records
  it could not project, exits 1, and leaves the log untouched (no `.bak`, no rewrite): the log is
  not broken, this build is simply not the one that should be folding it. Relatedly, a log whose
  records all name an **unknown lane** no longer reports itself as coming from another board
  format — the event kinds were recognised, only the lane was not, so it gets its own message.

- **A log faceto cannot project is now an error, not an empty board.** Skipping unrecognised event
  kinds is how an older faceto reads a newer log; pointed at a log from a *different* notation, the
  same rule produced a valid, completely empty board and exit code 0. Three reads now stop instead:
  a declared format this build does not speak, a `"format"` that is present but not a string (a
  malformed tag is not an absent one), and a log with records but not one recognised event kind. A
  log mixing known and unknown kinds keeps the forgiving read, so forward compatibility is
  untouched, and a line naming *no* kind at all — a typo'd `event` key — is still a silent skip
  rather than evidence of a foreign format. `render --base` and `serve --base` also refuse **two
  boards of different formats**: a diff joins the two sides on `id`, which names a different thing
  in each notation.

- **An edge tuple's third slot is no longer read.** `["E1", "E2", "added"]` in a `model.json` used
  to set the internal diff status of that connection, painting it as an overlay wire on an ordinary
  board. It was the diff channel leaking into the authored format — `docs/schema/` always said "not
  an authored field" — and a board carrying one now renders it as a plain connection. The two-slot
  tuple and the object form are unchanged.

- **The diff is no longer part of the board type** (F-board-vs-diff, #119): comparing two boards
  returns the board *and* a separate overlay saying what changed, instead of writing
  `diff` / `was` / `status` annotations onto the model itself. Nothing changes to look at — the
  rendered SVG and HTML are byte-identical, for a plain board and for a `--base` diff alike.

- **Rendering goes through a Scene IR** (F-scene-ir, #116): the board is built as geometry —
  `Rect` / `Line` / `Text` / `Circle` / `Path` and a nesting `Group` — and one serializer turns
  that into SVG, instead of every draw step writing SVG strings inline. The board is unchanged to
  look at; the emitted markup differs in two harmless ways, so a *committed* board SVG will show
  one-off churn on its next render: numbers print in their shortest form (`240.0` → `240`, while
  an opacity like `0.02` keeps its precision), and attributes are ordered geometry-first. Every
  `data-*` tag the client reads is unchanged.

- **Output & log names are derived from the model basename** (F-output-naming): `faceto render
  orders.model.json` now writes `orders.svg` / `orders.html` (was a shared `board.svg` /
  `index.html`), and the event log is `orders.event-log.jsonl` (was a shared `event-log.jsonl`), so
  sibling boards in one directory no longer clobber each other's outputs *or* their logs. A model
  and its log resolve to the same board name, so `render` of either writes the same files. **This
  renames the on-disk log convention**: an existing `event-log.jsonl` is no longer found for a
  sibling `<name>.model.json` — rename it to `<name>.event-log.jsonl` (the tracked
  `examples/event-log.jsonl` was migrated to `examples/sample.event-log.jsonl`). Also folds in the
  two F-auto-genesis review carry-overs — the wrong-board serve bug is fixed by construction, and a
  new warn-only nudge fires when a source replays to an empty board (0 elements).

### Fixed

- **An element with no lane no longer leaves the board in silence** (#149 review). A `type`
  outside the eight lanes has nowhere to go, so the entry is dropped on read — correct, but
  wordless, and `genesis` writes only the survivors into the log while keeping the edges that
  pointed at what it dropped. Reading a `model.json` now warns with the count (an entry missing
  its `id` or `label` counts too).

- **An `ElementMoved` naming an unreadable lane keeps its move.** The record was rejected whole, so
  a well-formed `col`/`y` was discarded along with a `type` this build could not name. The lane is
  now fatal only where it is load-bearing — an `ElementAdded` has nowhere to put the sticky — and
  elsewhere it is dropped while the rest of the record applies. `compact` still refuses such a log:
  a record read *minus* its lane was not read in full either.

- **`extract --type external` stops renaming the lane you typed.** It answered `error: system lane
  matched no elements`, which reads as though the argument had been ignored. The pre-ADR-1 spelling
  now announces itself with a `note:` where the substitution happens, so every later message makes
  sense.

- **The narrate skill can propose edges again** — its instructions claimed "you cannot propose
  edges […] never invent an edge kind (it will 400)", which stopped being true when
  `connect`/`disconnect` shipped with F-edge-connect. A missing arrow is the commonest gap the
  skill detects, so it was talking its way around the cheapest proposal on the board.
- **The board never auto-refreshed, and now the docs say so.** `SKILL.md`, two design notes, four
  `src/serve/` comments and the F-collab-sse roadmap row all described a "~1 Hz poll" of
  `/model-version` that repaints the board on its own. `git log -S setInterval` shows no such loop
  was ever written: the client fetches on load and on **Reload**, full stop. No behaviour changed —
  the claim did. Push (`F-collab-sse`) is still the un-shipped answer.

### Added

- **Boards declare a format** (F-format-tag, #121): a top-level `"format"` in a `model.json`, or a
  `BoardFormat` event in a log. One value ships — `event-storming` — and an absent tag still means
  exactly that, so every existing board reads unchanged and neither `genesis` nor `compact` starts
  writing a tag onto one. See [board formats](docs/src/reference/board-formats.md).

- **`faceto extract` — semantic sub-board extraction** (F-extract, #90). Carve a smaller board out
  of a bigger one by *meaning* rather than by geometry: `--region K2` (a band), `--focus E4
  --hops 2` (a bounded, undirected walk out from one element), or `--type hotspot` (a lane). The
  result is written beside the source as an already-genesis'd log (`orders-K2.event-log.jsonl`), so
  `render` and `serve` take it directly. **Ids and columns are preserved**, which makes the extract
  a valid baseline for a diff against its origin — `faceto render orders-K2.event-log.jsonl --base
  orders.event-log.jsonl` reports `0 moved, 0 changed`. Exactly one selector per run (two is a
  usage error, not an intersection); an edge with one endpoint outside the cut is dropped and
  `lint` surfaces the hole; regions come along, clipped to the survivors. A `col` the model file
  left out is resolved the way the board resolves it — so `--region` cuts what you can see inside
  the band — and written out explicitly on the result. Handed a `model.json` that already has a log
  beside it, `extract` reads the **log**, since the model is a stale bootstrap form by then (it
  never creates one, unlike `serve`). Like `genesis`, the write refuses to overwrite an existing
  log. See
  [the `extract` page](https://bastien-gallay.github.io/faceto/reference/cli/extract.html).

- **The keyboard sheet can no longer drift** (F-docs-reference, #129). The board's gestures are
  written twice by hand — the in-app `?` dialog and the manual's gesture page — with no generator
  between them. A CI job
  (`keyboard sheet`, locally `just keyboard-check`) now compares the two key sets in *both*
  directions, so a binding added to the app without a doc entry turns the check red, and so does a
  key the manual still promises after the app dropped it. (Red, not blocking: the job is not in the
  branch ruleset yet — see `docs/ci.md`.) Descriptions are deliberately not compared.

- **The model format and the event log are documented** (F-docs-reference, #129). Both reference
  pages had shipped as placeholders pointing at a closed issue; they are now the real thing. [The
  model format](https://bastien-gallay.github.io/faceto/reference/model-format.html) carries every
  `model.json` field with its type and default, the `id` / `col` / `type` rules, and — new to any
  surface — a table of **what the lenient parser drops in silence**, which nothing warned you about
  before. [The event log](https://bastien-gallay.github.io/faceto/reference/event-log.html) carries
  the five outcomes of reading a line, all 17 event kinds with their effect on replay, the id-mint
  prefix table, and the whole `POST /comment` write contract with the reason behind each guard —
  including the two things it does *not* guard: `connect` never checks that its endpoints exist,
  and an omitted `text` on `resolve` clears the element's note rather than leaving it.

- **A documentation site** (F-docs-book): the manual now lives in `docs/src/` as an mdBook,
  published to <https://bastien-gallay.github.io/faceto/> — a board guide organised by what you are
  looking at, a per-verb CLI reference, the lint rules, and a "Working with agents" part covering
  the context pack, the narrate skill and variants. The tour embeds a *real* board rendered at
  deploy time, not a screenshot. `create-missing = false` plus a CI `docs book` job mean a chapter
  the table of contents promises can never ship as an empty page.
- **Collapse a region to a band** (F-region-collapse): fold a wide board's off-topic regions to
  concentrate readability. Click the **▸/▾** disclosure on a region tab (or press **`z`** on it) to
  compress that region's columns to one thin summary slot — its stickies hide behind a `▸ Label · N`
  count chip and every column to its right shifts left, so the board actually gets **shorter**. It is
  a pure **per-viewer reading lens**: no model, event, or log change — the collapsed set lives in
  `localStorage` and re-lays-out server-side via `GET /board.svg?collapse=<id,id>` (composes with the
  `?base=` diff overlay). Column-fold only; edges *into* a folded band drop with their hidden nodes,
  edges merely *crossing* it stay as straight passthroughs (rerouting them is the deferred
  F-region-edge-fold).
- **Region frontiers** (F-region-frontiers): regions are now a **contiguous partition** of the
  timeline defined by shared frontiers, not independent `[fromCol, toCol]` spans. A single pure,
  deterministic, idempotent sweep (`model::normalize`, run in both `replay` and `from_json`)
  projects any phase list — new frontier events *and* legacy spans with holes/overlaps — onto a
  gap-free, overlap-free partition, so every `Model` obeys the invariant. New additive events
  `FrontierMoved { id, edge, col }` (drag a boundary — the neighbour re-borders atomically, clamped
  so it can't cross into a third phase) and `PhaseSplit { id, atCol, newId, newLabel }` (add = carve
  a phase in two, server-minted id); remove merges into the neighbour (no stranded columns). The
  board draws one grabbable frontier per boundary. Hole / overlap / unreachable-edge /
  can't-resize-at-the-extremes are all unrepresentable by construction.
- **ES-grammar linter** (F-es-lint): `faceto lint SOURCE` — a pure, zero-dep graph pass
  (`src/lint.rs`) that flags event-storming defects a workshop review would raise by hand
  (event with no producer, policy with no input/output, non-terminal event with no outbound
  edge). **Warn-only**, always exits 0. An optional board `level: big-picture | design`
  (top-level in `model.json`, a `BoardLeveled` log event) adds one stricter rule at `design`
  — `command-no-output`, a command that emits no event. `serve`'s `/comments` sidebar now
  merges the live findings as `kind:"lint"` entries (computed on read, suppressed once the
  element is resolved), so the tool's nudges sit beside human notes in the review loop.
- **Direct on-board editing** (F-inline-edit): rename a sticky in place (double-click
  or **F2** → an inline field; Enter commits, Escape cancels) and remove one with
  **Delete / Backspace** (with a confirm). Move was already direct (← / →), so the
  comment modal is now an optional path, not the only one. Both gestures reuse the
  existing `rename` / `drop` events and the log-mode append path.
- A `rename` now obeys the same non-blank-label rule as `add` (new shared
  `events::nonblank`): a blank or whitespace-only rename persists nothing, so direct
  editing can't blank a label into a permanent, never-renumbered empty box.
- **Richer board gestures** (F-board-gestures): the box itself is the edit surface —
  single-click focuses, **double-click / F2** renames, and a **drag left/right** moves
  it along its lane (the mouse counterpart to ← / →). Hovering reveals three bare ghost
  glyphs — `+` add, `×` remove, and a speech-bubble comment — individually anchored at
  the box's edges, never a floating toolbar. The comment modal is now **prose-only**
  (comment / split / open question / resolve); rename, drop and move became gestures,
  and `resolve` shows only on a hotspot or an element carrying an open question. Comment
  is also the **`c`** key.
- **Event-sourced spine** (`src/events.rs`): an append-only `event-log.jsonl` is the
  durable record; the `Model` is a projection replayed from it. `render` and `serve`
  accept a log by extension. Comments become first-class events.
- `faceto genesis [MODEL]` — migrate a `model.json` into the genesis batch of an
  `event-log.jsonl` (the bootstrap path into the event-sourced world). `model.json` is
  the source/authoring format; the log is the durable truth.
- `faceto compact [LOG]` — fold a log to a `LogCompacted` marker plus the genesis
  batch of the current projection, bounding replay length. Projection-preserving
  (history collapses; the prior log is saved to `<log>.bak`).
- Schema evolution: an old log keeps replaying as the event schema grows. Additive
  change is free (new optional fields ignored, new event kinds skipped on read); a
  renamed kind/field is migrated forward at the `upcast` read-path seam.
- Live structural edits in log mode: `POST /comment {kind:"add", type, text, col?}`
  appends an `ElementAdded` with a **server-minted, type-prefixed id**; moves (with
  swap), renames, hotspot resolutions, and `drop` (→ `ElementRemoved`) persist as their
  own events. The board modal gained an "add element" affordance for it.
- A **serif nameplate** for the board title (system serif; HTML header + SVG), the one
  display face on an otherwise single-sans instrument.
- Quality harness: `cargo fmt` / `clippy` config, pinned toolchain, and a GitHub
  Actions CI pipeline (fmt, clippy + test on macOS/Windows/Linux, markdownlint,
  actionlint).
- A **zero-dependency firewall** CI job that fails if any crate is ever added to
  the dependency tree.
- Unit tests for the JSON parser/serializer, the id-keyed model diff, the SVG
  label layout, and the server's hashing/date helpers.
- `CODING_STANDARDS.md` (Tidy First, CUPID & YAGNI, TDD+Reflect, Clean Code) and
  `AGENTS.md`, a cross-tool entry point for coding agents.
- A local `pre-commit` harness (`.pre-commit-config.yaml` + `.typos.toml`) that
  runs the CI gates — fmt, clippy, markdownlint, typos, and tests on push.

## [0.1.0]

### Added

- `faceto render` — write `board.svg` + `index.html` next to a JSON model.
- `faceto serve` — live board with a click → comment sidecar (`comments.jsonl`)
  and an in-page diff against a cached baseline, served by a std-only HTTP server.
- Event-storm board format: eight typed lanes on a shared left → right timeline,
  directed edges, phases, and hotspots.
- Hand-written, dependency-free JSON module (`src/json.rs`).

[Unreleased]: https://github.com/bastien-gallay/faceto/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bastien-gallay/faceto/releases/tag/v0.1.0
