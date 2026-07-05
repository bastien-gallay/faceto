// ---- drag a sticky (2D, grid) ------------------------------------------------
// The mouse counterpart to the ←/→ nudge: grab a box and drag it to a new column (x snaps to the
// timeline grid); the y travel, clamped to the lane band (`type` selects the lane and a move
// never crosses it), decides where the box lands *within a shared cell* — above or below its
// occupants. Placement is a grid, not a free canvas: the drop posts the `y` fraction only when
// the target cell is shared (it is an ordering key — the renderer snaps everyone onto row-slot
// centres and grows the lane to hold them; #lane-grow-guide previews that growth in the live-pen
// blue during the drag). A drop into an empty cell posts col-only, so the box stays auto-placed.
// Pointer Events + setPointerCapture (like the region-resize drag): once captured, every event
// including pointerup lands on the box even if the button releases off-window, so the drag can't
// leak (the fix the PR #11 review pinned for region resize). A press that never crosses
// DRAG_THRESHOLD on either axis is a plain click, which just focuses the box.
const DRAG_THRESHOLD = 4;   // px of travel before a press becomes a drag rather than a click
let stickyDrag = null;      // { id, pointerId, startX, startY, startCy, fromCol, fromFrac,
                            //   col, frac, moved } while dragging, else null
