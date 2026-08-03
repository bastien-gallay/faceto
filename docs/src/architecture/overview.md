# Pipeline and module map

```text
event-log.jsonl → replay → Model → Scene → SVG → HTML
        model.json ↗ (bootstrap / read-only source)
```

Eight modules, one stage each: `json` (a hand-written parser — no serde), `events` (the log,
replay, genesis, compaction), `model` (the typed board and its normalisation), `lint` (a pure
graph pass), `extract` (semantic sub-board selection — a second pure pass over a board, beside
`lint`), `render` (layout and the board's visual language, the board-to-board diff, plus HTML
and the export formats), `scene` (geometric primitives and the one SVG serializer), `serve` (a
`TcpListener` and threads — no web framework). `main.rs` is CLI dispatch only.

`Scene` is the seam between the two halves of drawing. A board's own vocabulary — lanes, columns,
stickies, regions, frontiers — lives in `render` and never crosses it; what crosses is geometry
(`Rect`, `Line`, `Text`, `Circle`, `Path`, and a nesting `Group`), which `scene` serializes without
knowing what a board is. One serializer, written once, for every board format.

Everything is pure Rust standard library at runtime. See
[the design decisions](./decisions.md) for why.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
