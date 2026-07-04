//! The live server's test suite. Kept as one co-located module: the cases share a
//! temp-log + `Ctx` harness and exercise the append -> replay -> render request path
//! transversally across routing, id minting, and the comment/lint sidebar.

use super::comment::{add_from_comment, add_region_from_comment};
use super::http::parse_collapse;
use super::sidebar::{comments_body, comments_from_log, lint_items};
use super::*;
use crate::json;

#[test]
fn parse_collapse_splits_ids_and_treats_empty_as_the_identity_set() {
    assert_eq!(parse_collapse("collapse=K2,K5"), vec!["K2", "K5"]);
    assert_eq!(parse_collapse("base=abc&collapse=K2"), vec!["K2"]);
    // Absent key, an empty value, and stray empty segments all fold to the empty (identity) set.
    assert!(parse_collapse("base=abc").is_empty());
    assert!(parse_collapse("collapse=").is_empty());
    assert_eq!(parse_collapse("collapse=,K2,"), vec!["K2"]);
}

#[test]
fn fnv12_is_deterministic_and_twelve_hex_chars() {
    // FNV-1a offset basis, for empty input.
    assert_eq!(fnv12(b""), "cbf29ce48422");
    let h = fnv12(b"faceto");
    assert_eq!(h.len(), 12);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(fnv12(b"faceto"), h);
    assert_ne!(fnv12(b"faceto"), fnv12(b"faceto "));
}

