// Persist a comment (move or feedback): POST when live, else stash in localStorage. Offline
// stash is best-effort and one-way — it is NOT replayed to the server on reconnect (the next
// live `load()` replaces `comments` with the server's), so structural ops made offline (`add`,
// `move`) never reach the log. Export keeps them; the user is told so below.
// A live drag owns the board: swapping the SVG mid-gesture detaches the captured box (orphaning
// the drag) and replayMoves clobbers its colOf preview. Board rewrites wait for the gesture's end.
// Last-resort leak check: a drag flag whose element no longer holds the pointer capture is dead
// (element detached, capture torn down without its events) and would otherwise defer these
// retries forever — turning Reload itself into a silent infinite no-op. Force-clear and move on.
function whenNoDrag(fn) {
  if (stickyDrag && !document.getElementById(stickyDrag.id)?.hasPointerCapture(stickyDrag.pointerId)) stickyDrag = null;
  if (frontierDrag && !document.querySelector(`.frontier[data-region="${frontierDrag.regionId}"][data-edge="${frontierDrag.edge}"]`)?.hasPointerCapture(frontierDrag.pointerId)) {
    frontierDrag = null;
    dragGuide.style.display = "none";
  }
  stickyDrag || frontierDrag ? setTimeout(() => whenNoDrag(fn), 150) : fn();
}
// The ids this client just wrote, so the next version bump reads as "my edit landed" (a plain
// refresh + settle pulse) rather than foreign change (the diff overlay). The freshness window
// keeps a stale marker from ever suppressing someone else's concurrent edit for long.
let ownEdit = null;   // { ids: [..], at: ms } or null
async function postComment(c) {
  let posted = false;
  try {
    const r = await fetch("/comment", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(c) });
    posted = r.ok;
  } catch {}
  if (posted) {
    ownEdit = { ids: [c.elemId, c.swapId, c.regionId].filter(Boolean), at: Date.now() };
    await load();
    return;
  }
  const a = lsGet(); a.push(c); lsSet(a); comments = a;
  whenNoDrag(() => { replayMoves(); paint(); });
  // add/drop/region-* can't reach the board offline (no server to mint/remove/resize/rename);
  // move is the one structural kind applied locally (see replayMoves).
  note(NOT_APPLIED_OFFLINE.has(c.kind)
    ? `offline — ${c.kind} saved locally (not applied to the board; Export to keep it)`
    : "offline — saved locally (Export to keep)");
}

// Reload: re-pull comments, and when the model changed under us, redraw the board with the
// changes-since-you-last-looked overlay (diffed against the version currently on screen).
async function load() {
  if (stickyDrag || frontierDrag) { whenNoDrag(load); return; }   // defer the swap, keep the gesture
  serverLive = false;
  try {
    const r = await fetch("/comments", { cache: "no-store" });
    if (r.ok) { comments = await r.json(); serverLive = true; }
  } catch {}
  if (!serverLive) { comments = lsGet(); replayMoves(); paint(); return; }
  const live = await liveVersion();
  if (live && shownVersion && live !== shownVersion) {
    // Your own edit never comes back to you as a diff: the overlay grammar means "changed since
    // you last looked", and you were looking. A fresh own-write redraws plain with a settle pulse
    // instead. An already-open diff session keeps accumulating (its baseline is still honest).
    if (ownEdit && Date.now() - ownEdit.at < 4000 && !diffing) await refreshPlain(live, ownEdit.ids);
    else await showDiff(shownVersion, live);
  }
  else if (shownVersion === null) {
    shownVersion = live;  // baseline the initial server-rendered board
    // The embedded server render is always plain; if the viewer left regions folded, re-apply the
    // lens now so a reload restores their view (F-region-collapse localStorage persistence).
    if (collapsedSet().size) await redrawView();
  }
  ownEdit = null;
  // A read-only variant overlay never replays local moves onto the diff DOM (there are none to
  // replay — editing is disabled — and the board must mirror the log/baseline exactly).
  if (!READONLY) replayMoves();
  paint();
}

