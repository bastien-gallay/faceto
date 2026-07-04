# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
