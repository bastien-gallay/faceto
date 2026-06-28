<!-- markdownlint-disable MD013 -->

# F-container — scope & decision record

Status: **scoping (model layer not yet built)** · Branch: `feat/F-container` · Last updated:
2026-06-28

The missing **bounded-context / region** primitive. A region is a labelled vertical band over a
column range, spanning all lanes; an element belongs to the region its `col` falls inside.
F-container is a hidden hub — UI bounded-context editing, [F-model-smells], and [F-ddd-process]
all silently depend on it, and the model has no region concept today. Cheap to add now,
expensive to retrofit once logs carry the shape.

## The picture

```text
lanes ↓   │   region / context A   │      region B     │
 actor    │  [X1]                  │                   │
 command  │       [C1]             │       [C2]        │
 event    │  [E1]       [E2]●pivotal│     [E3]          │   ● = event sitting on the border
 …        └── col → (time, left→right; membership = which band the col is in) ──┘
```

| Axis | Means | Consequence for the model |
| --- | --- | --- |
| **Horizontal** (`col`) | time **and** which region | meaningful for **events** (ordering + context) |
| **Vertical** (lane / `type`) | kind + intra-lane stacking | cosmetic reorder, no domain meaning |

## Decisions locked

These are the answers worked out before any code. They follow the project's existing grain
(event-sourced spine, `col` as the global timeline, identity by stable `id`, the calm-instrument
register).

### D1 — A region *evolves* `Phase`, it is not a parallel primitive

A region is geometrically identical to today's `Phase { label, from_col, to_col }` (the decorative
"soft vertical zones behind the timeline", `render.rs`). Two vertical-band systems on one board
would fight the calm-instrument register. So F-container **gives `Phase` teeth** — membership,
borders, pivotal, drag — rather than adding a second band concept. Whether the type is renamed
`Region` or kept as `Phase` is cosmetic; the decision is *one band system, not two*.

### D2 — Membership is spatial (the box *is* the membership)

An element belongs to the region whose `[from_col, to_col]` contains its `col`. There is **no
`container` field on `Element`** and **no member list on the region**. The stored region bounds are
the single source of truth, so membership can never drift from geometry. Overlap tie-break:
innermost (smallest) band wins. This is the only membership model that does not bolt a second
record onto a region that already stores its bounds.

Cost accepted: an element cannot belong to a region it is not drawn inside (no non-contiguous
membership). For event storming that constraint is a feature — non-contiguous membership is a
smell, not a capability.

### D3 — Pivotal events are derived, events-only

A **pivotal event** is *an `event`-lane element whose `col` equals a region-boundary col*. It is
**derived from position, never a stored flag** — consistent with D2's spatial single-source-of-truth
and immune to drift. Type-gated: a command / read-model / actor cannot straddle a border (on drop
it snaps to one side). A pivotal event bridges both adjacent contexts and is never flagged an
orphan.

### D4 — No edge cascade on region change; if ever, it is serve-time fan-out

Dragging an element across a border moves **only that element**. Cross-region edges are **not a bug —
they are the context map** (a policy in B reacting to an event in A is the whole point of drawing
boundaries). No auto-follow, no modal on drop (a per-drop modal breaks the calm instrument).

If a cascade is ever wanted, the correct rule is *bring only the linked cluster confined to the
source region* (the cohesive subgraph), behind a held modifier — never a default prompt. **Hard
spine guardrail:** any cascade expands into **N explicit `ElementMoved` events at append time**
(`serve.rs`). `replay` stays pure and deterministic — "moving A also moved B" can never be implicit
at replay.

### D5 — Resize disambiguates from pivotal by grab target, not by hover-%

"Drag near a border" must not mean both *make pivotal* (D3) and *widen region*. Resolve by **what is
grabbed**, not by threshold zones:

| Gesture | Grab target | Result |
| --- | --- | --- |
| Drag an **element** onto a border | a sticky | **pivotal** (derived, D3) |
| Drag a **region edge handle** | the band's border itself | **resize / widen** |
| Split on the timeline axis | axis `+` / divider | **new region** |

Deterministic, no fuzzy 30/60 % thresholds to tune — fuzzy thresholds read as *imprecise*, the
anti-reference for a calm instrument.

## What this pins in the model layer (built first)

- A region carries `id`, `label`, `from_col`, `to_col` (stored bounds — already `Phase`'s shape).
- **No** `container` field on `Element`, **no** `pivotal` field — both derived from `col` vs region
  bounds.
- New **additive** events: `RegionAdded` / `RegionResized` / `RegionRenamed` / `RegionRemoved`
  (or evolve `PhaseAdded`). Membership and pivotal need **zero** events — they fall out of geometry.
- Region ids get their own **mint namespace** (e.g. `K<N>`, outside the eight lane prefixes),
  minted under the appends lock like every other id.
  - ⚠️ **Stage 5 server mint must share the namespace with replay's synthetic ids.** Stage 1 already
    mints synthetic `K<n>` for legacy (id-less) bands at replay time (`model::resolve_region_id`,
    "one past the highest `K` suffix ever seen"). The Stage 5 server mint must therefore scan
    `PhaseAdded` ids (not just `ElementAdded`) and reserve removed-but-not-compacted suffixes, exactly
    like `serve::mint_id` does for lanes — otherwise a server-minted `K2` could collide with a
    synthetic `K2` already in a legacy log.
- `diff_models` gains region diffing (added / removed / renamed / resized, keyed on `id`).

## v1 boundary

Per the scoping call, v1 is **model brick + on-board UI** (not model-only): data model + events +
replay + diff + render outline, *and* the client gestures (draw / resize a region, drag elements
across borders) with server-side mint + append. The drag-n-drop substrate landed by F-inline-edit /
F-inline-add is the foundation the gestures layer onto.

Build order is model-first regardless (this doc, then the [plan]); the gestures are meaningless
until replay + render carry regions.

## Why now

Borders, spatial membership, and derivation are near-impossible to retrofit once logs carry the
shape. Deciding the *model* now — not the gestures — is the cheap-vs-expensive split that makes
F-container the keystone for [F-model-smells] and [F-ddd-process].

[F-model-smells]: ../ROADMAP.md
[F-ddd-process]: ../ROADMAP.md
[plan]: ./F-container-plan.md
