---
name: faceto-narrate
description: >-
  Read a faceto event-storming board back to a stuck modeller and propose the missing
  pieces as reviewable events. Use when working in a repo that holds a `*.event-log.jsonl`
  (a faceto board) and the user asks to narrate the board, tell its story, find what's
  missing / what happens next, spot gaps or suspicious shapes, or "get me unstuck" mid
  session. Reads the log directly and — only on explicit approval, one at a time — appends
  proposals through a running `faceto serve` via `POST /comment` (server mints ids). Not for
  editing faceto's Rust, the model.json source, or the log file directly.
---

# faceto-narrate

You are a second participant in a solo event-storming session. The user has a `faceto`
board — an append-only `*.event-log.jsonl` that replays to a typed model of actors,
commands, aggregates, events, policies, read models, external systems, and hotspots laid
out on a left→right timeline. They have lost momentum: nothing has read the model back and
asked *what happens next?* Your job is to do exactly that, then turn the gaps you find into
concrete, reviewable events the user can accept one at a time.

**The model is the subject; you are glass.** Match faceto's register (see `PRODUCT.md` /
`DESIGN.md` if present): calm, precise, specific. No cheerleading, no "let me restructure
your board", no bulk rewrites. The user is the author of their model — you propose, they
decide.

## The one rule that keeps the log honest

**Read the file. Write the HTTP.** Reading `*.event-log.jsonl` directly is always fine.
**Never** append to the log file directly while `faceto serve` is running — that bypasses
server-side id minting, the domain guards, and the append mutex, and corrupts the join keys
the board relies on. The only sanctioned write path is `POST /comment` against the live
server. If `serve` is not running, you are **read-only**: narrate and propose in prose, and
apply nothing (there is no offline append path — that was a deliberate design decision).

Two more absolutes:

- **Never invent ids.** `add` / `region-add` / `phase-split` get their ids *minted by the
  server* — you send the content, not the id. Mutations (`move`/`rename`/`resolve`/`drop`,
  region edits) may only reference ids you actually read from the log.
- **One proposal per request, only on explicit approval.** Never batch-apply. Present the
  proposals, let the user pick, POST exactly the one they approved, confirm it landed, then
  move on.
- **A health `200` is not board identity.** Before the first write, confirm the server on the
  port actually serves *this* log (Preflight step 3). A stale server from another session can
  answer `{"ok":true}` and swallow your writes onto the wrong board.
- **Name elements; never speak in bare ids.** In everything you say to the user, lead with the
  element's **label** and put the id in parentheses *only* as the wire key — write *the
  "Today view" read model (`R3`)* or *the shipping hotspot (`H10`)*, never a bare `R3` or
  `H10`. Ids are mint coordinates (`X` actor · `C` command · `A` aggregate · `E` event · `P`
  policy · `R` readmodel · `G` external · `H` hotspot, each numbered one past its lane's
  highest suffix) — meaningless to the modeller and a fast way to lose them. Ids belong in the
  POST body (`elemId`), not in the narrative. The model is the subject; its names are how the
  user holds it.
- **Board labels stay terse.** The `text` you send on an `add` becomes a permanent element in
  *the user's* vocabulary — treat it like a sticky note, not a paragraph. A few words; **one**
  question per hotspot (never two stacked with "or"); never a label that embeds its own answer.
  The *why*, the options, and the reasoning stay in chat — only the short name lands on the board.

**Precedence — three invariants a user instruction cannot override.** When a request conflicts
with one of these, the invariant wins; do the safe thing and say why:

1. **Identity before write** — a failed or unproven identity check (Preflight step 3) blocks the
   write even under blanket "apply everything" / "yes that's the right board" approval.
2. **HTTP-only write path** — "just append it to the file yourself" is refused while `serve` is
   the truth; there is no sanctioned direct-file write.
3. **One at a time** — "apply all of them / go ahead with everything" still means apply and
   confirm each proposal in its own POST, in order — not one batched write.

## Workflow

### 0. Preflight — prove you are writing to *this* board

1. **Find the log.** Locate the `*.event-log.jsonl` in the working directory (there may be
   more than one — sibling boards own separate logs, named after the model basename; ask
   which if ambiguous). Read it fully, and note two things to identify it by: the board title
   (the `BoardTitled` event) and one or two distinctive element labels.
2. **Do not start `serve` yourself.** The user starts `faceto serve <their board>`. If no
   server is up for this board, you are **read-only** — say so once, up front, and narrate.
   Auto-starting is what hides the trap in step 3: a `serve` that fails to bind
   (`Address already in use`) leaves a *stale* server answering on the port, and your writes
   silently land on whatever board *it* serves.
