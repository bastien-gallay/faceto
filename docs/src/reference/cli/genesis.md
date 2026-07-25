# `faceto genesis`

Migrate a model file into its event log — the bootstrap into the event-sourced world.

```text
faceto genesis [MODEL]
```

| Argument | Default | Meaning |
| --- | --- | --- |
| `MODEL` | `./model.json` | the model file to migrate |

```bash
faceto genesis orders.model.json
# seeded 14 events from orders.model.json → orders.event-log.jsonl
```

The log is written beside the model, named after the
[board name](../cli.md#the-board-name). Its first lines are the *genesis batch*: the events that,
replayed, reconstruct exactly the board the model described.

## It refuses to overwrite

If `orders.event-log.jsonl` already exists, `genesis` fails:

```text
error: orders.event-log.jsonl already exists — refusing to overwrite
```

This is not a check that runs before the write — it *is* the write. The file is opened with an
exclusive create, so the append-only guarantee holds even against a concurrent process. There is
no flag to force it. If you truly want to start over, move the old log aside yourself; it is your
history, and deleting it should be your deliberate act.

The model is loaded *before* the write is attempted, so a malformed model reports its own error
rather than the less useful "already exists".

## You usually don't need to run it

[`serve`](./serve.md) runs genesis for you the first time it is given a model with no log. Reach
for the explicit verb when you want the log created without starting a server — in a script, in
CI, or to inspect the migration before serving.

## After genesis

The log is the truth; the model becomes a derived, read-only authoring form. Keep it if you like
authoring JSON by hand (it stays a perfectly good `render` and `lint` source), but be aware that
edits made on the live board land in the **log**, not back in the model. Regenerating a model
*from* a log is [#77](https://github.com/bastien-gallay/faceto/issues/77).
