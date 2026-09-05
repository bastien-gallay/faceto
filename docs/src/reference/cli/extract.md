# `faceto extract`

Carve a **sub-board** out of a board — by meaning, not by geometry — into a sibling event log.

```text
faceto extract [SOURCE] (--region ID | --focus ID [--hops N] | --type KIND)
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `SOURCE` | `./model.json` | the board to carve from — a model file or an event log |
| `--region ID` | — | every element whose column falls inside that region's band |
| `--focus ID` | — | that element, plus its neighbourhood |
| `--hops N` | `1` | how many edges out from `--focus` to walk |
| `--type KIND` | — | every element in one lane (`event`, `hotspot`, `policy`, …) |

```bash
faceto extract orders.event-log.jsonl --region K2
# extracted region K2 — 7 elements, 5 edges, 1 region → orders-K2.event-log.jsonl

faceto extract orders.event-log.jsonl --focus E4 --hops 2
# extracted E4 + 2 hops — 6 elements, 5 edges, 2 regions → orders-E4-h2.event-log.jsonl

faceto extract orders.event-log.jsonl --type hotspot
# extracted hotspot lane — 2 elements, 0 edges, 1 region → orders-hotspot.event-log.jsonl
```

A whiteboard can only crop a rectangle. `extract` cuts along the board's *grammar*: a bounded
context, a flow around one event, every open question.

## The output is a log

The extract is written beside the source as `<board name>-<selector>.event-log.jsonl` — already
genesis'd, so [`render`](./render.md) and [`serve`](./serve.md) take it directly with no migration
step. The name is derived from the [board name](../cli.md#the-board-name) and a short slug:

| Selector | Output |
| --- | --- |
| `--region K2` | `orders-K2.event-log.jsonl` |
| `--focus E4 --hops 2` | `orders-E4-h2.event-log.jsonl` |
| `--type hotspot` | `orders-hotspot.event-log.jsonl` |

Like [`genesis`](./genesis.md), the write is an exclusive create: if that file already exists,
`extract` fails rather than overwrite it. Move the old one aside yourself.

## It reads the truth, not the bootstrap file

Point `extract` at a `model.json` that already has an event log beside it and it reads the **log**:

```text
orders.event-log.jsonl exists beside orders.model.json — extracting from the log
(it is the truth; the model is derived)
```

Once a log exists, every edit made on the live board landed there and none of them went back to
the model. A `render` from the stale model costs you a re-run; an `extract` would *persist* the
stale board into a new file. Unlike [`serve`](./serve.md), it never creates a log that does not
exist yet — reading is not a reason to write.

## Ids and columns are preserved

Nothing is renumbered and nothing is re-based to column 0. That is what makes the sub-board a
legitimate baseline for a diff against the board it came from:

```bash
faceto extract orders.event-log.jsonl --focus E4 --hops 2
faceto render orders-E4-h2.event-log.jsonl --base orders.event-log.jsonl
# rendered diff of orders-E4-h2 vs orders — 0 added, 7 removed, 0 moved, 0 changed
```

`0 moved, 0 changed` is the proof: every surviving sticky is exactly where it was. Extract →
edit the variant → diff is the full "what if" loop.

## Edges that leave the selection are dropped

An edge with one endpoint outside the cut is not kept — it would point at an element the
sub-board does not contain. The hole this leaves is deliberate and visible:

```bash
faceto lint orders-E4-h2.event-log.jsonl
# policy "when ItemAdded, project forward" [policy-no-output] — no output: this policy triggers nothing
```

That finding is the signal that your cut ran through the middle of a flow. Widen `--hops`, or
accept it: an extract of one context legitimately ends at its borders.

## Regions come along, clipped

The extract keeps every band its survivors' column span **crosses**, trimmed to that span, then
re-projected onto the gap-free partition every board owes. `--region K2` produces a board that
still says "K2".

Crossed is wider than occupied: a `--type hotspot` cut with stickies in the first and last regions
keeps the empty ones in between, because the timeline between two survivors is continuous and
dropping the middle would hand their columns to a neighbour they never belonged to. Only bands
entirely outside the span are dropped.

A model file may leave `col` out, in which case the board assigns one in file order — and `extract`
reads the same assignment, so `--region` cuts exactly the stickies you can see inside the band. The
resolved column is then **written out** explicitly on the extract, so the sub-board cannot
re-derive a different one from its smaller element set.

The board's `title` gains the selector as a suffix (`Orders · region K2`), and its
[`level`](./lint.md) is inherited, so the sub-board lints exactly as strictly as its origin.

## One selector at a time

`--region K2 --type hotspot` is a usage error (exit 2), not an intersection:

```text
extract takes one selector, not two (region K2 and hotspot lane)
(--region / --focus / --type are alternatives, not filters that combine)
```

Combining them would have to define whether `--focus E4 --hops 2 --type hotspot` walks before or
after the lane filter, and a guess there is worse than a refusal. `--hops` is held to the same
rule: without `--focus` it is rejected rather than silently ignored, and given twice it is rejected
rather than silently taking the last one.

Giving no selector at all is a usage error too — there is no default cut:

```text
extract needs a selector: --region ID, --focus ID [--hops N], or --type KIND
```

A selector's value may not begin with `-`, so a missing value cannot swallow the next flag:

```text
--focus needs a value, and --type looks like a flag (values cannot start with '-')
```

`--type` takes one of the eight lanes and nothing else. A misspelled one is refused by name rather
than cutting an empty board:

```text
--type hotspots is not one of the eight lanes: actor, command, aggregate, event, policy,
readmodel, system, hotspot
```

## When the selector matches nothing

A typo must not produce a valid, empty, useless board, so an empty selection is an error (exit 1):

```text
error: no region K9 (this board has K1, K2)
error: no element E9 on this board
error: readmodel lane matched no elements
```

## `--focus` walks both ways

The neighbourhood follows edges in **either** direction. An event's producer is as much its
neighbour as its consumer, so a downstream-only walk would hand you a flow with no cause.
`--hops 0` is legal and yields the element alone. Dangling edges — left by a deleted element —
are never traversed, so they cannot drag a phantom id into the cut.
