//! JSON codec for [`Event`]: parse a raw log record into an `Event` (with the legacy-kind
//! [`upcast`] seam) and serialize one back out ([`to_json`] / [`line()`] / [`to_jsonl`]).

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

/// The event kinds this build understands — the current schema plus the legacy aliases [`upcast`]
/// migrates forward. The single named list of the kind vocabulary, so [`parse_event`]'s match arms
/// and the malformed-vs-unknown split in `parse_log` read from one place instead of scattering the
/// strings. Keep in sync with [`parse_event`] / [`to_json`] whenever a variant is added.
pub(crate) const KNOWN_KINDS: &[&str] = &[
    "BoardTitled",
    "BoardFormat",
    "BoardLeveled",
    "PhaseAdded",
    "PhaseResized",
    "PhaseRenamed",
    "PhaseRemoved",
    "FrontierMoved",
    "PhaseSplit",
    "ElementAdded",
    "ElementRenamed",
    "ElementMoved",
    "ElementAnnotated",
    "HotspotResolved",
    "ElementRemoved",
    "EdgeAdded",
    "EdgeRemoved",
    "LogCompacted",
    // legacy aliases upcast() rewrites to a current kind
    "CommentAdded",
    "Comment",
];

/// Whether `kind` is one [`parse_event`] recognises (current schema + [`upcast`] aliases). Lets
/// `parse_log` distinguish a *malformed known* event (in this set, but [`parse_event`] couldn't
/// build it → hard error) from a *future/unknown* kind (outside it → skipped).
pub(crate) fn is_known_kind(kind: &str) -> bool {
    KNOWN_KINDS.contains(&kind)
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
        "BoardFormat" => Event::BoardFormat {
            format: str_field("format")?,
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
            links: crate::model::links_from(event.get("links")),
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
            label: str_field("label"),
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
        Event::BoardFormat { format } => {
            obj(vec![("event", s("BoardFormat")), ("format", s(format))])
        }
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
            links,
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
            if !links.is_empty() {
                p.push(("links", Json::Arr(links.iter().map(|l| s(l)).collect())));
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
        Event::EdgeAdded { src, dst, label } => {
            let mut p = vec![("event", s("EdgeAdded")), ("src", s(src)), ("dst", s(dst))];
            if let Some(l) = label {
                p.push(("label", s(l)));
            }
            obj(p)
        }
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
    use crate::events::testutil::*;
    use crate::events::*;
    use crate::json::{self, Json};

    #[test]
    fn is_known_kind_recognizes_every_current_event_variant() {
        // Guards the hand-maintained `KNOWN_KINDS` list against drift with `parse_event`'s match:
        // every kind `to_json` can emit must be recognized as *known*, so a malformed instance of
        // it is a hard error (not silently skipped as a future/unknown kind — the exact data-loss
        // bug the strict `parse_log` branch exists to prevent). One sample per variant; add one
        // when you add a variant. (Full compile-time enforcement needs a variant-enumeration crate,
        // which the zero-dep rule forbids — this guard plus the `KNOWN_KINDS` doc are the coupling.)
        let samples = vec![
            Event::BoardTitled { title: "t".into() },
            Event::BoardLeveled {
                level: "design".into(),
            },
            Event::PhaseAdded {
                id: Some("K1".into()),
                label: "a".into(),
                from_col: 0,
                to_col: 1,
            },
            Event::PhaseResized {
                id: "K1".into(),
                from_col: 0,
                to_col: 1,
            },
            Event::PhaseRenamed {
                id: "K1".into(),
                label: "a".into(),
            },
            Event::PhaseRemoved { id: "K1".into() },
            Event::FrontierMoved {
                id: "K1".into(),
                edge: "end".into(),
                col: 1,
            },
            Event::PhaseSplit {
                id: "K1".into(),
                at_col: 1,
                new_id: "K2".into(),
                new_label: "b".into(),
            },
            Event::ElementAdded {
                id: "E1".into(),
                kind: "event".into(),
                label: "a".into(),
                col: None,
                detail: None,
                y: None,
                links: Vec::new(),
            },
            Event::ElementRenamed {
                id: "E1".into(),
                label: "a".into(),
            },
            Event::ElementMoved {
                id: "E1".into(),
                col: None,
                kind: None,
                y: None,
            },
            Event::ElementAnnotated {
                id: "E1".into(),
                text: "x".into(),
            },
            Event::HotspotResolved {
                id: "H1".into(),
                resolution: "r".into(),
            },
            Event::ElementRemoved { id: "E1".into() },
            Event::EdgeAdded {
                src: "E1".into(),
                dst: "E2".into(),
                label: None,
            },
            Event::EdgeRemoved {
                src: "E1".into(),
                dst: "E2".into(),
            },
            Event::LogCompacted { folded: 3 },
        ];
        for ev in &samples {
            let kind = to_json(ev)
                .get("event")
                .and_then(Json::as_str)
                .expect("to_json emits an `event` kind")
                .to_string();
            assert!(
                is_known_kind(&kind),
                "`to_json` emits {kind:?} but `is_known_kind` rejects it — a malformed {kind} \
                 would be silently skipped instead of erroring; add it to `KNOWN_KINDS`."
            );
            // And the round-trip must rebuild it (no phantom / mis-typed sample).
            assert!(
                parse_event(&to_json(ev)).is_some(),
                "{kind} does not round-trip"
            );
        }
        // The legacy aliases `upcast` rewrites forward are known too (they are not `Event`
        // variants, so they can't appear in `samples`).
        assert!(is_known_kind("CommentAdded"));
        assert!(is_known_kind("Comment"));
    }

    #[test]
    fn phase_added_round_trips_its_id() {
        let e = ev(r#"{"event":"PhaseAdded","id":"K1","label":"Checkout","fromCol":0,"toCol":3}"#);
        assert!(matches!(&e, Event::PhaseAdded { id: Some(id), label, .. }
            if id == "K1" && label == "Checkout"));
        // serialize → parse is a fixed point
        assert_eq!(
            json::to_string(&to_json(&ev(&line(&e)))),
            json::to_string(&to_json(&e))
        );
    }

    #[test]
    fn frontier_and_split_events_round_trip() {
        for line in [
            r#"{"event":"FrontierMoved","id":"K1","edge":"end","col":5}"#,
            r#"{"event":"FrontierMoved","id":"K1","edge":"start","col":-2}"#,
            r#"{"event":"PhaseSplit","id":"K1","atCol":3,"newId":"K2","newLabel":"Right"}"#,
        ] {
            let e = ev(line);
            assert_eq!(super::line(&e), line, "canonical serialize round-trips");
            assert_eq!(ev(&super::line(&e)), e, "reparse round-trips");
        }
    }

    // H3: a renamed event kind from an older schema is migrated forward at the upcast seam, so an
    // old log still replays. `CommentAdded` predates the rename to `ElementAnnotated`.
    #[test]
    fn legacy_comment_kind_upcasts_to_element_annotated() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A"}"#),
            ev(r#"{"event":"CommentAdded","id":"E1","text":"from an old log"}"#),
            ev(r#"{"event":"Comment","id":"E1","text":"older still"}"#),
        ];
        assert!(matches!(&log[1], Event::ElementAnnotated { id, text }
            if id == "E1" && text == "from an old log"));
        assert!(matches!(&log[2], Event::ElementAnnotated { .. }));
        // …and the migrated event folds into the projection like any annotation.
        assert_eq!(
            replay(&log).elements[0].detail.as_deref(),
            Some("older still")
        );
    }

    // H3: additive change is free — an unknown field on a known event is ignored, not an error,
    // so a log written by a newer schema still replays on older code.
    #[test]
    fn unknown_fields_on_a_known_event_are_ignored() {
        let e = ev(
            r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","fromTheFuture":42}"#,
        );
        assert!(matches!(e, Event::ElementAdded { id, .. } if id == "E1"));
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
            y: None,
        };
        assert_eq!(
            line(&moved),
            r#"{"event":"ElementMoved","id":"E1","col":4}"#
        );
    }

    // ---- F-2d-placement: the stored vertical sub-position ---------------------------------
    // `y` is a fraction of the lane-band interior in [0, 1] — never identity (`id`), never the
    // lane (`type`), never the timeline (`col`). It evolves the schema additively: an old log
    // simply has no `y` and replays exactly as before.

    #[test]
    fn element_moved_round_trips_its_y() {
        let e = ev(r#"{"event":"ElementMoved","id":"E1","y":0.35}"#);
        assert!(
            matches!(&e, Event::ElementMoved { id, col: None, y: Some(y), .. }
            if id == "E1" && *y == 0.35)
        );
        assert_eq!(line(&e), r#"{"event":"ElementMoved","id":"E1","y":0.35}"#);
    }

    #[test]
    fn board_leveled_is_a_serialize_parse_fixed_point() {
        let e = ev(r#"{"event":"BoardLeveled","level":"design"}"#);
        assert!(matches!(&e, Event::BoardLeveled { level } if level == "design"));
        assert_eq!(
            json::to_string(&to_json(&ev(&line(&e)))),
            json::to_string(&to_json(&e))
        );
    }
}
