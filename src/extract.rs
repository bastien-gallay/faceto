//! Semantic sub-board extraction (F-extract) — carve a smaller board out of a bigger one by
//! *meaning*, not by geometry: a region, a bounded neighbourhood around one element, a lane.
//!
//! This stage is pure and depends on nothing downstream: `(Model, Selector) -> Model`, no IO, no
//! clocks — the same posture as [`crate::lint`]. The CLI (`main.rs`) owns loading the source and
//! writing the result out as a genesis'd log.
//!
//! Two rules make the extract useful rather than merely smaller:
//!
//! - **Ids are preserved**, never renumbered — so the sub-board diffs cleanly against the board it
//!   came from (F-variants), which is the whole point of extracting before a "what if".
//! - **`col` is preserved too.** A sub-board is not re-based to column 0: `col` is a global
//!   timeline coordinate, and shifting it would read as a `moved` verdict on every element in the
//!   very diff the id-preservation exists to keep clean.
//!
//! An edge with one endpoint outside the selection is **dropped**. The hole is deliberate and
//! visible: `lint` will report the orphaned event or the input-less policy, which is exactly the
//! signal that the cut ran through the middle of a flow.

use crate::model::{Model, Phase};
use std::collections::{HashMap, HashSet, VecDeque};

/// Which part of the board to carve out. **Exactly one selector per run** — combining them is a
/// usage error the CLI rejects (exit 2) rather than guessing at an intersection order, since
/// `--focus E4 --hops 2 --type hotspot` would have to define whether the BFS runs before or after
/// the lane filter. An intersection can be added later without breaking either shape.
#[derive(Clone, PartialEq, Debug)]
pub enum Selector {
    /// Every element whose `col` falls inside the named region's band. Membership is spatial —
    /// there is no membership field (see [`crate::model::region_of`]).
    Region(String),
    /// The element `id` plus everything within `hops` edges of it, following edges in **either**
    /// direction (a policy's trigger is as much its neighbourhood as its output).
    Focus { id: String, hops: usize },
    /// Every element in one lane (`event`, `hotspot`, …).
    Kind(String),
}

impl Selector {
    /// A calm human label for the extracted board's title suffix.
    pub fn label(&self) -> String {
        match self {
            Selector::Region(id) => format!("region {id}"),
            Selector::Focus { id, hops } => {
                format!("{id} + {hops} {}", if *hops == 1 { "hop" } else { "hops" })
            }
            Selector::Kind(kind) => format!("{kind} lane"),
        }
    }

    /// The filename discriminator appended to the source's board name (`orders` + `K2` →
    /// `orders-K2.event-log.jsonl`). Kept short and free of path separators or spaces so the
    /// result is a plain sibling file, never a nested path.
    pub fn slug(&self) -> String {
        let raw = match self {
            Selector::Region(id) => id.clone(),
            Selector::Focus { id, hops } => format!("{id}-h{hops}"),
            Selector::Kind(kind) => kind.clone(),
        };
        raw.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect()
    }
}

/// Carve the sub-board a selector names.
///
/// Fails (rather than returning an empty board) when the selector names something the board does
/// not have, or matches no element at all: a typo'd region id or a mistyped lane must not
/// silently produce a valid, empty, useless extract.
pub fn extract(m: &Model, sel: &Selector) -> Result<Model, String> {
    let keep = select(m, sel)?;
    if keep.is_empty() {
        return Err(format!("{} matched no elements", sel.label()));
    }

    let elements: Vec<_> = m
        .elements
        .iter()
        .filter(|e| keep.contains(e.id.as_str()))
        .cloned()
        .collect();
    // Both endpoints must survive: a half-edge would point at an element the sub-board does not
    // contain, which replays into a dangling reference `lint` already treats as no edge at all.
    let edges: Vec<_> = m
        .edges
        .iter()
        .filter(|e| keep.contains(e.src.as_str()) && keep.contains(e.dst.as_str()))
        .cloned()
        .collect();
    let phases = clip_phases(&m.phases, &elements);

    let label = sel.label();
    let title = if m.title.is_empty() {
        label
    } else {
        format!("{} · {}", m.title, label)
    };
    Ok(Model {
        title,
        level: m.level,
        phases,
        elements,
        edges,
    })
}

