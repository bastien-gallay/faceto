---
target: board gestures (src/template.html) on live CISAC model
total_score: 19
p0_count: 1
p1_count: 3
timestamp: 2026-07-02T15-51-03Z
slug: src-template-html
---
# Critique — faceto board gestures (src/template.html)

Target: the F-board-gestures layer, tested live against a real production model (CISAC CN2:
141 elements, 174 edges, 13 regions; 10,270×1,500px board ≈ 7 screens of horizontal scroll),
served in log mode. Assessment A = design-director review with live gesture testing (Chrome
DevTools MCP); Assessment B = deterministic detector + browser evidence.

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | Feedback lives in the header up to 700px from the gesture; transient notes clobbered ~300ms later by the diff banner |
| 2 | Match System / Real World | 3 | ES grammar faithful; swap-on-drag violates sticky-note physics (real stickies shove, they don't teleport) |
| 3 | User Control and Freedom | 1 | No undo anywhere; remove is confirm-then-permanent; every own-edit forces diff mode you must manually "Plain" out of; no Escape-to-cancel mid-drag |
| 4 | Consistency and Standards | 3 | F2/Escape/Enter/dblclick/native dialog all standard; `rename` internally inconsistent (structural op counted as prose feedback) |
| 5 | Error Prevention | 2 | Blank-rename/add guards mirror the server; but region resize silently creates neighbour overlaps (clamp only guards own other edge) |
| 6 | Recognition Rather Than Recall | 1 | 11-item per-sticky gesture vocabulary documented only in the `#board` aria-label; on screen, hover reveals 3 glyphs — the rest is folklore |
| 7 | Flexibility and Efficiency | 2 | hover+arrows fast; add / region-resize / region-add / lane-prepend mouse-only; no multi-select, no jump/zoom on a 7-screen board |
| 8 | Aesthetic and Minimalist Design | 4 | Superb — spotlight at 141 elements stays perfectly calm; glyphs never read as a toolbar |
| 9 | Error Recovery | 1 | A leaked `stickyDrag` silently deadlocks the whole gesture layer AND Reload (`whenNoDrag` self-defer); offline POST failure leaves status pill saying "● live" |
| 10 | Help and Documentation | 0 | No shortcut overlay, no `?` key, no first-run hint |
| **Total** | | **19/40** | **Poor band — but concentrated: presentation is 4/4, behaviour/recovery/help carry the damage** |

## Anti-Patterns Verdict

**LLM assessment**: not slop — a hand-machined, opinionated system (ghost glyphs, native
dialog, honest diff badges, hotspot resolve exhale). Earned familiarity ~80%: the missing 20%
is behavioural (invisible click payoff, swap-glitch feel, no Cmd+Z), exactly the
"pause at a subtly-wrong control" PRODUCT.md forbids.

**Deterministic scan**: 1 finding — `flat-type-hierarchy` (line 10), a **known false positive**
(recorded in project memory: the tight type scale is intentional for this product-register UI).
No contrast, spacing, or anti-pattern hits. Page console clean on load.

**Visual overlays**: no user-visible overlay available — the overlay server's port bind was
blocked by the sandbox (injection preflight itself passed; fallback signal is the CLI scan).

## Overall Impression

The presentation layer is the best of its class — spotlight, diff vocabulary, and a11y skeleton
all survive real production density. The gesture layer's failures are all in the feedback loop:
what a gesture did (self-diff nag, header-distance notes), whether it can be taken back (no
undo), and how you'd ever learn it exists (aria-label-only documentation). Biggest single
opportunity: make the keyboard channel truthful (P0) and self-edits quiet (P1) — the board then
matches its own register.

## Priority Issues

- **[P0] Focus and hover fight over one input channel, and focus loses.** Click a sticky, drift
  the mouse: `onmouseleave` (template.html:814–817) nulls `hoverId` though the sticky is still
  `document.activeElement` — ←/→, F2, c, Delete all silently die. Fix: keyboard target =
  focused sticky first with `hoverId` fallback; don't `hoverOff()` while focused; add a visible
  selected ring so click has a truthful payoff.
- **[P1] No undo, on an event log that makes undo cheap.** Inverse events are one POST each.
  Fix: Ctrl+Z appends the inverse of the last own event; then demote remove's `window.confirm`
  (register-breaking) to undoable-with-note.
- **[P1] `rename` missing from `STRUCTURAL_KINDS` (template.html:168–170).** Renames count as
  comments, paint a permanent has-note ring, and pollute the exported feedback the next LLM
  session reads. Fix: one word in the set; decide `resolve` semantics too (resolving currently
  increments the open count).
- **[P1] Own edits masquerade as foreign change.** Every gesture round-trips into diff mode
  ("showing what changed since you last looked" — you did it yourself). Fix: when the version
  bump matches this client's own POST, skip `showDiff`, advance `shownVersion`, confirm with a
  transient settle pulse on the affected sticky.
- **[P2] Keyboard/AT parity + silent feedback.** Add / lane-prepend / region-add / region-resize
  mouse-only; `#note`/`#status` have no `aria-live`; blank-rename commit drops focus to body;
  159 tab stops with no roving tabindex. Fix bundle: `a` = add-after on focused sticky,
  `aria-live="polite"` on `#note`, refocus after any `endRename`.
- **[P2] Silent total-failure mode in drag plumbing.** `stickyDrag` set before
  `setPointerCapture` (template.html:388 vs 391); a capture throw leaks the flag,
  `gestureBusy()` gates everything forever, and `whenNoDrag` turns Reload into an infinite
  silent retry. Fix: set state after capture succeeds; staleness cap on `whenNoDrag`.

## Persona Red Flags

**Alex (power user)**: P0 focus bug eats keystrokes after click; 10-element reorder = 10
swap-drags that scramble neighbours; no Cmd+Z; no zoom-fit/jump on a 7-screen board; never
learns F2/c/Delete exist.

**Sam (keyboard + screen reader)**: can move/rename/comment/resolve/remove (rare for a board
tool) but can never ADD anything (all add affordances hover-revealed `display:none`), cannot
resize regions, and hears zero feedback (no aria-live); failed rename dumps focus to body in a
159-stop tab sequence.

**The modeller mid-thought (project persona)**: hotspot capture — the method's "when in doubt,
park a hotspot" — is the slowest add (travel to a lane title at the board's far edge);
the self-diff banner interrupts every edit (think, edit, dismiss, repeat); renames leaking into
the feedback export hand the next LLM session a polluted note channel — the product's core
handoff artifact.

## Event-storming lens

Timeline-is-sacred: drag-move is fine for local nudges, insufficient for big reorderings
(swap vs insert-and-shift is a data-model convenience leaking into the hand). Hotspot capture
must be the FASTEST gesture, not the slowest. Stickies-are-disposable: remove-confirm +
no-undo inverts workshop speed. Regions support late emergence well (born/renamed/resized in
UI) except: no `region-remove` gesture exists though the event does, and a fully-regioned
board leaves only one column able to spawn a region.

## Minor Observations

- `+` glyph anchors at the right-face midpoint, camouflaged under highlighted edge anchors at
  connected hubs.
- Glyph targets 22×22px — under WCAG 2.5.8's 24px.
- Single-line rename input truncates 3-line labels — hostile to ubiquitous-language work.
- Server-minted ids (`E1`, `H1`) vs authored `e-EV_*` — visible watermark of UI-born stickies.
- Stale `#note` text lingers after context passes (offline warning shown while live).
- Escape doesn't cancel a drag.
- The packing-buttons regression in project memory did NOT reproduce (Rows→Columns re-rendered
  correctly) — memory may be stale.

## Questions to Consider

1. Should your own edit ever come back to you as a diff? The overlay grammar was built for
   "since you last looked"; the day two authors write concurrently, "what I did" and "what
   changed under me" must look different — today they are pixel-identical.
2. Is swap a data-model convenience leaking into the hand? If the timeline is sacred,
   reordering may need insert-and-shift even though the model makes swap cheaper.
3. If the UI is glass, where is the engraving on how to hold the instrument? Invisible chrome
   AND invisible documentation — is a `?` shortcut overlay a violation of calm, or its missing
   prerequisite?
