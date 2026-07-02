//! Render a board model into a static SVG and an interactive HTML page.
//!
//! Deterministic, pure std. The colour grammar (one type → one colour → one lane) and the
//! whole visual language are ported faithfully from the original Python harness.

use crate::model::{is_pivotal, Edge, Element, Model};
use std::collections::{HashMap, HashSet};

// Canonical lane order (top → bottom). `command` and `hotspot` are deepened from their classic
// event-storming swatches so white label text clears WCAG 4.5:1.
const LANES: [&str; 8] = [
    "actor",
    "command",
    "aggregate",
    "event",
    "policy",
    "readmodel",
    "external",
    "hotspot",
];

/// Each lane's id-mint prefix, index-aligned with `LANES`. `actor`/`aggregate` both start with
/// 'a', so actor takes 'X' and external takes 'G'. This is the single source of truth for
/// prefixes — `serve::id_prefix` reads it rather than re-listing the grammar.
const LANE_PREFIXES: [char; 8] = ['X', 'C', 'A', 'E', 'P', 'R', 'G', 'H'];

/// The id prefix for a lane `type`, or `None` if it is not one of the 8 lanes.
pub fn lane_prefix(kind: &str) -> Option<char> {
    LANES
        .iter()
        .position(|&l| l == kind)
        .map(|i| LANE_PREFIXES[i])
}

/// A lane's vertical rank in the fixed 8-lane grammar (`actor` = 0 … `hotspot` = 7). Used as the
/// y-band when ordering a crowded cell's members by their edge neighbours (F-edge-routing Lever A).
/// An unknown kind is never one of the 8 lanes, so it sorts to the top — harmless, never panics.
fn lane_index(kind: &str) -> usize {
    LANES.iter().position(|&l| l == kind).unwrap_or(0)
}

fn colour(kind: &str) -> &'static str {
    match kind {
        "actor" => "#FCEFA1",
        "command" => "#1A6FAE",
        "aggregate" => "#FFD23F",
        "event" => "#FF9F1C",
        "policy" => "#C39BD3",
        "readmodel" => "#6FCF97",
        "external" => "#F2A0C9",
        "hotspot" => "#C0392B",
        _ => "#cccccc",
    }
}

fn text_dark(kind: &str) -> bool {
    matches!(
        kind,
        "actor" | "aggregate" | "event" | "policy" | "readmodel" | "external"
    )
}

const RESOLVED_FILL: &str = "#D9DEE3";
const EDGE_FLOW: &str = "#9AA7B0";
const EDGE_HOTSPOT: &str = "#C39086";
// Muted axis + phase-band labels. Darkened from the old #90a4ae (≈2.6:1, fails AA) to clear WCAG
// 4.5:1 on the #fbfbfd board (≈5.3:1). These labels *name* the lane grammar — they are structure,
// not decoration, so they must be readable.
const AXIS_LABEL: &str = "#5b6b75";

fn diff_colour(s: &str) -> &'static str {
    match s {
        "added" => "#27ae60",
        "removed" => "#EB5757",
        "changed" | "moved" => "#E59500",
        _ => "#999999",
    }
}

fn diff_badge(s: &str) -> Option<&'static str> {
    match s {
        "added" => Some("+"),
        "removed" => Some("\u{2013}"), // en dash
        "changed" => Some("\u{2260}"), // ≠
        "moved" => Some("\u{2192}"),   // →
        _ => None,
    }
}

/// Map a region's diff verdict (added / removed / renamed / resized) onto the element-diff colour +
/// badge vocabulary, so a changed region speaks the same visual language as a changed sticky: a
/// rename reads like a relabel (`≠`), a resize like a relocation (`→`). `None` ⇒ no diff styling.
fn phase_diff_kind(diff: Option<&str>) -> Option<&'static str> {
    match diff {
        Some("added") => Some("added"),
        Some("removed") => Some("removed"),
        Some("renamed") => Some("changed"),
        Some("resized") => Some("moved"),
        _ => None,
    }
}

const COL_W: f64 = 210.0;
// When a (lane, col) cell holds several simultaneous stickies they auto-stack into sub-rows, each
// adding ROW_PITCH of height (a stored `y` places its element freely in the same band instead).
// LANE_VPAD keeps a single-row lane at the classic 108px (92 + 16), so uncrowded boards look
// exactly as before.
const ROW_PITCH: f64 = 92.0;
const LANE_VPAD: f64 = 16.0;
const MARGIN_L: f64 = 150.0;
const MARGIN_T: f64 = 116.0;
const STICKY_W: f64 = 176.0;
const STICKY_H: f64 = 74.0;
// A region's label tab: fixed height, width grows with the label (a per-char pitch + fixed
// padding). The client's region-add editor mirrors `REGION_TAB_H` via `__CONFIG__` rather than
// inventing its own box size (CUPID-Composable: render.rs is the single source of truth for a
// layout decision the client also needs — CODING_STANDARDS.md §Composable).
const REGION_TAB_H: f64 = 19.0;
const REGION_TAB_CHAR_W: f64 = 6.6;
const REGION_TAB_PAD: f64 = 18.0;
// How far apart sibling connectors fan when several meet a box on the same face (F-edge-routing
// Lever B). Deliberately small — the calm-instrument register wants a gentle spread, not a starburst.
// `fan_offsets` caps the per-slot step below this when a face is crowded, so the extreme anchor
// always stays on the box (a high-degree node packs tighter rather than spilling off the edge).
const FAN_SPREAD: f64 = 12.0;

