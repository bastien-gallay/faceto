# Regions, frontiers, collapse

A region is a named stretch of the timeline — a phase of the story, a stage, a bounded context.
Regions are the tabs across the top of the board.

```json
"phases": [
  { "id": "K1", "label": "ordering",   "fromCol": 0, "toCol": 3 },
  { "id": "K2", "label": "fulfilment", "fromCol": 4, "toCol": 7 }
]
```

## Membership is spatial

There is no membership field on an element. An element belongs to the region whose column span
contains its `col` — move the box, and it belongs to the other region. Move the *frontier*, and
every box it passes over changes hands.

This is deliberate: a bounded context is a region of the timeline, and an element cannot be in two
of them or in none. Nothing can drift out of sync because there is nothing to keep in sync.

## Regions partition the board

Regions are **not** independent spans. They tile the timeline: every column belongs to exactly one
region, with no holes between them and no overlaps.

One normalising sweep enforces it. It is pure, deterministic and idempotent, and it runs on every
path that builds a board — replaying a log *and* reading a model file. Hand-write a `phases` array
with a gap in the middle and overlapping spans on the sides, and what you get back is a clean
partition. Holes and overlaps are unrepresentable, not merely discouraged.

The practical consequence: **resizing a region always re-borders its neighbour**, atomically, in
the same event. There is no state in which a column is orphaned or contested.

## Working with regions

| Gesture | Effect | Event |
| --- | --- | --- |
| click a tab | rename the region | `PhaseRenamed` |
| <kbd>Shift</kbd> + <kbd>←</kbd> <kbd>→</kbd> | move this region's border by one column | `FrontierMoved` |
| drag a frontier | the same, with the mouse | `FrontierMoved` |
| hover a band, click the `+` | carve one region into two | `PhaseSplit` |
| the `+` on a board with no regions | create the first one | `PhaseAdded` |
| <kbd>Delete</kbd> on a tab | merge into the neighbour | `PhaseRemoved` |

A frontier is a **boundary between two regions**, drawn once and grabbable — not two edges that
happen to touch. Dragging it is one gesture with one meaning: this stage ends here, and the next
one begins. A frontier move is clamped so it cannot cross into a third region.

The outermost frontiers resize the **whole board**. Widening the board with *empty* columns is a
separate, heavier gesture that is not shipped —
[#50](https://github.com/bastien-gallay/faceto/issues/50).

Removing a region does not strand its columns: they are absorbed by the neighbour, and the
partition stays gap-free.

## Collapsing: a reading lens, not an edit

Press <kbd>z</kbd> on a region tab, or click the ▸/▾ disclosure, and the region folds to a thin
band. Its stickies hide behind a `▸ Label · N` count chip and every column to its right shifts
left, so a wide board actually becomes **shorter** rather than merely emptier.

Nothing about the model changes. The collapsed set lives in your browser's local storage and is
applied at render time through a view parameter on the board request. Three things follow:

- it is **private** — someone else looking at the same board sees it unfolded;
- it **survives** edits, reloads and diffs, because it is not part of what is being diffed;
- it composes with the [diff overlay](./diff.md): you can fold away the regions you are not
  reviewing.

Edges into a folded band drop along with the elements they pointed at. Edges merely *crossing* the
band stay, drawn straight through. Rerouting a crossing edge to the band's frontiers with a count
badge is the deferred half of the feature — worth knowing about, because a crossing edge you
cannot see is the one way folding can mislead you.

## Regions in a diff

Regions are diffed like elements, on their stable `id`: added, removed, renamed, resized. Renaming
a region never changes which elements are in it — the label is not the identity, and the span is
not the label.
