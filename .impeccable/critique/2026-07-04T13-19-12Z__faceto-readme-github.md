---
target: faceto README (github.com/bastien-gallay/faceto)
total_score: 27
p0_count: 0
p1_count: 2
timestamp: 2026-07-04T13-19-12Z
slug: faceto-readme-github
---
# Critique — faceto README (github.com/bastien-gallay/faceto)

Register: brand-adjacent (a project front door / long-form landing content). The README
is the one surface where a stranger decides in ~30s whether faceto is worth a `cargo
install`. Browser automation was unavailable; assessed from README source + the rendered
GitHub page. The bundled detector does not apply to a GitHub-rendered markdown URL.

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | CI badge + Status section are good; stale legacy content muddies "current state" |
| 2 | Match System / Real World | 3 | Plain, developer-fluent voice; one dense burst of event-sourcing jargon |
| 3 | User Control and Freedom | 2 | Long README, no in-body TOC / anchor nav (only GitHub's hidden hamburger) |
| 4 | Consistency and Standards | 3 | Style is consistent; content contradicts the actual shipped behaviour |
| 5 | Error Prevention | 2 | "legacy mode → comments.jsonl" instructions describe a removed code path |
| 6 | Recognition Rather Than Recall | 3 | Eight-lanes table + inline anchors help; a lot to hold at once |
| 7 | Flexibility and Efficiency | 3 | Quickstart-first serves novice + expert well; no TL;DR for pure scanners |
| 8 | Aesthetic and Minimalist Design | 3 | One clean hero, no badge-soup; mid-sections are dense walls of prose |
| 9 | Error Recovery | 2 | No troubleshooting; if `cargo install` fails the reader has nowhere to go |
| 10 | Help and Documentation | 3 | Good cross-links (CONTRIBUTING, CLAUDE.md, sample files) |
| **Total** | | **27/40** | **Good — a strong, voiced README with a few fixable trust + scannability gaps** |

## Anti-Patterns Verdict

**Does this look AI-generated? No.** This is a genuinely human, opinionated README. The
arrow-motif tagline ("typed file → visual board"), the name pun ("face-to" / "facet-o"),
and the specific technical detail (content-hash ring, server-minted ids, additive schema
evolution) are the opposite of AI slop. No section-eyebrow kickers, no hero-metric tiles,
no badge soup, no gradient anything. It matches PRODUCT.md's "calm, precise, faithful"
voice. The failure modes here are *content trust* and *scannability*, not slop.

Deterministic scan: n/a — target is a GitHub-rendered markdown URL, not local markup.
Visual overlays: not available — browser extension not connected this session.

## Overall Impression

A confident, well-written front door that shows its output above the fold and gets you to
a working command in 30 seconds. Two things hold it back from excellent: one section
actively describes behaviour that no longer ships (a trust leak), and the README's single
best selling point — the live click→note→diff loop — is described in prose but never
*shown*. The hero SVG proves "faceto renders a board"; nothing proves "faceto shows you
what changed," which is the actual magic.

## What's Working

- **Show, don't tell, above the fold.** The sample-board SVG sits right under the tagline,
  so a scanner sees the actual output before any prose. Exactly right for a visual tool.
- **Quickstart-first structure.** Install → 30-second tour → model file. A reader can copy
  two commands and see the board before committing to the deeper event-sourcing story.
- **Distinct, faithful voice.** Lowercase-leaning, plain, technical; the name-meaning aside
  and the "why zero dependencies" section give it a personality most tool READMEs lack.

## Priority Issues

- **[P1] Stale content: the "legacy mode → comments.jsonl" path no longer exists.** The
  "Click → note → event" section says *"In legacy mode (you served a model.json) it appends
  to a sibling comments.jsonl instead."* Per CLAUDE.md and PR #23 (F-auto-genesis), `serve`
  is now **event-log-only** — serving a `model.json` auto-genesises it to a log; there is no
  legacy comments.jsonl serve mode. **Why it matters:** a reader who trusts this will look
  for behaviour that isn't there; a stale factual claim in a README quietly discredits every
  other claim on the page. **Fix:** delete the legacy-mode sentence (and the "Either way"
  framing it sets up); state the single event-log path plainly.
  *Suggested command:* /impeccable clarify

- **[P1] The killer feature is told, never shown.** "Reload shows what changed" and the diff
  overlay are the differentiator, and the Status section literally lists "a short animated
  capture of the live click→note→diff loop" as a TODO. **Why it matters:** the one image
  proves rendering, not the live loop — the reader has to *imagine* the payoff. **Fix:** add
  a second visual: a GIF (or a before/after still) of a sticky being annotated and the diff
  overlay appearing on reload. Highest-leverage single addition on the page.
  *Suggested command:* /impeccable delight

- **[P2] Mid-README is a wall of dense prose.** "The event log is the source of truth",
  "Click → note → event", and "Reload shows what changed" fire event-sourcing concepts
  (projection, replay, compaction, content-hash ring, additive schema) in tight paragraphs.
  **Why it matters:** READMEs are scanned, not read; a first-timer hits high intrinsic load
  fast and bounces. **Fix:** lift one sentence of each to a lead, push the deep rationale to
  the already-linked docs/event-sourcing-status.md, and let the README stay lighter.
  *Suggested command:* /impeccable distill

- **[P2] No in-body navigation for a long README.** Ten H2 sections, no table of contents;
  the only jump-nav is GitHub's hidden hamburger menu. **Why it matters:** a reader who
  wants "just the model-file schema" or "just the lanes" has to scroll-hunt. **Fix:** a short
  TOC under the hero, or fold the reference-heavy sections (model file, event log, eight
  lanes) under `<details>` so the narrative reads top-to-bottom and reference expands on
  demand.
  *Suggested command:* /impeccable layout

- **[P3] Thin metadata + no license/troubleshooting.** One CI badge; no license badge, no
  "zero-dependencies" or Rust-version signal, no LICENSE mention, no "if install fails" note.
  **Why it matters:** evaluators read badges as a trust/maturity signal, and the zero-dep
  claim — the product's spine — isn't reinforced at a glance. **Fix:** add *one or two*
  restrained badges (license, and a "zero deps" shields.io static badge that reinforces the
  pitch) — not a badge wall, which would break the calm register. Add a one-line license
  mention near the bottom.
  *Suggested command:* /impeccable clarify

## Persona Red Flags

**Jordan (First-Timer evaluating in 30s):** Reads the tagline, sees the board image, copies
the tour — good so far. Then hits "The event log is the source of truth" and gets replay /
projection / compaction / content-hash-ring in three paragraphs with no picture of the
payoff. Risk of "this is more machinery than I wanted" bounce right at the depth transition.

**Alex (Power evaluator skimming headings + badges):** Sees a single CI badge and wonders
about license and maturity. Skims to "Click → note → event", reads the comments.jsonl
legacy path, and — if they also skim the code — notices it doesn't match `serve.rs`. Now
mildly distrusts the rest of the README.

**The mid-thought architect (project persona, from PRODUCT.md):** The audience faceto is
built for. Well served by the voice and the model-file example — but they came for the
"see what changed since last session" loop, and that exact loop is the one thing with no
visual. The README undersells the feature its own user cares about most.

## Minor Observations

- `faceto lint` is a real CLI verb (per CLAUDE.md) but never appears in the README.
- The hero SVG has a hardcoded light background (`#fbfbfd`); on GitHub's dark theme it reads
  as a bright slab. Arguably on-brand ("specimen on a bench"), but a `<picture>` with a
  dark-mode variant would sit better. Low priority — a deliberate call either way.
- "30-second tour" opens `examples/index.html` but `render` was pointed at
  `examples/sample.model.json`; a reader may not connect that the render wrote `index.html`
  next to the model. One clause would close the gap.

## Questions to Consider

- What if the second image (the diff loop) came *before* the event-sourcing prose, so the
  payoff lands before the machinery?
- Does the README need to carry the full model-file + event-log + eight-lanes reference, or
  is it doing docs' job — could `<details>` or a linked SCHEMA keep the front door lighter?
- If a stranger read only the hero + first screen, would they know faceto shows diffs — the
  one thing that isn't event-storming-tool table stakes?
