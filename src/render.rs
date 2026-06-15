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
const LANE_H: f64 = 108.0;
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

pub fn render_svg(model: &Model) -> String {
    let mut elements = model.elements.clone();

    let present: Vec<&str> = LANES
        .iter()
        .cloned()
        .filter(|t| elements.iter().any(|e| e.kind == *t))
        .collect();
    let mut lane_y: HashMap<String, f64> = HashMap::new();
    for (i, t) in present.iter().enumerate() {
        lane_y.insert((*t).to_string(), MARGIN_T + i as f64 * LANE_H);
    }

    // Auto-assign a column to any element missing `col`, preserving file order.
    let mut auto: i64 = 0;
    for e in elements.iter_mut() {
        if e.col.is_none() {
            e.col = Some(auto);
            auto += 1;
        }
    }
    for e in elements.iter_mut() {
        e.x = e.col.unwrap() as f64;
    }
    let max_x = elements.iter().map(|e| e.x).fold(0.0_f64, f64::max);

    let width = (MARGIN_L + (max_x + 1.0) * COL_W + 40.0) as i64;
    let height = (MARGIN_T + present.len() as f64 * LANE_H + 60.0) as i64;

    let by_id: HashMap<String, usize> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.clone(), i))
        .collect();
    let center = |e: &Element| -> (f64, f64) {
        (
            MARGIN_L + e.x * COL_W + COL_W / 2.0,
            lane_y[&e.kind] + LANE_H / 2.0,
        )
    };

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
    p.push(format!(
        "<text x=\"20\" y=\"34\" font-size=\"20\" font-weight=\"700\" fill=\"#222\">{}</text>",
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
    let band_bot = MARGIN_T + present.len() as f64 * LANE_H - 6.0;
    for ph in &model.phases {
        let xs: Vec<f64> = elements
            .iter()
            .filter(|e| {
                let c = e.col.unwrap();
                ph.from_col <= c && c <= ph.to_col
            })
            .map(|e| e.x)
            .collect();
        if xs.is_empty() {
            continue;
        }
        let minx = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let maxx = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let x = MARGIN_L + minx * COL_W;
        let w = (maxx - minx + 1.0) * COL_W;
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

    // Lane labels
    for t in &present {
        let y = lane_y[*t] + LANE_H / 2.0;
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
        let d = edge_path(center(&elements[si]), center(&elements[di]));
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
    for e in &elements {
        let (cx, cy) = center(e);
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
        p.push(format!(
            "<g id=\"{}\" class=\"{}\" data-hero=\"{}\" data-detail=\"{}\" style=\"cursor:pointer\"{}>",
            esc(&e.id),
            cls,
            esc(&hero),
            esc(&detail),
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
    HTML_TEMPLATE
        .replace("__TITLE__", &esc(title))
        .replace("__SVG__", svg)
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
