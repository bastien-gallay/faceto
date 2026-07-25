# Comments, hotspots and findings

Three kinds of remark share one sidebar, all joined to the board on the stable element `id`:

- **comments** — notes you post on an element; they are events in the log, not a side file;
- **hotspots** — open questions modelled as first-class elements in their own lane, which can be
  marked resolved;
- **lint findings** — the [grammar rules](../reference/lint-rules.md), computed on read so they
  can never go stale, and suppressed once the element is resolved.

Known gaps, tracked as [#21](https://github.com/bastien-gallay/faceto/issues/21): deleting an
element orphans its comments, and resolving one still needs a gesture rather than a hand-edited
file.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
