<!-- markdownlint-disable MD013 -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`faceto` turns a typed JSON model into an interactive HTML+SVG workshop board (event storming is
the first format). It renders a static board or serves a live one with a click→comment sidecar and
an in-page diff. The whole point is "a simple typed file you think through with an LLM."

For how to write code here — Tidy First, CUPID & YAGNI, TDD+Reflect, Clean Code, commit style — see
[`CODING_STANDARDS.md`](CODING_STANDARDS.md). [`AGENTS.md`](AGENTS.md) is the short cross-tool entry
point that maps the rest of the docs.

## Hard constraint: zero dependencies

`faceto` is **pure Rust standard library — no crates, ever.** This is a deliberate product
decision (trivial offline install), not an accident. Do not add a dependency to `Cargo.toml`.
Consequences you must respect:

- JSON is parsed/serialized by the hand-written `src/json.rs` (not serde).
- The HTTP server is `std::net::TcpListener` + threads (`src/serve.rs`), not a web framework.
- Dates (`now_iso`) and content hashing (`fnv12`, FNV-1a) are implemented by hand in `src/serve.rs`.

If a task seems to need a crate, implement it in std or push back.

## Commands

```bash
cargo build                 # debug build
cargo build --release       # release (opt-level 2, see Cargo.toml)
cargo install --path .      # install `faceto` to ~/.cargo/bin

# Local quality gate (mirrors CI; run before pushing):
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

faceto render examples/sample.model.json    # → board.svg + index.html next to the model
faceto serve  examples/sample.model.json     # → live board at http://127.0.0.1:8753
faceto serve  path/to/model.json -p 9000     # custom port
```

Tests are in-file under `#[cfg(test)] mod tests` (json parsing/roundtrip, the id-keyed
`diff_models`, SVG label layout, and the server's hash/date helpers). CI (`.github/workflows/
ci.yml`) runs fmt, the clippy + test matrix on macOS/Windows/Linux, markdownlint, actionlint, and a
`zero dependencies` job that fails if a crate enters `Cargo.lock`. The toolchain is pinned in
`rust-toolchain.toml`; keep it, `Cargo.toml`'s `rust-version`, and the CI `toolchain:` inputs in
lockstep. For board behaviour not covered by tests, render `examples/sample.model.json` or run
`serve` and interact.

## Architecture

The pipeline is `JSON file → Model → SVG → HTML`. Five source files, each one stage:

- **`src/json.rs`** — minimal JSON parser/serializer (`parse`, `to_string`, the `Json` enum with
  `get`/`as_str`/`as_f64`/`as_bool`/`as_array`). Everything else builds on this.
- **`src/model.rs`** — the typed board (`Model`, `Element`, `Edge`, `Phase`), `from_json`/`load`,
  and `diff_models`. This is where the domain rules live.
- **`src/render.rs`** — pure layout + SVG generation (`render_svg`) and HTML wrapping
  (`render_html`). Holds the lane order (`LANES`), the colour grammar (`colour`), geometry
  constants (`COL_W`, `LANE_H`, etc.), label wrapping, and diff styling.
- **`src/serve.rs`** — std-only HTTP server. Routes: `GET /` (page), `GET /board.svg`
  (re-rendered each request, `?base=<version>` produces a diff overlay), `GET /model-version`,
  `GET /comments`, `GET /health`, `POST /comment` (appends to `comments.jsonl`).
- **`src/template.html`** — the client, embedded into the binary via `include_str!` in
  `render.rs`. `render_html` substitutes `__SVG__` and `__TITLE__`. Client polls `/model-version`,
  swaps in diff/plain SVGs, and posts comments (falling back to `localStorage` when offline).

`src/main.rs` is the CLI dispatch only (`render` / `serve` / `help` / `version`).

## Domain invariants (do not break these)

These three rules are the contract the comment sidecar and the diff rely on — most subtle bugs
come from violating them:

- **`id` is the stable identity.** It is the comment join key *and* the diff key. Never derive
  identity from text or position. The model file convention is: never renumber an `id`, only add.
- **`col` is a global timeline coordinate** shared across all lanes (left→right = time), *not* a
  per-lane index. Order within a lane is just sort-by-`col`. Missing `col` auto-assigns in file
  order.
- **`type` selects the lane and colour** from the fixed 8-lane grammar: `actor`, `command`,
  `aggregate`, `event`, `policy`, `readmodel`, `external`, `hotspot`. Keep `LANES` (render.rs) and
  this set in sync.

`diff_models` joins old vs new on `id` and tags each element `added` / `removed` / `changed`
(label differs) / `moved` (col or type differs) / `unchanged`; layout follows the new side.

## Server diff mechanism

`serve` keeps a small ring (`CACHE_MAX = 12`) of recently-served models keyed by FNV content hash
(`fnv12`). `GET /board.svg?base=<oldhash>` looks up the baseline in that ring and renders a diff
overlay against it. If the baseline has aged out of the ring, it falls back to the plain current
board. No git, no persistence — the ring is in-memory only.

## Design Context

`faceto` carries an impeccable design context. **Register: `product`** — the live HTML+SVG
board is app UI that serves the event-storming workflow, not a brand/marketing surface.
Personality: a **calm instrument** (calm, precise, faithful) — the model is the subject,
the UI is glass. Strategic principles and anti-references (no SaaS-dashboard chrome, no
heavy branded chrome, no Miro/FigJam maximalism, no toy/childish look) live in
[`PRODUCT.md`](PRODUCT.md). Visual system (the 8-lane colour grammar, typography, spacing,
components, diff styling) is captured in [`DESIGN.md`](DESIGN.md). Read both before any UI
work on `src/template.html` or `src/render.rs`.
