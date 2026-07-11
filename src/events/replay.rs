//! The projection: fold a slice of [`Event`]s into a `Model` ([`replay`]), plus the region-id
//! watermark ([`region_watermark`]) the server reads when minting a fresh region id.

use super::Event;
use crate::model::{resolve_region_id, Edge, Element, Model, Phase};

/// The region-id watermark after `events`: the highest `K` suffix any `PhaseAdded` or `PhaseSplit`
/// has spent — explicit ids *and* the synthetic ones [`replay`] mints for legacy id-less bands —
/// folded through the same [`resolve_region_id`] tracker `replay` threads through its projection.
/// The single home of the region-id namespace rule, so a server-side mint can never hand out a
/// suffix `replay` would later synthesize (it just returns `watermark + 1`). `replay` keeps its own
/// running counter because it mints *in order while folding*; both fold the same kinds through
/// `resolve_region_id`, so they can't diverge.
pub fn region_watermark(events: &[Event]) -> u32 {
    let mut max_region = 0u32;
    for ev in events {
        match ev {
            Event::PhaseAdded { id, .. } => {
                resolve_region_id(id.as_deref(), &mut max_region);
            }
            Event::PhaseSplit { new_id, .. } => {
                resolve_region_id(Some(new_id), &mut max_region);
            }
            _ => {}
        }
    }
    max_region
}

/// Fold a sequence of events into the board they describe. The projection is pure and
/// deterministic: same log → same `Model`. (The `max_region` counter below is the running form of
/// [`region_watermark`]; both fold `PhaseAdded`/`PhaseSplit` through `resolve_region_id`.)
pub fn replay(events: &[Event]) -> Model {
    let mut m = Model::default();
    // Highest `K` region suffix seen so far — threaded across the fold so a synthetic id for a
    // legacy (id-less) band never reuses a suffix freed by `PhaseRemoved` or taken by an explicit
    // id. Mirrors `serve::mint_id`'s "highest ever added" rule (see `resolve_region_id`).
    let mut max_region = 0u32;
    for ev in events {
        match ev {
            Event::BoardTitled { title } => m.title = title.clone(),
            Event::BoardLeveled { level } => m.level = crate::model::level_from_str(level),
            // Region/phase arms delegate to the partition helpers below; the `resolve_region_id`
            // calls stay here because they thread `max_region` — the fold's id-namespace state.
            Event::PhaseAdded {
                id,
                label,
                from_col,
                to_col,
            } => {
                let id = resolve_region_id(id.as_deref(), &mut max_region);
                add_phase(&mut m.phases, id, label.clone(), *from_col, *to_col);
            }
            Event::PhaseResized {
                id,
                from_col,
                to_col,
            } => resize_phase(&mut m.phases, id, *from_col, *to_col),
            Event::PhaseRenamed { id, label } => rename_phase(&mut m.phases, id, label.clone()),
            Event::PhaseRemoved { id } => remove_phase(&mut m.phases, id),
            Event::FrontierMoved { id, edge, col } => move_frontier(&mut m.phases, id, edge, *col),
            Event::PhaseSplit {
                id,
                at_col,
                new_id,
                new_label,
            } => {
                let new_id = resolve_region_id(Some(new_id), &mut max_region);
                split_phase(&mut m.phases, id, *at_col, new_id, new_label.clone());
            }
            Event::ElementAdded {
                id,
                kind,
                label,
                col,
                detail,
                y,
                links,
            } => {
                if !m.elements.iter().any(|e| &e.id == id) {
                    m.elements.push(Element {
                        id: id.clone(),
                        kind: kind.clone(),
                        label: label.clone(),
                        col: *col,
                        detail: detail.clone(),
                        y: *y,
                        resolved: false,
                        links: links.clone(),
                        diff: None,
                        was: None,
                    });
                }
            }
            Event::ElementRenamed { id, label } => {
                if let Some(e) = find(&mut m, id) {
                    e.label = label.clone();
                }
            }
            Event::ElementMoved { id, col, kind, y } => {
                if let Some(e) = find(&mut m, id) {
                    if col.is_some() {
                        e.col = *col;
                    }
                    if let Some(k) = kind {
                        e.kind = k.clone();
                    }
                    if y.is_some() {
                        e.y = *y;
                    }
                }
            }
            Event::ElementAnnotated { id, text } => {
                if let Some(e) = find(&mut m, id) {
                    e.detail = Some(text.clone());
                }
            }
            Event::HotspotResolved { id, resolution } => {
                if let Some(e) = find(&mut m, id) {
                    e.resolved = true;
                    e.detail = Some(resolution.clone());
                }
            }
            Event::ElementRemoved { id } => {
                m.elements.retain(|e| &e.id != id);
                m.edges.retain(|e| &e.src != id && &e.dst != id);
            }
            Event::EdgeAdded { src, dst, label } => {
                if !m.edges.iter().any(|e| &e.src == src && &e.dst == dst) {
                    m.edges.push(Edge {
                        src: src.clone(),
                        dst: dst.clone(),
                        label: label.clone(),
                        status: None,
                    });
                }
            }
            Event::EdgeRemoved { src, dst } => {
                m.edges.retain(|e| !(&e.src == src && &e.dst == dst))
            }
            // A compaction marker carries no board state; it only records that earlier history
            // was folded away. Replaying it is a no-op.
            Event::LogCompacted { .. } => {}
        }
    }
    // Regions are a *contiguous partition* of the timeline (F-region-frontiers): after folding every
    // phase event — new frontier moves/splits *and* legacy independent spans (`PhaseAdded`/
    // `PhaseResized`, which could leave holes or overlaps) — project the phase list to a gap-free,
    // overlap-free partition. The rule (`model::normalize`) is pure and deterministic, so `replay`
    // stays a pure function; it is shared with `from_json` so every `Model` is a partition whatever
    // its source (log or bootstrap `model.json`).
    crate::model::normalize(&mut m.phases);
    m
}

