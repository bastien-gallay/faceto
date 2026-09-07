//! Edge geometry and within-cell ordering — pure layout math, no SVG strings.
use super::style::*;
use crate::model::Lane;
use crate::model::{Edge, Element};
use std::collections::HashMap;

/// One edge-endpoint queued on a box's face for fan-out: `(edge index, is-src, far-end cross pos)`.
type FaceMember = (usize, bool, f64);

/// A smooth connector between two box centres, anchored on the facing edges. `off1`/`off2` slide
/// each anchor along its facing edge (Lever B fan-out, F-edge-routing): the offset rides the *free*
/// axis of the chosen facing — Y for a left/right face, X for a top/bottom face — so several
/// connectors meeting one box on the same side spread out instead of collapsing onto one point.
/// Both offsets `0.0` reproduces the classic centre-to-centre path byte-for-byte.
pub(crate) fn edge_path(p1: (f64, f64), p2: (f64, f64), off1: f64, off2: f64) -> String {
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
pub(crate) fn cell_sub_order(
    elements: &[Element],
    edges: &[Edge],
    idx_of: &HashMap<&str, usize>,
) -> (Vec<i64>, HashMap<(Lane, i64), i64>) {
    let band = |j: usize| lane_index(elements[j].kind) as f64;
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

    let mut cell_members: HashMap<(Lane, i64), Vec<usize>> = HashMap::new();
    for (i, e) in elements.iter().enumerate() {
        cell_members
            .entry((e.kind, e.col.unwrap()))
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
    let cell_total: HashMap<(Lane, i64), i64> = cell_members
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
pub(crate) fn fan_offsets(
    ends: &[Option<(usize, usize)>],
    centers: &[(f64, f64)],
) -> (Vec<f64>, Vec<f64>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Lever B (F-edge-routing): the fan-out offset must be a pure addition — offset 0 reproduces
    // the classic centre-to-centre path byte-for-byte, and a non-zero offset slides only the anchor
    // along its facing edge (Y for a horizontal facing). p1→p2 is horizontal (dx = 400 ≥ STICKY_W).
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
}
