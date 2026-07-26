//! Fixed-grid SVG for the canvas.
//!
//! SPIKE FINDING (render, the cheap one). This file reuses **two functions** from the existing
//! renderer — `esc` and `wrap` — and *nothing else*. `render::style` is entirely ES vocabulary
//! (`LANES`, `colour("aggregate")`, `COL_W` as a timeline pitch, `is_pivotal`); `render::geometry`
//! is edge-routing math for a graph the canvas does not have. There was no fight and no
//! refactoring: the ES renderer is simply not in the way, because a slot template needs none of
//! what it offers.
//!
//! SPIKE FINDING (render, the expensive one). The 190 lines below are almost entirely
//! `format!("<rect …>")` string-building, and the diff overlay branches inside every mark — the
//! same immediate-mode structure `render::svg` has, reproduced from scratch. **This is the case
//! for the Scene IR** (`docs/multi-format-architecture.md` §The render contract): with a data
//! `Scene`, this file would end at "emit a `Vec<Shape>`" and the overlay styling, the serializer
//! and the client's hit-testing would all be inherited. The spike did not build a Scene IR — the
//! point was to feel the absence, and the absence is a per-format SVG string-builder, exactly as
//! predicted.
//!
//! Layout is a fixed 3 × 4 grid. There is no algorithm — [`GRID`] *is* the layout.

use super::model::{Canvas, Item, Slot, SLOTS};
use crate::render::{esc, wrap};

const COL_W: f64 = 300.0;
const GAP: f64 = 14.0;
const PAD: f64 = 12.0;
const MARGIN: f64 = 28.0;
const HEAD_H: f64 = 22.0;
const LINE_H: f64 = 15.0;
const ITEM_PAD: f64 = 9.0;
const NAMEPLATE_H: f64 = 52.0;
const WRAP_COLS: usize = 34;

/// Board surface + ink. Deliberately *not* `render::style::colour`: that function is keyed on the
/// eight event-storming lane names and answers `#cccccc` for everything here.
const PAPER: &str = "#fbfbfd";
const CARD: &str = "#ffffff";
const RULE: &str = "#dfe4e9";
const INK: &str = "#2c3a42";
const MUTED: &str = "#5b6b75";
/// One tint per slot, so the sections read as a template rather than a list. Low-chroma on
/// purpose — the canvas is prose, and colour here is grouping, not grammar (`DESIGN.md`: the
/// board is the subject, the UI is glass).
fn tint(slot: Slot) -> &'static str {
    match slot {
        Slot::Purpose => "#eef3f7",
        Slot::Classification => "#f2eef7",
        Slot::Roles => "#eef6f1",
        Slot::Inbound => "#fdf3e8",
        Slot::Outbound => "#fdf0f5",
        Slot::Language => "#f4f4ee",
        Slot::Decisions => "#eef2fb",
        Slot::Assumptions => "#f7f2ee",
        Slot::Metrics => "#eef5f6",
        Slot::Questions => "#f8eeee",
    }
}

/// Verdict colours, lifted 1:1 from `render::style::diff_colour` — with `reslotted` added and
/// `moved` never produced. Copied rather than shared because that function is `pub(crate)` to
/// `render`; sharing it is another kernel extraction, though a trivial one.
fn diff_colour(v: &str) -> &'static str {
    match v {
        "added" => "#27ae60",
        "removed" => "#EB5757",
        "changed" | "reslotted" => "#E59500",
        _ => "#999999",
    }
}

fn diff_badge(v: &str) -> Option<&'static str> {
    match v {
        "added" => Some("+"),
        "removed" => Some("\u{2013}"),
        "changed" => Some("\u{2260}"),
        "reslotted" => Some("\u{21C4}"), // ⇄ — a swap, not the ES → of a spatial move
        _ => None,
    }
}

/// The layout, as data: `(slot, row, col, colspan)`. Four rows of three columns.
const GRID: [(Slot, usize, usize, usize); 10] = [
    (Slot::Purpose, 0, 0, 3),
    (Slot::Classification, 1, 0, 1),
    (Slot::Roles, 1, 1, 1),
    (Slot::Language, 1, 2, 1),
    (Slot::Inbound, 2, 0, 1),
    (Slot::Decisions, 2, 1, 1),
    (Slot::Outbound, 2, 2, 1),
    (Slot::Assumptions, 3, 0, 1),
    (Slot::Metrics, 3, 1, 1),
    (Slot::Questions, 3, 2, 1),
];
const ROWS: usize = 4;

fn cell(slot: Slot) -> (usize, usize, usize) {
    let (_, r, c, span) = GRID.iter().find(|g| g.0 == slot).copied().unwrap();
    (r, c, span)
}

