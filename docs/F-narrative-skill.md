# F-narrative-skill — the reverse-narrative discovery skill

`faceto-narrate` is prompt-ware: a Claude Code skill (`.claude/skills/faceto-narrate/`),
**no Rust**. It lets an LLM agent read a board's `*.event-log.jsonl`, tell the model's story
back to a stuck modeller, and propose the missing pieces as first-class, reviewable events —
appended through the shipped `POST /comment` seam, one at a time, on explicit approval.

It exists to fix the third dogfood pain, *"solo & stuck"*: mid-session the modeller loses
momentum because nothing reads the model back and asks *what happens next?* The skill becomes
a second participant whose every proposal lands as an event in the append-only log.

Design provenance: reshaped from `F-mcp-narrative` by the feature-torture session of
2026-07-02 (verdict ✂️ reshape — ship the skill, park the MCP server as `F-mcp-server`). Full
spec and locked decisions: [`F-narrative-skill-spec.md`](F-narrative-skill-spec.md).

## Why no server code

The write seam an MCP server would expose already exists twice:

- `serve` re-reads the log from disk on **every** request and the client polls
  `/model-version` (~1 Hz), so any externally appended event repaints the live board as a diff
  overlay within one poll tick.
- `POST /comment` already provides everything a tool would: server-side id minting (under the
  appends mutex), the non-blank-label guard, the off-grammar lane-type rejection, and atomic
  multi-line appends. Regions and phase-splits are covered too.

So the skill reads the file directly and writes over HTTP. See the skill's own **write
contract** table for the wire format; the source of truth is `events::comment_to_events`,
`serve::add_from_comment` / `add_region_from_comment` / `split_region_from_comment`.

## How to use

1. Serve a board: `faceto serve examples/sample.model.json` (auto-genesis creates the log;
   default port `8753`).
2. In Claude Code, in that repo, invoke the skill (`/faceto-narrate`) or just ask *"narrate
   this board — what's missing?"*
3. Read the reverse narrative, approve the proposals you want **one at a time**; watch them
   land on the live board as a diff overlay.

If `serve` is not running the skill is **read-only**: it narrates and proposes in prose but
appends nothing (there is no offline write path, by design — a direct file append would
bypass minting and the guards).

## Worked example (regression baseline)

This is the skill exercised end-to-end against `examples/sample.model.json` — the daily-ops
board (Operator → *start the day* → DayPlan / DayStarted → *add an item* → ItemAdded → the
"project forward" policy → UnfinishedWorkProjectedForward → Today view). **Re-run it after any
event-schema change** and confirm the wire lines below still match `comment_to_events`.

### Setup

```bash
faceto genesis examples/sample.model.json      # → examples/sample.event-log.jsonl (if absent)
faceto serve  examples/sample.model.json &     # live board on :8753
curl -s http://127.0.0.1:8753/health           # → {"ok":true}
```

### Pass 1 — reverse narrative

> The **Today view** (`R1`) is what the operator ends up reading. It's projected from
> **UnfinishedWorkProjectedForward** (`E3`), which the *"when ItemAdded, project forward"*
> policy (`P1`) raises in response to **ItemAdded** (`E2`). `E2` comes from the operator's
> *"add an item"* command (`C2`), and the day itself opens with *"start the day"* (`C1`) →
> **DayStarted** scaffolding the **DayPlan** aggregate (`A1`).
>
> Reading it backwards, two things stand out. An **ItemAdded** can name a topic that doesn't
> exist yet — the board already flags this as the open hotspot **`H1` "TopicCreated (item
> added to a non-existent topic)"**, and nothing on the board creates a topic. And the story
> only ever *adds* work: no command completes or closes an item, so
> **UnfinishedWorkProjectedForward** has no counterweight — everything projects forward, forever.

### Pass 2 — discovery (proposals)

1. **Answer the open hotspot, or make it a real flow.** `H1` asks who creates a topic.
   Cheapest honest move is a `command` in the *begin* region — *"create a topic"* — that a
   `TopicCreated` event would follow. *(add · command)*
2. **The missing completion command.** Work is only ever added. A *"complete an item"* command
   → `ItemCompleted` event would close the loop the read model reports on. *(add · command)*
3. **Unsure it belongs?** If completion is out of scope for this board, leave a question instead
   of asserting the command — a `hotspot` with a **terse** label: *"Does an item get completed?"*
   The reasoning ("or is this an append-only day log?") stays here in chat, not on the sticky.
   *(add · hotspot — the honest fallback)*

### Pass 3 — apply, one at a time, on approval

The user approves (1). Exactly one POST; the server mints the id; the board repaints:

```bash
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"add","type":"command","text":"create a topic","col":0}'
# → {"ok":true}
# appended: {"event":"ElementAdded","id":"C3","type":"command","label":"create a topic","col":0}
```

The user is unsure about completion, so approves (3) — a hotspot, not an assertion:

```bash
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"add","type":"hotspot","text":"Does an item get completed?","col":2}'
# → {"ok":true}
# appended: {"event":"ElementAdded","id":"H3","type":"hotspot","label":"Does an item get completed?","col":2}
```

And answers the pre-existing hotspot `H1` (id read from the log — never invented):

```bash
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"resolve","elemId":"H1","text":"A topic is created up front by the new create-a-topic command"}'
# → {"ok":true}
# appended: {"event":"HotspotResolved","id":"H1","resolution":"A topic is created up front by the new create-a-topic command"}
```

Session done. Three events on the wire, all reviewable, all in the operator's own terms — and
the ids (`C3`, `H3`) were minted by the server, never by the skill.

## Verification

Per the spec's test shape (the `F-board-gestures` pattern) — no Rust changed, so the gate is
untouched:

- **Hand-verified live session** against `examples/sample.model.json` (genesis → serve →
  narrate → approve one add, one hotspot, one resolve) with the appended lines checked on the
  wire and the board diff observed.
- **This worked example is the regression baseline.** Re-run it after any event-schema change;
  if the appended lines drift from what `comment_to_events` now produces, update the skill's
  write-contract table (the code wins) and this document together.
