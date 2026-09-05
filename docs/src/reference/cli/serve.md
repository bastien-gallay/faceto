# `faceto serve`

Serve the live, editable board over HTTP.

```text
faceto serve [SOURCE] [-p PORT] [--base OTHER]
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `SOURCE` | `./model.json` | the board to serve |
| `-p`, `--port` | `8753` | the port to listen on |
| `--base OTHER` | none | overlay the live board against a fixed baseline, for the whole session |

The server binds `127.0.0.1`. It is a local instrument, not a deployment.

## Serving always goes through the log

`serve` is the only verb that **mutates**, and every mutation must land in the append-only log —
never in the derived model. So the source is resolved to a log before anything is served:

| You pass | What happens |
| --- | --- |
| an event log | served as-is |
| a model with a sibling log | the **log wins** — it is the truth, the model is derived |
| a model with no log | genesis runs once, and the fresh log is served |

```bash
faceto serve orders.model.json
# seeded 14 events from orders.model.json → orders.event-log.jsonl
# serving http://127.0.0.1:8753
```

There is no mode in which `serve` opens a model file for writing. A gesture on the board can
therefore never be recorded somewhere that later gets overwritten.

## Routes

| Route | Purpose |
| --- | --- |
| `GET /` | the board page |
| `GET /board.svg` | the board, re-rendered from the log on every request |
| `GET /board.svg?base=<version>` | the same board as a diff overlay against a cached version |
| `GET /board.svg?collapse=<id,id>` | with those regions folded to bands |
| `GET /model-version` | the current content hash — how the page tells a stale board from a fresh one |
| `GET /comments` | the comment sidecar, merged with the live lint findings |
| `GET /health` | liveness |
| `POST /comment` | append an edit or a note |

`POST /comment` is the single write path. A posted comment is translated into the events it
implies — an added, moved, renamed, annotated, removed element, a resolved hotspot, a connected or
disconnected edge — and appended. New ids are minted **server-side**, so two clients can never
choose the same one. Every append serialises through one lock, so concurrent posts never
interleave.

That endpoint is also how an agent participates: it is the same door your mouse uses. See
[the narrate skill](../../agents/narrate.md).

## `--base`: a live overlay

```bash
faceto serve after.event-log.jsonl --base before.event-log.jsonl
```

The baseline is loaded once, read-only, and pinned for the session: every subsequent edit to the
served board re-renders the overlay against that same "was" side. Useful for working *inside* a
diff — building a variant while watching how it departs from the original.

The baseline is never genesis'd and never mutated. The page is marked read-only, exactly as with
[`render --base`](./render.md).

A baseline of a **different format** from the live board is refused before the port opens — the
same rule as `render --base`, checked up front so the session never starts on a diff it cannot
mean. See [board formats](../board-formats.md#two-formats-never-diff).

## The diff ring

The server keeps the last **12** rendered models in memory, keyed by content hash. `?base=<hash>`
looks the baseline up in that ring. If it has aged out, the plain current board is served instead.
The ring is in-memory only — no git, no persistence. Restarting the server loses it, and that is
by design: the log holds the history, the ring only holds the *view*.
