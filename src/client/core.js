const LS_KEY = "facetoComments";
const CFG = __CONFIG__;   // { colW, stickyW, stickyH, regionTab… } — geometry constants, from render.rs
// Structural ops change the board itself, not feedback on a box — kept out of the note badges,
// counts and prior-comment list (and never resynced offline; the log is the truth). `rename`
// belongs here: it edits the model's label, it is not a note — counting it as feedback painted
// a permanent has-note ring and leaked "[rename]" lines into the channel the next session reads.
const STRUCTURAL_KINDS = new Set([
  "move", "add", "drop", "rename", "region-add", "region-rename", "region-remove",
  "frontier-move", "phase-split",   // F-region-frontiers: resize = move a frontier, add = split a phase
]);
// Of those, add/drop/region-* have no client-side fallback: there is no id to mint, or nothing to
// remove/resize/rename locally without a server round-trip. `move` is the one exception — it
// still applies (see replayMoves), so it's the one structural kind excluded here. Derived from
// STRUCTURAL_KINDS (not hand-copied) so a future structural kind is safe-by-default: it starts
// "not applied offline" until someone deliberately teaches replayMoves to apply it too.
const NOT_APPLIED_OFFLINE = new Set([...STRUCTURAL_KINDS].filter((k) => k !== "move"));
let serverLive = false;
let comments = [];        // {elemId, kind, text, ts}  (kind:"move" carries col/prevCol/swap…)
let shownVersion = null;  // model hash the board currently displays — the diff baseline
let diffing = false;
let diffBase = null;      // baseline hash the current diff overlay is drawn against (if diffing)
// F-region-collapse: the viewer's reading lens — which regions are folded to a thin band. Held in
// its own localStorage key (never the comment stash, never the log): a fold is one viewer's view,
// gone on reload only if they clear it, never shared or committed. `boardSrc` threads it onto EVERY
// board fetch (plain, diff, own-edit refresh) so the fold survives each swap without per-call plumbing.
const LS_COLLAPSE = "facetoCollapsed";
const collapsedSet = () => { try { return new Set(JSON.parse(localStorage.getItem(LS_COLLAPSE) || "[]")); } catch { return new Set(); } };
const setCollapsed = (s) => localStorage.setItem(LS_COLLAPSE, JSON.stringify([...s]));
const boardSrc = (extra = {}) => {
  const params = { ...extra };
  const c = [...collapsedSet()];
  if (c.length) params.collapse = c.join(",");
  const q = new URLSearchParams(params).toString();
  return "/board.svg" + (q ? "?" + q : "");
};

const $ = (s) => document.querySelector(s);
const lsGet = () => { try { return JSON.parse(localStorage.getItem(LS_KEY) || "[]"); } catch { return []; } };
const lsSet = (a) => localStorage.setItem(LS_KEY, JSON.stringify(a));
const note = (msg, tone) => {
  const n = $("#note");
  n.textContent = msg || "";
  if (tone) n.dataset.tone = tone; else delete n.dataset.tone;   // tone classes the message (danger)
};
const liveVersion = async () => {
  try { const v = await fetch("/model-version", { cache: "no-store" }); if (v.ok) return (await v.json()).version; } catch {}
  return null;
};

