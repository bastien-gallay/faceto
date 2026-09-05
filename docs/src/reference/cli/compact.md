# `faceto compact`

Fold an event log down to a snapshot, bounding how long replay has to run.

```text
faceto compact [LOG]
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `LOG` | `./model.event-log.jsonl` | the log to compact |

`compact` operates on an event log only. Pointing it at a model file is an error, not a silent
no-op.

```bash
faceto compact orders.event-log.jsonl
# compacted 412 events → 15 (1 marker + 14 genesis) in orders.event-log.jsonl · prior log saved to orders.event-log.jsonl.bak
```

## What it keeps and what it drops

The result is a `LogCompacted` marker followed by the genesis batch of the **current** board. So:

- **the board is preserved exactly** — replaying the compacted log gives the same projection,
  element for element, edge for edge, region for region;
- **history is dropped** — who moved what, when, and the comment history behind the current state,
  are gone from the log.

That is the whole trade: replay length for memory. Compaction is lossy about the *past*, never
about the *present*.

## It refuses a log it cannot fully read

Compaction rewrites the log from the projection, so anything the read could not project would not
survive the fold. A sticky whose `type` names a lane this build does not know is
[skipped on read](../event-log.md) — harmless when rendering, fatal here: folding would delete it
from the append-only truth, silently and with exit 0.

So `compact` checks first. If any record was skipped it writes the count to stderr, **exits 1, and
leaves the log untouched** — no `.bak`, no rewrite:

```console
$ faceto compact board.event-log.jsonl
error: board.event-log.jsonl refuses to compact — 1 record(s) could not be projected by this
build, and folding would delete them from the log. Compact with a faceto that reads them.
```

The log is not broken; this build is the one that cannot read all of it. A newer faceto that knows
the lane will fold it losslessly. This is the one place the log's forgiving read rules are not
enough on their own — everywhere else, skipping a record costs you a sticky on screen and nothing
in the file.

## The backup is not optional

Before the truth file is overwritten in place, the previous log is copied to `<log>.bak`. If you
compacted too early, that file is your undo. Nothing removes it for you.

## When to reach for it

Rarely. Replay is fast, and a log of a few thousand events costs nothing noticeable. Compaction
earns its keep when a board has accumulated a very long editing history and you no longer care
about it — for instance before publishing a board as a starting point for someone else.

Do not compact a log that clients are currently subscribed to; a live page holding a baseline that
just vanished will fall back to the plain board rather than its overlay.
