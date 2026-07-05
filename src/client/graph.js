let adj = {};          // id -> Set(neighbour id), built from the rendered edges
let edgesByNode = {};  // id -> [<path class="edge">], for hover highlighting
function buildGraph() {
  adj = {}; edgesByNode = {};
  document.querySelectorAll(".edge").forEach((p) => {
    const s = p.dataset.src, d = p.dataset.dst;
    (edgesByNode[s] ||= []).push(p); (edgesByNode[d] ||= []).push(p);
    (adj[s] ||= new Set()).add(d); (adj[d] ||= new Set()).add(s);
  });
}
function hoverOn(id) {
  $("#board").classList.add("dim");
  (edgesByNode[id] || []).forEach((p) => p.classList.add("hl"));
  document.getElementById(id)?.classList.add("adj");
  (adj[id] || []).forEach((n) => document.getElementById(n)?.classList.add("adj"));
}
function hoverOff() {
  $("#board").classList.remove("dim");
  document.querySelectorAll(".edge.hl").forEach((p) => p.classList.remove("hl"));
  document.querySelectorAll(".sticky.adj").forEach((g) => g.classList.remove("adj"));
}
// Which box the connector spotlight belongs to, in priority order: the box under the cursor, else
// the focused sticky, else the box being renamed inline (its editor holds focus, but the box is
// still the selection). null clears the spotlight. Pure so the precedence is testable and lives in
// exactly one place — the four sticky handlers below all recompute the owner through it.
function spotlightOwnerOf(hoverId, focusedStickyId, renamingId) {
  return hoverId || focusedStickyId || renamingId || null;
}
// Recompute the spotlight from the current owner: clear, then light whoever owns it. Called on every
// hover/focus transition so only one box is ever lit and an empty-space blur clears it (deselect).
function relightSpotlight() {
  hoverOff();
  const ae = document.activeElement;
  const focused = ae?.classList?.contains("sticky") ? ae.id : null;
  const renamingSticky = renaming && !renaming.region ? renaming.id : null;
  const id = spotlightOwnerOf(hoverId, focused, renamingSticky);
  if (id) hoverOn(id);
}
function bindStickies() {
  // A board swap detaches every old node: the hovered box never fires its mouseleave and a
  // captured drag never delivers its pointerup — clear the stale hover/gesture state here or a
  // dead id lingers in hoverId / the glyph stashes and a stuck drag flag gates every gesture.
  hoverId = null;
  hideGlyphs();
  hoverOff();
  if (stickyDrag) { stickyDrag = null; growGuide.style.display = "none"; }
  if (frontierDrag) { frontierDrag = null; dragGuide.style.display = "none"; }
  resetConnect();   // clear any in-flight wire drag / keyboard arm + the handle (nodes are new)
  buildGraph();
  readLayout();
  document.querySelectorAll(".sticky").forEach((g) => {
    // Single click just focuses / spotlights (select-then-edit — the calm gesture); double-click or
    // F2 renames in place. Comment moved off the click onto the c key + the comment glyph, so the
    // box's click is benign — no modal, no disambiguation timer. Grab-drag along the lane moves it.
    g.ondblclick = () => startRename(g);
    g.onpointerdown = startStickyDrag;
    g.onmouseenter = () => {
      hoverId = g.id; relightSpotlight();
      if (gestureBusy()) return;   // an open edit / drag owns the board — no action glyphs meanwhile
      const r = g.getBoundingClientRect();
      showPlus(r.right + 4, r.top + r.height / 2 - 11,
        { type: g.dataset.kind, col: +g.dataset.col + 1, sx: r.right + 12, sy: r.top });
      removeXBtn.at(r.right - 3, r.top - 9, g.id);    // × at the top-right corner
      commentBtn.at(r.left - 19, r.top - 9, g.id);    // comment at the top-left corner (clears the
                                                      // left-face arrowhead of any incoming edge)
    };
    g.onmouseleave = () => {
      // The spotlight follows the mouse, but a focused (or being-renamed) box keeps its claim:
      // drifting off a clicked box must not read as losing it. relightSpotlight re-lights that owner.
      if (hoverId === g.id) hoverId = null;
      relightSpotlight(); hidePlusSoon(); removeXBtn.hideSoon(); commentBtn.hideSoon();
    };
    // Keyboard parity with the mouse: focusing a sticky makes it the ← / → move target (same as
    // hover) and spotlights its connectors; Enter / Space opens its comment dialog.
    // Focusing a box spotlights its connectors; name how many, so the fan of ~30 lit edges on a
    // dense board has a legend ("5 connections") instead of being impressive but unreadable. Once
    // per deliberate selection, never on hover, and skipped on swap-refocus (see swapBoard).
    g.onfocus = () => {
      hoverId = g.id; relightSpotlight();
      placeConnectDot(g);   // select-scoped connect handle (F-edge-connect): shown on the focused box
      if (suppressFocusNote) return;
      const n = (adj[g.id] || new Set()).size;
      note(n ? `${n} connection${n === 1 ? "" : "s"}` : "no connections");
    };
    g.onblur = () => {
      // Focus left this box. Recompute the spotlight owner: if nobody is hovering, focused, or
      // renaming, it clears — so clicking empty space deselects (#45). The old `hoverId === g.id`
      // guard skipped clearing whenever the mouse had already drifted off, leaving it stuck.
      if (hoverId === g.id) hoverId = null;
      relightSpotlight();
      hideConnectDot();   // the handle follows the selection off this box
    };
    g.onkeydown = (e) => {
      // While a keyboard connect is armed, Enter on a box completes the wire to it (the toggle),
      // not the comment modal — completeConnect consumes the key. Otherwise Enter/Space comment.
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        if (connecting && completeConnect(g.id)) return;
        openModal(g);
      }
    };
  });
  bindLaneAdders();
  bindRegions();
}

function paint() {
  $("#status").textContent = serverLive ? "● live (server)" : "○ offline (localStorage)";
  $("#status").className = serverLive ? "live" : "";
  document.querySelectorAll(".sticky").forEach((g) => g.classList.remove("has-note"));
  // Structural ops (move/add/drop) aren't feedback on an existing box — keep them out of the note
  // badges, counts and prior-comment list. (Offline, a stashed `add`/`drop` lives only in
  // localStorage until export; there is no element to ring — or it's gone — see postComment.)
  const feedback = comments.filter((c) => !STRUCTURAL_KINDS.has(c.kind));
  const byEl = {};
  for (const c of feedback) if (c.elemId) (byEl[c.elemId] ||= []).push(c);
  for (const id in byEl) document.getElementById(id)?.classList.add("has-note");
  // A resolve comment quiets its hotspot right away (model `resolved:true` does it durably).
  for (const c of feedback) if (c.kind === "resolve") document.getElementById(c.elemId)?.classList.add("resolved");
  // A resolve is the act of closing, not a new open thread — counting it as open meant the
  // counter read one MORE open item right after the user resolved a hotspot.
  const open = feedback.filter((c) => c.status !== "resolved" && c.kind !== "resolve").length;
  $("#count").textContent = feedback.length ? `${feedback.length} comment(s), ${open} open` : "";
  window._byEl = byEl;
}

