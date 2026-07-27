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
just docs                   # build the user manual (mdBook); fails on a promised page with no file

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
`diff_boards`, SVG label layout, the event log's replay / model round-trip / `compact`, server-side
id minting, and the server's hash/date/concurrency helpers). CI (`.github/workflows/ci.yml`) runs fmt, clippy + test (ubuntu on PRs, macOS added on `main`),
markdownlint, actionlint, a justfile lint, and the runtime-only dependency firewall — a `zero
dependencies` job (`cargo tree -e normal` is faceto-only; dev-deps like `proptest` are allowed)
and a `binary size budget` job; see [`docs/ci.md`](docs/ci.md). The toolchain is pinned in
`rust-toolchain.toml`; keep it, `Cargo.toml`'s `rust-version`, and the CI `toolchain:` inputs in
lockstep. For board behaviour not covered by tests, render `examples/sample.model.json` or run
`serve` and interact.

## Architecture

The pipeline is `event-log.jsonl → replay → Model → Scene → SVG → HTML`; the `model.json → Model`
path is the genesis/bootstrap input and a read-only `render` / `lint` source (serving always goes
through the log). **Seven Rust modules**, each one stage — `json`/`model`/`lint`/`scene` are single
files, `events`/`render`/`serve` are directories with a `mod.rs` plus one file per concern — plus
the client, which is not a Rust module and gets the eighth bullet below. (The count is stated this
way on purpose — it has been wrong twice, once as "Seven" over six names and once as "Eight" over
seven. `grep -c '^mod .*;' src/main.rs` settles it: **7**. Keep the `;` — a bare `^mod` prefix also matches
`mod tests {` at the foot of the file and answers 8, which is how you talk yourself back into the
off-by-one this sentence exists to prevent.)

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
- **`src/model.rs`** — the typed board (`Model`, `Element`, `Edge`, `Phase`) and `from_json`/`load`.
  This is where the board's domain rules live. A `Model` is *always* a board: the diff overlay is
  no longer optionals on it (F-board-vs-diff) but a separate type in `render`.
- **`src/lint.rs`** — ES-grammar lint. `lint(&Model) -> Vec<Finding>`, a pure graph pass (no IO,
  no clocks) that flags event-storming defects (event with no producer, policy with no input /
  output, non-terminal event with no outbound edge; plus, only when the board declares
  `level: design`, a command with no output). Warn-only at every level; each `Finding` is keyed on
  the stable `id` (the comment-sidecar join key). A real edge connects two distinct existing
  elements. Findings surface in `serve`'s `/comments` sidebar as `kind:"lint"` entries, computed on
  read and suppressed once the element is `resolved` (see `src/serve/`).
- **`src/scene.rs`** — the Scene IR (F-scene-ir). Geometric primitives (`Rect`/`Line`/`Text`/
  `Circle`/`Path` + a **nesting** `Group`), a `Scene`, and the single `render_scene` serializer.
  **Geometric, never semantic**: a sticky, a lane, a region are event-storming words that stay on
  the `render` side of the seam. Numbers stay numbers (`Val::Num`), so a scene can be read back —
  which is what the render tests assert against instead of scraping serialized SVG.
