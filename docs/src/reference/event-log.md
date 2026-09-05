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

A JSON Schema for one line ships at `docs/schema/event-log-line.schema.json`.

## The line grammar

```json
{"event":"ElementAdded","id":"E1","type":"event","label":"OrderPlaced","col":2}
{"event":"EdgeAdded","src":"C1","dst":"E1"}
{"event":"ElementAnnotated","id":"E1","text":"is this the pivotal one?"}
```

File order **is** causal order. Reading applies five rules, and the difference between them is the
difference between a typo and a schema you have not met yet:

| Line | Outcome |
| --- | --- |
| blank or whitespace-only | skipped |
| not valid JSON | **hard error**, naming the line number |
| a known `event` kind missing a required field, or with a mis-typed one | **hard error** |
| an unknown `event` kind | **skipped**, silently |
| a line with no `event` key, or a non-string one | **skipped**, silently — a typo'd key loses the fact |
| a `BoardFormat` naming a format this build cannot project | **hard error** |
| records present, but **not one** of a recognised kind | **hard error** |

The last two rows arrived with the [format tag](./board-formats.md), and they are the one place the
skipping rule is suspended. Skipping unknown kinds is how an older faceto reads a newer log — and it
is also, pointed the other way, how a *different board format's* log reads as an empty
event-storming board. Nothing in a line distinguishes the two. So the count decides: a log carrying
some recognised events keeps the lenient reading, while a log carrying **none** has told the reader
nothing it can project, and says so instead of drawing a blank board.

The third row is the one worth dwelling on. A malformed *known* event is a fact that exists in the
append-only truth but would vanish from the projection, so it stops the read rather than quietly
shrinking your board. An unknown kind, by contrast, is how forward compatibility works: a log
written by a newer faceto still replays here.

The last row is the sharp edge of a hand-edited log: `{"evnt":"ElementAdded",…}` is well-formed
JSON with no recognisable kind, so it takes the same path a future kind does and vanishes without a
diagnostic. Validate against the schema when you edit a log by hand — it enumerates the kinds
strictly, which is exactly the check the runtime cannot make.

## Event kinds

Eighteen kinds. Every one carries its `event` discriminator plus the fields below; anything else on
the line is ignored.

| `event` | Fields | Effect on the board |
| --- | --- | --- |
| `BoardTitled` | `title` | Sets the title. Last one wins. |
| `BoardFormat` | `format` (`event-storming`) | Declares the [board format](./board-formats.md). Absent from a log ⇒ `event-storming`. A value this build cannot project stops the read. |
| `BoardLeveled` | `level` (`big-picture` \| `design`) | Sets the lint granularity. Absent from a log ⇒ `big-picture`. |
| `PhaseAdded` | `label`, `fromCol`, `toCol`, optional `id` | Adds a region. A legacy band with no `id` gets a deterministic `K<n>` minted on replay. |
| `PhaseResized` | `id`, `fromCol`, `toCol` | Sets both borders. The legacy span model — prefer `FrontierMoved`. |
| `PhaseRenamed` | `id`, `label` | Renames a region. |
| `PhaseRemoved` | `id` | Removes a region; its columns are absorbed by a neighbour, never stranded. |
| `FrontierMoved` | `id`, `edge` (`start` \| `end`), `col` | Moves one border. The neighbour re-borders atomically, so the partition can never open a gap. |
| `PhaseSplit` | `id`, `atCol`, `newId`, `newLabel` | Splits a region in two: `id` keeps the left half, `newId` takes the right. No-op unless `atCol` falls strictly inside. |
| `ElementAdded` | `id`, `type`, `label`, optional `col` / `detail` / `y` / `links` | Adds a sticky. An `id` already on the board is ignored, not duplicated. |
| `ElementRenamed` | `id`, `label` | Renames a sticky. |
| `ElementMoved` | `id`, optional `col` / `type` / `y` | Moves and/or re-lanes. An **absent** field leaves that dimension untouched — a col-only nudge never resets `y`. |
| `ElementAnnotated` | `id`, `text` | Sets the element's `detail`. **Replaces** the previous note; the log keeps the history, the board shows the latest. |
| `HotspotResolved` | `id`, `resolution` | Marks the element resolved and stores the resolution as its `detail`. |
| `ElementRemoved` | `id` | Removes the sticky **and cascades** to every edge touching it. The id stays reserved. |
| `EdgeAdded` | `src`, `dst`, optional `label` | Adds a directed edge. A duplicate pair is a no-op. |
| `EdgeRemoved` | `src`, `dst` | Removes that directed pair. |
| `LogCompacted` | `folded` | Provenance marker written by `compact`. A no-op on replay. |

Two things replay never does: it never validates that an edge's endpoints exist (a stray id is
tolerated, and cascade-cleaned when its element goes), and it never invents an element for an event
naming an id the board does not have — such an event is simply inert.

