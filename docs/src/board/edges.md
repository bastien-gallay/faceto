# Edges: connect and disconnect

Edges are directed connections between two existing elements. They can be authored in the model
file — `["C1", "E1"]` or `{ "src": "C1", "dst": "E1", "label": "emits" }` — or drawn on a live
board by pulling a wire from the dot on a selected box's edge. Dropping the wire on a box that is
already linked disconnects it instead. Self-loops are refused, and removing an element cascades to
the edges that touched it.

Routing is automatic: the layout reduces crossings and fans out edges that share an anchor. Manual
port control ([#58](https://github.com/bastien-gallay/faceto/issues/58)), clicking an edge to cut
it ([#88](https://github.com/bastien-gallay/faceto/issues/88)) and rewiring an endpoint
([#89](https://github.com/bastien-gallay/faceto/issues/89)) are not shipped yet.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
