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
///
/// Two things stop the read rather than shrinking the board silently (F-format-tag):
/// a `BoardFormat` naming a format this build cannot project, and a log with records but **no**
/// recognised event at all. The second is where forward compatibility and format discrimination
/// are the same mechanism pointed in opposite directions: skipping unknown kinds is exactly how an
/// older faceto reads a newer log, and it is also how a *foreign format's* log reads as an empty
/// event-storming board. A log with some recognised events keeps the lenient reading; a log with
/// none of them has told us nothing we can project, so it is a diagnostic, not a blank canvas.
pub fn parse_log(text: &str) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut skipped = 0usize;
    for (n, line) in jsonl_records(text) {
        let j = json::parse(line).map_err(|e| format!("event-log line {}: {}", n, e))?;
        match parse_event(&j) {
            Some(ev) => {
                if let Event::BoardFormat { format } = &ev {
                    crate::model::format_declared(Some(format))
                        .map_err(|e| format!("event-log line {}: {}", n, e))?;
                }
                events.push(ev)
            }
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
                skipped += 1;
            }
        }
    }
    if events.is_empty() && skipped > 0 {
        return Err(format!(
            "event-log: {} record(s), none of a recognised event kind — this log is from another \
             board format, or from a newer faceto",
            skipped
        ));
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_log_errors_on_a_malformed_known_event() {
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
    fn a_log_of_nothing_but_unknown_kinds_is_an_error_not_an_empty_board() {
        // The defect F-format-tag exists to close (spike #114): a log from another board format is
        // all-unknown-kinds, so the lenient read projected it as a silently empty ES board.
        let err = parse_log(
            "{\"event\":\"CanvasNamed\",\"name\":\"Billing\"}\n\
             {\"event\":\"SlotFilled\",\"slot\":\"ubiquitous-language\"}\n",
        )
        .unwrap_err();
        assert!(err.contains("2 record(s)"), "{}", err);
        assert!(err.contains("none of a recognised event kind"), "{}", err);
    }

    #[test]
    fn one_recognised_event_keeps_the_lenient_forward_compatible_read() {
        // The distinction the error above must not blur: a log carrying *some* future events is an
        // older faceto reading a newer log, which is the whole point of skipping unknown kinds.
        let log = parse_log(
            "{\"event\":\"ElementAdded\",\"id\":\"E1\",\"type\":\"event\",\"label\":\"A\"}\n\
             {\"event\":\"FromTheFuture\",\"x\":1}\n",
        )
        .unwrap();
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn a_board_format_this_build_cannot_project_is_a_hard_error() {
        let err =
            parse_log(r#"{"event":"BoardFormat","format":"bounded-context-canvas"}"#).unwrap_err();
        assert!(err.contains("bounded-context-canvas"), "{}", err);
        assert!(err.contains("line 1"), "{}", err);
    }

    #[test]
    fn an_explicit_event_storming_format_tag_reads_as_one_event() {
        let log = parse_log(r#"{"event":"BoardFormat","format":"event-storming"}"#).unwrap();
        assert_eq!(
            log,
            vec![Event::BoardFormat {
                format: "event-storming".into()
            }]
        );
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
