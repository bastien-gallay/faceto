# Elements and direct editing

An element is one sticky on the board.

```json
{
  "id": "E4",
  "type": "event",
  "label": "OrderPlaced",
  "col": 2,
  "detail": "raised once payment is authorised",
  "links": ["https://github.com/acme/shop/issues/812"],
  "resolved": false
}
```

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | the stable identity — the join key for comments and diffs. Never renumber it |
| `type` | yes | one of the eight types; picks the lane and the colour |
| `label` | yes | what is written on the sticky |
| `col` | no | position on the global timeline; auto-assigned in file order if absent |
| `detail` | no | a longer note, shown when you open the element |
| `links` | no | reference URLs — tickets, ADRs, docs — shown as chips when you open it |
| `resolved` | no | marks a hotspot as settled; also suppresses its lint findings |
| `y` | no | vertical sub-position *within* the lane, `0`–`1`; omitted means auto-stacked |

`y` is worth a word. Two elements in the same lane and the same column stack, and you can drag one
above the other; that choice is stored, so the board looks tomorrow exactly as you left it. It is
never identity (`id` is) and never a lane choice (`type` is) — it only places the sticky inside its
band.

## Editing on a live board

Every gesture below appends **one line** to the event log. There is no save, no dirty state, and
nothing is ever overwritten — see [the event log](../reference/event-log.md).

| You do | The log records |
| --- | --- |
| move a box (<kbd>←</kbd> <kbd>→</kbd> or drag) | `ElementMoved` |
| rename it (<kbd>F2</kbd> or double-click) | `ElementRenamed` |
| add one (<kbd>a</kbd> or <kbd>Insert</kbd>) | `ElementAdded` |
| remove it (<kbd>Delete</kbd>, twice) | `ElementRemoved` |
| write a note on it (<kbd>c</kbd>) | `ElementAnnotated` |
| mark a hotspot settled | `HotspotResolved` |

Full gesture list: [keyboard and gesture reference](./keyboard.md).

## Guards you will meet

**A rename to blank is refused.** Nothing is persisted and the label stays as it was. An id is
never renumbered, so a box blanked by accident would stay blank forever — the guard exists because
the mistake would be permanent.

**Removing is two presses.** <kbd>Delete</kbd> arms the removal and the box shows it; a second
press confirms, <kbd>Esc</kbd> keeps it. Removing an element also drops the edges that touched it.

**Ids are minted by the server.** When you add an element the server chooses its id, prefixed by
the lane — `E4`, `C7`, `X2`. Two clients adding at the same moment can never collide, because
neither of them is choosing.

**Undo covers your last move or rename.** <kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>Z</kbd> appends the
*inverse* event rather than deleting the original: the log keeps both, because the log keeps
everything. Text fields keep their native undo while you are typing.

## Removed is not erased

An `ElementRemoved` event hides the element from the current projection. The events that built it
are still in the log, and so is its history. What you lose from the *board* you keep in the
*record* — which is the whole reason the record is append-only.

Its comments, though, are currently orphaned rather than cascaded —
[#21](https://github.com/bastien-gallay/faceto/issues/21).