fn span_w(span: usize) -> f64 {
    COL_W * span as f64 + GAP * (span as f64 - 1.0)
}

/// An item's rendered lines: the text, plus a `via` line for inbound/outbound messages.
fn item_lines(i: &Item) -> Vec<String> {
    let mut lines = wrap(&i.text, WRAP_COLS, 3);
    if let Some(v) = &i.via {
        lines.push(format!("\u{2194} {v}"));
    }
    lines
}

fn item_h(i: &Item) -> f64 {
    item_lines(i).len() as f64 * LINE_H + ITEM_PAD * 2.0
}

fn section_h(c: &Canvas, slot: Slot) -> f64 {
    let items = c.slot_items(slot);
    let body: f64 = items.iter().map(|i| item_h(i) + 6.0).sum();
    HEAD_H + PAD + body.max(LINE_H) + PAD
}

/// Render a canvas (or a diff overlay, when `diff_meta` is set) to standalone SVG.
pub fn render_svg(c: &Canvas) -> String {
    // Row heights: each row is as tall as its tallest section. That is the whole layout pass.
    let mut row_h = [0.0_f64; ROWS];
    for slot in SLOTS {
        let (r, _, _) = cell(slot);
        row_h[r] = row_h[r].max(section_h(c, slot));
    }
    let row_y = |r: usize| MARGIN + NAMEPLATE_H + row_h[..r].iter().map(|h| h + GAP).sum::<f64>();
    let width = MARGIN * 2.0 + span_w(3);
    let height = row_y(ROWS) + MARGIN;

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" \
         viewBox=\"0 0 {w:.0} {h:.0}\" font-family=\"Inter, -apple-system, Segoe UI, sans-serif\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"{PAPER}\"/>\n",
        w = width,
        h = height
    ));

    // Nameplate — the serif title bar, matching the ES board's register.
    s.push_str(&format!(
        "<text x=\"{x:.0}\" y=\"{y:.0}\" font-family=\"Georgia, serif\" font-size=\"21\" \
         fill=\"{INK}\">{name}</text>\n",
        x = MARGIN,
        y = MARGIN + 20.0,
        name = esc(&c.name)
    ));
    s.push_str(&format!(
        "<text x=\"{x:.0}\" y=\"{y:.0}\" font-size=\"11\" fill=\"{MUTED}\">Bounded Context Canvas{extra}</text>\n",
        x = MARGIN,
        y = MARGIN + 37.0,
        extra = match &c.diff_meta {
            Some((a, b)) => esc(&format!(" \u{00B7} {a} \u{2192} {b}")),
            None => String::new(),
        }
    ));

    for slot in SLOTS {
        let (r, col, span) = cell(slot);
        let x = MARGIN + (COL_W + GAP) * col as f64;
        let y = row_y(r);
        s.push_str(&section(c, slot, x, y, span_w(span), row_h[r]));
    }
    s.push_str("</svg>\n");
    s
}

fn section(c: &Canvas, slot: Slot, x: f64, y: f64, w: f64, h: f64) -> String {
    let mut s = format!(
        "<g data-slot=\"{key}\">\n\
         <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"6\" \
         fill=\"{tint}\" stroke=\"{RULE}\"/>\n\
         <text x=\"{tx:.1}\" y=\"{ty:.1}\" font-size=\"10.5\" letter-spacing=\"0.6\" \
         fill=\"{MUTED}\">{title}</text>\n",
        key = slot.key(),
        tint = tint(slot),
        tx = x + PAD,
        ty = y + HEAD_H,
        title = esc(&slot.title().to_uppercase())
    );

    let mut cursor = y + HEAD_H + PAD;
    for item in c.slot_items(slot) {
        let ih = item_h(item);
        s.push_str(&item_svg(
            item,
            x + PAD,
            cursor,
            w - PAD * 2.0,
            ih,
            &c.diff_meta,
        ));
        cursor += ih + 6.0;
    }
    s.push_str("</g>\n");
    s
}

