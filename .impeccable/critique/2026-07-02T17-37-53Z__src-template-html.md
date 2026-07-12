---
target: board gestures (src/template.html) — measure gain after v3 backlog cleared
total_score: 33
p0_count: 0
p1_count: 0
timestamp: 2026-07-02T17-37-53Z
slug: src-template-html
---
# Critique v4 — faceto board gestures (src/template.html)

Fourth pass, measuring the gain after the v3 backlog was cleared (2 P1 + 2 P2 + 1 P3: armed-remove
on-element signal, discoverable help sheet, region-remove gesture, rename-blur=cancel, connection
count). Same method: Assessment A live design review (Chrome DevTools MCP, CISAC CN2 model) +
Assessment B deterministic detector. Four of the five fixes landed flawlessly; the fifth
(region-remove) shipped a cosmetic colour regression — **caught here and fixed immediately after the
run** (commit `33f3dd5`).

## Design Health Score

| # | Heuristic | v1 | v2 | v3 | v4 | Key issue now |
|---|-----------|----|----|----|----|---------------|
| 1 | Visibility of System Status | 2 | 3 | 3 | 3 | Every `#note` state renders in one success-green (#1b7a3d); danger has no text tone |
| 2 | Match System / Real World | 3 | 3 | 4 | 4 | Event-storming grammar faithful — connections, lanes, timeline, hotspots read as the domain |
| 3 | User Control and Freedom | 1 | 3 | 3 | 3 | Escape everywhere + undo (move/rename) + armed-remove + blur=cancel; remove/add/region-remove still irreversible |
| 4 | Consistency and Standards | 3 | 4 | 3 | 3 | Region-armed colour regression (blue vs sticky red) — **fixed post-run**; scored as observed |
| 5 | Error Prevention | 2 | 3 | 3 | 3 | Two-step arm + blur-cancel + non-blank guards + drag clamps — strong |
| 6 | Recognition Rather Than Recall *(ceiling)* | 1 | 2 | 2 | 3 | Help sheet + connection-count + rich aria-labels externalise the vocabulary; region create/resize keep no hint |
| 7 | Flexibility and Efficiency | 2 | 2 | 4 | 4 | Keyboard fast-paths, hover glyphs, drag, packing, undo — excellent |
| 8 | Aesthetic and Minimalist Design | 4 | 4 | 4 | 4 | Calm instrument; glyphs bare at rest; help sheet a quiet mono-keycap grid |
| 9 | Error Recovery *(ceiling)* | 1 | 3 | 2 | 3 | Armed × now on-element (red ring, corner-anchored) + Escape + 3s timeout + blur-cancel; destroys still un-undoable |
| 10 | Help and Documentation *(ceiling)* | 0 | 1 | 1 | 3 | From **nothing** to a discoverable `<dialog>` with two open paths and the full key vocab |
| **Total** | | **19** | **28** | **30** | **33/40** | **Good band — +3; all three ceiling heuristics (#6/#9/#10) finally moved** |

The +3 is honest and concentrated where it was blocked: **#10 Help 1→3 is the single largest gain
in the run's history** (a real surface where there was none), and #6 and #9 — stuck for three runs —
both moved. Four fixes were flawless; the fifth worked functionally but shipped a colour regression.

## The five fixes — verified

- **Armed-remove on-element signal** ✅ — corner-anchored × (562/209 against a box at 565/218, never
  the old 0,1600 stranding), red #b4232a dashed keyline white-cased for lane legibility, declared
  last so it beats the focus/has-note ring. Escape + 3s timeout keep the box.
- **Help sheet** ✅ — right-aligned system-mono keycaps, two-column key→action grid, both the header
  **?** button and the **?** key open it, native Escape/backdrop/× close. Reads like the instrument's
  engraved reference card.
- **Region remove** ✅ functional — focus a tab, Delete arms (red dashed + prompt), Delete again
  commits; verified end-to-end (13→12 tabs, version bump, region gone). ⚠️ shipped with a colour
  regression (below), now fixed.
- **Rename-blur = cancel** ✅ — a half-typed label left by a click is discarded ("rename cancelled"),
  only Enter commits, and the cancel does not steal focus back.
- **Connection count on focus** ✅ — "15 connections" gives the dense edge-fan a legend, correctly
  pluralised ("1 connection" / "no connections"), suppressed on swap-refocus so it never clobbers an
  action confirmation.

## Anti-Patterns Verdict

**LLM assessment: "Earned — this is not slop."** No dumped component library, no SaaS chrome, no
decorative gradients, no filler copy. Each decision carries a register-aware rationale (the red
keyline "cased in white so it survives the command lane"; the connection legend for an "impressive
but unreadable" fan). The armed-confirm/Escape/timeout triad, the `gestureBusy()` single-predicate
mutual exclusion, and the pointer-capture leak guards are the marks of driven edge cases. The one
defect was a specificity/source-order slip, not a generation artifact.

**Deterministic scan**: 1 raw finding (`flat-type-hierarchy`, template.html:10) = the documented
false positive, ignore-listed in `.impeccable/config.json` → **0 effective findings**. Console
across a full gesture session (armed remove, help, region remove, rename blur, focus count): zero JS
errors; one harmless favicon 404.

**Visual overlays**: none — the overlay server's port-bind is blocked by the sandbox (as v1–v3).
Live browser evidence (computed-style probes + screenshots + console read) is the fallback signal.

## Priority Issues

- **[P2] Region armed state showed blue, not red (regression — FIXED post-run).** `.region-tab.arming
  rect` (red dashed) was declared *before* `.region-tab:focus-visible rect` (blue) at equal
  specificity, so on the keyboard path — tab focused AND armed — the blue focus ring won the stroke.
  Fixed by moving the `.arming` rule after `:focus-visible` (commit `33f3dd5`); verified the armed
  region stroke is now `rgb(180,35,42)`.
- **[P2] The destructive-confirm prompt is painted success-green.** `#note` is uniformly `#1b7a3d`,
  so "remove …? — same gesture again to confirm" renders in the identical green as "region removed"
  and "renamed". The one moment the text channel must signal danger, it uses the confirmation
  colour. **Fix:** a `data-tone="danger"` on `#note` (amber/red) set by `doRemove`/`armRegion`,
  cleared on disarm — closes the colour-only-danger gap in one stroke.
- **[P3] Region create and resize are mouse-only.** `.region-rail` (create) and `.region-edge`
  (resize) have no `tabindex`; only rename (Enter/Space) and remove (Delete) are keyboard-reachable
  on a tab. Sam can delete a region but not create or resize one, and the help sheet doesn't disclose
  the asymmetry. **Fix:** keyboard affordances (focus a tab → Shift+←/→ resize; a lane-level add-region
  key) or at minimum note the mouse-only ops in the sheet.
- **[P3] No undo for destructive ops.** Remove/add/region-remove push nothing to `undoStack` (ids
  can't be re-minted); the two-step arm is the only net. **Fix:** the append-only log could carry a
  `re-add` event re-asserting the same id, making destruction reversible and letting the confirm
  gesture get lighter.

## Persona Red Flags

**Alex (power user)**: arrow-move is ±1 column only — no multi-column jump, no multi-select;
connection-count re-announces on every focus incl. snap-back-after-drag; undo covers move/rename but
not the destructive ops he uses most.

**Sam (accessibility / keyboard)**: region tabs are `<g tabindex=0>` — verify they expose an
accessible name the way stickies do (`role=button` + rich aria-label). Region create/resize are
unreachable by keyboard. Danger is conveyed chromatically; with the region colour regression fixed it
is red on-element again, but the text prompt is still green — reinforce with a text tone. The polite
connection-count fires on every Tab across 141 boxes — potentially heavy narration.

**Modeller mid-thought (solo + LLM)**: well served — help is one keystroke away, blur-cancel protects
a half-typed rename, the connection legend anchors the dense fan. Risk: the green destructive prompt
can momentarily read as "done, it's gone", and element/region removal is irreversible if the
two-step is completed on autopilot.

## Minor Observations

`#note` lingers ("15 connections") after context moves on — stale, not wrong · the corner × is
anchored at `r.top - 9`, which for a top-row sticky can ride under the header (an edge of a sound
fix) · `H1` (hotspot) with zero edges reads "no connections" cleanly — the zero case was handled ·
board re-render after region-remove is clean and server-authoritative (status stays "● live").

## Questions to Consider

1. Should a neutral "15 connections" and a destructive "remove …?" share one green live-region? A
   `data-tone` on `#note` would let the text channel pre-classify itself.
2. The armed-remove exists *because* remove is irreversible. If the append-only log can replay a
   `re-add` with the same id, is the two-step confirm still the right primitive — or does real undo
   let the gesture get lighter?
3. Region create/resize are mouse-only. Does a "calm instrument" accept that asymmetry, or does
   faithfulness demand full keyboard parity for every structural op?
4. Connection-count narrates on every focus — including each Tab across 141 boxes. On a dense board,
   is per-focus narration signal, or the very noise the calm instrument avoids?
