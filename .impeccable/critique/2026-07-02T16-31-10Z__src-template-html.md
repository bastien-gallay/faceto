---
target: board gestures (src/template.html) re-critique after harden+polish
total_score: 28
p0_count: 1
p1_count: 2
timestamp: 2026-07-02T16-31-10Z
slug: src-template-html
---
# Critique v2 — faceto board gestures (src/template.html)

Re-run after the harden + polish passes (P0 keyboard channel, drag plumbing, STRUCTURAL_KINDS
rename, resolve semantics, self-diff suppression, Cmd+Z undo, armed remove, aria-live note).
Same method: Assessment A live design review (Chrome DevTools MCP, CISAC CN2 model — 141
stickies, 173 edges, 13 regions) + Assessment B deterministic detector.

## Design Health Score

| # | Heuristic | v1 | v2 | Key issue now |
|---|-----------|----|----|---------------|
| 1 | Visibility of System Status | 2 | 3 | Precise aria-live wording; but the note sits up to 1,400px from the gesture and reflows the header |
| 2 | Match System / Real World | 3 | 3 | Swap-on-collision still not workshop physics (stormers insert-and-shift) |
| 3 | User Control and Freedom | 1 | 3 | Escape cancels drag/rename/arm; Cmd+Z undoes move+rename. add/drop still have no undo; rename commits on blur |
| 4 | Consistency and Standards | 3 | 4 | F2/Escape/Delete/Enter/dialog/arrows all canonical; one editor, four jobs |
| 5 | Error Prevention | 2 | 3 | Guards mirror the server; armed-remove via keyboard strands the red × unpositioned |
| 6 | Recognition Rather Than Recall | 1 | 2 | Keyboard vocabulary still invisible on screen (aria-label only); no help surface |
| 7 | Flexibility and Efficiency | 2 | 2 | Keyboard move is one-shot (focus dies after every edit — new P0); no keyboard add |
| 8 | Aesthetic and Minimalist Design | 4 | 4 | Ghost glyphs, board stays glass — exemplary |
| 9 | Error Recovery | 1 | 3 | Calm exact copy ("kept", "nothing to undo"); leak guards + Escape everywhere |
| 10 | Help and Documentation | 0 | 1 | Still no discoverable help |
| **Total** | | **19** | **28/40** | **Good band — +9, all five fixed areas confirmed live** |

## Anti-Patterns Verdict

**LLM assessment (v2): "Earned. Not slop — decisively."** The two-step armed remove replaced
`window.confirm` in register; Escape semantics layer correctly (drag → arm → rename); the
own-edit/foreign-edit distinction is "a genuinely original piece of interaction thinking",
verified live in both directions. Remaining risk: polish unevenly distributed — mouse path
excellent, keyboard path structurally weaker.

**Deterministic scan**: 1 raw finding (`flat-type-hierarchy`) = the documented false positive,
now ignore-listed in `.impeccable/config.json` → **0 effective findings**. Console: zero
messages across load and a full gesture session.

**Visual overlays**: none — overlay server port-bind blocked by sandbox (as v1); CLI scan is
the fallback signal.

## Fixes confirmed live (v1 → v2)

- Self-diff suppressed: own edit = plain refresh + settle; foreign edit = diff overlay. Both
  paths verified.
- Cmd+Z undoes move (incl. swap) and rename.
- rename no longer pollutes feedback (no ring, no count, no "[rename]" in the export channel).
- resolve no longer increments the open count.
- window.confirm gone; armed remove names the element, Esc keeps.
- Drag plumbing: capture-before-state, lostpointercapture, whenNoDrag force-clear — "the most
  defensively-correct drag code I've reviewed in a dependency-free client" (Assessment A).
- Focus-first keyboard targeting + visible ring... with one new exception (P1 below).

## Priority Issues (v2)

- **[P0] Keyboard move is one-shot — focus dies after every edit.** `refreshPlain` replaces
  `#board` innerHTML; focus lands on `<body>`; the second ArrowRight does nothing. Moving 5
  columns = re-acquiring focus 5 times through a 141-stop tab ring. *Fix:* record
  `document.activeElement.id` before the swap, re-`focus()` after `bindStickies()` (~3 lines;
  also restores the spotlight). Same for rename/remove commits.
- **[P1] No keyboard path to add anything.** `+` glyphs and region rail are hover-only
  `display:none`; region resize is pointer-only. *Fix:* `a`/Insert on the focused sticky →
  same editor at col+1; Enter+arrows on a focused region tab for resize.
- **[P1] Focus ring invisible on the command lane.** `.sticky:focus .card { stroke:#1A6FAE }`
  on a `#1A6FAE` fill — the click's payoff vanishes for one of eight types (has-note ring has
  the same collision). *Fix:* two-tone ring (white casing + blue) or lane-keyed stroke.
- **[P2] Armed remove has no on-element signal; keyboard arming strands the red × unpositioned.**
  *Fix:* position the × from the target rect in `doRemove`; add an `.armed` dashed red stroke
  on the sticky itself.
- **[P2] Regions cannot be removed.** `region-remove` declared but no gesture produces it.
  *Fix:* × glyph on region-tab hover, same armed flow.

## Persona Red Flags

**Alex**: rapid-fire reordering stutters — every move round-trips and kills focus/hover
(press-refocus-press); undo stack not invalidated by foreign board swaps (stale col replay).
**Sam**: real buttons + rich labels + aria-live are solid; but no create/resize path, flat
141-stop tab ring, and after each commit focus resets to top — reads as the page rebooting.
**Modeller mid-thought**: "park a hotspot" is now the most expensive gesture on the board
(hotspot lane below the fold, its + prepends at far-left — scroll down + 10k px left + drag
back 40 columns). The pragmatic fallback (`c` → question) attaches doubt to an element
instead of the timeline.

## Minor Observations

Long labels truncate with no hover reveal (a `<title>` would be free) · rename commits on
blur (defensible, but silent with a stray click) · move-undo says "moved left" where
rename-undo got the better "undid rename of …" copy · glyphs 22×22px (under WCAG 2.2's 24px)
· lane labels ~2.7:1 are also interactive hover targets · `note("")` on disarm timeout can
eat an unrelated fresh message · the header note reflows the control row (reserve width) ·
undo after a foreign edit can replay a stale col.

## Questions to Consider

1. Is swap the right physics for a timeline? Insert-and-shift serves the common act (making
   room); moves are already absolute-col events.
2. Why does the calmest instrument re-render the whole specimen after every touch? The client
   can already translate a sticky (`applyLayout`) — patch own edits in place, reserve the full
   swap for foreign change, and the P0 disappears by construction.
3. If "park a hotspot" is the sacred workshop reflex, does it deserve a global key (`h`) that
   drops a hotspot at the viewport's current column — making faceto faster than a physical
   wall at its own signature move?
