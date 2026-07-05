// Client-logic tests for the pieces of the board client that are pure enough to check without a
// DOM. The client is split into cohesive modules under src/client/ (F-js-modules) that the Rust
// build concatenates — byte-identical — into one embedded <script>; there is no module to import,
// so we read those files, concatenate them the same way, and lift a named helper out of the
// combined source to evaluate it in isolation. Keep the tested helpers self-contained (no closure
// captures beyond seeded globals, no string/comment braces on brace-lifted functions) so this stays
// honest. Run: `node tests/js/board-logic.test.mjs`.

import { readFileSync } from "node:fs";

// The client modules, in the SAME order src/render/html.rs concatenates them — so the source we
// lift from is exactly what ships. (Order is irrelevant for lifting a single named helper, but
// mirroring the build keeps this file the one place a reader learns the module layout.)
const MODULES = ["core", "layout", "drag", "edit", "region", "sync", "graph", "main"];
const source = MODULES
  .map((m) => readFileSync(new URL(`../../src/client/${m}.js`, import.meta.url), "utf8"))
  .join("\n");

// Lift `function <name>(...) { ... }` out of the source by counting braces from the first `{`. Good
// enough for the small pure helpers below; it would miscount on braces inside strings.
function lift(name) {
  const start = source.indexOf("function " + name + "(");
  if (start < 0) throw new Error(`function ${name}(...) not found in src/client/*.js`);
  let depth = 0;
  for (let i = source.indexOf("{", start); i < source.length; i++) {
    if (source[i] === "{") depth++;
    else if (source[i] === "}" && --depth === 0) {
      // eslint-disable-next-line no-new-func
      return new Function(`return (${source.slice(start, i + 1)})`)();
    }
  }
  throw new Error(`unbalanced braces lifting ${name}`);
}

// Lift a single-line `const <name> = <expr>;` arrow helper — the brace-counter above can't handle an
// arrow whose body carries an object literal (esc), so grab the initializer up to the line's end.
function liftLine(name) {
  const start = source.indexOf("const " + name + " = ");
  if (start < 0) throw new Error(`const ${name} = ... not found in src/client/*.js`);
  const eol = source.indexOf("\n", start);
  const rhs = source.slice(start + `const ${name} = `.length, eol).replace(/;\s*$/, "");
  // eslint-disable-next-line no-new-func
  return new Function(`return (${rhs})`)();
}

let failures = 0;
function check(name, cond) {
  console.log((cond ? "PASS" : "FAIL") + " — " + name);
  if (!cond) failures++;
}
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// --- revealScroll(el, view): the scroll delta (or null) to bring the focused box back inside the
// board viewport after a swap pins the pan. Rects are viewport/client coords {left,top,right,bottom}.
// The #46 scroll-preserve fix pins the pan, which can leave a just-moved focused sticky off-screen
// (review finding #1); revealScroll drives a minimal reveal only when the box actually left the frame.
const revealScroll = lift("revealScroll");
const view = { left: 0, top: 0, right: 800, bottom: 600 };

check("fully-visible box needs no reveal (pan preserved)",
  revealScroll({ left: 100, top: 100, right: 200, bottom: 160 }, view) === null);

check("box past the right edge scrolls right by the overflow",
  eq(revealScroll({ left: 780, top: 100, right: 900, bottom: 160 }, view), { dLeft: 100, dTop: 0 }));

check("box past the left edge scrolls left by the overflow",
  eq(revealScroll({ left: -30, top: 100, right: 70, bottom: 160 }, view), { dLeft: -30, dTop: 0 }));

check("box past the bottom edge scrolls down by the overflow",
  eq(revealScroll({ left: 100, top: 560, right: 200, bottom: 660 }, view), { dLeft: 0, dTop: 60 }));

check("box off both right and bottom reveals on both axes",
  eq(revealScroll({ left: 820, top: 620, right: 920, bottom: 700 }, view), { dLeft: 120, dTop: 100 }));

check("box exactly flush with the right edge is still visible",
  revealScroll({ left: 700, top: 100, right: 800, bottom: 160 }, view) === null);

// --- spotlightOwnerOf(hoverId, focusedStickyId, renamingId): which box the connector spotlight
// belongs to, in priority order — the box under the cursor, else the focused sticky, else the box
// being renamed inline; null clears it. The single precedence shared by onmouseenter/leave/focus/blur
// (review findings #2/#3/#4), so hovering another box while one is focused shows exactly one, and a
// blur onto empty space clears (deselect, #45). renaming keeps the edited box lit mid-rename (#5).
const owner = lift("spotlightOwnerOf");

