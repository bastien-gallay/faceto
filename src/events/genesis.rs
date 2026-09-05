//! Derive events from an existing model ([`from_model`], the genesis/migration path) and fold
//! a log to a shorter equivalent snapshot ([`compact`]).

use super::replay::replay;
use super::Event;
use crate::model::Model;

/// Turn an existing model into the genesis batch of events that reconstructs it — the
/// migration and bootstrap path (an old `model.json` becomes the start of a log). A
/// resolved hotspot is replayed as an add followed by its resolution, so its `detail`
/// (the resolution note) round-trips.
pub fn from_model(m: &Model) -> Vec<Event> {
    let mut ev = Vec::new();
    if !m.title.is_empty() {
        ev.push(Event::BoardTitled {
            title: m.title.clone(),
        });
    }
    ev.extend(header(m.format, |f| Event::BoardFormat {
        format: crate::model::format_to_str(f).into(),
    }));
    ev.extend(header(m.level, |l| Event::BoardLeveled {
        level: crate::model::level_to_str(l).into(),
    }));
    for p in &m.phases {
        ev.push(Event::PhaseAdded {
            id: Some(p.id.clone()),
            label: p.label.clone(),
            from_col: p.from_col,
            to_col: p.to_col,
        });
    }
    for e in &m.elements {
        ev.push(Event::ElementAdded {
            id: e.id.clone(),
            kind: e.kind,
            label: e.label.clone(),
            col: e.col,
            detail: if e.resolved { None } else { e.detail.clone() },
            y: e.y,
            links: e.links.clone(),
        });
        if e.resolved {
            ev.push(Event::HotspotResolved {
                id: e.id.clone(),
                resolution: e.detail.clone().unwrap_or_default(),
            });
        }
    }
    for e in &m.edges {
        ev.push(Event::EdgeAdded {
            src: e.src.clone(),
            dst: e.dst.clone(),
            label: e.label.clone(),
        });
    }
    ev
}

/// A board header (`format`, `level`) reaches the log only when it differs from the default, so an
/// ordinary board's genesis batch stays byte-identical to what it was before the field existed. The
/// guard is `!= default`, never a named variant, so a future value is emitted too.
fn header<T: Default + PartialEq>(value: T, event: impl FnOnce(T) -> Event) -> Option<Event> {
    (value != T::default()).then(|| event(value))
}

/// Fold a log down to the shortest sequence that replays to the same board: a `LogCompacted`
/// provenance marker, then the genesis batch of the current projection. This bounds replay
/// length (H1's snapshot escape hatch). It is lossy *by design* — only the projection survives,
/// so the comment **history** is dropped (each element keeps just its latest note, folded into
/// `detail`); the full prior log stays recoverable from version control or a `.bak`.
///
/// `replay(compact(log))` always projects the same `Model` as `replay(log)`, and the genesis
/// tail is a fixed point (compacting again changes only the marker's count).
pub fn compact(events: &[Event]) -> Vec<Event> {
    let model = replay(events);
    let mut out = vec![Event::LogCompacted {
        folded: events.len() as i64,
    }];
    out.extend(from_model(&model));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::testutil::*;
    use crate::events::*;
    use crate::json::{self};
    use proptest::prelude::*;

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

    // F-element-links: an element's `links` and an edge's `label` survive genesis → replay.
    #[test]
    fn from_model_then_replay_preserves_links_and_edge_label() {
        let src = r#"{
            "elements":[
                {"id":"E1","type":"event","label":"Made","links":["https://tix/42","adr://7"]},
                {"id":"E2","type":"command","label":"Do"}
            ],
            "edges":[{"src":"E2","dst":"E1","label":"causes"}]
        }"#;
        let original = crate::model::from_json(&json::parse(src).unwrap());
        let rebuilt = replay(&from_model(&original));

        let e1 = rebuilt.elements.iter().find(|e| e.id == "E1").unwrap();
        assert_eq!(e1.links, vec!["https://tix/42", "adr://7"]);
        assert_eq!(
            rebuilt
                .elements
                .iter()
                .find(|e| e.id == "E2")
                .unwrap()
                .links,
            Vec::<String>::new()
        );
        assert_eq!(rebuilt.edges[0].label.as_deref(), Some("causes"));
    }

    // ---- F-format-tag: the board format round-trips through the log ------------------------

    #[test]
    fn from_model_emits_no_board_format_for_the_default_format() {
        let es = crate::model::from_json(
            &json::parse(r#"{"format":"event-storming","elements":[]}"#).unwrap(),
        );
        assert_eq!(es.format, crate::model::Format::EventStorming);
        assert!(!from_model(&es)
            .iter()
            .any(|e| matches!(e, Event::BoardFormat { .. })));
    }

    #[test]
    fn a_board_format_event_replays_and_survives_compaction() {
        let log = vec![Event::BoardFormat {
            format: "event-storming".into(),
        }];
        assert_eq!(replay(&log).format, crate::model::Format::EventStorming);
        assert_eq!(
            replay(&compact(&log)).format,
            crate::model::Format::EventStorming
        );
    }

    // ---- F-es-lint: the board level round-trips through the log ----------------------------

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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// The load-bearing `compact` invariant: replaying a compacted log projects the *same*
        /// `Model` as replaying the original — `replay(compact(x)) == replay(x)`. Rendering is a
        /// pure function of the `Model`, so equal projections render identically; this is what
        /// lets `compact` fold history without changing the board.
        ///
        /// Compared on the `Model` directly (not through `from_model` → jsonl): the left side never
        /// passes through `from_model`, so a field the genesis emitter *forgets* to carry makes the
        /// two projections diverge and fails here — the exact escape a canonical-form comparison
        /// would share the blind spot on. `a_placed_elements_y_survives_compact` is the worked
        /// example of that failure mode; this generalises it over arbitrary comment histories.
        #[test]
        fn pbt_compact_preserves_the_projection_over_comment_logs(
            comments in prop::collection::vec(comment_strategy(), 1..=8),
        ) {
            let (mut log, _ids) = genesis();
            for v in &comments {
                log.extend(comment_to_events(v));
            }
            prop_assert_eq!(replay(&compact(&log)), replay(&log));
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(800))]

        /// The same `replay(compact(x)) == replay(x)` invariant over phase/region logs — the genesis
        /// path most prone to dropping state (region ids, span geometry). Complements the comment
        /// coverage above, which never mints a phase.
        #[test]
        fn pbt_compact_preserves_the_projection_over_phase_logs(log in phase_log_strategy()) {
            prop_assert_eq!(replay(&compact(&log)), replay(&log));
        }
    }
}
