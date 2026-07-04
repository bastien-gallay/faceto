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
pub(crate) fn comments_from_log(log: &[events::Event], model: &Model) -> Vec<String> {
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
pub(crate) fn lint_items(model: &Model) -> Vec<String> {
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