// One predicate for "a gesture owns the board" (inline edit, add, either drag): every gesture
// entry point checks it, so mutual exclusion is one flag added here — not an edit at five sites.
function gestureBusy() { return !!(renaming || adding || frontierDrag || stickyDrag || connectDrag || connecting); }
function hideGlyphs() { document.querySelectorAll(".hovglyph.show").forEach((b) => b.classList.remove("show")); }
// The column index whose centre is nearest an SVG-space x, searching the occupied span plus one
// past each end (colCenter interpolates the empty ones at the default pitch) — snap-to-grid.
function nearestCol(svgX) {
  const known = Object.keys(colX).map(Number);
  if (!known.length) return 0;
  let best = Math.min(...known) - 1, bestD = Infinity;
  for (let c = best; c <= Math.max(...known) + 1; c++) {
    const d = Math.abs(colCenter(c) - svgX);
    if (d < bestD) { bestD = d; best = c; }
  }
  return best;
}
function startStickyDrag(e) {
  if (e.button !== 0 || gestureBusy()) return;
  const g = e.currentTarget;
  if (colOf[g.id] === undefined) return;
  // Capture before any state: if the capture throws (torn-down pointerId, synthetic pointer) no
  // flag has leaked — a leaked stickyDrag would gate gestureBusy() and load()'s whenNoDrag forever,
  // silently killing every gesture including Reload. The press degrades to a plain click instead.
  try { g.setPointerCapture(e.pointerId); } catch { g.focus(); return; }
  // Everything invariant for the whole drag is captured once here — the pointermove handler
  // runs at pointer frequency and must stay O(occupants-of-one-cell): the SVG rect (the pointer
  // is captured, the board can't scroll), the lane's row count, and each same-lane cell's
  // sorted occupant keys (self excluded).
  const k = kindOf[g.id];
  const keysByCol = {};
  for (const oid in colOf) {
    if (oid !== g.id && kindOf[oid] === k) (keysByCol[colOf[oid]] ||= []).push(keyOf(oid));
  }
  for (const c in keysByCol) keysByCol[c].sort((a, b) => a - b);
  stickyDrag = { id: g.id, pointerId: e.pointerId, startX: e.clientX, startY: e.clientY,
                 startCy: effCy(g.id), fromCol: colOf[g.id], fromFrac: yFrac[g.id],
                 col: colOf[g.id], frac: undefined, rank: undefined, moved: false,
                 kind: k, keysByCol, rows: Math.round(bandH[k] / CFG.rowPitch),
                 svgRect: document.querySelector("#board svg").getBoundingClientRect() };
  // the pointer is captured now, so the hover glyphs' own mouseleave won't fire — hide them all
  hideGlyphs();
  g.addEventListener("pointermove", dragSticky);
  g.addEventListener("pointerup", endStickyDrag);
  g.addEventListener("pointercancel", endStickyDrag);
  // Cleanup of last resort: a board swap can detach the box mid-drag, and a detached target never
  // delivers its pointerup — without this the stuck stickyDrag would gate every gesture forever.
  g.addEventListener("lostpointercapture", endStickyDrag);
}
const growGuide = $("#lane-grow-guide");
function dragSticky(e) {
  // Only the captured pointer steers the drag — a second finger resting on the same box is ignored.
  if (!stickyDrag || e.pointerId !== stickyDrag.pointerId) return;
  if (!stickyDrag.moved
      && Math.abs(e.clientX - stickyDrag.startX) < DRAG_THRESHOLD
      && Math.abs(e.clientY - stickyDrag.startY) < DRAG_THRESHOLD) return;
  stickyDrag.moved = true;
  const d = stickyDrag, k = d.kind;
  const target = nearestCol(e.clientX - d.svgRect.left);
  // Y rides the pointer's travel from the grab point, clamped to the lane band — the drag can
  // never cross into another lane. The pointer y becomes an ordering key; the preview itself
  // snaps to grid slots (computeGrid), so what you see during the drag IS the committed layout.
  const cy = Math.min(bandTop[k] + bandH[k],
    Math.max(bandTop[k], d.startCy + (e.clientY - d.startY)));
  const frac = cyToFrac(k, cy);
  d.frac = frac;   // always the latest — this is what the drop posts
  // The grid only changes when the target column or the box's *rank* among that cell's
  // occupants changes (crossing an occupant's key) — not on every pixel. Everything else
  // below runs a handful of times per drag, not at pointer frequency.
  const keys = d.keysByCol[target] || [];
  let rank = 0;
  while (rank < keys.length && keys[rank] <= frac) rank++;
  if (target === d.col && rank === d.rank) return;
  d.col = target;
  d.rank = rank;
  // Lane-growth preview: the drop cell already holds stickies and the lane has no spare row —
  // show where its bottom rule will land on release (one row below the current one), the same
  // live-pen guide the region resize draws.
  if (keys.length + 1 > d.rows) {
    const bottom = d.svgRect.top + bandTop[k] + bandH[k] + CFG.laneVpad / 2 + CFG.rowPitch;
    Object.assign(growGuide.style, {
      left: `${d.svgRect.left + 12}px`,
      width: `${Math.max(0, d.svgRect.width - 32)}px`,
      top: `${bottom}px`,
      display: "block",
    });
  } else {
    growGuide.style.display = "none";
  }
  colOf[d.id] = target;   // live preview through the shared applyLayout (box + its edges)
  yFrac[d.id] = frac;
  applyLayout();
}
function endStickyDrag(e) {
  if (stickyDrag && e.pointerId !== stickyDrag.pointerId) return;   // another finger lifting, not ours
  const g = e.currentTarget;
  g.removeEventListener("pointermove", dragSticky);
  g.removeEventListener("pointerup", endStickyDrag);
  g.removeEventListener("pointercancel", endStickyDrag);
  g.removeEventListener("lostpointercapture", endStickyDrag);
  growGuide.style.display = "none";
  const d = stickyDrag;
  stickyDrag = null;
  if (!d) return;
  if (!g.isConnected) return;  // detached by a board swap mid-drag — state dropped, nothing to commit
  // Undo the preview so moveTo re-derives the true origin (and records an honest undo entry).
  colOf[d.id] = d.fromCol;
  if (d.fromFrac === undefined) delete yFrac[d.id]; else yFrac[d.id] = d.fromFrac;
  // The `y` ordering key only means something in a *shared* cell (above/below the occupants);
  // a drop into an empty cell posts col-only so the box stays auto-placed on the grid.
  const frac = (d.keysByCol[d.col] || []).length > 0 ? d.frac : undefined;
  if (d.moved && (d.col !== d.fromCol || frac !== undefined)) {
    moveTo(d.id, d.col, frac);     // sets colOf/yFrac + applyLayout + posts — no snap-back
  } else {
    applyLayout();           // sub-threshold / no-op: snap the box back to where it started
    g.focus();               // the single click's promised focus, not left to browser defaults
    // The click's default focus already fired onfocus during pointerdown — while stickyDrag was
    // set, so placeConnectDot saw a busy board and hid the handle; the focus() above is then a
    // no-op (the box is already focused) and never re-shows it. Place it now the gesture is over,
    // so a plain click reveals the connect dot (F-edge-connect / #48 select-scoped affordance).
    placeConnectDot(g);
  }
}