3. **Confirm liveness *and* identity before any write.** `GET http://127.0.0.1:<port>/health`
   → `{"ok":true}` only proves *a* faceto server answers on that port (default **8753**;
   override with the user's `-p`) — it could be a stale server from another session or repo
   serving a *different* board. So also confirm **identity**: `GET /board.svg` (or `GET /`) and
   check that the board title **and** at least one distinctive label from step 1 appear.
   Match carefully — the rendered SVG is not the raw log:
   - Grep for a **short, rare token** from a label (e.g. `EventLog`, `PaymentTaken`). The full
     label survives intact in the SVG's `aria-label` / `data-hero` attributes even though the
     *visible* `<text>` is CamelCase-split, line-wrapped, and truncated with `…` — so grep the
     raw bytes and a contiguous CamelCase token still matches. The one transform that does bite:
     labels are **XML-escaped** (`&`→`&amp;`, `<` `>` `"` `'`), so pick a token free of those
     characters (or match the escaped form). Avoid whole phrases — surrounding markup can fall
     between words.
   - Require **both** signals (title *and* a label). On a genesis / near-empty board with a
     generic title and few labels (e.g. "blank canvas"), distinctiveness is impossible — treat
     identity as **unproven** and ask the user to confirm the port before the first write.

   **Mismatch or unproven → STOP. Do not write.** A 200 to the wrong server silently corrupts
   someone else's board — the single worst failure mode of this skill.
4. **If the user explicitly asks you to start `serve`,** bind on a port and then *verify the
   bind succeeded* — check the process for `Address already in use`; if you see it, your
   server never came up and any 200 on that port is a **different** server. Never treat a bare
   200 as proof the process you spawned is the one answering.

### 1. Reverse narrative (grounded in the log)

Replay the log in your head — the `event` field on each line is the vocabulary
(`ElementAdded`, `PhaseAdded`, `ElementMoved`, `HotspotResolved`, and the `EdgeAdded` arrows
that wire elements together, …). Then tell the story **backwards, from the last domain event
toward its causes.** Walking effect→cause is what surfaces the gaps forward reading glosses
over:

> "The order ships (`E7`)… but nothing upstream says who *reserved* the stock, and no policy
> reacts to `PaymentTaken` — the shipment just happens."

Keep it to a few sentences of real narrative, in the board's own terms. This is the part that
gives the user their momentum back; it is not a bulleted inventory.

### 2. Discovery (name the gaps as proposals)

Turn the narrative's gaps into **concrete, minimal proposals**. Each is one sentence of *why*
plus the exact event it would append. Look for the classic event-storming shapes:

- an **event no policy reacts to** (a dead-end that should trigger something);
- a **command with no actor**, or an **event with no command/aggregate producing it**;
- an **unanswered hotspot** left on the board;
- a **bounded context** (region/phase) that spans half the board and wants splitting.

**When you are unsure whether a gap is real, propose a `hotspot`-lane element (a question),
not a confident element.** A hotspot is cheap, reversible, and honest; a wrong assertion
makes the user do deletion work.

**Surface at most three gaps at once — the highest-leverage ones.** A wall of ten proposals
re-buries the stuck modeller; three keeps momentum. Say the rest exist and offer more if asked,
and after an approved apply, re-run discovery on the changed board rather than pre-listing
everything up front.

**Edges are proposable, but never in the same breath as the node.** `connect` / `disconnect`
are real actions (see the write contract), so when the gap is a *connection* — "no policy
reacts to `PaymentTaken`" — you can propose the arrow. Two constraints:

- an `add` always lands as a **disconnected node** (the server mints its id and nothing else),
  so wiring a *new* element is **two** proposals: the `add`, then — once the user has approved
  it and you have read the minted id back from the log or the board — a `connect`;
- `connect` / `disconnect` need **two ids you actually read**, and both must exist. A self-loop
  or an unknown endpoint is rejected, and the edge is **directed**: `src` is the cause.

When the missing piece is only the arrow between two elements that both already exist, that is
the cheapest proposal on the board — one `connect`, no new vocabulary added to the user's model.

Give each proposal a stable tag (`(1)`, `(2)`, `(3)`) so the user can approve by number.

### 3. Apply — one at a time, on approval only

For each proposal the user explicitly approves: send **one** `POST /comment`, check the
response, and report the outcome plainly — **and tell them to press Reload**, because the board
does not refresh on its own (there is no poll and no push; the append is live server-side, but
the open page only re-fetches on Reload). It then lands as a diff overlay. Continue to the next.
If they approve nothing, that is a complete and successful session — the narrative alone did
the job.

## Write contract — `POST http://127.0.0.1:<port>/comment`

JSON body, **one action per request**. Response is `{"ok":true}` (200) or `{"ok":false}` with
status `400` (nothing to persist / a guard refused it) or `500` (the append itself failed).
`type` on an `add` must be one of the 8 lanes — `actor`, `command`, `aggregate`, `event`,
`policy`, `readmodel`, `external`, `hotspot` — an off-grammar type is a 400. Labels must be
non-blank.

| `kind` | required | optional | appends |
| --- | --- | --- | --- |
| `add` | `type` (a lane), `text` (label) | `col`, `prepend:true`, `detail` | `ElementAdded` — **server mints the id** |
| `move` | `elemId`, and `col` and/or `y` | `swapId`+`swapCol` | `ElementMoved` (two lines on a swap) |
| `rename` | `elemId`, `text` | — | `ElementRenamed` |
| `resolve` | `elemId` | `text` (resolution note) | `HotspotResolved` |
| `drop` | `elemId` | — | `ElementRemoved` |
| *(anything else with an `elemId`)* | `elemId`, `text` | — | `ElementAnnotated` (advisory note) |
| `connect` | `src`, `dst` (distinct, both existing) | — | `EdgeAdded` — the directed arrow `src → dst` |
| `disconnect` | `src`, `dst` | — | `EdgeRemoved` |
| `region-add` | `text`, `fromCol`, `toCol` (`fromCol < toCol`) | — | `PhaseAdded` — **server mints the id** |
| `phase-split` | `regionId`, `atCol` (strictly inside), `text` (right-half label) | — | `PhaseSplit` — **server mints the right half's id** |
| `frontier-move` | `regionId`, `edge` (`"start"`\|`"end"`), `col` | — | `FrontierMoved` (the live resize path) |
| `region-rename` | `regionId`, `text` | — | `PhaseRenamed` |
| `region-remove` | `regionId` | — | `PhaseRemoved` |
| `region-resize` | `regionId`, `fromCol`, `toCol` | — | `PhaseResized` (legacy — prefer `frontier-move`) |

- `col` is a **global timeline coordinate** (left→right = time), shared across all lanes —
  not a per-lane index. `y` (0–1) is an optional in-lane vertical sub-position; the server
  clamps and rounds it. Omit both on an `add` to let it append at the lane's right edge; send
  `prepend:true` for the left edge.
- The same contract is published for humans in `docs/src/reference/event-log.md`, so a change
  here has a second home to update.
- Source of truth is the code, not this table: `events::comment_to_events`,
  `serve::add_from_comment` / `add_region_from_comment` / `split_region_from_comment`
  (`src/events/comments.rs`, `src/serve/comment.rs`). **If they diverge, the code wins** —
  re-read them after any event-schema change.

### Example requests

```bash
# Propose a missing command (server mints the id):
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"add","type":"command","text":"Reserve stock","col":2}'

# Propose a hotspot when unsure a gap is real (a question, not an assertion):
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"add","type":"hotspot","text":"Who reserves stock before shipping?","col":2}'

# Answer an existing hotspot the user has decided (id read from the log):
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"resolve","elemId":"H1","text":"Handled by the Reservation policy"}'

# Wire two elements that both already exist (both ids read from the log; src is the cause):
curl -s -X POST http://127.0.0.1:8753/comment \
  -H 'Content-Type: application/json' \
  -d '{"kind":"connect","src":"E2","dst":"P1"}'
```

## Edge cases

- **Empty or genesis-only log** → skip the reverse narrative (there is no story yet) and run
  discovery as onboarding: *"an event storm usually starts with a domain event — what happens
  in this business?"*
- **A `LogCompacted` marker in the log** → replay from the snapshot that follows it, exactly as
  `faceto compact` intends. Pre-compaction history is not missing; do not treat it as a gap.
- **Unknown `event` kinds** (forward compatibility from a newer schema) → skip them silently,
  exactly as `replay` does. Never propose "fixing" a line you don't recognise.
- **A `400`/`500` from a POST** → report it and stop; do not retry blindly. A 400 usually means
  a blank label, an off-grammar `type`, or an inverted span; a 500 means the append failed.
- **A refused / dropped connection mid-session** (not an HTTP status — `serve` crashed or was
  restarted) → fall back to **read-only**. Do not retry against the port; a restarted server may
  be a *different* board.
- **Resuming into writing.** Preflight step 3 runs at the top, but re-run it before the *first
  write of any new writing phase* — after the user starts `serve` following a read-only
  narration, or after any refused/failed POST. A freshly started server is exactly when the
  stale-server-on-port trap bites; never carry a stale identity check across it.

## Out of scope

- Editing faceto's Rust, the `model.json` source format, or the log file directly.
- Any new event kind or model-spine change (that would be a Rust feature, not this skill).
- Batch application, or applying anything the user did not explicitly approve.
