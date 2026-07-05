# Regenerating `docs/diff-loop.gif`

`docs/diff-loop.gif` is the README's one animated asset — it shows the loop the static
[`docs/sample-board.svg`](sample-board.svg) hero can't: **annotate a sticky → a next session adjusts
the model → Reload → the diff overlay highlights what's added / changed / moved.**

Unlike the SVG hero (which is a real render and self-syncs), a GIF is a hand-captured binary that
**drifts** whenever the renderer or `src/template.html` changes. So it is committed, not built by CI,
and this is its regeneration recipe. Re-run it whenever the board's look or the diff overlay changes.

## What you need

- A release `faceto` binary (`cargo build --release`).
- [`ffmpeg`](https://ffmpeg.org/) — the only image tool used (a two-pass palette keeps the GIF small
  and clean; no `gifsicle` required).
- A way to drive the live board and screenshot each beat. The capture below uses the
  [`agent-browser`](https://github.com/dbfr3qs/agent-browser) CLI against a headless Chromium, but
  any browser driver that can hover / click / eval / screenshot works — the beats are what matter.

## 1. Serve a throwaway board

Never serve a tracked board — posting comments appends to its `*.event-log.jsonl`. Use the helper,
which copies `examples/sample.model.json` into a temp dir and serves that:

```bash
just demo-serve            # → http://127.0.0.1:8753, temp board, removed on exit (Ctrl-C)
```

To use a different port, pass it (`just demo-serve 9000`) and set `PORT` to match when you run the
capture script below.

## 2. Capture one PNG per beat

The board is ~1450 px wide once the "added" sticky lands, so frame it at **1500 px** so nothing is
clipped. The sequence (annotate `R1` → Save → inject a rename + a move + an add as a "next session"
→ Reload) is scripted below; save it as `capture.sh` and run it while `demo-serve` is up.

```bash
#!/usr/bin/env bash
set -euo pipefail
BASE="http://127.0.0.1:${PORT:-8753}"   # must match the port demo-serve is on
F="./frames"; rm -rf "$F"; mkdir -p "$F"
ab() { agent-browser "$@"; }
# -sf so a rejected POST (4xx/5xx) fails the run instead of silently producing a no-diff GIF.
post() { curl -sf -XPOST "$BASE/comment" -H 'content-type: application/json' -d "$1" -o /dev/null \
  || { echo "post failed: $1" >&2; exit 1; }; }

# Frame the whole board (header + 8 lanes + the col the added sticky will occupy).
ab set viewport 1500 1120 1 >/dev/null
ab open "$BASE" >/dev/null
ab wait "#board svg" >/dev/null
ab screenshot "$F/f00_base.png" >/dev/null

# Annotate a sticky the real way: hover → the speech-bubble glyph → the note modal.
ab hover "#R1" >/dev/null
ab click "#comment-c" >/dev/null
ab screenshot "$F/f01_modal_empty.png" >/dev/null

# Type the note (two beats, so the GIF shows it filling in).
ab eval "(()=>{const t=document.querySelector('#m-text');t.value='rename to reflect';t.dispatchEvent(new Event('input',{bubbles:true}));})();'ok'" >/dev/null
ab screenshot "$F/f02_modal_typing.png" >/dev/null
ab eval "(()=>{const t=document.querySelector('#m-text');t.value='rename to reflect it is the live view';t.dispatchEvent(new Event('input',{bubbles:true}));})();'ok'" >/dev/null
ab screenshot "$F/f03_modal_full.png" >/dev/null

# Save → the note appends to the log; your own edit redraws plain (not a diff).
ab click "#m-save" >/dev/null
ab wait 500 >/dev/null
ab screenshot "$F/f04_saved.png" >/dev/null

# A "next session" adjusts the model: rename (changed), move (moved), add (added).
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
post "{\"elemId\":\"R1\",\"kind\":\"rename\",\"text\":\"Today view (live)\",\"ts\":\"$TS\",\"status\":\"open\"}"
post "{\"elemId\":\"E3\",\"kind\":\"move\",\"col\":4,\"ts\":\"$TS\",\"status\":\"open\"}"
post "{\"kind\":\"add\",\"type\":\"event\",\"text\":\"ReportGenerated\",\"col\":5,\"ts\":\"$TS\",\"status\":\"open\"}"

# Reload → the diff overlay. (The Save's own load() already cleared `ownEdit`, so these later
# posts read as "foreign" and Reload takes the diff branch — no timing window to beat here.)
ab wait 300 >/dev/null
ab click "#refresh" >/dev/null
ab wait 800 >/dev/null
ab screenshot "$F/f05_diff.png" >/dev/null
```

Why the two-source dance: the client shows **your own** just-posted edit *plain* (a settle pulse),
never as a diff. The overlay only appears when the log grew from elsewhere — hence the `curl` posts,
then Reload. Mechanically, `load()` nulls `ownEdit` at the end of every call
(`src/template.html`), so once the Save's redraw has completed, any later version bump is treated
as someone else's change and Reload renders the diff.

## 3. Assemble the GIF

Hold each beat by repeating its PNG at the target frame rate (deterministic timing — the concat
demuxer truncates the last clip's duration, so an explicit numbered sequence is simpler):

```bash
n=0; rm -rf seq; mkdir -p seq
emit() { for _ in $(seq 1 "$2"); do ln -f "frames/$1" "$(printf 'seq/%04d.png' "$n")"; n=$((n+1)); done; }
emit f00_base.png 24        # ~1.6 s  board
emit f01_modal_empty.png 10 # ~0.7 s  modal opens
emit f02_modal_typing.png 8 # ~0.5 s  note filling
emit f03_modal_full.png 21  # ~1.4 s  note written
emit f04_saved.png 14       # ~0.9 s  saved, back to board
emit f05_diff.png 42        # ~2.8 s  the diff overlay (the payoff — hold it)

W=900   # README renders at ~100% width; 900 px keeps the sticky text legible
ffmpeg -y -framerate 15 -i seq/%04d.png \
  -vf "scale=$W:-1:flags=lanczos,palettegen=stats_mode=diff" palette.png
ffmpeg -y -framerate 15 -i seq/%04d.png -i palette.png \
  -lavfi "scale=$W:-1:flags=lanczos,paletteuse=dither=bayer:bayer_scale=3" \
  -loop 0 docs/diff-loop.gif
```

## 4. Check before committing

- Open `docs/diff-loop.gif` and confirm the loop reads: modal → note → Save → Reload → the overlay's
  green **added** / dashed **changed** / dashed **moved** highlights, all legible at README width.
- Keep it small — a committed binary. The current asset is a ~8 s loop, 900 px wide, **≈195 KB**.
  If it creeps up, drop the width or the frame rate before adding colours.
- The throwaway board and every intermediate (`frames/`, `seq/`, `palette.png`, `capture.sh`) are
  scratch — only `docs/diff-loop.gif` is committed.
