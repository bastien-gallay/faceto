# Comments, hotspots and findings

Three kinds of remark hang off a board, and all three join to it on the stable element `id`. They
share one sidebar, because in a review you want them together: what a person said, what the room
could not answer, and what the tool noticed.

## Comments

Press <kbd>c</kbd> on an element and write. The note is appended to the log as
`ElementAnnotated` — it is an **event**, not a side file, and it carries the same guarantees as
every other event: ordered, durable, never rewritten.

This is why identity matters so much. A comment points at `E4`; rename `E4`, move it three columns
right, put it in another region, and the comment still points at it. Renumber `E4` by hand and the
comment is now about something else. Never renumber an id.

## Hotspots

A hotspot is an **element**, not an annotation: `"type": "hotspot"`, its own lane, deep red, the
only squared corners on the board. Open questions, disagreements, "nobody in the room knew" — the
things a workshop actually produces.

Making them first-class has a consequence worth stating: they are positioned in time like anything
else. A hotspot sits at the column where the uncertainty bites, so you can see *when* in the flow
you stop knowing what happens.

Mark one settled and it appends `HotspotResolved`. A resolved hotspot stays on the board, drawn
neutral — the question is answered, not deleted.

> The ES-canonical placement is a floating annotation attached to its element rather than a lane of
> its own. That reshape is [F-floating-hotspots](https://github.com/bastien-gallay/faceto/issues/59)
> and will change where hotspots are drawn, not what they mean.

## Lint findings

The [grammar rules](../reference/lint-rules.md) surface in the same sidebar as entries of kind
`lint`. Two properties are load-bearing:

- **computed on read, never stored.** Findings are recomputed from the current board every time
  the sidebar is fetched, so a finding can never be stale — fix the flow and it disappears on the
  next read, with nothing to invalidate.
- **suppressed once resolved.** Marking the element resolved silences its findings, which is how
  you say "yes, I know, and it is intentional" without turning the rule off for the whole board.

They are warnings sitting beside human notes, in the same review surface, at the same weight. That
is the intended reading: the linter is another voice in the review, not a gate.

## The sidebar degrades rather than fails

If the log is malformed or half-written, the sidebar comes back with the stored comments alone
instead of failing the request. You would rather see part of the review than a blank panel.

## Known gaps

Tracked as [#21](https://github.com/bastien-gallay/faceto/issues/21):

- **deleting an element orphans its comments** — they stay in the log, pointing at an element the
  projection no longer shows;
- **resolving a comment still needs a gesture** — there is no button for it yet;
- **two representations of a comment** coexist (the sidebar's exported array, and the log) and
  should collapse toward the log.

None of them loses data. All three are cases where the record is ahead of the interface.