#[test]
fn concurrent_appends_never_interleave() {
    // H4: many threads append to one log through a shared Ctx; every line must land
    // whole and intact, with the expected total count and no torn/merged lines.
    let path = std::env::temp_dir().join(format!("faceto-h4-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let ctx = Arc::new(Ctx::new(path.clone()));

    const THREADS: usize = 8;
    const PER_THREAD: usize = 50;
    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let ctx = Arc::clone(&ctx);
            let path = path.clone();
            thread::spawn(move || {
                for i in 0..PER_THREAD {
                    // A long payload makes a torn write easy to detect if the lock fails.
                    let line = format!("t{t}-i{i}-{}", "x".repeat(200));
                    ctx.append_line(&path, &line).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    let contents = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), THREADS * PER_THREAD);
    // Every line is whole: matches the exact shape we wrote, nothing spliced.
    for line in &lines {
        assert!(
            line.starts_with('t') && line.ends_with(&"x".repeat(200)),
            "torn line: {line}"
        );
    }
}

fn added(id: &str, kind: &str) -> events::Event {
    events::Event::ElementAdded {
        id: id.into(),
        kind: kind.into(),
        label: id.into(),
        col: None,
        detail: None,
        y: None,
    }
}

#[test]
fn mint_id_picks_next_free_suffix_per_lane() {
    // H6: ids are type-prefixed and never renumbered — minting takes one past the
    // highest suffix already used under that prefix, independently per lane.
    let log = [
        added("E1", "event"),
        added("E3", "event"),
        added("C1", "command"),
    ];
    assert_eq!(mint_id("event", &log), "E4"); // past the highest E, not filling the E2 gap
    assert_eq!(mint_id("command", &log), "C2");
    assert_eq!(mint_id("hotspot", &log), "H1"); // empty lane starts at 1
    assert_eq!(mint_id("actor", &log), "X1"); // actor stamps X, not A
    assert_eq!(mint_id("aggregate", &log), "A1");
}

#[test]
fn mint_id_does_not_reuse_a_removed_id() {
    // A dropped element's ElementAdded stays in the log (until compaction), so its id must
    // stay reserved — re-minting it would alias leftover events (e.g. its annotations).
    let log = [
        added("E1", "event"),
        added("E2", "event"),
        events::Event::ElementRemoved { id: "E2".into() },
    ];
    assert_eq!(mint_id("event", &log), "E3");
}

fn region_added(id: &str, from_col: i64, to_col: i64) -> events::Event {
    events::Event::PhaseAdded {
        id: Some(id.into()),
        label: id.into(),
        from_col,
        to_col,
    }
}

#[test]
fn mint_region_id_picks_next_free_k_suffix() {
    let log = [region_added("K1", 0, 2), region_added("K3", 3, 5)];
    assert_eq!(mint_region_id(&log), "K4"); // past the highest K, not filling the K2 gap
    assert_eq!(mint_region_id(&[]), "K1"); // empty log starts at 1
}

#[test]
fn mint_region_id_does_not_reuse_a_removed_id() {
    let log = [
        region_added("K1", 0, 2),
        region_added("K2", 3, 5),
        events::Event::PhaseRemoved { id: "K2".into() },
    ];
    assert_eq!(mint_region_id(&log), "K3");
}

#[test]
fn mint_region_id_shares_the_namespace_with_replays_synthetic_ids() {
    // Review #3: a legacy id-less PhaseAdded replays to a synthetic K<n> (resolve_region_id).
    // The mint must reserve that suffix too, or a fresh region could collide with one replay
    // would later synthesize for the same log.
    let log = [events::Event::PhaseAdded {
        id: None,
        label: "Legacy".into(),
        from_col: 0,
        to_col: 2,
    }];
    assert_eq!(mint_region_id(&log), "K2"); // K1 is reserved for the legacy band
    let model = events::replay(&log);
    assert_eq!(model.phases[0].id, "K1");
}

#[test]
fn append_region_add_mints_persists_and_replays() {
    let path = std::env::temp_dir().join(format!("faceto-region-h6-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, events::line(&region_added("K1", 0, 2)) + "\n").unwrap();
    let ctx = Ctx::new(path.clone());

    let ev = ctx.append_region_add("Checkout".into(), 3, 6).unwrap();
    assert!(matches!(&ev, events::Event::PhaseAdded { id: Some(id), .. } if id == "K2"));
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let model = events::replay(&events::parse_log(&text).unwrap());
    let k2 = model.phases.iter().find(|p| p.id == "K2").unwrap();
    assert_eq!(k2.label, "Checkout");
    assert_eq!((k2.from_col, k2.to_col), (3, 6));
}

#[test]
fn mint_region_id_reserves_split_ids() {
    // A split's minted right-half id lives in the same namespace; the next mint must skip it.
    let log = [
        region_added("K1", 0, 5),
        events::Event::PhaseSplit {
            id: "K1".into(),
            at_col: 3,
            new_id: "K2".into(),
            new_label: "Right".into(),
        },
    ];
    assert_eq!(mint_region_id(&log), "K3", "K2 is spent by the split");
}

#[test]
fn append_phase_split_mints_the_right_half_and_replays_to_a_partition() {
    let path = std::env::temp_dir().join(format!("faceto-split-h6-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, events::line(&region_added("K1", 0, 5)) + "\n").unwrap();
    let ctx = Ctx::new(path.clone());

    let ev = ctx
        .append_phase_split("K1".into(), 3, "Right".into())
        .unwrap();
    assert!(matches!(
        &ev,
        events::Event::PhaseSplit { id, at_col: 3, new_id, new_label }
            if id == "K1" && new_id == "K2" && new_label == "Right"
    ));
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let model = events::replay(&events::parse_log(&text).unwrap());
    let spans: Vec<_> = model
        .phases
        .iter()
        .map(|p| (p.id.as_str(), p.from_col, p.to_col))
        .collect();
    assert_eq!(
        spans,
        vec![("K1", 0, 2), ("K2", 3, 5)],
        "split carves K1 in two, contiguous partition preserved"
    );
}

#[test]
fn append_phase_split_rejects_an_out_of_range_split_without_writing() {
    // Review #2: a stale/out-of-range split (atCol not strictly inside the target phase) must
    // Err *before* writing — no dead event in the append-only log, no burned region id, no false
    // success. Here atCol=9 is past K1[0,5]'s to_col.
    let path = std::env::temp_dir().join(format!("faceto-split-oor-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let seed = events::line(&region_added("K1", 0, 5)) + "\n";
    std::fs::write(&path, &seed).unwrap();
    let ctx = Ctx::new(path.clone());

    assert!(
        ctx.append_phase_split("K1".into(), 9, "Right".into())
            .is_err(),
        "out-of-range split is rejected"
    );
    let after = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(after, seed, "nothing was appended");
    // The next mint is still K2 — no id was burned by the rejected split.
    assert_eq!(mint_region_id(&events::parse_log(&after).unwrap()), "K2");
}

#[test]
fn region_resize_rename_remove_map_to_phase_events() {
    let resize =
        json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":0,"toCol":5}"#).unwrap();
    let evs = events::comment_to_events(&resize);
    assert_eq!(evs.len(), 1);
    assert!(matches!(
        &evs[0],
        events::Event::PhaseResized { id, from_col: 0, to_col: 5 } if id == "K1"
    ));

    let rename =
        json::parse(r#"{"kind":"region-rename","regionId":"K1","text":"Fulfillment"}"#).unwrap();
    let evs = events::comment_to_events(&rename);
    assert_eq!(evs.len(), 1);
    assert!(
        matches!(&evs[0], events::Event::PhaseRenamed { id, label } if id == "K1" && label == "Fulfillment")
    );

    let remove = json::parse(r#"{"kind":"region-remove","regionId":"K1"}"#).unwrap();
    let evs = events::comment_to_events(&remove);
    assert_eq!(evs.len(), 1);
    assert!(matches!(&evs[0], events::Event::PhaseRemoved { id } if id == "K1"));
}

#[test]
fn region_edits_with_missing_data_are_rejected() {
    let no_span = json::parse(r#"{"kind":"region-resize","regionId":"K1"}"#).unwrap();
    assert!(events::comment_to_events(&no_span).is_empty());

    let blank_rename =
        json::parse(r#"{"kind":"region-rename","regionId":"K1","text":"   "}"#).unwrap();
    assert!(events::comment_to_events(&blank_rename).is_empty());

    let no_region = json::parse(r#"{"kind":"region-remove"}"#).unwrap();
    assert!(events::comment_to_events(&no_region).is_empty());
}

#[test]
fn region_resize_rejects_an_inverted_or_zero_width_span() {
    // A resize into fromCol >= toCol would make region_of's `from_col <= col <= to_col`
    // test unsatisfiable for any col, silently dropping the region from every column's
    // membership while render still draws a (normalized) visible band for it.
    let inverted =
        json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":9,"toCol":2}"#).unwrap();
    assert!(events::comment_to_events(&inverted).is_empty());

    let zero_width =
        json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":3,"toCol":3}"#).unwrap();
    assert!(events::comment_to_events(&zero_width).is_empty());
}

#[test]
fn add_region_from_comment_rejects_an_inverted_or_zero_width_span() {
    let ctx = Ctx::new(std::env::temp_dir().join("faceto-nonexistent-region.jsonl"));
    // The span check runs before any file access, same as the label check — no file needed.
    let inverted =
        json::parse(r#"{"kind":"region-add","text":"X","fromCol":5,"toCol":2}"#).unwrap();
    assert_eq!(add_region_from_comment(&ctx, &inverted), Err(400));

    let zero_width =
        json::parse(r#"{"kind":"region-add","text":"X","fromCol":4,"toCol":4}"#).unwrap();
    assert_eq!(add_region_from_comment(&ctx, &zero_width), Err(400));
}

#[test]
fn drop_maps_to_element_removed() {
    let v = json::parse(r#"{"elemId":"E2","kind":"drop","text":"never happened"}"#).unwrap();
    let evs = events::comment_to_events(&v);
    assert_eq!(evs.len(), 1);
    assert!(matches!(&evs[0], events::Event::ElementRemoved { id } if id == "E2"));
}

#[test]
fn mint_id_saturates_instead_of_overflowing() {
    // A hand-edited log with a suffix at u32::MAX must not panic/wrap.
    let log = [added("E4294967295", "event")];
    assert_eq!(mint_id("event", &log), "E4294967295");
}

#[test]
fn move_without_a_col_is_rejected() {
    let v = json::parse(r#"{"elemId":"E1","kind":"move"}"#).unwrap();
    assert!(events::comment_to_events(&v).is_empty());
}

#[test]
fn move_ignores_a_self_swap_or_a_swap_missing_its_col() {
    // Self-swap → just the primary move; swapId without swapCol → no phantom partner move.
    let selfswap =
        json::parse(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E1","swapCol":0}"#).unwrap();
    assert_eq!(events::comment_to_events(&selfswap).len(), 1);
    let nocol = json::parse(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E2"}"#).unwrap();
    assert_eq!(events::comment_to_events(&nocol).len(), 1);
}

#[test]
fn comments_from_log_skips_a_removed_elements_feedback() {
    // Annotate E2, then drop it — its comment must not surface for a box off the board.
    let log = [
        added("E2", "event"),
        events::Event::ElementAnnotated {
            id: "E2".into(),
            text: "is this right?".into(),
        },
        events::Event::ElementRemoved { id: "E2".into() },
    ];
    let model = events::replay(&log);
    assert!(
        comments_from_log(&log, &model).is_empty(),
        "feedback on a removed element is dropped"
    );
}

// ---- F-es-lint: lint findings merged into the sidebar (derived on read) ----------------

fn model_of(src: &str) -> Model {
    model::from_json(&json::parse(src).unwrap())
}

#[test]
fn lint_items_surfaces_a_finding_as_a_lint_kind_comment() {
    // An orphan event (no producer, no consumer) yields two lint entries, both keyed on E1.
    let m = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"Lonely","col":0}]}"#);
    let items = lint_items(&m);
    assert_eq!(items.len(), 2);
    for item in &items {
        assert!(item.contains(r#""kind":"lint""#));
        assert!(item.contains(r#""elemId":"E1""#));
        assert!(item.contains(r#""status":"open""#));
    }
}

#[test]
fn lint_items_is_empty_for_a_grammar_clean_board() {
    let m = model_of(
        r#"{"elements":[
            {"id":"C1","type":"command","label":"do","col":0},
            {"id":"E1","type":"event","label":"Done","col":1},
            {"id":"R1","type":"readmodel","label":"view","col":2}],
          "edges":[["C1","E1"],["E1","R1"]]}"#,
    );
    assert!(lint_items(&m).is_empty());
}

#[test]
fn a_finding_on_a_resolved_element_is_suppressed() {
    // Reuse the existing resolve path: once E1 carries resolved:true its findings drop out —
    // no new endpoint, just the HotspotResolved-driven `resolved` flag the model already has.
    let src = r#"{"elements":[{"id":"E1","type":"event","label":"Lonely","col":0RESOLVED}]}"#;
    let live = model_of(&src.replace("RESOLVED", ""));
    assert_eq!(
        lint_items(&live).len(),
        2,
        "an unresolved orphan still nudges"
    );
    let resolved = model_of(&src.replace("RESOLVED", r#","resolved":true"#));
    assert!(
        lint_items(&resolved).is_empty(),
        "a resolved element's findings are suppressed"
    );
}

#[test]
fn a_design_board_surfaces_the_command_rule_through_lint_items() {
    // The merge honours the board's level for free: lint_items reads model.level.
    let m = model_of(
        r#"{"level":"design","elements":[
            {"id":"C1","type":"command","label":"orphan","col":0}]}"#,
    );
    let items = lint_items(&m);
    assert_eq!(items.len(), 1);
    assert!(items[0].contains(r#""elemId":"C1""#) && items[0].contains(r#""kind":"lint""#));
}

#[test]
fn comments_body_degrades_to_comments_only_on_a_malformed_source() {
    // A corrupt log must not 500 the sidebar: comments_body still returns a valid (here empty)
    // JSON array instead of failing, so a malformed source can't hide the stored comments.
    let path = std::env::temp_dir().join(format!("faceto-cb-bad-{}.jsonl", std::process::id()));
    std::fs::write(&path, "not json at all\n").unwrap();
    let ctx = Ctx::new(path.clone());
    let body = comments_body(&ctx);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        body, "[]",
        "a malformed log degrades to comments-only, never a 500"
    );
}

#[test]
fn comments_body_merges_lint_findings_when_the_board_parses() {
    // A valid log with an orphan event: no stored comments, two lint nudges, framed as an array.
    let path = std::env::temp_dir().join(format!("faceto-cb-ok-{}.jsonl", std::process::id()));
    std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
    let ctx = Ctx::new(path.clone());
    let body = comments_body(&ctx);
    let _ = std::fs::remove_file(&path);
    assert!(body.starts_with('[') && body.ends_with(']'));
    assert_eq!(body.matches(r#""kind":"lint""#).count(), 2);
}

#[test]
fn append_add_mints_persists_and_replays() {
    // The minted id round-trips: append_add writes an ElementAdded that replay folds
    // back into a real element, and a second add under the same lane increments.
    let path = std::env::temp_dir().join(format!("faceto-h6-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
    let ctx = Ctx::new(path.clone());

    let ev = ctx
        .append_add("event", "DayStarted".into(), Some(2), None, false)
        .unwrap();
    assert!(matches!(&ev, events::Event::ElementAdded { id, .. } if id == "E2"));
    let ev2 = ctx
        .append_add("command", "start".into(), None, None, false)
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let model = events::replay(&events::parse_log(&text).unwrap());
    let e2 = model.elements.iter().find(|e| e.id == "E2").unwrap();
    assert_eq!(e2.label, "DayStarted");
    assert_eq!(e2.col, Some(2));
    assert!(matches!(&ev2, events::Event::ElementAdded { id, .. } if id == "C1"));
}

#[test]
fn move_with_swap_persists_both_stickies() {
    // A move into an occupied column swaps two stickies; both relocations must be logged,
    // else the partner reverts on the next replay and the two overlap.
    let v =
        json::parse(r#"{"elemId":"E1","kind":"move","col":3,"swapId":"E2","swapCol":1}"#).unwrap();
    let evs = events::comment_to_events(&v);
    assert_eq!(evs.len(), 2);
    assert!(matches!(&evs[0], events::Event::ElementMoved { id, col: Some(3), .. } if id == "E1"));
    assert!(matches!(&evs[1], events::Event::ElementMoved { id, col: Some(1), .. } if id == "E2"));
}

#[test]
fn plain_move_is_one_event_and_no_elem_id_is_rejected() {
    let mv = json::parse(r#"{"elemId":"E1","kind":"move","col":2}"#).unwrap();
    assert_eq!(events::comment_to_events(&mv).len(), 1);
    let orphan = json::parse(r#"{"kind":"comment","text":"hi"}"#).unwrap();
    assert!(events::comment_to_events(&orphan).is_empty());
}

#[test]
fn append_add_errors_on_a_corrupt_log_rather_than_minting_from_empty() {
    // A malformed log must fail the add — not fold to an empty model and re-mint E1.
    let path = std::env::temp_dir().join(format!("faceto-corrupt-{}.jsonl", std::process::id()));
    std::fs::write(&path, "{ this is not json\n").unwrap();
    let ctx = Ctx::new(path.clone());
    let r = ctx.append_add("event", "X".into(), None, None, false);
    let _ = std::fs::remove_file(&path);
    assert!(r.is_err());
}

#[test]
fn add_with_a_blank_label_is_rejected() {
    // The label check fires before any file access, so a bare Ctx is enough.
    let ctx = Ctx::new(std::env::temp_dir().join("faceto-nonexistent.jsonl"));
    let v = json::parse(r#"{"kind":"add","type":"event","text":"   "}"#).unwrap();
    assert_eq!(add_from_comment(&ctx, &v), Err(400));

    // An off-grammar type would mint into a real lane's id space — reject it too.
    let off = json::parse(r#"{"kind":"add","type":"epic","text":"Saga"}"#).unwrap();
    assert_eq!(add_from_comment(&ctx, &off), Err(400));
}

#[test]
fn blank_rename_appends_nothing_but_a_real_one_persists() {
    // Integration over the log-mode POST /comment path: a comment is mapped to events exactly
    // as `handle` does, and only a non-empty block is appended. A blank inline rename must
    // leave the log byte-for-byte unchanged; a real one appends one ElementRenamed that
    // replays into the new label.
    let path = std::env::temp_dir().join(format!("faceto-rn-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
    let ctx = Ctx::new(path.clone());
    let before = std::fs::read_to_string(&path).unwrap();

    // Blank rename → empty event vec → the handler appends nothing.
    let blank = json::parse(r#"{"elemId":"E1","kind":"rename","text":"   "}"#).unwrap();
    let evs = events::comment_to_events(&blank);
    if !evs.is_empty() {
        let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
        ctx.append_line(&ctx.model_path, &block).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        before,
        "a blank inline rename must not touch the log"
    );

    // Real rename → one ElementRenamed → replays to the new label.
    let real = json::parse(r#"{"elemId":"E1","kind":"rename","text":"Reborn"}"#).unwrap();
    let evs = events::comment_to_events(&real);
    let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
    ctx.append_line(&ctx.model_path, &block).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let model = events::replay(&events::parse_log(&text).unwrap());
    let e1 = model.elements.iter().find(|e| e.id == "E1").unwrap();
    assert_eq!(e1.label, "Reborn");
}
