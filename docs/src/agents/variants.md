# Variants: propose, review, decide

"What if we made shipping asynchronous?" is the question a modelling session turns on, and the one
a whiteboard answers worst — you either wreck the board to try it, or you argue in the abstract.

A variant is the answer: a **second board**, built as an alternative, reviewed as a visual diff
against the original. It is the loop the typed model exists for, and no whiteboard competitor
offers it.

## The loop

**1. Branch the board.** A variant is just another log. Copy the one you have:

```bash
cp orders.event-log.jsonl orders-async.event-log.jsonl
```

Two files, two boards, no shared state. They are sibling boards, so each owns its own outputs —
nothing collides.

On a large board, branch a *part* instead: [`extract`](../reference/cli/extract.md) carves out one
region, one neighbourhood or one lane as a real sub-board, keeping ids and columns so it diffs
against the original exactly like a copy does.

```bash
faceto extract orders.event-log.jsonl --region K2   # → orders-K2.event-log.jsonl
```

**2. Build the alternative.** Serve the copy and edit it, or point an agent at it and let it
propose the change wholesale. Either way the original is untouched, because it is a different
file.

**3. Review it as a diff.**

```bash
faceto render orders-async.event-log.jsonl --base orders.event-log.jsonl
# rendered diff of orders-async vs orders — 2 added, 1 removed, 1 moved, 1 changed
```

The subject is always the positional argument; `--base` is what it is being compared to. You get
`orders-async.svg` / `.html`: the variant's board, with the original drawn into it — added
elements ringed green, removed ones ghosted in place, moved and reworded ones amber. See
[reading the diff overlay](../board/diff.md).

**4. Decide.** Keep the variant's log, or delete it. There is no merge step, because there is
nothing to merge: a board is a file, and choosing is choosing which file you keep. If you want
pieces of both, that is a modelling session, not a merge algorithm — do it on the board.

## Working inside a diff

```bash
faceto serve orders-async.event-log.jsonl --base orders.event-log.jsonl
```

The baseline is loaded once and **pinned for the session**: every edit you make re-renders the
overlay against that same fixed "was". You are building the variant while continuously watching
how far it has departed from the original — which is a different activity from editing a board,
and a surprisingly clarifying one.

The baseline is opened read-only. It is never migrated, never appended to, never touched.

## Why the review surface is read-only

A board rendered against a `--base` baseline disables every structural gesture: no drag, no
rename, no connect dot, no frontier handles. Only focus, the shortcut sheet and region folding
still respond.

That is not a limitation to work around. A diff shows two versions at once, so an edit made on it
would be ambiguous about which board it belonged to — and an ambiguous write to an append-only log
is exactly the thing the whole design refuses. You review here; you edit on the board itself.

## Either side can be anything

`--base` takes a model file or a log, and so does the subject. That buys you comparisons the
"variant" framing does not suggest:

| Compare | Command |
| --- | --- |
| two variants | `faceto render b.event-log.jsonl --base a.event-log.jsonl` |
| the board against where it started | `faceto render orders.event-log.jsonl --base orders.model.json` |
| a colleague's board against yours | `faceto render theirs.model.json --base mine.model.json` |

A wrong `--base` path is the one trap: a file that yields an empty board would read every current
element as "added". It warns rather than failing, so read the tally line.

## The agent's part

Point [the narrate skill](./narrate.md) at a *sibling* log and a variant becomes a **proposal**:
the agent builds the alternative board, you review the diff, and you decide. No new event kind, no
new mode, no approval workflow to build — the composition falls out of two shipped features.

That is the sentence worth keeping: **your agent proposes, you review a diff, you decide.** The
model is typed so the proposal can be seen rather than argued about.
