# `faceto lint`

Check the board against the event-storming grammar.

```text
faceto lint [SOURCE]
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `SOURCE` | `./model.json` | the board to check: a model file or an event log |

Reads only. No `--base`: a grammar check is about one board, not a comparison.

## Warn-only, always

```bash
faceto lint orders.model.json
# 3 grammar findings (warn-only — a big-picture board is legitimately incomplete):
#
#   event "OrderPlaced" [event-no-producer] — no producer: nothing emits this event (no incoming edge)
#   policy "reserve stock" [policy-no-output] — no output: this policy triggers nothing (no outgoing edge)
#   event "StockReserved" [event-dead-end] — no outbound edge: a dead end unless this event is terminal
```

**The exit code is always 0.** An incomplete board is a normal state of a live modelling session,
not a build failure — the tool nudges, it does not gate. If you want a gate in CI, decide the
policy yourself by parsing the output.

A clean board says so:

```text
no grammar findings — 12 elements checked in orders.model.json
```

## Findings are keyed on `id`

Each finding names the stable element `id`, the rule that fired, and a message. The label is
looked up only to make the line readable. That id is also the join key the comment sidecar uses,
which is why the same findings can appear beside human notes on the live board.

## In the browser

When you [`serve`](./serve.md) a board, `GET /comments` merges the live lint findings into the
sidebar as entries of kind `lint`. They are computed **on read**, never stored — so they can never
go stale — and they disappear once the element is marked resolved.

## Board level

The rule set depends on the board's declared `level`:

| Level | Rules |
| --- | --- |
| `big-picture` (default) | the four base rules |
| `design` | the base rules plus `command-no-output` |

Set it at the top level of the model file:

```json
{ "title": "Orders", "level": "design", "elements": [] }
```

See [lint rules](../lint-rules.md) for what each rule means and when to ignore it.
