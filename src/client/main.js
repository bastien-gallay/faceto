// Escapes all five markup-significant chars — the quote pair matters because esc feeds attribute
// contexts too (a link `href="…"`), not just text: a `"` in an authored URL (F-element-links) would
// otherwise break out of the attribute and inject a live handler. Mirrors the Rust `esc`
// (src/render/text.rs), so the client and server escape the same set.
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#x27;" }[c]));
// A sticky's `data-links` (newline-joined URLs, F-element-links) → clickable chips for the modal.
// Pure and side-effect-free (tested in tests/js). Only http/https/mailto open as anchors — any
// other scheme (a `javascript:` URL a hand-authored model.json might carry) renders as inert text,
// never a live link. `rel="noopener noreferrer"` and `target="_blank"` keep the board tab isolated.
function linkChips(raw) {
  return (raw || "")
    .split("\n")
    .map((u) => u.trim())
    .filter(Boolean)
    .map((u) =>
      /^(https?|mailto):/i.test(u)
        ? `<a class="chip" href="${esc(u)}" target="_blank" rel="noopener noreferrer">${esc(u)}</a>`
        : `<span class="chip">${esc(u)}</span>`,
    )
    .join("");
}
function openModal(g) {
  if (!g) return;   // a stale hoverId / glyph target can name a box a board swap removed
  const id = g.id;
  $("#m-id").textContent = id;
  $("#m-label").textContent = g.dataset.hero || "";
  $("#m-detail").textContent = g.dataset.detail || "";
  const links = linkChips(g.dataset.links);
  $("#m-links").innerHTML = links ? "links: " + links : "";
  // Relationships, the keyboard/non-hover way to read a box's connectors.
  const rel = [...(adj[id] || [])].map((n) => {
    const h = document.getElementById(n)?.dataset.hero || n;
    return `<span class="chip"><b>${esc(n)}</b> ${esc(h)}</span>`;
  }).join("");
  $("#m-rel").innerHTML = rel ? "connected to: " + rel : "";
  $("#m-text").value = "";
  $("#m-kind").value = "comment";
  // Resolve only makes sense on a hotspot or an element carrying an open question — hide it elsewhere
  // (the reset to "comment" above means a hidden option is never the live selection).
  const hasQuestion = (window._byEl?.[id] || []).some((c) => c.kind === "question");
  const resolveOpt = $("#m-kind option[value='resolve']");
  resolveOpt.hidden = resolveOpt.disabled = !(g.dataset.kind === "hotspot" || hasQuestion);
  const prior = (window._byEl?.[id] || []).map((c) => `<div>[${esc(c.kind)}] ${esc(c.text)}</div>`).join("");
  $("#m-prior").innerHTML = prior ? "<b>prior:</b>" + prior : "";
  $("#modal").returnValue = "cancel";
  $("#modal").showModal();
  window._activeId = id;
}

bindStickies();

$("#modal").addEventListener("close", () => {
  if ($("#modal").returnValue !== "save") return;
  const text = $("#m-text").value.trim();
  if (!text) return;
  const ts = new Date().toISOString();
  postComment({ elemId: window._activeId, kind: $("#m-kind").value, text, ts, status: "open" });
});

