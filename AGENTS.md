# AGENTS.md

Entry point for any coding agent or tool working in this repo. It orients you
and points at the canonical docs — it deliberately does **not** restate them, so
they can't drift.

## What this is

`faceto` turns a typed JSON model into an interactive HTML+SVG workshop board
(event storming is the first format). It renders a static board or serves a live
one with a click→comment sidecar and an in-page diff. The whole point is "a
simple typed file you think through with an LLM."

## The one hard rule: zero dependencies

`faceto` is **pure Rust standard library — no crates, ever** (dev-dependencies
included). It's a product decision — trivial, offline install — enforced by the
`zero dependencies` CI job, which fails if any crate enters `Cargo.lock`. If a
task seems to need a crate, implement it in `std` or push back.

## Commands

```bash
cargo build                 # debug build
cargo install --path .      # install `faceto` to ~/.cargo/bin

# Local quality gate (mirrors CI; run before pushing):
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
npx markdownlint-cli2 "**/*.md"

faceto render examples/sample.model.json    # → board.svg + index.html
faceto serve  examples/sample.model.json    # → live board at 127.0.0.1:8753
faceto genesis examples/sample.model.json   # → event-log.jsonl from a model
```

A local `.pre-commit-config.yaml` runs these gates automatically — install it
with `uvx pre-commit install` (see [`CONTRIBUTING.md`](CONTRIBUTING.md)).

## The pipeline (one file = one stage)

`JSON file → Model → SVG → HTML`, with `event-log.jsonl → replay → Model` as the
event-sourced path. Stages: `json` → `model` → `events` → `render` → `serve`,
plus `lint` (`Model → findings`, warn-only), `template.html` (the client) and
`main.rs` (CLI dispatch only).

## Three domain invariants (do not break)

- **`id` is the stable identity** — the comment join key *and* the diff key.
  Never derive identity from text or position; never renumber an `id`.
- **`col` is a global timeline coordinate** shared across all lanes
  (left→right = time), not a per-lane index.
- **`type` selects the lane and colour** from the fixed 8-lane grammar.

## Canonical docs (read these for depth)

| For… | Read |
| --- | --- |
| Architecture, pipeline detail, the invariants in full, server diff mechanism | [`CLAUDE.md`](CLAUDE.md) |
| How to write code here — Tidy First, CUPID & YAGNI, TDD+Reflect, Clean Code, commit style, toolchain | [`CODING_STANDARDS.md`](CODING_STANDARDS.md) |
| Contribution workflow, local checks, pre-commit setup | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Product strategy, anti-references, the "calm instrument" register | [`PRODUCT.md`](PRODUCT.md) |
| Visual system — colour grammar, typography, spacing, components, diff styling | [`DESIGN.md`](DESIGN.md) |

Commit discipline in one line: **separate structural "tidy" commits from
behavioural `feat`/`fix` ones** (Tidy First). No "Claude" signature in commit
messages.