check("cursor wins over focus and rename", owner("hover", "focused", "renaming") === "hover");
check("focused sticky wins when nothing is hovered", owner(null, "focused", "renaming") === "focused");
check("rename target lights when nothing hovered or focused (F2 mid-rename, #5)",
  owner(null, null, "renaming") === "renaming");
check("nothing hovered/focused/renaming clears the spotlight (deselect, #45)",
  owner(null, null, null) === null);
check("hover-only lights the hovered box", owner("hover", null, null) === "hover");
check("focus-only lights the focused box", owner(null, "focused", null) === "focused");

// --- esc(s): the HTML escaper used for connector chips + prior-comment lines in the modal. The
// three markup-significant characters must become entities; everything else passes through.
const esc = liftLine("esc");
check("esc turns </>& into entities", esc("a<b>&c") === "a&lt;b&gt;&amp;c");
check("esc coerces non-strings", esc(42) === "42");
check("esc leaves plain text untouched", esc("hotspot: order paid") === "hotspot: order paid");

// The geometry helpers read the shared CFG (from render.rs) and the DOM-measured column map; both
// are module globals in the browser, so seed them on globalThis for the lifted functions to close
// over. Values mirror render.rs (colW 210, stickyW 176) with a representative stickyH.
globalThis.CFG = { colW: 210, stickyW: 176, stickyH: 54 };

// --- edgePath([x1,y1],[x2,y2],o1,o2): port of render.rs edge_path — a smooth cubic anchored on the
// boxes' facing edges. Two branches: near boxes (|dx| < stickyW) leave via top/bottom faces, far
// boxes via left/right faces. Coordinates round to .1. Offsets slide each anchor along its face.
const edgePath = lift("edgePath");
check("far boxes route through left/right faces (horizontal branch)",
  edgePath([0, 0], [300, 0]) === "M88.0,0.0 C150.0,0.0 150.0,0.0 212.0,0.0");
check("near boxes route through top/bottom faces (vertical branch)",
  edgePath([0, 0], [50, 100]) === "M0.0,27.0 C0.0,50.0 50.0,50.0 50.0,73.0");
check("fan offsets slide the anchors along their faces",
  edgePath([0, 0], [50, 100], 6, -6) === "M6.0,27.0 C6.0,50.0 44.0,50.0 44.0,73.0");

// --- colCenter(col): the centre x of an authored column. Known columns read straight from the
// measured map; empty columns extrapolate from the first anchor at the uniform colW pitch, so a
// move into a gap lands exactly. With no columns measured yet it falls back to col*colW.
const colCenter = lift("colCenter");
globalThis.colX = { 0: 100, 1: 310 };
check("colCenter returns a measured column's centre", colCenter(0) === 100 && colCenter(1) === 310);
check("colCenter extrapolates an empty column to the right", colCenter(3) === 730);
check("colCenter extrapolates a negative (prepended) column", colCenter(-1) === -110);
globalThis.colX = {};
check("colCenter with nothing measured falls back to col*colW", colCenter(2) === 420);

// --- nearestCol(svgX): the column whose centre is nearest an SVG-space x — snap-to-grid for the
// drag. Searches the occupied span plus one column past each end (colCenter interpolates the gaps).
const nearestCol = lift("nearestCol");
globalThis.colCenter = colCenter;      // nearestCol calls colCenter — expose it to the lifted fn
globalThis.colX = { 0: 100, 1: 310 };
check("nearestCol snaps to the closest measured column", nearestCol(100) === 0 && nearestCol(310) === 1);
check("nearestCol picks the nearer of two columns", nearestCol(220) === 1 && nearestCol(190) === 0);
globalThis.colX = {};
check("nearestCol with nothing measured is column 0", nearestCol(500) === 0);

// --- cyToFrac(k, cy): a pixel-y inside a lane band → the [0,1] ordering key it denotes, rounded
// like events::clamp_y. Reads the per-lane band top/height measured from the rendered lane labels.
const cyToFrac = liftLine("cyToFrac");
globalThis.bandTop = { event: 100 };
globalThis.bandH = { event: 200 };
check("cyToFrac maps the band midpoint to 0.5", cyToFrac("event", 200) === 0.5);
check("cyToFrac maps the band top to 0", cyToFrac("event", 100) === 0);
check("cyToFrac maps the band bottom to 1", cyToFrac("event", 300) === 1);

if (failures) { console.error(`\n${failures} check(s) failed`); process.exit(1); }
console.log("\nall board-logic checks passed");
