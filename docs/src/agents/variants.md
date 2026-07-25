# Variants: propose, review, decide

The loop the typed model exists for. An agent builds an alternative board — a second log, beside
the first — and you review it as a **visual diff** rather than as a wall of prose:

```bash
faceto render after.event-log.jsonl --base before.event-log.jsonl
faceto serve  after.event-log.jsonl --base before.event-log.jsonl   # live, baseline pinned
```

A diff overlay is deliberately **read-only**: it is a review surface, so every structural edit
affordance is disabled on it. You accept a variant by keeping its log, not by merging anything.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
