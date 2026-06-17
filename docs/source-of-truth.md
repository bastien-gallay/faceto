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

## What this implies for faceto (the actionable part)

1. **A reconcile path.** A `faceto reconcile` step (CLI or a session ritual) that walks
   `comments.jsonl` and folds **every** comment into `model.json` — `resolve`/`rename`/
   `move`/`drop` → durable edits; `question`/unconsumed `comment`/`add` → an open `hotspot`
   or a typed pending annotation. Then truncates/archives the inbox.
2. **A pending representation in the schema.** `hotspot` already covers "open question".
   For the kinds that don't map to a hotspot (`add`, `rename` awaiting a decision), decide:
   either (a) model them as `hotspot`s with a `detail`, or (b) add a small `pending` field /
   element state. Pick one; don't let them live only in the inbox.
3. **Keep the ignore split honest.** `comments.jsonl` ignored = *transient inbox*;
   `board.svg`/`index.html` ignored = *generated output*. Same rule, two reasons — document
   both so nobody "rescues" the inbox into git later.
4. **Optional audit trail.** If the deliberation history has value, snapshot *consumed*
   comments into a curated, tracked log (dated, in `docs/` or the model's sibling notes) —
   never by tracking the live `.jsonl`.

## One-line statement of the convention

> The model is truth; the inbox is a queue. **Reconcile, don't archive** — drain every
> comment into the model (open → hotspot/pending, done → resolution), then drop the inbox.
