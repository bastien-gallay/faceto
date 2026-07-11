# faceto JSON Schemas

Machine-readable schemas for faceto's two file formats. They exist so an LLM (or a human, or
an editor) can generate a **valid file first try** — the founding "a simple typed file you think
through with an LLM" promise — and so the shapes are documented in one authoritative place.

| Schema | Describes | Parsed in code by |
| --- | --- | --- |
| [`model.schema.json`](model.schema.json) | one `*.model.json` — the authoring / bootstrap **source** | `model::from_json` |
| [`event-log-line.schema.json`](event-log-line.schema.json) | **one line** of a `*.event-log.jsonl` — the append-only truth | `events::parse_event` |

Both are [JSON Schema Draft 2020-12](https://json-schema.org/).

## The two formats, in one breath

- **`model.json`** is what you *author*. `faceto genesis` folds it into a founding event log; the
  board you see is a projection *replayed* from that log. Every top-level field is optional — `{}`
  is a valid (empty) board.
- **`*.event-log.jsonl`** is the durable record: one JSON event object per line. Validate each line
  against `event-log-line.schema.json` (it describes a single line, not the whole file).

One shape differs between them on purpose: an **edge** is a positional tuple `["C2", "E2"]` in
`model.json`, but an **object** `{"event":"EdgeAdded","src":"C2","dst":"E2"}` in the log.

## Additive evolution — why the schemas are permissive

faceto's schema evolves **additively**: on read, unknown event kinds are skipped and unknown fields
ignored, so old and new files stay mutually replayable. The schemas mirror that contract —
`additionalProperties` is `true`, so an extra field never fails validation. What the schemas *do*
pin down is the part that actually matters for a valid file:

- **required fields** per shape (an element needs `id` / `type` / `label`; a phase needs
  `label` / `fromCol` / `toCol`; …),
- the **8-lane `type` enum** (`actor` · `command` · `aggregate` · `event` · `policy` ·
  `readmodel` · `external` · `hotspot`),
- ranges and tuple shapes (`y` ∈ `[0, 1]`; an edge tuple is 2–3 strings).

`event-log-line.schema.json` enumerates the **current** event kinds. A future additive kind is
valid at runtime (skipped on read) even before it is listed here; when a new `Event` variant lands
in `src/events/codec.rs`, add its branch to the schema too.

## Validating a file

Any Draft 2020-12 validator works. For example, with Python:

```bash
uvx --with jsonschema python - <<'PY'
import json
from jsonschema import Draft202012Validator

model = Draft202012Validator(json.load(open("docs/schema/model.schema.json")))
model.validate(json.load(open("examples/sample.model.json")))

line = Draft202012Validator(json.load(open("docs/schema/event-log-line.schema.json")))
for ln in open("examples/sample.event-log.jsonl"):
    ln = ln.strip()
    if ln:  # skip blank / trailing lines — one event object per non-empty line
        line.validate(json.loads(ln))
print("valid")
PY
```

The tracked `examples/*.model.json` and `examples/*.event-log.jsonl` all validate against these
schemas — they double as worked, known-good references.
