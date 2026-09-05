//! Log-file IO and framing: recognise a log path and read/parse `event-log.jsonl` into
//! [`Event`]s, distinguishing a malformed known event (hard error) from an unknown kind (skip).

use super::codec::{parse_event, Rejected};
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

/// Parse JSONL text into events. [`Rejected`] carries the per-line rule; the read-outcome table in
/// `docs/src/reference/event-log.md` is its user-facing form.
///
/// The one rule that is not per-line: a log with records but **not one** recognised kind stops the
/// read. Skipping unknown kinds is how an older faceto reads a newer log, and — pointed the other
/// way — how a *foreign format's* log would read as an empty event-storming board. Nothing in a
/// single line separates the two, so the count decides.
pub fn parse_log(text: &str) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut foreign = 0usize;
    for (n, line) in jsonl_records(text) {
        let j = json::parse(line).map_err(|e| format!("event-log line {}: {}", n, e))?;
        match parse_event(&j) {
            Ok(ev) => {
                if let Event::BoardFormat { format } = &ev {
                    crate::model::format_declared(Some(format))
                        .map_err(|e| format!("event-log line {}: {}", n, e))?;
                }
                events.push(ev)
            }
            Err(Rejected::UnknownKind) | Err(Rejected::UnknownLane) => foreign += 1,
            Err(Rejected::Unnamed) => {}
            Err(Rejected::Malformed) => {
                let kind = j.get("event").and_then(Json::as_str).unwrap_or("?");
                return Err(format!(
                    "event-log line {}: {} event with a missing or mis-typed required field",
                    n, kind
                ));
            }
        }
    }
    if events.is_empty() && foreign > 0 {
        return Err(format!(
            "event-log: {} record(s), none of a recognised event kind — this log is from another \
             board format, or from a newer faceto",
            foreign
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
        let log = parse_log(
            "{\"event\":\"ElementAdded\",\"id\":\"E1\",\"type\":\"event\",\"label\":\"A\"}\n\
             {\"event\":\"FromTheFuture\",\"x\":1}\n",
        )
        .unwrap();
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn a_record_naming_no_kind_is_not_evidence_of_a_foreign_format() {
        // A typo'd key is a broken line, not another notation — sending the reader after a format
        // problem would be a worse diagnostic than the silence it replaced.
        assert!(parse_log(r#"{"evnet":"BoardTitled","title":"x"}"#)
            .unwrap()
            .is_empty());
        assert!(parse_log("[1,2]").unwrap().is_empty());
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
