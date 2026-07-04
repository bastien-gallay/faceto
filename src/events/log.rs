//! Log-file IO and framing: recognise a log path and read/parse `event-log.jsonl` into
//! [`Event`]s, distinguishing a malformed known event (hard error) from an unknown kind (skip).

use super::codec::{is_known_kind, parse_event};
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
/// 1-based line number. The single place the log's line grammar (skip blanks, trim) lives, used by
/// the log reader ([`parse_log`]).
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
    fn is_log_path_keys_on_extension() {
        assert!(is_log_path(Path::new("event-log.jsonl")));
        assert!(is_log_path(Path::new("a.log")));
        assert!(!is_log_path(Path::new("model.json")));
    }
}
