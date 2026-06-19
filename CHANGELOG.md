# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Event-sourced spine** (`src/events.rs`): an append-only `event-log.jsonl` is the
  durable record; the `Model` is a projection replayed from it. `render` and `serve`
  accept a log by extension. Comments become first-class events.
- `faceto genesis [MODEL]` — migrate a legacy `model.json` into the genesis batch of
  an `event-log.jsonl` (the bootstrap path into the event-sourced world).
- `faceto compact [LOG]` — fold a log to a `LogCompacted` marker plus the genesis
  batch of the current projection, bounding replay length. Projection-preserving
  (history collapses; the prior log is saved to `<log>.bak`).
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
