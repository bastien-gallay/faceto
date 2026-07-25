# `faceto export`

Print the board to stdout in a portable text format.

```text
faceto export [SOURCE] [--format mermaid|context]
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `SOURCE` | `./model.json` | the board to export: a model file or an event log |
| `-f`, `--format` | `mermaid` | the output format |

Reads only, writes only to stdout — so it pipes cleanly.

> **This verb is mid-reformulation.** [#77](https://github.com/bastien-gallay/faceto/issues/77)
> will add a `model` format (log → `model.json`, the inverse of `genesis`) and revisit how the
> comment sidecar is represented. The two formats below are stable; expect the verb to grow.

## `--format context`

A markdown + Mermaid **context pack**: the ubiquitous language, the flows, the regions and the
open hotspots, written for a coding agent to read.

```bash
faceto export orders.event-log.jsonl --format context > docs/domain-context.md
```

This is the intended way to give an agent your domain model without re-explaining it in every
session. See [the context pack](../../agents/context-pack.md) for the recommended
`AGENTS.md` convention.

Lossless as prose — only its embedded diagram degrades, for the reason below.

## `--format mermaid`

A Mermaid diagram of the board.

```bash
faceto export orders.model.json --format mermaid > orders.mmd
```

Useful for pasting into a README, a wiki, or anywhere Mermaid renders. It is **lossy** by nature:
Mermaid has no lanes, no timeline columns, no regions, no comment sidecar. The degradation is
announced twice — as a `%%` comment inside the output, and on stderr — so a piped stdout stays
clean Mermaid while an interactive user still sees the warning.

Use Mermaid to *show* a board elsewhere. Use the log or the model to *keep* it.

## Unknown formats

An unrecognised `--format` value fails with exit code 2 and lists the supported ones, rather than
being misread as a file path.
