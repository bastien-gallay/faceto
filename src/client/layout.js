// ---- moving stickies (2D: along the timeline + within the lane band) --------
// A move never touches edges' endpoints (identity is `id`); it shifts a sticky along the shared
// column axis (x = col, snapped) and/or within its lane band (a stored `y` fraction — never a
// lane change, `type` selects the lane). Persistence is a kind:"move" comment; in log mode the
// server folds it into ElementMoved, in legacy mode the client replays comments so a move
// survives reloads.
let hoverId = null;                              // sticky under the cursor, target of ← / →
let baseCol = {}, baseCx = {}, cyOf = {}, kindOf = {}, colOf = {};
let dataY = {};  // element id -> the server-stored ordering key (data-y), undefined = unplaced
let yFrac = {};  // element id -> replayed/preview y key (undefined = fall back to dataY)
let bandTop = {}, bandH = {};   // lane -> its band interior's top / height, in SVG user units —
                                // read from the lane labels; render.rs is the single source of
                                // truth for the frame the `y` key lives in.
let colX = {};   // authored column -> its centre x, read from the rendered stickies.

// Snapshot the server-rendered layout: each sticky's authored column, lane, centre and stored
// ordering key, each column's centre x (one col = one x — render.rs guarantees it), and each
// lane's band interior.
function readLayout() {
  baseCol = {}; baseCx = {}; cyOf = {}; kindOf = {}; colOf = {}; colX = {};
  dataY = {}; bandTop = {}; bandH = {};
  document.querySelectorAll(".sticky").forEach((g) => {
    const col = +g.dataset.col, cx = +g.dataset.cx;
    baseCol[g.id] = col;
    baseCx[g.id] = cx;
    cyOf[g.id] = +g.dataset.cy;
    kindOf[g.id] = g.dataset.kind;
    colOf[g.id] = col;
    if (g.dataset.y !== undefined) dataY[g.id] = +g.dataset.y;
    colX[col] = cx;
  });
  document.querySelectorAll(".lane-label").forEach((t) => {
    bandTop[t.dataset.lane] = +t.dataset.bandTop;
    bandH[t.dataset.lane] = +t.dataset.bandH;
  });
}

// The centre x of an authored column. The pitch is uniform (one COL_W slot per col), so an empty
// column (a move into a gap) extrapolates exactly from any measured anchor.
function colCenter(col) {
  if (colX[col] !== undefined) return colX[col];
  const known = Object.keys(colX).map(Number);
  if (!known.length) return col * CFG.colW;
  return colX[known[0]] + (col - known[0]) * CFG.colW;
}

// The ordering key a sticky currently carries: its replayed/preview key first, then the
// server-stored one, else the neutral middle — the mirror of render.rs `model::y_key`.
const keyOf = (id) => (yFrac[id] !== undefined ? yFrac[id]
  : dataY[id] !== undefined ? dataY[id] : 0.5);
// A pixel-y inside a lane band → the [0,1] key it denotes (rounded like events::clamp_y).
const cyToFrac = (k, cy) => +(((cy - bandTop[k]) / bandH[k]).toFixed(4));

// Mirror of render.rs's grid placement, so previews and legacy replays land exactly where the
// authoritative render will: each (lane, col) cell's members sort by key (tie-break: the
// server-rendered order via cyOf) and the stack centres in the band on row-slot centres — a
// lone box sits mid-line whatever its key, never at a free position. Recomputed by applyLayout
// into `cyNow`; the renderer stays the single source of truth, this only reproduces it.
let cyNow = {};
function computeGrid() {
  cyNow = {};
  const cells = {};
  for (const id in colOf) (cells[kindOf[id] + ":" + colOf[id]] ||= []).push(id);
  for (const cell in cells) {
    const members = cells[cell].sort((a, b) => keyOf(a) - keyOf(b) || cyOf[a] - cyOf[b]);
    const k = kindOf[members[0]];
    const rows = Math.round(bandH[k] / CFG.rowPitch);
    const lead = (rows - members.length) / 2;
    members.forEach((id, rank) => {
      cyNow[id] = bandTop[k] + (lead + rank + 0.5) * CFG.rowPitch;
    });
  }
}
const effCy = (id) => (cyNow[id] !== undefined ? cyNow[id] : cyOf[id]);
// A moved sticky shifts by the gap between real column centres in x, and to its grid slot in y.
const centerOf = (id) => [baseCx[id] + (colCenter(colOf[id]) - colCenter(baseCol[id])), effCy(id)];

