---
target: collapse feature (F-region-collapse)
total_score: 34
p0_count: 0
p1_count: 0
timestamp: 2026-07-03T20-29-29Z
slug: src-template-html
---
# Critique — F-region-collapse (fold a region to a band)

Scoped to the collapse feature only: the ▸/▾ region-tab disclosure, the `· N`
count chip, the thin folded band, and the `z` / triangle toggle. Assessment A
(heuristic design review against DESIGN.md / PRODUCT.md, product register) +
Assessment B (bundled detector on `src/template.html`).

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | Folded band reads almost identically to a merely-narrow region; only a 10px ▸ + `· N` mark the state |
| 2 | Match System / Real World | 4 | ▸/▾ is the universal disclosure metaphor; "fold to a band" matches the mental model |
| 3 | User Control and Freedom | 4 | Fully reversible, per-viewer, never touches the model; `z` and click both toggle |
| 4 | Consistency and Standards | 3 | Triangle-as-separate-hit-target lives inside the rename group — one visual object, two gestures |
| 5 | Error Prevention | 3 | View-state can't corrupt the model, but bare `z` collides with Cmd/Ctrl+Z undo while a tab is focused |
| 6 | Recognition Rather Than Recall | 3 | Triangle visible; `z` is recall-only; the `· N` count is unlabelled |
| 7 | Flexibility and Efficiency | 4 | Keyboard + mouse, persisted lens, composes with the `?base=` diff |
| 8 | Aesthetic and Minimalist Design | 4 | Calm, glass, no chrome at rest — genuinely on-register |
| 9 | Error Recovery | 3 | Unfold is trivial, but an offline toggle says "folded" even when the redraw fetch failed |
| 10 | Help and Documentation | 3 | Help-sheet line + title tooltip; count semantics and the z/undo collision undocumented |
| **Total** | | **34/40** | **Good / Excellent border** |

## Anti-Patterns Verdict

**LLM assessment: not slop.** Restrained and on-register — reuses the existing
folder-tab affordance instead of inventing a control, no card grid / gradient /
eyebrow / hero-metric tile, and fold state carries three non-colour signals
(triangle direction + `data-collapsed` + `· N`), honouring the colourblind-safe
principle.

**Deterministic scan:** detector returned one `warning` — `flat-type-hierarchy`
(11/13/16/17/20px). Known **false positive** for faceto's dense SVG board-label
scale (recorded in project memory). Not introduced by, nor specific to, collapse.

## What's Working

- The fold band is genuinely calm: a 60px slot + `· N` chip recedes rather than shouts.
- View-state, not a model edit — fold lives in its own localStorage key, never the log.
- Non-colour fold signal (triangle + data-collapsed + count).

## Priority Issues

- **[P2] Visible affordance ≠ keyboard affordance.** The ▸/▾ triangle has onclick but
  no tabindex/role; the focus ring is on the rename rect. Keyboard users fold only via
  `z`, undiscoverable from the tab. Fix: make the triangle a focusable control, or
  surface `z` on focus/aria. `/impeccable harden`.
- **[P2] `z` collides with undo.** Bare `z` on a focused tab stopPropagations, swallowing
  Cmd/Ctrl+Z. Fix: guard with `!e.ctrlKey && !e.metaKey`. `/impeccable harden`.
- **[P2] Folded state under-weighted.** Same wash + frontier as an expanded region; only a
  10px glyph distinguishes folded from narrow. Fix: a subtly distinct band fill. `/impeccable polish`.
- **[P3] Unlabelled count + aria drops it.** `work · 7` never says "elements"; aria-label
  omits the count. Fix: fold count into aria-label/title. `/impeccable clarify`.

## Persona Red Flags

**Alex (power user):** first Cmd+Z after touching a tab loses an undo to a fold — a silent
slip. Wants the triangle in tab order.

**Jordan (first-timer):** `work · 7` is ambiguous (stickies? columns? comments?); the ▾ is a
10px grey glyph that doesn't announce itself until hovered.

## Minor Observations

- Triangle sits left of the diff badge/label, nudging the region's identity anchor rightward.
- Offline toggle order can desync localStorage from what actually rendered.

## Questions to Consider

- Should folding be a property of the tab as a whole (click = fold, dblclick = rename)?
- Can a returning user see, at a glance, which regions they hid?
- Does `· N` earn the ambiguity, or should the band say what it hides?
