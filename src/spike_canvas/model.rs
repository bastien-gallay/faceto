//! The Bounded Context Canvas board: a **slot template**.
//!
//! Contrast with `crate::model::Model`, which is a timeline × lane grid. Here there is exactly
//! one placement concept — *which named section an item sits in* — and it is a closed set, not a
//! coordinate. Nothing sorts. Nothing has an x. Nothing has a y.

use crate::json::{self, Json};
use std::path::Path;

/// The canvas's fixed sections, in render order. A **closed** set: the template *is* the format.
///
/// SPIKE FINDING (kernel): this is the shape `crate::model::Element.kind` should have had — the
/// ES lane is also a closed 8-set, but it is stored as `String` and re-parsed with a `_ =>`
/// fallback in `colour`/`lane_index`. Writing the second format enum-first cost nothing and made
/// `slot_from_str`/`slot_key` total. See `docs/multi-format-architecture.md` §Type discipline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Purpose,
    Classification,
    Roles,
    Inbound,
    Outbound,
    Language,
    Decisions,
    Assumptions,
    Metrics,
    Questions,
}

/// Every slot, in canvas reading order. The canvas equivalent of `render::style::LANES` — and the
/// one structure the two formats genuinely share the *shape* of (an ordered closed vocabulary).
pub const SLOTS: [Slot; 10] = [
    Slot::Purpose,
    Slot::Classification,
    Slot::Roles,
    Slot::Inbound,
    Slot::Outbound,
    Slot::Language,
    Slot::Decisions,
    Slot::Assumptions,
    Slot::Metrics,
    Slot::Questions,
];

impl Slot {
    /// The wire string. Total by construction — no fallback arm, unlike `render::style::colour`.
    pub fn key(self) -> &'static str {
        match self {
            Slot::Purpose => "purpose",
            Slot::Classification => "classification",
            Slot::Roles => "roles",
            Slot::Inbound => "inbound",
            Slot::Outbound => "outbound",
            Slot::Language => "language",
            Slot::Decisions => "decisions",
            Slot::Assumptions => "assumptions",
            Slot::Metrics => "metrics",
            Slot::Questions => "questions",
        }
    }

    /// The human section title painted on the board.
    pub fn title(self) -> &'static str {
        match self {
            Slot::Purpose => "Purpose",
            Slot::Classification => "Strategic Classification",
            Slot::Roles => "Domain Roles",
            Slot::Inbound => "Inbound Communication",
            Slot::Outbound => "Outbound Communication",
            Slot::Language => "Ubiquitous Language",
            Slot::Decisions => "Business Decisions",
            Slot::Assumptions => "Assumptions",
            Slot::Metrics => "Verification Metrics",
            Slot::Questions => "Open Questions",
        }
    }

    /// The id-mint prefix for items in this slot — the canvas's `LANE_PREFIXES`.
    ///
    /// SPIKE FINDING (serve): the *mechanism* (highest-suffix-under-lock, `serve::ids::mint_id`)
    /// is format-agnostic and would be reused verbatim; only this table is format-owned. That
    /// half of `serve` needs no change at all.
    pub fn prefix(self) -> char {
        match self {
            Slot::Purpose => 'U',
            Slot::Classification => 'S',
            Slot::Roles => 'D',
            Slot::Inbound => 'I',
            Slot::Outbound => 'O',
            Slot::Language => 'L',
            Slot::Decisions => 'B',
            Slot::Assumptions => 'A',
            Slot::Metrics => 'M',
            Slot::Questions => 'Q',
        }
    }
}

/// Parse a wire slot string. `None` for an unknown section — the canvas equivalent of an
/// off-grammar `type`, and unlike the ES path it is *forced* to be handled at the boundary.
pub fn slot_from_str(s: &str) -> Option<Slot> {
    SLOTS.iter().copied().find(|sl| sl.key() == s)
}

/// One entry inside a section. Identity is the stable `id`, exactly as in event storming.
///
/// SPIKE FINDING (kernel, held): stable-id identity is genuinely format-agnostic. It is the only
/// piece of `crate::model` that transferred without argument.
#[derive(Clone, PartialEq, Debug)]
pub struct Item {
    pub id: String,
    pub slot: Slot,
    pub text: String,
    /// The collaborating context, for `Inbound`/`Outbound` items only (BCC draws these as
    /// "message ← collaborator"). Meaningless in the other eight slots.
    ///
    /// SPIKE FINDING (kernel): a slot-conditional field is the canvas's version of ES's
    /// "`resolved` only means something on a hotspot". Both formats grow *per-slot* optional
    /// fields, so a shared `Item`/`Element` product type would accumulate the union of both — an
    /// argument for the sealed `enum Board` over a shared element type.
    pub via: Option<String>,
    // diff annotation (not in the file): added / removed / changed / reslotted / unchanged.
    pub diff: Option<String>,
    /// The previous slot, on a `reslotted` verdict.
    pub was_slot: Option<Slot>,
    /// The previous text, on a `changed` verdict.
    pub was_text: Option<String>,
}

