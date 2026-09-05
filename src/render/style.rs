//! Colour grammar, lane order, and layout constants — the visual vocabulary.

use super::diff::Tone;
use crate::model::Lane;

// The lane order the board draws top to bottom is the grammar's own order, so it lives with the
// `Lane` type rather than beside the colours. The swatches below deepen `command` and `hotspot`
// from their classic event-storming values so white label text clears WCAG 4.5:1.
pub(crate) use crate::model::LANES;

/// The id-mint prefix a lane stamps. `actor`/`aggregate` both start with 'a', so actor takes 'X'
/// and external takes 'G'. The single source of truth for prefixes — `serve::ids` reads it rather
/// than re-listing the grammar — and total, so minting can no longer fall back to a first letter
/// that collides with a real lane's id space.
pub fn lane_prefix(lane: Lane) -> char {
    match lane {
        Lane::Actor => 'X',
        Lane::Command => 'C',
        Lane::Aggregate => 'A',
        Lane::Event => 'E',
        Lane::Policy => 'P',
        Lane::ReadModel => 'R',
        // ADR-1 renamed the lane, not the prefix: `G1…` ids stay valid, and an id is identity.
        Lane::System => 'G',
        Lane::Hotspot => 'H',
    }
}

/// A lane's vertical rank in the fixed 8-lane grammar (`actor` = 0 … `hotspot` = 7). Used as the
/// y-band when ordering a crowded cell's members by their edge neighbours (F-edge-routing Lever A).
pub(crate) fn lane_index(lane: Lane) -> usize {
    LANES
        .iter()
        .position(|&l| l == lane)
        .expect("LANES is total")
}

pub(crate) fn colour(lane: Lane) -> &'static str {
    match lane {
        Lane::Actor => "#FCEFA1",
        Lane::Command => "#1A6FAE",
        Lane::Aggregate => "#FFD23F",
        Lane::Event => "#FF9F1C",
        Lane::Policy => "#C39BD3",
        Lane::ReadModel => "#6FCF97",
        Lane::System => "#F2A0C9",
        Lane::Hotspot => "#C0392B",
    }
}

pub(crate) fn text_dark(lane: Lane) -> bool {
    matches!(
        lane,
        Lane::Actor | Lane::Aggregate | Lane::Event | Lane::Policy | Lane::ReadModel | Lane::System
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
    fn lane_prefix_is_total_and_collision_free() {
        assert_eq!(lane_prefix(Lane::Actor), 'X'); // not 'A' — aggregate owns that
        assert_eq!(lane_prefix(Lane::Aggregate), 'A');
        assert_eq!(lane_prefix(Lane::Hotspot), 'H');
        // ADR-1 renamed `external` to `system` but not its prefix: an id is identity, so every
        // `G1…` already in a log stays the id of the sticky it names.
        assert_eq!(lane_prefix(Lane::System), 'G');
        // Two lanes sharing a prefix would mint colliding ids into each other's space.
        let mut seen: Vec<char> = LANES.iter().map(|&l| lane_prefix(l)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), LANES.len());
    }
}
