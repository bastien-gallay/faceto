# Lanes and the colour grammar

An element's `type` does three things at once: it picks the **lane** the element sits in, the
**colour** it is drawn in, and the **prefix** of the id the server mints for it. There are exactly
eight types, and the set is fixed.

| `type` | Lane | Colour | Means |
| --- | --- | --- | --- |
| `actor` | Actors | Straw `#FCEFA1` | a person or role who acts |
| `command` | Commands | Deep blue `#1A6FAE` | an intention: someone asks the system to do something |
| `aggregate` | Aggregates | Amber `#FFD23F` | the thing that decides whether the command is allowed |
| `event` | Events | Orange `#FF9F1C` | something that happened, in the past tense — the board's spine |
| `policy` | Policies | Lilac `#C39BD3` | a reaction: "whenever X, do Y" |
| `readmodel` | Read models | Green `#6FCF97` | a view someone reads to decide |
| `system` | Systems | Pink `#F2A0C9` | a software system this board does not open up — third-party or your own |
| `hotspot` | Hotspots | Deep red `#C0392B` | an open question, a disagreement, a pain point |

The colours are the classic event-storming sticky palette, held fixed on purpose. Colour is
grammar here, not theming: someone who has run a workshop with paper stickies can read a faceto
board without a legend. Hotspots are also the only squared corners on the board — they read as
different even in greyscale.

## The lane order is the flow

Lanes are drawn top to bottom in the order above. It is the canonical event-storming reading
order: an actor issues a command, an aggregate decides, an event results, a policy reacts, a read
model records, with systems and hotspots below.

> The lane *order* is under review — [#79](https://github.com/bastien-gallay/faceto/issues/79)
> proposes reordering so policies and systems sit nearer the events they touch. The eight-type
> grammar itself is not changing.

## Columns are time, not slots

`col` is a **global timeline coordinate**, shared by every lane. Two elements with `col: 3` are at
the same moment in the story, whatever lanes they are in — that vertical alignment is what makes a
board readable. It is *not* a per-lane index, and it is not a row number.

Within a lane, order is simply sort-by-`col`. An element with no `col` is auto-assigned one in
file order.

Vertical position *inside* a lane is free: elements stack when several share a column, and you can
drag one above or below another. That sub-position is stored, so the board looks tomorrow exactly
as you left it today.

## Ids follow the lane

When you add an element on the live board, the server mints its id with the lane's prefix — `E4`
for an event, `C7` for a command, `X2` for an actor (`A` is taken by aggregates), `G` for a
system. Hand-authored ids are yours to choose; the convention just makes a log readable at a
glance.

Whatever the source, the rule is absolute: **never renumber an id.** It is the join key for
comments and for every diff. Only ever add.
