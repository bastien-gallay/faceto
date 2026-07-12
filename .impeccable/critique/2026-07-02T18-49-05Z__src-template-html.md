---
target: board gestures (src/template.html) — measure gain after v4 P2/P3 fixes
total_score: 36
p0_count: 0
p1_count: 0
timestamp: 2026-07-02T18-49-05Z
slug: src-template-html
---
# Critique v5 — faceto board gestures (src/template.html)

Fifth pass, measuring the gain after the v4 findings were closed (region armed-colour regression,
danger note tone, keyboard region resize). Same method: Assessment A live design review (Chrome
DevTools MCP, CISAC CN2 model) + Assessment B deterministic detector. All three fixes landed
cleanly, no regressions; the surface is now at diminishing returns.

## Design Health Score

| # | Heuristic | v1 | v2 | v3 | v4 | v5 | Key issue now |
|---|-----------|----|----|----|----|----|---------------|
| 1 | Visibility of System Status | 2 | 3 | 3 | 3 | 3 | `#note` never self-clears — stale "region resized"/"kept" lingers in an aria-live region |
| 2 | Match System / Real World | 3 | 3 | 4 | 4 | 4 | Event-storming grammar faithful — ceiling |
| 3 | User Control and Freedom | 1 | 3 | 3 | 3 | 3 | Esc + undo (move/rename) + armed two-step; no undo for remove/add/region-remove (by design) |
| 4 | Consistency and Standards | 3 | 4 | 3 | 3 | 4 | Danger grammar now uniform — armed tab red (not blue), note red (not green), all four paths `rgb(180,35,42)` |
| 5 | Error Prevention | 2 | 3 | 3 | 3 | 4 | Armed confirm + non-blank guards mirrored client/server + resize clamp never offers an invalid span |
| 6 | Recognition Rather Than Recall | 1 | 2 | 2 | 3 | 3 | Help sheet + tooltips + hover glyphs; region *create* has no on-canvas focus affordance |
| 7 | Flexibility and Efficiency | 2 | 2 | 4 | 4 | 4 | Shift+←/→ region resize closes the last keyboard gap — near-complete parity |
| 8 | Aesthetic and Minimalist Design | 4 | 4 | 4 | 4 | 4 | Calm instrument, glass UI — ceiling |
| 9 | Error Recovery | 1 | 3 | 2 | 3 | 3 | Armed note states recovery ("Esc to keep"); a failed POST surfaces only in the note line |
| 10 | Help and Documentation | 0 | 1 | 1 | 3 | 4 | Sheet now discloses region resize + "hover a gap to add" — the last undisclosed gesture closed |
| **Total** | | **19** | **28** | **30** | **33** | **36/40** | **Good/Excellent border — +3, at diminishing returns** |

The +3 maps cleanly to the three fixes: Consistency (danger grammar), Error Prevention + Flexibility
(keyboard resize parity), Help (disclosure). Heuristics 1/3/6/9 were held flat honestly — the
remaining caps are real, not fix-shaped.

## The three fixes — verified

- **Region armed colour (v4 regression) fixed** ✅ — keyboard-armed region tab is red dashed
  `rgb(180,35,42)`, not the blue focus ring; `.arming` declared after `:focus-visible`.
- **Danger note tone (v4 P2)** ✅ — the confirm prompt renders red `#b4232a` while armed and reverts
  to success-green `rgb(27,122,61)` on keep; set by `doRemove`/`armRegion`, cleared on disarm.
- **Keyboard region resize (v4 P3)** ✅ — Shift+ArrowRight grows `data-to-col` one column per press
  and `swapBoard` restores region-tab focus by `data-region`, so it repeats across the re-render
  (verified 11→12→13, focus kept). The sheet discloses resize + mouse-only create.

## Anti-Patterns Verdict

