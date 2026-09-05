//! Translate a posted `POST /comment` payload into the server-minted event it implies —
//! an `add`, a `region-add`, or a `phase-split`, each validated and minted under the lock.

use super::Ctx;
use crate::{events, json};

/// The non-blank `text` label a creation command requires, or `400` — the one place the
/// "a minted element/region must carry a label" rule turns into an HTTP status, shared by every
/// server-minted command so `add`, `region-add`, and `phase-split` can't diverge on it. A blank
/// label would mint a permanent, never-renumbered empty box; the same `nonblank` rule the `rename`
/// guard uses (a direct POST must not slip a blank in even though the client modal guards it).
fn required_label(v: &json::Json) -> Result<String, u16> {
    v.get_str("text").and_then(events::nonblank).ok_or(400u16)
}

/// Handle a `kind:"add"` post: an element-creation command rather than a comment on an
/// existing one. `type` (the lane) and a non-empty `text` (label) are required; optional
/// `col`/`detail`. The server mints the id (H6). Returns the HTTP status to fail with: `400` for
/// a missing/empty type or label, `500` if the append itself fails.
pub(crate) fn add_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    // `type` must name one of the eight lanes. Refused at the boundary rather than dropped the
    // way a log line is: this is a live command with a client to answer, and a 400 is a better
    // answer than a 200 whose sticky never appears.
    let kind = v
        .get_str("type")
        .and_then(crate::model::lane_from_str)
        .ok_or(400u16)?;
    let label = required_label(v)?;
    let col = v.get_i64("col");
    // The lane-title `+` posts `prepend:true` (no col); the server derives the left-edge col so the
    // rule lives in one place and stays consistent under concurrent adds.
    let prepend = v
        .get("prepend")
        .and_then(json::Json::as_bool)
        .unwrap_or(false);
    let detail = v
        .get_str("detail")
        .filter(|s| !s.is_empty())
        .map(String::from);
    ctx.append_add(kind, label, col, detail, prepend)
        .map_err(|_| 500u16)
}

/// Handle a `kind:"region-add"` post: a region-creation command, the region counterpart of
/// `add_from_comment`. A non-empty `text` (label) and a well-ordered `[fromCol, toCol]` span
/// (`events::valid_span`) are required; the server mints the id (review #3 / H6 for regions).
/// Returns the HTTP status to fail with: `400` for a missing label or an absent/inverted/
/// zero-width span, `500` if the append itself fails.
pub(crate) fn add_region_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let label = required_label(v)?;
    let from_col = v.get_i64("fromCol").ok_or(400u16)?;
    let to_col = v.get_i64("toCol").ok_or(400u16)?;
    if !events::valid_span(from_col, to_col) {
        return Err(400u16);
    }
    ctx.append_region_add(label, from_col, to_col)
        .map_err(|_| 500u16)
}

/// Handle a `kind:"phase-split"` post: divide the region `regionId` at `atCol` into two, the
/// server minting the right half's id (F-region-frontiers, the partition's "add"). A non-empty
/// `text` (the new right-half label) and an `atCol` are required; whether the column falls strictly
/// inside the phase is validated under the lock in `append_phase_split` (a stale/out-of-range split
/// is refused before writing). Returns the HTTP status to fail with: `400` for a missing label or
/// atCol, `500` if the append itself fails or the split is out of range.
pub(crate) fn split_region_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let id = v.get_str("regionId").map(str::to_string).ok_or(400u16)?;
    let label = required_label(v)?;
    let at_col = v.get_i64("atCol").ok_or(400u16)?;
    ctx.append_phase_split(id, at_col, label)
        .map_err(|_| 500u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Lane;
    use crate::serve::testutil::*;
    use crate::{events, json};

    #[test]
    fn region_resize_rename_remove_map_to_phase_events() {
        let resize =
            json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":0,"toCol":5}"#)
                .unwrap();
        let evs = events::comment_to_events(&resize);
        assert_eq!(evs.len(), 1);
        assert!(matches!(
            &evs[0],
            events::Event::PhaseResized { id, from_col: 0, to_col: 5 } if id == "K1"
        ));

        let rename =
            json::parse(r#"{"kind":"region-rename","regionId":"K1","text":"Fulfillment"}"#)
                .unwrap();
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
            json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":9,"toCol":2}"#)
                .unwrap();
        assert!(events::comment_to_events(&inverted).is_empty());

        let zero_width =
            json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":3,"toCol":3}"#)
                .unwrap();
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
    fn move_without_a_col_is_rejected() {
        let v = json::parse(r#"{"elemId":"E1","kind":"move"}"#).unwrap();
        assert!(events::comment_to_events(&v).is_empty());
    }

    #[test]
    fn move_ignores_a_self_swap_or_a_swap_missing_its_col() {
        // Self-swap → just the primary move; swapId without swapCol → no phantom partner move.
        let selfswap =
            json::parse(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E1","swapCol":0}"#)
                .unwrap();
        assert_eq!(events::comment_to_events(&selfswap).len(), 1);
        let nocol = json::parse(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E2"}"#).unwrap();
        assert_eq!(events::comment_to_events(&nocol).len(), 1);
    }

    #[test]
    fn move_with_swap_persists_both_stickies() {
        // A move into an occupied column swaps two stickies; both relocations must be logged,
        // else the partner reverts on the next replay and the two overlap.
        let v = json::parse(r#"{"elemId":"E1","kind":"move","col":3,"swapId":"E2","swapCol":1}"#)
            .unwrap();
        let evs = events::comment_to_events(&v);
        assert_eq!(evs.len(), 2);
        assert!(
            matches!(&evs[0], events::Event::ElementMoved { id, col: Some(3), .. } if id == "E1")
        );
        assert!(
            matches!(&evs[1], events::Event::ElementMoved { id, col: Some(1), .. } if id == "E2")
        );
    }

    #[test]
    fn plain_move_is_one_event_and_no_elem_id_is_rejected() {
        let mv = json::parse(r#"{"elemId":"E1","kind":"move","col":2}"#).unwrap();
        assert_eq!(events::comment_to_events(&mv).len(), 1);
        let orphan = json::parse(r#"{"kind":"comment","text":"hi"}"#).unwrap();
        assert!(events::comment_to_events(&orphan).is_empty());
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
        std::fs::write(&path, events::line(&added("E1", Lane::Event)) + "\n").unwrap();
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
}
