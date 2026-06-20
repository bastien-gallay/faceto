//! Render a board model into a static SVG and an interactive HTML page.
//!
//! Deterministic, pure std. The colour grammar (one type → one colour → one lane) and the
//! whole visual language are ported faithfully from the original Python harness.

use crate::model::{Element, Model};
use std::collections::HashMap;

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

const COL_W: f64 = 210.0;
// When a (lane, col) cell holds several simultaneous stickies they unpack into a sub-grid. A
// sub-row adds ROW_PITCH of height; a sub-column adds SUBCOL_W of width. LANE_VPAD keeps a single
// -row lane at the classic 108px (92 + 16), so uncrowded boards look exactly as before whatever the
// packing.
const ROW_PITCH: f64 = 92.0;
const SUBCOL_W: f64 = 190.0;
const LANE_VPAD: f64 = 16.0;
const MARGIN_L: f64 = 150.0;
const MARGIN_T: f64 = 116.0;
const STICKY_W: f64 = 176.0;
const STICKY_H: f64 = 74.0;

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

/// A smooth connector between two box centres, anchored on the facing edges.
fn edge_path(p1: (f64, f64), p2: (f64, f64)) -> String {
    let (x1, y1) = p1;
    let (x2, y2) = p2;
    if (x2 - x1).abs() < STICKY_W {
        let sgn = if y2 >= y1 { 1.0 } else { -1.0 };
        let ay1 = y1 + sgn * STICKY_H / 2.0;
        let ay2 = y2 - sgn * STICKY_H / 2.0;
        let my = (ay1 + ay2) / 2.0;
        format!(
            "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            x1, ay1, x1, my, x2, my, x2, ay2
        )
    } else {
        let sgn = if x2 >= x1 { 1.0 } else { -1.0 };
        let ax1 = x1 + sgn * STICKY_W / 2.0;
        let ax2 = x2 - sgn * STICKY_W / 2.0;
        let mx = (ax1 + ax2) / 2.0;
        format!(
            "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            ax1, y1, mx, y1, mx, y2, ax2, y2
        )
    }
}

/// How a cell of several *simultaneous* stickies (same lane + col) is unpacked so none hide. `col`
/// stays the timeline — we never spread members across fake columns — only their pixels move.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Packing {
    /// Stack into sub-rows: the board grows taller. The calm default.
    #[default]
    Rows,
    /// Fan into sub-columns: the board grows wider (a timeline scrolls sideways naturally).
    Columns,
    /// Pack into a near-square sub-grid: height and width grow modestly. The balanced mix.
    Grid,
}

impl Packing {
    /// Parse a `--pack` flag / `?pack=` query value; anything unrecognised falls back to `Rows`.
    pub fn parse(s: &str) -> Packing {
        match s.trim().to_ascii_lowercase().as_str() {
            "columns" | "column" | "cols" | "col" => Packing::Columns,
            "grid" | "mix" | "mixed" => Packing::Grid,
            _ => Packing::Rows,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Packing::Rows => "rows",
            Packing::Columns => "columns",
            Packing::Grid => "grid",
        }
    }

    /// The sub-grid `(cols, rows)` that a cell of `n` simultaneous stickies unpacks into.
    fn cell_grid(self, n: i64) -> (i64, i64) {
        let n = n.max(1);
        match self {
            Packing::Rows => (1, n),
            Packing::Columns => (n, 1),
            Packing::Grid => {
                let cols = (n as f64).sqrt().ceil() as i64;
                (cols, (n + cols - 1) / cols)
            }
        }
    }
}