/// The ids a selector picks out, or an error naming what the board does not have.
fn select<'a>(m: &'a Model, sel: &Selector) -> Result<HashSet<&'a str>, String> {
    match sel {
        Selector::Region(id) => {
            let band = m.phases.iter().find(|p| &p.id == id).ok_or_else(|| {
                let known: Vec<&str> = m.phases.iter().map(|p| p.id.as_str()).collect();
                match known.is_empty() {
                    true => format!("no region {id}: this board has no regions"),
                    false => format!("no region {id} (this board has {})", known.join(", ")),
                }
            })?;
            // Spatial membership, so an element with no `col` sits in no band at all — `col` is
            // assigned at render time, and inventing one here would make the cut depend on file
            // order rather than on the timeline the user is looking at.
            Ok(m.elements
                .iter()
                .filter(|e| {
                    e.col
                        .is_some_and(|c| band.from_col <= c && c <= band.to_col)
                })
                .map(|e| e.id.as_str())
                .collect())
        }
        Selector::Focus { id, hops } => {
            let start = m
                .elements
                .iter()
                .find(|e| &e.id == id)
                .ok_or_else(|| format!("no element {id} on this board"))?;
            Ok(neighbourhood(m, start.id.as_str(), *hops))
        }
        Selector::Kind(kind) => Ok(m
            .elements
            .iter()
            .filter(|e| &e.kind == kind)
            .map(|e| e.id.as_str())
            .collect()),
    }
}

/// Breadth-first closure around `start`, bounded to `hops` edges. Undirected: an edge is a
/// relation between two stickies, and reading only downstream would drop the command that
/// *causes* the event you focused on.
///
/// Only **real** edges are traversed — both endpoints must exist on the board — so a dangling
/// edge left by a deleted element cannot drag a phantom id into the selection. `hops: 0` is
/// legal and yields the element alone.
fn neighbourhood<'a>(m: &'a Model, start: &'a str, hops: usize) -> HashSet<&'a str> {
    let ids: HashSet<&str> = m.elements.iter().map(|e| e.id.as_str()).collect();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &m.edges {
        let (src, dst) = (e.src.as_str(), e.dst.as_str());
        if src == dst || !ids.contains(src) || !ids.contains(dst) {
            continue;
        }
        adj.entry(src).or_default().push(dst);
        adj.entry(dst).or_default().push(src);
    }

    let mut seen: HashSet<&str> = HashSet::from([start]);
    let mut queue: VecDeque<(&str, usize)> = VecDeque::from([(start, 0)]);
    while let Some((id, depth)) = queue.pop_front() {
        if depth == hops {
            continue;
        }
        for &next in adj.get(id).into_iter().flatten() {
            if seen.insert(next) {
                queue.push_back((next, depth + 1));
            }
        }
    }
    seen
}

