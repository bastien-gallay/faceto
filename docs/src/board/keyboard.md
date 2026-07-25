# Keyboard and gesture reference

Every gesture on the live board, in one place. The same sheet is available in the app: press
<kbd>?</kbd>, or click **?** in the header.

The board is **select-then-act**. A single click focuses a box and does nothing else; the keys
below then apply to whatever is focused. Nothing here is modal — there is no tool palette to
switch between.

## Selection

| Gesture | Effect |
| --- | --- |
| <kbd>click</kbd> | focus a box — it now answers the keyboard |
| <kbd>Tab</kbd> | move focus to the next box |
| click empty space | deselect |
| <kbd>?</kbd> | open this sheet |
| <kbd>Esc</kbd> | cancel a drag, a rename, an armed remove, or an armed connect |

## Editing an element

| Gesture | Effect |
| --- | --- |
| <kbd>←</kbd> <kbd>→</kbd> | move one column along the timeline |
| <kbd>drag</kbd> | move along the timeline; drop on an occupied slot to stack above or below it |
| <kbd>F2</kbd> or double-click | rename in place — <kbd>Enter</kbd> commits, <kbd>Esc</kbd> cancels |
| <kbd>a</kbd> or <kbd>Insert</kbd> | add an element after this one, same lane |
| <kbd>Delete</kbd> / <kbd>Backspace</kbd> | remove — press again to confirm, <kbd>Esc</kbd> keeps it |
| <kbd>c</kbd> or <kbd>Enter</kbd> | open the note box for this element |
| <kbd>⌘</kbd>/<kbd>Ctrl</kbd> + <kbd>Z</kbd> | undo your last move or rename |

A rename to a blank label is refused rather than persisted: an id is never renumbered, so a box
blanked by accident would stay blank forever.

## Connecting elements

| Gesture | Effect |
| --- | --- |
| drag the dot on a selected box's edge | pull a wire to another box to connect it |
| drop the wire on an already-linked box | disconnect instead — the preview turns red |
| <kbd>e</kbd> | arm "connect from this box" |
| <kbd>e</kbd> on another box | re-arm from that box instead |
| focus a target, <kbd>Enter</kbd> | complete the connection (again to disconnect) |
| <kbd>Esc</kbd> | cancel |

Edges are directed: the box you start from is the source. A self-loop is refused.

## Regions

A region is a phase of the timeline — a bounded context, a stage. Regions form a **contiguous
partition**: there are no holes and no overlaps, so resizing one re-borders its neighbour.

| Gesture | Effect |
| --- | --- |
| click a region tab | rename it |
| <kbd>Shift</kbd> + <kbd>←</kbd> <kbd>→</kbd> | resize — the neighbour re-borders atomically |
| drag a frontier | the same, with the mouse; the outermost frontiers resize the whole board |
| hover a band | split it in two |
| <kbd>Delete</kbd> on a tab | merge into the neighbour — no columns are stranded |
| <kbd>z</kbd> or the ▸/▾ on the tab | fold the region to a thin band |

Folding is a **reading lens**, private to you: it is stored in your browser, never in the model or
the log. Someone else looking at the same board sees it unfolded.

## Header

| Control | Effect |
| --- | --- |
| **Reload** | re-fetch the board as a diff against what you last looked at |
| **Plain** | drop the overlay, show the clean current board |
| **Export comments** | download the comment sidecar as JSON |
| **?** | this sheet |

## Read-only boards

A board rendered as a variant diff (`render --base` or `serve --base`) is a **review surface**: it
is deliberately not editable. Every structural shortcut is inert there, and only <kbd>?</kbd>,
<kbd>Esc</kbd> and region folding still respond. See [variants](../agents/variants.md).

## Offline

If the server is unreachable, comments and gestures queue in your browser's local storage so the
page stays usable. Queued *structural* edits are local-only — they are not replayed to the log
when the server comes back. Treat an offline session as reading, not authoring.