/// Append a phase, idempotent by id like `ElementAdded`: a duplicate `PhaseAdded` (a log appended
/// twice, or `from_model` of a model with non-unique phase ids — `normalize` never dedups ids) must
/// not push a second `Phase` sharing an id, or every later `Phase*` event would resolve by id and
/// address only the first, stranding a ghost region. A minted (legacy id-less) id is fresh past the
/// watermark, so this drops only true duplicates of an explicit id.
fn add_phase(phases: &mut Vec<Phase>, id: String, label: String, from_col: i64, to_col: i64) {
    if !phases.iter().any(|p| p.id == id) {
        phases.push(Phase {
            id,
            label,
            from_col,
            to_col,
            diff: None,
        });
    }
}

/// Legacy independent-span resize: set both borders. `normalize` (end of the fold) projects the
/// result back onto a gap-free partition, so a hole/overlap this opens is transient.
fn resize_phase(phases: &mut [Phase], id: &str, from_col: i64, to_col: i64) {
    if let Some(p) = phases.iter_mut().find(|p| p.id == id) {
        p.from_col = from_col;
        p.to_col = to_col;
    }
}

fn rename_phase(phases: &mut [Phase], id: &str, label: String) {
    if let Some(p) = phases.iter_mut().find(|p| p.id == id) {
        p.label = label;
    }
}

/// Remove = merge under the partition. An interior phase's freed columns are absorbed by the
/// neighbour `normalize` sweeps into; but a *board-end* phase would otherwise shrink the board and
/// strand its columns (region-less), contradicting "merge into the neighbour". So when the removed
/// phase held a board end, extend the outermost survivor to cover its span — the neighbour absorbs
/// it either way.
fn remove_phase(phases: &mut Vec<Phase>, id: &str) {
    if let Some(pos) = phases.iter().position(|p| p.id == id) {
        let rem = phases.remove(pos);
        if !phases.is_empty() {
            if rem.from_col <= phases.iter().map(|p| p.from_col).min().unwrap() {
                let lo = phases.iter_mut().min_by_key(|p| p.from_col).unwrap();
                lo.from_col = lo.from_col.min(rem.from_col);
            }
            if rem.to_col >= phases.iter().map(|p| p.to_col).max().unwrap() {
                let hi = phases.iter_mut().max_by_key(|p| p.to_col).unwrap();
                hi.to_col = hi.to_col.max(rem.to_col);
            }
        }
    }
}

