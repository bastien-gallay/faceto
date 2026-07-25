# Lint rules

Five rules today. They are a **grammar** check, not a quality score: they encode what an
event-storming board must look like for its arrows to mean anything, and nothing about whether the
domain is well modelled.

Every finding is warn-only. See [`faceto lint`](./cli/lint.md) for how to run them.

> With five rules, they live on one page. If the set grows past a dozen, split it one page per
> rule — the structure the `SUMMARY.md` already anticipates.

## The vocabulary the rules use

A **real edge** connects two distinct, existing elements. Edges pointing at a deleted element, or
an element to itself, are not counted by any rule.

## `event-no-producer`

> no producer: nothing emits this event (no incoming edge)

Fires on an `event` with no incoming edge. Something must cause a domain event — a command, a
policy, an external system. An event nobody emits is either a stub you have not wired up, or a
sign that the producer is missing from the board entirely.

**Legitimately ignorable when** the board is deliberately partial and this event is an entry point
you have chosen not to trace back yet.

## `event-dead-end`

> no outbound edge: a dead end unless this event is terminal

Fires on an `event` with no outgoing edge. Most events cause something: a policy reacts, a read
model updates, an actor is notified. A genuinely terminal event exists — the end of a flow — hence
the hedge in the message.

**Legitimately ignorable when** the event really does end the story.

## `policy-no-input`

> no input: nothing triggers this policy (no incoming edge)

Fires on a `policy` with no incoming edge. A policy is a reaction — "whenever X, do Y". Without an
input, the "whenever" is missing, and the policy cannot fire.

**Rarely ignorable.** A policy with no trigger is usually a real modelling gap.

## `policy-no-output`

> no output: this policy triggers nothing (no outgoing edge)

Fires on a `policy` with no outgoing edge. The "do Y" half is missing: the policy reacts to
something and then, on the board, does nothing.

**Rarely ignorable**, for the same reason.

## `command-no-output` — design level only

> no output: this command emits no event (no outgoing edge)

Fires on a `command` with no outgoing edge, and **only when the board declares
`"level": "design"`**. At big-picture level, a command whose consequences are not yet drawn is
exactly what an early session looks like — so the rule stays silent. Once you declare the board a
design-level artifact, you are asserting the flows are filled in, and a command that emits nothing
becomes a defect.

```json
{ "title": "Orders", "level": "design", "elements": [] }
```

## What is deliberately not linted

Orphans, cycles, over-large bounded contexts — "model smells" rather than grammar — need the
region primitive and a different graph pass. They are tracked separately as
[F-model-smells](https://github.com/bastien-gallay/faceto/blob/main/ROADMAP.md).