// Hover or focus a sticky and act on it from the keyboard — the keyboard-fast path, parity with
// the mouse: ← / → nudge a column, F2 renames in place, a / Insert adds after it, c opens its
// comment, Delete / Backspace removes. Skipped while the modal or an inline editor owns the
// keyboard, or while typing in a field.
// One inline editor, two modes: `adding` posts kind:"add", otherwise it commits a rename.
$("#rename-edit").addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); adding ? endAdd(true) : endRename(true); }
  else if (e.key === "Escape") { e.preventDefault(); adding ? endAdd(false) : endRename(false); }
  e.stopPropagation();   // keep edit keystrokes from reaching the board-level handler below
});
// Blur = abandon, not commit: clicking away from a half-typed label must not silently write it
// (only Enter commits). Mirrors the "Escape cancels" model used by every other gesture. The blur
// cancel does not steal focus back (refocus=false), so the click that caused it lands where aimed.
$("#rename-edit").addEventListener("blur", () => { adding ? endAdd(false) : endRename(false, false); });
// The keyboard acts on the focused sticky first, the hovered one as fallback. Focus survives
// mouse drift and scrolling; hover-only targeting silently dropped every keystroke the moment
// the cursor slipped off a clicked box — the exact focus the click had just promised.
function keyTarget() {
  const ae = document.activeElement;
  return ae?.classList?.contains("sticky") ? ae.id : hoverId;
}
document.addEventListener("keydown", (e) => {
  // Undo is the one modified shortcut we own: Ctrl/Cmd+Z appends the inverse of the last own
  // move/rename. Text fields keep their native undo; Shift (redo) passes through untouched.
  if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && (e.key === "z" || e.key === "Z")) {
    const ae = document.activeElement;
    if ($("#modal").open || gestureBusy() || (ae && /^(INPUT|TEXTAREA|SELECT)$/.test(ae.tagName))) return;
    e.preventDefault();
    undoLast();
    return;
  }
  if (e.metaKey || e.ctrlKey || e.altKey) return;   // never shadow Cmd/Ctrl shortcuts (copy, back…)
  // The shortcut sheet is the one key that needs no focused box — the discoverable home for the
  // whole keyboard vocabulary (also reachable via the header "?"). Escape closes it natively.
  if (e.key === "?" && !$("#modal").open && !$("#help").open) {
    const el = document.activeElement;
    if (!(el && /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))) { e.preventDefault(); $("#help").showModal(); return; }
  }
  if (e.key === "Escape" && removeArm) { e.preventDefault(); disarmRemove(); return; }
  if (e.key === "Escape" && regionArm) { e.preventDefault(); disarmRegion(); return; }
  if (e.key === "Escape" && connecting) { e.preventDefault(); cancelConnect(); return; }
  // A live wire drag bails like the other drags: null the flag first so the pointer release's
  // lostpointercapture is a no-op, then snap the preview away.
  if (e.key === "Escape" && connectDrag) {
    e.preventDefault();
    const d = connectDrag;
    try { connectDot.releasePointerCapture(d.pointerId); } catch {}
    teardownConnectDrag();
    note("");
    return;
  }
  // Escape bails out of a live drag before anything else: snap back, release the pointer,
  // nothing posted. Clearing the flag first makes the release's lostpointercapture a no-op.
  if (e.key === "Escape" && (stickyDrag || frontierDrag)) {
    e.preventDefault();
    if (stickyDrag) {
      const d = stickyDrag, g = document.getElementById(d.id);
      stickyDrag = null;
      growGuide.style.display = "none";
      colOf[d.id] = d.fromCol;
      if (d.fromFrac === undefined) delete yFrac[d.id]; else yFrac[d.id] = d.fromFrac;
      try { g?.releasePointerCapture(d.pointerId); } catch {}
      applyLayout();
      g?.focus();
    } else {
      const d = frontierDrag;
      frontierDrag = null;
      dragGuide.style.display = "none";
      const line = document.querySelector(`.frontier[data-region="${d.regionId}"][data-edge="${d.edge}"]`);
      try { line?.releasePointerCapture(d.pointerId); } catch {}
    }
    return;
  }
  // Re-arm a keyboard connect from a different box without pressing Esc first: an armed `connecting`
  // makes gestureBusy() true, which would otherwise swallow the `e` branch below. armConnect replaces
  // the previous source. (This is above the gestureBusy guard precisely so the arm doesn't block it.)
  if ((e.key === "e" || e.key === "E") && connecting) {
    const armAe = document.activeElement;
    if (armAe && /^(INPUT|TEXTAREA|SELECT)$/.test(armAe.tagName)) return;
    const t = keyTarget();
    if (t) { e.preventDefault(); armConnect(t); }
    return;
  }
  // gestureBusy: a drag captures the pointer but not the keyboard, so F2 or c could otherwise
  // fire mid-drag (review: no gesture guarded against the other's in-progress state).
  const target = keyTarget();
  if ($("#modal").open || gestureBusy() || !target) return;
  const ae = document.activeElement;
  if (ae && /^(INPUT|TEXTAREA|SELECT)$/.test(ae.tagName)) return;
  if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
    e.preventDefault();
    doMove(target, e.key === "ArrowLeft" ? "left" : "right");
  } else if (e.key === "F2") {
    e.preventDefault();
    startRename(document.getElementById(target));
  } else if (e.key === "c" || e.key === "C") {
    e.preventDefault();
    openModal(document.getElementById(target));
  } else if (e.key === "Delete" || e.key === "Backspace") {
    e.preventDefault();
    doRemove(target);
  } else if (e.key === "a" || e.key === "A" || e.key === "Insert") {
    // The keyboard twin of the hover + glyph: a new element one column to the right, same lane.
    // Same editor, same endAdd(kind:"add"); the id is minted server-side. Mirrors the mouse anchor
    // (r.right + 12, r.top) so the inline field opens exactly where the click path would place it.
    e.preventDefault();
    const g = document.getElementById(target);
    if (!g) return;   // a stale hoverId can name a box a board swap removed (parity with F2/c above)
    const r = g.getBoundingClientRect();
    startAdd({ type: g.dataset.kind, col: +g.dataset.col + 1, sx: r.right + 12, sy: r.top });
  } else if (e.key === "e" || e.key === "E") {
    // Arm "connect from this box" (F-edge-connect): the keyboard twin of dragging the connect dot.
    // Focus a target and Enter completes (toggle: disconnect if already linked); Esc cancels.
    e.preventDefault();
    armConnect(target);
  }
});

$("#refresh").addEventListener("click", load);
$("#plain").addEventListener("click", showPlain);
$("#help-btn").addEventListener("click", () => $("#help").showModal());
$("#help-close").addEventListener("click", () => $("#help").close());
$("#help").addEventListener("click", (e) => { if (e.target === $("#help")) $("#help").close(); });   // backdrop
$("#export").addEventListener("click", async () => {
  const data = JSON.stringify(comments, null, 2);
  try { await navigator.clipboard.writeText(data); } catch {}
  const blob = new Blob([data], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = "faceto-comments.json";
  a.click();
});

load();
