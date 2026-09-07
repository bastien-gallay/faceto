# The 30-second tour

The board below is real. It is `examples/sample.model.json`, rendered by the same `faceto render`
you just installed, embedded here as the self-contained HTML file it produces — no screenshot, no
recording, no server.

<iframe src="../assets/sample.html" title="a faceto board" loading="lazy"
        style="width:100%;height:32rem;border:1px solid #cfcfda;border-radius:6px"></iframe>

Try it: click a sticky and press <kbd>?</kbd> for the shortcut sheet. This page has no server
behind it, so **nothing you do here is recorded anywhere** — an edit changes what you see and stops
there, and the gestures that need the server (folding a region, posting a note) do nothing at all.
A live board is one command away, below.

## What you are looking at

**Eight lanes, top to bottom.** Actors, commands, aggregates, events, policies, read models,
systems, hotspots. The colour is the type; the type is the lane. Nothing is themed —
[the colour grammar](../board/lanes.md) is fixed so a board reads like the paper workshop it came
from.

**Time runs left to right.** Columns are shared across every lane, so a command, the aggregate
that accepts it and the event it produces line up vertically. That alignment is the whole point of
the layout.

**The tabs on top are regions.** Phases of the timeline — stages, bounded contexts. They tile the
board without holes: resizing one re-borders its neighbour. On a served board, <kbd>z</kbd> on a
tab folds a region into a thin band when a wide board stops fitting your screen.

**The red squares are hotspots.** Open questions, disagreements, things nobody could answer in the
room. They are first-class board elements, not annotations to clean up later — a workshop's real
output is often its hotspots.

## What you can't see from here

The three things that make it more than a diagram need the live server:

- **Direct editing** — every gesture appends one line to an event log, so nothing is ever
  overwritten;
- **The diff overlay** — press Reload and the board shows what changed since you last looked;
- **The agent loop** — an LLM reads the log and proposes elements through the same endpoint your
  mouse uses, and you review the proposal as a diff.

```bash
faceto serve examples/sample.model.json
```

Then work through [your first board](./first-board.md).