// Every board swap replaces every node, which used to drop focus on <body> and make keyboard
// editing one-shot (press, re-acquire focus through 141 tab stops, press again). `id` is the
// stable identity, so focus survives the swap by id — and restoring it re-fires onfocus, which
// brings the spotlight and the keyboard claim back with it.
let suppressFocusNote = false;   // true while re-focusing after a board swap — see onfocus
// Minimal scroll delta (or null) to bring a focused box back inside the board viewport. Rects are
// client coords {left,top,right,bottom}. Returns null when the box is already fully visible so an
// ordinary swap keeps its pan untouched; only a box that left the frame (e.g. a move at the edge)
// gets revealed. Positive dLeft/dTop scroll toward the box; add them to scrollLeft/scrollTop.
function revealScroll(el, view) {
  let dLeft = 0, dTop = 0;
  if (el.right > view.right) dLeft = el.right - view.right;
  else if (el.left < view.left) dLeft = el.left - view.left;
  if (el.bottom > view.bottom) dTop = el.bottom - view.bottom;
  else if (el.top < view.top) dTop = el.top - view.top;
  return dLeft || dTop ? { dLeft, dTop } : null;
}
function swapBoard(html) {
  // Focus survives the swap by stable identity — a sticky by id, a region tab by data-region — so a
  // repeated keyboard edit (arrow-move, Shift+arrow region resize) doesn't die after every commit.
  const board = $("#board");
  const ae = document.activeElement;
  const sticky = ae?.classList?.contains("sticky") ? ae.id : null;
  const region = ae?.classList?.contains("region-tab") ? ae.dataset.region : null;
  // The wholesale innerHTML swap resets the scroll box to top-left; snapshot the pan so an edit
  // doesn't yank the board away from where the eye already is, and restore it after re-bind.
  const { scrollLeft, scrollTop } = board;
  board.innerHTML = html;
  bindStickies();
  // Suppress the focus connection-count note so it can't clobber an action confirmation
  // ("moved", "renamed", "region resized") that the swap is landing.
  suppressFocusNote = true;
  // preventScroll so restoring focus doesn't fight the scroll restore below.
  if (sticky) document.getElementById(sticky)?.focus({ preventScroll: true });
  else if (region) document.querySelector(`.region-tab[data-region="${region}"]`)?.focus({ preventScroll: true });
  suppressFocusNote = false;
  board.scrollLeft = scrollLeft;
  board.scrollTop = scrollTop;
  // Preserving the pan can strand a box the swap just moved outside the viewport (an edge move
  // leaves it focused but off-screen, so the next ←/→ acts on something unseen). Reveal the focused
  // box only when it actually left the frame — ordinary swaps keep their restored pan.
  const focused = document.activeElement;
  if (focused && focused !== board && board.contains(focused)) {
    const d = revealScroll(focused.getBoundingClientRect(), board.getBoundingClientRect());
    if (d) { board.scrollLeft += d.dLeft; board.scrollTop += d.dTop; }
  }
}

// Redraw the plain current board (own edit just landed): advance the baseline silently and let
// the affected boxes settle where the eye already is — no banner, no overlay, no chore to clear.
async function refreshPlain(live, ids = []) {
  try {
    const r = await fetch(boardSrc(), { cache: "no-store" });
    if (!r.ok) return;
    swapBoard(await r.text());
    shownVersion = live;
    for (const id of ids) {
      const g = document.getElementById(id);
      if (!g) continue;
      g.classList.add("settle");
      setTimeout(() => g.classList.remove("settle"), 500);
    }
  } catch {}
}

// Replace the board with one diffed against `base`; advance the baseline to `live`.
async function showDiff(base, live) {
  try {
    const r = await fetch(boardSrc({ base }), { cache: "no-store" });
    if (!r.ok) return;
    swapBoard(await r.text());
    if (r.headers.get("X-Diff-Base")) {
      diffing = true;
      diffBase = base;
      $("#plain").style.display = "";
      note("showing what changed since you last looked — Plain to clear");
    } else {
      note("model changed (baseline gone) — showing current");
    }
    shownVersion = live;
  } catch {}
}

// Clear the diff overlay: redraw the plain current board and re-baseline to it.
async function showPlain() {
  try {
    const r = await fetch(boardSrc(), { cache: "no-store" });
    if (r.ok) { swapBoard(await r.text()); replayMoves(); }
  } catch {}
  diffing = false;
  diffBase = null;
  $("#plain").style.display = "none";
  note("");
  shownVersion = await liveVersion();
}

// Re-fetch and swap the board with the current view (fold set + any diff base) applied. `boardSrc`
// carries the collapse set for us; we only preserve whichever baseline is on screen so a fold
// mid-diff keeps the overlay. Used by the collapse toggle and by initial load when a stored fold exists.
async function redrawView() {
  const extra = diffing && diffBase ? { base: diffBase } : {};
  try {
    const r = await fetch(boardSrc(extra), { cache: "no-store" });
    if (!r.ok) return;
    swapBoard(await r.text());
    if (!diffing) replayMoves();
  } catch {}
}

// Fold / unfold a region — the reader's own lens, not a model edit (no POST, no event). Persisted
// per-viewer in localStorage; the server re-lays-out via ?collapse= on the next fetch.
async function toggleCollapse(id) {
  const s = collapsedSet();
  const folding = !s.has(id);
  folding ? s.add(id) : s.delete(id);
  setCollapsed(s);
  await redrawView();
  note(folding ? "region folded — z or ▸ to expand" : "region expanded");
}

