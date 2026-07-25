# The model format

`model.json` is the **authoring and bootstrap** form: the file you or an LLM writes by hand, and a
read-only source for `render`, `lint` and `export`. Once a board is served it is migrated into an
event log, and the log becomes the truth.

A JSON Schema ships with the repository at `docs/schema/model.schema.json`, alongside
`event-log-line.schema.json`. Both are deliberately permissive: the format evolves **additively**,
so unknown fields are ignored rather than rejected, and an older faceto reads a newer file.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
