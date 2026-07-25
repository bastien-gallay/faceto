# The context pack

Your coding agent does not know your domain. Every session starts with you re-explaining that an
order cannot ship before payment clears, that "reservation" means something specific here, that the
billing context ends where fulfilment begins. You already built that model in a workshop. The
context pack hands it over.

```bash
faceto export orders.event-log.jsonl --format context > docs/domain-context.md
```

## What comes out

A single markdown document, deterministic and pure — same board, same bytes — in six sections:

| Section | Contents |
| --- | --- |
| Header | the board title, its level, and a line telling the reader what this file is |
| Ubiquitous language | every element grouped by lane, with its id and its detail |
| Flows | every edge as `source → target`, with the edge label when it has one |
| Regions | the bounded contexts, each with its column span and its members |
| Open questions | unresolved hotspots and live lint findings |
| Diagram | the Mermaid rendering, embedded |

An excerpt, from `examples/sample.model.json`:

```markdown
## Ubiquitous language

### Commands

- **add an item** `C2`
  - [https://github.com/bastien-gallay/faceto/issues/96](<https://github.com/bastien-gallay/faceto/issues/96>)

### Events

- **DayStarted** `E1` — DayPlan scaffolded

## Flows

- add an item →(emits) ItemAdded
- ItemAdded → when ItemAdded, project forward

## Open questions

- ⬦ **TopicCreated** `H1` — open hotspot
```

Note what survives that a diagram export drops: the lanes (as the grouping of the vocabulary), the
regions and their spans, the ids, the attached links, and the open questions. The Mermaid diagram
at the end carries its own degradation notice inside the fence — honest about being the lossy part
of an otherwise lossless document.

The pack describes exactly the board the SVG draws: same on-grammar filter, same iteration order.
It cannot show an agent a flow you cannot see.

## Wire it into `AGENTS.md`

The convention is one line in the file your agent already reads:

```markdown
## Domain model

The domain model for this service is at [`docs/domain-context.md`](docs/domain-context.md) —
generated from `docs/orders.event-log.jsonl` with `faceto export --format context`.
Treat it as the source of truth for names, flows and boundaries. Its "Open questions"
section lists what is genuinely undecided; ask rather than assume.
```

`AGENTS.md` is read by Claude Code, and by most agent tooling that adopted the convention. If
yours reads something else, the pack is plain markdown — put it wherever that tool looks.

Regenerate it when the board changes. It is derived, so commit it or generate it in CI, whichever
your team prefers — but do not hand-edit it: the board is upstream.

## Why this beats writing the spec in prose

A markdown spec is frozen the moment it is written, and it drifts silently because nothing checks
it. A faceto model is **alive** in ways prose cannot be:

- **event-sourced** — every change to the domain understanding is a recorded event, so the pack
  has a history behind it;
- **diffable** — two versions of the model produce a visual diff, not a paragraph-level one;
- **lintable** — the grammar rules catch an unwired policy before the agent inherits the confusion;
- **reviewable** — an agent can propose a change to the model itself, and you see it as a board
  diff before it counts. See [variants](./variants.md).

The pack is a projection of that living model, not a document you maintain alongside one.

## Checking it worked

The question worth asking after a session: **did the agent stop needing the domain re-explained?**
If it still asks what a reservation is, the pack is either stale or your board is thinner than your
understanding. Both are useful things to learn.
