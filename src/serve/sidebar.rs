//! The `/comments` sidebar payload: fold the log's annotations/hotspots and the live lint
//! findings into the JSON the client renders ([`comments_body`]).

use super::Ctx;
use crate::model::Model;
use crate::{events, json};

/// The full `/comments` response body: stored feedback first, then the live lint findings,
/// framed as one JSON array. The lint merge is **best-effort** — if the log doesn't parse, the
/// stored comments still come back on their own: a malformed / half-written log degrades to
/// comments-only (here, empty) rather than hiding the sidebar behind a 500, the resilience the
/// endpoint had before lint was merged in.
///
/// The log is read + replayed **once**: the single projection feeds both the comment fold
/// ([`comments_from_log`]) and the lint pass, so the comment set and the findings always reflect
/// the same snapshot (and the log isn't read/replayed twice per request).
pub(crate) fn comments_body(ctx: &Ctx) -> String {
    let mut items = Vec::new();
    if let Ok(log) = events::read_log(&ctx.model_path) {
        let model = events::replay(&log);
        items.extend(comments_from_log(&log, &model));
        items.extend(lint_items(&model));
    }
    format!("[{}]", items.join(","))
}

/// One sidebar comment item — the `{elemId, kind, text, status:"open"}` JSON string the client
/// renders. The single definition of the sidebar wire-shape, shared by the log projection
/// ([`comments_from_log`]) and the lint merge ([`lint_items`]) so the two lanes can never drift.
fn comment_item(elem_id: &str, kind: &str, text: &str) -> String {
    let obj = json::Json::Obj(vec![
        ("elemId".into(), json::Json::Str(elem_id.to_string())),
        ("kind".into(), json::Json::Str(kind.to_string())),
        ("text".into(), json::Json::Str(text.to_string())),
        ("status".into(), json::Json::Str("open".into())),
    ]);
    json::to_string(&obj)
}

/// Project the log's *feedback* events (annotations, resolutions, renames) back into the
/// comment shape the client sidebar expects. Structural events (adds, moves, edges) are
/// omitted — they already live in the rendered board. Feedback on an element that was later
/// removed is dropped too, so the sidebar never lists a comment for a box that's off the board.
///
/// Takes the already-parsed `log` and its `model` projection ([`comments_body`] reads and replays
/// the source once, then feeds both here and to [`lint_items`]). Returns the item JSON strings, not
/// the joined array.
fn comments_from_log(log: &[events::Event], model: &Model) -> Vec<String> {
    let present: std::collections::HashSet<&str> =
        model.elements.iter().map(|e| e.id.as_str()).collect();
    let mut items: Vec<String> = Vec::new();
    for ev in log {
        let (id, kind, text) = match ev {
            events::Event::ElementAnnotated { id, text } => (id, "comment", text.clone()),
            events::Event::HotspotResolved { id, resolution } => {
                (id, "resolve", resolution.clone())
            }
            events::Event::ElementRenamed { id, label } => (id, "rename", label.clone()),
            _ => continue,
        };
        if !present.contains(id.as_str()) {
            continue;
        }
        items.push(comment_item(id, kind, &text));
    }
    items
}

/// The live lint findings for a board, in the same comment shape the sidebar renders — a
/// `kind:"lint"` entry keyed on the offending element's stable `id`. Computed on read (never
/// persisted): a finding is *derived* from the current graph, so recomputing it each request
/// keeps it always-fresh and can never go stale against an edited board. A finding on an element
/// the reviewer has already **resolved** (a `HotspotResolved` set `resolved:true`) is suppressed
/// — that is the whole "reuse serve→review→resolve" story, keyed on `Finding.element_id` == the
/// same stable id `HotspotResolved.id` uses. Per-finding acknowledgement is F-comment-lifecycle's.
///
/// This resolve-suppression is deliberately serve-only: the `faceto lint` CLI runs `lint()`
/// unfiltered (a full audit reports on resolved elements too). The divergence is intended — the
/// sidebar is the interactive review loop, the CLI is the complete check — and safe, since lint is
/// warn-only (exit 0) at both surfaces, so a suppressed nudge can never gate a build.
fn lint_items(model: &Model) -> Vec<String> {
    // Build the resolved-id set once (O(V)) so the per-finding suppression check is O(1) — the
    // same present-set idiom `comments_from_log` uses, instead of an O(findings × elements) rescan.
    let resolved: std::collections::HashSet<&str> = model
        .elements
        .iter()
        .filter(|e| e.resolved)
        .map(|e| e.id.as_str())
        .collect();
    crate::lint::lint(model)
        .into_iter()
        .filter(|f| !resolved.contains(f.element_id.as_str()))
        .map(|f| comment_item(&f.element_id, "lint", f.message))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events;
    use crate::serve::testutil::*;

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
}
