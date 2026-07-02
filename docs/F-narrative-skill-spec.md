# F-narrative-skill — spec

Reshaped from F-mcp-narrative by the feature-torture session of 2026-07-02
(report: `.personal/feature-torture/reports/F-mcp-narrative.md`). One sentence:
**a skill that lets an LLM agent read the event log, tell the model's story back, and
propose events through the shipped `POST /comment` seam — no new Rust.**

## Goal

Fix the third dogfood pain, "solo & stuck": mid-session, the modeller loses momentum
because nothing reads the model back and asks *what happens next?* The agent becomes a
second participant whose every proposal is a first-class, reviewable event in the
append-only log.

## Why no server code

Two shipped facts make this slice prompt-ware:

- `serve.rs` `current()` (line 42) re-reads the log from disk on **every** request, and the
  client polls `/model-version`. Any externally appended event repaints the live board as a
  diff overlay within one poll tick.
- `POST /comment` already gives an external writer everything an MCP tool would: server-side
  id minting (`mint_id`, under the appends mutex), the non-blank-label guard, the off-grammar
  lane-type rejection, and atomic multi-line appends. `region-add` is covered too
  (`serve.rs:367`), so bounded contexts are proposable — this closes the torture report's
  first open probe.

The MCP server itself is parked as **F-mcp-server** (see ROADMAP.md).

## Deliverable

One skill definition (working name **`faceto-narrate`**) usable from Claude Code against a
repo holding an `event-log.jsonl`, plus a docs page. Where it lives (project
`.claude/skills/`, a shipped `skills/` directory, or user-level) is an open question below —
default assumption: **checked into this repo** so the skill versions with the schema it reads.

## Behaviour

Two passes, both grounded in the log, run in order:

1. **Reverse narrative.** Replay the log mentally (the `Event` kinds in `src/events.rs` are
   the vocabulary), then tell the story **backwards** from the last event: "the order
   ships… but nothing says who reserved stock." Walking effect→cause surfaces missing
   commands, actors, and policies that forward reading glosses over.
2. **Discovery.** Name the gaps as concrete proposals: missing elements (with lane and
   column), unanswered hotspots, suspicious shapes (an event no policy reacts to, a command
   with no actor, a bounded context spanning half the board). Each proposal is one sentence
   of *why* plus the event it would append.

Then, **only on explicit user approval, one proposal at a time**, apply via HTTP. Never
batch-apply; the user is the author of their model (the pair-with-me stance).

## Write contract (the wire format the skill must use)

`POST http://127.0.0.1:<port>/comment`, JSON body, one action per request.
Response: `{"ok":true}` or `{"ok":false}` with 400/500.

| kind | required fields | optional | appends |
| --- | --- | --- | --- |
| `add` | `type` (one of the 8 lanes), `text` (label, non-blank) | `col`, `prepend:true`, `detail` | `ElementAdded`, **server-minted id** |
| `move` | `elemId`, `col` | `swapId` + `swapCol` | `ElementMoved` (two lines on swap) |
| `rename` | `elemId`, `text` (non-blank) | — | `ElementRenamed` |
| `resolve` | `elemId`, `text` (resolution) | — | `HotspotResolved` |
| `drop` | `elemId` | — | `ElementRemoved` |
| *(other)* | `elemId`, `text` | — | `ElementAnnotated` |
| `region-add` | `text` (non-blank), `fromCol`, `toCol` | — | `PhaseAdded`, server-minted id |
| `region-resize` | `regionId`, `fromCol`, `toCol` (valid span) | — | `PhaseResized` |
| `region-rename` | `regionId`, `text` (non-blank) | — | `PhaseRenamed` |
| `region-remove` | `regionId` | — | `PhaseRemoved` |

Source of truth: `events::comment_to_events`, `serve.rs::add_from_comment` /
`add_region_from_comment`. If this table and the code diverge, the code wins.

## Hard rules (the skill's guardrails)

- **Read the file, write the HTTP.** Reading `event-log.jsonl` directly is always fine.
  Writing to it directly is **forbidden while `serve` runs** — it bypasses minting, the
  domain guards, and the append mutex.
- **`serve` down → read-only.** Narrate and propose in prose; apply nothing. (No CLI append
  path exists, by decision — option 3 of the torture ADR was rejected for its mint race.)
- **Never invent ids.** `add` / `region-add` let the server mint; mutations only ever
  reference ids read from the log.
- **Respect the register.** Proposals are calm and specific (PRODUCT.md: the model is the
  subject). No cheerleading, no bulk rewrites, no "let me restructure your board".
- **Hotspots over assertions.** When the agent is unsure whether a gap is real, propose a
  `hotspot`-lane add (a question), not a confident element.

## Edges

- Empty or genesis-only log → skip reverse narrative, run discovery as onboarding ("an event
  storm usually starts with a domain event — what happens in this business?").
- Log with a `LogCompacted` marker → replay from the snapshot exactly as `events::compact`
  intends; do not treat pre-compaction history as missing.
- Unknown event kinds in the log (forward compatibility) → skip silently, exactly as
  `replay` does; never propose "fixing" them.
- Port discovery → default `8753`, confirm via `GET /health` before the first write.

## Test shape

No Rust changes → no new Rust tests; the gate stays green untouched. Verification is the
F-board-gestures pattern:

- Hand-verified live session against `examples/sample.model.json` (genesis → serve →
  narrate → approve one add, one hotspot, one region) with the appended lines checked on
  the wire and the board diff observed.
- A transcript-style worked example checked into `docs/` as both documentation and the
  skill's regression baseline (re-run it after any event-schema change).

## Out of scope

- The MCP server (parked: F-mcp-server) and any new event kind or model-spine change.
- Draft/staged proposal state on the board (torture open question `#ux` — resolve by
  dogfood; the one-at-a-time approval rule stands in until then).
- A machine-readable `GET /model` endpoint (torture `#deps` probe, ~30 LOC) — only add it
  if prose replay proves error-prone in practice.

## Open questions

- Skill packaging: repo-local `.claude/skills/` vs user-level — who else consumes it? `#scope`
- Should the transcript example live in `docs/` or `examples/`? `#scope`
- Diff-ring depth under two writers (`CACHE_MAX = 12`) — inherited from the torture
  report, observe during hand-verification. `#perf`
