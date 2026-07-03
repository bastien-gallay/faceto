//! Render tests — one module (shared helpers rsvg/el/phase/attr_values).

use super::geometry::*;
use super::html::*;
use super::style::*;
use super::svg::*;
use super::text::*;
use crate::model::{Edge, Element, Level, Model, Phase};

/// Render with the identity lens (nothing collapsed) — the default for every test that isn't
/// exercising F-region-collapse itself, so the `View` argument stays out of the assertions.
fn rsvg(model: &Model) -> String {
    render_svg(model, &View::none())
}

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
        level: Level::default(),
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
        level: Level::default(),
        diff_meta: None,
    }
}

// R: the lane scaffold is the board's structure, not a function of its contents — every lane
// renders even when empty, so an empty board shows all 8 lanes (onboarding) and every lane
// title is a hoverable add-target. Pin all 8 labels on a zero-element board.
#[test]
fn every_lane_renders_even_on_an_empty_board() {
    let svg = rsvg(&empty_board());
    for lane in LANES {
        assert!(
            svg.contains(&format!(">{lane}</text>")),
            "empty board is missing the `{lane}` lane label"
        );
    }
}

#[test]
fn sticky_group_exposes_layout_data_attributes() {
    let svg = rsvg(&one_event_at_col(2));
    assert!(svg.contains("data-kind=\"event\""));
    assert!(svg.contains("data-col=\"2\""));
    assert!(svg.contains("data-cx="));
    assert!(svg.contains("data-cy="));
}

// A sticky is the primary control; it must stay keyboard-reachable and screen-reader-named.
// If these ever stop being emitted the board silently becomes mouse-only again — pin them.
#[test]
fn sticky_group_is_a_focusable_labelled_button() {
    let svg = rsvg(&one_event_at_col(2));
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
        level: Level::default(),
        diff_meta: None,
    }
}

