# The context pack

```bash
faceto export orders.event-log.jsonl --format context > docs/domain-context.md
```

A structured markdown + Mermaid pack of the board — ubiquitous language, flows, regions, open
hotspots — written to be read by a coding agent. Reference it from your repository's `AGENTS.md`
and the agent stops needing the domain re-explained at the start of every session.

The pitch in one line: a markdown spec is frozen prose, while a faceto model is alive —
event-sourced, diffable, lintable, and reviewable as a visual diff before a change counts.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
