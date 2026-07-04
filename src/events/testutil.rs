//! Shared `#[cfg(test)]` harness for the events suite: `ev` (parse one event line),
//! the `Lcg` PRNG, and the `gen_comment` / `genesis` generators the property-based
//! tests fold through the comment -> event -> replay pipeline.

use super::codec::parse_event;
use super::*;
use crate::json::{self, Json};

pub(crate) fn ev(line: &str) -> Event {
    parse_event(&json::parse(line).unwrap()).unwrap()
}

// ---- F-container: regions (Stage 1, the event spine) ---------------------------------
// A region is a labelled vertical band that evolves the legacy `Phase`. Membership and
// pivotal are derived from geometry (later stages), so the spine only needs: add with a
// stable id, resize, rename, remove — plus legacy bands (no id) replaying deterministically.

pub(crate) struct Lcg(pub(crate) u64);

impl Lcg {
    pub(crate) fn next_u64(&mut self) -> u64 {
        // Knuth MMIX LCG constants — full-period over u64.
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// A mix of real strings and blanks: the no-blank-label invariant is precisely that a blank
// rename can never empty a box, so the generator must reach for blanks on purpose.
const TEXTS: [&str; 6] = ["Paid", "ItemAdded", "  spaced  ", "", "   ", "\t"];
const KINDS: [&str; 5] = ["rename", "move", "drop", "comment", "resolve"];

// One random comment over the given element ids, plus a textual form for failure reports.
pub(crate) fn gen_comment(rng: &mut Lcg, ids: &[&str]) -> (Json, String) {
    let id = ids[rng.below(ids.len())];
    let kind = KINDS[rng.below(KINDS.len())];
    let text = TEXTS[rng.below(TEXTS.len())];
    let mut o = vec![
        ("elemId".to_string(), Json::Str(id.to_string())),
        ("kind".to_string(), Json::Str(kind.to_string())),
        ("text".to_string(), Json::Str(text.to_string())),
    ];
    if kind == "move" {
        o.push(("col".to_string(), Json::Num(rng.below(6) as f64)));
    }
    let v = Json::Obj(o);
    (v.clone(), json::to_string(&v))
}

// A small fixed board of non-blank elements, one per lane id-prefix used here.
pub(crate) fn genesis() -> (Vec<Event>, Vec<&'static str>) {
    let ids = vec!["E1", "E2", "C1", "A1", "H1"];
    let kinds = ["event", "event", "command", "aggregate", "hotspot"];
    let evs = ids
        .iter()
        .zip(kinds)
        .map(|(id, k)| Event::ElementAdded {
            id: (*id).to_string(),
            kind: k.to_string(),
            label: format!("seed-{id}"),
            col: Some(0),
            detail: None,
            y: None,
        })
        .collect();
    (evs, ids)
}
