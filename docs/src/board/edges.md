# Edges: connect and disconnect

An edge is a **directed** connection between two existing elements: this command emits that event,
that event triggers this policy. Edges are what turn a wall of stickies into a flow — and what the
[linter](../reference/lint-rules.md) reads to tell you something is unwired.

## In the model file

Two forms, both accepted, mixable in one array:

```json
"edges": [
  ["X1", "C1"],
  ["C1", "E1"],
  { "src": "E1", "dst": "P1", "label": "triggers" }
]
```

The short form is a pair `[src, dst]`. The object form adds an optional `label`, drawn at the
midpoint so connection kinds stop reading identically.

## On a live board

Select a box: a dot appears on its edge — the live pen. Drag it onto another box to connect the
two. The preview tells you what will happen: **blue** connects, **red** over an already-linked box
disconnects instead. Release outside a box and nothing is posted.

The keyboard does the same thing:

| Key | Effect |
| --- | --- |
| <kbd>e</kbd> | arm "connect from this box" |
| <kbd>e</kbd> on another box | re-arm from that one instead — no need to cancel first |
| focus a target, <kbd>Enter</kbd> | connect — or disconnect, if they are already linked |
| <kbd>Esc</kbd> | cancel |

Connecting appends `EdgeAdded`; disconnecting appends `EdgeRemoved`. Both are ordinary events, so
a wiring session is as recoverable as any other.

## Rules the model enforces

- **Both ends must exist.** An edge naming an unknown id is not a real connection, and no rule
  counts it.
- **No self-loops.** An element cannot connect to itself; the gesture refuses it.
- **Removal cascades.** Deleting an element drops the edges that touched it, so no edge is left
  pointing at nothing.
- **Direction is meaningful.** `["C1", "E1"]` says the command produces the event. The linter reads
  incoming and outgoing edges separately — that is how it can tell you about a policy with an input
  but no output.

## Routing

Edge geometry is computed, not stored. The layout orders elements within a cell to reduce
crossings, and fans out edges sharing an anchor point so parallel connections stay
distinguishable. You do not place edges; you place elements, and the routing follows.

Edges into a [folded region](./regions.md) drop with the elements they pointed at. Edges merely
*crossing* a folded band stay, drawn straight through.

## Not shipped yet

Three edge gestures are designed but not built, and they share one blocker — an edge is a
2.4-pixel line, a poor click target, so all three wait on proper hit-targets:

| Want | Tracked |
| --- | --- |
| click an edge to cut it, from either end | [#88](https://github.com/bastien-gallay/faceto/issues/88) |
| drag an endpoint onto a different box | [#89](https://github.com/bastien-gallay/faceto/issues/89) |
| move where an edge meets a box border | [#58](https://github.com/bastien-gallay/faceto/issues/58) |

Until then, disconnect from the source box with the dot or with <kbd>e</kbd>.
