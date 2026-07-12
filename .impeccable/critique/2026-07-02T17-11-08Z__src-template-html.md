---
target: board gestures (src/template.html) — measure gain after v2 P1 fixes
total_score: 30
p0_count: 0
p1_count: 2
timestamp: 2026-07-02T17-11-08Z
slug: src-template-html
---
# Critique v3 — faceto board gestures (src/template.html)

Third pass, measuring the gain after the two v2 P1 fixes (keyboard add path; two-tone focus
ring). Same method: Assessment A live design review (Chrome DevTools MCP, CISAC CN2 model — 141
stickies, ~199 edge-nodes, 13 regions) + Assessment B deterministic detector. Both fixes verified
live and PASS; nothing regressed.

## Design Health Score

| # | Heuristic | v1 | v2 | v3 | Key issue now |
|---|-----------|----|----|----|---------------|
| 1 | Visibility of System Status | 2 | 3 | 4 | Live badge + note() + diff + focus-reveals-connectors; ring now visible on all 8 lanes |
| 2 | Match System / Real World | 3 | 3 | 4 | Event-storming grammar is the domain itself; left→right time, hotspots — exemplary |
| 3 | User Control and Freedom | 1 | 3 | 3 | Undo (Ctrl+Z), Escape cancels drag/rename/arm; still no region remove, redo unowned |
| 4 | Consistency and Standards | 3 | 4 | 3 | Keyboard now mirrors the mouse glyphs (a/Insert=+, F2, c, Del); but armed-× placement diverges between the two paths |
| 5 | Error Prevention | 2 | 3 | 3 | Two-step armed remove + non-blank guard; rename-on-blur still commits silently |
| 6 | Recognition Rather Than Recall | 1 | 2 | 2 | Mouse glyphs discoverable; the whole keyboard vocabulary is recall-only |
| 7 | Flexibility and Efficiency | 2 | 2 | 3 | Keyboard add closes the last gap — a full keyboard path now exists |
| 8 | Aesthetic and Minimalist Design | 4 | 4 | 4 | Ring defect fixed, board stays glass — exemplary |
| 9 | Error Recovery | 1 | 3 | 2 | Good note() prompts, but the keyboard-armed × strands off-canvas, breaking the recover loop |
| 10 | Help and Documentation | 0 | 1 | 1 | Still no discoverable help / shortcut surface |
| **Total** | | **19** | **28** | **30/40** | **Good band — +2; both fixes closed defects, ceiling still capped by #6/#9/#10** |

Note on #4 and #9: the raw dimension count moved up on 1/7/8 and the ring rescue lifted #1, but
Assessment A independently re-scored #4 and #9 *down* one from v2 — not a regression from the fixes,
a sharper read of two pre-existing flaws the keyboard-add path now makes load-bearing: the armed-×
placement diverges between mouse and keyboard, and keyboard arming strands the only confirm signal
off-canvas. Net +2, honestly earned: both fixes *closed defects* rather than adding breadth.

## Both fixes — verified PASS

- **Keyboard add** ✅ — focus a command sticky, press `a`: `#rename-edit` flips none→block, takes
  focus, opens at cmdRight+12 / cmdTop (exact mouse anchor), col = data-col+1, same lane. `Insert`
  also opens it; Escape cleans up with no post. Full keyboard create path now exists.
- **Two-tone focus ring** ✅ — computed across all 8 lanes: `stroke: rgb(255,255,255)` @2.5px + a
  blue (26,111,174) 0-blur offset casing, uniform. Screenshotted on the **command** lane (blue
  fill) — white keyline crisply visible where the old solid-blue ring vanished; also confirmed on
  **actor** (pale yellow), where the blue casing carries it. No regression.

## Anti-Patterns Verdict

**LLM assessment: "Not slop — earned craft."** Nobody would say "AI made this". The two-tone ring
is a genuinely non-obvious solution (white keyline cased in blue via a 0-blur offset drop-shadow
stack, inverted for has-note); keyboard add mirrors the exact mouse anchor; microcopy is specific,
not generic. The one construction tell is `region-remove` sitting in `STRUCTURAL_KINDS` with zero
emit sites — schema completeness ahead of a UI path.

**Deterministic scan**: 1 raw finding (`flat-type-hierarchy`, template.html:10) = the documented
false positive, ignore-listed in `.impeccable/config.json` → **0 effective findings**. Console
across a full load + gesture session: zero JS errors; one harmless favicon 404.

