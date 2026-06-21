# Cognitive / intent note — F-inline-add

Paired session (`/pair-with-me`), full adversarial depth. Started 2026-06-21.

## Intent (why this, why now)

Editing the board must not route exclusively through a modal. F-inline-edit made
rename / move / remove direct; F-inline-add closes the set by making **creation**
direct too. The verdict was **ship** — but the honest justification is narrower than
"mandatory for editing": `add` already works through the modal, so the direct gesture
is mostly *ergonomics*. The one place it is genuinely mandatory is the **empty / near-empty
board**: the modal `add` anchors "from this one" (`activeCol+1`) and so cannot bootstrap
a board that has no element to anchor on. That bootstrap case is the load-bearing reason.

## The boundary that was attacked and held

The initial verdict smuggled in "lane/**domain** cell group" and "spawn child: domain
separation". Domains do not exist on the board — that is **F-container** (roadmap: Later,
the "hidden hub"). Letting it in would have turned a client-only, zero-Rust slice into a
model-spine change. **Decision: F-inline-add stays lane-only (the 8 `type` lanes), client-only.**
F-container spins off as its own future session; the richer per-element gestures spin off as
**F-board-gestures** (both now recorded on the roadmap).

## Locked decisions (after Phase 1)

- **Two gestures, `+left` dropped:** (1) hover element → `+` on its **right** edge → mint same
  lane at `anchorCol + 1` (reuses the tested modal payload); (2) hover a **lane title** → `+` →
  prepend at `laneMin − 1`. Gesture (2) absorbs first-element, prepend, and empty-board bootstrap.
- **Render change R (accepted):** all 8 lanes always render, so the empty board shows the lane
  scaffold and every lane title is hoverable. Rationale: an event-storming beginner would be
  confused by a blank canvas. This is the slice's only non-client change.
- **Modal:** strip the `add` option and its dead `<select id="m-type">` lane picker; modal becomes
  prose-only.
- **`col` wrinkle dissolved:** dropping `+left` removes the file-order tie-break; prepend at strict
  `laneMin − 1` sorts left unambiguously, and the renderer already draws negative/sparse `col`.

## Why each fork landed where it did (the adversarial trail)

- Domain/bounded-context in the gesture → that's F-container, parked Later; held the line lane-only.
- "Mandatory for editing" → true only for the empty-board bootstrap (modal anchors "from this one").
- `+left` → dropped; its only real use (extreme-left/first element) is served by the lane-title `+`.
- Lane-title `+` only works if empty lanes render → forced render change R → human chose R over a
  separate empty-board affordance, for beginner onboarding.

## Regression surface to keep green (Phase 5)

- Absolute-y render tests on sparse models shift under R (e.g. `a_lone_sticky_stays_on_the_lane_mid_line`).
- `the_add_element_picker_offers_every_lane` is removed with the `<select>`.

## As-built refinements (Phase 4 verify → reopened Phase 2)

Two issues surfaced in browser verification and were fixed test-first:

- **Prepend col rule.** The first `min − 1` rule shoved every lane right when the *first* element
  of an empty lane was added. Refined to `lane_left_col(model, kind)`: an **empty target lane**
  aligns to the board's existing first column (no shift); a **non-empty lane** still prepends at
  `min − 1` (a "confusing-but-acceptable" case the user chose to defer). Empty board → 0.
- **Move-left guard.** `doMove` rejected any `target < 0` as "already at the left edge". Obsolete
  once prepend mints negative cols (the renderer draws them); the guard was removed, so move-left
  grows the board left symmetrically with prepend.

## Known gaps (deferred, not silently dropped)

- **No keyboard path to add.** The `+` is hover-only (`display:none` at rest), and removing the
  modal `add` deleted the only keyboard add-path. A focused-sticky key (e.g. a `+`/Insert →
  add-after) belongs in **F-board-gestures**. Flagged, not fixed in this slice.
- **Non-empty-lane prepend** still feels confusing (deferred by the user).
- **Ghost `+` doesn't follow board scroll** after it appears (it re-positions on hover only). Minor.
