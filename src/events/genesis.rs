//! Derive events from an existing model ([`from_model`], the genesis/migration path) and fold
//! a log to a shorter equivalent snapshot ([`compact`]).

use super::replay::replay;
use super::Event;
use crate::model::{Level, Model};

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
    // Emit the level only when it differs from the default, mirroring the title guard: a
    // big-picture (default) board writes no `BoardLeveled`, so its genesis batch is byte-identical
    // to before this field existed and round-trips unchanged. Guarding on `!= default` (not
    // `== Design`) means any future non-default level is emitted too, via the exhaustive
    // `level_to_str` — no variant silently round-trips as the default.
    if m.level != Level::default() {
        ev.push(Event::BoardLeveled {
            level: crate::model::level_to_str(m.level).into(),
        });
    }
    for p in &m.phases {
        ev.push(Event::PhaseAdded {
            id: Some(p.id.clone()),
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
            y: e.y,
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
