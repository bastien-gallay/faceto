# Invariants

Three rules the whole tool leans on. Most subtle bugs, in the code or in a hand-edited model, come
from breaking one of them.

## `id` is the stable identity

An element's `id` is the join key for the comment sidecar **and** for every diff. Identity is
never derived from the label or the position — rename a box, move it across the board, and it is
still the same element.

The file convention follows: **never renumber an id, only add.** Renumbering silently reassigns
every comment and makes yesterday's diff nonsense. This is also why a rename to a blank label is
refused: an id is forever, so a box blanked by accident would stay blank forever.

## `col` is a global timeline coordinate

`col` is shared across all lanes: left to right is time. Elements with the same `col` sit in the
same vertical slice of the story, whatever their lane. It is **not** a per-lane index.

Order within a lane is nothing more than sort-by-`col`. A missing `col` is auto-assigned in file
order — and that assignment is part of the rule, not a drawing detail: every reader of a position
resolves it the same way. The board places a `col`-less sticky by it, and
[`extract --region`](../reference/cli/extract.md) selects by it, so the region you see a sticky in
is the region a cut takes it from. An extract writes the resolved value out, so the smaller board
cannot re-derive a different one.

## `type` selects the lane and the colour

From the fixed eight-type grammar — `actor`, `command`, `aggregate`, `event`, `policy`,
`readmodel`, `external`, `hotspot`. The lane list in the renderer and this set are the same set;
adding a type means adding a lane, a colour and an id prefix together, never one of the three
alone.

See [lanes and the colour grammar](../board/lanes.md) for what each type means.

## Two more, for regions and the log

**Regions partition the timeline.** Phases are a contiguous, gap-free, overlap-free partition
defined by shared frontiers — not independent spans. One normalising sweep projects any phase
list, including legacy spans with holes or overlaps, onto that partition, and it runs on every
path that builds a board. Holes and overlaps are therefore unrepresentable rather than merely
discouraged.

**The log is append-only truth.** Replay is a pure function of the event list; ids are minted
server-side; nothing rewrites a line that has been written. The consequences are spelled out in
[the event log](../reference/event-log.md).
