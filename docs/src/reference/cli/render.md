# `faceto render`

Write a static board — an SVG and a self-contained HTML page — beside the source.

```text
faceto render [SOURCE] [--base OTHER]
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `SOURCE` | `./model.json` | the board to render: a model file or an event log |
| `--base OTHER` | none | render a **diff overlay** of `SOURCE` against the `OTHER` baseline |

Reads only. `render` never migrates a model, never creates a log, never mutates anything.

## Output

```bash
faceto render orders.model.json
# rendered 12 elements → orders.svg + orders.html
```

Two files, named after the [board name](../cli.md#the-board-name):

- `orders.svg` — the board as a standalone vector image;
- `orders.html` — the same board wrapped in the interactive page (pan, focus, the shortcut sheet).
  It is fully self-contained: no server, no external assets, no network. Mail it, commit it,
  publish it.

The HTML from `render` is a *reading* surface. Anything that needs the server does nothing here:
region folding and posting a note both go through an HTTP route that is not there, and an edit
gesture changes what you see without being recorded anywhere. To edit a board, use
[`serve`](./serve.md).

## `--base`: a cross-file diff

```bash
faceto render after.model.json --base before.model.json
# rendered diff of after vs before — 3 added, 1 removed, 0 moved, 2 changed → after.svg + after.html
```

The positional is always the subject ("now") and `--base` the baseline ("was") — the same
direction as [`serve --base`](./serve.md). Either side can be a model or a log, so you can diff a
model against a log, or two logs, or two variants of the same board.

Elements are joined on their stable `id` and tagged `added` / `removed` / `changed` (the label
differs) / `moved` (the column, type or in-lane position differs). The output keeps the *source*
stem: the variant is the subject of the review.

A `--base` file that yields an empty board warns rather than failing — otherwise a wrong path
would silently report every current element as "added".

The rendered page is marked **read-only**: a diff is a review artifact, so editing affordances are
disabled on it. See [variants](../../agents/variants.md) and
[reading the diff overlay](../../board/diff.md).
