//! The event-sourced spine: an append-only log is the durable record; the `Model` is a
//! projection replayed from it.
//!
//! This inverts the older "model file is truth, comments are a disposable inbox" stance
//! (see `docs/source-of-truth.md`). Here `event-log.jsonl` is the only durable record and
//! the only write path; `model.json` becomes derived output. Each log line is one JSON
//! object discriminated by an `"event"` field; [`replay`] folds a sequence into a `Model`,
//! and [`from_model`] turns an existing model file into a genesis batch (the migration and
//! bootstrap path). Unknown event kinds are skipped on read, so the schema can grow forward.

use crate::json::{self, Json};
use crate::model::{Edge, Element, Model, Phase};
use std::path::Path;

/// One fact in the log. Variants mirror the board operations a session performs; the
/// `Element*` variants all carry the stable `id` (identity is never text or position).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    BoardTitled {
        title: String,
    },
    PhaseAdded {
        label: String,
        from_col: i64,
        to_col: i64,
    },
    ElementAdded {
        id: String,
        kind: String,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
    },
    ElementRenamed {
        id: String,
        label: String,
    },
    ElementMoved {
        id: String,
        col: Option<i64>,
        kind: Option<String>,
    },
    ElementAnnotated {
        id: String,
        text: String,
    },
    HotspotResolved {
        id: String,
        resolution: String,
    },
    ElementRemoved {
        id: String,
    },
    EdgeAdded {
        src: String,
        dst: String,
    },
    EdgeRemoved {
        src: String,
        dst: String,
    },
    /// Provenance marker written by `faceto compact`: the log up to here was folded into the
    /// genesis batch that follows. A no-op on replay; `folded` is the event count it replaced.
    LogCompacted {
        folded: i64,
    },
}

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

/// Parse JSONL text into events. Blank lines are skipped; a line that does not parse as
/// JSON is a hard error (the log is the source of truth); a well-formed object whose
/// `"event"` is unknown is skipped (forward compatibility across schema versions).
pub fn parse_log(text: &str) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let j = json::parse(line).map_err(|e| format!("event-log line {}: {}", n + 1, e))?;
        if let Some(ev) = parse_event(&j) {
            events.push(ev);
        }
    }
    Ok(events)
}

/// One JSON object → an `Event`, or `None` for an unknown/ill-shaped event kind.
pub fn parse_event(j: &Json) -> Option<Event> {
    let s = |k: &str| j.get(k).and_then(Json::as_str).map(String::from);
    let i = |k: &str| j.get(k).and_then(Json::as_f64).map(|n| n as i64);
    Some(match j.get("event")?.as_str()? {
        "BoardTitled" => Event::BoardTitled { title: s("title")? },
        "PhaseAdded" => Event::PhaseAdded {
            label: s("label")?,
            from_col: i("fromCol")?,
            to_col: i("toCol")?,
        },
        "ElementAdded" => Event::ElementAdded {
            id: s("id")?,
            kind: s("type")?,
            label: s("label")?,
            col: i("col"),
            detail: s("detail"),
        },
        "ElementRenamed" => Event::ElementRenamed {
            id: s("id")?,
            label: s("label")?,
        },
        "ElementMoved" => Event::ElementMoved {
            id: s("id")?,
            col: i("col"),
            kind: s("type"),
        },
        "ElementAnnotated" => Event::ElementAnnotated {
            id: s("id")?,
            text: s("text")?,
        },
        "HotspotResolved" => Event::HotspotResolved {
            id: s("id")?,
            resolution: s("resolution")?,
        },
        "ElementRemoved" => Event::ElementRemoved { id: s("id")? },
        "EdgeAdded" => Event::EdgeAdded {
            src: s("src")?,
            dst: s("dst")?,
        },
        "EdgeRemoved" => Event::EdgeRemoved {
            src: s("src")?,
            dst: s("dst")?,
        },
        "LogCompacted" => Event::LogCompacted {
            folded: i("folded").unwrap_or(0),
        },
        _ => return None,
    })
}

