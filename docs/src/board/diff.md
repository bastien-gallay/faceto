# Reading the diff overlay

The board can draw itself as a **comparison** instead of a snapshot: not a list of changes beside
the picture, but the picture itself, annotated. It is the same layout in the same place, so your
eye compares positions rather than reading a changelog.

## The vocabulary

Elements are joined on their stable `id` — never on the label, never on the position — and each is
given one verdict:

| Verdict | Drawn as | Means |
| --- | --- | --- |
| **added** | green `#27ae60`, dashed ring, badge `+` | present now, absent in the baseline |
| **removed** | ghosted to 40% opacity, badge `–` | present in the baseline, gone now |
| **changed** | amber `#E59500`, badge `≠` | same element, different label |
| **moved** | amber, badge `→` | same element, different column, type, or place in its lane |
| **unchanged** | drawn normally | neither side touched it |

Removed elements stay **on the board**, ghosted, in the position they used to hold. That is the
point: a diff that erased them would tell you nothing about what the flow used to look like.

Regions are diffed too, on their own ids: added, removed, renamed, resized.

## Layout follows the new side

When two versions disagree about where something goes, the *current* board wins the layout, and
the baseline is drawn into it. A diff is a picture of now, annotated with what was — not an
attempt to show two boards at once.

## What counts as "moved"

A move is a change of column, of type (which means a change of lane), or of vertical position
within the lane. One subtlety is handled for you: an element with no stored vertical position and
one sitting at the neutral middle are treated as the **same** state, so undoing a placement does
not leave a phantom "moved" badge behind.

## Two ways in

### Since you last looked

On a live board, **Reload** re-fetches it as a diff against the version you were looking at, and
**Plain** drops back to the clean current board. This is the everyday loop: make a few edits, hit
Reload, see exactly what you changed.

The server keeps the last **12** rendered boards in memory, keyed by content hash. If the version
you held has aged out of that ring — or the server restarted — you get the plain board instead of
a diff. Nothing is lost: the history is in the log, the ring only holds the *view*.

### Between two files

```bash
faceto render after.event-log.jsonl --base before.event-log.jsonl
faceto serve  after.event-log.jsonl --base before.event-log.jsonl
```

The positional argument is always "now"; `--base` is "was". Either side can be a model file or a
log, so you can diff two variants, two days, or a model against the log it seeded.

`serve --base` pins the baseline for the whole session, so every edit you make re-renders against
that same fixed "was" — you are working *inside* the diff. See
[variants](../agents/variants.md).

## A diff is read-only

A board rendered against a `--base` baseline is a **review surface**: every structural editing
affordance is disabled on it — no dragging, no rename, no connect dot, no frontier handles. Only
focus, the shortcut sheet and region folding still respond.

This is deliberate. A diff shows two versions at once, so an edit made on it would be ambiguous
about which one it belonged to. You review here and edit there.

## Folding composes with it

[Collapsing a region](./regions.md) is a private reading lens applied at render time, so it works
on a diff as well: fold away the parts of the board you are not reviewing and the changed columns
sit closer together. What is folded is never part of what is compared.