Region events are followed by one normalisation sweep, so **every** replayed board has a gap-free,
overlap-free partition of the timeline, whatever the log did to get there.

## Ids are minted server-side

Never write an id yourself into a live board. `serve` mints `<PREFIX><N>` under the append lock, one
past the highest suffix **ever** used under that prefix — so a removed id is never handed out again,
and two concurrent adds cannot collide.

| Lane | `actor` | `command` | `aggregate` | `event` | `policy` | `readmodel` | `external` | `hotspot` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Prefix | `X` | `C` | `A` | `E` | `P` | `R` | `G` | `H` |

Regions use `K`. Note `actor` stamps `X`, not `A` — `aggregate` owns that letter.

## The write path: `POST /comment`

The live board never appends events directly. It posts a **comment** — one action per request — and
the server translates it into the event(s) it implies. That translation is the contract an agent
writes against; see [the narrate skill](../agents/narrate.md).

| `kind` | Required | Optional | Appends |
| --- | --- | --- | --- |
| `add` | `type` (a lane), `text` (label) | `col`, `prepend`, `detail` | `ElementAdded` — **server mints the id** |
| `move` | `elemId`, and `col` and/or `y` | `swapId` + `swapCol` | `ElementMoved` (two, on a swap) |
| `rename` | `elemId`, `text` | — | `ElementRenamed` |
| `resolve` | `elemId` | `text` (the resolution note) | `HotspotResolved` — an omitted `text` stores an **empty** note, clearing any previous one |
| `drop` | `elemId` | — | `ElementRemoved` |
| `connect` | `src`, `dst` (distinct, non-blank) | — | `EdgeAdded` |
| `disconnect` | `src`, `dst` | — | `EdgeRemoved` |
| `region-add` | `text`, `fromCol` < `toCol` | — | `PhaseAdded` — **server mints the id** |
| `phase-split` | `regionId`, `atCol`, `text` | — | `PhaseSplit` — **server mints the right half's id** |
| `frontier-move` | `regionId`, `edge`, `col` | — | `FrontierMoved` |
| `region-rename` | `regionId`, `text` | — | `PhaseRenamed` |
| `region-remove` | `regionId` | — | `PhaseRemoved` |
| `region-resize` | `regionId`, `fromCol`, `toCol` | — | `PhaseResized` (legacy — prefer `frontier-move`) |
| anything else with an `elemId` | `elemId` | `text` | `ElementAnnotated` — same: an omitted or blank `text` **clears** the element's note |

The guards are deliberate, and each one exists because its absence would write something permanent
and wrong:

- a **blank label** on `add` or `rename` is refused (`400`) — an id is never renumbered, so a box
  blanked by accident would stay blank forever;
- an **off-grammar `type`** on `add` is refused, because it would mint into a real lane's id space;
- a **self-loop**, and an absent or blank `src` / `dst`, are refused on `connect`;
- an **inverted or zero-width span** on `region-add` / `region-resize` is refused;
- a `move` carrying neither `col` nor `y` persists **nothing** — it would replay as a no-op.

Endpoint **existence** is *not* among them: `connect` sees only the posted comment, never the
board, so a typo'd id is accepted and appends a dangling edge. Replay tolerates it — nothing is
drawn, and the edge is cascade-cleaned if its element is later removed — but nothing tells you
either. Post ids you actually read back from the log.

A request that maps to no event is a `400`, not a silent success. The last row of the table is the
catch-all: an unrecognised `kind` naming an element becomes an advisory note, never a structural
edit.

## How the schema evolves

Three rules, and the third is the one people get wrong:

1. **A new optional field is free.** Older code does not read it; newer code defaults it. This is how
   `y`, `links` and edge labels all arrived.
2. **A new event kind is free.** Older code skips it as unknown.
3. **A renamed *kind*** is the one backward-incompatible change, and it is repaired at a single
   read-path seam that rewrites the old name to the current one. Today that seam maps `CommentAdded`
   and `Comment` → `ElementAnnotated`. A renamed *field* cannot be repaired this way — by shape, an
   absent key is indistinguishable from a new optional one — so fields only ever grow.

A kind's meaning is never repurposed in place. If semantics must change, a new kind is added and the
old one upcast.

## Genesis and compaction

[`genesis`](./cli/genesis.md) derives a log from a `model.json`: title, level (only when it is not
the default), then regions, elements and edges, in that order. A resolved hotspot emits its add
followed by its resolution, so the resolution note round-trips.

[`compact`](./cli/compact.md) folds a log to a `LogCompacted` marker plus the genesis batch of the
current board. `replay(compact(log))` gives the same board as `replay(log)` — but the fold is
**lossy by design**: only the projection survives, so the comment *history* goes and each element
keeps just its latest note. The prior log is backed up to `<log>.bak`, and it is tracked in git
besides.

The log is meant to be read by humans and by agents. It is line-oriented, one fact per line, and
`grep` is a perfectly good first tool on it.