/// Move one border of a phase; `normalize` (after the fold) re-borders the neighbour so the
/// partition stays gap-free. `"start"` moves only the board-left bound — the current leftmost
/// phase's `from_col`. Applying it to any other phase would change `normalize`'s sort key and
/// *reorder* the timeline, so restrict it to the leftmost (a stray `"start"` on any other phase is
/// then a true no-op, not a silent reorder).
fn move_frontier(phases: &mut [Phase], id: &str, edge: &str, col: i64) {
    if edge == "start" {
        let leftmost = phases.iter().map(|p| p.from_col).min();
        if let (Some(min), Some(p)) = (leftmost, phases.iter_mut().find(|p| p.id == id)) {
            if p.from_col == min {
                p.from_col = col;
            }
        }
    } else if let Some(p) = phases.iter_mut().find(|p| p.id == id) {
        p.to_col = col;
    }
}

/// Split a phase in two at `at_col`: `id` keeps `[from, at_col - 1]`, the minted `new_id` takes
/// `[at_col, to]`. A no-op unless `at_col` falls strictly inside the phase (so both halves keep ≥1
/// column). Placement of the right half doesn't matter — `normalize` (end of the fold) sorts by
/// column — so it is just pushed.
fn split_phase(phases: &mut Vec<Phase>, id: &str, at_col: i64, new_id: String, new_label: String) {
    if let Some(i) = phases.iter().position(|p| p.id == id) {
        let (from, to) = (phases[i].from_col, phases[i].to_col);
        if from < at_col && at_col <= to {
            phases[i].to_col = at_col - 1;
            phases.push(Phase {
                id: new_id,
                label: new_label,
                from_col: at_col,
                to_col: to,
                diff: None,
            });
        }
    }
}