fn is_upper(c: char) -> bool {
    c.is_ascii_uppercase()
}
fn is_lower_or_digit(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// Break a long CamelCase / Pascal token before a capital that follows a lower/digit, and
/// before the last capital of an acronym run — no space inserted.
fn hump_split(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let mut cuts = vec![0usize];
    for i in 1..n {
        let prev = chars[i - 1];
        let cur = chars[i];
        let cond1 = is_lower_or_digit(prev) && is_upper(cur);
        let cond2 =
            is_upper(prev) && is_upper(cur) && i + 1 < n && chars[i + 1].is_ascii_lowercase();
        if cond1 || cond2 {
            cuts.push(i);
        }
    }
    cuts.push(n);
    cuts.windows(2)
        .map(|w| chars[w[0]..w[1]].iter().collect())
        .collect()
}

/// Break one over-long token into wrap-able pieces: CamelCase humps first, then hard char-split.
fn atoms(word: &str, width: usize) -> Vec<String> {
    let pieces = if word.chars().count() > width {
        hump_split(word)
    } else {
        vec![word.to_string()]
    };
    let mut out = Vec::new();
    for p in pieces {
        let mut chars: Vec<char> = p.chars().collect();
        while chars.len() > width {
            out.push(chars[..width].iter().collect());
            chars = chars[width..].to_vec();
        }
        out.push(chars.iter().collect());
    }
    out
}

/// CamelCase-aware greedy wrap. Pieces of one broken token rejoin with no space (`glued`).
fn wrap(label: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut toks: Vec<(String, bool)> = Vec::new();
    for word in label.split_whitespace() {
        for (j, piece) in atoms(word, width).into_iter().enumerate() {
            toks.push((piece, j > 0));
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (text, glued) in toks {
        let sep = if glued || cur.is_empty() { "" } else { " " };
        if !cur.is_empty()
            && cur.chars().count() + sep.chars().count() + text.chars().count() > width
        {
            lines.push(cur);
            cur = text;
        } else {
            cur = format!("{}{}{}", cur, sep, text);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last().unwrap().clone();
        let trimmed: String = last
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            .trim_end()
            .to_string();
        *lines.last_mut().unwrap() = format!("{}\u{2026}", trimmed); // …
    }
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// A sticky reads as a hero line + optional smaller detail. An explicit `detail` wins;
/// otherwise a trailing parenthetical becomes the detail.
fn split_label(label: &str, detail: Option<&str>) -> (String, String) {
    if let Some(d) = detail {
        if !d.is_empty() {
            return (label.trim().to_string(), d.trim().to_string());
        }
    }
    if let Some(i) = label.find('(') {
        let rstripped = label.trim_end();
        if i > 0 && rstripped.ends_with(')') {
            let close = rstripped.rfind(')').unwrap();
            return (
                label[..i].trim().to_string(),
                label[i + 1..close].trim().to_string(),
            );
        }
    }
    (label.trim().to_string(), String::new())
}

fn opt_col(c: Option<i64>) -> String {
    c.map(|v| v.to_string()).unwrap_or_else(|| "None".into())
}

fn diff_tooltip(e: &Element, meta: &(String, String)) -> String {
    let (a, b) = (&meta.0, &meta.1);
    match e.diff.as_deref() {
        Some("added") => format!("added in {}", b),
        Some("removed") => format!("removed \u{2014} was in {}", a),
        Some("moved") => {
            let mut bits = Vec::new();
            if let Some(w) = &e.was {
                if w.kind != e.kind {
                    bits.push(format!("lane {} \u{2192} {}", w.kind, e.kind));
                }
                if w.col != e.col {
                    bits.push(format!(
                        "col {} \u{2192} {}",
                        opt_col(w.col),
                        opt_col(e.col)
                    ));
                }
                if w.y != e.y {
                    bits.push("repositioned in its lane".to_string());
                }
            }
            format!("moved: {}", bits.join(", "))
        }
        Some("changed") => format!(
            "was: {}",
            e.was.as_ref().map(|w| w.label.as_str()).unwrap_or("")
        ),
        _ => String::new(),
    }
}

/// One edge-endpoint queued on a box's face for fan-out: `(edge index, is-src, far-end cross pos)`.
type FaceMember = (usize, bool, f64);

/// A smooth connector between two box centres, anchored on the facing edges. `off1`/`off2` slide
/// each anchor along its facing edge (Lever B fan-out, F-edge-routing): the offset rides the *free*
/// axis of the chosen facing — Y for a left/right face, X for a top/bottom face — so several
/// connectors meeting one box on the same side spread out instead of collapsing onto one point.
/// Both offsets `0.0` reproduces the classic centre-to-centre path byte-for-byte.
fn edge_path(p1: (f64, f64), p2: (f64, f64), off1: f64, off2: f64) -> String {
    let (x1, y1) = p1;
    let (x2, y2) = p2;
    if (x2 - x1).abs() < STICKY_W {
        // Vertical facing: anchors ride the top/bottom faces, so the fan offset slides them in X.
        let sgn = if y2 >= y1 { 1.0 } else { -1.0 };
        let (ax1, ax2) = (x1 + off1, x2 + off2);
        let ay1 = y1 + sgn * STICKY_H / 2.0;
        let ay2 = y2 - sgn * STICKY_H / 2.0;
        let my = (ay1 + ay2) / 2.0;
        format!(
            "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            ax1, ay1, ax1, my, ax2, my, ax2, ay2
        )
    } else {
        // Horizontal facing: anchors ride the left/right faces, so the fan offset slides them in Y.
        let sgn = if x2 >= x1 { 1.0 } else { -1.0 };
        let ax1 = x1 + sgn * STICKY_W / 2.0;
        let ax2 = x2 - sgn * STICKY_W / 2.0;
        let (ay1, ay2) = (y1 + off1, y2 + off2);
        let mx = (ax1 + ax2) / 2.0;
        format!(
            "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            ax1, ay1, mx, ay1, mx, ay2, ax2, ay2
        )
    }
}

/// Order each `(lane, col)` cell's simultaneous members and return `(sub_ord, cell_total)`.
/// The primary key is the **stored `y`** (F-2d-placement): a dropped-on-top element (small
/// fraction) takes an upper slot, dropped-below (large fraction) a lower one; an unplaced member
/// keeps the neutral 0.5. Within equal keys, the edge-neighbour barycenter orders the stack
/// (Lever A, F-edge-routing — a neighbour's lane is fixed, so one deterministic pass), and a
/// member with no edges falls back to its own (shared) lane, so an edge-free cell keeps file
/// order through the stable sort. Output is independent of `HashMap` iteration order (each cell
/// writes disjoint `sub_ord` indices), so the render stays deterministic.
fn cell_sub_order(
    elements: &[Element],
    edges: &[Edge],
    idx_of: &HashMap<&str, usize>,
) -> (Vec<i64>, HashMap<(String, i64), i64>) {
    let band = |j: usize| lane_index(&elements[j].kind) as f64;
    // Running barycenter: a sum of neighbour bands and a count per node — no per-node Vec allocated.
    let mut bsum = vec![0.0_f64; elements.len()];
    let mut bcnt = vec![0u32; elements.len()];
    for e in edges {
        if let (Some(&s), Some(&d)) = (idx_of.get(e.src.as_str()), idx_of.get(e.dst.as_str())) {
            bsum[s] += band(d);
            bcnt[s] += 1;
            bsum[d] += band(s);
            bcnt[d] += 1;
        }
    }
    let bary = |i: usize| {
        if bcnt[i] == 0 {
            band(i)
        } else {
            bsum[i] / bcnt[i] as f64
        }
    };

    let mut cell_members: HashMap<(String, i64), Vec<usize>> = HashMap::new();
    for (i, e) in elements.iter().enumerate() {
        cell_members
            .entry((e.kind.clone(), e.col.unwrap()))
            .or_default()
            .push(i);
    }
    // A member's placement key — `model::y_key`, the single home of the ordering-key rule.
    let key = |j: usize| crate::model::y_key(elements[j].y);
    let mut sub_ord = vec![0i64; elements.len()];
    for members in cell_members.values_mut() {
        // Members enter in file order; the stable sort keeps that order for equal keys.
        members.sort_by(|&a, &b| {
            key(a)
                .partial_cmp(&key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    bary(a)
                        .partial_cmp(&bary(b))
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        for (rank, &i) in members.iter().enumerate() {
            sub_ord[i] = rank as i64;
        }
    }
    // `cell_total` is just each cell's count — derive it by consuming `cell_members` (no key clones).
    let cell_total = cell_members
        .into_iter()
        .map(|(k, v)| (k, v.len() as i64))
        .collect();
    (sub_ord, cell_total)
}

/// Lever B (F-edge-routing): per-edge fan offsets `(off_src, off_dst)` so several connectors meeting
/// one box on the same face spread along it instead of collapsing onto one anchor. `ends[ei]` is
/// edge `ei`'s `(src, dst)` element indices (`None` if an endpoint is unplaced); `centers` are the
/// resolved box centres. The face test mirrors `edge_path`'s facing rule, so the offset always rides
/// the free axis. A lone edge on a face keeps offset 0 (the classic centre anchor). Order-independent
/// (each endpoint writes its own `off_*[ei]` exactly once), so the render stays deterministic.
fn fan_offsets(ends: &[Option<(usize, usize)>], centers: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>) {
    // face: 0 right / 1 left (horizontal facing, fan in Y) · 2 bottom / 3 top (vertical, fan in X).
    // A face's members are `(edge index, is the box this edge's src?, far-end cross position)`.
    let mut face_groups: HashMap<(usize, u8), Vec<FaceMember>> = HashMap::new();
    for (ei, end) in ends.iter().enumerate() {
        let (s, d) = match end {
            Some(p) => *p,
            None => continue,
        };
        let (cs, cd) = (centers[s], centers[d]);
        let horizontal = (cd.0 - cs.0).abs() >= STICKY_W;
        let (face_s, cross_s, face_d, cross_d) = if horizontal {
            let fs = if cd.0 > cs.0 { 0 } else { 1 };
            let fd = if cs.0 > cd.0 { 0 } else { 1 };
            (fs, cd.1, fd, cs.1)
        } else {
            let fs = if cd.1 > cs.1 { 2 } else { 3 };
            let fd = if cs.1 > cd.1 { 2 } else { 3 };
            (fs, cd.0, fd, cs.0)
        };
        face_groups
            .entry((s, face_s))
            .or_default()
            .push((ei, true, cross_s));
        face_groups
            .entry((d, face_d))
            .or_default()
            .push((ei, false, cross_d));
    }
    let mut off_src = vec![0.0_f64; ends.len()];
    let mut off_dst = vec![0.0_f64; ends.len()];
    for (&(_, face), members) in face_groups.iter_mut() {
        let k = members.len();
        if k < 2 {
            continue;
        }
        // Sort by the far end's cross position; tie-break on edge index so the fan is deterministic.
        members.sort_by(|a, b| {
            a.2.partial_cmp(&b.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });
        // Clamp the per-slot step so the extreme anchor stays on the box face: a horizontal face
        // (0/1) fans in Y across STICKY_H, a vertical face (2/3) in X across STICKY_W. The extreme
        // slot rides step·(k−1)/2, so step ≤ half-extent·2/(k−1) keeps it on the box. For a small
        // k the cap exceeds FAN_SPREAD, so the common case is unchanged (byte-identical).
        let half = if face <= 1 { STICKY_H } else { STICKY_W } / 2.0;
        let step = FAN_SPREAD.min(2.0 * half / (k as f64 - 1.0));
        for (slot, &(ei, is_src, _)) in members.iter().enumerate() {
            let off = step * (slot as f64 - (k as f64 - 1.0) / 2.0);
            if is_src {
                off_src[ei] = off;
            } else {
                off_dst[ei] = off;
            }
        }
    }
    (off_src, off_dst)
}

pub fn render_svg(model: &Model) -> String {
    let mut elements = model.elements.clone();

    // R: the 8-lane scaffold is the board's structure, not a function of its contents. Every lane
    // always renders, so an empty board shows the full grammar (onboarding) and every lane title is
    // a hoverable add-target. (Previously this filtered to lanes that held an element.)
    let present: Vec<&str> = LANES.to_vec();

    // Auto-assign a column to any element missing `col`, preserving file order.
    let mut auto: i64 = 0;
    for e in elements.iter_mut() {
        if e.col.is_none() {
            e.col = Some(auto);
            auto += 1;
        }
    }
    // Column geometry is indexed by `col - min_col`, so a hand-authored negative or sparse `col`
    // lands at the left edge instead of casting to a wild `usize` (panic) or sizing a vast `ncols`
    // (OOM) — and unlike the old layout, a stray negative col is now drawn on-board, not off-screen.
    let min_col = elements.iter().map(|e| e.col.unwrap()).min().unwrap_or(0);
    let max_col = elements.iter().map(|e| e.col.unwrap()).max().unwrap_or(0);
    let ncols = (max_col - min_col + 1) as usize;

    // Two or more elements sharing a (lane, col) cell are *simultaneous* — `col` is the timeline,
    // not a per-lane slot — so we never spread them across fake columns (that would lie about when
    // they happen). Each cell auto-stacks into sub-rows: `sub_ord[i]` is element i's slot within
    // its cell, `cell_total` the cell's count. Lever A (F-edge-routing) orders a crowded cell by
    // its members' edge-neighbour barycenter — see `cell_sub_order`. A stored `y` overrides its
    // element's auto slot below (F-2d-placement). `idx_of` (the id→index map) is reused below to
    // resolve edge endpoints, so it is built once here.
    let idx_of: HashMap<&str, usize> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let (sub_ord, cell_total) = cell_sub_order(&elements, &model.edges, &idx_of);

    // Every timeline column is one COL_W slot — one col, one x (no sub-columns; the interstice
    // work of F-region-frontiers leans on this). A lane is as tall as its deepest cell's stack.
    let mut lane_rows: HashMap<&str, i64> = present.iter().map(|t| (*t, 1)).collect();
    for ((kind, _), total) in &cell_total {
        let r = lane_rows.get_mut(kind.as_str()).unwrap();
        *r = (*r).max(*total);
    }

    let col_left = |c: usize| MARGIN_L + c as f64 * COL_W;
    let board_right = col_left(ncols);

    let mut lane_top: HashMap<String, f64> = HashMap::new();
    let mut lane_h: HashMap<String, f64> = HashMap::new();
    let mut y = MARGIN_T;
    for t in &present {
        let h = lane_rows[*t] as f64 * ROW_PITCH + LANE_VPAD;
        lane_top.insert((*t).to_string(), y);
        lane_h.insert((*t).to_string(), h);
        y += h;
    }
    let lanes_bottom = y;

    // Resolve every element to an absolute centre. X is its column's midpoint — always. Y is a
    // **grid slot**, never a free position: the cell's stack (ordered by stored `y`, see
    // `cell_sub_order`) is centred within the room the lane reserved (`lead_r` rows of slack on
    // each side), so a lone sticky keeps its classic mid-lane position whatever its `y`, and a
    // shared cell splits its members onto distinct row centres (the lane grew to hold them).
    let centers: Vec<(f64, f64)> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let col = e.col.unwrap();
            let cx = col_left((col - min_col) as usize) + COL_W / 2.0;
            let band_top = lane_top[&e.kind] + LANE_VPAD / 2.0;
            let rows = lane_rows[e.kind.as_str()];
            let total = cell_total[&(e.kind.clone(), col)];
            let lead_r = (rows - total) as f64 / 2.0;
            let cy = band_top + (lead_r + sub_ord[i] as f64 + 0.5) * ROW_PITCH;
            (cx, cy)
        })
        .collect();

    let width = (board_right + 40.0) as i64;
    let height = (lanes_bottom + 60.0) as i64;

    let diff_meta = model.diff_meta.clone();
    let mut p: Vec<String> = Vec::new();
    p.push(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" font-family=\"-apple-system,Segoe UI,Roboto,sans-serif\">",
        w = width,
        h = height
    ));
    // Arrow fill = context-stroke, so each arrowhead takes its own edge's colour.
    p.push(
        "<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" \
         markerHeight=\"7\" orient=\"auto\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"context-stroke\"/>\
         </marker></defs>"
            .to_string(),
    );
    p.push(format!(
        "<rect width=\"{}\" height=\"{}\" fill=\"#fbfbfd\"/>",
        width, height
    ));
    // The board title is the instrument's engraved nameplate: a refined system serif, the one
    // place a second font family appears (see DESIGN.md §3). Everything else stays the SVG sans.
    p.push(format!(
        "<text x=\"20\" y=\"34\" font-size=\"20\" font-weight=\"700\" fill=\"#222\" \
         font-family=\"'Iowan Old Style','Palatino Linotype',Palatino,'Book Antiqua',Georgia,serif\">{}</text>",
        esc(&model.title)
    ));

    // Diff subtitle: rev labels + per-status counts with dashed swatches.
    if let Some((a, b)) = &diff_meta {
        p.push(format!(
            "<text x=\"20\" y=\"56\" font-size=\"12\" fill=\"#777\">{} \u{2192} {}</text>",
            esc(a),
            esc(b)
        ));
        let mut lx2 = 40.0 + 7.0 * ((a.chars().count() + b.chars().count() + 3) as f64);
        for k in ["added", "removed", "changed", "moved"] {
            let n = elements
                .iter()
                .filter(|e| e.diff.as_deref() == Some(k))
                .count();
            if n == 0 {
                continue;
            }
            p.push(format!(
                "<rect x=\"{}\" y=\"46\" width=\"12\" height=\"12\" rx=\"3\" fill=\"none\" \
                 stroke=\"{}\" stroke-width=\"2.5\" stroke-dasharray=\"3 2\"/>",
                lx2,
                diff_colour(k)
            ));
            p.push(format!(
                "<text x=\"{}\" y=\"56\" font-size=\"12\" fill=\"#555\">{} {}</text>",
                lx2 + 17.0,
                n,
                k
            ));
            lx2 += 17.0 + 8.0 * ((k.len() + n.to_string().len() + 1) as f64) + 16.0;
        }
    }

    // Regions (a.k.a. phases) — a region is a *thin labelled outline*, never a filled block
    // (DESIGN.md calm-instrument register; anti-reference: Miro maximalism). It reads as an open
    // "⊓": a top rule + two grabbable vertical edges, plus the faintest tonal wash (the only fill
    // DESIGN.md §4 sanctions for a band). The lanes flow through it; it frames, it does not box in.
    //
    // Geometry comes from the region's own stored bounds `[from_col, to_col]` (the single source of
    // truth — F-container scope D2), not from where elements happen to land, so an empty or removed
    // region still renders. Bounds are clamped into the present column range so an out-of-view band
    // collapses to the edge instead of indexing past `col_left`.
    let band_top = MARGIN_T - 26.0;
    let band_bot = lanes_bottom - 6.0;
    let ncols_i = ncols as i64;
    let clamp_idx = |c: i64| (c - min_col).clamp(0, ncols_i - 1) as usize;
    // Pivotal events sit *on the border line*, in the event lane (derived via `is_pivotal`; scope
    // D3 — no stored flag). `present` always carries the full 8-lane grammar, so "event" is here.
    let event_cy = lane_top.get("event").map(|t| t + lane_h["event"] / 2.0);
    // A pivotal node belongs to the *boundary*, not a region: two regions sharing a gutter
    // (`A.to_col` == `B.from_col`) collapse to one node. So gather the cols that carry a pivotal
    // event once (O(elements·phases), via `is_pivotal`), accumulate each boundary's x while drawing
    // the regions, then emit one deduped node per position after the loop.
    let pivotal_cols: HashSet<i64> = elements
        .iter()
        .filter(|e| is_pivotal(model, e))
        .filter_map(|e| e.col)
        .collect();
    let mut pivot_node_x: Vec<i64> = Vec::new(); // x·10 rounded, deduped before emit

    // A "region rail": one invisible hit-rect per visible column, always rendered (Stage 6 —
    // `create region` needs a click target even where no region exists yet). Painted *before* the
    // regions below, so a live region's own rect/edges/tab paint over it and stay clickable for
    // their own gestures; the rail only "shows through" (hoverable) in the gaps — exactly the
    // create-region affordance, with no extra client-side membership logic.
    for idx in 0..ncols {
        p.push(format!(
            "<rect class=\"region-rail\" data-col=\"{}\" x=\"{:.1}\" y=\"{}\" width=\"{:.1}\" \
             height=\"{}\" fill=\"transparent\"/>",
            min_col + idx as i64,
            col_left(idx),
            band_top,
            COL_W,
            band_bot - band_top
        ));
    }

    for ph in &model.phases {
        let a = clamp_idx(ph.from_col);
        let b = clamp_idx(ph.to_col);
        let (lo, hi) = (a.min(b), a.max(b));
        let x = col_left(lo);
        let right = col_left(hi) + COL_W;
        let w = right - x;
        // The *clamped* bound (review: a region drag desyncs if the client reads the raw stored
        // `ph.from_col`/`ph.to_col` — those can extend past `min_col..max_col` (the element-derived
        // range the region-rail covers), so a resize starting from an out-of-range "other edge"
        // could target a column with no rail cell at all. Emitting the same clamped value the
        // visual box already uses keeps the client's drag math and the rendered board in lockstep —
        // WYSIWYG: what's draggable is exactly what's drawn, never a hidden true bound.
        let clamped_from = min_col + lo as i64;
        let clamped_to = min_col + hi as i64;

        // Diff verdict mapped onto the element-diff vocabulary (Review #4: read `Phase.diff` or a
        // *removed* region — now fed into `model.phases` by `diff_phases` — paints as a phantom
        // unstyled band). A removed region is ghosted; the rest pick up the dashed diff stroke.
        let dk = phase_diff_kind(ph.diff.as_deref());
        let removed = ph.diff.as_deref() == Some("removed");
        let diff_col = dk.map(diff_colour); // computed once; each use picks its own bench fallback
        let stroke = diff_col.unwrap_or("#cfcfda");
        let top_stroke = diff_col.unwrap_or("#e0e0e6");
        let dash = if dk.is_some() {
            " stroke-dasharray=\"4 3\""
        } else {
            ""
        };

        // One group per region carries its identity + *clamped* bounds (Stage 6: the client reads
        // `data-from-col`/`data-to-col` to know a resize's *other* edge without inverse pixel math;
        // clamped so that edge always falls on a column the region-rail actually covers).
        p.push(format!(
            "<g class=\"region{}\" data-region=\"{}\" data-from-col=\"{}\" data-to-col=\"{}\">",
            if removed { " removed" } else { "" },
            esc(&ph.id),
            clamped_from,
            clamped_to
        ));
        if removed {
            p.push("<g opacity=\"0.45\">".to_string());
        }

        // Tonal wash (the lone sanctioned region fill, #000 @ 0.02) + the open "⊓" outline.
        p.push(format!(
            "<rect x=\"{:.1}\" y=\"{}\" width=\"{:.1}\" height=\"{}\" fill=\"#000\" opacity=\"0.02\"/>",
            x,
            band_top,
            w,
            band_bot - band_top
        ));
        p.push(format!(
            "<line x1=\"{:.1}\" y1=\"{}\" x2=\"{:.1}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"{}/>",
            x, band_top, right, band_top, top_stroke, dash
        ));
        for (edge_x, edge) in [(x, "from"), (right, "to")] {
            p.push(format!(
                "<line x1=\"{:.1}\" y1=\"{}\" x2=\"{:.1}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"{}/>",
                edge_x, band_top, edge_x, band_bot, stroke, dash
            ));
            // A wide transparent hit-region carries the resize affordance for the Stage-6 client to
            // grab (the *visual* half of D5: grab target = the band border, not a sticky). A removed
            // region is gone on the new side — there is nothing to resize, so no handle. The dragged
            // edge's own bound is resolved client-side from the rail cell under the cursor, not from
            // an attribute here — only the enclosing `<g>`'s `data-from-col`/`data-to-col` carry a
            // stored bound (the *other*, undragged edge's).
            if !removed {
                p.push(format!(
                    "<line class=\"region-edge\" data-region=\"{}\" data-edge=\"{}\" \
                     x1=\"{:.1}\" y1=\"{}\" x2=\"{:.1}\" y2=\"{}\" stroke=\"transparent\" \
                     stroke-width=\"8\"/>",
                    esc(&ph.id),
                    edge,
                    edge_x,
                    band_top,
                    edge_x,
                    band_bot
                ));
            }
        }

        // Label tab — the region's identity handle, a quiet folder tab straddling the top-left
        // corner (instrument grey, no domain colour: the Bench-Is-Grey Rule). Carries the diff badge
        // when the region changed. Grouped as one focusable button (mirrors the sticky pattern) so
        // the Stage-6 client can bind a click/dblclick/Enter → in-place rename to one hit target.
        let badge = dk.and_then(diff_badge);
        let label = match badge {
            Some(b) => format!("{b} {}", ph.label),
            None => ph.label.clone(),
        };
        let tab_h = REGION_TAB_H;
        let tab_w = label.chars().count() as f64 * REGION_TAB_CHAR_W + REGION_TAB_PAD;
        let tab_y = band_top - tab_h + 1.0;
        if !removed {
            // `data-label` carries the *raw* stored label (no diff badge) — the Stage-6 rename
            // editor prefills from this, not the badge-prefixed display `label` below.
            p.push(format!(
                "<g class=\"region-tab\" data-region=\"{}\" data-label=\"{}\" role=\"button\" \
                 tabindex=\"0\" aria-label=\"region {}, {}\" style=\"cursor:pointer\">",
                esc(&ph.id),
                esc(&ph.label),
                esc(&ph.id),
                esc(&ph.label)
            ));
        }
        p.push(format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" \
             fill=\"#ffffff\" stroke=\"{}\" stroke-width=\"1\"{}/>",
            x, tab_y, tab_w, tab_h, stroke, dash
        ));
        p.push(format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"11\" font-weight=\"600\" fill=\"{}\">{}</text>",
            x + 9.0,
            tab_y + 13.5,
            diff_col.unwrap_or(AXIS_LABEL),
            esc(&label)
        ));
        if !removed {
            p.push("</g>".to_string());
        }

        // Note each edge that carries a pivotal event (an `event` sitting on this region's
        // boundary col). A removed region is gone on the new side, so its borders mark nothing.
        if event_cy.is_some() && !removed {
            if pivotal_cols.contains(&ph.from_col) {
                pivot_node_x.push((x * 10.0).round() as i64);
            }
            if pivotal_cols.contains(&ph.to_col) {
                pivot_node_x.push((right * 10.0).round() as i64);
            }
        }

        if removed {
            p.push("</g>".to_string());
        }
        p.push("</g>".to_string());
    }

    // One pivotal node per unique boundary position (sorted for deterministic output; a node shared
    // by two adjacent regions is drawn once). It rides the border line at the event-lane centre.
    if let Some(cy) = event_cy {
        pivot_node_x.sort_unstable();
        pivot_node_x.dedup();
        for kx in &pivot_node_x {
            p.push(format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{AXIS_LABEL}\" \
                 stroke=\"#fbfbfd\" stroke-width=\"1.5\"/>",
                *kx as f64 / 10.0,
                cy
            ));
        }
    }

    // Faint horizontal lane rules — graph-paper bench lines that delimit lanes now that a busy
    // lane can span several rows.
    for t in present.iter().skip(1) {
        p.push(format!(
            "<line x1=\"12\" y1=\"{:.1}\" x2=\"{}\" y2=\"{:.1}\" stroke=\"#e0e0e6\" opacity=\"0.55\"/>",
            lane_top[*t],
            width - 20,
            lane_top[*t]
        ));
    }

    // Lane labels — centred on each lane's (possibly multi-row) band. `data-band-top`/
    // `data-band-h` expose the band *interior* (the `y` fraction's frame of reference) so the
    // client's vertical drag converts a pixel drop into a stored fraction without re-deriving
    // the lane geometry — render.rs stays the single source of truth for it (Composable).
    for t in &present {
        let y = lane_top[*t] + lane_h[*t] / 2.0;
        // `class`/`data-lane` let the client hang the lane-title `+` (inline-add prepend) on each
        // label; the rendered text content is unchanged.
        p.push(format!(
            "<text class=\"lane-label\" data-lane=\"{}\" data-band-top=\"{:.1}\" \
             data-band-h=\"{:.1}\" x=\"16\" y=\"{:.1}\" font-size=\"12\" \
             font-weight=\"600\" fill=\"{}\">{}</text>",
            esc(t),
            lane_top[*t] + LANE_VPAD / 2.0,
            lane_h[*t] - LANE_VPAD,
            y + 4.0,
            AXIS_LABEL,
            esc(t)
        ));
    }

    // Lever B (F-edge-routing): fan connectors that share a box face apart so they don't collapse
    // onto one anchor — see `fan_offsets`. `ends[ei]` resolves each edge's endpoints once (reusing
    // `idx_of`); the edge loop below reuses it to skip edges with an unplaced endpoint.
    let ends: Vec<Option<(usize, usize)>> = model
        .edges
        .iter()
        .map(
            |e| match (idx_of.get(e.src.as_str()), idx_of.get(e.dst.as_str())) {
                (Some(&s), Some(&d)) => Some((s, d)),
                _ => None,
            },
        )
        .collect();
    let (off_src, off_dst) = fan_offsets(&ends, &centers);

    // Edges (under the stickies). A hotspot connector is a concern, not a flow: dotted, arrow-less.
    for (ei, edge) in model.edges.iter().enumerate() {
        let (si, di) = match ends[ei] {
            Some(p) => p,
            None => continue,
        };
        let d = edge_path(centers[si], centers[di], off_src[ei], off_dst[ei]);
        let is_hot = elements[si].kind == "hotspot" || elements[di].kind == "hotspot";
        let cls = if is_hot { "edge hot" } else { "edge" };
        let attrs = format!(
            "class=\"{}\" data-src=\"{}\" data-dst=\"{}\" fill=\"none\"",
            cls,
            esc(&edge.src),
            esc(&edge.dst)
        );
        match edge.status.as_deref() {
            Some("added") => p.push(format!(
                "<path {a} d=\"{d}\" stroke=\"{c}\" stroke-width=\"1.8\" stroke-dasharray=\"6 4\" \
                 marker-end=\"url(#arrow)\" opacity=\"0.9\"/>",
                a = attrs,
                d = d,
                c = diff_colour("added")
            )),
            Some("removed") => p.push(format!(
                "<path {a} d=\"{d}\" stroke=\"{c}\" stroke-width=\"1.6\" stroke-dasharray=\"3 4\" \
                 marker-end=\"url(#arrow)\" opacity=\"0.5\"/>",
                a = attrs,
                d = d,
                c = diff_colour("removed")
            )),
            _ if is_hot => p.push(format!(
                "<path {a} d=\"{d}\" stroke=\"{c}\" stroke-width=\"1.4\" stroke-dasharray=\"0.1 6\" \
                 stroke-linecap=\"round\" opacity=\"0.7\"/>",
                a = attrs,
                d = d,
                c = EDGE_HOTSPOT
            )),
            _ => p.push(format!(
                "<path {a} d=\"{d}\" stroke=\"{c}\" stroke-width=\"1.5\" marker-end=\"url(#arrow)\" \
                 opacity=\"0.6\"/>",
                a = attrs,
                d = d,
                c = EDGE_FLOW
            )),
        }
    }

    // Stickies — each a clickable <g id="..."> the sidecar targets by id.
    for (i, e) in elements.iter().enumerate() {
        let (cx, cy) = centers[i];
        let x = cx - STICKY_W / 2.0;
        let y = cy - STICKY_H / 2.0;
        let (hero, detail) = split_label(&e.label, e.detail.as_deref());
        let is_hotspot = e.kind == "hotspot";
        let resolved = is_hotspot && e.resolved;
        let fill = if resolved {
            RESOLVED_FILL
        } else {
            colour(&e.kind)
        };
        let txt = if resolved || text_dark(&e.kind) {
            "#1a1a1a"
        } else {
            "#ffffff"
        };
        let shape_i: i64 = if is_hotspot { 2 } else { 8 };
        let status = e.diff.as_deref();

        let g_op = if status == Some("removed") {
            " opacity=\"0.4\""
        } else {
            ""
        };
        let mut cls = format!("sticky {}", e.kind);
        if resolved {
            cls.push_str(" resolved");
        }
        if let Some(s) = status {
            cls.push_str(&format!(" diff-{}", s));
        }
        // A sticky is the primary control: it must be reachable and operable without a mouse.
        // `role=button` + `tabindex=0` put it in the tab order; the aria-label names it the way a
        // sighted user reads it (id, label, lane). The client wires Enter/Space → comment dialog
        // and ←/→ → move on the *focused* sticky (template.html).
        let mut aria = format!("{}, {}", e.id, hero);
        if !detail.is_empty() {
            aria.push_str(&format!(", {}", detail));
        }
        aria.push_str(&format!(", {}", e.kind));
        if resolved {
            aria.push_str(", resolved");
        }
        // data-kind / data-col / data-cx / data-cy let the client replay a move (translate the
        // group, recompute its edges) without a server round-trip — see template.html. A placed
        // element also exposes its normalised ordering key (`data-y`), so the client's grid
        // preview sorts a dragged box against its occupants exactly as this renderer will.
        let data_y = match e.y {
            Some(_) => format!(" data-y=\"{}\"", crate::model::y_key(e.y)),
            None => String::new(),
        };
        p.push(format!(
            "<g id=\"{}\" class=\"{}\" role=\"button\" tabindex=\"0\" aria-label=\"{}\" \
             data-hero=\"{}\" data-detail=\"{}\" data-kind=\"{}\" \
             data-col=\"{}\" data-cx=\"{:.1}\" data-cy=\"{:.1}\"{} style=\"cursor:pointer\"{}>",
            esc(&e.id),
            cls,
            esc(&aria),
            esc(&hero),
            esc(&detail),
            esc(&e.kind),
            e.col.unwrap(),
            cx,
            cy,
            data_y,
            g_op
        ));
        if let (Some(_), Some(meta)) = (status, &diff_meta) {
            let tip = diff_tooltip(e, meta);
            if !tip.is_empty() {
                p.push(format!("<title>{}</title>", esc(&tip)));
            }
        }
        if matches!(status, Some("added") | Some("changed") | Some("moved")) {
            let s = status.unwrap();
            p.push(format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"none\" \
                 stroke=\"{}\" stroke-width=\"3\" stroke-dasharray=\"6 4\"/>",
                x - 4.0,
                y - 4.0,
                STICKY_W + 8.0,
                STICKY_H + 8.0,
                shape_i + 3,
                diff_colour(s)
            ));
        }
        p.push(format!(
            "<rect class=\"card\" x=\"{:.1}\" y=\"{:.1}\" width=\"{}\" height=\"{}\" rx=\"{}\" \
             fill=\"{}\" stroke=\"#0003\"/>",
            x, y, STICKY_W, STICKY_H, shape_i, fill
        ));
        p.push(format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"9\" font-weight=\"700\" fill=\"{}\" \
             opacity=\"0.6\">{}</text>",
            x + 8.0,
            y + 15.0,
            txt,
            esc(&e.id)
        ));
        let hlines = wrap(&hero, 20, if detail.is_empty() { 3 } else { 2 });
        let block_h = hlines.len() as f64 * 14.0 + if !detail.is_empty() { 12.0 } else { 0.0 };
        let start = cy - block_h / 2.0 + 11.0;
        for (i, ln) in hlines.iter().enumerate() {
            p.push(format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" font-weight=\"600\" \
                 text-anchor=\"middle\" fill=\"{}\">{}</text>",
                cx,
                start + i as f64 * 14.0,
                txt,
                esc(ln)
            ));
        }
        if !detail.is_empty() {
            let dtxt = wrap(&detail, 30, 1).into_iter().next().unwrap_or_default();
            p.push(format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"9.5\" text-anchor=\"middle\" fill=\"{}\" \
                 opacity=\"0.6\">{}</text>",
                cx,
                start + hlines.len() as f64 * 14.0,
                txt,
                esc(&dtxt)
            ));
        }
        if resolved {
            p.push(format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"8\" fill=\"#6FAE7E\"/>",
                x + STICKY_W - 3.0,
                y + 3.0
            ));
            p.push(format!(
                "<path d=\"M{:.1},{:.1} l2.4,2.6 l4,-5.2\" fill=\"none\" stroke=\"#fff\" \
                 stroke-width=\"1.6\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                x + STICKY_W - 7.0,
                y + 3.0
            ));
        }
        if status == Some("removed") {
            p.push(format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" \
                 stroke-width=\"2.5\"/>",
                x + 6.0,
                cy,
                x + STICKY_W - 6.0,
                cy,
                diff_colour("removed")
            ));
        }
        if let Some(badge) = status.and_then(diff_badge) {
            let s = status.unwrap();
            p.push(format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"9\" fill=\"{}\" stroke=\"#fff\" \
                 stroke-width=\"1.5\"/>",
                x + STICKY_W,
                y,
                diff_colour(s)
            ));
            p.push(format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" font-weight=\"700\" \
                 text-anchor=\"middle\" fill=\"#fff\">{}</text>",
                x + STICKY_W,
                y + 4.0,
                badge
            ));
        }
        p.push("</g>".to_string());
    }

    // Legend: type swatches, then a connector key (flow vs hotspot-concern).
    let ly = height - 28;
    let mut lx: i64 = 20;
    for t in &present {
        p.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"14\" height=\"14\" rx=\"3\" fill=\"{}\" stroke=\"#0003\"/>",
            lx,
            ly,
            colour(t)
        ));
        p.push(format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#555\">{}</text>",
            lx + 19,
            ly + 11,
            esc(t)
        ));
        lx += 26 + 7 * (t.len() as i64) + 14;
    }
    let midy = ly + 7;
    lx += 12;
    p.push(format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" \
         marker-end=\"url(#arrow)\"/>",
        lx,
        midy,
        lx + 26,
        midy,
        EDGE_FLOW
    ));
    p.push(format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#555\">triggers / leads to</text>",
        lx + 33,
        ly + 11
    ));
    lx += 33 + 7 * ("triggers / leads to".len() as i64) + 18;
    p.push(format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.4\" \
         stroke-dasharray=\"0.1 6\" stroke-linecap=\"round\"/>",
        lx,
        midy,
        lx + 26,
        midy,
        EDGE_HOTSPOT
    ));
    p.push(format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#555\">open question (hotspot)</text>",
        lx + 33,
        ly + 11
    ));

    p.push("</svg>".to_string());
    p.join("\n")
}

