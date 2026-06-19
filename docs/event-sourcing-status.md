<!-- markdownlint-disable MD013 -->

# Event-sourcing inversion — status & handoff

Status: **feature-complete (all hotspots H1–H6 resolved); pending merge** · Branch:
`feat/event-sourced-log` · Last updated: 2026-06-19

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
- **H4 — concurrency / append ordering → serialize through an `appends` mutex.** Every append
  (log events *and* legacy `comments.jsonl`) goes through `Ctx::append_line`, which holds a
  dedicated `Mutex<()>` and writes the whole line+`\n` in a single `write_all`. Concurrent
  `POST /comment` handlers (one thread each) can no longer interleave mid-line. Covered by
  `concurrent_appends_never_interleave` (8 threads × 50 appends, all lines land whole).

## What is built (this branch)

| File | Change |
| --- | --- |
| `src/events.rs` *(new)* | `Event` enum (10 variants); `parse_log`/`read_log`; `replay(&[Event]) -> Model`; `from_model(&Model) -> Vec<Event>` (genesis/migration); JSON (de)serialization. 7 in-file tests incl. a model→events→model round-trip. |
| `src/main.rs` | `render`/`serve` accept a log by extension (`is_log_path`). New verb **`faceto genesis [MODEL]`** writes `event-log.jsonl` next to a model (refuses to clobber). |
| `src/serve.rs` | `current()` replays in log mode (version hashes raw bytes → ring caches the projection). `POST /comment` **appends an event** (`move`→`ElementMoved` ×2 on a swap, `resolve`→`HotspotResolved`, `rename`→`ElementRenamed`, `drop`→`ElementRemoved`, else `ElementAnnotated`). `GET /comments` projects feedback events back for the sidebar. |

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
LogCompacted {folded}              # provenance marker from `compact`; no-op on replay
```

Read policy: blank lines skipped; malformed JSON is a hard error; **unknown event kinds are
skipped** and **unknown fields are ignored** (forward compatibility); a renamed kind/field is
migrated forward at the `upcast` seam (backward compatibility). See **H3** below.

## Verified end-to-end

- `faceto genesis sample.model.json` → 24 events; `faceto render event-log.jsonl` → 11
  elements (parity with the model).
- Live: `POST /comment {elemId:"H1", kind:"resolve", ...}` → appended a `HotspotResolved`
  line → `model-version` bumped → re-render shows `H1` as `sticky hotspot resolved` with the
  note. **The comment is the persistence; the model is reconstructed from it.**
- `cargo fmt --check` · `cargo clippy --all-targets -D warnings` · `cargo test` (30 passed,
  1 new H4 concurrency test) — all green.

### H6 — id minting for `add` → **server mints, derived from the log.** ✅

A `POST /comment {kind:"add", type:<lane>, text:<label>, col?, detail?}` appends an
`ElementAdded`. The server mints the id — **not** a client uuid — to preserve the board's
human-readable, type-prefixed grammar (`actor`→`X`, `command`→`C`, `aggregate`→`A`,
`event`→`E`, `policy`→`P`, `readmodel`→`R`, `external`→`G`, `hotspot`→`H`; the prefixes live in
`render::lane_prefix` next to `LANES`, and `serve::id_prefix`/`mint_id` read them). The new id is
`<PREFIX><N>` where `N` is **one past the highest suffix ever added under that prefix in the log**
— scanning every `ElementAdded`, including ids since removed but not yet compacted (so a removed
id is never re-minted while leftover events still reference it). Ids are never renumbered, and
there is **no counter state outside the log**. `Ctx::append_add` reads the log *and* writes under
the `appends` lock (H4), so two concurrent adds can never collide. Missing/empty `type` or blank
`label` → `400`; append failure →
`500`. Covered by `mint_id_picks_next_free_suffix_per_lane` and
`append_add_mints_persists_and_replays`. Verified live: against a genesis of `sample.model.json`
(highest `E3`, `C2`), an `event` add minted `E4` and a `command` add minted `C3`.

Client wiring (`src/template.html`) — **done.** The modal's `add` kind is now structural:
selecting it reveals a lane (`type`) select and retitles the button to *Add element*; the
textarea becomes the new element's label. On save it posts `{kind:"add", type, text, col}`
with `col = source col + 1` (the new element lands just after the one the modal was opened
from). The server mints the id, the model-version bumps, and the existing reload/diff path
shows the new sticky as *added*. Offline, the add stashes to `localStorage` like any other
feedback (no mint until back online). `/comments` omits `ElementAdded`, so a structural add
never shows up as a sidebar comment. Verified live: an `add` of a `readmodel` minted `R2`.

### H3 — event schema versioning / migration over time → **done.**

The schema is allowed to evolve, and an old log must still replay. The contract, now codified in
`events.rs`'s module docs and enforced by the read path:

- **Additive change is free (forward compatibility).** A new *optional field* is simply not read by
  older code; a wholly new *event kind* is skipped on read (`parse_log`). Neither breaks an old or a
  new log, so additive change is the preferred way to extend. (`unknown_event_kinds_are_skipped…`
  and `unknown_fields_on_a_known_event_are_ignored` pin both halves.)
- **Renames are migrated forward at one seam (backward compatibility).** Renaming an event kind or a
  field is the *only* backward-incompatible change, and `events::upcast` is the single place a legacy
  spelling is rewritten to today's shape before `parse_event` reads a field — so the rest of the
  pipeline only ever sees the current schema. Detection is **by shape** (the old spelling's presence),
  not a stored version counter, so an old log replays with no marker to set and the log format is
  unchanged (zero churn to existing logs, `compact`, or the verified counts above). The seam is
  seeded with the project's own history: the annotation event used to be a first-class "comment", so
  a log/tool still emitting `CommentAdded` / `Comment` is read as `ElementAnnotated`
  (`legacy_comment_kind_upcasts_to_element_annotated`).
- **A kind's meaning is never silently repurposed.** If semantics must change, add a *new* kind
  (additive) and upcast the old one; never redefine an existing kind in place.

This subsumes the earlier "partial answer to H3" (unknown kinds skipped): forward *and* backward
compatibility now have a defined rule and a test.

### H5 — fold the existing `comments.jsonl` into the log → **done.**

`faceto genesis` now completes the migration story. Alongside the model's genesis batch it folds
a *sibling* `comments.jsonl` (the legacy feedback inbox) into events, appended **after** the
batch — so the ids the comments reference are already minted when those events replay, and the
inbox lands on the board instead of being stranded. The mapping is the same one the live server
uses: `events::comment_to_events` (one JSON comment → its event(s)) is now the single source of
truth, shared by `POST /comment` in log mode and `events::from_comments` (the inbox folder).
`comment`/`question`/`split` → `ElementAnnotated`, `resolve` → `HotspotResolved`, `rename` →
`ElementRenamed`, `move` (+ optional `swapId`/`swapCol`) → one or two `ElementMoved`, `drop` →
`ElementRemoved`. The inbox was always a best-effort sidecar, so `from_comments` **skips** a blank,
unparseable, or `elemId`-less line rather than aborting the migration (the log proper still treats
malformed JSON as a hard error). Missing `comments.jsonl` is the common no-op case. Covered by
`from_comments_folds_a_legacy_inbox_onto_the_genesis_batch` and
`from_comments_skips_blank_malformed_and_element_less_lines`. Verified live: a
`genesis sample.model.json` next to a 4-line inbox seeded `24 genesis + 2 folded` (the garbage and
orphan lines dropped), with `ElementAnnotated`/`ElementRenamed` for `E1` appended after the batch.

### `faceto compact` — fold the log to a snapshot → **done.**

`faceto compact [LOG]` (default `event-log.jsonl`) replays the log, then rewrites it as a
`LogCompacted {folded}` provenance marker followed by `from_model(projection)` — the genesis
batch of the current board. This bounds replay length (H1's snapshot escape hatch). The fold is
**projection-preserving and lossy only in history**: renames/moves collapse into the element's
final `ElementAdded`, the latest annotation and any resolution survive as `detail`, but the
per-comment *timeline* is dropped. `LogCompacted` replays as a no-op; compacting again is a fixed
point (only the marker's count changes). The prior log is copied to `<log>.bak` before the truth
file is overwritten in place (and it's git-tracked, so deeper history is recoverable too). Domain
logic is `events::compact`; `main.rs` only does the IO. Covered by
`compact_preserves_the_projection_and_folds_history` and
`compacting_twice_leaves_the_snapshot_stable`. Verified live: a 29-event session log with a
rename, move, comment, resolution, and add folded to 27 with the projection (12 elements)
byte-for-byte intact and the rename/move gone from the log.

### Client `replayMoves` / offline fallback audit → **done.**

Audited `src/template.html` against the event semantics, three findings:

- **`replayMoves` is redundant in log mode but safe.** `/comments` (`comments_from_log`) returns
  feedback only — no `ElementMoved` — so the move loop iterates nothing and the server's
  re-rendered board is authoritative; it still drives legacy `comments.jsonl` mode. Moves carry an
  *absolute* target col (`colOf = c.col`), so re-applying a duplicate converges — idempotent, no
  drift. Kept, with a clarifying comment.
- **Offline `add` was mishandled (fixed).** A stashed `add` has no `elemId`, but `paint()` only
  excluded `move` from feedback, so it fell into `byEl[undefined]` and inflated the comment count.
  Now `move` *and* `add` are excluded and `byEl` is guarded against a missing `elemId`. Verified in
  a browser: an offline `add` no longer counts and creates no `undefined` bucket; a move still
  replays (col 2→4 → `translate(420,0)`); a real comment still rings its sticky.
- **Offline structural ops are one-way (documented, not "fixed").** The localStorage stash is
  best-effort and is *not* replayed to the server on reconnect (the next live `load()` replaces
  `comments`), so an `add`/`move` made offline never reaches the log. Building a resync queue is out
  of scope; instead the offline branch now tells the user plainly ("offline — add saved locally
  (not on the board yet; Export to keep it)") and the limitation is noted at `postComment`.

### Reconcile the docs → **done.**

- **`source-of-truth.md`** kept as a trimmed historical note: its principle + the
  lossless-reconcile *trap* stay (still worth reading), but the obsolete `faceto reconcile`
  action plan is replaced by a "How the event-sourcing inversion subsumes this" section —
  comments are events now, so nothing is ever stranded; `reconcile` became `genesis` + `compact`.
  It links forward to this note; this note links back.
- **`CLAUDE.md`** brought current: six source files with `events.rs` as the spine, the
  `event-log.jsonl → replay → Model → SVG → HTML` pipeline, the `genesis`/`compact` verbs, log-mode
  `POST /comment` semantics + server-side id minting, and a new "Event-sourced spine (do not break
  these)" invariants block (append-only truth, pure deterministic `replay`, server-minted ids).
- **`CHANGELOG.md`** carries the event-sourcing additions under Unreleased.

The branch is now doc-complete and ready to prep for merge.

## Notes / caveats

- `src/render.rs` and `src/template.html` are now part of this branch's work (the serif nameplate
  and the "add element" affordance); the earlier "left untouched" caveat no longer applies.
- `.gitignore` is already correct: `event-log.jsonl` is **tracked** (it is the new truth);
  `board.svg` / `index.html` / `comments.jsonl` stay ignored (derived).
