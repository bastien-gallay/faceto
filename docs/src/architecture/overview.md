# Pipeline and module map

```text
event-log.jsonl → replay → Model → SVG → HTML
        model.json ↗ (bootstrap / read-only source)
```

Six modules, one stage each: `json` (a hand-written parser — no serde), `events` (the log, replay,
genesis, compaction), `model` (the typed board, normalisation, diffing), `lint` (a pure graph
pass), `render` (layout, SVG, HTML, the export formats), `serve` (a `TcpListener` and threads — no
web framework). `main.rs` is CLI dispatch only.

Everything is pure Rust standard library at runtime. See
[the design decisions](./decisions.md) for why.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
