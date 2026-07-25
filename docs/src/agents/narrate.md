# The narrate skill

An agent skill that reads a board's event log directly and narrates it back to you when you are
stuck: what the board says, what is suspiciously missing, what plausibly happens next. Proposals
are appended one at a time, on your explicit approval, through the same `POST /comment` endpoint
the mouse uses — so server-side id minting, the guards and the append lock all apply unchanged.

It is prompt-ware: no Rust ships for it. The participation seam already existed, which is the
point. The skill lives at `.claude/skills/faceto-narrate/`.

> **This page is still being written.** The behaviour it covers is shipped; the
> documentation is not. Follow [#111](https://github.com/bastien-gallay/faceto/issues/111).
