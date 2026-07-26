//! SVG generation: the `View` lens, `render_svg`, and its per-stage draw helpers.
use super::geometry::*;
use super::style::*;
use super::text::*;
use crate::model::{is_pivotal, Element, Model};
use crate::scene::{render_scene, Scene, Shape};
use std::collections::{HashMap, HashSet};

pub(crate) fn diff_tooltip(e: &Element, meta: &(String, String)) -> String {
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

/// A per-viewer *reading lens* applied at render time — never persisted, never in the log. Today it
/// carries the set of collapsed region ids (F-region-collapse). It is a pure argument to
/// `render_svg`, exactly like the diff overlay: `(Model, View) -> SVG`, re-derived per request. The
/// static `render`/`genesis` output and the plain `GET /` page pass `View::none()`; only
/// `GET /board.svg` reads `?collapse=` into one.
#[derive(Default)]
pub struct View {
    /// Region ids the viewer has folded to a thin band. Unknown ids are ignored (a stale id from a
    /// since-removed region just no-ops), and order is irrelevant — the remap is order-independent.
    pub collapsed: Vec<String>,
}

impl View {
    /// The identity lens — nothing folded. The default render everywhere except `?collapse=`.
    pub fn none() -> Self {
        View::default()
    }
}

/// Render a board to SVG — the event-storming scene, through the kernel's one serializer.
pub fn render_svg(model: &Model, view: &View) -> String {
    render_scene(&board_scene(model, view))
}

/// The event-storming scene builder: `(Model, View) -> Scene`. Every ES-specific word — lane, col,
/// sticky, region, frontier, pivotal — lives on this side of the seam; what crosses it is pure
/// geometry the kernel serializes without knowing what a board is.
pub(crate) fn board_scene(model: &Model, view: &View) -> Scene {
    let mut elements = model.elements.clone();
    // `type` selects the lane; an element whose type isn't one of the 8 lanes has no lane to
    // occupy. Drop it from this projection (its edges are then skipped by the `idx_of` guard
    // below) rather than panicking on the per-lane `lane_rows`/`lane_top` lookups — `colour` and
    // `lane_index` already tolerate unknown kinds, so this keeps render consistently defensive on
    // off-grammar input (the log stays the truth; the view just can't place a lane-less sticky).
    elements.retain(|e| LANES.contains(&e.kind.as_str()));

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
    let ncols_i = ncols as i64;
    // Clamp a stored `col` into the present column-index range `0..ncols` (an out-of-view region
    // bound folds to the edge instead of indexing past the geometry). Defined here (not down by the
    // region loop as it once was) because the collapse remap below needs it to resolve band spans.
    let clamp_idx = |c: i64| (c - min_col).clamp(0, ncols_i - 1) as usize;

    // F-region-collapse — the pure `col -> x` fold. A per-viewer lens (`view.collapsed`) folds each
    // named region's clamped inclusive column span `[lo,hi]` to ONE thin summary slot (its leftmost
    // column, width `COLLAPSE_W`); the span's other columns vanish and everything to their right
    // shifts left, so the board actually shortens. This is the whole layout delta — no `Model`/log
    // change, `replay`/`from_json` untouched. `xs[i]` is column-index `i`'s left x after folding and
    // `xs[ncols]` the board's right edge; `hidden[i]` marks a column swallowed into a band (its
    // stickies and their edges are dropped). Adjacent collapsed regions fold independently (each
    // keeps its own leftmost summary slot), matching the F-region-frontiers contiguous partition. An
    // empty collapsed set (or only unknown ids) is the identity: `COL_W` per column, the classic
    // column layout — so the static `render`/`genesis` path keeps its pre-feature geometry (the tab
    // still gains an inert `▾` disclosure glyph, so the SVG is not byte-identical to pre-feature output).
    let mut is_band_rep = vec![false; ncols]; // leftmost (summary) column of a collapsed band
    let mut hidden = vec![false; ncols]; // any column inside a collapsed band
                                         // The ids actually folded this render — the single source of truth the region-tab loop reads for
                                         // its `▸`/`· N` chip, so the tab can never disagree with the columns the remap hid (they were two
                                         // independent membership tests before). A region folds only if all three hold:
    let mut folded_ids: HashSet<&str> = HashSet::new();
    for ph in &model.phases {
        // (a) it is *live* — under a diff overlay (`?collapse=X&base=`), `diff_phases` feeds a removed
        //     region back as a `removed` ghost carrying its old span; folding it would hide *current*
        //     elements in those columns with no live tab to expand them (mirrors the `live` filter below);
        let live = ph.diff.as_deref() != Some("removed");
        // (b) its stored span actually overlaps the content columns `[min_col, max_col]` — a region
        //     that F-region-frontiers let run entirely past the last element column has no on-board
        //     columns to fold, and clamping it would pin `lo=hi` onto an edge column owned by a
        //     *neighbour*, hiding that neighbour's sticky and miscounting the chip;
        let overlaps_content = ph.to_col >= min_col && ph.from_col <= max_col;
        // (c) the viewer named it.
        if live && overlaps_content && view.collapsed.iter().any(|c| c == &ph.id) {
            folded_ids.insert(ph.id.as_str());
            let a = clamp_idx(ph.from_col);
            let b = clamp_idx(ph.to_col);
            let (lo, hi) = (a.min(b), a.max(b));
            is_band_rep[lo] = true;
            for h in hidden.iter_mut().take(hi + 1).skip(lo) {
                *h = true;
            }
        }
    }
    let col_w_at = |i: usize| -> f64 {
        if is_band_rep[i] {
            COLLAPSE_W
        } else if hidden[i] {
            0.0
        } else {
            COL_W
        }
    };
    let xs: Vec<f64> = {
        let mut xs = vec![0.0_f64; ncols + 1];
        let mut x = MARGIN_L;
        for (i, slot) in xs.iter_mut().enumerate().take(ncols) {
            *slot = x;
            x += col_w_at(i);
        }
        xs[ncols] = x;
        xs
    };
    // `c` is a column *index* (`col - min_col`) in `0..=ncols`; `col_left(ncols)` is the right edge.
    let col_left = |c: usize| xs[c];
    // An element is hidden when its column folded into a band — skipped in the sticky and edge draws.
    let hidden_el: Vec<bool> = elements
        .iter()
        .map(|e| hidden[(e.col.unwrap() - min_col) as usize])
        .collect();

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

    let board_right = col_left(ncols);

    let mut lane_top: HashMap<String, f64> = HashMap::new();
    let mut lane_h: HashMap<String, f64> = HashMap::new();
    let mut y = MARGIN_T;
    for t in present.iter() {
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

    // Board size rounds down to whole pixels — the SVG viewBox has always been integral, and the
    // legend and lane rules are laid out against it.
    let width = (board_right + 40.0).trunc();
    let height = (lanes_bottom + 60.0).trunc();

    let diff_meta = model.diff_meta.clone();
    let mut p: Vec<Shape> = Vec::new();
    draw_header(&mut p, model, width, height, &diff_meta, &elements);
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
    for (idx, &is_hidden) in hidden.iter().enumerate() {
        // A folded column carries no add-target: it is swallowed into a band whose region rect
        // paints over it. Skipping keeps the rail from spilling a COL_W hit-rect across the thin slot.
        if is_hidden {
            continue;
        }
        p.push(
            Shape::rect(col_left(idx), band_top, COL_W, band_bot - band_top)
                .with("class", "region-rail")
                .with("data-col", min_col + idx as i64)
                .with("fill", "transparent"),
        );
    }

    for ph in &model.phases {
        let a = clamp_idx(ph.from_col);
        let b = clamp_idx(ph.to_col);
        let (lo, hi) = (a.min(b), a.max(b));
        let x = col_left(lo);
        // Right edge = the left of the column *past* the span, which the fold remap already accounts
        // for: `xs[hi+1]` is `COLLAPSE_W` past `x` when this region is folded, `COL_W` per column
        // otherwise. (Was `col_left(hi) + COL_W`, correct only under the unfolded identity remap.)
        let right = xs[hi + 1];
        let w = right - x;
        // Is *this* region folded? Read the pre-pass's `folded_ids` — the *same* decision that drove
        // the `hidden`/`is_band_rep` geometry above — so the tab's chip can never disagree with the
        // columns the remap actually hid (removed ghosts and out-of-content spans are already excluded).
        let collapsed = folded_ids.contains(ph.id.as_str());
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
        let dash = dk.map(|_| "4 3");

        // One group per region carries its identity + *clamped* bounds (the client reads
        // `data-from-col`/`data-to-col` to snap a drag to a rail column without inverse pixel math;
        // clamped so that edge always falls on a column the region-rail actually covers). It also
        // carries the *unclamped* `data-real-to`: the keyboard resize (which nudges the true
        // `to_col`, not the visible edge) must not read the clamped value or a "grow" would truncate
        // a region whose stored extent runs past the last element column.
        // Tonal wash (the lone sanctioned region fill, #000 @ 0.02) + the open "⊓" outline.
        let mut band: Vec<Shape> = vec![
            Shape::rect(x, band_top, w, band_bot - band_top)
                .with("fill", "#000")
                .with("opacity", 0.02),
            Shape::line(x, band_top, right, band_top)
                .with("stroke", top_stroke)
                .with("stroke-width", 1.0)
                .maybe("stroke-dasharray", dash),
        ];
        // The band's vertical sides are the partition's frontiers, drawn once (deduped) by the
        // frontier pass after this loop — a boundary is shared by two neighbours, so drawing it
        // per-region is exactly the doubled/overlapping-edge bug F-region-frontiers kills. A
        // *removed* ghost draws NO sides: under the partition a removal is a merge, so the absorbing
        // neighbour sweeps a live band (and its frontier) over the ghost's old columns — hard ghost
        // side-lines there would collide with that live frontier and read as the neighbour's. The
        // faint wash + top rule + label tab still mark where the region was.

        // Label tab — the region's identity handle, a quiet folder tab straddling the top-left
        // corner (instrument grey, no domain colour: the Bench-Is-Grey Rule). Carries the diff badge
        // when the region changed. Grouped as one focusable button (mirrors the sticky pattern) so
        // the Stage-6 client can bind a click/dblclick/Enter → in-place rename to one hit target.
        let badge = dk.and_then(diff_badge);
        // When folded, the tab becomes the summary chip: the hidden stickies collapse to a `· N`
        // count (in-band elements), so the tab still tells the reader how much is tucked away.
        let n_in_band = if collapsed {
            elements
                .iter()
                .filter(|e| {
                    let i = (e.col.unwrap() - min_col) as usize;
                    i >= lo && i <= hi
                })
                .count()
        } else {
            0
        };
        let mut label = match badge {
            Some(b) => format!("{b} {}", ph.label),
            None => ph.label.clone(),
        };
        if collapsed {
            label = format!("{label} \u{00b7} {n_in_band}");
        }
        // Disclosure triangle: ▸ folded, ▾ expanded. A separate hit target (class `region-collapse`)
        // so a click toggles the fold without tripping the tab's rename — the label to its right
        // keeps the rename gesture. Drawn only on a live region (a removed ghost has no tab group).
        let disclosure = if collapsed { "\u{25b8}" } else { "\u{25be}" };
        let tri_w = 13.0;
        let tab_h = REGION_TAB_H;
        let tab_w = tri_w + label.chars().count() as f64 * REGION_TAB_CHAR_W + REGION_TAB_PAD;
        let tab_y = band_top - tab_h + 1.0;
        let tab_rect = Shape::rect(x, tab_y, tab_w, tab_h)
            .with("rx", 6.0)
            .with("fill", "#ffffff")
            .with("stroke", stroke)
            .with("stroke-width", 1.0)
            .maybe("stroke-dasharray", dash);
        let tab_label = Shape::text(x + 7.0 + tri_w, tab_y + 13.5, label)
            .with("font-size", 11.0)
            .with("font-weight", 600i64)
            .with("fill", diff_col.unwrap_or(AXIS_LABEL));
        if removed {
            // A removed ghost has no tab *group*: nothing about it is operable, so it gets no hit
            // target, no focus stop, and no disclosure triangle — just the outline of where it was.
            band.push(tab_rect);
            band.push(tab_label);
        } else {
            // The triangle sits in the tab's left gutter; `title` names the gesture for hover/a11y.
            let toggle = Shape::text(x + 7.0, tab_y + 13.0, disclosure)
                .with("class", "region-collapse")
                .with("data-region", ph.id.as_str())
                .with("font-size", 10.0)
                .with("fill", AXIS_LABEL)
                .with("style", "cursor:pointer")
                .titled(format!(
                    "{} region (z)",
                    if collapsed { "expand" } else { "collapse" }
                ));
            // `data-label` carries the *raw* stored label (no diff badge, no count) — the rename
            // editor prefills from this, not the badge/count-prefixed display `label` above. The aria
            // state names the fold *and* the hidden count, so a screen-reader user hears what the
            // sighted `· N` chip shows (not just "collapsed").
            let aria_state = if collapsed {
                format!(", collapsed, {n_in_band} elements")
            } else {
                String::new()
            };
            band.push(
                Shape::group(vec![tab_rect, toggle, tab_label])
                    .with("class", "region-tab")
                    .with("data-region", ph.id.as_str())
                    .with("data-label", ph.label.as_str())
                    .with("data-collapsed", collapsed)
                    .with("role", "button")
                    .with("tabindex", 0i64)
                    .with(
                        "aria-label",
                        format!("region {}, {}{}", ph.id, ph.label, aria_state),
                    )
                    .with("style", "cursor:pointer"),
            );
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

        // A ghosted region fades as one: the opacity rides an inner group rather than every child,
        // which is the nesting a flat string list had to fake with a bare `<g opacity>` open/close.
        let inner = if removed {
            vec![Shape::group(band).with("opacity", 0.45)]
        } else {
            band
        };
        p.push(
            Shape::group(inner)
                .with("class", if removed { "region removed" } else { "region" })
                .with("data-region", ph.id.as_str())
                .with("data-from-col", clamped_from)
                .with("data-to-col", clamped_to)
                .with("data-real-to", ph.to_col),
        );
    }

    // Frontier lines — the grabbable boundaries of the contiguous partition (F-region-frontiers).
    // Live phases are sorted and contiguous, so each internal boundary is shared by two neighbours
    // and drawn *once* (killing the doubled/overlapping edges the independent-span model produced).
    // Every frontier maps to the one (region, edge) the client posts as a `FrontierMoved`: the
    // leftmost board edge is the first phase's `"start"`; every other frontier — internal or the
    // rightmost board edge — is a phase's `"end"`, so a drag re-borders exactly one phase and
    // `replay`'s `normalize` follows the neighbour. `data-col` is the boundary column the frontier
    // sits *before* (its left side); the rightmost board edge sits after the last column (+1). Bounds
    // are clamped into the present range so the client's drag math matches what's drawn (WYSIWYG).
    let live: Vec<&crate::model::Phase> = model
        .phases
        .iter()
        .filter(|p| p.diff.as_deref() != Some("removed"))
        .collect();
    if let (Some(first), Some(last)) = (live.first(), live.last()) {
        let bx = |c: i64| col_left(clamp_idx(c)); // left x of a (clamped) column
                                                  // (region_id, edge, boundary_col, x)
        let mut frontiers: Vec<(&str, &str, i64, f64)> = vec![(
            first.id.as_str(),
            "start",
            min_col + clamp_idx(first.from_col) as i64,
            bx(first.from_col),
        )];
        for pair in live.windows(2) {
            let r = pair[1];
            frontiers.push((
                pair[0].id.as_str(),
                "end",
                min_col + clamp_idx(r.from_col) as i64,
                bx(r.from_col),
            ));
        }
        frontiers.push((
            last.id.as_str(),
            "end",
            min_col + clamp_idx(last.to_col) as i64 + 1,
            xs[clamp_idx(last.to_col) + 1],
        ));
        for (id, edge, bcol, fx) in frontiers {
            // Visible boundary (neutral instrument grey — a frontier is structural, shared by two
            // regions; diff emphasis stays on each region's top rule + tab, not the shared line).
            p.push(
                Shape::line(fx, band_top, fx, band_bot)
                    .with("stroke", "#cfcfda")
                    .with("stroke-width", 1.5),
            );
            // Wide transparent hit-line — the grab target (D5: the border, not a sticky), reusing
            // the proven pointer-capture drag. `data-col` lets the client detect "no change" and
            // resolve the post without inverse pixel math.
            p.push(
                Shape::line(fx, band_top, fx, band_bot)
                    .with("class", "frontier")
                    .with("data-region", id)
                    .with("data-edge", edge)
                    .with("data-col", bcol)
                    .with("stroke", "transparent")
                    .with("stroke-width", 8.0),
            );
        }
    }

    // One pivotal node per unique boundary position (sorted for deterministic output; a node shared
    // by two adjacent regions is drawn once). It rides the border line at the event-lane centre.
    if let Some(cy) = event_cy {
        pivot_node_x.sort_unstable();
        pivot_node_x.dedup();
        for kx in &pivot_node_x {
            p.push(
                Shape::circle(*kx as f64 / 10.0, cy, 4.0)
                    .with("fill", AXIS_LABEL)
                    .with("stroke", "#fbfbfd")
                    .with("stroke-width", 1.5),
            );
        }
    }

    draw_lanes(&mut p, &present, &lane_top, &lane_h, width);
    draw_edges(&mut p, model, &elements, &idx_of, &centers, &hidden_el);
    draw_stickies(&mut p, &elements, &centers, &hidden_el, &diff_meta);
    draw_legend(&mut p, &present, height);

    Scene {
        width,
        height,
        shapes: p,
    }
}

/// Board frame: the background wash, the serif nameplate, and the diff subtitle. The `<svg>` root
/// and the shared arrow marker now belong to `render_scene` — every format needs both.
fn draw_header(
    p: &mut Vec<Shape>,
    model: &Model,
    width: f64,
    height: f64,
    diff_meta: &Option<(String, String)>,
    elements: &[Element],
) {
    p.push(Shape::rect(0.0, 0.0, width, height).with("fill", "#fbfbfd"));
    // The board title is the instrument's engraved nameplate: a refined system serif, the one
    // place a second font family appears (see DESIGN.md §3). Everything else stays the SVG sans.
    p.push(
        Shape::text(20.0, 34.0, model.title.as_str())
            .with("font-size", 20.0)
            .with("font-weight", 700i64)
            .with("fill", "#222")
            .with(
                "font-family",
                "'Iowan Old Style','Palatino Linotype',Palatino,'Book Antiqua',Georgia,serif",
            ),
    );

    // Diff subtitle: rev labels + per-status counts with dashed swatches.
    if let Some((a, b)) = diff_meta {
        p.push(
            Shape::text(20.0, 56.0, format!("{a} \u{2192} {b}"))
                .with("font-size", 12.0)
                .with("fill", "#777"),
        );
        let mut lx2 = 40.0 + 7.0 * ((a.chars().count() + b.chars().count() + 3) as f64);
        for k in ["added", "removed", "changed", "moved"] {
            let n = elements
                .iter()
                .filter(|e| e.diff.as_deref() == Some(k))
                .count();
            if n == 0 {
                continue;
            }
            p.push(
                Shape::rect(lx2, 46.0, 12.0, 12.0)
                    .with("rx", 3.0)
                    .with("fill", "none")
                    .with("stroke", diff_colour(k))
                    .with("stroke-width", 2.5)
                    .with("stroke-dasharray", "3 2"),
            );
            p.push(
                Shape::text(lx2 + 17.0, 56.0, format!("{n} {k}"))
                    .with("font-size", 12.0)
                    .with("fill", "#555"),
            );
            lx2 += 17.0 + 8.0 * ((k.len() + n.to_string().len() + 1) as f64) + 16.0;
        }
    }
}

/// Faint lane rules and the centred lane labels (which expose the band interior geometry).
fn draw_lanes(
    p: &mut Vec<Shape>,
    present: &[&str],
    lane_top: &HashMap<String, f64>,
    lane_h: &HashMap<String, f64>,
    width: f64,
) {
    // Faint horizontal lane rules — graph-paper bench lines that delimit lanes now that a busy
    // lane can span several rows.
    for t in present.iter().skip(1) {
        p.push(
            Shape::line(12.0, lane_top[*t], width - 20.0, lane_top[*t])
                .with("stroke", "#e0e0e6")
                .with("opacity", 0.55),
        );
    }

    // Lane labels — centred on each lane's (possibly multi-row) band. `data-band-top`/
    // `data-band-h` expose the band *interior* (the `y` fraction's frame of reference) so the
    // client's vertical drag converts a pixel drop into a stored fraction without re-deriving
    // the lane geometry — render.rs stays the single source of truth for it (Composable).
    for t in present.iter() {
        let y = lane_top[*t] + lane_h[*t] / 2.0;
        // `class`/`data-lane` let the client hang the lane-title `+` (inline-add prepend) on each
        // label; the rendered text content is unchanged.
        p.push(
            Shape::text(16.0, y + 4.0, *t)
                .with("class", "lane-label")
                .with("data-lane", *t)
                .with("data-band-top", lane_top[*t] + LANE_VPAD / 2.0)
                .with("data-band-h", lane_h[*t] - LANE_VPAD)
                .with("font-size", 12.0)
                .with("font-weight", 600i64)
                .with("fill", AXIS_LABEL),
        );
    }
}

/// Flow / hotspot edges under the stickies (fanned at shared faces; folded endpoints skipped).
fn draw_edges(
    p: &mut Vec<Shape>,
    model: &Model,
    elements: &[Element],
    idx_of: &HashMap<&str, usize>,
    centers: &[(f64, f64)],
    hidden_el: &[bool],
) {
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
    let (off_src, off_dst) = fan_offsets(&ends, centers);

    // Edges (under the stickies). A hotspot connector is a concern, not a flow: dotted, arrow-less.
    for (ei, edge) in model.edges.iter().enumerate() {
        let (si, di) = match ends[ei] {
            Some(p) => p,
            None => continue,
        };
        // An edge with either end folded into a collapsed band is dropped with its hidden node.
        // (Rerouting a crossing edge to the band frontier is F-region-edge-fold, held out of v1.)
        if hidden_el[si] || hidden_el[di] {
            continue;
        }
        let d = edge_path(centers[si], centers[di], off_src[ei], off_dst[ei]);
        let is_hot = elements[si].kind == "hotspot" || elements[di].kind == "hotspot";
        let cls = if is_hot { "edge hot" } else { "edge" };
        let wire = Shape::path(d)
            .with("class", cls)
            .with("data-src", edge.src.as_str())
            .with("data-dst", edge.dst.as_str())
            .with("fill", "none");
        // Four connector registers, one shape each: a diff-added wire, a diff-removed one, a
        // hotspot concern (dotted, arrow-less — a concern is not a flow), and the plain flow.
        p.push(match edge.status.as_deref() {
            Some("added") => wire
                .with("stroke", diff_colour("added"))
                .with("stroke-width", 1.8)
                .with("stroke-dasharray", "6 4")
                .with("marker-end", "url(#arrow)")
                .with("opacity", 0.9),
            Some("removed") => wire
                .with("stroke", diff_colour("removed"))
                .with("stroke-width", 1.6)
                .with("stroke-dasharray", "3 4")
                .with("marker-end", "url(#arrow)")
                .with("opacity", 0.5),
            _ if is_hot => wire
                .with("stroke", EDGE_HOTSPOT)
                .with("stroke-width", 1.4)
                .with("stroke-dasharray", "0.1 6")
                .with("stroke-linecap", "round")
                .with("opacity", 0.7),
            _ => wire
                .with("stroke", EDGE_FLOW)
                .with("stroke-width", 1.5)
                .with("marker-end", "url(#arrow)")
                .with("opacity", 0.6),
        });
        // An authored connection label (F-element-links), set small and muted at the edge midpoint
        // so a wire reads as *what* it is, not just *that* it is — a white halo keeps it legible
        // where it crosses a line. Suppressed on a fading "removed" diff edge (its endpoints are
        // going away). Calm-instrument register: never a loud tag.
        if let Some(lbl) = edge.label.as_deref().filter(|l| !l.is_empty()) {
            if edge.status.as_deref() != Some("removed") {
                let (sx, sy) = centers[si];
                let (dx, dy) = centers[di];
                p.push(
                    Shape::text((sx + dx) / 2.0, (sy + dy) / 2.0 - 3.0, lbl)
                        .with("class", "edge-label")
                        .with("font-size", 10.0)
                        .with("text-anchor", "middle")
                        .with("fill", AXIS_LABEL)
                        .with("stroke", "#fff")
                        .with("stroke-width", 3.0)
                        .with("paint-order", "stroke")
                        .with("opacity", 0.85),
                );
            }
        }
    }
}

/// The stickies themselves — each a focusable, id-keyed group the sidecar targets.
fn draw_stickies(
    p: &mut Vec<Shape>,
    elements: &[Element],
    centers: &[(f64, f64)],
    hidden_el: &[bool],
    diff_meta: &Option<(String, String)>,
) {
    // Stickies — each a clickable <g id="..."> the sidecar targets by id.
    for (i, e) in elements.iter().enumerate() {
        // Folded into a collapsed band — its count lives on the band chip, no sticky drawn.
        if hidden_el[i] {
            continue;
        }
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
        let data_y = e.y.map(|_| crate::model::y_key(e.y));
        // Reference URLs (F-element-links) ride the sticky as a newline-joined `data-links` (a URL
        // has no raw newline) — the client splits it and renders clickable chips in the modal. They
        // are surfaced on click, never painted on the calm board. Absent when the element has none.
        let data_links = (!e.links.is_empty()).then(|| e.links.join("\n"));

        let mut kids: Vec<Shape> = Vec::new();
        if matches!(status, Some("added") | Some("changed") | Some("moved")) {
            let s = status.unwrap();
            kids.push(
                Shape::rect(x - 4.0, y - 4.0, STICKY_W + 8.0, STICKY_H + 8.0)
                    .with("rx", (shape_i + 3) as f64)
                    .with("fill", "none")
                    .with("stroke", diff_colour(s))
                    .with("stroke-width", 3.0)
                    .with("stroke-dasharray", "6 4"),
            );
        }
        kids.push(
            Shape::rect(x, y, STICKY_W, STICKY_H)
                .with("class", "card")
                .with("rx", shape_i as f64)
                .with("fill", fill)
                .with("stroke", "#0003"),
        );
        kids.push(
            Shape::text(x + 8.0, y + 15.0, &e.id)
                .with("font-size", 9.0)
                .with("font-weight", 700i64)
                .with("fill", txt)
                .with("opacity", 0.6),
        );
        let hlines = wrap(&hero, 20, if detail.is_empty() { 3 } else { 2 });
        let block_h = hlines.len() as f64 * 14.0 + if !detail.is_empty() { 12.0 } else { 0.0 };
        let start = cy - block_h / 2.0 + 11.0;
        for (i, ln) in hlines.iter().enumerate() {
            kids.push(
                Shape::text(cx, start + i as f64 * 14.0, ln.as_str())
                    .with("font-size", 12.0)
                    .with("font-weight", 600i64)
                    .with("text-anchor", "middle")
                    .with("fill", txt),
            );
        }
        if !detail.is_empty() {
            let dtxt = wrap(&detail, 30, 1).into_iter().next().unwrap_or_default();
            kids.push(
                Shape::text(cx, start + hlines.len() as f64 * 14.0, dtxt)
                    .with("font-size", 9.5)
                    .with("text-anchor", "middle")
                    .with("fill", txt)
                    .with("opacity", 0.6),
            );
        }
        if resolved {
            kids.push(Shape::circle(x + STICKY_W - 3.0, y + 3.0, 8.0).with("fill", "#6FAE7E"));
            kids.push(
                Shape::path(format!(
                    "M{:.1},{:.1} l2.4,2.6 l4,-5.2",
                    x + STICKY_W - 7.0,
                    y + 3.0
                ))
                .with("fill", "none")
                .with("stroke", "#fff")
                .with("stroke-width", 1.6)
                .with("stroke-linecap", "round")
                .with("stroke-linejoin", "round"),
            );
        }
        if status == Some("removed") {
            kids.push(
                Shape::line(x + 6.0, cy, x + STICKY_W - 6.0, cy)
                    .with("stroke", diff_colour("removed"))
                    .with("stroke-width", 2.5),
            );
        }
        if let Some(badge) = status.and_then(diff_badge) {
            let s = status.unwrap();
            kids.push(
                Shape::circle(x + STICKY_W, y, 9.0)
                    .with("fill", diff_colour(s))
                    .with("stroke", "#fff")
                    .with("stroke-width", 1.5),
            );
            kids.push(
                Shape::text(x + STICKY_W, y + 4.0, badge)
                    .with("font-size", 12.0)
                    .with("font-weight", 700i64)
                    .with("text-anchor", "middle")
                    .with("fill", "#fff"),
            );
        }

        let tip = match (status, diff_meta) {
            (Some(_), Some(meta)) => Some(diff_tooltip(e, meta)).filter(|t| !t.is_empty()),
            _ => None,
        };
        let mut g = Shape::group(kids)
            .with("id", e.id.as_str())
            .with("class", cls)
            .with("role", "button")
            .with("tabindex", 0i64)
            .with("aria-label", aria)
            .with("data-hero", hero)
            .with("data-detail", detail)
            .with("data-kind", e.kind.as_str())
            .with("data-col", e.col.unwrap())
            .with("data-cx", cx)
            .with("data-cy", cy)
            .maybe("data-y", data_y)
            .maybe("data-links", data_links)
            .with("style", "cursor:pointer");
        if status == Some("removed") {
            g = g.with("opacity", 0.4);
        }
        if let Some(t) = tip {
            g = g.titled(t);
        }
        p.push(g);
    }
}

/// Legend: the lane colour swatches and the connector key.
fn draw_legend(p: &mut Vec<Shape>, present: &[&str], height: f64) {
    // Legend: type swatches, then a connector key (flow vs hotspot-concern).
    let ly = height - 28.0;
    let mut lx = 20.0;
    for t in present.iter() {
        p.push(
            Shape::rect(lx, ly, 14.0, 14.0)
                .with("rx", 3.0)
                .with("fill", colour(t))
                .with("stroke", "#0003"),
        );
        p.push(
            Shape::text(lx + 19.0, ly + 11.0, *t)
                .with("font-size", 11.0)
                .with("fill", "#555"),
        );
        lx += 26.0 + 7.0 * (t.len() as f64) + 14.0;
    }
    let midy = ly + 7.0;
    lx += 12.0;
    p.push(
        Shape::line(lx, midy, lx + 26.0, midy)
            .with("stroke", EDGE_FLOW)
            .with("stroke-width", 1.5)
            .with("marker-end", "url(#arrow)"),
    );
    p.push(
        Shape::text(lx + 33.0, ly + 11.0, "triggers / leads to")
            .with("font-size", 11.0)
            .with("fill", "#555"),
    );
    lx += 33.0 + 7.0 * ("triggers / leads to".len() as f64) + 18.0;
    p.push(
        Shape::line(lx, midy, lx + 26.0, midy)
            .with("stroke", EDGE_HOTSPOT)
            .with("stroke-width", 1.4)
            .with("stroke-dasharray", "0.1 6")
            .with("stroke-linecap", "round"),
    );
    p.push(
        Shape::text(lx + 33.0, ly + 11.0, "open question (hotspot)")
            .with("font-size", 11.0)
            .with("fill", "#555"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Level, Phase};

    /// Render with the identity lens (nothing collapsed) — the default for every test that isn't
    /// exercising F-region-collapse itself, so the `View` argument stays out of the assertions.
    fn rsvg(model: &Model) -> String {
        render_svg(model, &View::none())
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
                links: Vec::new(),
                diff: None,
                was: None,
            }],
            edges: vec![],
            level: Level::default(),
            diff_meta: None,
        }
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

    /// Every numeric value of `attr` across the scene, in draw order. Reads the `Scene` rather than
    /// scraping the serialized SVG, so a geometry assertion pins the *number* the layout computed
    /// and never the serializer's formatting of it.
    fn attr_nums(model: &Model, attr: &str) -> Vec<f64> {
        fn walk(shapes: &[Shape], attr: &str, out: &mut Vec<f64>) {
            for s in shapes {
                match s.attrs().get(attr) {
                    Some(crate::scene::Val::Num(n)) => out.push(*n),
                    Some(crate::scene::Val::Int(i)) => out.push(*i as f64),
                    _ => {}
                }
                if let Shape::Group { children, .. } = s {
                    walk(children, attr, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&board_scene(model, &View::none()).shapes, attr, &mut out);
        out
    }

    /// Every connector path whose `attr` equals `value`, as `(d, start-anchor Y)`. Reads the
    /// `Scene`, so an edge-routing assertion works off the path the layout produced rather than
    /// hunting for `M` in a serialized document.
    fn edge_anchors(model: &Model, attr: &str, value: &str) -> Vec<f64> {
        board_scene(model, &View::none())
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Path { d, attrs } => {
                    let hit =
                        matches!(attrs.get(attr), Some(crate::scene::Val::Str(v)) if v == value);
                    // `d` opens `M<x>,<y> ` — the start anchor the fan spreads.
                    hit.then(|| {
                        let head = d.split(' ').next().unwrap_or("");
                        head[head.find(',').unwrap() + 1..].parse().unwrap()
                    })
                }
                _ => None,
            })
            .collect()
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
                    links: Vec::new(),
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
                attr_nums(&m, "data-cy"),
                vec![494.0],
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
        let m = empty_board();
        // actor is the first lane: top = MARGIN_T + LANE_VPAD/2 = 124, interior = ROW_PITCH = 92.
        assert_eq!(attr_nums(&m, "data-band-top")[0], 124.0);
        assert_eq!(attr_nums(&m, "data-band-h")[0], 92.0);
        assert!(rsvg(&m).contains("class=\"lane-label\" data-lane=\"actor\""));
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
            links: Vec::new(),
            diff: None,
            was: None,
        });
        assert_eq!(distinct(&rsvg(&m), "data-cx"), 2);
        // col -3 is the leftmost authored column → slot 0 → classic single-cell centre 255.0.
        let cxs = attr_nums(&m, "data-cx");
        assert!(cxs.contains(&255.0), "got {cxs:?}");
    }

    // A lone sticky keeps its classic position: centred on a single-row lane, no horizontal fan.
    // Under R every lane always renders, so `event` is the 4th lane (actor/command/aggregate sit
    // above it), each an empty single-row band of height ROW_PITCH + LANE_VPAD = 108.
    #[test]
    fn a_lone_sticky_stays_on_the_lane_mid_line() {
        let m = events_at_col(0, 1);
        // lane_top(event) = MARGIN_T + 3*108 = 440; + LANE_VPAD/2 + ROW_PITCH/2 = 440 + 8 + 46.
        assert_eq!(attr_nums(&m, "data-cy"), vec![494.0]);
        // col 0 centre, no stagger: MARGIN_L + COL_W/2 = 150 + 105.
        assert_eq!(attr_nums(&m, "data-cx"), vec![255.0]);
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
                label: None,
                status: None,
            }, // wholly inside K2
            Edge {
                src: "E1".into(),
                dst: "E4".into(),
                label: None,
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
        assert!(svg
            .contains("class=\"frontier\" data-region=\"K1\" data-edge=\"start\" data-col=\"0\""));
        assert!(
            svg.contains("class=\"frontier\" data-region=\"K1\" data-edge=\"end\" data-col=\"2\"")
        );
        // The enclosing group carries the region's *clamped* bounds — K1's authored to_col (2) is
        // past the last visible column (elements only reach col 1), so the group reports the
        // clamped bound (1), matching the visual box exactly. Review: emitting the raw, unclamped
        // `ph.to_col` here desynced the client's drag math from the rail (which only covers
        // min_col..max_col) — a resize could target a column with no rail cell at all.
        assert!(svg
            .contains("class=\"region\" data-region=\"K1\" data-from-col=\"0\" data-to-col=\"1\""));
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

    fn el(id: &str, kind: &str, col: i64) -> Element {
        Element {
            id: id.into(),
            kind: kind.into(),
            label: "L".into(),
            col: Some(col),
            detail: None,
            y: None,
            resolved: false,
            links: Vec::new(),
            diff: None,
            was: None,
        }
    }

    // F-element-links: an edge `label` draws as a midpoint `edge-label` text; an element's `links`
    // ride the sticky as `data-links` (surfaced in the modal), never painted on the board.
    #[test]
    fn edge_label_and_element_links_reach_the_svg() {
        let mut src = el("C1", "command", 0);
        src.links = vec!["https://tix/9".into(), "adr://2".into()];
        let m = Model {
            elements: vec![src, el("E1", "event", 1)],
            edges: vec![Edge {
                src: "C1".into(),
                dst: "E1".into(),
                label: Some("emits".into()),
                status: None,
            }],
            ..Default::default()
        };
        let svg = rsvg(&m);
        assert!(
            svg.contains("class=\"edge-label\""),
            "edge label text is drawn"
        );
        assert!(svg.contains(">emits</text>"), "the label reads its text");
        // links ride the source sticky, newline-joined, and are not painted as board text.
        assert!(svg.contains("data-links=\"https://tix/9\nadr://2\""));
    }

    // An edge with no authored label draws no `edge-label` text (the common case stays clean).
    #[test]
    fn an_unlabelled_edge_draws_no_label_text() {
        let m = Model {
            elements: vec![el("C1", "command", 0), el("E1", "event", 1)],
            edges: vec![Edge {
                src: "C1".into(),
                dst: "E1".into(),
                label: None,
                status: None,
            }],
            ..Default::default()
        };
        assert!(!rsvg(&m).contains("edge-label"));
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
                    label: None,
                    status: None,
                },
                Edge {
                    src: "E_hi".into(),
                    dst: "R1".into(),
                    label: None,
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
                    label: None,
                    status: None,
                },
                Edge {
                    src: "X1".into(),
                    dst: "C2".into(),
                    label: None,
                    status: None,
                },
            ],
            level: Level::default(),
            diff_meta: None,
        };
        // Each edge path's start anchor Y. They must differ by the fan spread.
        let starts = edge_anchors(&m, "class", "edge");
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
                label: None,
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
        let cy = cy_of(&rsvg(&m), "X1");
        let anchors = edge_anchors(&m, "data-src", "X1");
        assert_eq!(anchors.len(), 9, "expected 9 fanned connectors");
        for y in anchors {
            assert!(
                (y - cy).abs() <= STICKY_H / 2.0 + 0.05,
                "anchor slid off the box: y={y}, cy={cy}"
            );
        }
    }
}
