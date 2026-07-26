//! The canvas's event-sourced spine: `CanvasEvent`, `replay`, `from_canvas`, and — the part that
//! matters for the spike — a **verbatim copy** of the log framing.
//!
//! SPIKE FINDING (the headline one). `crate::events::log` is already generic in everything but its
//! type: `jsonl_records`, "blank lines skip / bad JSON is fatal / unknown kind skips / known kind
//! with a bad field is fatal", and the `upcast` seam are all format-agnostic *policy* — but they
//! are written against `Event` and are `pub(crate)` to `crate::events`. So a second format cannot
//! call them; it must copy them, and then the two copies can drift. This is the single clearest
//! extraction the spike found, and it is cheap: the policy only needs
//! `parse: &dyn Fn(&Json) -> Option<E>` + `is_known_kind: &dyn Fn(&str) -> bool`.
//!
//! SPIKE FINDING (the dangerous one). See `a_canvas_log_replays_as_a_silent_empty_es_board` at the
//! bottom of this file. Without a format tag, `crate::events::parse_log` reads a canvas log
//! **successfully** and yields an empty `Model` — every line is an "unknown kind", and unknown
//! kinds are skipped by design for forward compatibility. Forward compatibility and format
//! discrimination are the same mechanism pointed in opposite directions. The genesis-header format
//! tag (`docs/multi-format-architecture.md` §The Format seam) is therefore not a convenience; it is
//! the only thing standing between "wrong file" and "silently blank board".

use super::model::{slot_from_str, Canvas, Item, Slot};
use crate::json::{self, Json};
use std::path::Path;

/// One fact in a canvas log.
///
/// SPIKE FINDING (kernel, broke): this enum shares **no variant** with `crate::events::Event`.
/// Not one. `ElementMoved { col, kind, y }` has no canvas counterpart; `ItemReslotted { slot }`
/// has no ES counterpart (`ElementMoved.kind` is the nearest, and it moves an element *between
/// lanes* — spatially, whereas a reslot is purely categorical). `PhaseAdded`/`FrontierMoved`/
/// `PhaseSplit`/`HotspotResolved` are pure ES. So the answer to "does `replay` generalise?" is
/// **no**: the `Event` enum forks per format, and the kernel keeps only the *journal*, never the
/// vocabulary. The note's lean toward a tolerant `Vec<Json>` kernel log is correct.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    CanvasNamed {
        name: String,
    },
    ItemAdded {
        id: String,
        slot: Slot,
        text: String,
        via: Option<String>,
    },
    ItemEdited {
        id: String,
        text: String,
    },
    /// Move an item to a different section. The canvas's *entire* "move" vocabulary — categorical,
    /// with no coordinate, no ordering, and no neighbour to re-border.
    ItemReslotted {
        id: String,
        slot: Slot,
    },
    ItemRemoved {
        id: String,
    },
    /// Kept byte-identical to the ES marker on purpose: `compact` provenance is genuinely generic.
    LogCompacted {
        folded: i64,
    },
}

/// The kinds this build understands. Copied structure from `crate::events::codec::KNOWN_KINDS`.
const KNOWN_KINDS: &[&str] = &[
    "CanvasNamed",
    "ItemAdded",
    "ItemEdited",
    "ItemReslotted",
    "ItemRemoved",
    "LogCompacted",
];

fn is_known_kind(kind: &str) -> bool {
    KNOWN_KINDS.contains(&kind)
}

fn parse_event(j: &Json) -> Option<CanvasEvent> {
    match j.get_str("event")? {
        "CanvasNamed" => Some(CanvasEvent::CanvasNamed {
            name: j.get_str("name")?.to_string(),
        }),
        "ItemAdded" => Some(CanvasEvent::ItemAdded {
            id: j.get_str("id")?.to_string(),
            slot: slot_from_str(j.get_str("slot")?)?,
            text: j.get_str("text")?.to_string(),
            via: j.get_str("via").map(String::from),
        }),
        "ItemEdited" => Some(CanvasEvent::ItemEdited {
            id: j.get_str("id")?.to_string(),
            text: j.get_str("text")?.to_string(),
        }),
        "ItemReslotted" => Some(CanvasEvent::ItemReslotted {
            id: j.get_str("id")?.to_string(),
            slot: slot_from_str(j.get_str("slot")?)?,
        }),
        "ItemRemoved" => Some(CanvasEvent::ItemRemoved {
            id: j.get_str("id")?.to_string(),
        }),
        "LogCompacted" => Some(CanvasEvent::LogCompacted {
            folded: j.get_i64("folded")?,
        }),
        _ => None,
    }
}

