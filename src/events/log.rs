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
    Ok(read_log_full(path)?.events)
}

/// [`read_log`], keeping the unprojected-record counts — the read `compact` must use.
pub fn read_log_full(path: &Path) -> Result<LogRead, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    parse_log_full(&text)
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
    Ok(parse_log_full(text)?.events)
}

/// A log read, plus what it could not project. Folding a log rewrites it from the projection, so
/// a dropped record would be **deleted from append-only truth** — the two counts are what `compact`
/// refuses on. Every other caller only renders, and can ignore them.
///
/// They are separate because the remedies are opposite: `unread` waits for a build that knows the
/// schema, `corrupt` waits for a human to repair the line.
pub struct LogRead {
    pub events: Vec<Event>,
    /// Records this build cannot project but a newer faceto could: an unknown kind, or a lane
    /// outside this build's grammar.
    pub unread: usize,
    /// Records no build will ever project: a line naming no event kind.
    pub corrupt: usize,
}

/// [`parse_log`], keeping the counts of what it could not project.
pub fn parse_log_full(text: &str) -> Result<LogRead, String> {
    let mut events = Vec::new();
    let mut records = 0usize;
    let mut foreign = 0usize;
    let mut unknown_lane = 0usize;
    let mut unnamed = 0usize;
    for (n, line) in jsonl_records(text) {
        records += 1;
        let j = json::parse(line).map_err(|e| format!("event-log line {}: {}", n, e))?;
        match parse_event(&j) {
            Ok(ev) => {
                // Read, but not read in full: an optional lane this build cannot name was
                // dropped while the rest of the record projected. `compact` must refuse the log
                // for the same reason it refuses a skipped one.
                if super::codec::names_an_unknown_lane(&j) {
                    unknown_lane += 1;
                }
                if let Event::BoardFormat { format } = &ev {
                    crate::model::format_declared(Some(format))
                        .map_err(|e| format!("event-log line {}: {}", n, e))?;
                }
                events.push(ev)
            }
            Err(Rejected::UnknownKind) => foreign += 1,
            Err(Rejected::UnknownLane) => unknown_lane += 1,
            Err(Rejected::Unnamed) => unnamed += 1,
            Err(Rejected::Malformed) => {
                let kind = j.get("event").and_then(Json::as_str).unwrap_or("?");
                return Err(format!(
                    "event-log line {}: {} event with a missing or mis-typed required field",
                    n, kind
                ));
            }
        }
    }
    if events.is_empty() {
        // Two different diagnoses, told apart by *what* was unreadable. An unknown kind may be a
        // whole other notation; an unknown lane cannot be — the kinds were ours.
        if foreign > 0 {
            return Err(format!(
                "event-log: {} record(s), none of a recognised event kind — this log is from \
                 another board format, or from a newer faceto",
                records
            ));
        }
        if unknown_lane > 0 {
            return Err(format!(
                "event-log: {} record(s) and no readable board — {} of them name a lane this \
                 faceto does not know, so the log is from a newer faceto",
                records, unknown_lane
            ));
        }
    }
    Ok(LogRead {
        events,
        unread: foreign + unknown_lane,
        corrupt: unnamed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn an_optional_lane_it_cannot_read_still_leaves_the_rest_of_the_move() {
        // The lane cannot be applied, but `col` is well-formed and unambiguous. Rejecting the
        // whole record threw the move away with it.
        let read = parse_log_full(
            r#"{"event":"ElementAdded","id":"E1","type":"event","label":"P","col":0}
{"event":"ElementMoved","id":"E1","col":5,"type":"timer"}"#,
        )
        .unwrap();
        assert_eq!(read.events.len(), 2);
        assert!(matches!(
            &read.events[1],
            Event::ElementMoved {
                col: Some(5),
                kind: None,
                ..
            }
        ));
        // Still not read in full, so `compact` must refuse it: the lane change is real data and
        // folding from the projection would delete it.
        assert_eq!(read.unread, 1);
    }

    #[test]
    fn a_required_lane_it_cannot_read_still_drops_the_whole_record() {
        // An `ElementAdded` has nowhere to put the sticky, so there is no half to keep.
        let read = parse_log_full(
            r#"{"event":"ElementAdded","id":"T1","type":"timer","label":"P"}
{"event":"BoardTitled","title":"T"}"#,
        )
        .unwrap();
        assert_eq!(read.events.len(), 1);
        assert_eq!(read.unread, 1);
    }

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
    fn an_unknown_lane_is_not_evidence_of_a_foreign_format() {
        // The kinds here are recognised; only the lane is not. Counting them as foreign sends the
        // reader after a notation problem when the log is simply from a newer faceto.
        let err = parse_log(
            "{\"event\":\"ElementAdded\",\"id\":\"T1\",\"type\":\"timer\",\"label\":\"A\"}\n",
        )
        .unwrap_err();
        assert!(err.contains("lane"), "{}", err);
        assert!(!err.contains("another board format"), "{}", err);
    }

    #[test]
    fn a_read_reports_what_it_skipped_so_compact_cannot_fold_it_away() {
        // `compact` rewrites the log from the projection, so a skipped record would be deleted
        // from append-only truth. The count is how the caller knows not to.
        let read = parse_log_full(
            "{\"event\":\"ElementAdded\",\"id\":\"E1\",\"type\":\"event\",\"label\":\"A\"}\n\
             {\"event\":\"ElementAdded\",\"id\":\"T1\",\"type\":\"timer\",\"label\":\"B\"}\n\
             {\"event\":\"FromTheFuture\",\"x\":1}\n",
        )
        .unwrap();
        assert_eq!(read.events.len(), 1);
        assert_eq!(read.unread, 2, "one unknown lane + one unknown kind");
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
    fn an_unrelated_type_field_is_not_a_lane_this_build_cannot_read() {
        // The log grammar ignores fields it does not know, so a `type` on a kind that carries no
        // lane is legal data — reading it as an off-grammar lane made `compact` refuse a log it
        // had in fact read in full.
        let read = parse_log_full(
            "{\"event\":\"BoardTitled\",\"title\":\"T\",\"type\":\"heading\"}\n\
             {\"event\":\"ElementAdded\",\"id\":\"E1\",\"type\":\"event\",\"label\":\"A\"}\n",
        )
        .unwrap();
        assert_eq!(read.events.len(), 2);
        assert_eq!(read.unread, 0);
    }

    #[test]
    fn a_corrupt_line_is_counted_apart_from_one_a_newer_faceto_would_read() {
        // The two need opposite remedies — wait for a build that knows the schema, or repair the
        // line by hand. One count can only offer one of them, and would offer the wrong one.
        let read = parse_log_full(
            "{\"event\":\"ElementAdded\",\"id\":\"E1\",\"type\":\"event\",\"label\":\"A\"}\n\
             {\"evnet\":\"typo\",\"x\":1}\n\
             {\"event\":\"FromTheFuture\",\"x\":1}\n",
        )
        .unwrap();
        assert_eq!(read.unread, 1);
        assert_eq!(read.corrupt, 1);
    }

    #[test]
    fn the_nothing_readable_errors_count_every_record_they_read() {
        // The count names the file, so it must be the file's: reporting only the records that hit
        // one counter told the reader a two-line log had one line.
        let err = parse_log(
            "{\"event\":\"CanvasNamed\",\"name\":\"B\"}\n\
             {\"evnet\":\"typo\",\"x\":1}\n",
        )
        .unwrap_err();
        assert!(err.contains("2 record(s)"), "{}", err);
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
