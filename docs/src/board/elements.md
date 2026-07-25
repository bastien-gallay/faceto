# Elements and direct editing

An element is one sticky on the board: an `id`, a `type`, a `label`, a column, and an optional
`detail`. On a served board every element is directly editable — rename in place, move along the
timeline, stack above or below a neighbour, add a sibling, remove with a confirm — and each of
those gestures appends one line to the event log rather than overwriting anything.

The full gesture list is in [keyboard and gesture reference](./keyboard.md); the field-by-field
shape is in [the model format](../reference/model-format.md).

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