fn to_json(e: &CanvasEvent) -> Json {
    let s = |v: &str| Json::Str(v.to_string());
    let mut o: Vec<(String, Json)> = Vec::new();
    let mut put = |k: &str, v: Json| o.push((k.to_string(), v));
    match e {
        CanvasEvent::CanvasNamed { name } => {
            put("event", s("CanvasNamed"));
            put("name", s(name));
        }
        CanvasEvent::ItemAdded {
            id,
            slot,
            text,
            via,
        } => {
            put("event", s("ItemAdded"));
            put("id", s(id));
            put("slot", s(slot.key()));
            put("text", s(text));
            if let Some(v) = via {
                put("via", s(v));
            }
        }
        CanvasEvent::ItemEdited { id, text } => {
            put("event", s("ItemEdited"));
            put("id", s(id));
            put("text", s(text));
        }
        CanvasEvent::ItemReslotted { id, slot } => {
            put("event", s("ItemReslotted"));
            put("id", s(id));
            put("slot", s(slot.key()));
        }
        CanvasEvent::ItemRemoved { id } => {
            put("event", s("ItemRemoved"));
            put("id", s(id));
        }
        CanvasEvent::LogCompacted { folded } => {
            put("event", s("LogCompacted"));
            put("folded", Json::Num(*folded as f64));
        }
    }
    Json::Obj(o)
}

pub fn to_jsonl(events: &[CanvasEvent]) -> String {
    events
        .iter()
        .map(|e| format!("{}\n", json::to_string(&to_json(e))))
        .collect()
}

// ---- log framing — COPIED from `crate::events::log`, see the module note -------------------

