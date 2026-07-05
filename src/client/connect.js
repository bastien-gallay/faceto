// ---- connect / disconnect edges (F-edge-connect) ----------------------------------------------
// Wire two existing boxes on a live board — today edges only enter through the bootstrap model, so
// a served board can't be connected. The affordance is a small live-pen dot on the SELECTED box's
// right border (select-scoped, per #48 — not the hover +/×/comment glyphs): drag a wire from it to
// another box to create a directed edge src→dst. Dropping on a box the source ALREADY points at
// removes that edge instead — one gesture, a toggle — and the live preview wire is blue to connect,
// red to cut, so the outcome is read before release. Keyboard parity: `e` arms "connect from the
// focused box", focus a target and Enter completes (same toggle), Esc cancels.
//
// Directed and faithful to the file: the edge is drag-source→drop-target (keyboard: armed-source→
// target), the same ordered pair model.json authors — no auto-orientation. Persisted as a
// kind:"connect"/"disconnect" comment folding to EdgeAdded/EdgeRemoved (events/comments.rs). No id
// is minted, so it rides the plain append path; the board re-renders on the round-trip like any
// other own edit.
const connectDot = $("#connect-dot");
const connectOverlay = $("#connect-overlay");
const connectPreview = $("#connect-preview");
let connectDrag = null;   // { src, pointerId, ax, ay, dst, decision } while dragging the wire, else null
let connecting = null;    // { src } while the keyboard path is armed, else null

// Which op a gesture from `src` onto `dst` commits, given the board's directed edges: "disconnect"
// if src already points at dst, "connect" if not, null if the pair is unusable (missing endpoint or
// a self-loop — the server rejects those too). Pure so the toggle rule lives in one testable place.
function connectDecision(edges, src, dst) {
  if (!src || !dst || src === dst) return null;
  return edges.some((e) => e.src === src && e.dst === dst) ? "disconnect" : "connect";
}

// The board's edges as directed {src,dst} pairs, read from the rendered connectors. Snapshotted at
// gesture start (edges don't move mid-drag) and passed to connectDecision.
function directedEdges() {
  return [...document.querySelectorAll(".edge")].map((p) => ({ src: p.dataset.src, dst: p.dataset.dst }));
}

// A smooth connector for the drag preview, drawn in client (screen) px in the fixed overlay. The
// board's edge_path lives in SVG user units and anchors on box faces; the preview rides the cursor,
// so it gets its own simpler horizontal-biased cubic. Purely cosmetic — the committed edge is
// re-rendered server-side.
function screenPath(x1, y1, x2, y2) {
  const mx = (x1 + x2) / 2, f = (n) => n.toFixed(1);
  return `M${f(x1)},${f(y1)} C${f(mx)},${f(y1)} ${f(mx)},${f(y2)} ${f(x2)},${f(y2)}`;
}

// Place / hide the connect handle on the currently-selected box, anchored on its right-border
// midpoint (the flow-out edge) — inside the edge, distinct from the + add glyph that floats just
// outside it. Hidden while any gesture owns the board (including a keyboard arm), so it never
// competes with an in-flight edit. Repositioned on board scroll (see the listener below) so the
// persistent handle tracks its box.
function placeConnectDot(g) {
  if (!g || gestureBusy()) { hideConnectDot(); return; }
  const r = g.getBoundingClientRect();
  connectDot._src = g.id;
  connectDot.style.left = `${r.right - 6}px`;
  connectDot.style.top = `${r.top + r.height / 2 - 6}px`;
  connectDot.classList.add("show");
}
function hideConnectDot() {
  connectDot.classList.remove("show");
  connectDot._src = null;
}

// Clear the live preview: hide the overlay wire and drop any target highlight. Idempotent.
function clearConnectPreview() {
  connectOverlay.classList.remove("show");
  connectPreview.classList.remove("cut");
  connectPreview.removeAttribute("d");
  document.querySelectorAll(".sticky.connect-target, .sticky.connect-cut")
    .forEach((g) => g.classList.remove("connect-target", "connect-cut"));
}

// Remove the drag listeners + clear the flag and preview, returning the drag that was in flight (or
// null). One teardown for commit (endConnect), Escape, and a mid-drag board swap (resetConnect) —
// the flag is nulled first so a following lostpointercapture is a no-op.
function teardownConnectDrag() {
  const d = connectDrag;
  connectDrag = null;
  if (d) {
    connectDot.removeEventListener("pointermove", moveConnect);
    connectDot.removeEventListener("pointerup", endConnect);
    connectDot.removeEventListener("pointercancel", endConnect);
    connectDot.removeEventListener("lostpointercapture", endConnect);
  }
  clearConnectPreview();
  return d;
}

// Press on the dot starts the wire drag. Pointer-capture (like startStickyDrag) so pointerup lands
// here even if released off a box; stopPropagation so the box's own move-drag doesn't also arm.
connectDot.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || gestureBusy()) return;
  const src = connectDot._src;
  if (!src || !document.getElementById(src)) return;
  e.preventDefault();
  e.stopPropagation();
  try { connectDot.setPointerCapture(e.pointerId); } catch { return; }
  const dot = connectDot.getBoundingClientRect();
  connectDrag = { src, pointerId: e.pointerId, edges: directedEdges(),
                  ax: dot.left + dot.width / 2, ay: dot.top + dot.height / 2,
                  dst: null, decision: null };
  hideGlyphs();
  connectOverlay.classList.add("show");
  connectPreview.setAttribute("d", screenPath(connectDrag.ax, connectDrag.ay, e.clientX, e.clientY));
  connectDot.addEventListener("pointermove", moveConnect);
  connectDot.addEventListener("pointerup", endConnect);
  connectDot.addEventListener("pointercancel", endConnect);
  connectDot.addEventListener("lostpointercapture", endConnect);
  note("drag to a box to connect — onto a linked box to disconnect, Esc cancels");
});

