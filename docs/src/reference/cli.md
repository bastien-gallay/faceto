# CLI

```text
faceto render  [SOURCE] [--base OTHER]              write <name>.svg + <name>.html
faceto serve   [SOURCE] [-p PORT] [--base OTHER]    serve the live board (default :8753)
faceto lint    [SOURCE]                             check the ES grammar (warn-only)
faceto export  [SOURCE] [--format mermaid|context]  print the board to stdout
faceto genesis [MODEL]                              migrate a model.json into its event log
faceto compact [LOG]                                fold a log to a snapshot
faceto help | version
```

## SOURCE

Every verb takes one positional `SOURCE`, defaulting to `./model.json` — except
[`compact`](./cli/compact.md), which operates on a log and defaults to `./model.event-log.jsonl`
(the log a default `genesis` would have produced). A `SOURCE` is either:

- a **model file** — any `.json` that is not a log, e.g. `orders.model.json`; or
- an **event log** — `.jsonl` or `.log`, e.g. `orders.event-log.jsonl`.

The extension chooses the reader. `render`, `lint` and `export` accept both and never mutate
anything. `serve` mutates, so it always resolves to the log first (see
[`serve`](./cli/serve.md)).

## The board name

Output names are derived from the source basename, so sibling boards in one directory don't
collide:

| Source | Board name |
| --- | --- |
| `orders.model.json` | `orders` |
| `orders.event-log.jsonl` | `orders` |
| `foo.json` | `foo` |

A model and *its* log resolve to the **same** name, so `render` of either writes the same
`orders.svg` / `orders.html`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | success — including `lint` findings, which are warnings, never a gate |
| `1` | an operational error: unreadable source, malformed JSON, a refused write |
| `2` | a usage error: unknown command, unknown flag, `--base` or `--format` missing its value |

An unknown flag always fails loudly rather than being misread as a file path.

One flag is laxer than the rest: `serve -p` with a missing or unparseable value **silently keeps
the default port 8753** instead of failing. Check the address the server prints rather than
assuming your `-p` was honoured.

## Warnings

Two warn-only nudges print on stderr and never change the exit code:

- **empty board** — a source that yields zero elements is usually a wrong or mis-suffixed file, so
  it is flagged before the blank board is rendered;
- **legacy log name** — a bare `event-log.jsonl` beside a model, from before logs were named after
  the board, would otherwise have its history silently stranded.

## Per-verb help

`faceto help` prints the usage block above. Per-subcommand `--help` (`faceto render --help`) is
**not implemented yet** — today it is read as a file path. Tracked as
[#16](https://github.com/bastien-gallay/faceto/issues/16).