pub fn render_html(svg: &str, title: &str) -> String {
    // The client reuses these geometry constants to re-place a moved sticky and redraw its edges
    // in the browser — keep render.rs the single source of truth for them. `regionTabH`/
    // `regionTabCharW` do the same for the region-add editor's box (Composable — the client must
    // not invent its own tab size).
    // `rowPitch`/`laneVpad` let the drag preview place the lane-growth guide exactly one row
    // below the lane's current bottom rule — the same numbers this renderer will use on commit.
    let cfg = format!(
        "{{\"colW\":{},\"stickyW\":{},\"stickyH\":{},\"rowPitch\":{},\"laneVpad\":{},\"regionTabH\":{},\"regionTabCharW\":{},\"regionTabPad\":{}}}",
        COL_W, STICKY_W, STICKY_H, ROW_PITCH, LANE_VPAD, REGION_TAB_H, REGION_TAB_CHAR_W, REGION_TAB_PAD
    );
    HTML_TEMPLATE
        .replace("__TITLE__", &esc(title))
        .replace("__SVG__", svg)
        .replace("__CONFIG__", &cfg)
}

const HTML_TEMPLATE: &str = include_str!("template.html");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Phase;

    #[test]
    fn hump_split_breaks_camelcase_and_acronym_runs() {
        assert_eq!(hump_split("ItemAdded"), vec!["Item", "Added"]);
        assert_eq!(hump_split("HTTPServer"), vec!["HTTP", "Server"]);
        assert_eq!(hump_split("plain"), vec!["plain"]);
    }

    #[test]
    fn esc_encodes_the_five_xml_special_chars() {
        assert_eq!(esc("&<>\"'"), "&amp;&lt;&gt;&quot;&#x27;");
    }

    #[test]
    fn split_label_prefers_detail_then_trailing_parenthetical() {
        assert_eq!(
            split_label("Title", Some("a detail")),
            ("Title".to_string(), "a detail".to_string())
        );
        assert_eq!(
            split_label("ItemAdded (when cart open)", None),
            ("ItemAdded".to_string(), "when cart open".to_string())
        );
        assert_eq!(
            split_label("Plain", None),
            ("Plain".to_string(), String::new())
        );
    }

    // The client's instant-move replay reads col / lane / centre off the sticky group; if these
    // attributes ever stop being emitted, moves silently break. Pin them here.
    fn one_event_at_col(col: i64) -> Model {
        Model {
            title: "t".into(),
            phases: vec![],
            elements: vec![Element {
                id: "E1".into(),
                kind: "event".into(),
                label: "L".into(),
                col: Some(col),
                detail: None,
                y: None,
                resolved: false,
                diff: None,
                was: None,
            }],
            edges: vec![],
            diff_meta: None,
        }
    }

    #[test]
    fn lane_prefix_is_aligned_with_lanes_and_total() {
        assert_eq!(LANES.len(), LANE_PREFIXES.len());
        assert!(LANES.iter().all(|l| lane_prefix(l).is_some()));
        assert_eq!(lane_prefix("actor"), Some('X')); // not 'A' — aggregate owns that
        assert_eq!(lane_prefix("aggregate"), Some('A'));
        assert_eq!(lane_prefix("hotspot"), Some('H'));
        assert_eq!(lane_prefix("not-a-lane"), None);
    }

    fn empty_board() -> Model {
        Model {
            title: "t".into(),
            phases: vec![],
            elements: vec![],
            edges: vec![],
            diff_meta: None,
        }
    }

    // R: the lane scaffold is the board's structure, not a function of its contents — every lane
    // renders even when empty, so an empty board shows all 8 lanes (onboarding) and every lane
    // title is a hoverable add-target. Pin all 8 labels on a zero-element board.
    #[test]
    fn every_lane_renders_even_on_an_empty_board() {
        let svg = render_svg(&empty_board());
        for lane in LANES {
            assert!(
                svg.contains(&format!(">{lane}</text>")),
                "empty board is missing the `{lane}` lane label"
            );
        }
    }

    #[test]
    fn sticky_group_exposes_layout_data_attributes() {
        let svg = render_svg(&one_event_at_col(2));
        assert!(svg.contains("data-kind=\"event\""));
        assert!(svg.contains("data-col=\"2\""));
        assert!(svg.contains("data-cx="));
        assert!(svg.contains("data-cy="));
    }

    // A sticky is the primary control; it must stay keyboard-reachable and screen-reader-named.
    // If these ever stop being emitted the board silently becomes mouse-only again — pin them.
    #[test]
    fn sticky_group_is_a_focusable_labelled_button() {
        let svg = render_svg(&one_event_at_col(2));
        assert!(svg.contains("role=\"button\""));
        assert!(svg.contains("tabindex=\"0\""));
        assert!(svg.contains("aria-label=\"E1, L, event\""));
    }

    fn attr_values(svg: &str, attr: &str) -> Vec<String> {
        let needle = format!("{}=\"", attr);
        svg.match_indices(&needle)
            .map(|(i, _)| {
                let rest = &svg[i + needle.len()..];
                rest[..rest.find('"').unwrap()].to_string()
            })
            .collect()
    }

    fn events_at_col(col: i64, n: usize) -> Model {
        Model {
            title: "t".into(),
            phases: vec![],
            elements: (0..n)
                .map(|k| Element {
                    id: format!("E{k}"),
                    kind: "event".into(),
                    label: format!("L{k}"),
                    col: Some(col),
                    detail: None,
                    y: None,
                    resolved: false,
                    diff: None,
                    was: None,
                })
                .collect(),
            edges: vec![],
            diff_meta: None,
        }
    }

    // The faithfulness contract: simultaneous stickies (same lane + col) with no stored `y` must
    // never render on top of one another. They auto-stack into sub-rows down one column — one col,
    // one x (the packing modes and their sub-columns are gone; F-2d-placement) — so every centre
    // is unique and no element is hidden.
    #[test]
    fn simultaneous_stickies_stack_into_distinct_centres() {
        let svg = render_svg(&events_at_col(2, 5));
        let cys = attr_values(&svg, "data-cy");
        assert_eq!(cys.len(), 5);
        let unique: std::collections::HashSet<&String> = cys.iter().collect();
        assert_eq!(unique.len(), 5, "stacked stickies share a centre: {cys:?}");
        assert_eq!(distinct(&svg, "data-cx"), 1, "one col = one x, always");
        // The dark grey "time-slot tray" went with the packing modes — a poor 2D representation.
        assert!(
            !svg.contains("fill=\"#90a4ae\""),
            "the grey cell tray must be gone"
        );
    }

    fn distinct(svg: &str, attr: &str) -> usize {
        attr_values(svg, attr)
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    // F-2d-placement (grid form): a stored `y` is an *ordering key*, never a free position —
    // everything renders on row-slot centres. A lone element stays on the classic mid-line
    // whatever its `y`; a shared cell splits its members top/bottom by their keys (an unplaced
    // member holds the neutral middle), and an out-of-range log value clamps into the stack
    // instead of drawing off-band.
    #[test]
    fn a_lone_element_stays_on_the_grid_mid_line_whatever_its_y() {
        for y in [0.0, 0.5, 0.93] {
            let mut m = one_event_at_col(0);
            m.elements[0].y = Some(y);
            assert_eq!(
                attr_values(&render_svg(&m), "data-cy"),
                vec!["494.0".to_string()],
                "y={y} must still render on the single-slot centre"
            );
        }
    }

    #[test]
    fn a_stored_y_orders_a_shared_cell_top_or_bottom_on_slot_centres() {
        // Two events share (event, col 2): the lane grows to 2 rows (band top 448, slots at
        // 494 / 586). E1 carries the y; E0 is unplaced (neutral 0.5).
        let place = |y: f64| {
            let mut m = events_at_col(2, 2);
            m.elements[1].y = Some(y);
            (cy_of(&render_svg(&m), "E0"), cy_of(&render_svg(&m), "E1"))
        };
        assert_eq!(place(0.9), (494.0, 586.0), "dropped below → bottom slot");
        assert_eq!(place(0.1), (586.0, 494.0), "dropped above → top slot");
        assert_eq!(
            place(7.0),
            (494.0, 586.0),
            "out-of-range clamps into the stack"
        );
    }

    // The client's vertical drag converts a pixel drop into a `y` fraction using the band frame
    // the server itself rendered — pin the attributes it reads.
    #[test]
    fn lane_labels_expose_the_band_interior_geometry() {
        let svg = render_svg(&empty_board());
        // actor is the first lane: top = MARGIN_T + LANE_VPAD/2 = 124, interior = ROW_PITCH = 92.
        assert!(svg.contains(
            "class=\"lane-label\" data-lane=\"actor\" data-band-top=\"124.0\" data-band-h=\"92.0\""
        ));
    }

    // A hand-authored negative or sparse `col` must render, not panic/OOM. Column geometry is
    // indexed by `col - min_col`, so the leftmost authored column maps to slot 0.
    #[test]
    fn negative_and_sparse_columns_render_without_panicking() {
        let mut m = events_at_col(-3, 1);
        m.elements.push(Element {
            id: "E9".into(),
            kind: "event".into(),
            label: "far".into(),
            col: Some(2),
            detail: None,
            y: None,
            resolved: false,
            diff: None,
            was: None,
        });
        let svg = render_svg(&m);
        assert_eq!(distinct(&svg, "data-cx"), 2);
        // col -3 is the leftmost authored column → slot 0 → classic single-cell centre 255.0.
        let cxs = attr_values(&svg, "data-cx");
        assert!(cxs.contains(&"255.0".to_string()), "got {cxs:?}");
    }

    // A lone sticky keeps its classic position: centred on a single-row lane, no horizontal fan.
    // Under R every lane always renders, so `event` is the 4th lane (actor/command/aggregate sit
    // above it), each an empty single-row band of height ROW_PITCH + LANE_VPAD = 108.
    #[test]
    fn a_lone_sticky_stays_on_the_lane_mid_line() {
        let svg = render_svg(&events_at_col(0, 1));
        // lane_top(event) = MARGIN_T + 3*108 = 440; + LANE_VPAD/2 + ROW_PITCH/2 = 440 + 8 + 46.
        assert_eq!(attr_values(&svg, "data-cy"), vec!["494.0".to_string()]);
        // col 0 centre, no stagger: MARGIN_L + COL_W/2 = 150 + 105.
        assert_eq!(attr_values(&svg, "data-cx"), vec!["255.0".to_string()]);
    }

    fn phase(id: &str, label: &str, from: i64, to: i64, diff: Option<&str>) -> Phase {
        Phase {
            id: id.into(),
            label: label.into(),
            from_col: from,
            to_col: to,
            diff: diff.map(Into::into),
        }
    }

    // A region renders as a thin labelled outline (scope D1, calm instrument): a label tab carrying
    // its name, two grabbable border edges keyed by region id + side (the visual half of D5), and a
    // pivotal node where an event sits on a boundary col (derived, scope D3).
    #[test]
    fn region_renders_as_a_labelled_outline_with_grab_handles_and_pivotal_node() {
        let m = Model {
            title: "t".into(),
            phases: vec![phase("K1", "Context A", 0, 2, None)],
            elements: vec![el("E1", "event", 0), el("E2", "event", 1)],
            edges: vec![],
            diff_meta: None,
        };
        let svg = render_svg(&m);
        assert!(svg.contains(">Context A<"), "region label tab is missing");
        // Both border edges carry the resize affordance, addressed by region id + side.
        assert!(svg.contains("class=\"region-edge\" data-region=\"K1\" data-edge=\"from\""));
        assert!(svg.contains("class=\"region-edge\" data-region=\"K1\" data-edge=\"to\""));
        // The enclosing group carries the region's *clamped* bounds — K1's authored to_col (2) is
        // past the last visible column (elements only reach col 1), so the group reports the
        // clamped bound (1), matching the visual box exactly. Review: emitting the raw, unclamped
        // `ph.to_col` here desynced the client's drag math from the rail (which only covers
        // min_col..max_col) — a resize could target a column with no rail cell at all.
        assert!(svg
            .contains("class=\"region\" data-region=\"K1\" data-from-col=\"0\" data-to-col=\"1\""));
        // The label tab is one focusable rename target (mirrors the sticky's role=button pattern).
        assert!(svg.contains(
            "class=\"region-tab\" data-region=\"K1\" data-label=\"Context A\" role=\"button\" \
                 tabindex=\"0\""
        ));
        // E1 sits on the region's from-edge → a pivotal node; E2 (interior) does not add a third.
        assert_eq!(
            svg.matches("<circle").count(),
            1,
            "expected one pivotal node"
        );
    }

    // Stage 6: `create region` needs a click target even where no region exists yet, so a rail
    // cell must cover every visible column regardless of whether any phase is present.
    #[test]
    fn region_rail_covers_every_visible_column_even_with_no_regions() {
        let m = Model {
            title: "t".into(),
            phases: vec![],
            elements: vec![el("E1", "event", 0), el("E2", "event", 2)],
            edges: vec![],
            diff_meta: None,
        };
        let svg = render_svg(&m);
        for col in 0..=2 {
            assert!(
                svg.contains(&format!("class=\"region-rail\" data-col=\"{col}\"")),
                "missing region-rail cell for col {col}"
            );
        }
    }

    // Review #4: `diff_phases` feeds *removed* regions into `model.phases`. Render must read
    // `Phase.diff` and ghost them — otherwise a removed band paints as a phantom unstyled region,
    // and offers a resize handle for something that no longer exists.
    #[test]
    fn removed_region_is_ghosted_and_drops_its_grab_handle() {
        let m = Model {
            title: "t".into(),
            phases: vec![phase("K9", "Gone", 0, 1, Some("removed"))],
            elements: vec![el("E1", "event", 0)],
            edges: vec![],
            diff_meta: Some(("v1".into(), "v2".into())),
        };
        let svg = render_svg(&m);
        assert!(
            svg.contains("<g opacity=\"0.45\">"),
            "removed region is not ghosted"
        );
        // The region still carries an identifying group (Stage 6: the client needs `data-region`
        // to tell regions apart even when removed), but must not offer a resize handle or a
        // rename tab for something that no longer exists.
        assert!(
            !svg.contains("class=\"region-edge\" data-region=\"K9\""),
            "a removed region must not offer a resize handle"
        );
        assert!(
            !svg.contains("class=\"region-tab\" data-region=\"K9\""),
            "a removed region must not offer a rename tab"
        );
    }

    // Read the data-cy a given element group carries, so a test can correlate id → centre.
    fn cy_of(svg: &str, id: &str) -> f64 {
        let g = format!("<g id=\"{}\"", id);
        let i = svg.find(&g).expect("element group");
        let rest = &svg[i..];
        let key = "data-cy=\"";
        let j = rest.find(key).unwrap() + key.len();
        rest[j..][..rest[j..].find('"').unwrap()].parse().unwrap()
    }

    fn el(id: &str, kind: &str, col: i64) -> Element {
        Element {
            id: id.into(),
            kind: kind.into(),
            label: "L".into(),
            col: Some(col),
            detail: None,
            y: None,
            resolved: false,
            diff: None,
            was: None,
        }
    }

    // Lever A (F-edge-routing): a crowded cell stacks its members by the mean lane of their edge
    // neighbours, not file order — a sticky wired to a lane *above* takes the upper sub-row even
    // when it appears later in the file. Two events share (event, col 1); `E_lo` links up to an
    // actor, `E_hi` links down to a read model. File order lists `E_hi` first, so the old file-order
    // packing put it on top; barycenter ordering must flip them so the connectors don't cross.
    #[test]
    fn crowded_cell_orders_members_by_neighbour_barycenter() {
        let m = Model {
            title: "t".into(),
            phases: vec![],
            elements: vec![
                el("X1", "actor", 0),
                el("R1", "readmodel", 2),
                el("E_hi", "event", 1),
                el("E_lo", "event", 1),
            ],
            edges: vec![
                Edge {
                    src: "X1".into(),
                    dst: "E_lo".into(),
                    status: None,
                },
                Edge {
                    src: "E_hi".into(),
                    dst: "R1".into(),
                    status: None,
                },
            ],
            diff_meta: None,
        };
        let svg = render_svg(&m);
        assert!(
            cy_of(&svg, "E_lo") < cy_of(&svg, "E_hi"),
            "E_lo (up-neighbour) must take the upper sub-row"
        );
    }

    // An edge-free crowded cell has no barycenter signal, so it must keep file order untouched —
    // this is what guarantees the packing tests above stay green. Five events, no edges: their
    // centres must descend in file order E0..E4 (Rows packing stacks top→bottom).
    #[test]
    fn edge_free_cell_keeps_file_order() {
        let svg = render_svg(&events_at_col(2, 5));
        let cys: Vec<f64> = (0..5).map(|k| cy_of(&svg, &format!("E{k}"))).collect();
        assert!(
            cys.windows(2).all(|w| w[0] < w[1]),
            "edge-free cell reordered: {cys:?}"
        );
    }

    // Lever B (F-edge-routing): the fan-out offset must be a pure addition — offset 0 reproduces
    // the classic centre-to-centre path byte-for-byte (no regression on the lone-edge common case),
    // and a non-zero offset slides only the anchor along its facing edge (Y for a horizontal facing),
    // never the opposite axis. p1→p2 is horizontal facing (dx = 400 ≥ STICKY_W).
    #[test]
    fn edge_path_offset_zero_is_classic_and_offset_slides_the_anchor() {
        let p1 = (100.0, 200.0);
        let p2 = (500.0, 260.0);
        assert_eq!(
            edge_path(p1, p2, 0.0, 0.0),
            "M188.0,200.0 C300.0,200.0 300.0,260.0 412.0,260.0"
        );
        // +12 at the source slides that anchor (and its control point) down 12px in Y only.
        assert_eq!(
            edge_path(p1, p2, 12.0, 0.0),
            "M188.0,212.0 C300.0,212.0 300.0,260.0 412.0,260.0"
        );
    }

    // Two flow edges leaving the same actor on its right face must fan to distinct anchor Ys, so the
    // bundle reads as two lines, not one. `X1` issues `C1` (above) and `C2` (below); the connectors
    // share the actor's right face and must not start at the same point.
    #[test]
    fn sibling_edges_fan_apart_at_a_shared_face() {
        let m = Model {
            title: "t".into(),
            phases: vec![],
            elements: vec![
                el("X1", "actor", 0),
                el("C1", "command", 1),
                el("C2", "command", 1),
            ],
            edges: vec![
                Edge {
                    src: "X1".into(),
                    dst: "C1".into(),
                    status: None,
                },
                Edge {
                    src: "X1".into(),
                    dst: "C2".into(),
                    status: None,
                },
            ],
            diff_meta: None,
        };
        let svg = render_svg(&m);
        // Each edge path's start anchor Y (the M y-coord). They must differ by the fan spread.
        let starts: Vec<f64> = svg
            .match_indices("<path class=\"edge\"")
            .map(|(i, _)| {
                let m0 = svg[i..].find('M').unwrap() + i + 1;
                let seg = &svg[m0..];
                let comma = seg.find(',').unwrap();
                let end = seg.find(' ').unwrap();
                seg[comma + 1..end].parse().unwrap()
            })
            .collect();
        assert_eq!(starts.len(), 2, "expected two flow edges");
        assert!(
            (starts[0] - starts[1]).abs() > 1.0,
            "sibling edges share an anchor Y: {starts:?}"
        );
    }

    // Lever B clamp (F-edge-routing): one actor wired to 9 commands on its right → 9 connectors
    // share the actor's right face. Unclamped, the extreme fan offset (FAN_SPREAD·(9-1)/2 = 48)
    // exceeds the face half-extent (STICKY_H/2 = 37) and would start a connector off the box; the
    // clamp must tighten the step so every anchor stays on the actor.
    #[test]
    fn fan_clamp_keeps_anchors_on_the_box_for_a_high_degree_face() {
        let mut elements = vec![el("X1", "actor", 0)];
        let mut edges = vec![];
        for k in 0..9 {
            elements.push(el(&format!("C{k}"), "command", 1));
            edges.push(Edge {
                src: "X1".into(),
                dst: format!("C{k}"),
                status: None,
            });
        }
        let m = Model {
            title: "t".into(),
            phases: vec![],
            elements,
            edges,
            diff_meta: None,
        };
        let svg = render_svg(&m);
        let cy = cy_of(&svg, "X1");
        let mut count = 0;
        for (i, _) in svg.match_indices("data-src=\"X1\"") {
            let after = &svg[i + svg[i..].find('M').unwrap() + 1..];
            let comma = after.find(',').unwrap();
            let end = after.find(' ').unwrap();
            let y: f64 = after[comma + 1..end].parse().unwrap();
            assert!(
                (y - cy).abs() <= STICKY_H / 2.0 + 0.05,
                "anchor slid off the box: y={y}, cy={cy}"
            );
            count += 1;
        }
        assert_eq!(count, 9, "expected 9 fanned connectors");
    }

    #[test]
    fn render_html_injects_the_geometry_config() {
        let html = render_html("<svg></svg>", "t");
        assert!(!html.contains("__CONFIG__"));
        assert!(html.contains("\"colW\":210"));
        assert!(html.contains("\"stickyW\":176"));
    }

    #[test]
    fn wrap_fits_short_labels_and_ellipsises_overflow() {
        assert_eq!(
            wrap("Order Placed", 20, 2),
            vec!["Order Placed".to_string()]
        );
        // Three 4-char tokens, width 4, capped at one line -> truncated with an ellipsis.
        assert_eq!(
            wrap("aaaa bbbb cccc", 4, 1),
            vec!["aaa\u{2026}".to_string()]
        );
    }
}
