---
paths:
  - "src/template.html"
  - "src/client/**"
  - "src/render/**"
---

<!-- markdownlint-disable MD013 -->

# UI / design context

Loaded when editing the board's rendered surface (`src/template.html`, `src/client/`, `src/render/`).

`faceto` carries an impeccable design context. **Register: `product`** — the live HTML+SVG
board is app UI that serves the event-storming workflow, not a brand/marketing surface.
Personality: a **calm instrument** (calm, precise, faithful) — the model is the subject,
the UI is glass. Strategic principles and anti-references (no SaaS-dashboard chrome, no
heavy branded chrome, no Miro/FigJam maximalism, no toy/childish look) live in
[`PRODUCT.md`](../../PRODUCT.md). Visual system (the 8-lane colour grammar, typography, spacing,
components, diff styling) is captured in [`DESIGN.md`](../../DESIGN.md). Read both before any UI
work on `src/template.html` or `src/render/`.