// The faithfulness contract: simultaneous stickies (same lane + col) with no stored `y` must
// never render on top of one another. They auto-stack into sub-rows down one column — one col,
// one x (the packing modes and their sub-columns are gone; F-2d-placement) — so every centre
// is unique and no element is hidden.
#[test]
fn simultaneous_stickies_stack_into_distinct_centres() {
    let svg = rsvg(&events_at_col(2, 5));
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
            attr_values(&rsvg(&m), "data-cy"),
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
        (cy_of(&rsvg(&m), "E0"), cy_of(&rsvg(&m), "E1"))
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
    let svg = rsvg(&empty_board());
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
    let svg = rsvg(&m);
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
    let svg = rsvg(&events_at_col(0, 1));
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

// ---- F-region-collapse -------------------------------------------------------------------
// The svg root's own width (the first `width="…"` in the document is the `<svg>` element).
fn svg_root_width(svg: &str) -> i64 {
    attr_values(svg, "width")[0].parse().unwrap()
}

// A contiguous 3-region partition over cols 0..=5, one event per column, so a fold has clear
// in-band vs out-of-band stickies and neighbours to shift. R1=[0,1] R2=[2,3] R3=[4,5].
fn three_region_board() -> Model {
    Model {
        title: "t".into(),
        phases: vec![
            phase("K1", "Alpha", 0, 1, None),
            phase("K2", "Beta", 2, 3, None),
            phase("K3", "Gamma", 4, 5, None),
        ],
        elements: (0..6).map(|c| el(&format!("E{c}"), "event", c)).collect(),
        edges: vec![],
        level: Level::default(),
        diff_meta: None,
    }
}

fn folded(m: &Model, ids: &[&str]) -> String {
    render_svg(
        m,
        &View {
            collapsed: ids.iter().map(|s| s.to_string()).collect(),
        },
    )
}

// The core fold: a collapsed region's columns compress to one thin slot, its stickies vanish
// behind a `· N` count chip, and the board actually gets shorter (the whole point).
#[test]
fn collapsing_a_region_folds_its_columns_hides_its_stickies_and_shortens_the_board() {
    let m = three_region_board();
    let plain = rsvg(&m);
    let f = folded(&m, &["K2"]);
    assert!(
        svg_root_width(&f) < svg_root_width(&plain),
        "collapsing K2 did not shorten the board ({} !< {})",
        svg_root_width(&f),
        svg_root_width(&plain)
    );
    // K2's in-band stickies are gone; its neighbours' stay.
    assert!(!f.contains("id=\"E2\""), "E2 (in K2) should be hidden");
    assert!(!f.contains("id=\"E3\""), "E3 (in K2) should be hidden");
    assert!(f.contains("id=\"E1\"") && f.contains("id=\"E4\""));
    // The chip carries the in-band count (2 hidden stickies) and the folded triangle "▸".
    assert!(f.contains("Beta \u{00b7} 2"), "count chip missing");
    assert!(f.contains("\u{25b8}"), "folded disclosure triangle missing");
    assert!(f.contains("data-region=\"K2\" data-label=\"Beta\" data-collapsed=\"true\""));
}

// Pure-remap contract 1: an empty collapsed set — or one naming only unknown ids (a stale fold
// of a since-removed region) — is byte-identical to the plain render. This is what keeps the
// static `render`/`genesis` output untouched by the feature.
#[test]
fn empty_or_unknown_collapse_set_is_the_identity_render() {
    let m = three_region_board();
    assert_eq!(rsvg(&m), folded(&m, &[]));
    assert_eq!(rsvg(&m), folded(&m, &["ZZ-not-a-region"]));
}

// Pure-remap contract 2: the fold is a set operation — order-independent and idempotent, the
// same determinism bar as `replay`/`normalize`.
#[test]
fn collapse_is_order_independent_and_idempotent() {
    let m = three_region_board();
    assert_eq!(folded(&m, &["K1", "K3"]), folded(&m, &["K3", "K1"]));
    assert_eq!(folded(&m, &["K1", "K1"]), folded(&m, &["K1"]));
}

// Adjacent folded regions each keep their own summary slot (two chips, not a merged band) —
// the contiguous partition (F-region-frontiers) folds band-by-band.
#[test]
fn adjacent_collapsed_regions_fold_independently() {
    let m = three_region_board();
    let f = folded(&m, &["K1", "K2"]);
    assert!(f.contains("Alpha \u{00b7} 2") && f.contains("Beta \u{00b7} 2"));
    for id in ["E0", "E1", "E2", "E3"] {
        assert!(
            !f.contains(&format!("id=\"{id}\"")),
            "{id} should be hidden"
        );
    }
    assert!(f.contains("id=\"E4\""), "K3 (unfolded) keeps its stickies");
}

// An edge with an endpoint *inside* a folded band is dropped with its hidden node; an edge that
// merely *crosses* the band (both ends visible) is left as a straight passthrough — rerouting it
// to the band frontier is F-region-edge-fold, deliberately out of v1.
#[test]
fn edges_into_a_folded_band_drop_but_crossing_edges_pass_through() {
    let mut m = three_region_board();
    m.edges = vec![
        Edge {
            src: "E2".into(),
            dst: "E3".into(),
            status: None,
        }, // wholly inside K2
        Edge {
            src: "E1".into(),
            dst: "E4".into(),
            status: None,
        }, // crosses K2, both ends visible
    ];
    let f = folded(&m, &["K2"]);
    assert!(
        !f.contains("data-src=\"E2\" data-dst=\"E3\""),
        "an edge inside the folded band must drop with its hidden nodes"
    );
    assert!(
        f.contains("data-src=\"E1\" data-dst=\"E4\""),
        "a crossing edge (both ends visible) stays a passthrough in v1"
    );
}

// A region whose stored span sits entirely PAST the last element column (F-region-frontiers lets
// an outer frontier run past content) has no on-board columns to fold. Folding it must be a no-op,
// not clamp onto the last content column and hide a *neighbour's* sticky (nor draw a bogus chip).
#[test]
fn folding_an_out_of_content_region_is_a_no_op() {
    let mut m = three_region_board(); // elements in cols 0..=5, ncols = 6
                                      // A trailing region past all content — clamp_idx(8)=clamp_idx(9)=5 would otherwise pin it onto
                                      // col 5 (owned by K3) and hide E5.
    m.phases.push(phase("K9", "Ghosttail", 8, 9, None));
    let f = folded(&m, &["K9"]);
    assert!(
        f.contains("id=\"E5\""),
        "folding an out-of-content region must not hide the last content column's sticky"
    );
    assert!(
        !f.contains("Ghosttail \u{00b7}"),
        "an out-of-content region draws no count chip (nothing was folded)"
    );
    // And its tab reports expanded, not collapsed — the flag agrees with the (empty) fold.
    assert!(f.contains("data-region=\"K9\" data-label=\"Ghosttail\" data-collapsed=\"false\""));
}

// A removed-ghost region (diff overlay) must NOT fold, even if its id is in the collapse set: it
// has no live tab to expand, so folding it would hide *current* elements in its old columns with
// no way back. Under `?collapse=K2&base=`, diff_models feeds a removed ghost K2 whose old span
// still overlaps live stickies; the fold must skip it (mirrors the frontier `live` filter).
#[test]
fn a_collapsed_removed_ghost_region_does_not_fold_live_elements() {
    let mut m = three_region_board();
    // Simulate the diff-overlay shape: K2 is a removed ghost, but live elements still sit in its
    // old columns 2..=3 (layout follows the new side).
    m.phases[1] = phase("K2", "Beta", 2, 3, Some("removed"));
    let f = folded(&m, &["K2"]);
    // The live stickies in the ghost's old span stay on the board — not swallowed by a stale fold.
    assert!(
        f.contains("id=\"E2\"") && f.contains("id=\"E3\""),
        "removed-ghost fold hid live E2/E3"
    );
    // No count chip is drawn for a ghost (it has no tab), so no "Beta · N".
    assert!(
        !f.contains("Beta \u{00b7}"),
        "a removed ghost must not render a fold chip"
    );
    // The board is NOT shortened by folding a ghost.
    assert_eq!(
        svg_root_width(&f),
        svg_root_width(&rsvg(&m)),
        "ghost fold changed board width"
    );
}

// A region renders as a thin labelled outline (scope D1, calm instrument): a label tab carrying
// its name, grabbable partition frontiers keyed by region id + edge (F-region-frontiers), and a
// pivotal node where an event sits on a boundary col (derived, scope D3).
#[test]
fn region_renders_as_a_labelled_outline_with_frontier_handles_and_pivotal_node() {
    let m = Model {
        title: "t".into(),
        phases: vec![phase("K1", "Context A", 0, 2, None)],
        elements: vec![el("E1", "event", 0), el("E2", "event", 1)],
        edges: vec![],
        level: Level::default(),
        diff_meta: None,
    };
    let svg = rsvg(&m);
    assert!(svg.contains(">Context A<"), "region label tab is missing");
    // A lone phase draws its two board-end frontiers: the leftmost is its "start", the rightmost
    // its "end". `data-col` is the clamped boundary each sits before (start at col 0; the right
    // board edge sits after the last visible column 1, so col 2).
    assert!(
        svg.contains("class=\"frontier\" data-region=\"K1\" data-edge=\"start\" data-col=\"0\"")
    );
    assert!(svg.contains("class=\"frontier\" data-region=\"K1\" data-edge=\"end\" data-col=\"2\""));
    // The enclosing group carries the region's *clamped* bounds — K1's authored to_col (2) is
    // past the last visible column (elements only reach col 1), so the group reports the
    // clamped bound (1), matching the visual box exactly. Review: emitting the raw, unclamped
    // `ph.to_col` here desynced the client's drag math from the rail (which only covers
    // min_col..max_col) — a resize could target a column with no rail cell at all.
    assert!(
        svg.contains("class=\"region\" data-region=\"K1\" data-from-col=\"0\" data-to-col=\"1\"")
    );
    // The label tab is one focusable rename target (mirrors the sticky's role=button pattern);
    // `data-collapsed` (false when expanded) is the fold-state flag the client's `z` toggle reads.
    assert!(svg.contains(
        "class=\"region-tab\" data-region=\"K1\" data-label=\"Context A\" \
             data-collapsed=\"false\" role=\"button\" tabindex=\"0\""
    ));
    // E1 sits on the region's from-edge → a pivotal node; E2 (interior) does not add a third.
    assert_eq!(
        svg.matches("<circle").count(),
        1,
        "expected one pivotal node"
    );
}

// Two adjacent regions in the partition share their boundary: it is drawn as ONE grabbable
// frontier (the left region's "end"), not two overlapping edges. Three phases → four frontiers
// (two board ends + two internal), each addressable exactly once.
#[test]
fn adjacent_regions_share_a_single_frontier() {
    let m = Model {
        title: "t".into(),
        phases: vec![
            phase("K1", "A", 0, 1, None),
            phase("K2", "B", 2, 3, None),
            phase("K3", "C", 4, 5, None),
        ],
        elements: vec![el("E1", "event", 0), el("E2", "event", 5)],
        edges: vec![],
        level: Level::default(),
        diff_meta: None,
    };
    let svg = rsvg(&m);
    assert_eq!(
        svg.matches("class=\"frontier\"").count(),
        4,
        "3 phases → 4 frontiers, no doubled boundary"
    );
    // The K1|K2 boundary is the left region's "end" at col 2; the K2|K3 boundary K2's "end" at 4.
    assert!(svg.contains("data-region=\"K1\" data-edge=\"end\" data-col=\"2\""));
    assert!(svg.contains("data-region=\"K2\" data-edge=\"end\" data-col=\"4\""));
    // No frontier is keyed to a *right* region's "start" for an internal boundary (that would be
    // the doubled edge). Only the leftmost board edge is a "start".
    assert_eq!(
        svg.matches("data-edge=\"start\"").count(),
        1,
        "one board-left start only"
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
        level: Level::default(),
        diff_meta: None,
    };
    let svg = rsvg(&m);
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
        level: Level::default(),
        diff_meta: Some(("v1".into(), "v2".into())),
    };
    let svg = rsvg(&m);
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

#[test]
fn render_drops_an_off_grammar_type_instead_of_panicking() {
    // `type` picks the lane; an element whose type isn't one of the 8 lanes has no lane. It is
    // dropped from the view (before any geometry is computed), so the board is identical to the
    // valid-only one — and, crucially, rendering it does not panic on the lane lookups.
    let valid = Model {
        elements: vec![el("E1", "event", 0)],
        ..Default::default()
    };
    let mixed = Model {
        elements: vec![el("E1", "event", 0), el("X1", "not-a-lane", 1)],
        ..Default::default()
    };
    assert_eq!(rsvg(&mixed), rsvg(&valid));
}

#[test]
fn a_label_equal_to_a_template_token_is_not_clobbered() {
    // A sticky labelled `__CONFIG__` reaches the SVG verbatim (esc leaves underscores). The
    // single-pass fill must insert it as-is, not let the later `__CONFIG__` substitution
    // rewrite it into the geometry JSON.
    let html = render_html("<text>__CONFIG__</text>", "t");
    assert!(html.contains("<text>__CONFIG__</text>")); // label survived
    assert!(html.contains("\"colW\":")); // real config JSON still landed
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
        level: Level::default(),
        diff_meta: None,
    };
    let svg = rsvg(&m);
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
    let svg = rsvg(&events_at_col(2, 5));
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
        level: Level::default(),
        diff_meta: None,
    };
    let svg = rsvg(&m);
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
        level: Level::default(),
        diff_meta: None,
    };
    let svg = rsvg(&m);
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
