//! Fold a legacy `comments.jsonl` into events ([`from_comments`]) and map one posted comment
//! to the events it implies ([`comment_to_events`]) — the single source of truth shared with
//! `serve.rs`'s `POST /comment`.

use super::log::jsonl_records;
use super::Event;
use crate::json::{self, Json};

/// A label with content: the string trimmed, or `None` when it is blank. The one place the
/// "a label must carry content" rule lives — a blank one would mint or rename into a permanent,
/// never-renumbered empty box. Shared by the `add` guard (`serve.rs`) and the `rename` guard in
/// [`comment_to_events`], so direct on-board editing and a raw POST obey the same invariant.
pub fn nonblank(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// A region span with content: `from_col < to_col`, or `false` for an inverted or zero-width
/// span. An inverted span would mint or resize into a region `region_of`'s `from_col <= col &&
/// col <= to_col` test can never match — a permanent, silent gap between the model (which drops
/// the region from every column's membership) and the render, which normalizes the span and
/// draws it as a real band regardless. Shared by the `region-add` guard (`serve.rs`) and the
/// `region-resize` guard in [`region_comment_to_events`], so a raw POST can't create what the
/// other can't.
pub fn valid_span(from_col: i64, to_col: i64) -> bool {
    from_col < to_col
}

/// Normalise a posted vertical sub-position to its stored form: clamped into `[0, 1]` and
/// rounded to 4 decimals so the log carries a clean human-readable number, not a float's full
/// noise. This is the **write-seam** half of the rule; the **read** half — how a stored `y`
/// (or its absence) is interpreted as an ordering key — is `model::y_key`.
pub fn clamp_y(y: f64) -> f64 {
    (y.clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0
}

/// Map one posted/stored comment object to the event(s) it persists — the single source of
/// truth for the comment→event translation, shared by the live server (`POST /comment` in log
/// mode) and the `comments.jsonl` migration ([`from_comments`]). `move`/`resolve`/`rename`/`drop`
/// carry structural intent and fold straight into the projection; `split`/`question`/`comment`
/// stay advisory annotations. A `move` that displaces an occupant — the client sends
/// `swapId`/`swapCol` — yields **two** `ElementMoved`s so the swap round-trips. Returns an empty
/// vec when the comment names no element, when a `move` carries no target col, or when a `rename`
/// carries a blank label (all would replay as no-ops or corrupt the board): the caller treats that
/// as "nothing to persist".
///
/// Region edits (`region-resize`/`region-rename`/`region-remove`) key off `regionId` instead of
/// `elemId` and are dispatched to [`region_comment_to_events`] before the element path runs.
/// `region-add` is **not** handled here — like the element `add`, it needs a server-minted id and
/// is special-cased in `serve.rs` (`add_region_from_comment`).
pub fn comment_to_events(v: &Json) -> Vec<Event> {
    let kind = v.get_str("kind").unwrap_or("comment");
    if matches!(
        kind,
        "region-resize" | "region-rename" | "region-remove" | "frontier-move"
    ) {
        return region_comment_to_events(v, kind);
    }
    let Some(id) = v.get_str("elemId").map(str::to_string) else {
        return Vec::new();
    };
    let text = v.get_str("text").unwrap_or("").to_string();
    match kind {
        "move" => move_events(v, id),
        "resolve" => vec![Event::HotspotResolved {
            id,
            resolution: text,
        }],
        "rename" => match nonblank(&text) {
            Some(label) => vec![Event::ElementRenamed { id, label }],
            None => Vec::new(),
        },
        "drop" => vec![Event::ElementRemoved { id }],
        _ => vec![Event::ElementAnnotated { id, text }],
    }
}

/// A `move` comment → the `ElementMoved`(s) it persists. Relocates along the timeline (`col`)
/// and/or within the lane band (`y`, F-2d-placement); carrying neither would replay as a no-op, so
/// it yields an empty vec rather than a phantom move. A `swapId`/`swapCol` pair (old clients /
/// stashed offline moves — the 2D client no longer swaps) appends a second `ElementMoved` for the
/// displaced sticky, guarded against a self-swap or a swap missing its target col.
fn move_events(v: &Json, id: String) -> Vec<Event> {
    let col = v.get_i64("col");
    let y = v
        .get("y")
        .and_then(Json::as_f64)
        .filter(|y| y.is_finite())
        .map(clamp_y);
    if col.is_none() && y.is_none() {
        return Vec::new();
    }
    let mut evs = vec![Event::ElementMoved {
        id: id.clone(),
        col,
        kind: None,
        y,
    }];
    if let (Some(swap_id), Some(swap_col)) = (v.get_str("swapId"), v.get_i64("swapCol")) {
        if swap_id != id.as_str() {
            evs.push(Event::ElementMoved {
                id: swap_id.to_string(),
                col: Some(swap_col),
                kind: None,
                y: None,
            });
        }
    }
    evs
}

/// The region half of [`comment_to_events`]: `region-resize`/`region-rename`/`region-remove`,
/// keyed by `regionId` rather than `elemId` (a region is not an element). Returns an empty vec
/// when the comment names no region, when a `region-resize` carries no `[fromCol, toCol]` span or
/// an inverted/zero-width one (`valid_span`), or when a `region-rename` carries a blank label —
/// same no-op guards as the element path, so a malformed post never replays as a phantom edit.
fn region_comment_to_events(v: &Json, kind: &str) -> Vec<Event> {
    let Some(id) = v.get_str("regionId").map(str::to_string) else {
        return Vec::new();
    };
    match kind {
        // Legacy independent-span resize (old clients / stashed offline / `comments.jsonl`
        // migration). The live client posts `frontier-move` instead; either way `normalize`
        // projects the result onto a contiguous partition.
        "region-resize" => match (v.get_i64("fromCol"), v.get_i64("toCol")) {
            (Some(from_col), Some(to_col)) if valid_span(from_col, to_col) => {
                vec![Event::PhaseResized {
                    id,
                    from_col,
                    to_col,
                }]
            }
            _ => Vec::new(),
        },
        // Move one frontier (F-region-frontiers resize): set the named border, `replay`'s
        // `normalize` re-borders the neighbour. A missing/unknown `edge` or `col` is a no-op —
        // same "nothing to persist" guard as the element path.
        "frontier-move" => match (v.get_str("edge"), v.get_i64("col")) {
            (Some(edge), Some(col)) if edge == "start" || edge == "end" => {
                vec![Event::FrontierMoved {
                    id,
                    edge: edge.to_string(),
                    col,
                }]
            }
            _ => Vec::new(),
        },
        "region-rename" => {
            let text = v.get_str("text").unwrap_or("");
            match nonblank(text) {
                Some(label) => vec![Event::PhaseRenamed { id, label }],
                None => Vec::new(),
            }
        }
        "region-remove" => vec![Event::PhaseRemoved { id }],
        _ => Vec::new(),
    }
}

/// Fold a legacy `comments.jsonl` into the events it represents — the answer to H5, the second
/// half of the migration story alongside [`from_model`](crate::events::from_model). Each non-blank
/// line is one stored comment;
/// [`comment_to_events`] translates it. Unlike the log proper, the comments inbox was always a
/// *best-effort* sidecar, so a line that cannot be migrated is **skipped** (not a hard error) —
/// migrating disposable feedback must not abort on one stray line. Append the result after a
/// model's genesis batch: the batch mints the ids these comments reference, so replaying the two
/// together reconstructs the board *and* its annotations/resolutions/renames.
///
/// Returns the events **and the count of non-blank lines that produced none** — unparseable, not
/// an object, naming no element, or a kind that carries no board change (e.g. a legacy `add`, which
/// in non-log mode was only ever an inbox note and carries no `elemId` to attach to). The count
/// lets the caller report the loss instead of dropping those lines silently.
pub fn from_comments(text: &str) -> (Vec<Event>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for (_, line) in jsonl_records(text) {
        match json::parse(line) {
            Ok(v @ Json::Obj(_)) => {
                let evs = comment_to_events(&v);
                if evs.is_empty() {
                    skipped += 1;
                } else {
                    out.extend(evs);
                }
            }
            _ => skipped += 1,
        }
    }
    (out, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::testutil::*;
    use crate::events::*;
    use crate::json::{self};

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
        let v =
            json::parse(r#"{"elemId":"E1","kind":"rename","text":"  PaymentTaken  "}"#).unwrap();
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
            let v =
                json::parse(&format!(r#"{{"elemId":"E1","kind":"move","y":{posted}}}"#)).unwrap();
            let evs = comment_to_events(&v);
            assert!(
                matches!(&evs[..], [Event::ElementMoved { y: Some(y), .. }] if *y == stored),
                "posted {posted}: got {evs:?}"
            );
        }
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
}
