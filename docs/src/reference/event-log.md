# The event log

`<name>.event-log.jsonl` is the durable record: one JSON object per line, appended, never
rewritten. The board you see is a **projection** replayed from it — which is why there is no save
button and no destructive edit.

What that buys you: every state the board was ever in is reachable; a comment is an event like any
other, not a side file; two clients can never mint the same id, because ids are minted server-side
under a lock; and a log written by a newer faceto still replays in an older one, because unknown
event kinds are skipped and unknown fields ignored.

[`compact`](./cli/compact.md) is the escape hatch when a log grows long: it preserves the board
exactly and drops only the history.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
