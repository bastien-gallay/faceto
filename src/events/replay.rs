//! The projection: fold a slice of [`Event`]s into a `Model` ([`replay`]), plus the region-id
//! watermark ([`region_watermark`]) the server reads when minting a fresh region id.

use super::Event;
use crate::model::{resolve_region_id, Edge, Element, Model, Phase};

/// The region-id watermark after `events`: the highest `K` suffix any `PhaseAdded` or `PhaseSplit`
/// has spent — explicit ids *and* the synthetic ones [`replay`] mints for legacy id-less bands —
/// folded through the same [`resolve_region_id`] tracker `replay` threads through its projection.
/// The single home of the region-id namespace rule, so a server-side mint can never hand out a
/// suffix `replay` would later synthesize (it just returns `watermark + 1`). `replay` keeps its own
/// running counter because it mints *in order while folding*; both fold the same kinds through
/// `resolve_region_id`, so they can't diverge.
pub fn region_watermark(events: &[Event]) -> u32 {
    let mut max_region = 0u32;
    for ev in events {
        match ev {
            Event::PhaseAdded { id, .. } => {
                resolve_region_id(id.as_deref(), &mut max_region);
            }
            Event::PhaseSplit { new_id, .. } => {
                resolve_region_id(Some(new_id), &mut max_region);
            }
            _ => {}
        }
    }
    max_region
}