// Port of render.rs `edge_path`: a smooth connector anchored on the boxes' facing edges. `o1`/`o2`
// slide each anchor along its facing edge (Lever B fan-out) — Y for a left/right face, X for a
// top/bottom face — so siblings meeting one box on the same side spread out. o1=o2=0 is the classic
// centre-to-centre path.
function edgePath([x1, y1], [x2, y2], o1 = 0, o2 = 0) {
  const W = CFG.stickyW, H = CFG.stickyH, f = (n) => n.toFixed(1);
  if (Math.abs(x2 - x1) < W) {
    const s = y2 >= y1 ? 1 : -1, ax1 = x1 + o1, ax2 = x2 + o2,
      ay1 = y1 + s * H / 2, ay2 = y2 - s * H / 2, my = (ay1 + ay2) / 2;
    return `M${f(ax1)},${f(ay1)} C${f(ax1)},${f(my)} ${f(ax2)},${f(my)} ${f(ax2)},${f(ay2)}`;
  }
  const s = x2 >= x1 ? 1 : -1, ax1 = x1 + s * W / 2, ax2 = x2 - s * W / 2,
    ay1 = y1 + o1, ay2 = y2 + o2, mx = (ax1 + ax2) / 2;
  return `M${f(ax1)},${f(ay1)} C${f(mx)},${f(ay1)} ${f(mx)},${f(ay2)} ${f(ax2)},${f(ay2)}`;
}

// Port of render.rs Lever B fan-out: group each connector's two endpoints by (box, facing side),
// then fan a side's members apart, ordered by the far end's cross position. Returns the live edge
// list (skipping any whose endpoints aren't placed) and, per edge, the [srcOffset, dstOffset] the
// server applied — so the in-page move-nudge redraws connectors exactly as the authoritative render.
const FAN_SPREAD = 12;
function fanOffsets() {
  const edges = [...document.querySelectorAll(".edge")].filter(
    (p) => colOf[p.dataset.src] !== undefined && colOf[p.dataset.dst] !== undefined);
  const off = edges.map(() => [0, 0]);
  const groups = new Map();
  const push = (node, face, member) => {
    const k = node + ":" + face;
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(member);
  };
  edges.forEach((p, ei) => {
    const [sx, sy] = centerOf(p.dataset.src), [dx, dy] = centerOf(p.dataset.dst);
    if (Math.abs(dx - sx) >= CFG.stickyW) {
      push(p.dataset.src, dx > sx ? 0 : 1, [ei, true, dy]);
      push(p.dataset.dst, sx > dx ? 0 : 1, [ei, false, sy]);
    } else {
      push(p.dataset.src, dy > sy ? 2 : 3, [ei, true, dx]);
      push(p.dataset.dst, sy > dy ? 2 : 3, [ei, false, sx]);
    }
  });
  for (const [key, members] of groups) {
    const k = members.length;
    if (k < 2) continue;
    members.sort((a, b) => a[2] - b[2] || a[0] - b[0]);
    // Clamp the step so the extreme anchor stays on the box face (mirrors render.rs fan_offsets):
    // a horizontal face (0/1) fans in Y over stickyH, a vertical face (2/3) in X over stickyW.
    const face = +key.slice(key.lastIndexOf(":") + 1);
    const half = (face <= 1 ? CFG.stickyH : CFG.stickyW) / 2;
    const step = Math.min(FAN_SPREAD, 2 * half / (k - 1));
    members.forEach(([ei, isSrc], slot) => {
      off[ei][isSrc ? 0 : 1] = step * (slot - (k - 1) / 2);
    });
  }
  return { edges, off };
}