**LLM assessment: "Earned — not slop."** CSS ordered by specificity *with the reason stated*; the
danger grammar applied consistently across four independent code paths (sticky/region ×
keyboard/mouse); pointer-capture leak-guards cite the PR review that pinned them. No dead
affordances, no boilerplate. Construction tells of slop are absent.

**Deterministic scan**: 1 raw finding (`flat-type-hierarchy`, template.html:10) = the documented
false positive, ignore-listed → **0 effective findings**. Console across a full gesture session:
zero JS errors; one favicon 404.

**Visual overlays**: none — the overlay server's port-bind is blocked by the sandbox (as v1–v4);
live browser evidence (computed-style probes + screenshots + console read) is the fallback, and clean.

## Priority Issues

- **[P2] `#note` staleness.** A transient action note ("region resized", "moved", "kept") never
  expires; it sits in an `aria-live="polite"` region indefinitely, so a screen reader may re-announce
  stale state and a sighted user reads a stale claim. **Fix:** expire the note after ~4s (or clear on
  the next focus/gesture); also unify disarm — the region path clears to `""`, the sticky path leaves
  "kept".
- **[P2] Connection-count narrated on every focus (a11y).** `onfocus` fires `note("N connections")`
  into the same `aria-live` channel that carries the danger prompt. A keyboard user tabbing through
  141 boxes hears "5 connections / no connections" on every stop, burying each box's accessible name
  and desensitising the one channel that also warns "about to delete". **Fix:** move the count into
  the box's `aria-description` (read as part of its name), or narrate only on an explicit key rather
  than every focus.
- **[P3] Region create is keyboard-unreachable.** Now disclosed in the sheet, but a keyboard user can
  resize/rename/remove a region and not create one (the rail has no focus stop). **Fix (deferred-ok):**
  a global "new region" affordance or a focusable rail cell.
- **[P3] No undo for destructive ops.** Deliberately deferred — re-asserting a removed id touches the
  Rust event core + the "ids minted server-side" invariant. The armed-confirm mitigates it. Noted,
  not counted as a regression.

## Persona Red Flags

**Alex (power user)**: near-ceiling. Would want multi-select / range-move and undo-for-remove; the
stale note could mislead during rapid successive edits.

**Sam (accessibility / keyboard)**: biggest friction is the connection-count-on-every-focus flooding
the live region; region-create is unreachable. Otherwise strong — `role="button"` + full aria-label
now on both stickies AND region tabs (a prior weak spot, resolved: "region K2, Validation (mwi-submit
pipeline)"), focus survives swap, Esc bails every gesture.

**Modeller mid-thought**: a mis-fired Delete is now legibly recoverable (red, "Esc to keep") — protects
the flow. But a stale "region resized" lingering while they've moved to a sticky breaks the "model is
the subject" spell for a beat.

## Minor Observations

The long CN2 title renders twice (header nameplate + SVG board nameplate) — intentional but redundant
on a narrow header · the long danger note wraps to two lines and crowds the packing segment near
~960px (no clipping at 1440) · the "corner-× under a top-row header" concern is moot here (top lane at
y≈218, glyph z-index 19 > header 5) · disarm messages differ by path ("kept" for a sticky vs `""` for
a region) — small inconsistency.

## Questions to Consider

1. If `#note` is your primary status surface but it goes stale, is it *status* or a *log*? Should
   transient confirmations and persistent state share one aria-live line at all?
2. Region create is the only gesture without keyboard reach. For a "solo modeller pairing with an
   LLM", does the pairing story implicitly assume a keyboard-complete surface the agent can drive?
3. Every focus narrates a connection count into the danger channel. Is that legend helping — or
   teaching the user to tune out the exact aria-live line that also says "about to delete"?

**Bottom line:** the three fixes landed cleanly, no regressions, console clean. At 36/40 this surface
is at diminishing returns — the two P2s (note staleness, focus narration flooding the live region)
are the only findings with real leverage left; everything below is polish.