/// Fold a sequence of events into the board they describe. The projection is pure and
/// deterministic: same log → same `Model`. (The `max_region` counter below is the running form of
/// [`region_watermark`]; both fold `PhaseAdded`/`PhaseSplit` through `resolve_region_id`.)
pub fn replay(events: &[Event]) -> Model {
    let mut m = Model::default();
    // Highest `K` region suffix seen so far — threaded across the fold so a synthetic id for a
    // legacy (id-less) band never reuses a suffix freed by `PhaseRemoved` or taken by an explicit
    // id. Mirrors `serve::mint_id`'s "highest ever added" rule (see `resolve_region_id`).
    let mut max_region = 0u32;
    for ev in events {
        match ev {
            Event::BoardTitled { title } => m.title = title.clone(),
            Event::BoardLeveled { level } => m.level = crate::model::level_from_str(level),
            Event::PhaseAdded {
                id,
                label,
                from_col,
                to_col,
            } => {
                // A legacy band carries no id; mint the next free `K<n>` past the highest ever
                // seen so resize/rename/remove can target it and no suffix is ever reused.
                let id = resolve_region_id(id.as_deref(), &mut max_region);
                // Idempotent by id, like `ElementAdded`: a duplicate `PhaseAdded` (a log appended
                // twice, or `from_model` of a model with non-unique phase ids — `normalize` never
                // dedups ids) must not push a second Phase sharing an id, or every later Phase*
                // event would resolve by id and address only the first, stranding a ghost region.
                // A minted (legacy id-less) id is fresh past the watermark, so this drops only true
                // duplicates of an explicit id.
                if !m.phases.iter().any(|p| p.id == id) {
                    m.phases.push(Phase {
                        id,
                        label: label.clone(),
                        from_col: *from_col,
                        to_col: *to_col,
                        diff: None,
                    });
                }
            }
            Event::PhaseResized {
                id,
                from_col,
                to_col,
            } => {
                if let Some(p) = m.phases.iter_mut().find(|p| &p.id == id) {
                    p.from_col = *from_col;
                    p.to_col = *to_col;
                }
            }
            Event::PhaseRenamed { id, label } => {
                if let Some(p) = m.phases.iter_mut().find(|p| &p.id == id) {
                    p.label = label.clone();
                }
            }
            Event::PhaseRemoved { id } => {
                // Remove = merge under the partition. An interior phase's freed columns are absorbed
                // by the neighbour `normalize` sweeps into; but a *board-end* phase would otherwise
                // shrink the board and strand its columns (region-less), contradicting "merge into
                // the neighbour". So when the removed phase held a board end, extend the outermost
                // survivor to cover its span — the neighbour absorbs it either way.
                if let Some(pos) = m.phases.iter().position(|p| &p.id == id) {
                    let rem = m.phases.remove(pos);
                    if !m.phases.is_empty() {
                        if rem.from_col <= m.phases.iter().map(|p| p.from_col).min().unwrap() {
                            let lo = m.phases.iter_mut().min_by_key(|p| p.from_col).unwrap();
                            lo.from_col = lo.from_col.min(rem.from_col);
                        }
                        if rem.to_col >= m.phases.iter().map(|p| p.to_col).max().unwrap() {
                            let hi = m.phases.iter_mut().max_by_key(|p| p.to_col).unwrap();
                            hi.to_col = hi.to_col.max(rem.to_col);
                        }
                    }
                }
            }
            Event::FrontierMoved { id, edge, col } => {
                // Set the named border; `normalize` (after the fold) re-borders the neighbour so the
                // partition stays gap-free. `"start"` moves only the board-left bound — the current
                // leftmost phase's `from_col`. Applying it to any other phase would change
                // `normalize`'s sort key and *reorder* the timeline, so restrict it to the leftmost
                // (a stray `"start"` on any other phase is then a true no-op, not a silent reorder).
                if edge == "start" {
                    let leftmost = m.phases.iter().map(|p| p.from_col).min();
                    if let (Some(min), Some(p)) =
                        (leftmost, m.phases.iter_mut().find(|p| &p.id == id))
                    {
                        if p.from_col == min {
                            p.from_col = *col;
                        }
                    }
                } else if let Some(p) = m.phases.iter_mut().find(|p| &p.id == id) {
                    p.to_col = *col;
                }
            }
            Event::PhaseSplit {
                id,
                at_col,
                new_id,
                new_label,
            } => {
                // Feed `new_id` through the same id-namespace tracker as `PhaseAdded` so a later
                // legacy (id-less) band never reuses this suffix.
                let new_id = resolve_region_id(Some(new_id), &mut max_region);
                if let Some(i) = m.phases.iter().position(|p| &p.id == id) {
                    let (from, to) = (m.phases[i].from_col, m.phases[i].to_col);
                    // Strictly inside → both halves keep ≥1 column. Otherwise nothing to split.
                    if from < *at_col && *at_col <= to {
                        m.phases[i].to_col = *at_col - 1;
                        // Placement doesn't matter — `normalize` (end of the fold) sorts by column,
                        // so the new right half lands in order regardless; just push it.
                        m.phases.push(Phase {
                            id: new_id,
                            label: new_label.clone(),
                            from_col: *at_col,
                            to_col: to,
                            diff: None,
                        });
                    }
                }
            }
            Event::ElementAdded {
                id,
                kind,
                label,
                col,
                detail,
                y,
            } => {
                if !m.elements.iter().any(|e| &e.id == id) {
                    m.elements.push(Element {
                        id: id.clone(),
                        kind: kind.clone(),
                        label: label.clone(),
                        col: *col,
                        detail: detail.clone(),
                        y: *y,
                        resolved: false,
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
            Event::ElementMoved { id, col, kind, y } => {
                if let Some(e) = find(&mut m, id) {
                    if col.is_some() {
                        e.col = *col;
                    }
                    if let Some(k) = kind {
                        e.kind = k.clone();
                    }
                    if y.is_some() {
                        e.y = *y;
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
    // Regions are a *contiguous partition* of the timeline (F-region-frontiers): after folding every
    // phase event — new frontier moves/splits *and* legacy independent spans (`PhaseAdded`/
    // `PhaseResized`, which could leave holes or overlaps) — project the phase list to a gap-free,
    // overlap-free partition. The rule (`model::normalize`) is pure and deterministic, so `replay`
    // stays a pure function; it is shared with `from_json` so every `Model` is a partition whatever
    // its source (log or bootstrap `model.json`).
    crate::model::normalize(&mut m.phases);
    m
}

fn find<'a>(m: &'a mut Model, id: &str) -> Option<&'a mut Element> {
    m.elements.iter_mut().find(|e| e.id == id)
}
