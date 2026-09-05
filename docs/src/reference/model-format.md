# The model format

`model.json` is the **authoring and bootstrap** form: the file you or an LLM writes by hand, and a
read-only source for `render`, `lint` and `export`. Once a board is served it is migrated into an
event log, and the log becomes the truth.

A JSON Schema ships with the repository at `docs/schema/model.schema.json`, alongside
`event-log-line.schema.json`. Both are deliberately permissive: the format evolves **additively**,
so unknown fields are ignored rather than rejected, and an older faceto reads a newer file.

## A whole board

```json
{
  "title": "Checkout",
  "format": "event-storming",
  "level": "big-picture",
  "phases": [
    { "id": "K1", "label": "Browse", "fromCol": 0, "toCol": 2 },
    { "id": "K2", "label": "Pay", "fromCol": 3, "toCol": 5 }
  ],
  "elements": [
    { "id": "X1", "type": "actor", "label": "Customer", "col": 0 },
    { "id": "C1", "type": "command", "label": "PlaceOrder", "col": 1 },
    { "id": "E1", "type": "event", "label": "OrderPlaced", "col": 2, "detail": "the pivotal one" },
    { "id": "P1", "type": "policy", "label": "ChargeCard", "col": 3, "y": 0.25 },
    { "id": "H1", "type": "hotspot", "label": "Partial refunds?", "col": 4 }
  ],
  "edges": [
    ["X1", "C1"],
    ["C1", "E1"],
    { "src": "E1", "dst": "P1", "label": "whenever" }
  ]
}
```

Every top-level field is optional. `{}` is a valid — empty — board.

## Top level

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `title` | string | `"board"` | Board title. Drawn on the nameplate and used as the page title. |
| `format` | string | `"event-storming"` | The [board format](./board-formats.md) — which projector reads the file. One value today. |
| `level` | string | `"big-picture"` | Modeling granularity: `"big-picture"` or `"design"`. [Lint](./lint-rules.md) gates a rule on it, and the [context pack](./cli/export.md) states it; the drawn board ignores it. |
| `phases` | array | `[]` | Labelled vertical bands over the column timeline. See [regions](../board/regions.md). |
| `elements` | array | `[]` | The stickies. |
| `edges` | array | `[]` | Directed connections between elements. |

`level` is parsed leniently — anything that is not `"design"` reads as big-picture — but author only
the two documented values. `format` is the exception to that leniency: a value faceto does not
recognise is **refused at load**, because a board it cannot project would otherwise render as an
empty event-storming one. See [board formats](./board-formats.md).

## `elements`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `id` | string | **yes** | Stable identity: the comment join key *and* the diff key. Never derived from text or position. |
| `type` | string | **yes** | One of the eight lanes — `actor`, `command`, `aggregate`, `event`, `policy`, `readmodel`, `external`, `hotspot`. Selects the lane *and* the colour. |
| `label` | string | **yes** | The sticky's headline text. |
| `col` | integer | no | Global timeline coordinate. Omit to auto-assign in file order. |
| `detail` | string | no | A smaller second line under the label. With no `detail`, a trailing `(parenthetical)` in the label becomes one — an explicit `detail` wins. |
| `y` | number `[0,1]` | no | Vertical sub-position *within* the lane band. Omit for auto-stacking. |
| `resolved` | boolean | `false` | For a `hotspot`: quiet grey + check instead of loud red. |
| `links` | array of strings | `[]` | Reference URLs — tickets, docs, ADRs. Clickable chips in the modal; never painted on the board. |

Three rules govern these fields and nothing overrides them:

- **`id` is identity.** The convention for a hand-edited file is *never renumber, only add* —
  renumbering silently reassigns every comment and every diff verdict attached to that sticky.
- **`col` is a global timeline coordinate**, shared across all lanes (left→right = time), *not* a
  per-lane index. Order within a lane is just sort-by-`col`.
- **`type` selects the lane and the colour** from the fixed eight-lane grammar. An off-grammar value
  does not crash the renderer, but it has no lane, so it is not drawn.

`y` is an ordering key more than a coordinate: it is clamped into `[0, 1]` on read, and an absent
`y` means exactly the same thing as the neutral `0.5`.

## `phases`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `id` | string | no | Stable region identity. Omit to have a deterministic `K<n>` minted on load. |
| `label` | string | **yes** | The band's tab label. |
| `fromCol` | integer | **yes** | Left column, inclusive. |
| `toCol` | integer | **yes** | Right column, inclusive. |

Membership is **spatial**: an element belongs to the band whose span contains its `col`. There is no
membership field, and nothing to keep in sync.

Authored bands are **normalised on load** into a gap-free, overlap-free partition, so a file with
holes or overlaps still yields a legal board — with adjusted spans. Two bands claiming the same
columns is not an error you get told about; it is a shape that resolves. Write bands that already
partition the timeline if you want the file and the board to agree exactly.

## `edges`

An edge is directed — `src` is the cause — and takes either form:

```json
["E1", "P1"]
{ "src": "E1", "dst": "P1", "label": "whenever" }
```

The tuple form is two ids **and nothing else**: further slots are ignored. They used to seed the
internal diff channel, which let an authored file paint an overlay wire onto an ordinary board;
comparing two boards is a render concern and has no representation in an authored file.

Only the object form carries a `label`, drawn at the edge midpoint.

## What the parser drops

Parsing is lenient by design — a malformed part is dropped, never fatal, so a large hand-edited
board is never rejected wholesale for one bad entry. Nothing warns you, so it is worth knowing what
vanishes:

| Input | Result |
| --- | --- |
| an element missing `id`, `type` or `label` | the element is dropped |
| a phase missing `label`, `fromCol` or `toCol` | the band is dropped |
| a tuple edge with fewer than two string ids | the edge is dropped |
| a `links` that is not an array, or a non-string entry | that value is dropped, the element stays |
| an unknown top-level or per-item field | ignored |

One warning does exist: `render` prints a nudge on stderr when a source yields **zero** elements
(and does so for a `--base` baseline too), which catches the common mis-suffixed-file mistake.

## How this file relates to the log

```text
model.json ──genesis──▶ <name>.event-log.jsonl ──replay──▶ Model ──▶ SVG / HTML
```

- [`faceto render`](./cli/render.md), [`lint`](./cli/lint.md) and [`export`](./cli/export.md) read
  `model.json` **purely** — they never write to it and never create a log.
- [`faceto serve`](./cli/serve.md) resolves a model to its sibling `<name>.event-log.jsonl`, running
  [`genesis`](./cli/genesis.md) first if none exists. From then on the **log is the truth** and the
  model file is a stale snapshot of when you started.
- [`faceto extract`](./cli/extract.md) prefers the sibling log when one exists — it cuts from what is
  true — and writes the sub-board as a **new** sibling log, never back into the model.
- There is no log → `model.json` writer today. `export` emits Mermaid and the context pack; a
  `model` format is [#77](https://github.com/bastien-gallay/faceto/issues/77).

So: hand-author a `model.json` to bootstrap a board, then let the log carry it. Editing the model
file after serving has started changes nothing anyone will see.

## Evolving the format

Fields are added, never renamed or repurposed. An older faceto ignores a field it does not know; a
newer one falls back to a default when a field is absent — which is why every field above except the
three required ones has a documented default. The same rule, one level down, governs
[the event log](./event-log.md).