impl Item {
    pub fn new(id: &str, slot: Slot, text: &str) -> Item {
        Item {
            id: id.to_string(),
            slot,
            text: text.to_string(),
            via: None,
            diff: None,
            was_slot: None,
            was_text: None,
        }
    }
}

/// The board. No `phases`, no `edges`, no `level`, no ordering coordinate of any kind.
///
/// SPIKE FINDING (kernel, broke): `crate::model::Model` carries `diff` / `was` / `status`
/// optionals so one type means both "the board" and "a diff overlay". I reproduced that mistake
/// here (`Item.diff`) *on purpose*, to see whether it hurts more or less without positions — it
/// hurts the same. `diff_meta` below is the same smell. Splitting board from overlay is a
/// pre-requisite for either format, not a multi-format concern.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Canvas {
    pub name: String,
    pub items: Vec<Item>,
    pub diff_meta: Option<(String, String)>,
}

impl Canvas {
    /// The items of one section, in file order. The canvas's entire layout query — compare
    /// `render::svg`'s col→x arithmetic plus `cell_sub_order` plus `y_key`.
    pub fn slot_items(&self, slot: Slot) -> Vec<&Item> {
        self.items.iter().filter(|i| i.slot == slot).collect()
    }
}

pub fn load(path: &Path) -> Result<Canvas, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    Ok(from_json(&json::parse(&raw)?))
}

/// Parse a `*.canvas.json` bootstrap file. Same lenient posture as `model::from_json`: a malformed
/// item is dropped, never fatal.
///
/// Shape:
/// ```json
/// { "name": "Orders", "slots": { "purpose": [ {"id":"U1","text":"…"} ], … } }
/// ```
pub fn from_json(j: &Json) -> Canvas {
    let name = j.get_str("name").unwrap_or("canvas").to_string();
    let mut items = Vec::new();
    if let Some(Json::Obj(pairs)) = j.get("slots") {
        // Iterate SLOTS, not the file, so render order is the template's, not the author's.
        for slot in SLOTS {
            let Some((_, arr)) = pairs.iter().find(|(k, _)| k == slot.key()) else {
                continue;
            };
            let Some(arr) = arr.as_array() else { continue };
            for entry in arr {
                if let Some(item) = item_from(entry, slot) {
                    items.push(item);
                }
            }
        }
    }
    Canvas {
        name,
        items,
        diff_meta: None,
    }
}

fn item_from(j: &Json, slot: Slot) -> Option<Item> {
    Some(Item {
        id: j.get_str("id")?.to_string(),
        slot,
        text: j.get_str("text")?.to_string(),
        via: j.get_str("via").map(String::from),
        diff: None,
        was_slot: None,
        was_text: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_key_round_trips_and_is_total() {
        for slot in SLOTS {
            assert_eq!(slot_from_str(slot.key()), Some(slot));
        }
        assert_eq!(slot_from_str("event"), None, "an ES lane is not a slot");
    }

    #[test]
    fn slot_prefixes_are_distinct() {
        let mut seen: Vec<char> = SLOTS.iter().map(|s| s.prefix()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), SLOTS.len(), "prefixes collide");
    }

    #[test]
    fn from_json_reads_slots_in_template_order_not_file_order() {
        // `questions` is declared first in the file but renders last: the template owns the order,
        // so there is nothing for the format to sort by — no `col`, no stable-sort subtlety.
        let c = from_json(
            &json::parse(
                r#"{"name":"Orders","slots":{
                    "questions":[{"id":"Q1","text":"who owns refunds?"}],
                    "purpose":[{"id":"U1","text":"accept and track orders"}],
                    "inbound":[{"id":"I1","text":"PlaceOrder","via":"Storefront"}]}}"#,
            )
            .unwrap(),
        );
        let order: Vec<&str> = c.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(order, vec!["U1", "I1", "Q1"]);
        assert_eq!(c.items[1].via.as_deref(), Some("Storefront"));
    }

    #[test]
    fn an_unknown_slot_and_a_malformed_item_drop_silently() {
        let c = from_json(
            &json::parse(
                r#"{"name":"X","slots":{
                    "not-a-slot":[{"id":"Z1","text":"ignored"}],
                    "purpose":[{"id":"U1"},{"text":"no id"},{"id":"U2","text":"ok"}]}}"#,
            )
            .unwrap(),
        );
        let ids: Vec<&str> = c.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["U2"]);
    }
}