/// Fold a sequence of events into the board they describe. The projection is pure and
/// deterministic: same log → same `Model`.
pub fn replay(events: &[Event]) -> Model {
    let mut m = Model::default();
    for ev in events {
        match ev {
            Event::BoardTitled { title } => m.title = title.clone(),
            Event::PhaseAdded {
                label,
                from_col,
                to_col,
            } => m.phases.push(Phase {
                label: label.clone(),
                from_col: *from_col,
                to_col: *to_col,
            }),
            Event::ElementAdded {
                id,
                kind,
                label,
                col,
                detail,
            } => {
                if !m.elements.iter().any(|e| &e.id == id) {
                    m.elements.push(Element {
                        id: id.clone(),
                        kind: kind.clone(),
                        label: label.clone(),
                        col: *col,
                        detail: detail.clone(),
                        resolved: false,
                        x: 0.0,
                        diff: None,
                        was: None,
                    });
                }
            }
            Event::ElementRenamed { id, label } => {
                if let Some(e) = find(&mut m, id) {
                    e.label = label.clone();
                }
            }
            Event::ElementMoved { id, col, kind } => {
                if let Some(e) = find(&mut m, id) {
                    if col.is_some() {
                        e.col = *col;
                    }
                    if let Some(k) = kind {
                        e.kind = k.clone();
                    }
                }
            }
            Event::ElementAnnotated { id, text } => {
                if let Some(e) = find(&mut m, id) {
                    e.detail = Some(text.clone());
                }
            }
            Event::HotspotResolved { id, resolution } => {
                if let Some(e) = find(&mut m, id) {
                    e.resolved = true;
                    e.detail = Some(resolution.clone());
                }
            }
            Event::ElementRemoved { id } => {
                m.elements.retain(|e| &e.id != id);
                m.edges.retain(|e| &e.src != id && &e.dst != id);
            }
            Event::EdgeAdded { src, dst } => {
                if !m.edges.iter().any(|e| &e.src == src && &e.dst == dst) {
                    m.edges.push(Edge {
                        src: src.clone(),
                        dst: dst.clone(),
                        status: None,
                    });
                }
            }
            Event::EdgeRemoved { src, dst } => {
                m.edges.retain(|e| !(&e.src == src && &e.dst == dst))
            }
            // A compaction marker carries no board state; it only records that earlier history
            // was folded away. Replaying it is a no-op.
            Event::LogCompacted { .. } => {}
        }
    }
    m
}

fn find<'a>(m: &'a mut Model, id: &str) -> Option<&'a mut Element> {
    m.elements.iter_mut().find(|e| e.id == id)
}

/// Turn an existing model into the genesis batch of events that reconstructs it — the
/// migration and bootstrap path (an old `model.json` becomes the start of a log). A
/// resolved hotspot is replayed as an add followed by its resolution, so its `detail`
/// (the resolution note) round-trips.
pub fn from_model(m: &Model) -> Vec<Event> {
    let mut ev = Vec::new();
    if !m.title.is_empty() {
        ev.push(Event::BoardTitled {
            title: m.title.clone(),
        });
    }
    for p in &m.phases {
        ev.push(Event::PhaseAdded {
            label: p.label.clone(),
            from_col: p.from_col,
            to_col: p.to_col,
        });
    }
    for e in &m.elements {
        ev.push(Event::ElementAdded {
            id: e.id.clone(),
            kind: e.kind.clone(),
            label: e.label.clone(),
            col: e.col,
            detail: if e.resolved { None } else { e.detail.clone() },
        });
        if e.resolved {
            ev.push(Event::HotspotResolved {
                id: e.id.clone(),
                resolution: e.detail.clone().unwrap_or_default(),
            });
        }
    }
    for e in &m.edges {
        ev.push(Event::EdgeAdded {
            src: e.src.clone(),
            dst: e.dst.clone(),
        });
    }
    ev
}

