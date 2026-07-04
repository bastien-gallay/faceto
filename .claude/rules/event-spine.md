---
paths:
  - "src/events/**"
  - "src/serve/**"
---

<!-- markdownlint-disable MD013 -->

# Event-sourced spine (do not break these)

Loaded when editing the log/replay/append code (`src/events/`, `src/serve/`). Full
rationale and locked decisions: [`docs/event-sourcing-status.md`](../../docs/event-sourcing-status.md).

- **The log is append-only truth.** Append events; never rewrite history in place. The one
  exception is `faceto compact`, which folds the log to an equivalent shorter snapshot (and backs
  up the prior log to `<log>.bak`). `event-log.jsonl` is **tracked** in git; `board.svg` /
  `index.html` stay ignored (derived).
- **`replay` is pure and deterministic** — same log → same `Model`. Keep it free of clocks/IO. New
  `Event` variants must extend `parse_event`/`to_json`/`replay` together (the compiler enforces the
  match), and unknown kinds must keep being skipped on read. **Evolve the schema additively**
  (new optional field, or new kind) so old and new logs stay mutually replayable; a renamed *kind*
  is the only backward-incompatible change and belongs in the `upcast` seam (a renamed *field*
  can't be shape-detected, so evolve fields additively; never repurpose a kind's meaning in place).
- **Ids are minted server-side**, never by the client: `<PREFIX><N>`, one past the highest suffix
  ever added under that lane's prefix in the log (removed-but-not-compacted ids stay reserved),
  computed under the appends lock so concurrent adds can't collide (`mint_id` in
  `src/serve/ids.rs`; prefixes from `render::lane_prefix`, the single source of truth alongside
  `LANES`).
