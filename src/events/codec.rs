//! JSON codec for [`Event`]: parse a raw log record into an `Event` (with the legacy-kind
//! [`upcast`] seam) and serialize one back out ([`to_json`] / [`line`] / [`to_jsonl`]).

use super::Event;
use crate::json::{self, Json};
use std::borrow::Cow;

/// Normalise a raw event object to the current schema before [`parse_event`] matches it — the
/// single seam where the log's *history* is migrated forward (H3). Additive change needs no entry
/// here (new fields are ignored, new kinds are skipped on read); only a renamed *event kind* is
/// repaired, so everything downstream sees today's kinds. (A renamed *field* can't be repaired by
/// shape — an absent key looks like a new optional one — so fields evolve additively instead.)
/// Detection is by shape, not a version counter, and a current-shape object is returned untouched
/// (borrowed, no allocation).
///
/// Current rules:
/// - The annotation event was once a first-class "comment" (see this module's history); a log or
///   external tool that still emits `CommentAdded` / `Comment` is read as `ElementAnnotated`.
fn upcast(j: &Json) -> Cow<'_, Json> {
    // Rewrite the `event` discriminator to `to`, preserving every other field in order. Only
    // reached once a known legacy kind string has matched, so the slot is always present.
    let rename = |pairs: &[(String, Json)], to: &str| {
        Json::Obj(
            pairs
                .iter()
                .map(|(k, v)| match k.as_str() {
                    "event" => (k.clone(), Json::Str(to.to_string())),
                    _ => (k.clone(), v.clone()),
                })
                .collect(),
        )
    };
    match j {
        Json::Obj(pairs) => match j.get("event").and_then(Json::as_str) {
            Some("CommentAdded") | Some("Comment") => Cow::Owned(rename(pairs, "ElementAnnotated")),
            _ => Cow::Borrowed(j),
        },
        _ => Cow::Borrowed(j),
    }
}

/// One JSON object → an `Event`, or `None` for an unknown/ill-shaped event kind. The object is
/// first run through [`upcast`], so a legacy on-disk shape is migrated to the current schema (H3)
/// before any field is read.
pub fn parse_event(raw: &Json) -> Option<Event> {
    let event = upcast(raw);
    let event = event.as_ref();
    // Typed field accessors over the (upcast) event object: absent or mis-typed → `None`.
    let str_field = |key: &str| event.get(key).and_then(Json::as_str).map(String::from);
    let int_field = |key: &str| event.get(key).and_then(Json::as_f64).map(|n| n as i64);
    let num_field = |key: &str| event.get(key).and_then(Json::as_f64);
    Some(match event.get("event")?.as_str()? {
        "BoardTitled" => Event::BoardTitled {
            title: str_field("title")?,
        },
        "BoardLeveled" => Event::BoardLeveled {
            level: str_field("level")?,
        },
        "PhaseAdded" => Event::PhaseAdded {
            id: str_field("id"),
            label: str_field("label")?,
            from_col: int_field("fromCol")?,
            to_col: int_field("toCol")?,
        },
        "PhaseResized" => Event::PhaseResized {
            id: str_field("id")?,
            from_col: int_field("fromCol")?,
            to_col: int_field("toCol")?,
        },
        "PhaseRenamed" => Event::PhaseRenamed {
            id: str_field("id")?,
            label: str_field("label")?,
        },
        "PhaseRemoved" => Event::PhaseRemoved {
            id: str_field("id")?,
        },
        "FrontierMoved" => Event::FrontierMoved {
            id: str_field("id")?,
            edge: str_field("edge")?,
            col: int_field("col")?,
        },
        "PhaseSplit" => Event::PhaseSplit {
            id: str_field("id")?,
            at_col: int_field("atCol")?,
            new_id: str_field("newId")?,
            new_label: str_field("newLabel")?,
        },
        "ElementAdded" => Event::ElementAdded {
            id: str_field("id")?,
            kind: str_field("type")?,
            label: str_field("label")?,
            col: int_field("col"),
            detail: str_field("detail"),
            y: num_field("y"),
        },
        "ElementRenamed" => Event::ElementRenamed {
            id: str_field("id")?,
            label: str_field("label")?,
        },
        "ElementMoved" => Event::ElementMoved {
            id: str_field("id")?,
            col: int_field("col"),
            kind: str_field("type"),
            y: num_field("y"),
        },
        "ElementAnnotated" => Event::ElementAnnotated {
            id: str_field("id")?,
            text: str_field("text")?,
        },
        "HotspotResolved" => Event::HotspotResolved {
            id: str_field("id")?,
            resolution: str_field("resolution")?,
        },
        "ElementRemoved" => Event::ElementRemoved {
            id: str_field("id")?,
        },
        "EdgeAdded" => Event::EdgeAdded {
            src: str_field("src")?,
            dst: str_field("dst")?,
        },
        "EdgeRemoved" => Event::EdgeRemoved {
            src: str_field("src")?,
            dst: str_field("dst")?,
        },
        "LogCompacted" => Event::LogCompacted {
            folded: int_field("folded").unwrap_or(0),
        },
        _ => return None,
    })
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
        Event::BoardLeveled { level } => {
            obj(vec![("event", s("BoardLeveled")), ("level", s(level))])
        }
        Event::PhaseAdded {
            id,
            label,
            from_col,
            to_col,
        } => {
            let mut p = vec![("event", s("PhaseAdded"))];
            if let Some(id) = id {
                p.push(("id", s(id)));
            }
            p.push(("label", s(label)));
            p.push(("fromCol", n(*from_col)));
            p.push(("toCol", n(*to_col)));
            obj(p)
        }
        Event::PhaseResized {
            id,
            from_col,
            to_col,
        } => obj(vec![
            ("event", s("PhaseResized")),
            ("id", s(id)),
            ("fromCol", n(*from_col)),
            ("toCol", n(*to_col)),
        ]),
        Event::PhaseRenamed { id, label } => obj(vec![
            ("event", s("PhaseRenamed")),
            ("id", s(id)),
            ("label", s(label)),
        ]),
        Event::PhaseRemoved { id } => obj(vec![("event", s("PhaseRemoved")), ("id", s(id))]),
        Event::FrontierMoved { id, edge, col } => obj(vec![
            ("event", s("FrontierMoved")),
            ("id", s(id)),
            ("edge", s(edge)),
            ("col", n(*col)),
        ]),
        Event::PhaseSplit {
            id,
            at_col,
            new_id,
            new_label,
        } => obj(vec![
            ("event", s("PhaseSplit")),
            ("id", s(id)),
            ("atCol", n(*at_col)),
            ("newId", s(new_id)),
            ("newLabel", s(new_label)),
        ]),
        Event::ElementAdded {
            id,
            kind,
            label,
            col,
            detail,
            y,
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
            if let Some(y) = y {
                p.push(("y", Json::Num(*y)));
            }
            obj(p)
        }
        Event::ElementRenamed { id, label } => obj(vec![
            ("event", s("ElementRenamed")),
            ("id", s(id)),
            ("label", s(label)),
        ]),
        Event::ElementMoved { id, col, kind, y } => {
            let mut p = vec![("event", s("ElementMoved")), ("id", s(id))];
            if let Some(c) = col {
                p.push(("col", n(*c)));
            }
            if let Some(k) = kind {
                p.push(("type", s(k)));
            }
            if let Some(y) = y {
                p.push(("y", Json::Num(*y)));
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
