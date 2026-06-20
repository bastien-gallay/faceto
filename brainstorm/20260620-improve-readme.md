# Brainstorm: Improve the README (screenshots + model/comments extracts)

| Field | Value |
| --- | --- |
| **Date** | 2026-06-20 |
| **Duration** | ~14 min (11:08 – 11:22) |
| **Participants** | User + AI Facilitator |
| **Problem shape** | Few ideas → converged early to Decision-under-constraints |

## Session Plan

| # | Phase | Technique | Duration | Status |
| --- | --- | --- | --- | --- |
| 0 | Intake | Grounding (grep repo) + seed/extend | 4 min | Done |
| 1 | Step | SCAMPER | — | Skipped (early convergence on scope) |
| 2 | Step | Impact / Effort + feasibility | 6 min | Done |
| 3 | Crystallize | Action items | 4 min | Done |

## Ideas — Starting Point

Seeds (user): screenshots, model/comments file extracts.
Extended (AI): GIF of the live loop, fix event-sourcing framing, 30-sec quickstart,
colour-grammar legend.

## Grounding findings

- README is 95 lines, strong prose, **zero images**. `examples/board.svg` already exists and
  renders natively on GitHub → free hero image.
- README never mentions the **event-sourcing spine**; still frames `comments.jsonl` +
  "when `model.json` changed" as the model. Per `CLAUDE.md` the durable truth is
  `event-log.jsonl` and the `Model` is a projection. **Correctness gap, not just polish.**
- `examples/` ships `comments.jsonl` but **no `event-log.jsonl`**, despite CLAUDE.md saying the
  log is tracked. Running `faceto genesis` would produce the real artefact to extract from.

## Step 2: Impact / Effort (11:14 – 11:20)

### Output

| Deliverable | Impact | Effort | Drift | Verdict |
| --- | --- | --- | --- | --- |
| Inline `board.svg` hero | High | ~0 | None | Ship now |
| Model extract + caption | High | Low | None | Ship now |
| Fix event-sourcing framing | High (correctness) | Low–Med | — | Ship now |
| Generate + track `examples/event-log.jsonl` | Med (enabler) | Low | None | Ship now |
| Event-log / comment extract | High | Low | None | Ship now |
| Colour-grammar legend (8 lanes) | Med | Low–Med | Low | Ship now |
| 30-sec quickstart path | Med | Low | — | Ship now |
| PNG of live page | Med | Med | High | **Dropped** (SVG covers it) |
| Animated GIF click→modal→diff | High | High | High | **Follow-up + regen recipe** |

### Feedback

> Scope: visuals + framing + examples fix. Visuals: all four formats.
> Binaries: GIF as follow-up, drop PNG.

### Facilitator note

Key insight: zero-deps is a binary rule, not an asset rule — PNG/GIF are allowed but drift.
SVG + code extracts are self-syncing; that asymmetry drove dropping the PNG and deferring the GIF.

---

## Outcome

### Selected Ideas / Decisions

1. **Inline the existing `board.svg` as a hero image** — high impact, zero new assets, stays
   in sync on every `faceto render`.
2. **Add a worked model→board extract** — a fenced slice of `sample.model.json` next to the
   board it produces, so the "typed file → visual" premise is shown, not just claimed.
3. **Add an event-log / comment extract** — show the click→note→**event** flow with real JSONL
   lines, replacing the legacy comments-only framing.
4. **Fix the event-sourcing framing** — state that `event-log.jsonl` is the durable truth and
   the `Model` is a projection; correct the "when `model.json` changed" language in the
   Reload/diff section.
5. **Generate + track `examples/event-log.jsonl`** via `faceto genesis` so all extracts come
   from real shipped files (and the repo matches what CLAUDE.md claims is tracked).
6. **Add a colour-grammar legend + a 30-second quickstart path** — small visual/onboarding wins.
7. **Drop the PNG; defer the GIF** to a follow-up that includes a documented regeneration recipe.

### Action Items

- [ ] Run `faceto genesis examples/sample.model.json` → commit `examples/event-log.jsonl` — owner: user/Claude — this PR
- [ ] Rewrite README: inline SVG hero, model extract, event-log extract, fixed spine framing,
      lane legend, quickstart — owner: Claude — this PR
- [ ] Sanity-check all extracts are copy-paste-faithful to the committed example files — this PR
- [ ] Open a follow-up issue: "Animated GIF of click→modal→diff loop + regen recipe" — follow-up

---

## Session Meta-Analysis

- **Duration:** ~14 min
- **Techniques used:** Grounding (grep), Impact/Effort + feasibility
- **Techniques skipped:** SCAMPER (early scope convergence — user chose full breadth at intake)
- **Adaptations made:** dropped divergence; led with feasibility because the real tension was
  effort/drift asymmetry, not idea shortage
- **Problem shape:** Few ideas → Decision under constraints
- **Convergence point:** Step 2 (Impact/Effort) — the binary-asset fork settled scope
- **What worked well:** grounding surfaced a correctness gap (stale spine framing) the user's
  cosmetic ask would have papered over
- **What could improve:** could have caught the missing `event-log.jsonl` in intake without the
  second grep
- **Session energy:** high, decisive
- **Recommendation for similar sessions:** for "improve the docs" asks, always grep the docs
  against the code's stated invariants first — doc/spine drift is the highest-value find