/// The bands that survive the cut, clipped to the columns the selection actually occupies.
///
/// An extract keeps its regions rather than coming out phase-less: `--region K2` should produce a
/// board that still says "K2". Bands that no longer cover anything are dropped, and the rest are
/// trimmed to the surviving `[min, max]` column span, then re-projected by
/// [`crate::model::normalize`] onto the contiguous, gap-free partition every `Model` owes.
///
/// A selection with no columns at all (every element `col`-less) keeps no bands: there is no
/// timeline span to clip against.
fn clip_phases(phases: &[Phase], elements: &[crate::model::Element]) -> Vec<Phase> {
    let cols: Vec<i64> = elements.iter().filter_map(|e| e.col).collect();
    let (Some(&min), Some(&max)) = (cols.iter().min(), cols.iter().max()) else {
        return Vec::new();
    };
    let mut kept: Vec<Phase> = phases
        .iter()
        .filter(|p| p.from_col <= max && p.to_col >= min)
        .map(|p| Phase {
            id: p.id.clone(),
            label: p.label.clone(),
            from_col: p.from_col.max(min),
            to_col: p.to_col.min(max),
        })
        .collect();
    crate::model::normalize(&mut kept);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;
    use crate::model::from_json;

    /// A three-region board: K1 (0..1) actor→command, K2 (2..4) the order flow, K3 (5..6) a tail.
    const BOARD: &str = r#"{
        "title":"Orders",
        "phases":[
            {"id":"K1","label":"Browse","fromCol":0,"toCol":1},
            {"id":"K2","label":"Order","fromCol":2,"toCol":4},
            {"id":"K3","label":"Ship","fromCol":5,"toCol":6}
        ],
        "elements":[
            {"id":"A1","type":"actor","label":"Buyer","col":0},
            {"id":"C1","type":"command","label":"Place order","col":2},
            {"id":"E1","type":"event","label":"Order placed","col":3},
            {"id":"P1","type":"policy","label":"On order","col":4},
            {"id":"E2","type":"event","label":"Shipped","col":5},
            {"id":"H1","type":"hotspot","label":"VAT?","col":6},
            {"id":"H2","type":"hotspot","label":"refunds?","col":0}
        ],
        "edges":[["A1","C1"],["C1","E1"],["E1","P1"],["P1","E2"],["E2","H1"]]
    }"#;

    fn board() -> Model {
        from_json(&json::parse(BOARD).unwrap())
    }

    fn ids(m: &Model) -> Vec<&str> {
        m.elements.iter().map(|e| e.id.as_str()).collect()
    }

    // ---- by region -----------------------------------------------------------------------

    #[test]
    fn region_takes_the_elements_whose_col_falls_in_the_band() {
        let sub = extract(&board(), &Selector::Region("K2".into())).unwrap();
        assert_eq!(ids(&sub), ["C1", "E1", "P1"]);
    }

    #[test]
    fn region_keeps_its_band_clipped_to_the_survivors() {
        let sub = extract(&board(), &Selector::Region("K2".into())).unwrap();
        assert_eq!(sub.phases.len(), 1, "only the selected band survives");
        assert_eq!(sub.phases[0].id, "K2", "the region keeps its identity");
        assert_eq!((sub.phases[0].from_col, sub.phases[0].to_col), (2, 4));
    }

    #[test]
    fn an_unknown_region_is_an_error_not_an_empty_board() {
        let err = extract(&board(), &Selector::Region("K9".into())).unwrap_err();
        assert!(err.contains("K9"), "{err}");
        assert!(
            err.contains("K1, K2, K3"),
            "names the ones that exist: {err}"
        );
    }

    // ---- by neighbourhood ----------------------------------------------------------------

    #[test]
    fn focus_zero_hops_is_the_element_alone() {
        let sel = Selector::Focus {
            id: "E1".into(),
            hops: 0,
        };
        let sub = extract(&board(), &sel).unwrap();
        assert_eq!(ids(&sub), ["E1"]);
        assert!(sub.edges.is_empty(), "no edge has both endpoints inside");
    }

    #[test]
    fn focus_walks_edges_in_both_directions() {
        let sel = Selector::Focus {
            id: "E1".into(),
            hops: 1,
        };
        let sub = extract(&board(), &sel).unwrap();
        // C1 is upstream, P1 downstream — an event's producer is as much its neighbour as its
        // consumer, so a directed walk would be the wrong reading.
        assert_eq!(ids(&sub), ["C1", "E1", "P1"]);
    }

    #[test]
    fn focus_hops_bound_the_walk() {
        let two = extract(
            &board(),
            &Selector::Focus {
                id: "E1".into(),
                hops: 2,
            },
        )
        .unwrap();
        assert_eq!(ids(&two), ["A1", "C1", "E1", "P1", "E2"]);
        assert!(!ids(&two).contains(&"H1"), "H1 is three hops out");
    }

    #[test]
    fn focus_on_an_unknown_element_is_an_error() {
        let sel = Selector::Focus {
            id: "E9".into(),
            hops: 1,
        };
        assert!(extract(&board(), &sel).unwrap_err().contains("E9"));
    }

    #[test]
    fn a_dangling_edge_cannot_drag_a_phantom_id_in() {
        let mut m = board();
        m.edges.push(crate::model::Edge {
            src: "E1".into(),
            dst: "GHOST".into(),
            label: None,
        });
        let sub = extract(
            &m,
            &Selector::Focus {
                id: "E1".into(),
                hops: 1,
            },
        )
        .unwrap();
        assert_eq!(ids(&sub), ["C1", "E1", "P1"]);
    }

    // ---- by lane -------------------------------------------------------------------------

    #[test]
    fn kind_takes_one_lane_across_the_whole_timeline() {
        let sub = extract(&board(), &Selector::Kind("hotspot".into())).unwrap();
        assert_eq!(ids(&sub), ["H1", "H2"]);
    }

    #[test]
    fn scattered_survivors_keep_every_band_they_touch_as_a_partition() {
        // H2 sits in K1 (col 0), H1 in K3 (col 6) — K2 spans between them and is kept too, since
        // the clipped span is 0..6. `normalize` guarantees the result is still a partition.
        let sub = extract(&board(), &Selector::Kind("hotspot".into())).unwrap();
        assert_eq!(
            sub.phases.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["K1", "K2", "K3"]
        );
        for pair in sub.phases.windows(2) {
            assert_eq!(
                pair[1].from_col,
                pair[0].to_col + 1,
                "bands stay gap-free and overlap-free"
            );
        }
    }

    #[test]
    fn an_empty_lane_is_an_error() {
        let err = extract(&board(), &Selector::Kind("readmodel".into())).unwrap_err();
        assert!(err.contains("matched no elements"), "{err}");
    }

    // ---- what every extract owes ---------------------------------------------------------

    #[test]
    fn ids_and_cols_are_preserved_so_the_extract_diffs_cleanly() {
        let m = board();
        let sub = extract(&m, &Selector::Region("K2".into())).unwrap();
        for e in &sub.elements {
            let origin = m.elements.iter().find(|o| o.id == e.id).unwrap();
            assert_eq!(e, origin, "an extracted element is the original, verbatim");
        }
    }

    #[test]
    fn an_edge_leaving_the_selection_is_dropped() {
        let sub = extract(&board(), &Selector::Region("K2".into())).unwrap();
        let wires: Vec<_> = sub
            .edges
            .iter()
            .map(|e| (e.src.as_str(), e.dst.as_str()))
            .collect();
        // A1→C1 and P1→E2 cross the cut; only the interior wires survive.
        assert_eq!(wires, [("C1", "E1"), ("E1", "P1")]);
    }

    #[test]
    fn the_title_says_where_the_board_came_from() {
        let sub = extract(&board(), &Selector::Region("K2".into())).unwrap();
        assert_eq!(sub.title, "Orders · region K2");
    }

    #[test]
    fn the_level_is_inherited_so_lint_stays_as_strict() {
        let mut m = board();
        m.level = crate::model::Level::Design;
        let sub = extract(&m, &Selector::Region("K2".into())).unwrap();
        assert_eq!(sub.level, crate::model::Level::Design);
    }

    #[test]
    fn a_col_less_selection_keeps_no_bands() {
        let m = from_json(
            &json::parse(
                r#"{"phases":[{"id":"K1","label":"p","fromCol":0,"toCol":3}],
                    "elements":[{"id":"E1","type":"event","label":"a"}]}"#,
            )
            .unwrap(),
        );
        let sub = extract(&m, &Selector::Kind("event".into())).unwrap();
        assert_eq!(ids(&sub), ["E1"]);
        assert!(sub.phases.is_empty(), "no span to clip against");
    }

    // ---- naming --------------------------------------------------------------------------

    #[test]
    fn slugs_are_plain_filename_fragments() {
        assert_eq!(Selector::Region("K2".into()).slug(), "K2");
        assert_eq!(
            Selector::Focus {
                id: "E4".into(),
                hops: 2
            }
            .slug(),
            "E4-h2"
        );
        assert_eq!(Selector::Kind("read model".into()).slug(), "read-model");
        assert_eq!(
            Selector::Region("../etc".into()).slug(),
            "---etc",
            "a path separator can never survive into the output filename"
        );
    }
}
