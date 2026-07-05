// Client-logic tests for the pieces of src/template.html that are pure enough to check without a
// DOM. The board client is an embedded, dependency-free page (served whole via include_str!), so
// there is no module to import — instead we lift a named function out of the template by
// brace-matching and evaluate it in isolation. Keep the tested functions self-contained (no closure
// captures, no string/comment braces) so this stays honest. Run: `node tests/js/board-logic.test.mjs`.

import { readFileSync } from "node:fs";

const template = readFileSync(new URL("../../src/template.html", import.meta.url), "utf8");

// Lift `function <name>(...) { ... }` out of the template source by counting braces from the first
// `{`. Good enough for the small pure helpers below; it would miscount on braces inside strings.
function lift(name) {
  const start = template.indexOf("function " + name + "(");
  if (start < 0) throw new Error(`function ${name}(...) not found in template.html`);
  let depth = 0;
  for (let i = template.indexOf("{", start); i < template.length; i++) {
    if (template[i] === "{") depth++;
    else if (template[i] === "}" && --depth === 0) {
      // eslint-disable-next-line no-new-func
      return new Function(`return (${template.slice(start, i + 1)})`)();
    }
  }
  throw new Error(`unbalanced braces lifting ${name}`);
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

if (failures) { console.error(`\n${failures} check(s) failed`); process.exit(1); }
console.log("\nall board-logic checks passed");
