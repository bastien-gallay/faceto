//! Log-file IO and framing: recognise a log path and read/parse `event-log.jsonl` into
//! [`Event`]s, distinguishing a malformed known event (hard error) from an unknown kind (skip).

use super::codec::parse_event;
use super::replay::replay;
use super::Event;
use crate::json::{self, Json};
use crate::model::Model;
use std::path::Path;

/// Is this path an event log (vs. a legacy `model.json`)? Chosen by extension so the same
/// CLI verbs accept either source during the migration.
pub fn is_log_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("jsonl") | Some("log")
    )
}

/// Read + replay a log file into a `Model`.
pub fn load(path: &Path) -> Result<Model, String> {
    Ok(replay(&read_log(path)?))
}

/// Read a log file into its events (file order = causal order).
pub fn read_log(path: &Path) -> Result<Vec<Event>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    parse_log(&text)
}

/// Iterate the meaningful records of JSONL text: each non-blank line, trimmed, paired with its
/// 1-based line number. The single place the line grammar (skip blanks, trim) lives, shared by
/// the log reader ([`parse_log`]) and the comments fold ([`from_comments`]) so the two never drift.
pub(crate) fn jsonl_records(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty())
}

/// Parse JSONL text into events. Blank lines are skipped; a line that does not parse as
/// JSON is a hard error (the log is the source of truth); a well-formed object whose
/// `"event"` is unknown is skipped (forward compatibility across schema versions).
pub fn parse_log(text: &str) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    for (n, line) in jsonl_records(text) {
        let j = json::parse(line).map_err(|e| format!("event-log line {}: {}", n, e))?;
        match parse_event(&j) {
            Some(ev) => events.push(ev),
            // `parse_event` returns `None` for two very different cases; only one is skippable.
            // An *unknown* kind is a future/other-tool event → skip (forward compatibility). A
            // *known* kind that still didn't build means a required field is missing or mis-typed
            // (e.g. a numeric `id`, an absent `fromCol`): the fact is in the append-only truth but
            // would silently vanish from the projection, so it is a hard error, like a line that
            // isn't valid JSON at all.
            None => {
                if let Some(kind) = j.get("event").and_then(Json::as_str) {
                    if is_known_kind(kind) {
                        return Err(format!(
                            "event-log line {}: {} event with a missing or mis-typed required field",
                            n, kind
                        ));
                    }
                }
            }
        }
    }
    Ok(events)
}

/// The event kinds this build understands — the current schema plus the legacy aliases [`upcast`]
/// migrates forward. Distinguishes a *malformed known event* (in this set, but [`parse_event`]
/// couldn't build it → hard error) from a *future/unknown kind* (outside it → skipped). Must list
/// exactly the kinds `parse_event` matches (plus upcast's aliases); kept adjacent so the two are
/// edited together whenever a variant is added.
fn is_known_kind(kind: &str) -> bool {
    matches!(
        kind,
        "BoardTitled"
            | "BoardLeveled"
            | "PhaseAdded"
            | "PhaseResized"
            | "PhaseRenamed"
            | "PhaseRemoved"
            | "FrontierMoved"
            | "PhaseSplit"
            | "ElementAdded"
            | "ElementRenamed"
            | "ElementMoved"
            | "ElementAnnotated"
            | "HotspotResolved"
            | "ElementRemoved"
            | "EdgeAdded"
            | "EdgeRemoved"
            | "LogCompacted"
            // legacy aliases upcast() rewrites to a current kind
            | "CommentAdded"
            | "Comment"
    )
}
