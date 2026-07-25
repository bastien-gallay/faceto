# faceto

**A typed file becomes an interactive event-storming board you think through with an LLM.**

You write a small JSON model of a domain — actors, commands, events, policies, read models.
`faceto` turns it into an HTML+SVG board you can read, edit directly in the browser, comment on,
diff against yesterday, lint for grammar defects, and hand to a coding agent as a context pack.

```bash
faceto serve orders.model.json    # → a live board at http://127.0.0.1:8753
```

## What makes it different

**The board is a file you own.** No account, no cloud, no export dance. A model is JSON; the
durable record is an append-only event log beside it. Both are plain text, both go in git, both
are diffable by tools you already have.

**Nothing is ever lost.** Every edit — a move, a rename, a comment, a removed element — is
appended to `<name>.event-log.jsonl`. The board you see is a *projection* replayed from that log.
There is no destructive save, because there is no save.

**Your agent proposes; you decide.** An LLM can read the log, propose new elements through the
same endpoint your mouse uses, or build a whole variant board — and you review the change as a
visual diff before it counts. That loop is why the model is typed in the first place.

**It runs offline, forever.** The shipped binary is pure Rust standard library: no runtime
crates, no network calls, no telemetry. JSON parsing, the HTTP server and the content hashing are
all hand-written. It is under a megabyte, and it will still run in ten years.

## Who it's for

Anyone who runs — or wants to run — an event-storming session and then has to *keep* the result:
domain modellers, architects, tech leads, and developers pairing with a coding agent on a system
they have to explain first. It is deliberately a **single-player instrument** with an agent as the
second participant, not a collaborative whiteboard.

## Where to go next

| You want to… | Read |
| --- | --- |
| Get a board on screen | [Installation](./guide/installation.md) → [Your first board](./guide/first-board.md) |
| Learn the board itself | [The board](./board/lanes.md) |
| Look up a flag or a field | [Reference](./reference/cli.md) |
| Wire it into a coding agent | [Working with agents](./agents/context-pack.md) |
| Understand how it's built | [Architecture](./architecture/overview.md) |

> This documentation covers the shipped behaviour. Where a feature is still being reshaped, the
> page says so rather than describing an interface that is about to move.
