<!-- markdownlint-disable MD013 -->

# AGENTS.md

Canonical guide for any coding agent or tool working in this repo — the single source of
truth it orients you and holds the project's standing guidance. Claude Code reads it via
`@AGENTS.md` in [`CLAUDE.md`](CLAUDE.md); Google Antigravity (Gemini) loads it automatically
as a project-level rule file. Keep the substance here so the tools don't drift.

## What this is

`faceto` turns a typed JSON model into an interactive HTML+SVG workshop board (event storming is
the first format). It renders a static board or serves a live one with a click→comment sidecar and
an in-page diff. The whole point is "a simple typed file you think through with an LLM."

The durable record is an **append-only event log** (`<name>.event-log.jsonl`, named after the
model basename so sibling boards in one directory own separate logs); the `Model` is a
projection replayed from it, and `model.json` is a derived/bootstrap form. Comments are
first-class events. This event-sourcing inversion is the current spine — see
[`docs/event-sourcing-status.md`](docs/event-sourcing-status.md) for the full rationale and the
locked decisions.

For how to write code here — Tidy First, CUPID & YAGNI, TDD+Reflect, Clean Code, commit style — see
[`CODING_STANDARDS.md`](CODING_STANDARDS.md).

## Hard constraint: zero runtime dependencies

`faceto`'s shipped binary is **pure Rust standard library — no runtime crates, ever.** This is a
deliberate product decision (trivial offline install), not an accident. Do not add a
`[dependencies]` entry to `Cargo.toml`. Consequences you must respect:

- JSON is parsed/serialized by the hand-written `src/json.rs` (not serde).
- The HTTP server is `std::net::TcpListener` + threads (`src/serve/http.rs`), not a web framework.
- Dates (`now_iso`) and content hashing (`fnv12`, FNV-1a) are implemented by hand in `src/serve/hash.rs`.

**Dev-dependencies are the one exception** — test-only crates never enter the binary or the offline
install, so they don't touch the promise (`proptest` powers the property-based tests). The CI `zero
dependencies` job enforces exactly this line: it checks the *normal* (runtime) dependency tree via
`cargo tree -e normal`, which excludes dev-deps. If runtime code seems to need a crate, implement it
in std or push back — and **ask before adding even a dev-dependency**.

## Commands

```bash
cargo build                 # debug build
cargo build --release       # release (opt-level 2, see Cargo.toml)
cargo install --path .      # install `faceto` to ~/.cargo/bin

# Local quality gate (mirrors CI; run before pushing):
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
npx markdownlint-cli2 "**/*.md"

faceto render examples/sample.model.json       # → sample.svg + sample.html next to the model
faceto lint   examples/sample.model.json       # → ES-grammar findings (warn-only, exits 0)
faceto serve  examples/sample.model.json       # → live board at http://127.0.0.1:8753
faceto serve  path/to/model.json -p 9000       # custom port

# Event-sourced flow (genesis creates the log the next two commands consume):
faceto genesis examples/sample.model.json      # migrate → examples/sample.event-log.jsonl
faceto render  examples/sample.event-log.jsonl # render/serve also accept a log (by extension)
faceto compact examples/sample.event-log.jsonl # fold a log to a snapshot, bounding replay
```

A local `.pre-commit-config.yaml` runs these gates automatically — install it with
`uvx pre-commit install` (see [`CONTRIBUTING.md`](CONTRIBUTING.md)).

Tests are in-file under `#[cfg(test)] mod tests` (json parsing/roundtrip, the id-keyed
`diff_models`, SVG label layout, the event log's replay / model round-trip / `compact`, server-side
id minting, and the server's hash/date/concurrency helpers). CI (`.github/workflows/ci.yml`) runs fmt, clippy + test (ubuntu on PRs, macOS added on `main`),
markdownlint, actionlint, a justfile lint, and the runtime-only dependency firewall — a `zero
dependencies` job (`cargo tree -e normal` is faceto-only; dev-deps like `proptest` are allowed)
and a `binary size budget` job; see [`docs/ci.md`](docs/ci.md). The toolchain is pinned in
`rust-toolchain.toml`; keep it, `Cargo.toml`'s `rust-version`, and the CI `toolchain:` inputs in
lockstep. For board behaviour not covered by tests, render `examples/sample.model.json` or run
`serve` and interact.

## Architecture

The pipeline is `event-log.jsonl → replay → Model → SVG → HTML`; the `model.json → Model` path is
the genesis/bootstrap input and a read-only `render` / `lint` source (serving always goes through
the log). Seven modules, each one stage (`json`/`model`/`lint` are single files; `events`/`render`/
`serve` are directories with a `mod.rs` plus one file per concern):

- **`src/json.rs`** — minimal JSON parser/serializer (`parse`, `to_string`, the `Json` enum with
  `get`/`as_str`/`as_f64`/`as_bool`/`as_array`). Everything else builds on this.
