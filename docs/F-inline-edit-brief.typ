// F-inline-edit — working brief (steps 1–6 of the TDD process).
// Compile: typst compile docs/F-inline-edit-brief.typ
// Purpose: hold the intent so the next reader (or the next session) starts with
// the root cause and the test contract already in hand — not re-derived.

#set page(width: 21cm, height: auto, margin: 2cm)
#set text(font: ("Iowan Old Style", "Palatino", "Georgia"), size: 10.5pt)
#set par(justify: true, leading: 0.62em)
#show heading: set text(weight: "bold")
#show heading.where(level: 1): set text(size: 16pt)
#show heading.where(level: 2): set text(size: 11.5pt)

#let chip(body) = box(fill: rgb("#eef1f4"), inset: (x: 5pt, y: 2pt),
  radius: 3pt, text(size: 9pt, body))

= F-inline-edit — the board as a direct instrument

#chip[branch `feat/F-inline-edit`] #chip[issue: ROADMAP.md] #chip[status: tests red]
#chip[2026-06-20]

== 1 · Assignment

Single-maintainer repo; F-inline-edit is the top *Now* slice (status ☐) in
`ROADMAP.md`, which is the issue of record — there is no GitHub issue. Owned, in
scope, started on its own branch.

== 2 · Root cause & isolation

Editing today is *modal-only*: every rename/remove routes through the comment
dropdown (`#modal` in `template.html`). Move is already direct (← / →, Move
←/→ buttons), so this slice adds *direct rename* and *direct remove* gestures and
demotes the modal to "optional, not the only path".

Wiring a direct rename exposes a latent defect. The `rename` arm of
`comment_to_events` (`events.rs`) — and `replay` — accept a *blank label* and
persist it, producing a never-renumbered empty box. This is the exact failure the
`add` path already guards (`add_from_comment` in `serve.rs`: trim, reject empty).
Inline editing makes "select-all → delete → Enter" a one-gesture mistake, so the
gap stops being theoretical.

*Tidy-first.* The non-blank-label rule exists once, inline, in `add_from_comment`.
Green step extracts it to one named helper (`nonblank`) reused by both the `add`
and the new `rename` guard — one rule, one name, two call sites. The invariant
lives in the Rust *domain seam*, not only in client JS, so the client stays thin
and the rule stays testable and authoritative (CUPID: predictable, domain-based).

== 3 · Test data

From `examples/sample.model.json` / `event-log.jsonl`: real ids and labels —
`E1` `ItemAdded`, `C1` `start the day`, `A1` `DayPlan`, `H1` (hotspot). The PBT
genesis uses one element per lane prefix (`E1 E2 C1 A1 H1`) so id-prefix minting
and lane handling are exercised, not just a single lane.

== 4 · Related docs

`docs/event-sourcing-status.md` and `docs/source-of-truth.md` (the log is truth,
the model is a projection — so the guard belongs at the write seam, not at
replay); `CODING_STANDARDS.md` (TDD + Reflect, CUPID, Tidy First); `DESIGN.md` /
`PRODUCT.md` (calm instrument — the gesture must stay quiet, no new chrome).

== 5 · Note in the issue

Recorded under *Working note — F-inline-edit* in `ROADMAP.md`: root cause + the
tests-to-done checklist below.

== 6 · The test contract (red first)

#table(
  columns: (auto, 1fr, auto),
  inset: 6pt, align: (left, left, center), stroke: 0.4pt + rgb("#ddd"),
  table.header([*Kind*], [*Names the behaviour*], [*now*]),
  [UT], [`rename` rejects a blank/whitespace label → nothing to persist], [red],
  [UT], [`rename` trims surrounding whitespace], [red],
  [UT], [a real rename still renames (non-regression)], [green],
  [PBT], [no comment sequence ever leaves a blank label (500 seeds)], [red],
  [PBT], [comments never invent an element; only `drop` removes (500 seeds)], [green],
  [Integ], [blank rename appends nothing; a real one persists one `ElementRenamed`], [red],
)

The two green tests are deliberate: they pin the *adjacent* move/drop/annotate
semantics this feature sits next to, so the green step can't silently regress
them. PBT is hand-rolled (a small deterministic LCG) because faceto takes no
crates; a failing seed prints its full comment sequence for exact replay.

*Decision pinned:* the guard goes at the write seam (`comment_to_events`), not in
`replay`. `replay` stays a faithful projection — a blank rename already on disk is
history and replays as-is; we simply never write one. Symmetric with `add`.
