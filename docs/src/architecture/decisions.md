# Design decisions

The durable ones, in short:

- **Zero runtime dependencies.** The shipped binary is pure `std` — JSON, HTTP, dates and hashing
  are all hand-written — so installing faceto is copying a file and it will still run offline in
  ten years. Test-only dev-dependencies are free; CI enforces the line on the *runtime* tree only.
- **The log is the truth, the model is derived.** Serving never opens a model for writing, so an
  edit can never land somewhere that is later overwritten.
- **Warn, never gate.** The linter always exits 0. An incomplete board is a normal state of a live
  session, not a build failure.
- **Local-first, not local-only.** Collaboration is allowed to exist; it just is not the default.

Longer arguments live in the repository: `docs/event-sourcing-status.md`,
`docs/source-of-truth.md`, `docs/multi-format-architecture.md`. Turning this page into a numbered
ADR index — starting with ADR-1, the `external` → `system` rename — is
[#127](https://github.com/bastien-gallay/faceto/issues/127).
