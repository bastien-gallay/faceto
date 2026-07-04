<!-- markdownlint-disable MD013 -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`faceto` turns a typed JSON model into an interactive HTML+SVG workshop board (event storming is
the first format). It renders a static board or serves a live one with a click→comment sidecar and
an in-page diff. The whole point is "a simple typed file you think through with an LLM."

The durable record is an **append-only event log** (`event-log.jsonl`); the `Model` is a
projection replayed from it, and `model.json` is a derived/bootstrap form. Comments are
first-class events. This event-sourcing inversion is the current spine — see
[`docs/event-sourcing-status.md`](docs/event-sourcing-status.md) for the full rationale and the
locked decisions.

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

faceto render examples/sample.model.json     # → board.svg + index.html next to the model
faceto lint   examples/sample.model.json     # → ES-grammar findings (warn-only, exits 0)
faceto serve  examples/sample.model.json     # → live board at http://127.0.0.1:8753
faceto serve  path/to/model.json -p 9000     # custom port

# Event-sourced flow (genesis creates the log the next two commands consume):
faceto genesis examples/sample.model.json    # migrate a model.json → examples/event-log.jsonl
faceto render  examples/event-log.jsonl      # render/serve also accept a log (by extension)
faceto compact examples/event-log.jsonl      # fold a log to a snapshot, bounding replay
```

Tests are in-file under `#[cfg(test)] mod tests` (json parsing/roundtrip, the id-keyed
`diff_models`, SVG label layout, the event log's replay / model round-trip / `compact`, server-side
id minting, and the server's hash/date/concurrency helpers). CI (`.github/workflows/
ci.yml`) runs fmt, the clippy + test matrix on macOS/Windows/Linux, markdownlint, actionlint, and a
`zero dependencies` job that fails if a crate enters `Cargo.lock`. The toolchain is pinned in
`rust-toolchain.toml`; keep it, `Cargo.toml`'s `rust-version`, and the CI `toolchain:` inputs in
lockstep. For board behaviour not covered by tests, render `examples/sample.model.json` or run
`serve` and interact.

## Architecture

The pipeline is `event-log.jsonl → replay → Model → SVG → HTML`; the `model.json → Model` path is
the genesis/bootstrap input and a read-only `render` / `lint` source (serving always goes through
the log). Seven source files, each one stage:

- **`src/json.rs`** — minimal JSON parser/serializer (`parse`, `to_string`, the `Json` enum with
  `get`/`as_str`/`as_f64`/`as_bool`/`as_array`). Everything else builds on this.
- **`src/events.rs`** — the event-sourced spine. The `Event` enum (one JSON object per log line),
  `parse_log`/`read_log`, `replay(&[Event]) -> Model` (the projection), `from_model` (genesis/
  migration), `comment_to_events` (map one posted comment to the events it implies — the single
  source of truth shared with `serve.rs`'s `POST /comment`), and `compact`
  (fold a log to a `LogCompacted` marker + genesis snapshot). Schema evolves additively — unknown
  event kinds are skipped and unknown fields ignored on read (forward compatibility) — and a
  renamed event *kind* is migrated forward at the `upcast` read-path seam (backward compatibility;
  fields evolve additively, since a renamed field is indistinguishable from a new one by shape).
- **`src/model.rs`** — the typed board (`Model`, `Element`, `Edge`, `Phase`), `from_json`/`load`,
  and `diff_models`. This is where the board's domain rules live.
- **`src/lint.rs`** — ES-grammar lint. `lint(&Model) -> Vec<Finding>`, a pure graph pass (no IO,
  no clocks) that flags event-storming defects (event with no producer, policy with no input /
  output, non-terminal event with no outbound edge; plus, only when the board declares
  `level: design`, a command with no output). Warn-only at every level; each `Finding` is keyed on
  the stable `id` (the comment-sidecar join key). A real edge connects two distinct existing
  elements. Findings surface in `serve`'s `/comments` sidebar as `kind:"lint"` entries, computed on
  read and suppressed once the element is `resolved` (see `serve.rs`).
- **`src/render.rs`** — pure layout + SVG generation (`render_svg`) and HTML wrapping
  (`render_html`). Holds the lane order (`LANES`), the colour grammar (`colour`), geometry
  constants (`COL_W`, `LANE_H`, etc.), label wrapping, the serif nameplate, and diff styling.
- **`src/serve.rs`** — std-only HTTP server, **event-log-only** (F-auto-genesis killed legacy
  mode: `main` resolves any `model.json` to its sibling `event-log.jsonl` via `serve_log_path`
  before calling `serve`, auto-running genesis if no log exists yet, so the server only ever
  mutates the log). Routes: `GET /` (page), `GET /board.svg` (re-rendered each request,
  `?base=<version>` produces a diff overlay), `GET /model-version`, `GET /comments`, `GET /health`,
  `POST /comment`. `POST /comment` appends an *event* (the comment's `kind` maps to
  `ElementAdded`/`ElementMoved`/`ElementRenamed`/`HotspotResolved`/`ElementRemoved` (`drop`)/
  `ElementAnnotated`); `add` mints a server-side type-prefixed id. All appends serialize through
  one mutex so concurrent posts never interleave (H4).
- **`src/template.html`** — the client, embedded into the binary via `include_str!` in
  `render.rs`. `render_html` substitutes `__SVG__`, `__TITLE__`, and `__CONFIG__`. Client polls
  `/model-version`, swaps in diff/plain SVGs, and posts comments/structural ops (falling back to
  `localStorage` when offline — offline structural ops are local-only, not resynced).

`src/main.rs` is the CLI dispatch only (`render` / `lint` / `serve` / `genesis` / `compact` /
`help` / `version`).

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
(label differs) / `moved` (col, type, or in-lane `y` key differs — compared through
`model::y_key`, so "no y" and the neutral `0.5` are one state) / `unchanged`; layout follows
the new side.

### Event-sourced spine (do not break these)

- **The log is append-only truth.** Append events; never rewrite history in place. The one
  exception is `faceto compact`, which folds the log to an equivalent shorter snapshot (and backs
  up the prior log to `<log>.bak`). `event-log.jsonl` is **tracked** in git; `board.svg` /
  `index.html` / `comments.jsonl` stay ignored (derived).
- **`replay` is pure and deterministic** — same log → same `Model`. Keep it free of clocks/IO. New
  `Event` variants must extend `parse_event`/`to_json`/`replay` together (the compiler enforces the
  match), and unknown kinds must keep being skipped on read. **Evolve the schema additively**
  (new optional field, or new kind) so old and new logs stay mutually replayable; a renamed *kind*
  is the only backward-incompatible change and belongs in the `upcast` seam (a renamed *field*
  can't be shape-detected, so evolve fields additively; never repurpose a kind's meaning in place).
- **Ids are minted server-side**, never by the client: `<PREFIX><N>`, one past the highest suffix
  ever added under that lane's prefix in the log (removed-but-not-compacted ids stay reserved),
  computed under the appends lock so concurrent adds can't collide (`mint_id` in `serve.rs`;
  prefixes from `render::lane_prefix`, the single source of truth alongside `LANES`).

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