pub fn render_svg_packed(model: &Model, packing: Packing) -> String {
    let mut elements = model.elements.clone();

    let present: Vec<&str> = LANES
        .iter()
        .cloned()
        .filter(|t| elements.iter().any(|e| e.kind == *t))
        .collect();

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
    // they happen). Instead each cell unpacks into a sub-grid (`Packing`): `sub_ord[i]` is element
    // i's slot within its cell (file order), `cell_total` the cell's count.
    let mut sub_ord: Vec<i64> = Vec::with_capacity(elements.len());
    let mut cell_total: HashMap<(String, i64), i64> = HashMap::new();
    for e in &elements {
        let n = cell_total
            .entry((e.kind.clone(), e.col.unwrap()))
            .or_insert(0);
        sub_ord.push(*n);
        *n += 1;
    }

    // Each timeline column is as wide as its busiest cell's sub-columns demand; each lane as tall
    // as its deepest cell's sub-rows. The chosen packing decides how a cell's count splits between
    // the two, so the same board can grow tall (Rows), wide (Columns), or both modestly (Grid).
    let mut col_subcols = vec![1i64; ncols];
    let mut lane_rows: HashMap<&str, i64> = present.iter().map(|t| (*t, 1)).collect();
    for ((kind, col), total) in &cell_total {
        let (cc, cr) = packing.cell_grid(*total);
        let c = &mut col_subcols[(*col - min_col) as usize];
        *c = (*c).max(cc);
        let r = lane_rows.get_mut(kind.as_str()).unwrap();
        *r = (*r).max(cr);
    }

    // Column x positions (cumulative widths) and lane y positions (cumulative heights).
    let mut col_left = vec![0.0_f64; ncols];
    let mut col_width = vec![COL_W; ncols];
    let mut x = MARGIN_L;
    for c in 0..ncols {
        let w = if col_subcols[c] <= 1 {
            COL_W
        } else {
            col_subcols[c] as f64 * SUBCOL_W
        };
        col_left[c] = x;
        col_width[c] = w;
        x += w;
    }
    let board_right = x;

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

    // Resolve every element to an absolute centre. Its cell's sub-grid is centred within the room
    // the column/lane reserved (`lead_*` slots of slack on each side), so a lone sticky keeps its
    // classic mid-lane position and only crowded cells expand symmetrically around it.
    let centers: Vec<(f64, f64)> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let col = e.col.unwrap();
            let (cc, cr) = packing.cell_grid(cell_total[&(e.kind.clone(), col)]);
            let (sc, sr) = (sub_ord[i] % cc, sub_ord[i] / cc);
            let c = (col - min_col) as usize;
            let slot_w = col_width[c] / col_subcols[c] as f64;
            let lead_c = (col_subcols[c] - cc) as f64 / 2.0;
            let cx = col_left[c] + (lead_c + sc as f64 + 0.5) * slot_w;
            let lead_r = (lane_rows[e.kind.as_str()] - cr) as f64 / 2.0;
            let cy = lane_top[&e.kind] + LANE_VPAD / 2.0 + (lead_r + sr as f64 + 0.5) * ROW_PITCH;
            (cx, cy)
        })
        .collect();

    let width = (board_right + 40.0) as i64;
    let height = (lanes_bottom + 60.0) as i64;

    let by_id: HashMap<String, usize> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.clone(), i))
        .collect();

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

    // Phase bands (soft vertical zones behind the timeline).
    let band_bot = lanes_bottom - 6.0;
    for ph in &model.phases {
        let cols: Vec<i64> = elements
            .iter()
            .map(|e| e.col.unwrap())
            .filter(|c| ph.from_col <= *c && *c <= ph.to_col)
            .collect();
        if cols.is_empty() {
            continue;
        }
        let minc = (*cols.iter().min().unwrap() - min_col) as usize;
        let maxc = (*cols.iter().max().unwrap() - min_col) as usize;
        let x = col_left[minc];
        let w = col_left[maxc] + col_width[maxc] - x;
        p.push(format!(
            "<rect x=\"{:.1}\" y=\"{}\" width=\"{:.1}\" height=\"{}\" fill=\"#000\" opacity=\"0.02\"/>",
            x,
            MARGIN_T - 26.0,
            w,
            band_bot - MARGIN_T + 26.0
        ));
        p.push(format!(
            "<line x1=\"{:.1}\" y1=\"{}\" x2=\"{:.1}\" y2=\"{}\" stroke=\"#e0e0e6\"/>",
            x,
            MARGIN_T - 26.0,
            x,
            band_bot
        ));
        p.push(format!(
            "<text x=\"{:.1}\" y=\"{}\" font-size=\"12\" font-weight=\"600\" fill=\"#90a4ae\">{}</text>",
            x + 10.0,
            MARGIN_T - 32.0,
            esc(&ph.label)
        ));
    }

    // Time-slot trays — a faint rounded backing hugging every cell that holds more than one sticky,
    // so a fanned/stacked group still reads as "these are simultaneous" however it is packed. Sorted
    // for deterministic output (HashMap order is not stable).
    let mut cell_box: HashMap<(String, i64), (f64, f64, f64, f64)> = HashMap::new();
    for (i, e) in elements.iter().enumerate() {
        let (cx, cy) = centers[i];
        let b = cell_box
            .entry((e.kind.clone(), e.col.unwrap()))
            .or_insert((cx, cy, cx, cy));
        b.0 = b.0.min(cx);
        b.1 = b.1.min(cy);
        b.2 = b.2.max(cx);
        b.3 = b.3.max(cy);
    }
    let mut cells: Vec<&(String, i64)> = cell_box.keys().filter(|k| cell_total[*k] > 1).collect();
    cells.sort();
    for k in cells {
        let (minx, miny, maxx, maxy) = cell_box[k];
        let pad = 9.0;
        p.push(format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"12\" \
             fill=\"#90a4ae\" opacity=\"0.1\"/>",
            minx - STICKY_W / 2.0 - pad,
            miny - STICKY_H / 2.0 - pad,
            (maxx - minx) + STICKY_W + 2.0 * pad,
            (maxy - miny) + STICKY_H + 2.0 * pad,
        ));
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

    // Lane labels — centred on each lane's (possibly multi-row) band.
    for t in &present {
        let y = lane_top[*t] + lane_h[*t] / 2.0;
        p.push(format!(
            "<text x=\"16\" y=\"{:.1}\" font-size=\"12\" font-weight=\"600\" fill=\"#90a4ae\">{}</text>",
            y + 4.0,
            esc(t)
        ));
    }

    // Edges (under the stickies). A hotspot connector is a concern, not a flow: dotted, arrow-less.
    for edge in &model.edges {
        let (si, di) = match (by_id.get(&edge.src), by_id.get(&edge.dst)) {
            (Some(&si), Some(&di)) => (si, di),
            _ => continue,
        };
        let d = edge_path(centers[si], centers[di]);
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
        // data-kind / data-col / data-cx / data-cy let the client replay a move (translate the
        // group, recompute its edges) without a server round-trip — see template.html.
        p.push(format!(
            "<g id=\"{}\" class=\"{}\" data-hero=\"{}\" data-detail=\"{}\" data-kind=\"{}\" \
             data-col=\"{}\" data-cx=\"{:.1}\" data-cy=\"{:.1}\" style=\"cursor:pointer\"{}>",
            esc(&e.id),
            cls,
            esc(&hero),
            esc(&detail),
            esc(&e.kind),
            e.col.unwrap(),
            cx,
            cy,
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

pub fn render_html(svg: &str, title: &str, packing: Packing) -> String {
    // The client reuses these geometry constants to re-place a moved sticky and redraw its edges
    // in the browser, and `pack` so its packing control opens on the mode the SVG was rendered in —
    // keep render.rs the single source of truth for them.
    let cfg = format!(
        "{{\"colW\":{},\"stickyW\":{},\"stickyH\":{},\"pack\":\"{}\"}}",
        COL_W,
        STICKY_W,
        STICKY_H,
        packing.as_str()
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

    #[test]
    fn the_add_element_picker_offers_every_lane() {
        // The client's <select id="m-type"> is a hand-written copy of the lane grammar; pin it to
        // LANES so a new/renamed lane can't silently drop out of the "add element" picker.
        for lane in LANES {
            assert!(
                HTML_TEMPLATE.contains(&format!("<option value=\"{lane}\"")),
                "template.html #m-type is missing lane `{lane}`"
            );
        }
    }

    #[test]
    fn sticky_group_exposes_layout_data_attributes() {
        let svg = render_svg_packed(&one_event_at_col(2), Packing::default());
        assert!(svg.contains("data-kind=\"event\""));
        assert!(svg.contains("data-col=\"2\""));
        assert!(svg.contains("data-cx="));
        assert!(svg.contains("data-cy="));
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
                    resolved: false,
                    diff: None,
                    was: None,
                })
                .collect(),
            edges: vec![],
            diff_meta: None,
        }
    }

    // The faithfulness contract: simultaneous stickies (same lane + col) must never render on top
    // of one another. They stack into sub-rows, so every centre is unique — no element is hidden.
    #[test]
    fn simultaneous_stickies_stack_into_distinct_centres() {
        let svg = render_svg_packed(&events_at_col(2, 5), Packing::default());
        let cys = attr_values(&svg, "data-cy");
        assert_eq!(cys.len(), 5);
        let unique: std::collections::HashSet<&String> = cys.iter().collect();
        assert_eq!(unique.len(), 5, "stacked stickies share a centre: {cys:?}");
    }

    fn distinct(svg: &str, attr: &str) -> usize {
        attr_values(svg, attr)
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    // Each packing keeps every centre distinct (nothing hidden) but grows a different axis:
    // Columns fans 4 stickies across 4 x-positions on one row; Rows stacks them down one column;
    // Grid splits 4 into a 2×2 block. This is the whole point of the three modes.
    #[test]
    fn packing_chooses_its_growth_axis() {
        let cols = render_svg_packed(&events_at_col(0, 4), Packing::Columns);
        assert_eq!(
            (distinct(&cols, "data-cx"), distinct(&cols, "data-cy")),
            (4, 1)
        );

        let rows = render_svg_packed(&events_at_col(0, 4), Packing::Rows);
        assert_eq!(
            (distinct(&rows, "data-cx"), distinct(&rows, "data-cy")),
            (1, 4)
        );

        let grid = render_svg_packed(&events_at_col(0, 4), Packing::Grid);
        assert_eq!(
            (distinct(&grid, "data-cx"), distinct(&grid, "data-cy")),
            (2, 2)
        );
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
            resolved: false,
            diff: None,
            was: None,
        });
        let svg = render_svg_packed(&m, Packing::Grid);
        assert_eq!(distinct(&svg, "data-cx"), 2);
        // col -3 is the leftmost authored column → slot 0 → classic single-cell centre 255.0.
        let cxs = attr_values(&svg, "data-cx");
        assert!(cxs.contains(&"255.0".to_string()), "got {cxs:?}");
    }

    #[test]
    fn packing_parses_aliases_and_falls_back_to_rows() {
        assert_eq!(Packing::parse("columns"), Packing::Columns);
        assert_eq!(Packing::parse("COL"), Packing::Columns);
        assert_eq!(Packing::parse("grid"), Packing::Grid);
        assert_eq!(Packing::parse("mix"), Packing::Grid);
        assert_eq!(Packing::parse("nonsense"), Packing::Rows);
        assert_eq!(Packing::Grid.as_str(), "grid");
    }

    // A lone sticky keeps its classic position: centred on a single-row lane, no horizontal fan.
    #[test]
    fn a_lone_sticky_stays_on_the_lane_mid_line() {
        let svg = render_svg_packed(&events_at_col(0, 1), Packing::default());
        // lane_top (MARGIN_T) + LANE_VPAD/2 + ROW_PITCH/2 = 116 + 8 + 46.
        assert_eq!(attr_values(&svg, "data-cy"), vec!["170.0".to_string()]);
        // col 0 centre, no stagger: MARGIN_L + COL_W/2 = 150 + 105.
        assert_eq!(attr_values(&svg, "data-cx"), vec!["255.0".to_string()]);
    }

    #[test]
    fn render_html_injects_the_geometry_config() {
        let html = render_html("<svg></svg>", "t", Packing::Grid);
        assert!(!html.contains("__CONFIG__"));
        assert!(html.contains("\"colW\":210"));
        assert!(html.contains("\"stickyW\":176"));
        assert!(html.contains("\"pack\":\"grid\""));
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
