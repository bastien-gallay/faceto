//! Colour grammar, lane order, and layout constants — the visual vocabulary.

use super::diff::Tone;

// Canonical lane order (top → bottom). `command` and `hotspot` are deepened from their classic
// event-storming swatches so white label text clears WCAG 4.5:1.
pub(crate) const LANES: [&str; 8] = [
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
pub(crate) const LANE_PREFIXES: [char; 8] = ['X', 'C', 'A', 'E', 'P', 'R', 'G', 'H'];

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
pub(crate) fn lane_index(kind: &str) -> usize {
    LANES.iter().position(|&l| l == kind).unwrap_or(0)
}

pub(crate) fn colour(kind: &str) -> &'static str {
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

pub(crate) fn text_dark(kind: &str) -> bool {
    matches!(
        kind,
        "actor" | "aggregate" | "event" | "policy" | "readmodel" | "external"
    )
}

pub(crate) const RESOLVED_FILL: &str = "#D9DEE3";
pub(crate) const EDGE_FLOW: &str = "#9AA7B0";
pub(crate) const EDGE_HOTSPOT: &str = "#C39086";
// Muted axis + phase-band labels. Darkened from the old #90a4ae (≈2.6:1, fails AA) to clear WCAG
// 4.5:1 on the #fbfbfd board (≈5.3:1). These labels *name* the lane grammar — they are structure,
// not decoration, so they must be readable.
pub(crate) const AXIS_LABEL: &str = "#5b6b75";

/// The diff palette: one colour per [`Tone`]. The overlay's vocabulary is a closed enum, so this
/// is total — there is no "unknown verdict" fallback to drift.
pub(crate) fn diff_colour(tone: Tone) -> &'static str {
    match tone {
        Tone::Added => "#27ae60",
        Tone::Removed => "#EB5757",
        Tone::Changed | Tone::Moved => "#E59500",
    }
}

/// The corner badge a changed thing wears — the glyph half of the same closed vocabulary, so a
/// renamed region reads like a relabelled sticky (`≠`) and a resized one like a relocated one (`→`).
pub(crate) fn diff_badge(tone: Tone) -> &'static str {
    match tone {
        Tone::Added => "+",
        Tone::Removed => "\u{2013}", // en dash
        Tone::Changed => "\u{2260}", // ≠
        Tone::Moved => "\u{2192}",   // →
    }
}

pub(crate) const COL_W: f64 = 210.0;
// F-region-collapse: a folded region's whole column span compresses to one thin summary slot of
// this width (its stickies hidden behind a count chip on the tab); columns to its right shift left
// so a wide board actually shortens. Narrow enough to read as "quieted", wide enough to seat the
// band's tonal wash and frontier lines.
pub(crate) const COLLAPSE_W: f64 = 60.0;
// When a (lane, col) cell holds several simultaneous stickies they auto-stack into sub-rows, each
// adding ROW_PITCH of height (a stored `y` places its element freely in the same band instead).
// LANE_VPAD keeps a single-row lane at the classic 108px (92 + 16), so uncrowded boards look
// exactly as before.
pub(crate) const ROW_PITCH: f64 = 92.0;
pub(crate) const LANE_VPAD: f64 = 16.0;
pub(crate) const MARGIN_L: f64 = 150.0;
pub(crate) const MARGIN_T: f64 = 116.0;
pub(crate) const STICKY_W: f64 = 176.0;
pub(crate) const STICKY_H: f64 = 74.0;
// A region's label tab: fixed height, width grows with the label (a per-char pitch + fixed
// padding). The client's region-add editor mirrors `REGION_TAB_H` via `__CONFIG__` rather than
// inventing its own box size (CUPID-Composable: render.rs is the single source of truth for a
// layout decision the client also needs — CODING_STANDARDS.md §Composable).
pub(crate) const REGION_TAB_H: f64 = 19.0;
pub(crate) const REGION_TAB_CHAR_W: f64 = 6.6;
pub(crate) const REGION_TAB_PAD: f64 = 18.0;
// How far apart sibling connectors fan when several meet a box on the same face (F-edge-routing
// Lever B). Deliberately small — the calm-instrument register wants a gentle spread, not a starburst.
// `fan_offsets` caps the per-slot step below this when a face is crowded, so the extreme anchor
// always stays on the box (a high-degree node packs tighter rather than spilling off the edge).
pub(crate) const FAN_SPREAD: f64 = 12.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_prefix_is_aligned_with_lanes_and_total() {
        assert_eq!(LANES.len(), LANE_PREFIXES.len());
        assert!(LANES.iter().all(|l| lane_prefix(l).is_some()));
        assert_eq!(lane_prefix("actor"), Some('X')); // not 'A' — aggregate owns that
        assert_eq!(lane_prefix("aggregate"), Some('A'));
        assert_eq!(lane_prefix("hotspot"), Some('H'));
        assert_eq!(lane_prefix("not-a-lane"), None);
    }
}