**Visual overlays**: none — the overlay server's port-bind is blocked by the sandbox (as v1/v2).
Live browser evidence (console read + computed-style probes + screenshots) is the fallback signal,
and it is clean.

## Priority Issues

- **[P1] Keyboard-armed remove strands the × off-canvas (still open, now more load-bearing).**
  `doRemove` only calls `classList.add("armed")`; it never positions the glyph (only the mouse
  `graceGlyph.at(x,y)` sets coordinates). Verified: focused sticky at (1019, 373), the × renders at
  (0, 1600), and the sticky itself carries no armed class. On the keyboard path the *only* visual
  confirm signal is invisible. **Fix:** in `doRemove`, on arm, position `#remove-x` from the
  target's `getBoundingClientRect()` corner (mirror the add anchor), AND add an `.arming` class to
  the focused sticky so the box signals danger regardless of glyph position.
- **[P1] No discoverable help / shortcut surface (heuristic 10, untouched).** No `aria-keyshortcuts`,
  no `?` affordance, no title hints. Every keyboard verb (`a`/`Insert`, `F2`, `c`, `←`/`→`, `Del`,
  `Ctrl+Z`, `Esc`) is undiscoverable — the keyboard-add fix delivers power nobody can find. **Fix:**
  a `?` key opening a dismissible shortcut sheet, or a persistent one-line hint rail. Low cost,
  unlocks everything already built.
- **[P2] Regions have no remove gesture (still open).** `region-remove` is declared but has zero
  emit sites; add/resize/rename exist, delete does not. 13 regions here and no way to drop one.
  **Fix:** wire a region ×/Delete on the focused region rail to post the already-defined kind.
- **[P2] Rename commits on blur (still open).** `#rename-edit` blur → `endRename(true)`. Clicking
  away from a half-typed rename silently writes it; the only abandon is remembering Escape *before*
  losing focus. **Fix:** treat blur as cancel (commit only on explicit Enter) — matches the
  "Escape cancels" model used everywhere else.
- **[P3] Focus reveals connectors but there's no legend/count for the highlight.** Focusing the
  actor lane lit ~30 edges at once with no count; impressive but unreadable on a ~199-edge board.

## Persona Red Flags

**Alex (power user)**: now genuinely served — full keyboard verb set, undo, column nudges — but he
will never learn `a`/`c`/`F2` exist without a cheat sheet, and the first keyboard-armed delete with
no visible × will make him distrust the whole keyboard path. The fix he most needs is the help
sheet, not more verbs.

**Sam (accessibility / keyboard)**: ring visibility is a real win — focus is now perceivable on
every lane. Three blockers remain: (1) the armed-× is off-screen for keyboard users — the confirm
affordance is unreachable without a mouse; (2) `aria-keyshortcuts` is absent, so a screen reader
announces no shortcuts; (3) the focus-reveals-connectors state isn't announced. Keyboard
*operability* improved; keyboard *legibility of consequences* did not.

**Modeller mid-thought (solo + LLM)**: the peak loop — focus a box, type `a`, name the next event
one column right — now works without touching the mouse, exactly the "think through a typed file
with an LLM" flow. But rename-on-blur is a mid-thought trap: glance at the LLM's suggestion and your
half-typed label commits. And when the LLM over-scopes a region, there's no gesture to prune it. The
instrument helps you add thoughts faster than it lets you retract them.

## Minor Observations

Status microcopy is excellent ("kept", "add needs a label — nothing added", "remove X? — same
gesture again to confirm") — heuristic-9 help doing real work; it just can't compensate for the
stranded glyph · focus survives board swaps (spotlight + keyboard claim return) — a subtle correct
detail · `a` is a bare unmodified letter — a future text-search/filter feature will contend for it ·
the favicon 404 is the one console blemish.

## Questions to Consider

1. You built a complete keyboard vocabulary and then hid it entirely — is a calm instrument one with
   *no* visible controls, or one whose controls are quiet but *findable*? Right now it's the former.
2. The armed-× has been stranded on the keyboard path across two review cycles. Is the two-path
   glyph architecture (mouse positions, keyboard doesn't) worth keeping, or should arming always
   position from the element rect regardless of trigger — making the bug structurally impossible?
3. Rename commits on blur but remove needs a second explicit confirm. Why is destroying a *label*
   safer than destroying an *element*? The asymmetry suggests the danger model isn't consistent.
4. `region-remove` exists in the schema but not the UI. Did the enum lead the design — and how many
   other "supported" operations have no gesture a modeller can actually perform?
