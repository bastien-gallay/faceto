# Regions, frontiers, collapse

A region is a named phase of the timeline — a stage of the story, or a bounded context. Regions
form a **contiguous partition**: every column belongs to exactly one region, so there are no holes
and no overlaps, and resizing one re-borders its neighbour atomically. Adding a region splits an
existing one; removing a region merges it into its neighbour rather than stranding its columns.

Collapsing a region folds it to a thin band with a count chip, which makes a wide board *shorter*.
That is a private reading lens — stored in your browser, never in the model or the log — so it
never changes what anyone else sees. Edges that merely cross a folded band stay as straight
passthroughs; rerouting them to the band's frontiers is not shipped yet.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