/// Map one posted/stored comment object to the event(s) it persists — the single source of
/// truth for the comment→event translation, shared by the live server (`POST /comment` in log
/// mode) and the `comments.jsonl` migration ([`from_comments`]). `move`/`resolve`/`rename`/`drop`
/// carry structural intent and fold straight into the projection; `split`/`question`/`comment`
/// stay advisory annotations. A `move` that displaces an occupant — the client sends
/// `swapId`/`swapCol` — yields **two** `ElementMoved`s so the swap round-trips. Returns an empty
/// vec when the comment names no element, or when a `move` carries no target col (both would
/// replay as no-ops): the caller treats that as "nothing to persist".
pub fn comment_to_events(v: &Json) -> Vec<Event> {
    let Some(id) = v.get_str("elemId").map(str::to_string) else {
        return Vec::new();
    };
    let kind = v.get_str("kind").unwrap_or("comment");
    let text = v.get_str("text").unwrap_or("").to_string();
    match kind {
        "move" => {
            // A move is a column change; a missing target col would replay as a no-op, so reject
            // it (empty vec) rather than logging a phantom move.
            let Some(col) = v.get_i64("col") else {
                return Vec::new();
            };
            let mut evs = vec![Event::ElementMoved {
                id: id.clone(),
                col: Some(col),
                kind: None,
            }];
            // A swap also relocates the displaced sticky — but only a *different* one, to a real
            // col. Guard against a self-swap or a swap missing its target col (would no-op).
            if let (Some(swap_id), Some(swap_col)) = (v.get_str("swapId"), v.get_i64("swapCol")) {
                if swap_id != id.as_str() {
                    evs.push(Event::ElementMoved {
                        id: swap_id.to_string(),
                        col: Some(swap_col),
                        kind: None,
                    });
                }
            }
            evs
        }
        "resolve" => vec![Event::HotspotResolved {
            id,
            resolution: text,
        }],
        "rename" => vec![Event::ElementRenamed { id, label: text }],
        "drop" => vec![Event::ElementRemoved { id }],
        _ => vec![Event::ElementAnnotated { id, text }],
    }
}

/// Fold a legacy `comments.jsonl` into the events it represents — the answer to H5, the second
/// half of the migration story alongside [`from_model`]. Each line is one stored comment;
/// [`comment_to_events`] translates it. Unlike the log proper, the comments inbox was always a
/// *best-effort* sidecar, so a blank, unparseable, or element-less line is **skipped** (not a hard
/// error) — migrating disposable feedback must not abort on one stray line. Append the result
/// after a model's genesis batch: the batch mints the ids these comments reference, so replaying
/// the two together reconstructs the board *and* its annotations/resolutions/renames.
pub fn from_comments(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v @ Json::Obj(_)) = json::parse(line) {
            out.extend(comment_to_events(&v));
        }
    }
    out
}

/// Fold a log down to the shortest sequence that replays to the same board: a `LogCompacted`
/// provenance marker, then the genesis batch of the current projection. This bounds replay
/// length (H1's snapshot escape hatch). It is lossy *by design* — only the projection survives,
/// so the comment **history** is dropped (each element keeps just its latest note, folded into
/// `detail`); the full prior log stays recoverable from version control or a `.bak`.
///
/// `replay(compact(log))` always projects the same `Model` as `replay(log)`, and the genesis
/// tail is a fixed point (compacting again changes only the marker's count).
pub fn compact(events: &[Event]) -> Vec<Event> {
    let model = replay(events);
    let mut out = vec![Event::LogCompacted {
        folded: events.len() as i64,
    }];
    out.extend(from_model(&model));
    out
}