/// Copy of `crate::events::parse_log`, retyped to `CanvasEvent`. Line-for-line the same policy.
pub fn parse_log(text: &str) -> Result<Vec<CanvasEvent>, String> {
    let mut events = Vec::new();
    for (n, line) in text
        .lines()
        .enumerate()
        .map(|(n, l)| (n + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
    {
        let j = json::parse(line).map_err(|e| format!("canvas-log line {}: {}", n, e))?;
        match parse_event(&j) {
            Some(ev) => events.push(ev),
            None => {
                if let Some(kind) = j.get_str("event") {
                    if is_known_kind(kind) {
                        return Err(format!(
                            "canvas-log line {}: {} event with a missing or mis-typed required field",
                            n, kind
                        ));
                    }
                }
            }
        }
    }
    Ok(events)
}

pub fn read_log(path: &Path) -> Result<Vec<CanvasEvent>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    parse_log(&text)
}

pub fn load_log(path: &Path) -> Result<Canvas, String> {
    Ok(replay(&read_log(path)?))
}

// ---- projection ---------------------------------------------------------------------------

/// Fold a canvas log into the board. Pure and deterministic, like `crate::events::replay`.
///
/// SPIKE FINDING (kernel, held): the *shape* of replay — `events.iter().fold(Default::default())`,
/// mutate-by-id, ignore an event naming an absent id — transfers perfectly. What does not transfer
/// is that ES's `replay` ends in `normalize(&mut phases)`: a whole invariant-restoring pass over a
/// coordinate space the canvas does not have. The canvas's replay has **no post-pass at all**,
/// because a slot template has no invariant that a single event can break.
pub fn replay(events: &[CanvasEvent]) -> Canvas {
    let mut c = Canvas::default();
    for e in events {
        match e {
            CanvasEvent::CanvasNamed { name } => c.name = name.clone(),
            CanvasEvent::ItemAdded {
                id,
                slot,
                text,
                via,
            } => {
                if !c.items.iter().any(|i| &i.id == id) {
                    let mut item = Item::new(id, *slot, text);
                    item.via = via.clone();
                    c.items.push(item);
                }
            }
            CanvasEvent::ItemEdited { id, text } => {
                if let Some(i) = c.items.iter_mut().find(|i| &i.id == id) {
                    i.text = text.clone();
                }
            }
            CanvasEvent::ItemReslotted { id, slot } => {
                if let Some(i) = c.items.iter_mut().find(|i| &i.id == id) {
                    i.slot = *slot;
                }
            }
            CanvasEvent::ItemRemoved { id } => c.items.retain(|i| &i.id != id),
            CanvasEvent::LogCompacted { .. } => {}
        }
    }
    c
}

/// Genesis: turn a bootstrap `*.canvas.json` into the log that replays to it.
pub fn from_canvas(c: &Canvas) -> Vec<CanvasEvent> {
    let mut out = vec![CanvasEvent::CanvasNamed {
        name: c.name.clone(),
    }];
    for i in &c.items {
        out.push(CanvasEvent::ItemAdded {
            id: i.id.clone(),
            slot: i.slot,
            text: i.text.clone(),
            via: i.via.clone(),
        });
    }
    out
}

/// Fold a log to a `LogCompacted` marker + a genesis batch. Structurally identical to
/// `crate::events::compact` — which is itself evidence that *compact is generic*: it is
/// `from_<board>(replay(log))` prefixed by a marker, for any format.
pub fn compact(events: &[CanvasEvent]) -> Vec<CanvasEvent> {
    let mut out = vec![CanvasEvent::LogCompacted {
        folded: events.len() as i64,
    }];
    out.extend(from_canvas(&replay(events)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> Vec<CanvasEvent> {
        vec![
            CanvasEvent::CanvasNamed {
                name: "Orders".into(),
            },
            CanvasEvent::ItemAdded {
                id: "U1".into(),
                slot: Slot::Purpose,
                text: "accept and track orders".into(),
                via: None,
            },
            CanvasEvent::ItemAdded {
                id: "I1".into(),
                slot: Slot::Inbound,
                text: "PlaceOrder".into(),
                via: Some("Storefront".into()),
            },
            CanvasEvent::ItemEdited {
                id: "U1".into(),
                text: "accept, track and settle orders".into(),
            },
            CanvasEvent::ItemReslotted {
                id: "I1".into(),
                slot: Slot::Outbound,
            },
        ]
    }

    #[test]
    fn replay_folds_add_edit_and_reslot() {
        let c = replay(&log());
        assert_eq!(c.name, "Orders");
        assert_eq!(c.items[0].text, "accept, track and settle orders");
        assert_eq!(c.items[1].slot, Slot::Outbound, "reslot is the whole move");
        assert_eq!(c.items[1].via.as_deref(), Some("Storefront"));
    }

    #[test]
    fn an_event_naming_an_absent_id_is_a_no_op() {
        let c = replay(&[CanvasEvent::ItemEdited {
            id: "ghost".into(),
            text: "x".into(),
        }]);
        assert!(c.items.is_empty());
    }

    #[test]
    fn log_round_trips_through_jsonl() {
        let events = log();
        let back = parse_log(&to_jsonl(&events)).unwrap();
        assert_eq!(back, events);
    }

    #[test]
    fn compact_preserves_the_replayed_board() {
        let events = log();
        assert_eq!(replay(&compact(&events)), replay(&events));
    }

    #[test]
    fn a_malformed_known_event_is_fatal_but_an_unknown_kind_skips() {
        assert!(parse_log(r#"{"event":"ItemAdded","id":"U1"}"#).is_err());
        assert!(parse_log(r#"{"event":"ItemAdded","id":"U1","slot":"nope","text":"x"}"#).is_err());
        assert_eq!(parse_log(r#"{"event":"FromTheFuture"}"#).unwrap().len(), 0);
    }

    // ---- the cross-format finding ---------------------------------------------------------

    /// **SPIKE FINDING (dangerous).** Hand a canvas log to the event-storming reader and it does
    /// not fail — it returns `Ok` with an empty board, because "skip unknown kinds" is exactly how
    /// forward compatibility is specified. The reverse holds too. Nothing in the current design
    /// distinguishes "a log from a newer faceto" from "a log from a different format"; only a
    /// format tag can. `main`'s `warn_if_empty` would print a warning and render a blank board.
    #[test]
    fn a_canvas_log_replays_as_a_silent_empty_es_board() {
        let jsonl = to_jsonl(&log());
        let es = crate::events::parse_log(&jsonl).expect("no error — that is the finding");
        assert!(es.is_empty(), "every canvas line read as an unknown kind");
        assert!(crate::events::replay(&es).elements.is_empty());
    }

    /// And symmetrically: an ES log read as a canvas is an empty canvas, silently.
    #[test]
    fn an_es_log_replays_as_a_silent_empty_canvas() {
        let es = crate::events::to_jsonl(&crate::events::from_model(&crate::model::from_json(
            &json::parse(r#"{"title":"T","elements":[{"id":"E1","type":"event","label":"A"}]}"#)
                .unwrap(),
        )));
        let canvas = replay(&parse_log(&es).expect("no error — that is the finding"));
        assert!(canvas.items.is_empty());
        assert_eq!(
            canvas.name, "",
            "not even the title survives — kinds differ"
        );
    }
}
