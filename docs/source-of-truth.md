<!-- markdownlint-disable MD013 -->

# Design note: the model is the single source of truth (comments are a disposable inbox)

> **Superseded (2026-06-17).** This stance was inverted. The current direction is event
> sourcing: the *log* is truth, the model is a projection, comments are first-class events.
> See [`event-sourcing-status.md`](event-sourcing-status.md). Kept for context — it states
> the trap (lossless reconcile, every comment gets a home) that the inversion also honours.

Status: **superseded** by the event-sourcing inversion · Date: 2026-06-16

## The principle

```text
comment (raw inbox)  ──reconcile──▶  model.json   ← the ONLY durable record
   comments.jsonl                    (carries both resolved outcomes AND open state)
```

- **`model.json` is the single source of truth.** Everything durable lives here.
- **`comments.jsonl` is a live inbox**, not an artefact: append-only *and* rewritten in
  place (consuming flips `status → resolved`). It is mutable and merge-hostile — keep it
  **gitignored**, alongside the generated `board.svg` / `index.html`.
- **`board.svg` / `index.html`** are generated output (deterministic from the model) →
  gitignored for a different reason than the inbox: nothing to preserve.

## The trap this avoids

A naïve convention — *"apply the consumed comments into the model, discard the rest"* —
**strands every open comment**: local-only, machine-bound, invisible to a PR or another
device. Data loss waiting to happen.

**Fix:** make reconcile **lossless** by giving every comment a home in the model:

| Comment state | Where it lands on reconcile |
| --- | --- |
| resolved / applied | element edit + `resolution` note (+ `resolved: true` for a hotspot) |
| **still open** | promoted into the model as a **`hotspot`** (open question) or a pending annotation |

After a lossless reconcile, `comments.jsonl` is **safely disposable** — the model holds
everything, open and closed.

> Corollary: if some pending feedback *cannot* be expressed in the model (a half-decided
> `add`, a queued `rename`), that is **a missing state in the schema**, not a reason to
> track the scratch log. Add the state to the model.

## How the event-sourcing inversion subsumes this

The original "actionable part" proposed a `faceto reconcile` CLI that folds `comments.jsonl`
into `model.json` and a schema `pending` state for kinds that didn't map to a hotspot. The
inversion makes that plan obsolete while honouring the trap it guarded against:

- **No comment is ever stranded, by construction.** Comments are now *events*
  (`ElementAnnotated`, `HotspotResolved`, `ElementRenamed`, …) appended to the durable
  `event-log.jsonl`. There is nothing to "reconcile into the model" — the log *is* the record,
  and the model is replayed from it. The lossless-reconcile requirement is satisfied trivially:
  every comment already has a permanent home.
- **`reconcile` → `genesis` + `compact`.** Bootstrapping an existing `model.json` is
  `faceto genesis` (a one-time migration to a genesis batch). Bounding replay length is
  `faceto compact` (fold to a `LogCompacted` snapshot) — the durable analogue of "drain, then
  drop the inbox", except the prior log is preserved (git + `<log>.bak`), not discarded.
- **The "missing schema state" corollary still holds**, restated for events: a half-decided
  `add` or a queued `rename` is just an unappended event, not a reason to keep a scratch inbox.
- **The ignore split is unchanged in spirit, inverted in target.** The `<name>.event-log.jsonl`
  is now *tracked* (it is the truth); the rendered `<name>.svg` / `<name>.html` / `comments.jsonl`
  are derived / transient (gitignored under `examples/`, where this repo renders). Output and log
  names are derived from the model basename so sibling boards in one directory don't collide.

See [`event-sourcing-status.md`](event-sourcing-status.md) for the current design and decisions.
