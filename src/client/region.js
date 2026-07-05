// ---- region gestures (F-region-frontiers) ---------------------------------
// Regions are a contiguous partition, edited through their shared frontiers:
// • Resize  — drag a `.frontier` hit-line (render.rs) to a column boundary; posts kind:"frontier-move"
//             and `replay`'s normalize re-borders the neighbour atomically (no hole, no overlap).
// • Split   — hover a region's band (not a sticky, not a frontier) → a + at the hovered column;
//             clicking names the new right half, posting kind:"phase-split" (server mints its id).
// • Create  — on an EMPTY board only, the rail's + makes the first full-width phase (region-add).
// • Rename  — click / Enter on the label tab (keyed by regionId).
// • Merge   — the tab × / Delete removes a phase; the partition absorbs its columns into the
//             neighbour (region-remove + normalize). No directional-merge gesture in v1.
// `regionBounds` snapshots each *live* region's `[from, to]` (refreshed per board swap) for the
// keyboard resize + the split guard; a removed region's group is a diff-overlay ghost only.
let regionBounds = {};      // regionId -> { from, to }
let railLeft = {}, railRight = {};   // col -> its rail cell's left/right x, in SVG user units
let railCols = [];   // ascending list of covered columns; cached per swap (rail is fixed between swaps)
function readRegionRail() {
  railLeft = {}; railRight = {};
  document.querySelectorAll(".region-rail").forEach((r) => {
    const col = +r.dataset.col;
    railLeft[col] = +r.getAttribute("x");
    railRight[col] = railLeft[col] + +r.getAttribute("width");
  });
  railCols = Object.keys(railLeft).map(Number).sort((a, b) => a - b);
}
let _svg = null;
function svgLeftPx() { return (_svg ??= document.querySelector("#board svg")).getBoundingClientRect().left; }
// The column whose rail cell contains SVG-space x, or the nearest end column if x falls outside
// every rail cell. render.rs clamps each region's own bounds into the rail's covered range, so a
// column this resolves is always addressable.
function colAtSvgX(x) {
  if (!railCols.length) return null;
  for (const c of railCols) if (x >= railLeft[c] && x < railRight[c]) return c;
  return x < railLeft[railCols[0]] ? railCols[0] : railCols[railCols.length - 1];
}
// The column *boundary* nearest SVG-space x, as { b, x }. Boundaries number one more than columns:
// each column's left edge is boundary `b = col`; the last column's right edge is `b = last + 1`
// (the board's right end). A frontier drag snaps to these, so it can reach the right board edge —
// which a column-only snap (colAtSvgX clamps to the last column) never could.
function boundaryAtSvgX(x) {
  if (!railCols.length) return null;
  const last = railCols[railCols.length - 1];
  let best = { b: railCols[0], x: railLeft[railCols[0]] };
  for (const c of railCols) {
    if (Math.abs(railLeft[c] - x) < Math.abs(best.x - x)) best = { b: c, x: railLeft[c] };
  }
  if (Math.abs(railRight[last] - x) < Math.abs(best.x - x)) best = { b: last + 1, x: railRight[last] };
  return best;
}
// The SVG-space x of boundary `b` (b == last+1 is the board's right edge).
function boundaryX(b) {
  const last = railCols[railCols.length - 1];
  return b > last ? railRight[last] : railLeft[b];
}
const dragGuide = $("#region-drag-guide");
let frontierDrag = null;   // { regionId, edge, baseCol, svgLeft, pointerId, b? } while dragging
// Pointer Events + setPointerCapture (the proven region-drag pattern): once captured every event —
// including pointerup off-window — is delivered to the line, so the drag can never leak.
function startFrontierDrag(e) {
  if (e.button !== 0 || gestureBusy()) return;
  const line = e.currentTarget;
  const region = line.dataset.region, edge = line.dataset.edge, rb = regionBounds[region];
  // A folded band emits no `.region-rail` for its columns, so the boundary-snap math
  // (boundaryAtSvgX/boundaryX, built from railCols) can't resolve a frontier that *touches* one —
  // a commit there would POST a resize the viewer never intended, turning the read-only collapse
  // lens into a destructive model edit. But *only* the touching frontiers are ambiguous: a frontier
  // between two expanded regions stays fully addressable while some *other* region is folded, so it
  // must keep working (the fold is a per-viewer lens, not a board-wide edit lock). An internal
  // boundary is owned by the left region's "end"; the leftmost "start" touches only its own region.
  // So block iff this frontier's own region or its right neighbour is collapsed.
  const collapsed = collapsedSet();
  const rightId = rb && Object.keys(regionBounds).find((k) => regionBounds[k].from === rb.to + 1);
  if (collapsed.has(region) || (edge !== "start" && rightId && collapsed.has(rightId))) {
    note("expand this region (z) to resize its frontier");
    return;
  }
  e.preventDefault();
  const svgLeft = svgLeftPx();
  // Capture before any state — a throw here must not leave a frontierDrag flag (or the guide)
  // behind to gate every other gesture.
  try { line.setPointerCapture(e.pointerId); } catch { return; }
  // Clamp the drag so a frontier can never cross its neighbours (the guard the old region-edge drag
  // enforced): dragging past a neighbour would otherwise crush that phase to one column, or — for
  // the leftmost "start" — reorder the whole partition. An "end" frontier stays within (its own
  // start, +1) and the right neighbour's far edge (or the board's right rail end when it is last);
  // the leftmost "start" stays within the board's left rail end and its own to_col (so the first
  // region keeps ≥1 column and stays leftmost).
  const railMin = railCols[0], railMax = railCols[railCols.length - 1] + 1;
  let clampMin, clampMax;
  if (edge === "start") {
    clampMin = railMin; clampMax = rb ? rb.to : railMax;
  } else {
    clampMin = rb ? rb.from + 1 : railMin;
    const right = rb && Object.values(regionBounds).find((n) => n.from === rb.to + 1);
    clampMax = right ? right.to : railMax;
  }
  frontierDrag = { regionId: region, edge, baseCol: +line.dataset.col, svgLeft, pointerId: e.pointerId, clampMin, clampMax };
  const bandRect = line.getBoundingClientRect();   // the frontier line already spans band_top..band_bot
  Object.assign(dragGuide.style, { top: `${bandRect.top}px`, height: `${bandRect.height}px`, display: "block" });
  line.addEventListener("pointermove", dragFrontier);
  line.addEventListener("pointerup", endFrontierDrag);
  line.addEventListener("pointercancel", endFrontierDrag);
}
function dragFrontier(e) {
  if (!frontierDrag || e.pointerId !== frontierDrag.pointerId) return;   // see dragSticky
  const bnd = boundaryAtSvgX(e.clientX - frontierDrag.svgLeft);
  if (!bnd) return;
  const b = Math.max(frontierDrag.clampMin, Math.min(frontierDrag.clampMax, bnd.b));
  frontierDrag.b = b;
  dragGuide.style.left = `${frontierDrag.svgLeft + boundaryX(b)}px`;
}
function endFrontierDrag(e) {
  if (frontierDrag && e.pointerId !== frontierDrag.pointerId) return;   // another finger lifting, not ours
  e.currentTarget.removeEventListener("pointermove", dragFrontier);
  e.currentTarget.removeEventListener("pointerup", endFrontierDrag);
  e.currentTarget.removeEventListener("pointercancel", endFrontierDrag);
  dragGuide.style.display = "none";
  const d = frontierDrag;
  frontierDrag = null;
  if (!d || d.b === undefined) return;   // pointerdown with no drag (or a cancel) — nothing to commit
  if (d.b === d.baseCol) { note("no change"); return; }
  // A frontier at boundary `b` sits between columns b-1 and b: its left phase ends at b-1, its right
  // phase starts at b. So a "start" edge posts the new from_col (= b); every other edge posts the new
  // to_col (= b-1). normalize re-borders the neighbour from that single border.
  const col = d.edge === "start" ? d.b : d.b - 1;
  note("region resized");
  postComment({ regionId: d.regionId, kind: "frontier-move", edge: d.edge, col, ts: new Date().toISOString(), status: "open" });
}
// Re-bound after every board swap, alongside the stickies and lane adders.
function bindRegions() {
  readRegionRail();
  regionBounds = {};
  document.querySelectorAll(".region:not(.removed)").forEach((g) => {
    // `to` is the clamped (visible) bound the drag snaps against; `realTo` is the true stored
    // to_col the keyboard resize nudges (may run past the last visible column).
    regionBounds[g.dataset.region] = { from: +g.dataset.fromCol, to: +g.dataset.toCol, realTo: +g.dataset.realTo };
  });
  // Create the FIRST phase (empty board): the rail shows through everywhere, its + spans the whole
  // board. Once any phase exists the partition covers every column, so the rail is fully painted
  // over and further phases come only from a split.
  if (!Object.keys(regionBounds).length && railCols.length) {
    const lo = railCols[0], hi = Math.max(railCols[railCols.length - 1], lo + 1);   // ≥1 col wide (valid_span)
    document.querySelectorAll(".region-rail").forEach((r) => {
      r.onmouseenter = () => {
        const rect = r.getBoundingClientRect();
        showPlus(rect.left + rect.width / 2 - 11, rect.top + 4,
          { region: true, fromCol: lo, toCol: hi, sx: rect.left + 2, sy: rect.top + 2 });
      };
      r.onmouseleave = hidePlusSoon;
    });
  }
  document.querySelectorAll(".region-tab").forEach((g) => {
    g.onclick = () => startRename(g, true);
    g.ondblclick = () => startRename(g, true);   // parity with a sticky's dblclick/F2 rename
    g.onkeydown = (e) => {
      if (e.key === "Enter" || e.key === " ") { e.preventDefault(); startRename(g, true); }
      else if (e.key === "Delete" || e.key === "Backspace") { e.preventDefault(); e.stopPropagation(); armRegion(g.dataset.region, g); }
      else if ((e.key === "z" || e.key === "Z") && !e.ctrlKey && !e.metaKey) { e.preventDefault(); e.stopPropagation(); toggleCollapse(g.dataset.region); }   // bare z folds; Cmd/Ctrl+Z must reach the document undo handler
      else if (e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault(); e.stopPropagation();
        resizeRegionByKey(g.dataset.region, e.key === "ArrowRight" ? 1 : -1);
      }
    };
    g.onmouseenter = () => {
      if (gestureBusy()) return;   // a drag / edit owns the board — no region × meanwhile
      const r = g.getBoundingClientRect();
      regionXBtn.at(r.right - 3, r.top - 9, g.dataset.region);
    };
    g.onmouseleave = () => regionXBtn.hideSoon();
  });
  // The disclosure triangle (▸/▾ in the tab's left gutter) toggles the fold. It lives inside the
  // region-tab group, so stopPropagation keeps the click off the tab's rename gesture.
  document.querySelectorAll(".region-collapse").forEach((t) => {
    t.onclick = (e) => { e.stopPropagation(); toggleCollapse(t.dataset.region); };
    t.ondblclick = (e) => e.stopPropagation();   // dblclick doesn't inherit the click's stopPropagation — keep a fold double-tap off the tab's rename
  });
  // Split affordance: a + follows the cursor over a region's band, snapped to the column boundary a
  // split would create. Guarded to columns strictly inside the phase (a split at its very start is a
  // no-op, and leaves the label tab's corner alone). Frontiers and stickies paint over the band, so
  // hovering a boundary gives the resize cursor and hovering a sticky its own glyphs — the + shows
  // only in the open band, exactly where a new boundary makes sense.
  document.querySelectorAll(".region:not(.removed)").forEach((g) => {
    g.onmousemove = (e) => {
      if (gestureBusy()) return;
      const b = regionBounds[g.dataset.region];
      if (!b) return;
      const sl = svgLeftPx();   // one forced layout read per move, not three (invariant this handler)
      const col = colAtSvgX(e.clientX - sl);
      if (col === null || col <= b.from || col > b.to) { hidePlusSoon(); return; }
      showPlus(sl + railLeft[col] - 11, e.clientY - 11,
        { split: true, regionId: g.dataset.region, atCol: col, sx: sl + railLeft[col] + 4, sy: e.clientY - 6 });
    };
    g.onmouseleave = hidePlusSoon;
  });
  document.querySelectorAll(".frontier").forEach((line) => {
    line.onpointerdown = startFrontierDrag;
  });
}

