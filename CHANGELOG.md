# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Quality harness: `cargo fmt` / `clippy` config, pinned toolchain, and a GitHub
  Actions CI pipeline (fmt, clippy + test on macOS/Windows/Linux, markdownlint,
  actionlint).
- A **zero-dependency firewall** CI job that fails if any crate is ever added to
  the dependency tree.
- Unit tests for the JSON parser/serializer, the id-keyed model diff, the SVG
  label layout, and the server's hashing/date helpers.

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