- **`src/render/`** — pure layout + the board's visual language (`render_svg` builds a `Scene`;
  `board_scene` is the `(Model, View) -> Scene` builder) and HTML wrapping (`render_html`). Holds
  the lane order (`LANES`), the colour grammar (`colour`), geometry constants (`COL_W`, `LANE_H`,
  etc.), label wrapping, the serif nameplate, and diff styling. It also owns the **diff overlay**
  (`diff.rs`: `diff_boards -> (Model, Overlay)` and the verdict enums) — comparing two boards is a
  render concern, so no board type carries a diff. It no longer writes SVG text — `scene` does,
  once, for every format.
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
  is a thin shell (head, static body DOM, four placeholders); the CSS and the ~1.6k lines of JS live
  in sibling files, split into nine cohesive modules (`core` → `layout` → `drag` → `connect` →
  `edit` → `region` → `sync` → `graph` → `main`). `src/render/html.rs` `include_str!`s them all and `concat!`s the JS
  modules — in that order, `"\n"`-separated — back into one classic `<script>` at build time (no
  bundler ships; the concatenation is one shared scope, behaviour-identical to the former inline
  script). `render_html` then
  does a two-stage fill: `__CONFIG__` into the script first (single-pass fill never re-scans an
  inserted value), then `__STYLE__` / `__SCRIPT__` / `__SVG__` / `__TITLE__` into the shell. The
  client fetches `/model-version` on load and on **Reload** (there is no polling loop and no SSE —
  `F-collab-sse` is the un-shipped push path), swaps in diff/plain SVGs, and posts comments/structural ops (falling
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

`render::diff_boards` joins old vs new on `id` and returns **two** values: the union board (a
plain `Model` — the new side's layout plus the old side's ghosts) and an `Overlay` judging each
element `added` / `removed` / `changed` (label differs) / `moved` (col, type, or in-lane `y` key
differs — compared through `model::y_key`, so "no y" and the neutral `0.5` are one state) /
`unchanged`. Layout follows the new side. The overlay is a *render* argument, passed beside the
board exactly like the `View` lens — never a field on it, never in the log.

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

## Documentation is part of the feature (do not skip this)

The user-facing surface is the **mdBook in [`docs/src/`](docs/src/)**, published to
<https://bastien-gallay.github.io/faceto/>. `CHANGELOG.md` and `ROADMAP.md` are *not* user
documentation — one is a release record, the other is the project's narrative. A user looking for
"how do I fold a region" reads the book.

**A change a user can notice is not done until the book says so, in the same PR.** That covers a
new or changed CLI flag, a new gesture or keybinding, a new event kind or comment kind, a new
model/log field, a new lint rule, a changed default, and any removal. It does *not* cover pure
refactors, structural "tidy" commits, or internal renames a user cannot observe — those touch no
page, by definition.

Where each change lands:

| You changed… | Update |
| --- | --- |
| a CLI verb, flag, default, or exit code | `docs/src/reference/cli/<verb>.md` (+ `cli.md` if the shared rules move) |
| a keybinding or a mouse gesture | `docs/src/board/keyboard.md` **and** the in-app sheet in `src/template.html` |
| an element/edge/region behaviour or guard | the matching `docs/src/board/*.md` page |
| a lint rule (added, removed, level-gated) | `docs/src/reference/lint-rules.md` |
| an `Event` variant or a `comment_to_events` kind | `docs/src/reference/event-log.md` **and** the write-contract table in [`.claude/skills/faceto-narrate/SKILL.md`](.claude/skills/faceto-narrate/SKILL.md) |
| a `model.json` field | `docs/src/reference/model-format.md` + `docs/schema/*.schema.json` |
| an export format | `docs/src/reference/cli/export.md` + `docs/src/agents/context-pack.md` |
| an invariant, or the pipeline | `docs/src/architecture/` |

Three traps this table exists to prevent, each one already met:

- **The keyboard sheet is duplicated.** `src/template.html`'s `#help` dialog and
  `docs/src/board/keyboard.md` are two hand-maintained lists of the same gestures. Change a
  binding and you must change both — there is no generator keeping them honest yet.
- **The narrate skill documents `comment_to_events`.** Its write-contract table is a hand-copy of
  the code. Add a comment `kind` without updating it and the agent will not know the action
  exists (this is exactly how the shipped `connect`/`disconnect` kinds went unlisted).
- **The schemas are documentation too.** An additive model field that never reaches
  `docs/schema/` silently stops being discoverable.

Write against **the code, not the CHANGELOG** — the entry describes intent at merge time, the code
describes present behaviour. **And not against the code's own comments either**: a doc comment can
describe a design that was never built. Six places here — `SKILL.md`, two design notes, four
`src/serve/` comments and a roadmap row — described a "~1 Hz poll" of `/model-version` repainting
the board on its own; `git log -S setInterval` proved no such loop was ever written. When a comment
asserts a *mechanism*, grep for the identifier that would implement it before repeating the claim.
**And re-read your own replacement against the code, not against the text it replaced.** A commit
titled *"correct three claims the code does not support"* introduced a fourth of the same shape: it
rewrote a stale "what's next" paragraph and listed two features as upcoming that had shipped weeks
earlier — two commits after correcting exactly that defect elsewhere in the same PR. Rewriting a
stale claim puts you in the mindset of the claim; the fix is to verify the new sentence from
scratch, the way you verified that the old one was wrong. Document what is shipped; when a feature is mid-reformulation, say so
on the page and link the issue rather than describing an interface about to move. Internal
artefacts (`docs/notes/`, `docs/F-*-plan.md`, `.personal/**`) stay out of `docs/src`: the book
publishes decisions, not deliberations.

`create-missing = false` in `docs/book.toml` plus CI's `docs book` job mean a `SUMMARY.md` entry
with no file fails the build — a promised page can never ship as a silent empty stub. Build the
book locally with `just docs` (or `just docs-serve` for live reload).

## Canonical docs (read these for depth)

| For… | Read |
| --- | --- |
| Event-sourcing rationale + locked decisions | [`docs/event-sourcing-status.md`](docs/event-sourcing-status.md) |
| How to write code here — Tidy First, CUPID & YAGNI, TDD+Reflect, Clean Code, commit style, toolchain | [`CODING_STANDARDS.md`](CODING_STANDARDS.md) |
| Contribution workflow, local checks, pre-commit setup | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| CI jobs in full | [`docs/ci.md`](docs/ci.md) |
| Product strategy, anti-references, the "calm instrument" register | [`PRODUCT.md`](PRODUCT.md) |
| Visual system — colour grammar, typography, spacing, components, diff styling | [`DESIGN.md`](DESIGN.md) |
| What a *user* is told the tool does (the published manual) | [`docs/src/`](docs/src/) → <https://bastien-gallay.github.io/faceto/> |

Commit discipline in one line: **separate structural "tidy" commits from behavioural `feat`/`fix`
ones** (Tidy First) — and a behavioural commit carries its documentation change with it, not in a
follow-up. No "Claude" signature in commit messages.
