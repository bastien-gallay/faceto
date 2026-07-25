# Your first board

Five minutes, from an empty directory to a live board you have edited.

## 1. Write a model

A board is a JSON file. The only required piece is `elements`; everything else is optional.

```json
{
  "title": "Orders",
  "elements": [
    { "id": "X1", "type": "actor",     "label": "Customer",    "col": 0 },
    { "id": "C1", "type": "command",   "label": "place order", "col": 0 },
    { "id": "E1", "type": "event",     "label": "OrderPlaced", "col": 1 },
    { "id": "P1", "type": "policy",    "label": "when OrderPlaced, reserve stock", "col": 2 },
    { "id": "R1", "type": "readmodel", "label": "Order status", "col": 3 }
  ],
  "edges": [["X1", "C1"], ["C1", "E1"], ["E1", "P1"], ["P1", "R1"]]
}
```

Save it as `orders.model.json`. Three things are doing the work here:

- **`type`** picks the lane *and* the colour, from a fixed grammar of eight
  ([the lanes](../board/lanes.md)). It is not decoration.
- **`col`** is a position on the **global timeline** shared by every lane — left to right is time,
  not a per-lane index. Two elements with the same `col` happened at the same moment.
- **`id`** is the stable identity. Comments join on it, diffs join on it. Never renumber an id;
  only ever add.

## 2. See it

```bash
faceto render orders.model.json
# rendered 5 elements → orders.svg + orders.html
```

Open `orders.html` in a browser. That file is self-contained — no server, no assets, no network.
You can mail it to someone.

## 3. Make it live

```bash
faceto serve orders.model.json
# seeded 6 events from orders.model.json → orders.event-log.jsonl
# serving http://127.0.0.1:8753
```

Two things just happened. The board is now served live at that address — and your model was
**migrated into an event log**, `orders.event-log.jsonl`, sitting beside it. From here on that log
is the truth: every edit appends to it, and the board is replayed from it on each request. Your
`orders.model.json` stays untouched, a bootstrap and authoring form.

## 4. Edit it

On the live board, click a sticky. It takes focus and answers the keyboard:

- <kbd>←</kbd> / <kbd>→</kbd> moves it along the timeline
- <kbd>F2</kbd> renames it in place
- <kbd>a</kbd> adds a new element after it
- <kbd>c</kbd> opens the note box
- <kbd>?</kbd> shows every gesture — the same list as
  [Keyboard and gesture reference](../board/keyboard.md)

Each of those appends one line to the log. Watch it grow:

```bash
tail -f orders.event-log.jsonl
```

## 5. See what changed

Press **Reload** in the header. The board comes back as a **diff overlay**: what you added is
ringed in green, what you removed is ghosted, what moved or was reworded is amber. Press **Plain**
to drop back to the clean board. Nothing was lost in either direction —
[reading the diff overlay](../board/diff.md).

## 6. Check the grammar

```bash
faceto lint orders.model.json
```

The linter reads the board as an event-storming grammar and reports defects a workshop reviewer
would raise by hand — an event nobody emits, a policy nothing triggers. It is **warn-only** and
always exits 0: a big-picture board is legitimately incomplete
([lint rules](../reference/lint-rules.md)).

## Where to go next

- Hand the board to a coding agent: [the context pack](../agents/context-pack.md)
- Group the timeline into bounded contexts: [regions](../board/regions.md)
- Understand what the log guarantees: [the event log](../reference/event-log.md)
