# faceto — Roadmap

First direction, chosen 2026-06-20: **the board as a single-player instrument.**
Make editing direct and the board legible before adding any new participant (human
or AI) or new format. Horizons are dogfood-driven, not date-pinned — they shift as
real sessions surface the next felt pain. Source: `.personal/brainstorm/20260620-roadmap-first-direction.md`.

**Status:** ☐ todo · 🚧 in progress · ✅ done
**Horizon:** Now (this slice) · Next · Later (named, deferred) · Parked (not felt yet) · ✅ Shipped (done)

## Single source of truth

Three registries, canonical **per concern** — don't hand-sync them all:

- **Issues** (`bastien-gallay/faceto`) — the capture surface: anyone files a bug/enhancement; it
  auto-flows into Project #2.
- **[Project #2](https://github.com/users/bastien-gallay/projects/2)** — canonical for **priority,
  order, and Horizon** (the `Horizon` field: Now/Next/Later/Parked/Shipped). Edit these here,
  visually — not in this file.
- **This file (`ROADMAP.md`)** — canonical for the **narrative**: rationale, seam-clustering,
  torture verdicts, shipped history, and the per-row `Summary` prose.

Each live row carries a primary **`Tracked #N`** (its issue). Before starting any work, run
**`just sync-roadmap`**: a one-way generator that overwrites only the `Status` + `Horizon` columns
of tracked rows from the board/issues (`Done → ✅`, `In Progress → 🚧`, `Todo → ☐`), leaving the
`Summary` prose untouched. `just roadmap-check` reports drift without writing (live rows missing an
issue; open issues not referenced by any row). Rows without a `Tracked #N` (shipped history,
umbrella rows) are historical and never rewritten.

## Feature catalog

**Reprioritised 2026-07-04** (single-player thesis reaffirmed): `F-narrative-skill` is the
sole live **Now** item — the on-thesis answer to "solo & stuck". The collab pair
(`F-collab-sse`, `F-event-author`) is **demoted Next → Later**: collaboration is real (the
cloudflared-tunnel dogfood) but not the felt pain while the solo loop still has an open item.
Shipped features carry **✅ Shipped** in the Horizon column so the live work reads cleanly.

**Feature-torture triage 2026-07-05** added 15 wishlist/dogfood ideas as F-rows across 4 new
seam clusters — **A** client-architecture (`F-js-modules`), **B** power-gestures (`F-duplicate`,
`F-multi-select`, `F-command-box`), **E** export (`F-mermaid-export`, `F-narrative-export`), **D**
theming (`F-theme`) — landing 5 in **Next**; the rest (chat+MCP, image/Miro import, TUI, plugins,
multi-board, C4-xref, code-binding) parked as sibling-apps or big bets. Per-idea verdicts are
inline in each row. The 8 live-horizon rows are tracked as #67–#74; the 7 Parked rows carry
`issue TBD` (minted when they leave Parked).

**Next — worked by seam cluster, in order:**

1. **CLI / `main.rs`** — `F-output-naming` + `F-cli-help`, one dispatch pass;
   `F-model-export`'s `export` verb rides the same pass.
2. **Source / comment representation** — `F-model-export` → `F-comment-lifecycle` →
   `F-commit-flow` (pulled up from Later); all touch the exported-array-vs-log duality — touch
   it once.
3. **Lane-set seam** — one joint shaping of `F-lane-flow` + `F-floating-hotspots` +
   `F-es-vocabulary` (promoted to Later to sit with the bundle); ship `F-lane-flow` out of it
   and leave `LANES` / `colour` / `lane_prefix` extensible for the other two.
4. **Client + power-gestures seam** — `F-js-modules` (split `template.html` + JS tests) **first**,
   then the shipped gesture engine grows `F-duplicate` → `F-command-box`; `F-multi-select` follows
   in Later. Touch the client blob once, testably, before piling gestures onto it.

| ID | Direction | Status | Horizon | Summary |
| --- | --- | --- | --- | --- |
| F-inline-edit | UI · direct edit | ✅ | ✅ Shipped | Rename / move / remove elements directly on the board; the comment box becomes optional, not the only path. Wires client gestures to the existing `ElementRenamed/Moved/Removed` events + server-side minting — high impact, low effort. Shipped PR #4. |
| F-inline-add | UI · direct edit | ✅ | ✅ Shipped | Direct on-board element creation (the `add` substrate already exists end-to-end via the modal). Hover-element `+` and an empty-board affordance replace the modal dropdown's `add` option. Lane-only, client-only. Shipped PR #5. |
| F-edge-routing | UI · legibility | ✅ | ✅ Shipped | Reduce edge crossings via a layout heuristic in `render.rs`. Self-contained, no model-spine change. Two levers: barycenter within-cell ordering + fan-out edge anchoring (both ports kept in lockstep). Shipped PR #6. |
| F-container | model · grouping | ✅ | ✅ Shipped | The missing bounded-context / region primitive (vertical bands; spatial membership; derived pivotal). Model brick (PR #8), render (PR #9), serve mint/append (PR #10), client gestures (PR #11) — create/resize/rename a region directly on the board. Decisions + plan in [`docs/F-container-scope.md`](docs/F-container-scope.md) / [`docs/F-container-plan.md`](docs/F-container-plan.md). Unlocks F-model-smells and F-ddd-process. |
| F-narrative-skill | AI · participant | ✅ | ✅ Shipped | *(was F-mcp-narrative — reshaped by feature-torture 2026-07-02.)* Reverse-narrative / discovery skill: an LLM agent reads `event-log.jsonl` directly and proposes events through the existing `POST /comment` seam (server-side minting + guards + append mutex, all shipped). Prompt-ware only — **no new Rust**; the participation seams already exist (`serve.rs` re-reads the log per request, so agent appends show live). The on-thesis answer to "solo & stuck". Skill: [`.claude/skills/faceto-narrate/`](.claude/skills/faceto-narrate/SKILL.md); docs + hand-verified worked example: [`docs/F-narrative-skill.md`](docs/F-narrative-skill.md); spec: [`docs/F-narrative-skill-spec.md`](docs/F-narrative-skill-spec.md); torture report: `.personal/feature-torture/reports/F-mcp-narrative.md`. Tracked #76. |
| F-auto-genesis | CLI · migration | ✅ | ✅ Shipped | **Kill legacy mode.** `faceto serve model.json` **auto-runs genesis** — creates `event-log.jsonl` beside the model (the shipped genesis path, `main.rs`; the sibling-`comments.jsonl` fold it originally carried was later cut — see F-model-export) — then serves in **log mode only**; if the sibling log already exists, the log wins (it is truth, the model is derived) and the `.json` argument just redirects to it. Removes `serve.rs`'s entire legacy branch (`log_mode = false`: `comments.jsonl` appends, structural gestures stored as dead comments) — the "gesture lies" defect dies **by construction**, not behind a warning. `model.json` stays as a **read-only retrieval / bootstrap format**: `render` and `genesis` keep reading it purely; only serving (mutation) forces the log. Supersedes the legacy-mode-guard idea (F-region-frontiers torture + working note, 2026-07-03). Sequenced **before** F-region-frontiers so the frontier client work never grows a legacy branch. Tracked: #20. Shipped 2026-07-03 (`main.rs` `serve_log_path` + `write_genesis`; `serve.rs` reduced to event-log-only). |
| F-mcp-server | AI · interop | ☐ | Parked | std-only stdio JSON-RPC MCP server exposing read-log / propose-event tools. Spawned from the F-mcp-narrative torture (2026-07-02): redundant while the dogfood agent has file + shell tools. Revisit when a shell-less client (claude.ai, Claude Desktop) becomes a real usage context, or a second agent platform needs typed tool discovery. |
| F-multiplayer | collab · network | ☐ | Parked | *(shaped by feature-torture 2026-07-03 → 🧬 split; umbrella row.)* Multi-collaborator over network + event reconciliation + user naming. The **live-collab loop** was un-parked once a real multi-user need appeared (a cloudflared tunnel proved the log already propagates edges both ways) and split into ship-now children **F-collab-sse** + **F-event-author** (Next) and the reshaped **F-live-statepreserve** (Later), with heavier reconciliation pieces in the backlog rows below. This umbrella stays Parked as the collective name; the felt work lives in the children. Torture report: `.personal/feature-torture/reports/F-collab-live.md`. |
| F-collab-sse | collab · network | ☐ | Later | *(spawned by F-collab-live torture 2026-07-03 — the lead child, ship.)* Replace the `/model-version` **polling** (`template.html:276` `liveVersion`) with a `GET /events` **SSE** stream (`text/event-stream`, zero-dep — just HTTP lines): the server broadcasts one line per append at the single locked write point (`serve.rs:81` `append_line`, the H4 mutex), the client swaps the poll for an `EventSource` and refetches the plain board on ping. *The* leverage — polling→push is the change that makes the board feel live. ~80–120 LOC, no client model. Edges: reconnect/backfill after a dropped stream; a mid-session `compact` forces a full reload. |
| F-event-author | collab · identity | ☐ | Later | *(spawned by F-collab-live torture 2026-07-03 — smallest change, highest social value/LOC, ship.)* Additive **`author`** field on events (`events.rs` `Event`, threaded through `parse_event`/`to_json`/`replay` — old logs replay identically) + a **name-on-connect** prompt, surfaced on diff/sidebar entries. Independent of SSE (attribution works even under polling). Shares the sidebar/identity seam with **F-comment-lifecycle** — sequence together. Note: no auth (public tunnel) → the name is unverified courtesy, not identity; say so in UI copy. |
| F-live-statepreserve | UI · legibility | ☐ | Later | *(spawned by F-collab-live torture 2026-07-03 — the reshaped priority #3.)* Preserve **scroll / selection / in-progress edit** across the existing SVG swap, so a co-author's update no longer blows away local state. Explicitly **not** a JS port of `replay` — the torture killed the full client-side-model port (~400 LOC duplicating `render.rs`, a permanent drift tax against the log-is-truth spine). Revisit the port only if this cheap preservation proves insufficient in real N≥2 dogfood *and* per-element live patching becomes the felt pain. **Dogfood 2026-07-04:** the **scroll slice is a single-player P1 bug** — the board jumps to top-left on any element action (`swapBoard` doesn't save `scrollLeft` / `scrollTop`); split out from the collab framing and promoted to do-next. **Scroll slice shipped 2026-07-05 (#46):** `swapBoard` snapshots `#board` scroll before the `innerHTML` swap and restores it after re-bind, with `focus({preventScroll:true})`, and reveals the focused box only when a move strands it outside the restored viewport (`revealScroll`); selection + in-progress-edit preservation still open. Tracked #46. |
| F-collab-concurrency | collab · network | ☐ | Parked | *(backlog from the F-collab-live design chat 2026-07-03.)* **Semantic** concurrency: client posts carry a `base=<version>` (optimistic concurrency, mirroring the read-side `?base=` diff seam) so the server can reject / flag a write against a stale field, instead of silent last-write-wins on a same-element edit. The append mutex (H4) already prevents *storage* races; this closes the *semantic* gap. Two commuting `ElementMoved` on distinct elements already merge cleanly — this is only for same-element concurrent edits. Depends on F-event-author (needs a version/actor to reason about). |
| F-presence | collab · awareness | ☐ | Parked | *(backlog from the F-collab-live design chat 2026-07-03.)* **Ephemeral** presence — who's online, whose selection/cursor is where, soft-locks ("someone is editing this element"). Not truth: it must **not** go in the log; it piggybacks on the F-collab-sse stream as a separate channel. The "it feels collaborative" delight layer (Figma-style) and a cheap way to cut the F-collab-concurrency conflict surface. Depends on F-collab-sse. |
| F-offline-sync | collab · reconciliation | ☐ | Parked | *(backlog from the F-collab-live design chat 2026-07-03.)* True offline-first reconciliation: today offline ops stash in `localStorage` and are **local-only, not resynced**. The structural blocker is **server-minted ids** (`mint_id` in `serve.rs`, under the appends lock) — an offline `add` can't get a real id. Needs client-generated tentative ids reconciled on reconnect (CRDT/GUID-style), a real design lift. Heaviest std-only piece; open only when offline editing is a felt need. |
| F-compact-live | collab · reconciliation | ☐ | Parked | *(backlog from the F-collab-live design chat 2026-07-03.)* Handle `faceto compact` (history rewrite → snapshot) while clients are **live-subscribed**: a client syncing "events since version N" can have N vanish under compaction. Needs a "your base was compacted → full resync" signal on the F-collab-sse stream. Small but real once SSE ships. Depends on F-collab-sse. |
| F-format-interop | interop | ☐ | Parked | Import/export to known event-storming formats and visual tools (Excalidraw, Miro). Not felt pain today. |
| F-es-vocabulary | modelling fidelity | ☐ | Later | Deeper pure event-storming vocabulary — parallel / recurrent events, out-of-lane elements, and two sticky types a real board reached for: **`timer` / `temporal`** (time-triggered policies) and **`process`** (stateful, longer-running workflows). Each is an additive lane in `LANES` + `colour` + `lane_prefix`. Open when the model can't express something a real session needs. Field feedback #13 §2. **Dogfood 2026-07-04:** the image / UI element type (**F-image-element**, #60) rides this additive-lane seam. |
| F-ddd-process | DDD process | ☐ | Parked | Adjacent capabilities from the ddd-crew starter modelling process. Depends on F-container; open after it lands. |
| F-new-diagrams | new formats | ☐ | Parked | New diagram types: C4, User Story Mapping, BPMN. The long-term PRODUCT.md ambition; deferred until the event-storming board is excellent. |
| F-model-smells | linting | ☐ | Parked | Detect model smells — orphans, loops, heavy bounded-contexts. Needs the F-container primitive and a graph pass; open once grouping exists. |
| F-board-gestures | UI · direct edit | ✅ | ✅ Shipped | Richer on-element gestures layered over F-inline-add: **chromeless** bare ghost glyphs (`+` add · `×` remove · comment), not a floating toolbar (DESIGN §6); single-click focuses only (select-then-edit), double-click / F2 rename, drag left/right moves, `c` / comment glyph opens the modal. The modal then carries only prose actions, and `resolve` shows only on hotspots / open questions. Working note below; shipped 2026-07-01. **Dogfood 2026-07-04 follow-ups:** ~~deselect-on-empty-click bug (#45, P1)~~ ✅ fixed 2026-07-05 — the four sticky handlers now recompute one spotlight owner (cursor › focus › inline-rename) through a shared `relightSpotlight`/pure `spotlightOwnerOf`, so empty-space blur deselects, only one box lights at a time, and a renamed box stays lit mid-edit; show-glyphs-on-select / persistent affordance (#48, P2); UX papercuts sweep (#47, P2). |
| F-region-frontiers | model · grouping | ✅ | ✅ Shipped | *(reshaped by feature-torture 2026-07-03 — frontier core only; shipped 2026-07-03.)* Regions are now a **contiguous partition defined by shared *frontiers***, not independent `[fromCol, toCol]` spans. `model::normalize` — one pure, deterministic, idempotent sweep — projects **any** phase list (new frontier events *and* legacy spans with holes/overlaps) onto a gap-free, overlap-free partition, in **both** `replay` and `from_json`, so every `Model` obeys the invariant. Resize = drag a `.frontier` (`FrontierMoved {id, edge, col}`; normalize re-borders the neighbour atomically), add = **split** a phase (`PhaseSplit`, server-minted right-half id), remove = **merge** (`PhaseRemoved` + normalize absorbs the columns — no hole), the outermost frontiers resize the **whole board**. Kills by construction the hole / overlap / unreachable-edge confusions. **As-built deltas from the shaping** (see working note): `PhaseMerged` **deferred** (YAGNI — no v1 gesture picks merge direction; `PhaseRemoved`+normalize already merges); `FrontierMoved` carries an `edge` discriminator (moves *both* board ends); render draws **one** grabbable frontier per boundary (dedup), not two per-region edges. **Cut from v1 (unchanged):** the pivot / interstice column (co-owned with F-lane-flow (c) / F-floating-hotspots — frontier draws on the column boundary meanwhile) and move-region-as-reorder (→ candidate F-region-reorder). Model-spine change across all five files. Working note below; torture report: `.personal/feature-torture/reports/F-region-frontiers.md`. **Dogfood 2026-07-04:** discoverable phase split + tab-click affordance (#49, P2); tab-rename dblclick parity + remove-misfire → papercuts (#47); widen-board-from-frontier → new **F-frontier-width** (#50). |
| F-region-collapse | UI · legibility | ✅ | ✅ Shipped | *(reshaped by feature-torture 2026-07-03 — ✂️ column-fold only; edge-fold spun out to F-region-edge-fold. Shipped 2026-07-03.)* Collapse / hide a region to concentrate readability: fold its stickies into a summarised band so a wide board gets **shorter**. Pure **view-state** — no model / event change — via the proven `?base=` seam extended to `GET /board.svg?collapse=<id,id>` (client holds the collapsed-set in `localStorage`, server re-lays-out a `col → x` remap). Orthogonal to F-region-frontiers; reads the normalized partition. v1 **drops** the crossing-edge summarisation (the risky endpoint-in-span / adjacent-band correctness surface) → that is **F-region-edge-fold**. Pairs with F-frozen-headers (same wide-board legibility push). Working note below; plan: [`docs/F-region-collapse-plan.md`](docs/F-region-collapse-plan.md); torture report: `.personal/feature-torture/reports/F-region-collapse.md`. |
| F-region-edge-fold | UI · legibility | ☐ | Later | *(spawned by F-region-collapse torture 2026-07-03.)* The deferred v1 tier of F-region-collapse: once a region folds to a band, **reroute the edges that cross it** to the band's two frontiers with a count badge, instead of dropping them. The risky 30% held out of collapse v1 — endpoint-in-span math (which edges cross a `[from_col, to_col]` span in `col` space, before remap) plus adjacent-collapsed-band composition, each needing a pure red-first test. Depends on F-region-collapse; ships with **zero rework** of it. Revisit-trigger: first dogfood session where a hidden crossing-edge causes a misread. |
| F-2d-placement | model · layout | ✅ | ✅ Shipped | Replace the rows / columns / grid **packing** (and its dark grey group box — a poor 2D representation) with a **stored 2D sub-position**: keep `x = col` (global timeline) and `type = lane` — both invariants — but give each element a free **Y within its lane band** instead of auto-packing. Removes the packing control entirely and fixes two dogfood bugs: moving within a stacked group force-**swaps** (can't re-insert without displacing the survivor), and moving from / into a group **superposes**. Model change — `ElementMoved` gains the sub-position. Absorbs feedback #1 / #3 / #4 / #10. Shipped 2026-07-02 (PR #17); as-built note below. **Dogfood 2026-07-04:** magnetic row-snap reshape — kill the 1.5 / 2.5 vertical centering so elements snap to integer lane rows (`svg.rs:208-209` + client `computeGrid` mirror). Tracked #51 (P2). |
| F-lane-flow | UI · legibility | ☐ | Next | Reorder the 8 lanes to the **canonical event-storming flow** (actor → command → aggregate / system → event → policy → … → read-model → UI → actor) so system and policy sit *near* events / commands, not at the bottom. Forks to shape: (a) reorder `LANES`; (b) **merge** adjacent lanes (aggregate+external, readmodel+policy) as an expandable *display grouping* — `type` still selects a pure lane, so the 8-colour grammar invariant holds; (c) alternate event / non-event **column cadence** — recoups the pivot / interstice column of F-region-frontiers, so shape together. Also shares the `LANES` / `colour` / `lane_prefix` seam with **F-floating-hotspots** (removes the hotspot lane) and **F-es-vocabulary** (adds `timer` / `process` lanes) — touch the lane set once, not three times. Feedback #2. Tracked #79. |
| F-floating-hotspots | model · ES fidelity | ☐ | Later | Hotspots become **floating annotations attached to an element** (placed beside it, ES-canonical) rather than a bottom lane — removing `hotspot` from `LANES` (the shared lane-set seam with **F-lane-flow** and **F-es-vocabulary** — sequence together). Split the modal into two direct gestures: **`c` = comment**, **`h` = hotspot / open question**; drop **split** (add / rename / remove already cover it). Feedback #5 / #6. **Dogfood 2026-07-04:** hide-resolved-hotspots view toggle (#59, P3) — a `View` lens on the `?collapse=` seam, no new event. |
| F-frozen-headers | UI · legibility | ☐ | Later | Pin the **lane titles** to the left through horizontal scroll (condensable to initial + colour) and the **phase tabs** to the top through vertical scroll — frozen row / column headers. The board is one scrolling SVG, so this needs an overlay layer (or a split render), not plain `position: sticky`. Feedback #7 / #8. **Dogfood 2026-07-04:** pinning the phase tabs is the reachable-through-scroll half of the split affordance (#49). |
| F-commit-flow | UI · server flow | ☐ | Next | Replace the counterintuitive **Export comments / Reload** header actions with a single **Commit / Save** that re-baselines and clears the since-you-last-looked diff overlay. Framing: event-sourcing has **no server-side uncommitted state** (the log is truth, every edit is already appended), so "commit" = **re-baseline the diff view** (today's "Plain" button, reframed), not a write. The **Export** rethink shares the comments-representation seam with **F-comment-lifecycle** (which collapses the exported-array vs `comments.jsonl` duality) — reconcile the two. Feedback #11. **Dogfood 2026-07-04:** editable **board title** (#57, P2) — spine has `BoardTitled`, no client edit path; the title renders twice (header + SVG nameplate) so an editor must re-fetch the board (the dup-title #9 seam). Tracked #78. |
| F-es-lint | linting | ✅ | ✅ Shipped | **ES-grammar linter** — `faceto lint` over the replayed `Model`, a pure graph pass, zero-dep. Rules validated by a real 147-element workshop (all 6 review comments were mechanical grammar defects): event with no producer, policy with no output, policy with no input, non-terminal event with no outbound edge. **Warn-only** (a big-picture board is legitimately incomplete — never a gate that breaks the calm loop). Distinct from **F-model-smells** (orphans / loops / heavy bounded-contexts) which needs F-container; this one needs only the graph. Field feedback #13 §3 — the headline item. **Shipped in three slices.** *Slice 1* (`src/lint.rs` + `faceto lint SOURCE`, exit 0 always, model.json and log): the four base rules keyed on stable `id`, deterministic order. *Slice 2* — the `level: big-picture \| design` knob: an additive `Model.level` (top-level `"level"` in model.json, a `BoardLeveled` log event mirroring `BoardTitled`), and one design-only rule `command-no-output` (a command that emits no event — legitimate incompleteness at big-picture, a defect once the flow is filled in). Still warn-only at every level. *Slice 3* — findings flow into the serve sidecar: `GET /comments` merges the live `lint()` pass as `kind:"lint"` entries (derived on read, never persisted, so never stale), suppressed once the element is `resolved` — reusing the existing serve→review→resolve path, no new endpoint. The per-finding resolve *gesture* stays **F-comment-lifecycle**'s. Shipped 2026-07-03 (PR #19), reconciled with F-auto-genesis (log-only sidebar merge). |
| F-comment-lifecycle | comment · identity | ☐ | Next | Close the sidecar identity gaps surfaced at scale: deleting an element **orphans** its comments (needs cascade/tombstone in `replay`); resolving a comment needs a **gesture**, not hand-edited JSONL (likely a small serve endpoint + client button over the existing `HotspotResolved` / comments-as-events); collapse the **two comment representations** (exported array vs `comments.jsonl`) toward the log-is-truth spine; ID-rename sidecar migration is **reframed as guardrails/docs** (`id` is defined-stable — "never renumber, only add"), not tooling. The two-representations collapse meets **F-commit-flow**'s Export rethink — same seam. Field feedback #13 §5. Tracked: #21. **Dogfood 2026-07-04:** editable element description (#52, P2) — `detail` is writable via `ElementAnnotated` but the modal shows it read-only and frames the write as a "comment". |
| F-output-naming | CLI · output | ✅ | ✅ Shipped | Derive `<name>.svg` / `<name>.html` names from the **model basename** (`output_stem` in `main.rs`) so sibling boards in one directory don't clobber each other. Small correctness win. Field feedback #13 §1. **Absorbed the two F-auto-genesis review carry-overs (PR #23):** (carry-over a) the served log name was hard-coded `event-log.jsonl` regardless of source basename, so `faceto serve b.json` beside a log genesis'd from `a.json` silently served the *wrong* board — **fixed by construction**: `log_beside` now derives `<name>.event-log.jsonl`, and a model + its log share one `output_stem` so `render` of either writes the same board; (carry-over b) `serve`/`genesis` accept any JSON as a model (`model::from_json` is lenient) and a mis-suffixed `model.jsonl` replays to an empty board — **added `warn_if_empty`**, a warn-only nudge when a source yields 0 elements. Log-name change migrated the tracked `examples/event-log.jsonl` → `examples/sample.event-log.jsonl`. Shipped 2026-07-05 (PR #75). Tracked #15. |
| F-cli-help | CLI · ergonomics | ☐ | Next | `--help` / `-h` per subcommand (`faceto render --help` currently treats `--help` as a file path); **plus `faceto <file>` defaulting to `serve`** — the primary action — while `render` / `genesis` / `compact` stay explicit. Small CLI-dispatch ergonomics in `main.rs`. Field feedback #13 §1 + dogfood #12 (reconciled). Tracked #16. |
| F-png-docs | docs | ☐ | Later | **Document** the sanctioned SVG→PNG paths (`rsvg-convert` / `resvg` / headless Chromium) rather than build a rasterizer — PNG encoding + font rasterization is not feasible in pure std, so raster export stays a **deliberate non-goal** under the zero-dep constraint. A good idea, kept out of the binary. Field feedback #13 §1. |
| F-status-tracking | model · fidelity | ☐ | Later | Optional as-is / to-be **status field** on `Element` (additive) for mixed implemented/target boards, rendered as a visual state (e.g. dashed = target). Field feedback #13 §2. |
| F-typed-edges | model · fidelity | ☐ | Later | Give edges an optional **`type` / label** so connection kinds stop rendering identically. Additive; shape **with F-edge-routing** (which owns edge geometry) to avoid touching `edge_path` twice. Field feedback #13 §2. |
| F-tech-names | model · fidelity | ☐ | Parked | Optional **technical-name layer** distinct from the human label — before building, confirm it isn't `id` misuse. Field feedback #13 §2. |
| F-model-export | CLI · source format | ☐ | Next | **Consolidate to one truth + one source.** (1) **Cut** the legacy `comments.jsonl` import: remove `from_comments` + the genesis sibling-fold + tests (any stray inbox is handled outside faceto). **Keep `comment_to_events`** — the live `POST /comment` path translates one posted comment through it. (2) **Reframe `model.json` as the *source / authoring* format** — kill the "legacy `model.json`" wording in code comments (`events/log.rs`) and docs; it is the human/LLM-authored input, not deprecated. (3) New **`export`** verb (`event-log.jsonl` → `model.json`), the inverse of `genesis`/`from_model`; needs a `Model → model.json` serializer (only SVG/HTML come out today) plus a genesis→export round-trip test. Name chosen over "freeze" (implies immutable — the output re-enters the edit loop). (4) Client header: replace the (log-mode no-op) **Export comments** with one **`Export ▾`** menu → **History** (raw log) + **Model** (current state → `model.json`), preserving an offline escape for localStorage-queued gestures. Reconciles with **F-commit-flow** and **F-comment-lifecycle** (same comment-representation seam). **On implementation, update the README**: flip the lifecycle diagram's `export` arrow from *planned* to shipped and revise the source-format prose. Decided 2026-07-04; slice (1) — the `comments.jsonl` cut — in progress. **Dogfood 2026-07-04:** the **History (raw log)** menu item shares its surface with **F-side-panel**'s history tab (#55) — likely one read-only `GET /events` route. Tracked #77. |
| F-dep-policy | build · CI | ✅ | ✅ Shipped | **Reshape "zero deps, ever" → zero *runtime* deps + a size budget.** *(Shipped: unblocked the proptest PBTs.)* The old rule counted every `Cargo.lock` package — wrong scope: it forbade test-only crates and measured *count*, not the real fear, *size*. New policy: **core is pure-std at runtime** — CI enforces the **normal** graph only (`cargo tree -e normal --prefix none \| sort -u` → just `faceto`); **`[dev-dependencies]` are free** (unblocked `proptest`, never built by `cargo install`); a **binary-size budget** job guards growth (anchor: release binary **~905K** → ceiling 2 MiB, tunable). **Local-*first*, not local-only** — collaboration stays allowed. Reworded `Cargo.toml` / `CLAUDE.md` / `AGENTS.md` / `CODING_STANDARDS.md` / README + the `runtime deps: 0` badge, and swapped the CI `zero dependencies` grep-of-`Cargo.lock` for the `cargo tree -e normal` check. Parked seam: a future `serve`/remote/collab layer may split into a pluggable tool with its own looser rules. Brainstorm: `brainstorm/20260704-zero-dep-constraint-reshape.md`. |
| F-edge-connect | model · edges | ☐ | Next | *(dogfood 2026-07-04.)* Connect / disconnect edges between **existing** elements — today edges only enter via the bootstrap `model.json`, so a live board can't be wired. The spine already supports it (`EdgeAdded` / `EdgeRemoved` — replay, codec, cascade-on-remove all present); the gap is a `connect` / `disconnect` mapping in `comment_to_events` + a client gesture (drag from a border dot when selected, or an action glyph). The C7 "connect / disconnect edge" concept. Highest-leverage gap for a single-player instrument. **P1** · Tracked #53. |
| F-focus-graph | UI · legibility | ☐ | Next | *(dogfood 2026-07-04.)* Neighbourhood spotlight: from a selected element, reveal its whole connected sub-graph (direct **and** indirect), with two switchable modes — **blur** others or **hide** others (elements / edges / phases). High feasibility on shipped infra: `buildGraph` already builds `adj` from the DOM and `.dim` / `.adj` CSS already blurs 1-hop — extend to a **BFS over `adj`** + an `isolate` hide class + a toggle. Client-only. **P2** · Tracked #54. |
| F-deep-links | UI · sharing | ☐ | Next | *(dogfood 2026-07-04.)* A shareable link per **element / lane / phase**. Client-only: stable ids are already on the DOM (`.sticky` id, region `data-region`, `.lane-label`) and focus-by-id survives board swaps — add a `location.hash` handler that focuses (reusing the spotlight path) + `scrollIntoView` on load. No server change (`?base=` / `?collapse=` are the query precedents). **P2** · Tracked #56. |
| F-side-panel | UI · navigation | ☐ | Later | *(dogfood 2026-07-04.)* A right nav pane with three tabs: **history** (all events, limit / lazy-load), **comments / hotspots**, and a **glossary** for ubiquitous-language building. Reuses the `/comments` fold + client `comments[]` keyed by stable id; needs a read-only `GET /events` route (tail-N = lazy load) that overlaps **F-model-export** (History / raw log) and **F-collab-sse** (planned `GET /events`). Comments tab overlaps **F-comment-lifecycle**. Glossary = new persistence — **P3** sub-item. **P2** · Tracked #55. |
| F-frontier-width | model · grouping | ☐ | Later | *(dogfood 2026-07-04; family of F-region-frontiers.)* Widen the board **from a frontier** instead of only by moving the rightmost element. Two non-exclusive gestures: **(b)** shift-drag moves the frontier and steals a column from the neighbour — essentially what `FrontierMoved` + `normalize` already do for phase borders, a cheap client rebind (P2); **(a)** plain-drag extends with **empty** columns, pushing everything right — new global column-insert model work (`~ColumnsInserted{at, n}` + replay + normalize), **P3** stretch. **P2** · Tracked #50. |
| F-edge-ports | model · fidelity | ☐ | Later | *(dogfood 2026-07-04.)* Manual edge **anchor / port** control — move where an edge meets a box border to uncross or improve routing; moving the element resets it. Today anchoring is **auto-only** (`fan_offsets`, computed transiently per render; `Edge` stores no port). New primitive: a stored port / offset on `Edge` + an event to set it + a drag gesture. Shape **with F-edge-routing** (owns `edge_path`) and **F-typed-edges** (also touches `Edge`) — touch the edge model once. **P3** · Tracked #58. |
| F-edge-remove | UI · edges | ☐ | Later | *(spawned from F-edge-connect, 2026-07-05.)* **Click a connector line and cut it** — a direct, direction-agnostic delete complementing the directed disconnect shipped in #53 (today you must drag the connect dot *from the source*). **Server already done** — reuse the `disconnect` → `EdgeRemoved` mapping (`edge_comment_to_events`); the gesture reads the edge's `data-src`/`data-dst` and posts `{kind:"disconnect", src, dst}`. **Client-only, two parts:** a transparent wide **hit-target** path per edge (2.4px stroke is a poor click target), and a hover→highlight→two-step-confirm gesture (mirror `doRemove` in `src/client/edit.js`). Build on #53's `src/client/connect.js`; shape the edge hit-target surface once with **F-edge-ports** (#58). Sequence after #86 merges. **P2** · Tracked #88. |
| F-edge-reconnect | UI · edges | ☐ | Later | *(spawned from F-edge-connect, 2026-07-05.)* **Rewire an endpoint** — grab an existing edge's src or dst handle and drag it onto a different box. **No new event type**: re-pointing is `EdgeRemoved` + `EdgeAdded`, both present. Preferred mapping: one additive `reconnect` kind in `comment_to_events` reading `src`/`oldDst`/`newDst` that emits **both** events in one appended block (atomic under the lock, reads as one intent). The real work is **client**: endpoint drag handles on a selected edge, reusing `connect.js`'s pointer-capture drag + live preview + `elementFromPoint`. Needs the same edge **hit-targets** as **F-edge-remove** (#88); keep distinct from **F-edge-ports** (#58) (that moves the anchor on the *same* box). Sequence after #86 and ideally after #88. **P3** · Tracked #89. |
| F-image-element | model · ES fidelity | ☐ | Parked | *(dogfood 2026-07-04.)* A **UI / image element** available in any lane (beside `actor`) holding an imported, zoomable image (shows a reduced version). The **type** is easy — an additive 9th lane on the **F-es-vocabulary** seam (`LANES` / `colour` / `lane_prefix`). The blocker is **persistence**: image **bytes** in the log break the size budget + log-is-truth (whole log re-replayed per request; `MAX_BODY` 1 MiB), and no std image decoder exists for a server-side reduced version under zero-dep — so store an image **reference / URL** in `detail`, not bytes (link-rot risk). Decide before building. **P3** · Tracked #60. |
| F-rich-notes | model · fidelity | ☐ | Parked | *(dogfood 2026-07-04.)* Long, **rich-format** notes on elements for richer descriptions when the workshop enables it. `Element.detail` exists but has no post-add setter event — add an additive `ElementNoted {id, note}` (or a `detail` setter). Rich markup in an SVG board is awkward → likely an HTML overlay, dovetailing with **F-side-panel**. Same persistence question as **F-image-element**; overlaps **F-comment-lifecycle**. **P3** · Tracked #61. |
| F-js-modules | UI · tech-health | ✅ | ✅ Shipped | *(feature-torture triage 2026-07-05 — 👍 ship, enabler.)* Split the monolithic `src/template.html` client (embedded via `include_str!` in `render.rs`) into cohesive JS modules and add **JS-level tests** — today only Rust is tested, the client has none. Pays down the blob **before** the power-gesture / command-box / theme work piles onto it. Must keep the `__SVG__` / `__TITLE__` / `__CONFIG__` substitution contract and zero-runtime-dep in the shipped binary (concatenate / inline at build — no bundler ships). Seam cluster **A (client architecture)**; unblocks `F-duplicate`, `F-command-box`, `F-theme`. **P1** · Tracked #67. **Shipped 2026-07-05:** `template.html` reduced to a thin shell; CSS → `src/client/style.css`, the ~1.3k-line script → 8 modules (`core`/`layout`/`drag`/`edit`/`region`/`sync`/`graph`/`main`) `concat!(include_str!…)`'d back into one `<script>` at build in `render.rs` (two-stage `__CONFIG__`→script→shell fill); byte-identical render verified; JS test coverage 2→7 pure helpers (28 checks). |
| F-duplicate | UI · direct edit | ☐ | Next | *(feature-torture triage 2026-07-05 — 👍 ship.)* Duplicate an element with **Miro-parity** shortcuts — **Mod+D** / **Mod+→** duplicates to the right, **Mod+dir** duplicates in that direction. Cheap: reuses the shipped `add` event + server-side `mint_id`; client-only gesture layered on **F-board-gestures**. On-thesis single-player muscle memory. Seam cluster **B (power gestures)**. **P1** · Tracked #68. |
| F-command-box | UI · direct edit | ☐ | Next | *(feature-torture triage 2026-07-05 — 👍 ship.)* Keyboard **command box / palette** (nerd, no-mouse): type `/add event "SlashTriggered"`, `/move E3` then arrows, etc.; the trigger char is configurable. **No MCP, no new Rust** — parses to the existing `POST /comment` payloads (add/move/rename/remove/resolve) the shipped engine already accepts. The command-line sibling of **F-board-gestures**; the chat+MCP superset is the parked **F-inboard-chat**. Seam cluster **B/C**. **P2** · Tracked #69. |
| F-mermaid-export | interop · export | ✅ | ✅ Shipped | *(feature-torture triage 2026-07-05 — 👍 ship.)* Export the board to **Mermaid** `flowchart LR` text with an explicit **degradation warning** (lanes / timeline columns / regions / diff overlay / y-placement don't survive — a `%%` comment header in the output plus a stderr notice). Pure `Model → String` in `src/render/mermaid.rs`, zero-dep. **Introduced the `export` verb** the spec expected to ride (F-model-export hadn't shipped it): `faceto export [SOURCE] [--format mermaid]` prints to **stdout** (pipeable), with a `Format` enum + `--format` dispatch shaped so F-model-export's `model` and F-narrative-export's `narrative` slot in. Each of the 8 types maps to a distinct Mermaid shape, and the **colour grammar is preserved** via per-type `classDef` (sourced from `style::colour`), so colour — unlike the torture note — *does* survive. Docks onto the **F-new-diagrams** family as its first concrete exporter. Seam cluster **E (export/interop)**. **P2** · Tracked #70. |
| F-theme | UI · theming | ☐ | Next | *(feature-torture triage 2026-07-05 — 🧬 split.)* Core **light/dark + token** theme switch, staying inside the calm-instrument register (`DESIGN.md` — no SaaS chrome, no maximalism). Ships as CSS custom properties over the 8-lane `colour` grammar + a toggle persisted in `localStorage`. Arbitrary / user-authored themes are **deferred to F-plugins** (its first client). Seam cluster **D (theming)**. **P3** · Tracked #71. |
| F-multi-select | UI · direct edit | ☐ | Later | *(feature-torture triage 2026-07-05 — 🧬 split.)* **Selection model** (shift-click / marquee multi-select) first, then **bulk actions** (move / delete / duplicate the set). The selection brick is the prerequisite; bulk-duplicate composes with **F-duplicate**. Client-only on **F-board-gestures**; each bulk op still appends N discrete events through the H4 mutex. Seam cluster **B**. **P2** · Tracked #72. |
| F-version-tags | history · versioning | ☐ | Later | *(feature-torture triage 2026-07-05 — 🧬 split.)* Light git-like versioning. **Tag** = cheap, log-native named offset (mark "version N" at a log length / content hash) — shippable. **Branches / switch / cross-branch diff** = **delegate to real git** on the tracked `event-log.jsonl` rather than rebuilding a VCS inside the log (user's call — if faceto ever handles branches for diff/switch, git is the better substrate). Docks onto **F-commit-flow**. Seam cluster **G (version/history)**. **P3** · Tracked #73. |
| F-narrative-export | interop · export | ☐ | Later | *(feature-torture triage 2026-07-05 — 🧬 split; reshaped from "narrative skill".)* **Narrative event-storming export**: walk the event **timeline** and emit a user/system journey as prose. Cheap primary path = **templated sentences** over the ordered events, gated by **event-cluster selection as a prerequisite** (needs a way to group events into a journey — the real design cost). Rides the `export` verb, zero-dep Rust. A richer **LLM-skill** variant (free-form narration) is the deferred child. Output-side sibling of **F-narrative-skill** (which is *input*: narrative→events). Seam cluster **E**. **P3** · Tracked #74. |
| F-inboard-chat | AI · interop | ☐ | Parked | *(feature-torture triage 2026-07-05 — 🧬 split → park.)* In-board (and TUI) **chat box** that drives the board via **MCP** tool calls. The keyboard/command core ships separately as **F-command-box** (no MCP); this superset **reopens F-mcp-server** (Parked as redundant while the agent has file+shell tools). Revisit when a shell-less client makes MCP the real transport. Seam cluster **C**. **Parked** · issue TBD (gh auth down). |
| F-board-import | interop · ingestion | ☐ | Parked | *(feature-torture triage 2026-07-05 — 🧬 split + 🏠 sibling.)* Reconstruct a board from an external source: **(a)** scan an **image** of a real / virtual board (vision/LLM) and **(b)** a **Miro connector** (API-scan stickies). Both need network / vision → **cannot live in the zero-runtime-dep core** → **sibling tool / skill** that emits a `model.json` / event log the core ingests. Extends **F-format-interop**. Seam cluster **F (import/ingestion)**. **Parked** · issue TBD (gh auth down). |
| F-tui | UI · alt-frontend | ☐ | Parked | *(feature-torture triage 2026-07-05 — ⏸ park + 🏠 sibling.)* A **terminal** visual board. Reuses the core `replay → Model` as a **library**; a large alt-frontend surface, off the single-player-*web*-board thesis while the web loop still has open items. Likely a sibling binary/crate depending on faceto-core. Seam cluster **H (alt frontends)**. **Parked** · issue TBD (gh auth down). |
| F-plugins | architecture · extensibility | ☐ | Parked | *(feature-torture triage 2026-07-05 — ⏸ park.)* A **plugin** system — **themes** and **workshop models** (formats) as the first test clients; core themes/formats stay **built-in, not plugins**. Big architectural bet with **zero-runtime-dep tension** (dynamic loading vs a pure-std binary). YAGNI until ≥2 concrete plugins exist. Eventual home for custom **F-theme** themes and the **F-c4-xref** format. Seam cluster **I (extensibility)**. **Parked** · issue TBD (gh auth down). |
| F-multi-board | project · structure | ☐ | Parked | *(feature-torture triage 2026-07-05 — ⏸ park.)* **Multi-board projects** — group several boards for a complex system so they can be worked, discussed, and shared together. Needs a project container + board index + cross-board nav; the **substrate for F-c4-xref**. Park until the single board is excellent (same call as **F-new-diagrams**). Seam cluster **J (references/multi-scope)**. **Parked** · issue TBD (gh auth down). |
| F-c4-xref | interop · cross-format | ☐ | Parked | *(feature-torture triage 2026-07-05 — 🤷 defer-decision.)* **Cross-workshop references** — link ES events / systems to **C4** classes / systems. Blocked on **both** **F-new-diagrams** (C4 must exist first) **and** **F-multi-board** (the substrate). Same cross-reference primitive as **F-code-binding**. Seam cluster **J**. **Parked** · issue TBD (gh auth down). |
| F-code-binding | model · linking | ☐ | Parked | *(feature-torture triage 2026-07-05 — 🤷 defer-decision, needs its own torture.)* **Code ↔ board binding**: links between code and board elements to follow modifications on both sides. Under-specified — reference by `file:line`? symbol? one-way vs bidirectional sync? Powerful (kin to **F-c4-xref**) but needs a dedicated feature-torture before scoping. Same cross-reference seam as **F-c4-xref**. Seam cluster **J**. **Parked** · issue TBD (gh auth down). |

## Working note — F-inline-edit (2026-06-20, branch `feat/F-inline-edit`)

**Root cause / scope.** Editing is *modal-only*: every rename/remove routes through the
comment dropdown. Move is already direct (← / →, Move ←/→). So this slice adds **direct
rename + direct remove** gestures and demotes the modal to "optional, not the only path".
Wiring a direct rename surfaces a latent defect: the `rename` arm of `comment_to_events`
(and `replay`) accepts a **blank label**, persisting a never-renumbered empty box — the exact
failure the `add` path already guards. Select-all → delete → Enter would trip it in one gesture.
The fix keeps the *non-blank-label* invariant in the Rust domain seam (not only in JS).

**Tests to done** (red first, then green):

- UT: `rename` rejects a blank/whitespace label (→ nothing to persist); trims surrounding space.
- PBT (std-only, hand-rolled): over random comment sequences, no element ever ends with a
  blank label via the comment seam; move/annotate preserve element cardinality & identity.
- Integration: a blank rename appends nothing to the log; a real one persists one `ElementRenamed`.
- Non-regression: move/swap, server-side mint, and `add`'s blank-guard stay green.

## Working note — F-inline-add (2026-06-21, paired)

**Scope (hardened, ratified).** `add` already works end-to-end through the comment modal's
dropdown: `serve.rs` `append_add` + server-side `mint_id` + the non-blank-label guard, all
tested. This slice makes add a **direct on-board gesture** and strips the modal's `add` option.
**Lane-only, client-only** — domains / bounded-contexts are explicitly *not* in scope (that is
**F-container**, which stays parked at Later; F-inline-add must not touch the model spine).

**Gesture (ratified, two affordances — `+left` dropped).**

- *Add after:* hover an element → a `+` appears on its **right** edge → mints in the **same lane**
  (`type`) at `anchorCol + 1`. This is byte-identical to today's modal `add` payload, so the whole
  server path is already written and tested.
- *Prepend / first element / empty board:* hover a **lane title** → a `+` → mints at the **left of
  that lane** via `model::lane_left_col(model, kind)`: a **first element of an empty lane** aligns
  to the board's existing first column (no rightward shift of the other lanes); a **prepend into a
  non-empty lane** marches one column further left (the renderer draws negative/sparse `col`
  on-board). Because the lane title is always present (see the render change below), this one
  affordance covers prepend-into-a-lane, the first element of a lane, **and** the empty-board
  bootstrap the modal cannot reach. (The non-empty-lane prepend feel is deferred for later.)
- *Modal:* remove the `add` option (and its now-dead `<select id="m-type">` lane picker). Modal
  stays prose-only — comment, hotspot resolve, rename, open question. No reshape.

**Render change (R, deliberate, accepted).** `render_svg_packed` currently builds `present` by
filtering `LANES` to lanes that *have* an element, so empty lanes — and the whole empty board —
draw no row or title. R makes all 8 lanes always render, so an empty board shows the lane scaffold
(onboarding for an event-storming beginner) and every lane title is hoverable. This is the one
non-client change in the slice. Regression surface: absolute-y render tests on sparse models
(e.g. `a_lone_sticky_stays_on_the_lane_mid_line`) shift and must be re-pinned; the dead lane-picker
test (`the_add_element_picker_offers_every_lane`) is removed with the `<select>`.

**`col` wrinkle — resolved by design, not by code.** Dropping `+left` removes the file-order
tie-break problem entirely; prepend uses a strict lane-minimum − 1, which sorts left unambiguously.

**Out of scope / parked:** F-container (domains) and the F-board-gestures future set (hover
tool-buttons, click-centre rename, drag-to-move).

## Working note — F-edge-routing (2026-06-27, branch `feat/F-edge-routing`)

**The locked-node constraint (the whole shape of this slice).** Node positions are *not*
free here: `col` is the global timeline (x) and `type` is the lane (y) — both are domain
invariants we must not break. So the textbook crossing-reducer (permute node positions) is
off the table. The only genuinely free levers inside `render.rs` are **(a) the order of
*simultaneous* stickies within a single `(lane, col)` cell** (`sub_ord`, today just file
order) and **(b) how edges anchor and route between fixed centres** (`edge_path`). This slice
spends both, and touches nothing in the event/model spine.

**Lever A — barycenter within-cell ordering.** For each crowded cell, sort its members by the
mean position of their edge neighbours, then assign `sub_ord` from that order (stable, file-order
tiebreak). Because a neighbour's *lane* is fixed, its vertical band is essentially fixed, so the
barycenter is computable in a **single deterministic pass** — no Sugiyama iteration, no clocks,
no randomness. Rows packing sorts sub-rows by neighbour **lane index**; Columns packing sorts
sub-columns by neighbour **col**. A lone sticky in a cell is unaffected (its classic mid-lane spot
holds). This is the part that removes *topological* crossings.

**Lever B — fan-out anchoring.** When several edges meet a box on the same side, they all anchor
at the box centre today and read as one fat bundle (e.g. `X1`→`C1` and `X1`→`C2` in the sample).
Generalise `edge_path` to take a small per-edge anchor offset so siblings spread along the facing
side. This is legibility polish (reduces visual *overlap*, not crossings), kept **subtle** per the
calm-instrument register in DESIGN.md — a few px of spread, never a starburst.

**Hard sync constraint (R).** `src/template.html` carries a JS port of `edge_path` (`edgePath`,
~line 211) used for the in-page move-nudge. Any change to the `edge_path` *signature/geometry* must
be mirrored there or the client nudge diverges from the authoritative server render. (In log mode
the server re-render lands moments later and corrects it; in legacy `model.json` mode the nudge is
the only feedback, so the ports must match.) Lever A changes only `sub_ord`/centres, which the
client already reads from the DOM — no JS change. Lever B changes `edge_path`'s signature — **must**
update `edgePath` too.

**Out of scope (deliberately).** Obstacle-avoidance routing (bowing a cross-lane edge around an
intervening sticky) is *not* in this slice: it needs every box's geometry on both server and client,
risks a busy non-calm look, and has a poor effort/payoff ratio. Park it; reopen only if dogfood
shows cross-lane edges genuinely getting lost under boxes.

**Known regression (accepted, not fixed).** Dogfooding this branch surfaced that the header
Rows / Columns / Grid packing buttons no longer switch the board. Cause unconfirmed — the server
renders each packing correctly (`packing_chooses_its_growth_axis` is green) and the client re-render
path rebuilds its position maps (`renderPack → bindStickies → readLayout`), so code inspection
didn't pin it on this slice. Left unfixed on purpose: packing is likely to be replaced soon by a
thin-positioning model, so investing in the three-mode control now would be wasted. Revisit only if
packing survives that change.

**Tests to done** (red first, then green):

- UT (Lever A): a two-member cell whose neighbours sit in opposite lane-bands orders so the
  upper-neighbour member takes the upper sub-row; a lone sticky keeps its mid-lane centre
  (re-pin / preserve `a_lone_sticky_stays_on_the_lane_mid_line`).
- UT (Lever A): ordering is deterministic and stable — equal barycenters fall back to file order.
- UT (Lever B): `edge_path` with offset 0 is byte-identical to today's path (no-regression on the
  common single-edge case); a non-zero offset shifts the anchor along the facing side only.
- Non-regression: absolute-y render tests on sparse models re-pinned; `diff` styling, hotspot
  dotted connector, and the JS `edgePath` port stay in lockstep (manual board check).

## Working note — F-board-gestures (2026-07-01, branch `feat/F-board-gestures`)

**Scope.** Close the "Now" slice by making the box itself the edit surface, layered on the existing
F-inline-add / F-inline-edit / region-resize gesture engine. Client-only — every event
(`move` / `drop` / `rename` / `comment` / `resolve`) already exists end to end, so `render.rs` and
the server are untouched; the whole slice is `src/template.html`. No new Rust behaviour, hence no
new Rust tests — the gate stays green and the gestures are hand-verified on a live `serve` (the
F-container Stage 6 pattern).

**Two forks ratified before building.**

- **Affordance style (D1).** The roadmap line said "hover opens a small tool-button set," but
  `DESIGN.md §6` forbids floating toolbars. Chosen: **chromeless — individual bare ghost glyphs**
  (`+` add on the right edge, `×` remove top-right, a speech-bubble comment top-left), never a
  button row. DESIGN wins over the literal wording; the glyphs stay in the live-pen accent, no chrome
  at rest, and hide together when a drag starts.
- **Single-click (D2).** Chosen **focus / spotlight only** (select-then-edit, the calm gesture);
  comment relocates off the click to the **`c`** key + the comment glyph — the user's redirect,
  cleaner than either option first offered. So the click is benign, which let the disambiguation
  timer and the drag's `suppressClick` guard both go away.

**Gesture map (the contract).** single-click → focus · double-click / **F2** → rename in place ·
drag left/right (or ← / →) → move along the lane · **`c`** / comment glyph → the prose modal ·
**`×`** glyph / Delete → remove · `+` → add. The modal is now **prose-only**
(comment / split / open question / resolve); `resolve` is gated in `openModal` to a hotspot or an
element carrying an open `question`.

**As built.** `moveTo(id, targetCol)` was extracted from `doMove` first (tidy) so the arrow nudge
and the new drag share one move contract. Drag reuses the region-resize pattern — Pointer Events +
`setPointerCapture`, a 4px threshold below which a press is just a click, snap-to-column via the
rendered centres, and an occupied same-lane target swaps (both `ElementMoved` lines confirmed on the
wire). A small `graceGlyph` helper factors the fade / grace-travel / stashed-target plumbing so `×`
and comment are one line of wiring each. Hand-verification surfaced one fix — the comment glyph moved
from the left-centre edge (where it landed on incoming arrowheads) to the top-left corner.

**Out of scope.** Lane change via vertical drag (breaks `type` = lane); server-side enforcement of
the resolve-gating (a UI concern); any new event kind or model-spine change.

## Working note — F-region-frontiers (2026-07-02, design surfaced by dogfood)

**Root cause.** F-container shipped regions as **independent `[fromCol, toCol]` spans**. Dogfooding
the CISAC model surfaced the confusions that model allows: dragging one region's edge past a
neighbour's opens a **hole** or an **overlap**, and on overlap the underneath edge becomes
**unreachable** to grab. Minor as method, but very disorienting.

**The real fork (name it before coding).** The confusion is a symptom of an unnamed question — *what
is a region?*

- **Phase** (pivotal-event model): a **contiguous partition of the timeline**; holes / overlaps are
  impossible by construction. A *pivotal event is literally the frontier between two phases* — so a
  region boundary and a pivotal event are the same object.
- **Bounded context**: a semantic grouping where overlap is legitimate (two contexts can share
  stickies).

The dogfood instinct — "a region should always be present, holes shouldn't exist" — plus the pivotal
= frontier identity point to the **Phase** reading. Chosen: **Option A — contiguous partition,
frontier-based.** If overlapping bounded contexts are ever needed, that is a *second primitive*, not
a bent phase model.

**The unifying primitive — the frontier.** Defining a region by its shared frontiers (not
independent spans) collapses four gestures into one:

- **resize** = move a frontier → the two neighbouring phases re-border atomically (like a
  table-column boundary);
- **add** = *split* a phase at a column (the `+` glyphs left / right of a frontier in the interstice);
- **remove** = *merge* two phases (delete the frontier between them);
- **board ends** = the outermost frontiers have only one neighbour, so dragging them **grows /
  shrinks the whole board** (fixes "can't resize at the extremes").

**The pivot / interstice column.** To keep "one element per column", a frontier gets its **own
dedicated column** between the element columns. That interstice hosts, in one place: the frontier
itself, the region-operation glyphs, and — canonically — the **pivotal event** that marks the phase
boundary (materialises F-container's "derived pivotal"). Under A the frontier runs through the middle
of that column.

**The one geste that gets harder.** In a contiguous partition, **move-region = reorder** (this phase
now happens before / after that one), and it should **carry its content** — the stickies whose `col`
falls inside the region *at move time* (membership = spatial containment; regions never *own*
elements, they are `col` ranges). A compound operation, unlike the simple delta a span-move would be.

**Separable, do not bundle.**

- **F-region-collapse** is pure view-state (fold a region's stickies; crossing-edge summarisation
  spun out to **F-region-edge-fold** by the 2026-07-03 torture); it rides on top of whichever
  border model and belongs to its own slice.
- **Legacy-mode guard (superseded 2026-07-03 → F-auto-genesis).** Region structural ops only apply
  in **log mode**; in legacy `model.json` mode `POST /comment` stores them as dead comments yet the
  gesture still reports success ("region resized"). The gesture *lies*. The guard idea (client
  learns the mode, refuses / warns) is superseded by the stronger call: **kill the legacy serve
  mode** — `serve` auto-runs genesis on a `model.json` and always operates on the log, so the lying
  state is unrepresentable. See the F-auto-genesis row (Now).

**Architecture note.** This is a **model-spine change** — `events.rs` needs frontier semantics
(evolved additively; a frontier move re-borders two phases atomically), `render.rs` needs the
interstice-column layout, and the client gestures rebind to frontiers. Not a template patch: shape it
(`/impeccable shape` or `feature-torture F-region-frontiers`) before any code.

**Shaped (feature-torture, 2026-07-03) — verdict ✂️, frontier core only.** v1 keeps the partition
semantics and drops two bundles from this note: the **interstice column** waits for joint shaping
with F-lane-flow (c) / F-floating-hotspots (until then the frontier draws on the column boundary —
the pointer-capture edge drag is already proven), and **move-region-as-reorder** is deferred until a
real session needs it. Top open question: the deterministic normalization rule `replay` applies to
legacy span logs with holes / overlaps. Full ADR + spec stub:
`.personal/feature-torture/reports/F-region-frontiers.md`.

**As built (2026-07-03, branch `feat/F-region-frontiers`).** Shipped across all five files; 5 stages,
gate green. Three decisions departed from the shaping spec — each surfaced for review, none reversing
the thesis:

- **The normalization rule (the top open question), resolved.** `model::normalize` is one left→right
  sweep: sort phases by `(from_col, to_col, id)`; anchor the board-left bound at the first phase's
  `from_col`; then start each phase where the previous ended (+1) and keep its own `to_col` as the
  right edge (clamped ≥1 col). Pure, deterministic, **idempotent** (a partition is its fixed point).
  A clean partition renders byte-identically; a legacy overlap/hole resolves to a defined partition
  (named diff, the accepted cost). Proven by an **800-seed property test** — random interleavings of
  `PhaseAdded`/`PhaseResized`/`FrontierMoved`/`PhaseSplit`/`PhaseRemoved` never replay to a hole or
  overlap. Placed in `model.rs` (domain-rules home) and called by **both** `replay` (log) and
  `from_json` (bootstrap `model.json`), so **every `Model` is a partition** whatever the source — a
  small scope extension beyond "replay normalizes", needed so render can trust a partition. The old
  `region_of` "innermost-on-overlap" test became a "normalizes-overlap-into-a-partition" test:
  overlaps are unrepresentable now.
- **`PhaseMerged` deferred, not shipped.** Under the partition, `PhaseRemoved` + normalize already
  merges (the neighbour absorbs the freed columns, no hole). A distinct `PhaseMerged` only earns its
  keep with a gesture that picks merge **direction** / surviving label — and v1's remove (tab ×/
  Delete) has none. So the two new kinds are **`FrontierMoved` + `PhaseSplit`**; `PhaseMerged` waits
  with the deferred `F-region-reorder`. (YAGNI over the spec's "three kinds"; reversible.)
- **`FrontierMoved { id, edge, col }` carries an `edge` (`"start"`/`"end"`).** The sweep anchors the
  board-left bound and honours each `to_col`, so an internal frontier and the right board edge post
  a left phase's `"end"`; only the leftmost frontier posts the first phase's `"start"`. It's the
  clean way to move *both* board ends and is plainly not `PhaseResized` (which set both borders at
  once — the span model we killed).

**Render / client contract (interstice still cut).** Frontiers draw **on the column boundary**, one
grabbable `<line class="frontier" data-region data-edge data-col>` per boundary (internal + two
board ends) — the doubled/overlapping per-region edges are gone. Client rebind: drag a frontier →
`frontier-move` (snaps to a column boundary via `boundaryAtSvgX`, reaching the right board edge a
column-snap can't); **add = split** — hover a region's open band → a `+` at the hovered column →
`phase-split` (server mints the right-half id); **create first phase** — on an empty board only, the
rail's `+` makes one full-width phase; **remove = merge** — tab ×/Delete, unchanged `region-remove`.
Split's discoverability (hover-band `+`) is the interstice's stand-in; revisit when the interstice
lands with F-lane-flow (c). Live browser gesture testing was blocked (extension offline); verified via
the server round-trip (all four gestures) + JS syntax + the Rust suite — **dogfood the drags** to
confirm feel.

## Working note — dogfood batch (2026-07-02): layout, lanes, hotspots, headers, commit

A second dogfood pass on the CISAC model produced twelve retours; they cluster into five slices
(above) plus two quick fixes. Recorded here so the reasoning and the invariant tensions survive.

**The through-line — three slices converge on one column.** The event / non-event **column cadence**
(F-lane-flow option c), the **pivot / interstice column** of F-region-frontiers, and **floating
hotspots** beside their element all want to place non-element material *between* the element columns.
Shape them together or they will repeatedly rework the same layout seam.

**F-2d-placement — the invariant guard.** "True 2D" must **not** become free-float: `col` is the
global timeline (x) and `type` is the lane (y) — domain invariants. The target is *stored Y within
the lane band* replacing *derived packing*, not position-anywhere. Keeping that line is what lets the
grey group box and the move-swap / superpose bugs go away without breaking the diff join (still keyed
on `id`) or the timeline.

**F-lane-flow — merge without breaking the grammar.** A merged lane (aggregate+external,
readmodel+policy) is a **display grouping**, not a new `type`: an element's `type` still resolves to
one of the eight pure lanes and keeps its colour; the merge only stacks two bands into one row that
can expand back. That preserves the "type selects the lane and colour" invariant while giving the
denser default the user wants.

**F-commit-flow — there is nothing to "save".** The event log is append-only truth; every gesture is
already persisted server-side the instant it posts. So a Commit / Save button cannot mean "flush
pending writes" (there are none) — it can only mean **re-baseline the client's since-you-last-looked
diff overlay**. That is today's "Plain" button with an intent-revealing name. Worth a rename +
rethink of Export (a power-user escape hatch, not a primary action), not a new persistence path.

**Two quick fixes (each carries a small fork).**

- **Duplicate title (#9).** The header `<b>` and the in-SVG serif nameplate (DESIGN.md §3, "the
  engraved maker's mark") both print the model title. Keep one. The header is always-visible and
  functional; the SVG nameplate is the treasured brand mark that scrolls away — DESIGN has a stake,
  so decide before cutting.
- **Serve by default (#12).** `faceto <file>` should launch `serve` (the primary action) instead of
  requiring the subcommand. A CLI-contract change in `main.rs` dispatch — small, but it changes the
  bare-argument meaning, so keep `render` / `genesis` / `compact` explicit. **Reconciled** into the
  CLI cluster → tracked on **F-cli-help** (with `--help` / F-output-naming, one `main.rs` pass).

## Working note — F-region-collapse (2026-07-03, branch `feat/F-region-collapse`, as built)

**Scope built = the reshaped v1 exactly:** column-fold only, no crossing-edge reroute (that stays
**F-region-edge-fold**). A folded region's clamped column span compresses to one thin `COLLAPSE_W`
(60px) summary slot; its stickies hide behind a `▸ Label · N` count chip on the tab, and every
column to its right shifts left so the board actually shortens. **Pure view-state** — no `Model`,
event, `replay`, or `from_json` change; the whole delta is `render.rs`, one `serve.rs` query seam,
and `template.html`.

**As built.**

- **`render.rs` — the fold is one pure `col → x` remap.** New `pub struct View { collapsed }`
  threaded as `render_svg(&Model, &View)`. Before the draw loop, each collapsed phase's clamped
  inclusive span `[lo,hi]` marks `is_band_rep[lo]` + `hidden[lo..=hi]`; a cumulative `xs[i]` gives
  each column-index its post-fold left x (`COLLAPSE_W` at a band's leftmost column, `0` for its
  interior, `COL_W` otherwise), and `col_left(c) = xs[c]`. In-band stickies and any edge with an
  in-band endpoint are skipped; a *crossing* edge (both ends visible) stays a straight passthrough.
  Empty / unknown-id set = identity remap, so `render`/`genesis`/`GET /` keep their pre-feature
  *column geometry* (`View::none()`) — not byte-identical output, since every region tab now also
  emits an inert `▾` disclosure glyph. The region right-edge became `xs[hi+1]` (was
  `col_left(hi)+COL_W`, only correct unfolded); the rightmost frontier likewise.
- **`serve.rs` — `?collapse=K2,K5`** parsed by `parse_collapse` (empty segments dropped, absent =
  identity) into the `View`; composes with `?base=` by folding the *baseline* model with the same
  view so the diff overlay lines up.
- **`template.html` — the reader's lens.** Collapsed-set in its own `localStorage` key
  (`facetoCollapsed`, never the comment stash / log); `boardSrc` appends `collapse=` to *every*
  board fetch so a fold survives each swap. Toggle: **`z`** on a focused region tab or click the
  **▸/▾** disclosure glyph (a `.region-collapse` hit target inside the tab, `stopPropagation` keeps
  it off the rename click). Reload re-applies the stored lens to the plain server render.

**Deltas from the plan.** (1) The count chip is **element-count only** (`· N`) — no crossing-edge
count, since edge-fold is deferred. (2) A crossing edge is left as a **full straight passthrough**,
not the plan's "faint" one — dropping a real edge whose both nodes are on-screen would be a worse
lie than drawing it. (3) Added a permanent **▾ disclosure triangle on every live region tab** (▸
when folded) as the click affordance — the discoverable half of the `z`/click pair; kept subtle
(grey, darkens on hover) for the calm register.

**Tests to done (all green).** Pure UT (via rendered SVG, the observable contract): fold shortens
the board + hides in-band stickies + emits the `· N` chip; empty/unknown set = identity; fold is
order-independent + idempotent; adjacent folds stay independent (two chips); an in-band edge drops
while a crossing edge passes through. `serve`: `parse_collapse` splitting + identity. Live-verified
on `serve` (curl): plain 1240px → `?collapse=K2` 670px, `work · 7` chip, cols 2–4 hidden, identity
holds, `?collapse=&base=` still returns `X-Diff-Base`. Browser click not driven (extension offline)
— client JS syntax-checked; wiring is the standard region-tab glyph pattern.

## Working note — F-2d-placement (2026-07-02, branch `feat/F-2d-placement`, as built)

**The stored form.** `y` is an optional **fraction of the lane-band interior in `[0, 1]`** —
band-relative on purpose (the first shaping lock): it survives a lane merge (F-lane-flow b), a
region collapse, or any band-height change without remapping. It rides `ElementMoved` *and*
`ElementAdded` (both additive — an old log simply has no `y` and replays identically);
`ElementAdded` must carry it or `compact`/genesis would silently flatten a placed board. A
col-only move never resets a stored `y`. The fraction is clamped + rounded at the comment seam
(`events::clamp_y`) and clamped again at render, so an out-of-range log value can't draw off-band.

**Reshaped mid-dogfood: grid, not free canvas.** The first cut rendered `y` as a literal free
position; testing showed free vertical placement carries little meaning. As built, `y` is an
**ordering key**: a cell's members sort by it (unplaced = the neutral 0.5, barycenter tie-break)
and *everyone* renders on **row-slot centres** — a lone box sits on the classic mid-line whatever
its `y`, two sharing a cell split top / bottom and the lane grows a row to hold them. Same log
schema, same replay; only the render interpretation changed.

**Default without `y` = the old Rows stack.** Auto-stacked elements keep the barycenter ordering
(F-edge-routing Lever A) and the lane-height rule is unchanged (deepest cell) — which sidesteps
the fraction/band-height circularity a "grow to fit stored Ys" rule would create, and renders an
un-migrated log byte-identically.

**Packing is gone everywhere.** The `Packing` enum, `--pack`/`-k`, `?pack=`, the Rows/Columns/Grid
header control, the grey time-slot tray, and the sub-column machinery (`SUBCOL_W`, per-column
widths). **One col = one x slot** now holds unconditionally — the second shaping lock (zero
intra-cell X spread), and exactly the ground the F-region-frontiers interstice column assumes.
This also buries the stale "packing buttons don't switch" note under F-edge-routing.

**Gesture.** Drag is 2D: x snaps to columns as before, and the pointer's y (clamped to the lane
band — `type` = lane is untouchable) becomes an ordering key whose **preview snaps to the same
grid slots the commit will produce**: the client mirrors the renderer's cell-stack placement
(`computeGrid`, fed by the `data-y` keys render.rs emits), so a drop never "jumps" on the
authoritative re-render and legacy/offline replays land on the grid too. A drop posts the `y`
key **only when the target cell is shared**; into an empty cell it posts col-only, so the box
stays auto-placed. While the drag hovers a cell that would deepen the lane, a horizontal
**lane-growth guide** (`#lane-grow-guide`, the region-resize live-pen blue) marks where the
lane's bottom rule will land on release. ←/→ still posts col-only. The **force-swap is removed**
(dogfood bug #1): nothing is displaced, stickies sharing a cell are simultaneous and stack on
the grid. The server keeps *parsing* `swapId` so old logs and stashed offline moves replay
faithfully. Undo of a placement restores the prior key — the neutral `0.5` for a
previously-unplaced box, which `model::y_key` makes indistinguishable from "no y" — and a
y-only change diffs as `moved` through that same key, so a neutralised placement never reads
as a phantom move.

## Why this slice

Chosen by filtering all eight directions through felt dogfood pain. The three live
pains were clunky editing, an unreadable board, and losing momentum solo — **not**
thin modelling vocabulary, which dropped F-es-vocabulary, F-ddd-process,
F-new-diagrams, and F-model-smells out of the slice automatically.

Two deferred items are named on purpose:

- **F-container** is a hidden hub — UI bounded-context editing, F-model-smells, and
  F-ddd-process all silently depend on it, and the model has no container concept
  today. Cheap to add now, expensive to retrofit; build it when grouping is the pain.
- **F-narrative-skill** (né F-mcp-narrative) is the on-thesis answer to "solo & stuck"
  (faceto is "a simple typed file you think through with an LLM"). Reshaped 2026-07-02:
  the write seam an MCP server would expose already ships (`POST /comment` + per-request
  log re-read), so the slice is a skill, not a server — the server is parked as
  **F-mcp-server**. F-multiplayer stays parked because it solves a different problem —
  crowded, not solo.

## Working note — Field feedback triage (issue #13, 2026-07-02)

Source: field feedback from authoring + workshop-reviewing a **147-element / 186-edge /
48-column** two-bounded-context board through a full author → serve review → fix → resolve
loop ([issue #13](https://github.com/bastien-gallay/faceto/issues/13)). Highest-signal input
to date — the whole loop ran on a real board. Mapping of every item to a feature:

- **§1 CLI / Output** → **F-output-naming** (sibling clobber), **F-cli-help** (`--help`),
  **F-png-docs** (raster export).
- **§2 Model format** → **F-status-tracking** (as-is/to-be), **F-typed-edges** (untyped edges),
  **F-tech-names** (technical-name layer), **F-es-vocabulary** (timer / process sticky types).
  Bounded contexts → see pushback below.
- **§3 ES-grammar lint** → **F-es-lint** (the headline; warn-only + `level` + sidecar flow).
- **§4 Timeline at scale** → no new feature. Single-row-breaks-past-~20-cols is the concurrent-
  lifecycle problem already owned by **F-2d-placement** (free Y within a lane) and the region
  work (**F-region-frontiers** / **F-region-collapse**); wide-board back-edge readability is
  **F-edge-routing** + **F-region-collapse**. Re-scope those with the §4 evidence rather than
  add an ID.
- **§5 Comment lifecycle / identity** → **F-comment-lifecycle**.
- **§6 What worked well** → protect, don't build: the serve→review→fix→resolve loop, LLM-safe
  `model.json` transforms, the hotspot lane. Constrains **F-es-lint** to stay warn-only.

**Two pushbacks resolved (author call):**

- **Raster/PNG export is a genuinely good idea, but the zero-dep constraint holds.** Ship it as
  documentation of the sanctioned external paths (`rsvg-convert` / `resvg` / headless Chromium),
  not a built-in rasterizer → **F-png-docs**. Raster-in-binary is a deliberate non-goal.
- **Bounded contexts already shipped (F-container, PR #8–11) but not in the form this board
  needed** — the in-flight region rework (**F-region-frontiers**) improves the usable model, and
  a future MVP walkthrough + clear tool-usage docs are the real fix so this class of "I invented
  a convention because I didn't know it existed" comment stops surfacing. No new build; treat as
  a **discoverability / docs** gap, not a missing primitive.

New catalog rows are tagged *Field feedback #13*. Suggested first slice (value-to-effort, no
design debt): **F-es-lint** + **F-output-naming** + **F-cli-help**.

## Working note — Batch reconciliation (2026-07-02)

Two feedback batches landed the same day — the **dogfood batch** (#1–12, from re-reviewing the CISAC
board: five slices + two quick fixes) and the **field-feedback batch** (issue #13, from the
147-element workshop loop: eight rows). Reconciled into one set — no feature is a duplicate; the
overlaps are cross-referenced, not merged away:

- **CLI cluster.** Dogfood #12 (`faceto <file>` → `serve`) folded into **F-cli-help**, alongside
  **F-output-naming** — one `main.rs` dispatch pass.
- **Lane-set seam.** **F-lane-flow** (reorder / merge), **F-floating-hotspots** (removes the hotspot
  lane) and **F-es-vocabulary** (adds `timer` / `process` lanes) all mutate `LANES` / `colour` /
  `lane_prefix` — sequence them so the lane set is touched **once**.
- **Comment / export seam.** **F-commit-flow** (Export → commit re-baseline) and
  **F-comment-lifecycle** (collapse the exported-array vs `comments.jsonl` duality) meet at the same
  representation — reconcile together.
- **Already cross-mapped by #13 §4:** timeline-at-scale points at **F-2d-placement** (shipped),
  **F-region-frontiers** / **F-region-collapse**, and **F-edge-routing** — shared ground, no new ID.
- **Lint stays split:** **F-es-lint** (graph-only) is distinct from **F-model-smells** (needs
  F-container).
- **No #13 sibling:** dogfood #9 (duplicate title) stays a standalone quick fix.

## Working note — code-review hardening pass (2026-07-03, branch `fix/harden-render-events-serve`)

Not a catalog feature — a review of the three largest source files (`render.rs`, `events.rs`,
`serve.rs`, all ~1200–2100 lines) surfaced eight issues; all fixed on this branch (PR #29), with
regression tests for the five correctness ones. Recorded here so the reasoning and the one
behaviour change survive.

**Correctness.** (1) `render_svg` panicked on any element whose `type` isn't one of the 8 lanes
(the per-lane `lane_rows`/`lane_top` lookups), though `colour`/`lane_index` already tolerate
unknown kinds — now off-grammar stickies drop from the view before geometry (edges skipped by the
`idx_of` guard); the log stays truth. (2) A panic in the `append_minted` critical section poisoned
the `appends` mutex, permanently bricking every future `POST /comment` — all lock sites now recover
via `unwrap_or_else(|p| p.into_inner())`. (3) `parse_log` silently dropped a **known**-kind event
with a missing/mis-typed field exactly like an unknown kind — now a hard error (only unknown kinds
skip, per the forward-compat contract). (4) `render_html`'s chained `.replace` let a label equal to
a template token (`__CONFIG__`) get clobbered — replaced with a single-pass `fill_template`. (5)
`replay`'s `PhaseAdded` is now idempotent by id like `ElementAdded`, so a duplicate never strands a
ghost region.

**Efficiency / cleanup.** The read path re-parsed + replayed the whole log every request (the cache
was only an insertion guard) — `current()` gained a fast-path and `/model-version` (the ~1 Hz poll)
a replay-free `version()`. `mint_region_id` re-implemented replay's fold in `serve.rs` — extracted
`events::region_watermark`, the namespace rule now lives once in the spine. Request line + headers
read unbounded (`MAX_BODY` guards only the body) — added `read_line_capped` + `MAX_HEADER_LINE` /
`MAX_HEADERS` (→ `431`).

**Dropped after verification** (guarded by `normalize()`, which runs in both `replay` and
`from_json`): the "unsorted phases" and "overlapping-band miscount" candidates.

**Behaviour change (intended).** `parse_log` now **rejects** a known event with a bad field it
previously tolerated — a hand-authored / externally-generated log relying on silent-skip will now
error on load.

## Working note — source-file module split (2026-07-04, branches `refactor/split-render` + `refactor/split-events-serve`)

The direct follow-up to the hardening pass: the same three files the review flagged as too large
(`render.rs` 2164, `events.rs` 1928, `serve.rs` 1311) were decomposed into concern-focused
submodules. **Pure refactor, no behaviour change** — the gate (`fmt`, `clippy -D warnings`, the full
test suite, a smoke render) stayed green at every step, and no `Cargo.toml` change (zero-deps
intact). Recorded so the *seams* — the deliberate part — survive.

- **`events/`** (PR #33): `codec` (JSON ⟷ Event, plus the single `KNOWN_KINDS` vocabulary) · `log`
  (IO/framing) · `replay` (the projection; the F-region-frontiers arms extracted to `add_phase` /
  `remove_phase` / `move_frontier` / `split_phase` helpers) · `genesis` (`from_model`/`compact`) ·
  `comments` (comment→event) · `mod` holds the `Event` enum + re-exports the external `events::*`
  API unchanged.
- **`serve/`** (PR #33): `mod` (server core: `Cache`, `Ctx` + the append critical section, `serve`)
  · `http` (wire layer; `handle` routes to one `route_*` fn per endpoint) · `ids` (mint) · `comment`
  (`POST /comment`→event) · `sidebar` (`/comments` + lint merge) · `hash` (FNV-1a). `Ctx` is the
  `pub(crate)` hub the wire/comment/sidebar layers share; only the methods/constants they reach are
  elevated.
- **`render/`** (PR #31, landed on `main` first): `style` (colour grammar + `lane_prefix`) · `text`
  (label wrapping/esc) · `geometry` (constants + layout math) · `svg` (`render_svg`, decomposed
  840→468 via `draw_header`/`draw_lanes`/`draw_edges`/`draw_stickies`/`draw_legend`) · `html`
  (`render_html` + single-pass `fill_template`) · `mod` re-exports.

**Tests co-located with their code.** Each submodule carries its own `#[cfg(test)] mod tests`; the
shared property-based / temp-log+`Ctx` harnesses live in a `#[cfg(test)] mod testutil` per crate
module (`ev`/`Lcg`/`gen_comment`/`genesis` for events, `added`/`region_added`/`model_of` for serve).
An earlier pass had parked each suite in one `tests.rs`; review feedback ("tests must be kept with
their file") moved them home — which also let three helpers (`parse_collapse`, `comments_from_log`,
`lint_items`) drop back from `pub(crate)` to private, since their tests no longer reach across a
module boundary. Production code per module stays small; a file reads longer only because its tests
now sit beside it.