fn item_svg(i: &Item, x: f64, y: f64, w: f64, h: f64, meta: &Option<(String, String)>) -> String {
    let verdict = i.diff.as_deref().filter(|v| *v != "unchanged");
    let (stroke, dash, opacity) = match verdict {
        Some("removed") => (diff_colour("removed"), " stroke-dasharray=\"4 3\"", 0.55),
        Some(v) => (diff_colour(v), "", 1.0),
        None => (RULE, "", 1.0),
    };
    let width = if verdict.is_some() { 2.0 } else { 1.0 };

    // `data-id` is the client's join key — identical to the ES board's contract, and the one
    // client-facing convention that transferred unchanged.
    let mut s = format!(
        "<g data-id=\"{id}\" opacity=\"{opacity}\">\n\
         <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"4\" \
         fill=\"{CARD}\" stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}/>\n",
        id = esc(&i.id)
    );
    if let (Some(v), Some(meta)) = (verdict, meta) {
        let tip = super::diff::diff_tooltip(i, meta);
        if !tip.is_empty() {
            s.push_str(&format!("<title>{}</title>\n", esc(&tip)));
        }
        if let Some(badge) = diff_badge(v) {
            s.push_str(&format!(
                "<text x=\"{bx:.1}\" y=\"{by:.1}\" font-size=\"12\" font-weight=\"600\" \
                 text-anchor=\"end\" fill=\"{c}\">{badge}</text>\n",
                bx = x + w - 6.0,
                by = y + 14.0,
                c = diff_colour(v)
            ));
        }
    }
    for (n, line) in item_lines(i).iter().enumerate() {
        let muted = line.starts_with('\u{2194}');
        s.push_str(&format!(
            "<text x=\"{tx:.1}\" y=\"{ty:.1}\" font-size=\"12\" fill=\"{fill}\">{line}</text>\n",
            tx = x + ITEM_PAD,
            ty = y + ITEM_PAD + LINE_H * (n as f64 + 0.8),
            fill = if muted { MUTED } else { INK },
            line = esc(line)
        ));
    }
    s.push_str("</g>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::super::diff::diff_canvases;
    use super::*;

    fn sample() -> Canvas {
        let mut i1 = Item::new("I1", Slot::Inbound, "PlaceOrder");
        i1.via = Some("Storefront".into());
        Canvas {
            name: "Orders".into(),
            items: vec![
                Item::new("U1", Slot::Purpose, "accept and track orders"),
                i1,
            ],
            diff_meta: None,
        }
    }

    #[test]
    fn every_slot_is_painted_exactly_once_even_when_empty() {
        let svg = render_svg(&sample());
        for slot in SLOTS {
            assert_eq!(
                svg.matches(&format!("data-slot=\"{}\"", slot.key()))
                    .count(),
                1,
                "{} missing or duplicated",
                slot.key()
            );
        }
    }

    #[test]
    fn the_grid_is_the_layout_no_element_carries_a_coordinate() {
        // SPIKE FINDING. Reversing the *file* changes nothing: rendering walks `SLOTS` and filters,
        // so cross-slot order is not merely unimportant — it is unobservable. There is exactly one
        // ordering left in the whole format (file order *within* one slot), against ES's three
        // interacting ones (`col`, lane index, `y_key` + the barycentre tiebreak in
        // `geometry::cell_sub_order`).
        let a = sample();
        let mut b = a.clone();
        b.items.reverse();
        assert_eq!(
            render_svg(&a),
            render_svg(&b),
            "cross-slot file order is unobservable"
        );
        // Within one slot, file order *is* the order — the format's only ordering rule.
        let one = |first: &str, second: &str| Canvas {
            name: "Orders".into(),
            items: vec![
                Item::new("Q1", Slot::Questions, first),
                Item::new("Q2", Slot::Questions, second),
            ],
            diff_meta: None,
        };
        assert_ne!(
            render_svg(&one("alpha", "beta")),
            render_svg(&one("beta", "alpha"))
        );
        // …but the *sections* never move: the grid is fixed data, not a computed layout.
        let y_of = |svg: &str, key: &str| {
            let g = svg.find(&format!("data-slot=\"{key}\"")).unwrap();
            svg[g..].split("y=\"").nth(1).unwrap()[..6].to_string()
        };
        for slot in SLOTS {
            assert_eq!(
                y_of(&render_svg(&a), slot.key()),
                y_of(&render_svg(&b), slot.key())
            );
        }
    }

    #[test]
    fn an_overlay_marks_each_verdict_and_never_emits_moved() {
        let a = sample();
        let mut b = a.clone();
        b.items[1].slot = Slot::Outbound;
        b.items
            .push(Item::new("Q1", Slot::Questions, "who owns refunds?"));
        let d = diff_canvases(&a, &b, ("before".into(), "after".into()));
        let svg = render_svg(&d);
        assert!(svg.contains("reslotted: Inbound Communication"));
        assert!(svg.contains("added in after"));
        assert!(!svg.contains("moved"), "no spatial verdict exists here");
        assert!(
            svg.contains("before \u{2192} after"),
            "overlay names its baseline"
        );
    }

    #[test]
    fn a_via_line_is_drawn_muted_under_its_message() {
        let svg = render_svg(&sample());
        assert!(svg.contains("\u{2194} Storefront"));
    }
}
