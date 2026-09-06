# Board formats

A faceto board declares which **format** it is written in — which workshop notation its stickies,
lanes and edges follow, and therefore which projector reads its log.

Today faceto speaks exactly one: **`event-storming`**. The tag exists anyway, and it earns its place
before a second format arrives, for a reason worth stating plainly.

## Why the tag exists before a second format does

Reading a faceto log is deliberately forgiving: a line whose `event` kind it does not recognise is
skipped, so a log written by a newer faceto still replays in an older one.

Point that same rule at a log from a *different notation* and it does something else entirely. Every
line is an unrecognised kind, every line is skipped, and what comes out the other side is a
perfectly valid, completely empty event-storming board — with no error, and exit code 0. Forward
compatibility and format discrimination are one mechanism aimed in opposite directions, and nothing
in an individual line tells them apart.

The tag is what tells them apart.

## Declaring it

In a `model.json`, as a top-level string:

```json
{ "format": "event-storming", "title": "Checkout", "elements": [] }
```

In an event log, as an event — normally the first line:

```json
{"event":"BoardFormat","format":"event-storming"}
```

**Absent means `event-storming`.** That is the same additive default rule
[`level`](./model-format.md#top-level) uses, so every board written before the tag existed reads
exactly as it always did, and neither `genesis` nor `compact` starts writing a tag onto one.

## What faceto refuses

Three reads stop rather than hand you a board that is quietly wrong:

| The source says | faceto |
| --- | --- |
| `"format"` (or a `BoardFormat`) naming something it cannot project | **errors**, naming the format |
| `"format"` present but not a string (`null`, a number, an array) | **errors** — a malformed tag is not an absent one |
| nothing it recognises at all — records present, not one known event kind | **errors**, naming the count |

These are failures, not warnings: the alternative is an empty board and a zero exit status, which is
the shape of bug that survives a whole session before anyone notices.

The last rule is narrower than it looks. It only fires when **no** line is recognised. A log
holding a mix — some events this build knows, some from a newer faceto — keeps the forgiving read,
because it has told the reader something it can actually project.

## Two formats never diff

`render --base` and `serve --base` compare two boards by joining them on `id`. An id names a
different thing in each notation, so a cross-format overlay would judge unrelated stickies `moved`
and report a board's worth of phantom changes. Both commands refuse the pair up front, naming the
two formats, rather than drawing that.

## Adding a format

Not yet possible: formats are a compile-time set, not a plugin surface, and one variant ships today.
The multi-format work is tracked under [F-new-diagrams #126][umbrella]; the tag itself is
[F-format-tag #121][tag]. When a second format lands, this page grows a table rather than a caveat.

[umbrella]: https://github.com/bastien-gallay/faceto/issues/126
[tag]: https://github.com/bastien-gallay/faceto/issues/121
