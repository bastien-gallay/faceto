//! The event-sourced spine's test suite. Kept as one co-located module because the
//! property-based tests share a generator harness (`Lcg` / `gen_comment` / `genesis`)
//! and most cases exercise the comment -> event -> replay pipeline transversally.

use super::codec::{parse_event, to_json};
use super::*;
use crate::json::{self, Json};
use std::path::Path;

fn ev(line: &str) -> Event {
    parse_event(&json::parse(line).unwrap()).unwrap()
}

// ---- F-container: regions (Stage 1, the event spine) ---------------------------------
// A region is a labelled vertical band that evolves the legacy `Phase`. Membership and
// pivotal are derived from geometry (later stages), so the spine only needs: add with a
// stable id, resize, rename, remove — plus legacy bands (no id) replaying deterministically.

#[test]
fn parse_log_errors_on_a_malformed_known_event_but_skips_an_unknown_kind() {
    // An unknown/future kind is skipped for forward compatibility.
    assert_eq!(
        parse_log(r#"{"event":"FromTheFuture","x":1}"#)
            .unwrap()
            .len(),
        0
    );
    // A *known* kind missing a required field is a hard error: the fact is in the append-only
    // log but would otherwise vanish from the projection with no diagnostic.
    assert!(parse_log(r#"{"event":"ElementAdded","id":"E1"}"#).is_err()); // no type/label
    assert!(parse_log(r#"{"event":"PhaseAdded","label":"A","fromCol":0}"#).is_err());
    // no toCol
}

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
fn phase_added_round_trips_its_id() {
    let e = ev(r#"{"event":"PhaseAdded","id":"K1","label":"Checkout","fromCol":0,"toCol":3}"#);
    assert!(matches!(&e, Event::PhaseAdded { id: Some(id), label, .. }
        if id == "K1" && label == "Checkout"));
    // serialize → parse is a fixed point
    assert_eq!(
        json::to_string(&to_json(&ev(&line(&e)))),
        json::to_string(&to_json(&e))
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
fn frontier_and_split_events_round_trip() {
    for line in [
        r#"{"event":"FrontierMoved","id":"K1","edge":"end","col":5}"#,
        r#"{"event":"FrontierMoved","id":"K1","edge":"start","col":-2}"#,
        r#"{"event":"PhaseSplit","id":"K1","atCol":3,"newId":"K2","newLabel":"Right"}"#,
    ] {
        let e = ev(line);
        assert_eq!(super::line(&e), line, "canonical serialize round-trips");
        assert_eq!(ev(&super::line(&e)), e, "reparse round-trips");
    }
}

#[test]
fn frontier_move_maps_from_a_comment_with_guards() {
    let mk = |body: &str| comment_to_events(&json::parse(body).unwrap());
    assert_eq!(
        mk(r#"{"kind":"frontier-move","regionId":"K1","edge":"end","col":5}"#),
        vec![Event::FrontierMoved {
            id: "K1".into(),
            edge: "end".into(),
            col: 5
        }]
    );
    assert!(
        mk(r#"{"kind":"frontier-move","regionId":"K1","edge":"sideways","col":5}"#).is_empty(),
        "an unknown edge is nothing to persist"
    );
    assert!(
        mk(r#"{"kind":"frontier-move","regionId":"K1","edge":"end"}"#).is_empty(),
        "a missing col is nothing to persist"
    );
}

#[test]
fn from_model_emits_region_ids_so_genesis_round_trips() {
    // compact()/genesis fold the final state into PhaseAdded; the id must survive so a
    // compacted log keeps stable region identity.
    let log = vec![
        ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":2}"#),
        ev(r#"{"event":"PhaseResized","id":"K1","fromCol":0,"toCol":9}"#),
    ];
    let folded = compact(&log);
    let m = replay(&folded);
    assert_eq!(m.phases[0].id, "K1");
    assert_eq!(m.phases[0].to_col, 9, "resize survives the fold");
}

// ---- F-inline-edit: a direct rename must not be able to blank a label -----------------
// Inline editing makes "select-all → delete → Enter" a one-gesture mistake. A blank rename
// must persist nothing (an empty label would replay into a never-renumbered empty box — the
// exact failure the `add` path already guards). These name the contract before it exists.

#[test]
fn rename_with_a_blank_label_is_rejected() {
    for blank in ["", "   ", "\t", "\n  "] {
        let v = json::parse(&format!(
            r#"{{"elemId":"E1","kind":"rename","text":{:?}}}"#,
            blank
        ))
        .unwrap();
        assert!(
            comment_to_events(&v).is_empty(),
            "blank rename {:?} should persist nothing",
            blank
        );
    }
}

#[test]
fn rename_trims_surrounding_whitespace() {
    let v = json::parse(r#"{"elemId":"E1","kind":"rename","text":"  PaymentTaken  "}"#).unwrap();
    let evs = comment_to_events(&v);
    assert!(
        matches!(&evs[..], [Event::ElementRenamed { id, label }] if id == "E1" && label == "PaymentTaken"),
        "got {:?}",
        evs
    );
}

#[test]
fn rename_with_real_text_still_renames() {
    // Non-regression: a genuine rename is unchanged by the new guard.
    let v = json::parse(r#"{"elemId":"E1","kind":"rename","text":"Reborn"}"#).unwrap();
    let evs = comment_to_events(&v);
    assert!(matches!(&evs[..], [Event::ElementRenamed { id, label }]
        if id == "E1" && label == "Reborn"));
}

// ---- Property-based tests (std-only, hand-rolled) -------------------------------------
// faceto takes no crates (CLAUDE.md: zero dependencies), so there is no proptest/quickcheck.
// A tiny deterministic LCG drives reproducible random scenarios — each seed is one case, and
// a failure prints the seed + the offending comment sequence so it replays exactly.

struct Lcg(u64);
impl Lcg {
    fn next_u64(&mut self) -> u64 {
        // Knuth MMIX LCG constants — full-period over u64.
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// A mix of real strings and blanks: the no-blank-label invariant is precisely that a blank
// rename can never empty a box, so the generator must reach for blanks on purpose.
const TEXTS: [&str; 6] = ["Paid", "ItemAdded", "  spaced  ", "", "   ", "\t"];
const KINDS: [&str; 5] = ["rename", "move", "drop", "comment", "resolve"];

// One random comment over the given element ids, plus a textual form for failure reports.
fn gen_comment(rng: &mut Lcg, ids: &[&str]) -> (Json, String) {
    let id = ids[rng.below(ids.len())];
    let kind = KINDS[rng.below(KINDS.len())];
    let text = TEXTS[rng.below(TEXTS.len())];
    let mut o = vec![
        ("elemId".to_string(), Json::Str(id.to_string())),
        ("kind".to_string(), Json::Str(kind.to_string())),
        ("text".to_string(), Json::Str(text.to_string())),
    ];
    if kind == "move" {
        o.push(("col".to_string(), Json::Num(rng.below(6) as f64)));
    }
    let v = Json::Obj(o);
    (v.clone(), json::to_string(&v))
}

// A small fixed board of non-blank elements, one per lane id-prefix used here.
fn genesis() -> (Vec<Event>, Vec<&'static str>) {
    let ids = vec!["E1", "E2", "C1", "A1", "H1"];
    let kinds = ["event", "event", "command", "aggregate", "hotspot"];
    let evs = ids
        .iter()
        .zip(kinds)
        .map(|(id, k)| Event::ElementAdded {
            id: (*id).to_string(),
            kind: k.to_string(),
            label: format!("seed-{id}"),
            col: Some(0),
            detail: None,
            y: None,
        })
        .collect();
    (evs, ids)
}

#[test]
fn pbt_no_comment_sequence_ever_leaves_a_blank_label() {
    // Property: folding any sequence of comment objects through `comment_to_events` and
    // replaying never yields an element whose label is blank. RED today — a blank rename
    // overwrites the label with "".
    for seed in 0..500u64 {
        let mut rng = Lcg(seed.wrapping_mul(2_654_435_761).wrapping_add(1));
        let (mut log, ids) = genesis();
        let n = 1 + rng.below(8);
        let mut trace = Vec::new();
        for _ in 0..n {
            let (v, shown) = gen_comment(&mut rng, &ids);
            trace.push(shown);
            log.extend(comment_to_events(&v));
        }
        let model = replay(&log);
        for e in &model.elements {
            assert!(
                !e.label.trim().is_empty(),
                "seed {seed}: element {} got a blank label after:\n  {}",
                e.id,
                trace.join("\n  ")
            );
        }
    }
}

#[test]
fn pbt_comments_never_invent_an_element_and_only_drop_removes() {
    // Non-regression over the adjacent move/rename/annotate/resolve arms: none of them may
    // create or destroy an element — only `drop` removes, and nothing adds. Guards the move
    // path this feature sits next to.
    for seed in 0..500u64 {
        let mut rng = Lcg(seed.wrapping_mul(40_503).wrapping_add(7));
        let (mut log, ids) = genesis();
        let mut dropped = std::collections::HashSet::new();
        let n = 1 + rng.below(8);
        for _ in 0..n {
            let (v, _) = gen_comment(&mut rng, &ids);
            if v.get_str("kind") == Some("drop") {
                if let Some(id) = v.get_str("elemId") {
                    dropped.insert(id.to_string());
                }
            }
            log.extend(comment_to_events(&v));
        }
        let model = replay(&log);
        let present: std::collections::HashSet<&str> =
            model.elements.iter().map(|e| e.id.as_str()).collect();
        // No phantom creation: every surviving id was a genesis id.
        for id in &present {
            assert!(ids.contains(id), "seed {seed}: invented element {id}");
        }
        // Exactly the non-dropped genesis ids survive.
        for id in &ids {
            let want = !dropped.contains(*id);
            assert_eq!(
                present.contains(id),
                want,
                "seed {seed}: element {id} present={} but dropped={}",
                present.contains(id),
                dropped.contains(*id)
            );
        }
    }
}

#[test]
fn pbt_phase_events_never_replay_to_a_hole_or_overlap() {
    // Property (F-region-frontiers): fold any interleaving of phase events — legacy independent
    // spans (`PhaseAdded`/`PhaseResized`, which alone could gap or overlap), atomic frontier
    // moves, splits, and removes — and the replayed phases are always a *contiguous partition*:
    // sorted, gap-free, overlap-free, each ≥1 column wide. And `normalize` is its own fixed
    // point (a second pass changes nothing).
    for seed in 0..800u64 {
        let mut rng = Lcg(seed.wrapping_mul(2_246_822_519).wrapping_add(3));
        let mut log: Vec<Event> = Vec::new();
        let mut minted = 0u32; // client-minted ids for add/split, distinct from replay's own
        let n = 1 + rng.below(12);
        let mut trace = Vec::new();
        for _ in 0..n {
            // Ids that could exist so far (K1..=K{minted}); ops on absent ids are valid no-ops.
            let target = format!("K{}", 1 + rng.below((minted.max(1)) as usize));
            let (a, b) = (rng.below(9) as i64 - 2, rng.below(9) as i64 - 2);
            let ev = match rng.below(5) {
                0 => {
                    minted += 1;
                    Event::PhaseAdded {
                        id: Some(format!("K{minted}")),
                        label: format!("p{minted}"),
                        from_col: a.min(b),
                        to_col: a.max(b),
                    }
                }
                1 => Event::PhaseResized {
                    id: target,
                    from_col: a.min(b),
                    to_col: a.max(b),
                },
                2 => Event::FrontierMoved {
                    id: target,
                    edge: if rng.below(2) == 0 { "start" } else { "end" }.into(),
                    col: a,
                },
                3 => {
                    minted += 1;
                    Event::PhaseSplit {
                        id: target,
                        at_col: a,
                        new_id: format!("K{minted}"),
                        new_label: format!("s{minted}"),
                    }
                }
                _ => Event::PhaseRemoved { id: target },
            };
            trace.push(line(&ev));
            log.push(ev);
        }
        let mut phases = replay(&log).phases;
        for w in phases.windows(2) {
            assert!(
                w[0].to_col + 1 == w[1].from_col,
                "seed {seed}: not contiguous ({}..{} then {}..{}) after:\n  {}",
                w[0].from_col,
                w[0].to_col,
                w[1].from_col,
                w[1].to_col,
                trace.join("\n  ")
            );
        }
        for p in &phases {
            assert!(
                p.from_col <= p.to_col,
                "seed {seed}: phase {} inverted",
                p.id
            );
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
        assert_eq!(before, after, "seed {seed}: normalize not idempotent");
    }
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

// The migration contract: an existing model → genesis events → replay must reproduce it.
#[test]
fn from_model_then_replay_round_trips() {
    let src = r#"{
        "title":"Round Trip",
        "phases":[{"label":"p","fromCol":0,"toCol":2}],
        "elements":[
            {"id":"E1","type":"event","label":"Made","col":1},
            {"id":"E2","type":"command","label":"Do","col":0,"detail":"a note"},
            {"id":"H1","type":"hotspot","label":"q","col":2,"resolved":true,"detail":"done"}
        ],
        "edges":[["E2","E1"]]
    }"#;
    let original = crate::model::from_json(&json::parse(src).unwrap());
    let rebuilt = replay(&from_model(&original));

    assert_eq!(rebuilt.title, original.title);
    assert_eq!(rebuilt.phases.len(), 1);
    assert_eq!(rebuilt.elements.len(), 3);
    assert_eq!(rebuilt.edges.len(), 1);
    let h1 = rebuilt.elements.iter().find(|e| e.id == "H1").unwrap();
    assert!(h1.resolved);
    assert_eq!(h1.detail.as_deref(), Some("done"));
    let e2 = rebuilt.elements.iter().find(|e| e.id == "E2").unwrap();
    assert_eq!(e2.detail.as_deref(), Some("a note"));
}

// ---- F-es-lint: the board level round-trips through the log ----------------------------

#[test]
fn board_leveled_is_a_serialize_parse_fixed_point() {
    let e = ev(r#"{"event":"BoardLeveled","level":"design"}"#);
    assert!(matches!(&e, Event::BoardLeveled { level } if level == "design"));
    assert_eq!(
        json::to_string(&to_json(&ev(&line(&e)))),
        json::to_string(&to_json(&e))
    );
}

#[test]
fn replay_sets_the_model_level_from_board_leveled() {
    let m = replay(&[ev(r#"{"event":"BoardLeveled","level":"design"}"#)]);
    assert_eq!(m.level, crate::model::Level::Design);
}

#[test]
fn from_model_emits_board_leveled_only_for_a_design_board() {
    // A design board round-trips its level and writes exactly one BoardLeveled event.
    let design =
        crate::model::from_json(&json::parse(r#"{"level":"design","elements":[]}"#).unwrap());
    let batch = from_model(&design);
    assert_eq!(
        batch
            .iter()
            .filter(|e| matches!(e, Event::BoardLeveled { .. }))
            .count(),
        1
    );
    assert_eq!(replay(&batch).level, crate::model::Level::Design);

    // A big-picture (default) board emits none, so its genesis batch is unchanged.
    let big = crate::model::from_json(&json::parse(r#"{"elements":[]}"#).unwrap());
    assert!(!from_model(&big)
        .iter()
        .any(|e| matches!(e, Event::BoardLeveled { .. })));
}

#[test]
fn a_design_board_survives_compaction() {
    let design =
        crate::model::from_json(&json::parse(r#"{"level":"design","elements":[]}"#).unwrap());
    let folded = compact(&from_model(&design));
    assert_eq!(replay(&folded).level, crate::model::Level::Design);
}

#[test]
fn compact_preserves_the_projection_and_folds_history() {
    let log = [
        ev(r#"{"event":"BoardTitled","title":"T"}"#),
        ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"Born","col":1}"#),
        ev(r#"{"event":"ElementRenamed","id":"E1","label":"Reborn"}"#),
        ev(r#"{"event":"ElementAnnotated","id":"E1","text":"a note"}"#),
        ev(r#"{"event":"ElementAdded","id":"H1","type":"hotspot","label":"q"}"#),
        ev(r#"{"event":"HotspotResolved","id":"H1","resolution":"settled"}"#),
    ];
    let folded = compact(&log);

    // Leads with a provenance marker recording the prior length, and reparses cleanly.
    assert!(matches!(folded[0], Event::LogCompacted { folded: 6 }));
    let reparsed = parse_log(&to_jsonl(&folded)).unwrap();
    assert!(matches!(reparsed[0], Event::LogCompacted { folded: 6 }));

    // Shorter than the original: the rename + annotate + resolve history collapsed.
    assert!(folded.len() < log.len());

    // Same projection: title, the *latest* label, the note folded into detail, the resolution.
    let (before, after) = (replay(&log), replay(&folded));
    assert_eq!(after.title, before.title);
    let e1 = after.elements.iter().find(|e| e.id == "E1").unwrap();
    assert_eq!(e1.label, "Reborn");
    assert_eq!(e1.detail.as_deref(), Some("a note"));
    let h1 = after.elements.iter().find(|e| e.id == "H1").unwrap();
    assert!(h1.resolved);
    assert_eq!(h1.detail.as_deref(), Some("settled"));
}

#[test]
fn compacting_twice_leaves_the_snapshot_stable() {
    let log = [
        ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":0}"#),
        ev(r#"{"event":"ElementMoved","id":"E1","col":2}"#),
    ];
    let once = compact(&log);
    let twice = compact(&once);
    // The genesis tail (everything past the marker) is a fixed point; only the count moves.
    assert_eq!(to_jsonl(&once[1..]), to_jsonl(&twice[1..]));
}

// H5: a legacy comments.jsonl folded after a model's genesis batch must reconstruct both the
// board and its feedback (annotation, resolution, rename, move).
#[test]
fn from_comments_folds_a_legacy_inbox_onto_the_genesis_batch() {
    let model_src = r#"{
        "title":"Legacy",
        "elements":[
            {"id":"E1","type":"event","label":"Born","col":0},
            {"id":"H1","type":"hotspot","label":"open?","col":2}
        ]
    }"#;
    let model = crate::model::from_json(&json::parse(model_src).unwrap());
    let inbox = "\
        {\"elemId\":\"E1\",\"kind\":\"comment\",\"text\":\"a note\"}\n\
        {\"elemId\":\"E1\",\"kind\":\"rename\",\"text\":\"Reborn\"}\n\
        {\"elemId\":\"E1\",\"kind\":\"move\",\"col\":4}\n\
        {\"elemId\":\"H1\",\"kind\":\"resolve\",\"text\":\"settled\"}\n";

    let (folded, skipped) = from_comments(inbox);
    assert_eq!(skipped, 0); // every line migrated
    let mut log = from_model(&model);
    log.extend(folded);
    let m = replay(&log);

    let e1 = m.elements.iter().find(|e| e.id == "E1").unwrap();
    assert_eq!(e1.label, "Reborn"); // rename applied
    assert_eq!(e1.col, Some(4)); // move applied
                                 // The annotation lands first, then the rename overwrites the label — but `detail` keeps
                                 // the note (annotation sets detail; rename only touches the label).
    assert_eq!(e1.detail.as_deref(), Some("a note"));
    let h1 = m.elements.iter().find(|e| e.id == "H1").unwrap();
    assert!(h1.resolved);
    assert_eq!(h1.detail.as_deref(), Some("settled"));
}

#[test]
fn from_comments_skips_blank_malformed_and_element_less_lines() {
    let inbox = "\
        \n  \n\
        {not json}\n\
        {\"kind\":\"comment\",\"text\":\"orphan, no elemId\"}\n\
        {\"kind\":\"add\",\"type\":\"event\",\"text\":\"legacy add, no elemId\"}\n\
        {\"elemId\":\"E1\",\"kind\":\"comment\",\"text\":\"kept\"}\n";
    let (evs, skipped) = from_comments(inbox);
    assert_eq!(evs.len(), 1);
    assert!(matches!(&evs[0], Event::ElementAnnotated { id, text }
        if id == "E1" && text == "kept"));
    // Blank lines are not counted; the malformed line, the orphan, and the legacy `add` are.
    assert_eq!(skipped, 3);
}

// H3: a renamed event kind from an older schema is migrated forward at the upcast seam, so an
// old log still replays. `CommentAdded` predates the rename to `ElementAnnotated`.
#[test]
fn legacy_comment_kind_upcasts_to_element_annotated() {
    let log = [
        ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A"}"#),
        ev(r#"{"event":"CommentAdded","id":"E1","text":"from an old log"}"#),
        ev(r#"{"event":"Comment","id":"E1","text":"older still"}"#),
    ];
    assert!(matches!(&log[1], Event::ElementAnnotated { id, text }
        if id == "E1" && text == "from an old log"));
    assert!(matches!(&log[2], Event::ElementAnnotated { .. }));
    // …and the migrated event folds into the projection like any annotation.
    assert_eq!(
        replay(&log).elements[0].detail.as_deref(),
        Some("older still")
    );
}

// H3: additive change is free — an unknown field on a known event is ignored, not an error,
// so a log written by a newer schema still replays on older code.
#[test]
fn unknown_fields_on_a_known_event_are_ignored() {
    let e =
        ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","fromTheFuture":42}"#);
    assert!(matches!(e, Event::ElementAdded { id, .. } if id == "E1"));
}

#[test]
fn unknown_event_kinds_are_skipped_for_forward_compat() {
    let log = parse_log(
        "{\"event\":\"ElementAdded\",\"id\":\"E1\",\"type\":\"event\",\"label\":\"A\"}\n\
         {\"event\":\"SomethingFromTheFuture\",\"id\":\"E1\"}\n",
    )
    .unwrap();
    assert_eq!(log.len(), 1);
}

#[test]
fn blank_lines_skipped_but_malformed_json_is_an_error() {
    assert!(parse_log("\n  \n").unwrap().is_empty());
    assert!(parse_log("{not json}").is_err());
}

#[test]
fn events_serialize_to_canonical_jsonl_and_reparse() {
    let original =
        ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":2,"detail":"d"}"#);
    assert_eq!(ev(&line(&original)), original);
    let moved = Event::ElementMoved {
        id: "E1".into(),
        col: Some(4),
        kind: None,
        y: None,
    };
    assert_eq!(
        line(&moved),
        r#"{"event":"ElementMoved","id":"E1","col":4}"#
    );
}

// ---- F-2d-placement: the stored vertical sub-position ---------------------------------
// `y` is a fraction of the lane-band interior in [0, 1] — never identity (`id`), never the
// lane (`type`), never the timeline (`col`). It evolves the schema additively: an old log
// simply has no `y` and replays exactly as before.

#[test]
fn element_moved_round_trips_its_y() {
    let e = ev(r#"{"event":"ElementMoved","id":"E1","y":0.35}"#);
    assert!(
        matches!(&e, Event::ElementMoved { id, col: None, y: Some(y), .. }
        if id == "E1" && *y == 0.35)
    );
    assert_eq!(line(&e), r#"{"event":"ElementMoved","id":"E1","y":0.35}"#);
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

#[test]
fn a_placed_elements_y_survives_compact() {
    // `compact` folds the projection into ElementAdded lines; without `y` on the add the
    // whole 2D placement would silently flatten on every snapshot.
    let log = [
        ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":0}"#),
        ev(r#"{"event":"ElementMoved","id":"E1","y":0.25}"#),
    ];
    let folded = compact(&log);
    let reparsed = parse_log(&to_jsonl(&folded)).unwrap();
    assert_eq!(replay(&reparsed).elements[0].y, Some(0.25));
}

#[test]
fn move_comment_with_y_only_persists_one_moved_event() {
    let v = json::parse(r#"{"elemId":"E1","kind":"move","y":0.6}"#).unwrap();
    let evs = comment_to_events(&v);
    assert!(
        matches!(&evs[..], [Event::ElementMoved { id, col: None, y: Some(y), .. }]
            if id == "E1" && *y == 0.6),
        "got {evs:?}"
    );
}

#[test]
fn move_comment_with_neither_col_nor_y_is_rejected() {
    let v = json::parse(r#"{"elemId":"E1","kind":"move"}"#).unwrap();
    assert!(
        comment_to_events(&v).is_empty(),
        "a move carrying no target would replay as a no-op"
    );
}

#[test]
fn move_comment_clamps_and_rounds_its_y() {
    // Out-of-band fractions would draw off the lane; float noise would dirty the log.
    for (posted, stored) in [("1.7", 1.0), ("-0.3", 0.0), ("0.333333333333", 0.3333)] {
        let v = json::parse(&format!(r#"{{"elemId":"E1","kind":"move","y":{posted}}}"#)).unwrap();
        let evs = comment_to_events(&v);
        assert!(
            matches!(&evs[..], [Event::ElementMoved { y: Some(y), .. }] if *y == stored),
            "posted {posted}: got {evs:?}"
        );
    }
}

#[test]
fn is_log_path_keys_on_extension() {
    assert!(is_log_path(Path::new("event-log.jsonl")));
    assert!(is_log_path(Path::new("a.log")));
    assert!(!is_log_path(Path::new("model.json")));
}