// Translate every sticky to its current column and redraw the connectors that touch it. The shift
// is measured against the real column centres read from the DOM (colCenter), so it is exact in Rows
// and correct under the variable column widths of Columns/Grid. This is the only feedback in legacy
// `model.json` mode, where a move never changes the model version and so triggers no authoritative
// server re-render (in log mode the re-render lands moments later and replaces this nudge).
function applyLayout() {
  computeGrid();
  document.querySelectorAll(".sticky").forEach((g) => {
    if (colOf[g.id] === undefined) return;
    const dx = colCenter(colOf[g.id]) - colCenter(baseCol[g.id]);
    const dy = effCy(g.id) - cyOf[g.id];
    g.setAttribute("transform",
      Math.abs(dx) > 0.05 || Math.abs(dy) > 0.05 ? `translate(${dx.toFixed(1)},${dy.toFixed(1)})` : "");
  });
  const { edges, off } = fanOffsets();
  edges.forEach((p, ei) => {
    p.setAttribute("d", edgePath(centerOf(p.dataset.src), centerOf(p.dataset.dst), off[ei][0], off[ei][1]));
  });
}

// Rebuild positions from the authored layout + every move comment, in order. Idempotent: each
// move carries an *absolute* target col, so re-applying the same comment converges (no drift).
// Against the server this is a no-op — it folds moves into the board via `ElementMoved` and
// `/comments` returns feedback only — so the loop iterates nothing; it still replays any
// offline-stashed moves (the `localStorage` fallback), riding on top of the authored layout.
function replayMoves() {
  for (const id in baseCol) colOf[id] = baseCol[id];
  yFrac = {};
  for (const c of comments) {
    if (c.kind !== "move" || colOf[c.elemId] === undefined) continue;
    if (c.col !== undefined) colOf[c.elemId] = c.col;
    if (c.y !== undefined) yFrac[c.elemId] = c.y;
    // Swaps are legacy (the 2D client no longer displaces an occupant), but a stashed offline
    // move from an old session may still carry one — keep replaying it faithfully.
    if (c.swapId !== undefined && colOf[c.swapId] !== undefined) colOf[c.swapId] = c.swapCol;
  }
  applyLayout();
}

// Move a sticky to an absolute target column and (optionally) a `y` fraction within its band.
// Shared by the ←/→ nudge (doMove) and drag-to-move — the single move contract. Nothing is
// displaced: two stickies may share a (lane, col) cell (they are simultaneous; the renderer
// stacks the unplaced ones), so the old force-swap is gone. A no-op if neither axis changes.
// Undo (Ctrl/Cmd+Z): each own move/rename pushes its inverse — one more POST, since the log is
// append-only truth (history is never rewritten, the undo is itself an event). `add` and `drop`
// have no inverse through this seam (ids are minted server-side and can't be re-asked), which is
// why remove keeps its own armed-confirm guard below.
const undoStack = [];
function undoLast() {
  const u = undoStack.pop();
  if (!u) { note("nothing to undo"); return; }
  if (u.kind === "move") {
    moveTo(u.id, u.col, u.y, false);   // false: applying an inverse must not record a new undo entry
  } else if (u.kind === "rename") {
    note(`undid rename of ${u.id}`);
    postComment({ elemId: u.id, kind: "rename", text: u.text, ts: new Date().toISOString(), status: "open" });
  }
}

function moveTo(id, target, frac, recordUndo = true) {
  if (colOf[id] === undefined) return;
  const from = colOf[id];
  if (target === from && frac === undefined) return;
  // Record the prior ordering key only when this move changes it — undoing a pure column nudge
  // must not pin an auto-stacked box as a side effect. For a box that had no stored key this
  // records the neutral 0.5, which sorts identically to "unplaced" (model::y_key), so the undo
  // genuinely restores the pre-placement behaviour.
  if (recordUndo) undoStack.push({ kind: "move", id, col: from, y: frac !== undefined ? keyOf(id) : undefined });
  // No fixed left edge: `col` is a global coordinate and the lane-title `+` (prepend) mints
  // negative cols, which the renderer draws on-board. Moving left grows the board left the same way.
  colOf[id] = target;
  if (frac !== undefined) yFrac[id] = frac;
  applyLayout();
  const dir = target < from ? "left" : target > from ? "right" : "within its lane";
  const c = { elemId: id, kind: "move", col: target, prevCol: from,
              text: dir === "within its lane" ? "moved within its lane" : `moved ${dir} to col ${target}`,
              ts: new Date().toISOString(), status: "open" };
  if (frac !== undefined) c.y = frac;
  note(`moved ${dir}`);
  postComment(c);
}

// Nudge a sticky one column left/right — the keyboard/arrow fast path.
function doMove(id, dir) {
  if (colOf[id] === undefined) return;
  moveTo(id, colOf[id] + (dir === "left" ? -1 : 1));
}

