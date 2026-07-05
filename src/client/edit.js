// ---- inline rename --------------------------------------------------------
// Edit a label where it sits: an input floats over the sticky's screen rect — the live pen
// writing directly on the box (DESIGN: the accent marks where you are acting). Double-click or
// F2 opens it; Enter commits a kind:"rename", Escape cancels. The same non-blank rule the server
// enforces (events::nonblank) is applied here, and an unchanged label is a no-op, so a slip never
// posts a dead event. `renaming` holds the open edit (and silences the board's move/remove keys).
// A region's label tab reuses the same editor and function (`region` true), keyed by `regionId`
// instead of `elemId` — a region isn't an element (F-container Stage 6). `frontierDrag` also gates
// this: a resize drag captures the pointer, but the keyboard (F2) isn't captured, so a rename must
// not open mid-drag either (review: the two gestures had no mutual exclusion).
let renaming = null;   // { id, original, region? } while editing, else null
function startRename(g, region) {
  if (!g || gestureBusy()) return;
  hideGlyphs();   // the edit owns the board — a lingering × / comment click must not land mid-rename
  const input = $("#rename-edit"), r = g.getBoundingClientRect();
  Object.assign(input.style,
    { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px`, display: "block" });
  input.value = (region ? g.dataset.label : g.dataset.hero) || "";
  renaming = region ? { id: g.dataset.region, original: input.value, region: true } : { id: g.id, original: input.value };
  input.focus();
  input.select();
}
function endRename(commit, refocus = true) {
  if (!renaming) return;
  const { id, original, region } = renaming, text = $("#rename-edit").value.trim();
  renaming = null;
  $("#rename-edit").style.display = "none";
  // refocus false = the blur came from a click elsewhere; returning focus to the box would fight
  // that click. Escape (keyboard) keeps refocus true so the box you were editing stays selected.
  if (!commit) { note("rename cancelled"); if (refocus && !region) document.getElementById(id)?.focus(); return; }
  if (!text) { note("rename needs a label — left unchanged"); return; }   // mirrors the server guard
  if (text === original.trim()) { note("no change"); return; }
  const ts = new Date().toISOString();
  if (region) {
    note("region renamed");
    postComment({ regionId: id, kind: "region-rename", text, ts, status: "open" });
    return;
  }
  note("renamed");
  undoStack.push({ kind: "rename", id, text: original.trim() });
  postComment({ elemId: id, kind: "rename", text, ts, status: "open" });
}

// ---- inline remove --------------------------------------------------------
// Delete / Backspace (or the × glyph) on the hovered or focused sticky drops it. Destructive and
// not undoable through this seam (the id can't be re-minted), so it arms first: the same gesture
// again within the window commits, Escape or the timeout keeps the box. In-place and in-register —
// no window.confirm shattering the glass. In log mode the server folds ElementRemoved and the
// reload shows the box gone (offline it is stashed, not applied — see postComment).
let removeArm = null;   // { id, t } while the confirm window is open, else null
function disarmRemove(quiet) {
  if (!removeArm) return;
  clearTimeout(removeArm.t);
  const id = removeArm.id;
  removeArm = null;
  $("#remove-x").classList.remove("armed", "show");   // .show too: doRemove places+shows it on arm
  document.getElementById(id)?.classList.remove("arming");
  note(quiet ? "" : "kept");   // quiet: the arm prompt must not linger after the window closes
}
function doRemove(id) {
  const g = document.getElementById(id);
  if (!g) return;
  if (removeArm?.id === id) {
    // A fast double-click would arm and confirm in one motion, before the eye reads the danger
    // prompt. Swallow a confirm that lands within 300ms of the arm (keeping the arm live), so a
    // real "click again" past that window still deletes.
    if (Date.now() - removeArm.at < 300) return;
    disarmRemove(true);
    note(`removed ${id}`);
    postComment({ elemId: id, kind: "drop", text: "removed inline", ts: new Date().toISOString(), status: "open" });
    return;
  }
  disarmRemove(true);   // arming a different box replaces the previous arm
  removeArm = { id, at: Date.now(), t: setTimeout(() => disarmRemove(true), 3000) };
  // The keyboard path (Delete) never hovered, so the corner × was never placed — it stranded
  // off-canvas, leaving no visible confirm signal. Position it at the box corner ourselves (the
  // same anchor the mouse hover uses) and arm the box itself with a red dashed ring, so the danger
  // reads on-element regardless of where — or whether — the glyph shows.
  const r = g.getBoundingClientRect();
  removeXBtn.at(r.right - 3, r.top - 9, id);
  $("#remove-x").classList.add("armed");
  g.classList.add("arming");
  note(`remove "${g.dataset.hero || id}"? — same gesture again to confirm, Esc to keep`, "danger");
}

// A hover glyph that acts on a stashed target id: a fixed ghost button (the × remove) that fades in
// at a box corner and stays reachable while the cursor travels onto it — the + add glyph's grace
// behaviour, factored so each glyph is a line of wiring, not a copy of the timer plumbing. `hoverId`
// is cleared the moment the cursor leaves the box for the glyph, so the target is stashed on show.
function graceGlyph(btn, act) {
  let hideT = null;
  btn.at = (x, y, targetId) => {
    clearTimeout(hideT);
    btn._target = targetId;
    btn.style.left = `${x}px`;
    btn.style.top = `${y}px`;
    btn.classList.add("show");
  };
  btn.hideSoon = () => { hideT = setTimeout(() => btn.classList.remove("show"), 140); };
  btn.addEventListener("mouseenter", () => clearTimeout(hideT));
  btn.addEventListener("mouseleave", () => btn.hideSoon());
  btn.addEventListener("click", () => {
    if (gestureBusy()) return;   // belt-and-braces: never fire an action over an in-progress gesture
    const id = btn._target;
    btn.classList.remove("show");
    if (id) act(id);
  });
  return btn;
}
const removeXBtn = graceGlyph($("#remove-x"), doRemove);
// Comment moved off the single click (which now just focuses) onto the c key + this glyph — so the
// modal is prose-only. Both open it via openModal (hoisted below).
const commentBtn = graceGlyph($("#comment-c"), (id) => openModal(document.getElementById(id)));

// ---- inline region remove -------------------------------------------------
// The delete twin for regions: `region-remove` was declared in STRUCTURAL_KINDS but had no gesture
// that emitted it. Same two-step armed flow as an element remove (destructive, and no client-side
// undo — the id can't be re-minted), reached two ways: the × glyph on region-tab hover, or Delete /
// Backspace on a focused tab. Keyed by `regionId`, posting kind:"region-remove" (serve.rs folds it
// to PhaseRemoved). Escape or the 3s timeout keeps the region.
let regionArm = null;   // { id, t } while a region's confirm window is open, else null
function disarmRegion(quiet) {
  if (!regionArm) return;
  clearTimeout(regionArm.t);
  const id = regionArm.id;
  regionArm = null;
  $("#region-x").classList.remove("armed", "show");
  document.querySelector(`.region-tab[data-region="${id}"]`)?.classList.remove("arming");
  if (!quiet) note("");
}
function armRegion(regionId, tab) {
  if (!regionId) return;
  if (regionArm?.id === regionId) {
    if (Date.now() - regionArm.at < 300) return;   // swallow a double-click confirm (see doRemove)
    disarmRegion(true);
    note("region removed");
    postComment({ regionId, kind: "region-remove", ts: new Date().toISOString(), status: "open" });
    return;
  }
  disarmRegion(true);   // arming a different region replaces the previous arm
  regionArm = { id: regionId, at: Date.now(), t: setTimeout(() => disarmRegion(true), 3000) };
  if (tab) {
    const r = tab.getBoundingClientRect();
    regionXBtn.at(r.right - 3, r.top - 9, regionId);   // the keyboard path never hovered — place it
    tab.classList.add("arming");
  }
  $("#region-x").classList.add("armed");
  note(`remove region "${tab?.dataset.label || regionId}"? — same gesture again to confirm, Esc to keep`, "danger");
}
const regionXBtn = graceGlyph($("#region-x"),
  (rid) => armRegion(rid, document.querySelector(`.region-tab[data-region="${rid}"]`)));

// Keyboard resize parity for regions: Shift+←/→ on a focused region tab nudges its right frontier
// (the `"end"` edge) in/out one column, posting the same kind:"frontier-move" the frontier drag
// posts — `replay`'s normalize re-borders the neighbour. Clamped so the phase keeps ≥1 column; a
// grow past the neighbour is safe (normalize absorbs it) but the last phase's grow extends the
// board. Region *create* stays a mouse gesture (hover a band to split) — disclosed in the sheet.
function resizeRegionByKey(regionId, delta) {
  const b = regionBounds[regionId];
  if (!b) return;
  const toCol = b.realTo + delta;   // nudge the TRUE to_col, not the clamped visible edge (fix: a
  if (toCol < b.from) { note("region already at its smallest"); return; }  // grow must not truncate
  note("region resized");
  postComment({ regionId, kind: "frontier-move", edge: "end", col: toCol, ts: new Date().toISOString(), status: "open" });
}

// ---- inline add (the + affordance) ----------------------------------------
// A ghost ink-blue + fades in on sticky hover (its right edge → add-after, same lane, col+1) and on
// lane-title hover (→ prepend; the server derives the left-edge col). Clicking it opens the *same*
// inline editor as rename to type the new label — Enter posts kind:"add", Escape cancels. The id is
// minted server-side and the non-blank guard is mirrored here, so a blank slip never posts.
// The region rail (F-container Stage 6) reuses the same + and editor for `region-add`
// (`region: true`, carrying the new region's `[fromCol, toCol]` instead of a lane/col).
let adding = null;       // { type, col?, prepend?, sx, sy } or { region: true, fromCol, toCol, sx, sy }
let plusHideT = null;
const plusBtn = $("#add-plus");
function showPlus(x, y, action) {
  clearTimeout(plusHideT);
  plusBtn._action = action;
  plusBtn.style.left = `${x}px`;
  plusBtn.style.top = `${y}px`;
  plusBtn.classList.add("show");
}
// A short grace on leave lets the cursor travel from the sticky/label onto the + without it
// vanishing; the + cancels the timer on its own mouseenter.
function hidePlusSoon() { plusHideT = setTimeout(() => plusBtn.classList.remove("show"), 140); }
plusBtn.addEventListener("mouseenter", () => clearTimeout(plusHideT));
plusBtn.addEventListener("mouseleave", hidePlusSoon);
plusBtn.addEventListener("click", () => { if (plusBtn._action) startAdd(plusBtn._action); });
function startAdd(action) {
  if (gestureBusy()) return;
  hideGlyphs();   // the edit owns the board (see startRename) — this also hides the + itself
  const input = $("#rename-edit");
  // The region-add box mirrors render.rs's own tab-sizing formula via CFG (Composable — render.rs
  // stays the single source of truth for this layout decision; see REGION_TAB_H etc. in render.rs).
  // The label isn't known yet (the user hasn't typed it), so width guesses a placeholder length —
  // same rough-preview spirit as the element box, which doesn't know the future label either.
  const w = action.region ? CFG.regionTabCharW * 10 + CFG.regionTabPad : CFG.stickyW;
  const h = action.region ? CFG.regionTabH : CFG.stickyH;
  Object.assign(input.style,
    { left: `${action.sx}px`, top: `${action.sy}px`, width: `${w}px`, height: `${h}px`, display: "block" });
  input.value = "";
  adding = action;
  input.focus();
}
function endAdd(commit) {
  if (!adding) return;
  const a = adding, text = $("#rename-edit").value.trim();
  adding = null;
  $("#rename-edit").style.display = "none";
  if (!commit) { note("add cancelled"); return; }
  if (!text) { note("add needs a label — nothing added"); return; }   // mirrors the server guard
  const ts = new Date().toISOString();
  // Split an existing phase in two (F-region-frontiers "add"): the server mints the right half's
  // id, `text` names it, the original keeps the left half.
  if (a.split) {
    note("phase split");
    postComment({ kind: "phase-split", regionId: a.regionId, atCol: a.atCol, text, ts, status: "open" });
    return;
  }
  // First phase on an empty board: create one spanning the whole board (region-add). Once a phase
  // exists the partition covers every column, so further phases come only from a split.
  if (a.region) {
    note("region added");
    postComment({ kind: "region-add", text, fromCol: a.fromCol, toCol: a.toCol, ts, status: "open" });
    return;
  }
  note("element added");
  const c = { kind: "add", type: a.type, text, ts, status: "open" };
  if (a.prepend) c.prepend = true; else c.col = a.col;
  postComment(c);
}
// Lane titles are always rendered (R), so the prepend + reaches even an empty board. Re-bound after
// every board swap, alongside the stickies.
function bindLaneAdders() {
  document.querySelectorAll(".lane-label").forEach((t) => {
    t.style.cursor = "pointer";
    t.onmouseenter = () => {
      const r = t.getBoundingClientRect();
      showPlus(r.right + 6, r.top + r.height / 2 - 11,
        { type: t.dataset.lane, prepend: true, sx: r.right + 14, sy: r.top - 6 });
    };
    t.onmouseleave = hidePlusSoon;
  });
}