/// Serialize one event to its canonical JSON object.
pub fn to_json(ev: &Event) -> Json {
    let obj = |pairs: Vec<(&str, Json)>| {
        Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    };
    let s = |x: &str| Json::Str(x.to_string());
    let n = |x: i64| Json::Num(x as f64);
    match ev {
        Event::BoardTitled { title } => obj(vec![("event", s("BoardTitled")), ("title", s(title))]),
        Event::PhaseAdded {
            label,
            from_col,
            to_col,
        } => obj(vec![
            ("event", s("PhaseAdded")),
            ("label", s(label)),
            ("fromCol", n(*from_col)),
            ("toCol", n(*to_col)),
        ]),
        Event::ElementAdded {
            id,
            kind,
            label,
            col,
            detail,
        } => {
            let mut p = vec![
                ("event", s("ElementAdded")),
                ("id", s(id)),
                ("type", s(kind)),
                ("label", s(label)),
            ];
            if let Some(c) = col {
                p.push(("col", n(*c)));
            }
            if let Some(d) = detail {
                p.push(("detail", s(d)));
            }
            obj(p)
        }
        Event::ElementRenamed { id, label } => obj(vec![
            ("event", s("ElementRenamed")),
            ("id", s(id)),
            ("label", s(label)),
        ]),
        Event::ElementMoved { id, col, kind } => {
            let mut p = vec![("event", s("ElementMoved")), ("id", s(id))];
            if let Some(c) = col {
                p.push(("col", n(*c)));
            }
            if let Some(k) = kind {
                p.push(("type", s(k)));
            }
            obj(p)
        }
        Event::ElementAnnotated { id, text } => obj(vec![
            ("event", s("ElementAnnotated")),
            ("id", s(id)),
            ("text", s(text)),
        ]),
        Event::HotspotResolved { id, resolution } => obj(vec![
            ("event", s("HotspotResolved")),
            ("id", s(id)),
            ("resolution", s(resolution)),
        ]),
        Event::ElementRemoved { id } => obj(vec![("event", s("ElementRemoved")), ("id", s(id))]),
        Event::EdgeAdded { src, dst } => obj(vec![
            ("event", s("EdgeAdded")),
            ("src", s(src)),
            ("dst", s(dst)),
        ]),
        Event::EdgeRemoved { src, dst } => obj(vec![
            ("event", s("EdgeRemoved")),
            ("src", s(src)),
            ("dst", s(dst)),
        ]),
        Event::LogCompacted { folded } => {
            obj(vec![("event", s("LogCompacted")), ("folded", n(*folded))])
        }
    }
}

/// One event → one JSONL line (no trailing newline).
pub fn line(ev: &Event) -> String {
    json::to_string(&to_json(ev))
}