function moveConnect(e) {
  if (!connectDrag || e.pointerId !== connectDrag.pointerId) return;
  const d = connectDrag;
  const tgt = document.elementFromPoint(e.clientX, e.clientY)?.closest?.(".sticky");
  const dst = tgt && tgt.id !== d.src ? tgt.id : null;
  // The target ring + decision only change when the box under the cursor changes — recompute them
  // (and touch the DOM) only then, not on every pointermove, so a fast drag doesn't re-decide and
  // re-class at pointer frequency. `clearConnectPreview` is still the teardown safety net.
  if (dst !== d.dst) {
    if (d.dst) document.getElementById(d.dst)?.classList.remove("connect-target", "connect-cut");
    d.decision = dst ? connectDecision(d.edges, d.src, dst) : null;   // committed for endConnect
    if (dst) tgt.classList.add(d.decision === "disconnect" ? "connect-cut" : "connect-target");
    connectPreview.classList.toggle("cut", d.decision === "disconnect");
    d.dst = dst;
  }
  // The wire itself follows the cursor every move — snapped onto the target box when over one.
  let fx = e.clientX, fy = e.clientY;
  if (dst) {
    const tr = tgt.getBoundingClientRect();
    fx = tr.left + tr.width / 2; fy = tr.top + tr.height / 2;
  }
  connectPreview.setAttribute("d", screenPath(d.ax, d.ay, fx, fy));
}

// Release commits whatever the preview showed at the last move: a valid target → the decided op, an
// empty release / self / no-move → nothing. Honest — you commit exactly what you saw.
function endConnect(e) {
  const d = connectDrag;
  if (d && e && e.pointerId !== d.pointerId) return;   // another pointer lifting, not ours
  teardownConnectDrag();
  if (!d) return;
  if (d.dst && d.decision) postEdge(d.decision, d.src, d.dst);
  else note("");
}

// Post the decided edge op through the single comment seam. STRUCTURAL_KINDS keeps it out of the
// feedback badges; offline it stashes but can't re-render the graph (NOT_APPLIED_OFFLINE), same as
// add/drop.
function postEdge(kind, src, dst) {
  note(kind === "disconnect" ? `disconnected ${src} → ${dst}` : `connected ${src} → ${dst}`);
  postComment({ kind, src, dst, text: `${kind} ${src} → ${dst}`, ts: new Date().toISOString(), status: "open" });
}

// ---- keyboard path (arm from the focused box, Enter on a target completes) --------------------
// `e` on a focused box arms "connect from here"; the source wears a .connect-src ring while you
// navigate to a target and press Enter (completeConnect, called from the box's keydown before it
// would open the modal). Esc cancels. Same toggle as the drag.
function armConnect(id) {
  const g = id && document.getElementById(id);
  if (!g || (gestureBusy() && !connecting)) return;   // a re-arm from an armed state is allowed
  cancelConnect(true);   // re-arming from a new box replaces the old
  connecting = { src: id };
  g.classList.add("connect-src");
  hideConnectDot();   // the arm owns the board — the mouse handle must not compete with .connect-src
  note(`connect ${id} to… — focus a box and press Enter, Esc cancels`);
}
// Complete a keyboard-armed connect on the focused box. Returns true when it consumed the key (so
// the box's keydown knows not to also open the modal). Enter on the source itself just cancels.
function completeConnect(dst) {
  if (!connecting) return false;
  const src = connecting.src;
  cancelConnect(true);
  if (!dst || dst === src) { note(""); return true; }
  const decision = connectDecision(directedEdges(), src, dst);
  if (decision) postEdge(decision, src, dst); else note("");
  return true;
}
function cancelConnect(quiet) {
  if (!connecting) return;
  document.getElementById(connecting.src)?.classList.remove("connect-src");
  connecting = null;
  if (!quiet) note("");
}

// A board swap detaches every node: force-clear any in-flight drag or arm and the handle/overlay,
// so no dead id lingers and no orphaned .connect-src class survives on a recreated node. Called from
// bindStickies alongside the other gesture resets.
function resetConnect() {
  if (connectDrag) {
    try { connectDot.releasePointerCapture(connectDrag.pointerId); } catch {}
    teardownConnectDrag();
  }
  clearConnectPreview();
  hideConnectDot();
  cancelConnect(true);
}

// The handle is a screen-space fixed element placed from the box's rect, so anything that moves the
// box under it — a board scroll or a window resize (which reflows the board) — leaves it stranded.
// Re-place it against the still-selected box on both; skipped mid-drag (the wire owns the overlay)
// and when nothing is selected.
function repositionConnectDot() {
  if (connectDrag || !connectDot._src) return;
  const g = document.getElementById(connectDot._src);
  if (g && document.activeElement === g) placeConnectDot(g); else hideConnectDot();
}
$("#board").addEventListener("scroll", repositionConnectDot);
window.addEventListener("resize", repositionConnectDot);
