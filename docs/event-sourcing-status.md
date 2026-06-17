<!-- markdownlint-disable MD013 -->

# Event-sourcing inversion — status & handoff

Status: **in progress** · Branch: `feat/event-sourced-log` · Last updated: 2026-06-17

## The inversion (what & why)

We are inverting the stance in [`source-of-truth.md`](source-of-truth.md). That note said
*model = truth, comments = disposable inbox*. We reverse it:

> **`event-log.jsonl` is the only durable record and the only write path. The `Model` is a
> projection replayed from it. `model.json` becomes derived, disposable output. Comments
> become first-class events.**

This is classic event sourcing applied to faceto itself. The design was worked out *on a
faceto board* — see [`../examples/faceto-event-sourced.model.json`](../examples/faceto-event-sourced.model.json)
(faceto event-storming its own inversion). Render or serve that file to see the picture.

### Decisions locked (resolved hotspots on that board)

- **H2 — model.json's role → gone as truth (pure event sourcing).** The log is authoritative;
  `model.json` is derived output. You append events instead of editing a file. An existing
  `model.json` enters the world as a *genesis batch* of events (migration = bootstrap).
- **H1 — replay cost → cache the projection.** `version = fnv12(raw log bytes)`; reuse
  `serve.rs`'s existing recent-models ring keyed on that hash; replay runs only on a cache
  miss (i.e. when a new event lands). Snapshotting (a future `compact`) will bound replay length.

## What is built (this branch)

| File | Change |
| --- | --- |
| `src/events.rs` *(new)* | `Event` enum (10 variants); `parse_log`/`read_log`; `replay(&[Event]) -> Model`; `from_model(&Model) -> Vec<Event>` (genesis/migration); JSON (de)serialization. 7 in-file tests incl. a model→events→model round-trip. |
| `src/main.rs` | `render`/`serve` accept a log by extension (`is_log_path`). New verb **`faceto genesis [MODEL]`** writes `event-log.jsonl` next to a model (refuses to clobber). |
| `src/serve.rs` | `current()` replays in log mode (version hashes raw bytes → ring caches the projection). `POST /comment` **appends an event** (`move`→`ElementMoved`, `resolve`→`HotspotResolved`, `rename`→`ElementRenamed`, else `ElementAnnotated`). `GET /comments` projects feedback events back for the sidebar. |

The pipeline is now `event-log.jsonl → replay → Model → SVG → HTML`, with `model.json → Model`
still supported for the legacy/genesis path.

### Event schema (one JSON object per line, discriminated by `"event"`)

```text
BoardTitled {title}
PhaseAdded {label, fromCol, toCol}
ElementAdded {id, type, label, col?, detail?}
ElementRenamed {id, label}
ElementMoved {id, col?, type?}
ElementAnnotated {id, text}        # the former "comment" — comments are now events
HotspotResolved {id, resolution}   # sets resolved + detail on replay
ElementRemoved {id}                # also drops touching edges
EdgeAdded {src, dst}
EdgeRemoved {src, dst}
```

Read policy: blank lines skipped; malformed JSON is a hard error; **unknown event kinds are
skipped** (forward compatibility — partial answer to H3).

## Verified end-to-end

- `faceto genesis sample.model.json` → 24 events; `faceto render event-log.jsonl` → 11
  elements (parity with the model).
- Live: `POST /comment {elemId:"H1", kind:"resolve", ...}` → appended a `HotspotResolved`
  line → `model-version` bumped → re-render shows `H1` as `sticky hotspot resolved` with the
  note. **The comment is the persistence; the model is reconstructed from it.**
- `cargo fmt --check` · `cargo clippy --all-targets -D warnings` · `cargo test` (29 passed,
  7 new) — all green.

## Pick up here tomorrow (open work, roughly in order)

1. **H4 — concurrency / append ordering.** `POST /comment` in `serve.rs` appends with no lock;
   concurrent posts can interleave lines. Add a write mutex (or `O_APPEND` is atomic per-line
   on most platforms — verify, or just serialize through a `Mutex` in `Ctx`).
2. **H6 — id minting for `add`.** The client can post a comment but there is no "add element"
   command yet. Decide who mints the new `id` (server-side counter? client uuid-ish?) and add
   an `add` kind → `ElementAdded` mapping in `comment_to_event`.
3. **`faceto compact`** — snapshot the log (fold to a minimal genesis batch) to bound replay;
   this is the concrete form of H1's "snapshot" escape hatch. (On the board: `LogCompacted`.)
4. **Client (`src/template.html`)** — in log mode, server-side `ElementMoved` already moves the
   sticky, so the client's `replayMoves` is redundant (harmless). Audit the offline/localStorage
   fallback path against the new event semantics.
5. **Reconcile the docs** — `source-of-truth.md` is now superseded (banner added). Decide
   whether to rewrite it or fold it into this note once the inversion lands. Update the
   architecture section of `CLAUDE.md` (now six source files; `events.rs` is the new spine)
   **when the branch is ready to merge**, not before.

## Notes / caveats

- `src/render.rs` and `src/template.html` had **pre-existing** uncommitted changes from before
  this work began; they are unrelated to the inversion and were intentionally left untouched.
- `.gitignore` is already correct: `event-log.jsonl` is **tracked** (it is the new truth);
  `board.svg` / `index.html` / `comments.jsonl` stay ignored (derived).