- **`src/events/`** — the event-sourced spine. The `Event` enum (one JSON object per log line),
  `parse_log`/`read_log`, `replay(&[Event]) -> Model` (the projection), `from_model` (genesis/
  migration), `comment_to_events` (map one posted comment to the events it implies — the single
  source of truth shared with `serve`'s `POST /comment`), and `compact`
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
  read and suppressed once the element is `resolved` (see `src/serve/`).
- **`src/render/`** — pure layout + SVG generation (`render_svg`) and HTML wrapping
  (`render_html`). Holds the lane order (`LANES`), the colour grammar (`colour`), geometry
  constants (`COL_W`, `LANE_H`, etc.), label wrapping, the serif nameplate, and diff styling.
- **`src/serve/`** — std-only HTTP server, **event-log-only** (F-auto-genesis killed legacy
  mode: `main` resolves any `model.json` to its sibling `<name>.event-log.jsonl` via `serve_log_path`
  before calling `serve`, auto-running genesis if no log exists yet, so the server only ever
  mutates the log). Routes: `GET /` (page), `GET /board.svg` (re-rendered each request,
  `?base=<version>` produces a diff overlay), `GET /model-version`, `GET /comments`, `GET /health`,
  `POST /comment`. `POST /comment` appends an *event* (the comment's `kind` maps to
  `ElementAdded`/`ElementMoved`/`ElementRenamed`/`HotspotResolved`/`ElementRemoved` (`drop`)/
  `ElementAnnotated`); `add` mints a server-side type-prefixed id. All appends serialize through
  one mutex so concurrent posts never interleave (H4).
- **`src/template.html` + `src/client/*.js` + `src/client/style.css`** — the client. `template.html`
  is a thin shell (head, static body DOM, four placeholders); the CSS and the ~1.3k lines of JS live
  in sibling files, split into cohesive modules (`core` → `layout` → `drag` → `edit` → `region` →
  `sync` → `graph` → `main`). `src/render/html.rs` `include_str!`s them all and `concat!`s the JS
  modules — in that order, `"\n"`-separated — back into one classic `<script>` at build time (no
  bundler ships; the concatenation is one shared scope, behaviour-identical to the former inline
  script). `render_html` then
  does a two-stage fill: `__CONFIG__` into the script first (single-pass fill never re-scans an
  inserted value), then `__STYLE__` / `__SCRIPT__` / `__SVG__` / `__TITLE__` into the shell. The
  client polls `/model-version`, swaps in diff/plain SVGs, and posts comments/structural ops (falling
  back to `localStorage` when offline — offline structural ops are local-only, not resynced).
  Pure helpers are checked by `tests/js/board-logic.test.mjs` (plain node, no deps).

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
  `aggregate`, `event`, `policy`, `readmodel`, `external`, `hotspot`. Keep `LANES` (`src/render/`) and
  this set in sync.

`diff_models` joins old vs new on `id` and tags each element `added` / `removed` / `changed`
(label differs) / `moved` (col, type, or in-lane `y` key differs — compared through
`model::y_key`, so "no y" and the neutral `0.5` are one state) / `unchanged`; layout follows
the new side.

### Event-sourced spine (do not break these)

The append-only-truth / pure-`replay` / server-side-id-minting invariants live in
[`.claude/rules/event-spine.md`](.claude/rules/event-spine.md) (path-scoped: auto-loads in Claude
Code when you edit `src/events/` or `src/serve/`). **Read it before touching the log, replay, or
the append path.**

## Server diff mechanism

`serve` keeps a small ring (`CACHE_MAX = 12`) of recently-served models keyed by FNV content hash
(`fnv12`). `GET /board.svg?base=<oldhash>` looks up the baseline in that ring and renders a diff
overlay against it. If the baseline has aged out of the ring, it falls back to the plain current
board. No git, no persistence — the ring is in-memory only.

## Design Context

`faceto` carries an impeccable design context (register: `product`; personality: a **calm
instrument**). The strategic principles, anti-references, and visual system live in
[`PRODUCT.md`](PRODUCT.md) + [`DESIGN.md`](DESIGN.md), summarised in
[`.claude/rules/ui-design.md`](.claude/rules/ui-design.md) (path-scoped: auto-loads in Claude Code
when you edit `src/template.html` or `src/render/`). **Read all three before any UI work.**

## Canonical docs (read these for depth)

| For… | Read |
| --- | --- |
| Event-sourcing rationale + locked decisions | [`docs/event-sourcing-status.md`](docs/event-sourcing-status.md) |
| How to write code here — Tidy First, CUPID & YAGNI, TDD+Reflect, Clean Code, commit style, toolchain | [`CODING_STANDARDS.md`](CODING_STANDARDS.md) |
| Contribution workflow, local checks, pre-commit setup | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| CI jobs in full | [`docs/ci.md`](docs/ci.md) |
| Product strategy, anti-references, the "calm instrument" register | [`PRODUCT.md`](PRODUCT.md) |
| Visual system — colour grammar, typography, spacing, components, diff styling | [`DESIGN.md`](DESIGN.md) |

Commit discipline in one line: **separate structural "tidy" commits from behavioural `feat`/`fix`
ones** (Tidy First). No "Claude" signature in commit messages.
