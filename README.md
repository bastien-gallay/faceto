<!-- markdownlint-disable MD013 -->

# faceto

[![CI](https://github.com/bastien-gallay/faceto/actions/workflows/ci.yml/badge.svg)](https://github.com/bastien-gallay/faceto/actions/workflows/ci.yml)

**A simple typed file → a visual workshop board you think through with an LLM.**

Point it at a JSON model; get an interactive HTML+SVG board you can read at a glance and
**click any element → write a short note → have your next session adjust the model**.
Event storming is the first board format; C4, story mapping, impact mapping and friends are
the direction. Pure Rust standard library — **zero dependencies, offline, one fast install**.

The name reads two ways, both true: **face-to**(-face — the thing you discuss *with* the model)
and **facet-o** (many facets cut from one typed source).

## Install

```bash
cargo install --path .          # puts `faceto` on your PATH (~/.cargo/bin)
```

No network, no crates to download — it builds from the standard library alone.

## Use

```bash
# Render a static board (board.svg + index.html) next to the model:
faceto render examples/sample.model.json
open examples/index.html

# Live board with the sync comment sidecar + in-page diff:
faceto serve examples/sample.model.json        # → http://127.0.0.1:8753
faceto serve path/to/model.json -p 9000        # custom port
```

Click a sticky → a modal opens → pick a kind (`comment` / `add` / `split` / `rename` / `drop` /
`move` / `question` / `resolve`) → type a short note → **Save**. With `serve` running, the note is
appended to `comments.jsonl` (next to the model) and echoed to stdout. Without a server the page
still works offline (localStorage + **Export comments**).

## The model file

```json
{
  "title": "my board",
  "phases": [{ "label": "begin", "fromCol": 0, "toCol": 1 }],
  "elements": [
    { "id": "E1", "type": "event", "label": "ItemAdded", "col": 2 },
    { "id": "H1", "type": "hotspot", "label": "open question (detail here)", "col": 3 }
  ],
  "edges": [["E1", "H1"]]
}
```

- **`type`** is the colour grammar / lane: `actor` · `command` · `aggregate` · `event` · `policy` ·
  `readmodel` · `external` · `hotspot`.
- **`col`** is a *global* timeline coordinate shared across all lanes (left→right = time). Order
  within a lane is just `sort by col`. Missing `col` auto-assigns in file order.
- **`id`** is the stable identity — the comment join key and the diff key. Never renumber; only add.
- A trailing `(parenthetical)` or an explicit **`detail`** field becomes the sticky's smaller
  second line. A hotspot with **`"resolved": true`** goes quiet (grey + check) instead of loud red.

## Reload shows what changed

**Reload** re-pulls comments and, when `model.json` changed under you, redraws the board with a
**diff overlay against the version you were just looking at** — no git, no page reload. Joined on
`id`: a reworded or relocated sticky reads as **changed** / **moved**, not drop-plus-add. **Plain**
clears the overlay and re-baselines. The server keeps a small ring of recently-served models keyed
by content hash; if a baseline has aged out it falls back to the plain current board.

## Why zero dependencies

`faceto` is a working instrument you reach for mid-thought, so install has to be trivial and
offline. The model is parsed by a small hand-written JSON module (`src/json.rs`) — fitting, since
the whole premise is *a simple typed file*. The server is `std::net` only. A CI job (`zero
dependencies`) fails the build if a crate ever sneaks into the dependency tree.

## Development

```bash
cargo test --all-targets                          # unit tests
cargo fmt --all --check                           # formatting
cargo clippy --all-targets -- -D warnings         # lints
```

These mirror CI, which also runs the test + clippy matrix on macOS, Windows, and Linux. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the (one) hard rule and [CLAUDE.md](CLAUDE.md) for the
architecture and domain invariants.

## Status

Extracted from the daily-ops inception event-storm harness. v0.1 ports the event-storm renderer,
the comment sidecar, and the live in-page diff faithfully. Next: more board formats and a `reorder`
affordance (per-sticky nudge + backward-edge contradiction styling).