fn find<'a>(m: &'a mut Model, id: &str) -> Option<&'a mut Element> {
    m.elements.iter_mut().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::testutil::*;
    use crate::events::*;
    use proptest::prelude::*;

    #[test]
    fn duplicate_phase_added_id_replays_to_a_single_region() {
        // A second `PhaseAdded` sharing an id (a double-appended log) must not create a ghost
        // region — replay is idempotent by id, like `ElementAdded`.
        let log = parse_log(
            "{\"event\":\"PhaseAdded\",\"id\":\"K1\",\"label\":\"A\",\"fromCol\":0,\"toCol\":3}\n\
             {\"event\":\"PhaseAdded\",\"id\":\"K1\",\"label\":\"B\",\"fromCol\":0,\"toCol\":3}",
        )
        .unwrap();
        assert_eq!(
            replay(&log).phases.iter().filter(|p| p.id == "K1").count(),
            1
        );
    }

    #[test]
    fn legacy_phase_without_id_replays_to_a_stable_positional_id() {
        // An old log's bands carry no id; replay must mint deterministic `K<n>` so resize/rename
        // can target them and two replays of the same log agree.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","label":"A","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseAdded","label":"B","fromCol":3,"toCol":5}"#),
        ];
        let m = replay(&evs);
        assert_eq!(
            m.phases.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["K1", "K2"]
        );
    }

    #[test]
    fn region_resize_rename_remove_fold_by_id() {
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"Old","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"Keep","fromCol":3,"toCol":4}"#),
            ev(r#"{"event":"PhaseResized","id":"K1","fromCol":0,"toCol":5}"#),
            ev(r#"{"event":"PhaseRenamed","id":"K1","label":"New"}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K2"}"#),
        ];
        let m = replay(&evs);
        assert_eq!(m.phases.len(), 1, "K2 removed");
        let k1 = &m.phases[0];
        assert_eq!(
            (k1.id.as_str(), k1.label.as_str(), k1.from_col, k1.to_col),
            ("K1", "New", 0, 5)
        );
    }

    #[test]
    fn synthetic_region_ids_never_reuse_a_freed_suffix() {
        // Regression: deriving the synthetic id from the live phase *count* would re-mint `K2`
        // after a removal. The id must come from the highest suffix ever seen, never reused —
        // the same reservation rule serve::mint_id uses for elements.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","label":"A","fromCol":0,"toCol":1}"#),
            ev(r#"{"event":"PhaseAdded","label":"B","fromCol":2,"toCol":3}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K1"}"#),
            ev(r#"{"event":"PhaseAdded","label":"C","fromCol":4,"toCol":5}"#),
        ];
        let m = replay(&evs);
        let ids: Vec<_> = m.phases.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["K2", "K3"],
            "the third add must be K3, not a reused K2"
        );
    }

    #[test]
    fn a_synthetic_id_skips_past_an_explicit_one() {
        // An explicit id raises the watermark, so a following legacy band mints past it.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K5","label":"Explicit","fromCol":0,"toCol":1}"#),
            ev(r#"{"event":"PhaseAdded","label":"Legacy","fromCol":2,"toCol":3}"#),
        ];
        let m = replay(&evs);
        assert_eq!(
            m.phases[1].id, "K6",
            "synthetic id mints one past the highest seen"
        );
    }

    #[test]
    fn region_ops_on_an_unknown_id_are_no_ops() {
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseRenamed","id":"K9","label":"ghost"}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K9"}"#),
        ];
        let m = replay(&evs);
        assert_eq!(m.phases.len(), 1);
        assert_eq!(m.phases[0].label, "A");
    }

    // ---- F-region-frontiers: the contiguous-partition spine ------------------------------
    // Regions are a partition, not independent spans. Frontier moves re-border a neighbour
    // atomically, split carves a phase in two, and `normalize` guarantees no log — new or legacy —
    // ever replays to a hole or an overlap.

    #[test]
    fn frontier_move_end_reborders_the_right_neighbour_atomically() {
        // Move the A|B frontier: posting only A's new `to_col`, `normalize` pulls B's `from_col`
        // with it. One event, both phases re-border — the partition can't open a gap.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"FrontierMoved","id":"K1","edge":"end","col":5}"#),
        ];
        let m = replay(&evs);
        let span = |i: usize| (m.phases[i].from_col, m.phases[i].to_col);
        assert_eq!(span(0), (0, 5), "A grew right to the new frontier");
        assert_eq!(span(1), (6, 7), "B's start followed — no gap, no overlap");
    }

    #[test]
    fn frontier_move_start_moves_the_board_left_bound() {
        // The outermost (leftmost) frontier is the first phase's `start`; moving it grows/shrinks
        // the whole board. `normalize` preserves that first `from_col` as the board-left anchor.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"FrontierMoved","id":"K1","edge":"start","col":-2}"#),
        ];
        let m = replay(&evs);
        assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (-2, 3));
        assert_eq!((m.phases[1].from_col, m.phases[1].to_col), (4, 7));
    }

    #[test]
    fn frontier_move_start_on_a_non_leftmost_phase_is_a_true_noop() {
        // Defensive (review #6): a "start" only moves the board-left bound — the current leftmost
        // phase. Applied to any other phase it would set that phase's from_col (normalize's sort
        // key) and reorder the timeline; replay must ignore it instead.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"FrontierMoved","id":"K2","edge":"start","col":-9}"#),
        ];
        let m = replay(&evs);
        let ids: Vec<_> = m.phases.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["K1", "K2"],
            "K2's stray start did not reorder the partition"
        );
        assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (0, 3));
        assert_eq!((m.phases[1].from_col, m.phases[1].to_col), (4, 7));
    }

    #[test]
    fn removing_a_board_end_phase_merges_into_the_neighbour() {
        // Review #4: removing a *board-end* phase must not strand its columns (shrink the board);
        // the neighbour absorbs them, so remove is always a merge. Remove the first phase → the new
        // first phase extends left to cover it; remove the last → the new last extends right.
        let seed = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"PhaseAdded","id":"K3","label":"C","fromCol":8,"toCol":11}"#),
        ];
        let mut first = seed.clone();
        first.push(ev(r#"{"event":"PhaseRemoved","id":"K1"}"#));
        let m = replay(&first);
        assert_eq!(
            m.phases.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["K2", "K3"]
        );
        assert_eq!(
            (m.phases[0].from_col, m.phases[0].to_col),
            (0, 7),
            "K2 absorbed K1's columns"
        );

        let mut last = seed;
        last.push(ev(r#"{"event":"PhaseRemoved","id":"K3"}"#));
        let m = replay(&last);
        assert_eq!(
            m.phases.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["K1", "K2"]
        );
        assert_eq!(
            (m.phases[1].from_col, m.phases[1].to_col),
            (4, 11),
            "K2 absorbed K3's columns"
        );
    }

    #[test]
    fn phase_split_carves_a_phase_in_two_keeping_a_partition() {
        // Add = split. The original id keeps the left half, the minted id takes the right, the two
        // stay contiguous. `newId` also raises the region-id watermark (a later legacy band mints
        // past it).
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"Whole","fromCol":0,"toCol":5}"#),
            ev(r#"{"event":"PhaseSplit","id":"K1","atCol":3,"newId":"K2","newLabel":"Right"}"#),
        ];
        let m = replay(&evs);
        assert_eq!(m.phases.len(), 2);
        assert_eq!(
            (
                m.phases[0].id.as_str(),
                m.phases[0].from_col,
                m.phases[0].to_col
            ),
            ("K1", 0, 2),
            "original keeps the left half"
        );
        assert_eq!(
            (
                m.phases[1].id.as_str(),
                m.phases[1].label.as_str(),
                m.phases[1].from_col,
                m.phases[1].to_col
            ),
            ("K2", "Right", 3, 5),
            "new phase takes the right half, contiguous"
        );
    }

    #[test]
    fn phase_split_outside_the_phase_is_a_no_op() {
        // `at_col` must land strictly inside (from < at <= to) so both halves keep ≥1 column.
        let base = ev(r#"{"event":"PhaseAdded","id":"K1","label":"W","fromCol":0,"toCol":3}"#);
        for at in ["0", "4", "9"] {
            let split = ev(&format!(
                r#"{{"event":"PhaseSplit","id":"K1","atCol":{at},"newId":"K2","newLabel":"R"}}"#
            ));
            let m = replay(&[base.clone(), split]);
            assert_eq!(m.phases.len(), 1, "at_col={at} splits nothing");
            assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (0, 3));
        }
    }

    #[test]
    fn removing_a_middle_phase_leaves_no_hole() {
        // Remove = merge under the partition: the freed columns are absorbed by the neighbour that
        // sweeps into them, never a gap. (v1 folds directional merge into remove — see the
        // F-region-frontiers working note; PhaseMerged is deferred.)
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"PhaseAdded","id":"K3","label":"C","fromCol":8,"toCol":11}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K2"}"#),
        ];
        let m = replay(&evs);
        let ids: Vec<_> = m.phases.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["K1", "K3"]);
        assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (0, 3));
        assert_eq!(
            (m.phases[1].from_col, m.phases[1].to_col),
            (4, 11),
            "C absorbed B's freed columns — the partition stays gap-free"
        );
    }

    #[test]
    fn replay_builds_the_board_the_events_describe() {
        let log = [
            ev(r#"{"event":"BoardTitled","title":"T"}"#),
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"Born","col":1}"#),
            ev(r#"{"event":"ElementAdded","id":"E2","type":"command","label":"Do","col":0}"#),
            ev(r#"{"event":"ElementRenamed","id":"E1","label":"Reborn"}"#),
            ev(r#"{"event":"ElementMoved","id":"E2","col":3}"#),
            ev(r#"{"event":"EdgeAdded","src":"E2","dst":"E1"}"#),
        ];
        let m = replay(&log);
        assert_eq!(m.title, "T");
        let e1 = m.elements.iter().find(|e| e.id == "E1").unwrap();
        assert_eq!(e1.label, "Reborn");
        let e2 = m.elements.iter().find(|e| e.id == "E2").unwrap();
        assert_eq!(e2.col, Some(3));
        assert_eq!(m.edges.len(), 1);
    }

    #[test]
    fn resolving_a_hotspot_flips_state_and_records_the_note() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"H1","type":"hotspot","label":"open?"}"#),
            ev(r#"{"event":"HotspotResolved","id":"H1","resolution":"settled"}"#),
        ];
        let h = &replay(&log).elements[0];
        assert!(h.resolved);
        assert_eq!(h.detail.as_deref(), Some("settled"));
    }

    #[test]
    fn remove_drops_the_element_and_its_edges() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A"}"#),
            ev(r#"{"event":"ElementAdded","id":"E2","type":"event","label":"B"}"#),
            ev(r#"{"event":"EdgeAdded","src":"E1","dst":"E2"}"#),
            ev(r#"{"event":"ElementRemoved","id":"E1"}"#),
        ];
        let m = replay(&log);
        assert_eq!(m.elements.len(), 1);
        assert!(m.edges.is_empty());
    }

    #[test]
    fn replay_sets_the_model_level_from_board_leveled() {
        let m = replay(&[ev(r#"{"event":"BoardLeveled","level":"design"}"#)]);
        assert_eq!(m.level, crate::model::Level::Design);
    }

    #[test]
    fn replay_applies_y_and_a_col_only_move_preserves_it() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":0}"#),
            ev(r#"{"event":"ElementMoved","id":"E1","y":0.8}"#),
            ev(r#"{"event":"ElementMoved","id":"E1","col":3}"#),
        ];
        let e1 = &replay(&log).elements[0];
        assert_eq!(e1.col, Some(3));
        assert_eq!(e1.y, Some(0.8), "a col-only nudge must not reset the Y");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// None of the move/rename/annotate/resolve arms creates or destroys an element — only
        /// `drop` removes, and nothing adds. Exactly the non-dropped genesis ids survive.
        #[test]
        fn pbt_comments_never_invent_an_element_and_only_drop_removes(
            comments in prop::collection::vec(comment_strategy(), 1..=8),
        ) {
            let (mut log, ids) = genesis();
            let mut dropped = std::collections::HashSet::new();
            for v in &comments {
                if v.get_str("kind") == Some("drop") {
                    if let Some(id) = v.get_str("elemId") {
                        dropped.insert(id.to_string());
                    }
                }
                log.extend(comment_to_events(v));
            }
            let model = replay(&log);
            let present: std::collections::HashSet<&str> =
                model.elements.iter().map(|e| e.id.as_str()).collect();
            // No phantom creation: every surviving id was a genesis id.
            for id in &present {
                prop_assert!(ids.contains(id), "invented element {id}");
            }
            // Exactly the non-dropped genesis ids survive.
            for id in &ids {
                prop_assert_eq!(
                    present.contains(id),
                    !dropped.contains(*id),
                    "element {} survival wrong",
                    id
                );
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(800))]

        /// F-region-frontiers: fold any interleaving of phase events — legacy independent spans
        /// (`PhaseAdded`/`PhaseResized`, which alone could gap or overlap), atomic frontier moves,
        /// splits, and removes — and the replayed phases are always a *contiguous partition*:
        /// sorted, gap-free, overlap-free, each ≥1 column wide; and `normalize` is its own fixed
        /// point. On failure proptest shrinks to the fewest events that still break it.
        #[test]
        fn pbt_phase_events_never_replay_to_a_hole_or_overlap(log in phase_log_strategy()) {
            let mut phases = replay(&log).phases;
            for w in phases.windows(2) {
                prop_assert_eq!(w[0].to_col + 1, w[1].from_col, "not a contiguous partition");
            }
            for p in &phases {
                prop_assert!(p.from_col <= p.to_col, "phase {} inverted", p.id);
            }
            // Idempotence: normalizing the already-normalized result changes nothing.
            let before: Vec<_> = phases
                .iter()
                .map(|p| (p.id.clone(), p.from_col, p.to_col))
                .collect();
            crate::model::normalize(&mut phases);
            let after: Vec<_> = phases
                .iter()
                .map(|p| (p.id.clone(), p.from_col, p.to_col))
                .collect();
            prop_assert_eq!(before, after, "normalize not idempotent");
        }
    }
}