/// A whole batch → JSONL text, one event per line (newline-terminated).
pub fn to_jsonl(events: &[Event]) -> String {
    let mut out = String::new();
    for ev in events {
        out.push_str(&line(ev));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(line: &str) -> Event {
        parse_event(&json::parse(line).unwrap()).unwrap()
    }

    #[test]
    fn replay_builds_the_board_the_events_describe() {
        let log = [
            ev(r#"{"event":"BoardTitled","title":"T"}"#),
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"Born","col":1}"#),
            ev(r#"{"event":"ElementAdded","id":"E2","type":"command","label":"Do","col":0}"#),
            ev(r#"{"event":"ElementRenamed","id":"E1","label":"Reborn"}"#),
            ev(r#"{"event":"ElementMoved","id":"E2","col":3}"#),
            ev(r#"{"event":"EdgeAdded","src":"E2","dst":"E1"}"#),
        ];
        let m = replay(&log);
        assert_eq!(m.title, "T");
        let e1 = m.elements.iter().find(|e| e.id == "E1").unwrap();
        assert_eq!(e1.label, "Reborn");
        let e2 = m.elements.iter().find(|e| e.id == "E2").unwrap();
        assert_eq!(e2.col, Some(3));
        assert_eq!(m.edges.len(), 1);
    }

    #[test]
    fn resolving_a_hotspot_flips_state_and_records_the_note() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"H1","type":"hotspot","label":"open?"}"#),
            ev(r#"{"event":"HotspotResolved","id":"H1","resolution":"settled"}"#),
        ];
        let h = &replay(&log).elements[0];
        assert!(h.resolved);
        assert_eq!(h.detail.as_deref(), Some("settled"));
    }

    #[test]
    fn remove_drops_the_element_and_its_edges() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A"}"#),
            ev(r#"{"event":"ElementAdded","id":"E2","type":"event","label":"B"}"#),
            ev(r#"{"event":"EdgeAdded","src":"E1","dst":"E2"}"#),
            ev(r#"{"event":"ElementRemoved","id":"E1"}"#),
        ];
        let m = replay(&log);
        assert_eq!(m.elements.len(), 1);
        assert!(m.edges.is_empty());
    }

    // The migration contract: an existing model → genesis events → replay must reproduce it.
    #[test]
    fn from_model_then_replay_round_trips() {
        let src = r#"{
            "title":"Round Trip",
            "phases":[{"label":"p","fromCol":0,"toCol":2}],
            "elements":[
                {"id":"E1","type":"event","label":"Made","col":1},
                {"id":"E2","type":"command","label":"Do","col":0,"detail":"a note"},
                {"id":"H1","type":"hotspot","label":"q","col":2,"resolved":true,"detail":"done"}
            ],
            "edges":[["E2","E1"]]
        }"#;
        let original = crate::model::from_json(&json::parse(src).unwrap());
        let rebuilt = replay(&from_model(&original));

        assert_eq!(rebuilt.title, original.title);
        assert_eq!(rebuilt.phases.len(), 1);
        assert_eq!(rebuilt.elements.len(), 3);
        assert_eq!(rebuilt.edges.len(), 1);
        let h1 = rebuilt.elements.iter().find(|e| e.id == "H1").unwrap();
        assert!(h1.resolved);
        assert_eq!(h1.detail.as_deref(), Some("done"));
        let e2 = rebuilt.elements.iter().find(|e| e.id == "E2").unwrap();
        assert_eq!(e2.detail.as_deref(), Some("a note"));
    }

    #[test]
    fn compact_preserves_the_projection_and_folds_history() {
        let log = [
            ev(r#"{"event":"BoardTitled","title":"T"}"#),
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"Born","col":1}"#),
            ev(r#"{"event":"ElementRenamed","id":"E1","label":"Reborn"}"#),
            ev(r#"{"event":"ElementAnnotated","id":"E1","text":"a note"}"#),
            ev(r#"{"event":"ElementAdded","id":"H1","type":"hotspot","label":"q"}"#),
            ev(r#"{"event":"HotspotResolved","id":"H1","resolution":"settled"}"#),
        ];
        let folded = compact(&log);

        // Leads with a provenance marker recording the prior length, and reparses cleanly.
        assert!(matches!(folded[0], Event::LogCompacted { folded: 6 }));
        let reparsed = parse_log(&to_jsonl(&folded)).unwrap();
        assert!(matches!(reparsed[0], Event::LogCompacted { folded: 6 }));

        // Shorter than the original: the rename + annotate + resolve history collapsed.
        assert!(folded.len() < log.len());

        // Same projection: title, the *latest* label, the note folded into detail, the resolution.
        let (before, after) = (replay(&log), replay(&folded));
        assert_eq!(after.title, before.title);
        let e1 = after.elements.iter().find(|e| e.id == "E1").unwrap();
        assert_eq!(e1.label, "Reborn");
        assert_eq!(e1.detail.as_deref(), Some("a note"));
        let h1 = after.elements.iter().find(|e| e.id == "H1").unwrap();
        assert!(h1.resolved);
        assert_eq!(h1.detail.as_deref(), Some("settled"));
    }

    #[test]
    fn compacting_twice_leaves_the_snapshot_stable() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":0}"#),
            ev(r#"{"event":"ElementMoved","id":"E1","col":2}"#),
        ];
        let once = compact(&log);
        let twice = compact(&once);
        // The genesis tail (everything past the marker) is a fixed point; only the count moves.
        assert_eq!(to_jsonl(&once[1..]), to_jsonl(&twice[1..]));
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

        let mut log = from_model(&model);
        log.extend(from_comments(inbox));
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
            {\"elemId\":\"E1\",\"kind\":\"comment\",\"text\":\"kept\"}\n";
        let evs = from_comments(inbox);
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], Event::ElementAnnotated { id, text }
            if id == "E1" && text == "kept"));
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
    fn events_serialize_to_canonical_jsonl_and_reparse() {
        let original = ev(
            r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":2,"detail":"d"}"#,
        );
        assert_eq!(ev(&line(&original)), original);
        let moved = Event::ElementMoved {
            id: "E1".into(),
            col: Some(4),
            kind: None,
        };
        assert_eq!(
            line(&moved),
            r#"{"event":"ElementMoved","id":"E1","col":4}"#
        );
    }

    #[test]
    fn is_log_path_keys_on_extension() {
        assert!(is_log_path(Path::new("event-log.jsonl")));
        assert!(is_log_path(Path::new("a.log")));
        assert!(!is_log_path(Path::new("model.json")));
    }
}
