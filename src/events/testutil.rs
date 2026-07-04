//! Shared `#[cfg(test)]` harness for the events suite: `ev` (parse one event line), the fixed
//! `genesis` board, and the `proptest` strategies (`comment_strategy` / `phase_log_strategy`) the
//! property-based tests fold through the comment/phase → event → replay pipeline. proptest replaces
//! the old hand-rolled `Lcg` seed loops: same generators, but it shrinks a failure to a minimal
//! reproducing input instead of dumping a raw seed.

use super::codec::parse_event;
use super::*;
use crate::json::{self, Json};
use proptest::prelude::*;

pub(crate) fn ev(line: &str) -> Event {
    parse_event(&json::parse(line).unwrap()).unwrap()
}

/// A small fixed board of non-blank elements, one per lane id-prefix used here — the base state
/// the comment property tests fold their generated comment sequences onto.
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

// A mix of real strings and blanks: the no-blank-label invariant is precisely that a blank rename
// can never empty a box, so the generator must reach for blanks on purpose.
const TEXTS: &[&str] = &["Paid", "ItemAdded", "  spaced  ", "", "   ", "\t"];
const COMMENT_KINDS: &[&str] = &["rename", "move", "drop", "comment", "resolve"];

/// One posted comment over the fixed `genesis` board. Picks an element id, a kind, and a text
/// (deliberately including blanks to stress the blank-label guard), plus a `col` used only by
/// `move`. Fold a `vec` of these through `comment_to_events` to drive the comment property tests.
pub(crate) fn comment_strategy() -> impl Strategy<Value = Json> {
    (
        prop::sample::select(vec!["E1", "E2", "C1", "A1", "H1"]),
        prop::sample::select(COMMENT_KINDS.to_vec()),
        prop::sample::select(TEXTS.to_vec()),
        0i64..6,
    )
        .prop_map(|(id, kind, text, col)| {
            let mut o = vec![
                ("elemId".to_string(), Json::Str(id.to_string())),
                ("kind".to_string(), Json::Str(kind.to_string())),
                ("text".to_string(), Json::Str(text.to_string())),
            ];
            if kind == "move" {
                o.push(("col".to_string(), Json::Num(col as f64)));
            }
            Json::Obj(o)
        })
}

/// One raw phase-op template. Ids can't depend on prior draws inside a pure strategy, so the
/// strategy emits these templates and [`build_phase_log`] threads the `minted` counter while it
/// turns them into events.
#[derive(Debug, Clone)]
pub(crate) struct PhaseOp {
    kind: u8,
    a: i64,
    b: i64,
    target: usize,
    edge_start: bool,
}

/// A log of `1..=12` phase events over a growing id space. Mixes legacy independent spans
/// (`PhaseAdded`/`PhaseResized`, which alone could gap or overlap), atomic frontier moves, splits,
/// and removes; an op on an id not yet minted is a valid no-op. Client-minted ids (`K1..`) are
/// distinct from the ones `replay` synthesizes. proptest shrinks a counterexample to the fewest,
/// simplest events that still break the partition.
pub(crate) fn phase_log_strategy() -> impl Strategy<Value = Vec<Event>> {
    let op = (0u8..5, -2i64..7, -2i64..7, 0usize..12, any::<bool>()).prop_map(
        |(kind, a, b, target, edge_start)| PhaseOp {
            kind,
            a,
            b,
            target,
            edge_start,
        },
    );
    prop::collection::vec(op, 1..=12).prop_map(build_phase_log)
}

fn build_phase_log(ops: Vec<PhaseOp>) -> Vec<Event> {
    let mut log = Vec::new();
    let mut minted = 0u32;
    for op in ops {
        // Target an already-minted id (K1..=K{minted}); before the first add, K1 is absent, so
        // the op replays as a no-op — the exact "ops on absent ids are valid no-ops" case.
        let target = format!("K{}", 1 + op.target % (minted.max(1) as usize));
        let (from, to) = (op.a.min(op.b), op.a.max(op.b));
        let ev = match op.kind {
            0 => {
                minted += 1;
                Event::PhaseAdded {
                    id: Some(format!("K{minted}")),
                    label: format!("p{minted}"),
                    from_col: from,
                    to_col: to,
                }
            }
            1 => Event::PhaseResized {
                id: target,
                from_col: from,
                to_col: to,
            },
            2 => Event::FrontierMoved {
                id: target,
                edge: if op.edge_start { "start" } else { "end" }.into(),
                col: op.a,
            },
            3 => {
                minted += 1;
                Event::PhaseSplit {
                    id: target,
                    at_col: op.a,
                    new_id: format!("K{minted}"),
                    new_label: format!("s{minted}"),
                }
            }
            _ => Event::PhaseRemoved { id: target },
        };
        log.push(ev);
    }
    log
}
