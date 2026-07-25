# The narrate skill

Event storming is a group activity done alone. Half its value comes from someone else reading your
board back to you and asking *and then what happens?* — and when you model solo, nobody does.
`faceto-narrate` is that second participant.

It lives at [`.claude/skills/faceto-narrate/`](https://github.com/bastien-gallay/faceto/blob/main/.claude/skills/faceto-narrate/SKILL.md)
and ships **no Rust at all**. The seam it uses — a log an agent can read, a `POST /comment` an
agent can write, ids minted by the server — already existed for the mouse. That is the point: the
agent is not a special client.

## Using it

```bash
faceto serve orders.model.json     # you start the server; the agent never does
```

Then, in Claude Code, ask it to narrate the board, tell you what is missing, or get you unstuck.
It works in three moves.

**1. It tells the story backwards.** From the last domain event toward its causes — because
walking effect→cause surfaces the gaps that forward reading glosses over:

> "The order ships (`E7`)… but nothing upstream says who *reserved* the stock, and no policy
> reacts to `PaymentTaken` — the shipment just happens."

**2. It names at most three gaps.** The highest-leverage ones, each with one sentence of *why* and
the exact change it would make. Three, not ten: a wall of proposals re-buries a stuck modeller.
When it is unsure whether a gap is real, it proposes a **hotspot** — a question — rather than a
confident element. A wrong question costs you nothing; a wrong assertion costs you deletion work.

**3. You approve, one at a time.** Each approved proposal is one `POST /comment`, confirmed before
the next. Approving nothing is a complete session: the narrative alone often does the job.

## The rules it works under

These are not politeness settings. They are what keeps the log trustworthy, and the skill treats
them as overriding your instructions — if you tell it to skip one, it declines and says why.

**Read the file, write the HTTP.** Reading the log directly is always fine. Appending to the log
file directly is never fine while the server runs: it would bypass id minting, the domain guards
and the append lock, corrupting the join keys the board depends on. If no server is running, the
agent is **read-only** — it narrates and proposes in prose, and applies nothing. There is no
offline append path, deliberately.

**It never invents ids.** Creations get their id from the server; mutations may only reference ids
it actually read from your log.

**It proves it is writing to *your* board.** A `/health` 200 only proves *a* faceto server answers
on that port — it could be a stale one from another session, serving someone else's board. Before
the first write it must match the board title *and* a distinctive label from your log against what
the server actually renders. Unproven identity blocks the write, even under blanket approval. This
guard exists because the failure it prevents — silently corrupting a different board — is the worst
thing this skill could do.

**One proposal per request.** "Apply them all" still means one POST each, in order, each confirmed.

**Labels stay terse.** What it sends becomes a permanent element in *your* vocabulary, so a
proposal is a sticky note, not a paragraph: a few words, one question per hotspot, and the
reasoning stays in the chat where you can argue with it.

## What it will not do

It does not restructure your board, batch-rewrite anything, or edit the model file or the log
directly. It does not start the server for you — that is what makes the identity check meaningful.
And it does not edit faceto's own source: it is a participant in your session, not a contributor to
the tool.

The register is deliberate, and matches the rest of faceto: the model is the subject, the agent is
glass. You are the author of your model; it proposes, you decide.

## When it applies something

The board is polling, so an approved proposal appears within a tick — as a **diff overlay**, ringed
green. You see what the agent did in exactly the same vocabulary you see your own edits in. If it
was wrong, remove it; the log keeps the record of both the proposal and your rejection, which is
more than most review surfaces can say.
