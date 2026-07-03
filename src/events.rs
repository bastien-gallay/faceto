//! The event-sourced spine: an append-only log is the durable record; the `Model` is a
//! projection replayed from it.
//!
//! This inverts the older "model file is truth, comments are a disposable inbox" stance
//! (see `docs/source-of-truth.md`). Here `event-log.jsonl` is the only durable record and
//! the only write path; `model.json` becomes derived output. Each log line is one JSON
//! object discriminated by an `"event"` field; [`replay`] folds a sequence into a `Model`,
//! and [`from_model`] turns an existing model file into a genesis batch (the migration and
//! bootstrap path).
//!
//! **Schema evolution (H3).** The on-disk schema is allowed to grow over time, and an old log
//! must still replay. The rules:
//! - *Additive change is free.* A new optional field is simply not read by older code, and a
//!   wholly new event kind is skipped on read ([`parse_log`]). Neither breaks an old or a new log,
//!   so this is the preferred way to extend. *Fields* evolve only this way: a renamed field is, by
//!   shape, indistinguishable from a new optional one, so add the new name and keep reading the old.
//! - *A renamed event kind is migrated forward at one seam.* Renaming a kind is the
//!   backward-incompatible change [`upcast`] repairs: it is the single place a legacy kind string
//!   is rewritten to today's, so the rest of the pipeline only ever sees current kinds. Detection
//!   is by shape (the old kind's presence), not a stored version counter — an old log replays with
//!   nothing to set.
//! - *A kind's meaning is never silently repurposed.* If semantics must change, introduce a new
//!   kind (additive) and upcast the old one; never redefine an existing kind in place.

use crate::json::{self, Json};
use crate::model::{resolve_region_id, Edge, Element, Model, Phase};
use std::borrow::Cow;
use std::path::Path;

/// One fact in the log. Variants mirror the board operations a session performs; the
/// `Element*` variants all carry the stable `id` (identity is never text or position).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    BoardTitled {
        title: String,
    },
    PhaseAdded {
        /// Stable region id. `None` on a legacy band (predates region editing); `replay` mints a
        /// deterministic positional id so old logs stay replayable without an `upcast` (additive
        /// field, not a renamed kind).
        id: Option<String>,
        label: String,
        from_col: i64,
        to_col: i64,
    },
    PhaseResized {
        id: String,
        from_col: i64,
        to_col: i64,
    },
    PhaseRenamed {
        id: String,
        label: String,
    },
    PhaseRemoved {
        id: String,
    },
    /// Move one border of a phase (F-region-frontiers). `edge` is `"start"` (the phase's
    /// `from_col`) or `"end"` (its `to_col`); `col` is the new position. Unlike `PhaseResized`
    /// (which set *both* borders independently, the span model that allowed holes/overlaps), a
    /// frontier move sets one border and `replay`'s `normalize` re-borders the neighbour
    /// atomically — the partition can never open a gap. An internal frontier between phases A (left)
    /// and B (right) is always posted as A's `"end"`; only the board's leftmost frontier moves a
    /// `"start"` (the first phase's `from_col`, which `normalize` preserves as the board-left bound).
    FrontierMoved {
        id: String,
        edge: String,
        col: i64,
    },
    /// Split a phase in two at `at_col` (F-region-frontiers, the partition's "add"): `id` keeps
    /// `[from_col, at_col - 1]` and a new phase `new_id` takes `[at_col, to_col]` with `new_label`.
    /// `new_id` is minted server-side (like `PhaseAdded` on `region-add`). A no-op unless the column
    /// falls strictly inside the phase (`from_col < at_col <= to_col`), so both halves stay ≥1 wide.
    PhaseSplit {
        id: String,
        at_col: i64,
        new_id: String,
        new_label: String,
    },
    ElementAdded {
        id: String,
        kind: String,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
        /// Stored vertical sub-position within the lane band (F-2d-placement): a fraction of the
        /// band interior in `[0, 1]`, `None` = auto-stacked. Carried on add so `compact`/genesis
        /// round-trips a placed board (additive field — an old log simply never has it).
        y: Option<f64>,
    },
    ElementRenamed {
        id: String,
        label: String,
    },
    ElementMoved {
        id: String,
        col: Option<i64>,
        kind: Option<String>,
        /// New vertical sub-position (fraction of the lane-band interior, `[0, 1]`). `None`
        /// leaves the stored sub-position untouched — a col-only nudge never resets the Y.
        y: Option<f64>,
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

/// Iterate the meaningful records of JSONL text: each non-blank line, trimmed, paired with its
/// 1-based line number. The single place the line grammar (skip blanks, trim) lives, shared by
/// the log reader ([`parse_log`]) and the comments fold ([`from_comments`]) so the two never drift.
fn jsonl_records(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty())
}

/// Parse JSONL text into events. Blank lines are skipped; a line that does not parse as
/// JSON is a hard error (the log is the source of truth); a well-formed object whose
/// `"event"` is unknown is skipped (forward compatibility across schema versions).
pub fn parse_log(text: &str) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    for (n, line) in jsonl_records(text) {
        let j = json::parse(line).map_err(|e| format!("event-log line {}: {}", n, e))?;
        if let Some(ev) = parse_event(&j) {
            events.push(ev);
        }
    }
    Ok(events)
}

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

/// Fold a sequence of events into the board they describe. The projection is pure and
/// deterministic: same log → same `Model`.
pub fn replay(events: &[Event]) -> Model {
    let mut m = Model::default();
    // Highest `K` region suffix seen so far — threaded across the fold so a synthetic id for a
    // legacy (id-less) band never reuses a suffix freed by `PhaseRemoved` or taken by an explicit
    // id. Mirrors `serve::mint_id`'s "highest ever added" rule (see `resolve_region_id`).
    let mut max_region = 0u32;
    for ev in events {
        match ev {
            Event::BoardTitled { title } => m.title = title.clone(),
            Event::PhaseAdded {
                id,
                label,
                from_col,
                to_col,
            } => {
                // A legacy band carries no id; mint the next free `K<n>` past the highest ever
                // seen so resize/rename/remove can target it and no suffix is ever reused.
                let id = resolve_region_id(id.as_deref(), &mut max_region);
                m.phases.push(Phase {
                    id,
                    label: label.clone(),
                    from_col: *from_col,
                    to_col: *to_col,
                    diff: None,
                });
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
            Event::PhaseRemoved { id } => m.phases.retain(|p| &p.id != id),
            Event::FrontierMoved { id, edge, col } => {
                // Set the named border; `normalize` (after the fold) re-borders the neighbour so
                // the partition stays gap-free. A `"start"` on a non-leftmost phase is harmless —
                // `normalize` derives every inner `from_col` from the sweep and only honours the
                // very first phase's start, so such a raw post simply no-ops.
                if let Some(p) = m.phases.iter_mut().find(|p| &p.id == id) {
                    if edge == "start" {
                        p.from_col = *col;
                    } else {
                        p.to_col = *col;
                    }
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
                        m.phases.insert(
                            i + 1,
                            Phase {
                                id: new_id,
                                label: new_label.clone(),
                                from_col: *at_col,
                                to_col: to,
                                diff: None,
                            },
                        );
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

/// A label with content: the string trimmed, or `None` when it is blank. The one place the
/// "a label must carry content" rule lives — a blank one would mint or rename into a permanent,
/// never-renumbered empty box. Shared by the `add` guard (`serve.rs`) and the `rename` guard in
/// [`comment_to_events`], so direct on-board editing and a raw POST obey the same invariant.
pub fn nonblank(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// A region span with content: `from_col < to_col`, or `false` for an inverted or zero-width
/// span. An inverted span would mint or resize into a region `region_of`'s `from_col <= col &&
/// col <= to_col` test can never match — a permanent, silent gap between the model (which drops
/// the region from every column's membership) and the render, which normalizes the span and
/// draws it as a real band regardless. Shared by the `region-add` guard (`serve.rs`) and the
/// `region-resize` guard in [`region_comment_to_events`], so a raw POST can't create what the
/// other can't.
pub fn valid_span(from_col: i64, to_col: i64) -> bool {
    from_col < to_col
}

/// Normalise a posted vertical sub-position to its stored form: clamped into `[0, 1]` and
/// rounded to 4 decimals so the log carries a clean human-readable number, not a float's full
/// noise. This is the **write-seam** half of the rule; the **read** half — how a stored `y`
/// (or its absence) is interpreted as an ordering key — is `model::y_key`.
pub fn clamp_y(y: f64) -> f64 {
    (y.clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0
}

/// Map one posted/stored comment object to the event(s) it persists — the single source of
/// truth for the comment→event translation, shared by the live server (`POST /comment` in log
/// mode) and the `comments.jsonl` migration ([`from_comments`]). `move`/`resolve`/`rename`/`drop`
/// carry structural intent and fold straight into the projection; `split`/`question`/`comment`
/// stay advisory annotations. A `move` that displaces an occupant — the client sends
/// `swapId`/`swapCol` — yields **two** `ElementMoved`s so the swap round-trips. Returns an empty
/// vec when the comment names no element, when a `move` carries no target col, or when a `rename`
/// carries a blank label (all would replay as no-ops or corrupt the board): the caller treats that
/// as "nothing to persist".
///
/// Region edits (`region-resize`/`region-rename`/`region-remove`) key off `regionId` instead of
/// `elemId` and are dispatched to [`region_comment_to_events`] before the element path runs.
/// `region-add` is **not** handled here — like the element `add`, it needs a server-minted id and
/// is special-cased in `serve.rs` (`add_region_from_comment`).
pub fn comment_to_events(v: &Json) -> Vec<Event> {
    let kind = v.get_str("kind").unwrap_or("comment");
    if matches!(
        kind,
        "region-resize" | "region-rename" | "region-remove" | "frontier-move"
    ) {
        return region_comment_to_events(v, kind);
    }
    let Some(id) = v.get_str("elemId").map(str::to_string) else {
        return Vec::new();
    };
    let text = v.get_str("text").unwrap_or("").to_string();
    match kind {
        "move" => {
            // A move relocates along the timeline (`col`) and/or within the lane band (`y`,
            // F-2d-placement). Carrying neither would replay as a no-op, so reject it (empty
            // vec) rather than logging a phantom move.
            let col = v.get_i64("col");
            let y = v
                .get("y")
                .and_then(Json::as_f64)
                .filter(|y| y.is_finite())
                .map(clamp_y);
            if col.is_none() && y.is_none() {
                return Vec::new();
            }
            let mut evs = vec![Event::ElementMoved {
                id: id.clone(),
                col,
                kind: None,
                y,
            }];
            // A swap also relocates the displaced sticky — but only a *different* one, to a real
            // col. Guard against a self-swap or a swap missing its target col (would no-op).
            // Kept for old clients / stashed offline moves; the 2D client no longer swaps.
            if let (Some(swap_id), Some(swap_col)) = (v.get_str("swapId"), v.get_i64("swapCol")) {
                if swap_id != id.as_str() {
                    evs.push(Event::ElementMoved {
                        id: swap_id.to_string(),
                        col: Some(swap_col),
                        kind: None,
                        y: None,
                    });
                }
            }
            evs
        }
        "resolve" => vec![Event::HotspotResolved {
            id,
            resolution: text,
        }],
        "rename" => match nonblank(&text) {
            Some(label) => vec![Event::ElementRenamed { id, label }],
            None => Vec::new(),
        },
        "drop" => vec![Event::ElementRemoved { id }],
        _ => vec![Event::ElementAnnotated { id, text }],
    }
}

/// The region half of [`comment_to_events`]: `region-resize`/`region-rename`/`region-remove`,
/// keyed by `regionId` rather than `elemId` (a region is not an element). Returns an empty vec
/// when the comment names no region, when a `region-resize` carries no `[fromCol, toCol]` span or
/// an inverted/zero-width one (`valid_span`), or when a `region-rename` carries a blank label —
/// same no-op guards as the element path, so a malformed post never replays as a phantom edit.
fn region_comment_to_events(v: &Json, kind: &str) -> Vec<Event> {
    let Some(id) = v.get_str("regionId").map(str::to_string) else {
        return Vec::new();
    };
    match kind {
        // Legacy independent-span resize (old clients / stashed offline / `comments.jsonl`
        // migration). The live client posts `frontier-move` instead; either way `normalize`
        // projects the result onto a contiguous partition.
        "region-resize" => match (v.get_i64("fromCol"), v.get_i64("toCol")) {
            (Some(from_col), Some(to_col)) if valid_span(from_col, to_col) => {
                vec![Event::PhaseResized {
                    id,
                    from_col,
                    to_col,
                }]
            }
            _ => Vec::new(),
        },
        // Move one frontier (F-region-frontiers resize): set the named border, `replay`'s
        // `normalize` re-borders the neighbour. A missing/unknown `edge` or `col` is a no-op —
        // same "nothing to persist" guard as the element path.
        "frontier-move" => match (v.get_str("edge"), v.get_i64("col")) {
            (Some(edge), Some(col)) if edge == "start" || edge == "end" => {
                vec![Event::FrontierMoved {
                    id,
                    edge: edge.to_string(),
                    col,
                }]
            }
            _ => Vec::new(),
        },
        "region-rename" => {
            let text = v.get_str("text").unwrap_or("");
            match nonblank(text) {
                Some(label) => vec![Event::PhaseRenamed { id, label }],
                None => Vec::new(),
            }
        }
        "region-remove" => vec![Event::PhaseRemoved { id }],
        _ => Vec::new(),
    }
}

/// Fold a legacy `comments.jsonl` into the events it represents — the answer to H5, the second
/// half of the migration story alongside [`from_model`]. Each non-blank line is one stored comment;
/// [`comment_to_events`] translates it. Unlike the log proper, the comments inbox was always a
/// *best-effort* sidecar, so a line that cannot be migrated is **skipped** (not a hard error) —
/// migrating disposable feedback must not abort on one stray line. Append the result after a
/// model's genesis batch: the batch mints the ids these comments reference, so replaying the two
/// together reconstructs the board *and* its annotations/resolutions/renames.
///
/// Returns the events **and the count of non-blank lines that produced none** — unparseable, not
/// an object, naming no element, or a kind that carries no board change (e.g. a legacy `add`, which
/// in non-log mode was only ever an inbox note and carries no `elemId` to attach to). The count
/// lets the caller report the loss instead of dropping those lines silently.
pub fn from_comments(text: &str) -> (Vec<Event>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for (_, line) in jsonl_records(text) {
        match json::parse(line) {
            Ok(v @ Json::Obj(_)) => {
                let evs = comment_to_events(&v);
                if evs.is_empty() {
                    skipped += 1;
                } else {
                    out.extend(evs);
                }
            }
            _ => skipped += 1,
        }
    }
    (out, skipped)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(line: &str) -> Event {
        parse_event(&json::parse(line).unwrap()).unwrap()
    }

    // ---- F-container: regions (Stage 1, the event spine) ---------------------------------
    // A region is a labelled vertical band that evolves the legacy `Phase`. Membership and
    // pivotal are derived from geometry (later stages), so the spine only needs: add with a
    // stable id, resize, rename, remove — plus legacy bands (no id) replaying deterministically.

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
    fn legacy_phase_without_id_replays_to_a_stable_positional_id() {
        // An old log's bands carry no id; replay must mint deterministic `K<n>` so resize/rename
        // can target them and two replays of the same log agree.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","label":"A","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseAdded","label":"B","fromCol":3,"toCol":5}"#),
        ];
        let m = replay(&evs);
        assert_eq!(
            m.phases.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["K1", "K2"]
        );
    }

    #[test]
    fn region_resize_rename_remove_fold_by_id() {
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"Old","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"Keep","fromCol":3,"toCol":4}"#),
            ev(r#"{"event":"PhaseResized","id":"K1","fromCol":0,"toCol":5}"#),
            ev(r#"{"event":"PhaseRenamed","id":"K1","label":"New"}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K2"}"#),
        ];
        let m = replay(&evs);
        assert_eq!(m.phases.len(), 1, "K2 removed");
        let k1 = &m.phases[0];
        assert_eq!(
            (k1.id.as_str(), k1.label.as_str(), k1.from_col, k1.to_col),
            ("K1", "New", 0, 5)
        );
    }

    #[test]
    fn synthetic_region_ids_never_reuse_a_freed_suffix() {
        // Regression: deriving the synthetic id from the live phase *count* would re-mint `K2`
        // after a removal. The id must come from the highest suffix ever seen, never reused —
        // the same reservation rule serve::mint_id uses for elements.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","label":"A","fromCol":0,"toCol":1}"#),
            ev(r#"{"event":"PhaseAdded","label":"B","fromCol":2,"toCol":3}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K1"}"#),
            ev(r#"{"event":"PhaseAdded","label":"C","fromCol":4,"toCol":5}"#),
        ];
        let m = replay(&evs);
        let ids: Vec<_> = m.phases.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["K2", "K3"],
            "the third add must be K3, not a reused K2"
        );
    }

    #[test]
    fn a_synthetic_id_skips_past_an_explicit_one() {
        // An explicit id raises the watermark, so a following legacy band mints past it.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K5","label":"Explicit","fromCol":0,"toCol":1}"#),
            ev(r#"{"event":"PhaseAdded","label":"Legacy","fromCol":2,"toCol":3}"#),
        ];
        let m = replay(&evs);
        assert_eq!(
            m.phases[1].id, "K6",
            "synthetic id mints one past the highest seen"
        );
    }

    #[test]
    fn region_ops_on_an_unknown_id_are_no_ops() {
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseRenamed","id":"K9","label":"ghost"}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K9"}"#),
        ];
        let m = replay(&evs);
        assert_eq!(m.phases.len(), 1);
        assert_eq!(m.phases[0].label, "A");
    }

    // ---- F-region-frontiers: the contiguous-partition spine ------------------------------
    // Regions are a partition, not independent spans. Frontier moves re-border a neighbour
    // atomically, split carves a phase in two, and `normalize` guarantees no log — new or legacy —
    // ever replays to a hole or an overlap.

    #[test]
    fn frontier_move_end_reborders_the_right_neighbour_atomically() {
        // Move the A|B frontier: posting only A's new `to_col`, `normalize` pulls B's `from_col`
        // with it. One event, both phases re-border — the partition can't open a gap.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"FrontierMoved","id":"K1","edge":"end","col":5}"#),
        ];
        let m = replay(&evs);
        let span = |i: usize| (m.phases[i].from_col, m.phases[i].to_col);
        assert_eq!(span(0), (0, 5), "A grew right to the new frontier");
        assert_eq!(span(1), (6, 7), "B's start followed — no gap, no overlap");
    }

    #[test]
    fn frontier_move_start_moves_the_board_left_bound() {
        // The outermost (leftmost) frontier is the first phase's `start`; moving it grows/shrinks
        // the whole board. `normalize` preserves that first `from_col` as the board-left anchor.
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"FrontierMoved","id":"K1","edge":"start","col":-2}"#),
        ];
        let m = replay(&evs);
        assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (-2, 3));
        assert_eq!((m.phases[1].from_col, m.phases[1].to_col), (4, 7));
    }

    #[test]
    fn phase_split_carves_a_phase_in_two_keeping_a_partition() {
        // Add = split. The original id keeps the left half, the minted id takes the right, the two
        // stay contiguous. `newId` also raises the region-id watermark (a later legacy band mints
        // past it).
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"Whole","fromCol":0,"toCol":5}"#),
            ev(r#"{"event":"PhaseSplit","id":"K1","atCol":3,"newId":"K2","newLabel":"Right"}"#),
        ];
        let m = replay(&evs);
        assert_eq!(m.phases.len(), 2);
        assert_eq!(
            (
                m.phases[0].id.as_str(),
                m.phases[0].from_col,
                m.phases[0].to_col
            ),
            ("K1", 0, 2),
            "original keeps the left half"
        );
        assert_eq!(
            (
                m.phases[1].id.as_str(),
                m.phases[1].label.as_str(),
                m.phases[1].from_col,
                m.phases[1].to_col
            ),
            ("K2", "Right", 3, 5),
            "new phase takes the right half, contiguous"
        );
    }

    #[test]
    fn phase_split_outside_the_phase_is_a_no_op() {
        // `at_col` must land strictly inside (from < at <= to) so both halves keep ≥1 column.
        let base = ev(r#"{"event":"PhaseAdded","id":"K1","label":"W","fromCol":0,"toCol":3}"#);
        for at in ["0", "4", "9"] {
            let split = ev(&format!(
                r#"{{"event":"PhaseSplit","id":"K1","atCol":{at},"newId":"K2","newLabel":"R"}}"#
            ));
            let m = replay(&[base.clone(), split]);
            assert_eq!(m.phases.len(), 1, "at_col={at} splits nothing");
            assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (0, 3));
        }
    }

    #[test]
    fn removing_a_middle_phase_leaves_no_hole() {
        // Remove = merge under the partition: the freed columns are absorbed by the neighbour that
        // sweeps into them, never a gap. (v1 folds directional merge into remove — see the
        // F-region-frontiers working note; PhaseMerged is deferred.)
        let evs = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":3}"#),
            ev(r#"{"event":"PhaseAdded","id":"K2","label":"B","fromCol":4,"toCol":7}"#),
            ev(r#"{"event":"PhaseAdded","id":"K3","label":"C","fromCol":8,"toCol":11}"#),
            ev(r#"{"event":"PhaseRemoved","id":"K2"}"#),
        ];
        let m = replay(&evs);
        let ids: Vec<_> = m.phases.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["K1", "K3"]);
        assert_eq!((m.phases[0].from_col, m.phases[0].to_col), (0, 3));
        assert_eq!(
            (m.phases[1].from_col, m.phases[1].to_col),
            (4, 11),
            "C absorbed B's freed columns — the partition stays gap-free"
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

    #[test]
    fn frontier_move_maps_from_a_comment_with_guards() {
        let mk = |body: &str| comment_to_events(&json::parse(body).unwrap());
        assert_eq!(
            mk(r#"{"kind":"frontier-move","regionId":"K1","edge":"end","col":5}"#),
            vec![Event::FrontierMoved {
                id: "K1".into(),
                edge: "end".into(),
                col: 5
            }]
        );
        assert!(
            mk(r#"{"kind":"frontier-move","regionId":"K1","edge":"sideways","col":5}"#).is_empty(),
            "an unknown edge is nothing to persist"
        );
        assert!(
            mk(r#"{"kind":"frontier-move","regionId":"K1","edge":"end"}"#).is_empty(),
            "a missing col is nothing to persist"
        );
    }

    #[test]
    fn from_model_emits_region_ids_so_genesis_round_trips() {
        // compact()/genesis fold the final state into PhaseAdded; the id must survive so a
        // compacted log keeps stable region identity.
        let log = vec![
            ev(r#"{"event":"PhaseAdded","id":"K1","label":"A","fromCol":0,"toCol":2}"#),
            ev(r#"{"event":"PhaseResized","id":"K1","fromCol":0,"toCol":9}"#),
        ];
        let folded = compact(&log);
        let m = replay(&folded);
        assert_eq!(m.phases[0].id, "K1");
        assert_eq!(m.phases[0].to_col, 9, "resize survives the fold");
    }

    // ---- F-inline-edit: a direct rename must not be able to blank a label -----------------
    // Inline editing makes "select-all → delete → Enter" a one-gesture mistake. A blank rename
    // must persist nothing (an empty label would replay into a never-renumbered empty box — the
    // exact failure the `add` path already guards). These name the contract before it exists.

    #[test]
    fn rename_with_a_blank_label_is_rejected() {
        for blank in ["", "   ", "\t", "\n  "] {
            let v = json::parse(&format!(
                r#"{{"elemId":"E1","kind":"rename","text":{:?}}}"#,
                blank
            ))
            .unwrap();
            assert!(
                comment_to_events(&v).is_empty(),
                "blank rename {:?} should persist nothing",
                blank
            );
        }
    }

    #[test]
    fn rename_trims_surrounding_whitespace() {
        let v =
            json::parse(r#"{"elemId":"E1","kind":"rename","text":"  PaymentTaken  "}"#).unwrap();
        let evs = comment_to_events(&v);
        assert!(
            matches!(&evs[..], [Event::ElementRenamed { id, label }] if id == "E1" && label == "PaymentTaken"),
            "got {:?}",
            evs
        );
    }

    #[test]
    fn rename_with_real_text_still_renames() {
        // Non-regression: a genuine rename is unchanged by the new guard.
        let v = json::parse(r#"{"elemId":"E1","kind":"rename","text":"Reborn"}"#).unwrap();
        let evs = comment_to_events(&v);
        assert!(matches!(&evs[..], [Event::ElementRenamed { id, label }]
            if id == "E1" && label == "Reborn"));
    }

    // ---- Property-based tests (std-only, hand-rolled) -------------------------------------
    // faceto takes no crates (CLAUDE.md: zero dependencies), so there is no proptest/quickcheck.
    // A tiny deterministic LCG drives reproducible random scenarios — each seed is one case, and
    // a failure prints the seed + the offending comment sequence so it replays exactly.

    struct Lcg(u64);
    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            // Knuth MMIX LCG constants — full-period over u64.
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }
    }

    // A mix of real strings and blanks: the no-blank-label invariant is precisely that a blank
    // rename can never empty a box, so the generator must reach for blanks on purpose.
    const TEXTS: [&str; 6] = ["Paid", "ItemAdded", "  spaced  ", "", "   ", "\t"];
    const KINDS: [&str; 5] = ["rename", "move", "drop", "comment", "resolve"];

    // One random comment over the given element ids, plus a textual form for failure reports.
    fn gen_comment(rng: &mut Lcg, ids: &[&str]) -> (Json, String) {
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
    fn genesis() -> (Vec<Event>, Vec<&'static str>) {
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

    #[test]
    fn pbt_no_comment_sequence_ever_leaves_a_blank_label() {
        // Property: folding any sequence of comment objects through `comment_to_events` and
        // replaying never yields an element whose label is blank. RED today — a blank rename
        // overwrites the label with "".
        for seed in 0..500u64 {
            let mut rng = Lcg(seed.wrapping_mul(2_654_435_761).wrapping_add(1));
            let (mut log, ids) = genesis();
            let n = 1 + rng.below(8);
            let mut trace = Vec::new();
            for _ in 0..n {
                let (v, shown) = gen_comment(&mut rng, &ids);
                trace.push(shown);
                log.extend(comment_to_events(&v));
            }
            let model = replay(&log);
            for e in &model.elements {
                assert!(
                    !e.label.trim().is_empty(),
                    "seed {seed}: element {} got a blank label after:\n  {}",
                    e.id,
                    trace.join("\n  ")
                );
            }
        }
    }

    #[test]
    fn pbt_comments_never_invent_an_element_and_only_drop_removes() {
        // Non-regression over the adjacent move/rename/annotate/resolve arms: none of them may
        // create or destroy an element — only `drop` removes, and nothing adds. Guards the move
        // path this feature sits next to.
        for seed in 0..500u64 {
            let mut rng = Lcg(seed.wrapping_mul(40_503).wrapping_add(7));
            let (mut log, ids) = genesis();
            let mut dropped = std::collections::HashSet::new();
            let n = 1 + rng.below(8);
            for _ in 0..n {
                let (v, _) = gen_comment(&mut rng, &ids);
                if v.get_str("kind") == Some("drop") {
                    if let Some(id) = v.get_str("elemId") {
                        dropped.insert(id.to_string());
                    }
                }
                log.extend(comment_to_events(&v));
            }
            let model = replay(&log);
            let present: std::collections::HashSet<&str> =
                model.elements.iter().map(|e| e.id.as_str()).collect();
            // No phantom creation: every surviving id was a genesis id.
            for id in &present {
                assert!(ids.contains(id), "seed {seed}: invented element {id}");
            }
            // Exactly the non-dropped genesis ids survive.
            for id in &ids {
                let want = !dropped.contains(*id);
                assert_eq!(
                    present.contains(id),
                    want,
                    "seed {seed}: element {id} present={} but dropped={}",
                    present.contains(id),
                    dropped.contains(*id)
                );
            }
        }
    }

    #[test]
    fn pbt_phase_events_never_replay_to_a_hole_or_overlap() {
        // Property (F-region-frontiers): fold any interleaving of phase events — legacy independent
        // spans (`PhaseAdded`/`PhaseResized`, which alone could gap or overlap), atomic frontier
        // moves, splits, and removes — and the replayed phases are always a *contiguous partition*:
        // sorted, gap-free, overlap-free, each ≥1 column wide. And `normalize` is its own fixed
        // point (a second pass changes nothing).
        for seed in 0..800u64 {
            let mut rng = Lcg(seed.wrapping_mul(2_246_822_519).wrapping_add(3));
            let mut log: Vec<Event> = Vec::new();
            let mut minted = 0u32; // client-minted ids for add/split, distinct from replay's own
            let n = 1 + rng.below(12);
            let mut trace = Vec::new();
            for _ in 0..n {
                // Ids that could exist so far (K1..=K{minted}); ops on absent ids are valid no-ops.
                let target = format!("K{}", 1 + rng.below((minted.max(1)) as usize));
                let (a, b) = (rng.below(9) as i64 - 2, rng.below(9) as i64 - 2);
                let ev = match rng.below(5) {
                    0 => {
                        minted += 1;
                        Event::PhaseAdded {
                            id: Some(format!("K{minted}")),
                            label: format!("p{minted}"),
                            from_col: a.min(b),
                            to_col: a.max(b),
                        }
                    }
                    1 => Event::PhaseResized {
                        id: target,
                        from_col: a.min(b),
                        to_col: a.max(b),
                    },
                    2 => Event::FrontierMoved {
                        id: target,
                        edge: if rng.below(2) == 0 { "start" } else { "end" }.into(),
                        col: a,
                    },
                    3 => {
                        minted += 1;
                        Event::PhaseSplit {
                            id: target,
                            at_col: a,
                            new_id: format!("K{minted}"),
                            new_label: format!("s{minted}"),
                        }
                    }
                    _ => Event::PhaseRemoved { id: target },
                };
                trace.push(line(&ev));
                log.push(ev);
            }
            let mut phases = replay(&log).phases;
            for w in phases.windows(2) {
                assert!(
                    w[0].to_col + 1 == w[1].from_col,
                    "seed {seed}: not contiguous ({}..{} then {}..{}) after:\n  {}",
                    w[0].from_col,
                    w[0].to_col,
                    w[1].from_col,
                    w[1].to_col,
                    trace.join("\n  ")
                );
            }
            for p in &phases {
                assert!(
                    p.from_col <= p.to_col,
                    "seed {seed}: phase {} inverted",
                    p.id
                );
            }
            // Idempotence: normalizing the already-normalized result changes nothing.
            let before: Vec<_> = phases
                .iter()
                .map(|p| (p.id.clone(), p.from_col, p.to_col))
                .collect();
            crate::model::normalize(&mut phases);
            let after: Vec<_> = phases
                .iter()
                .map(|p| (p.id.clone(), p.from_col, p.to_col))
                .collect();
            assert_eq!(before, after, "seed {seed}: normalize not idempotent");
        }
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

        let (folded, skipped) = from_comments(inbox);
        assert_eq!(skipped, 0); // every line migrated
        let mut log = from_model(&model);
        log.extend(folded);
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
            {\"kind\":\"add\",\"type\":\"event\",\"text\":\"legacy add, no elemId\"}\n\
            {\"elemId\":\"E1\",\"kind\":\"comment\",\"text\":\"kept\"}\n";
        let (evs, skipped) = from_comments(inbox);
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], Event::ElementAnnotated { id, text }
            if id == "E1" && text == "kept"));
        // Blank lines are not counted; the malformed line, the orphan, and the legacy `add` are.
        assert_eq!(skipped, 3);
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
    fn replay_applies_y_and_a_col_only_move_preserves_it() {
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":0}"#),
            ev(r#"{"event":"ElementMoved","id":"E1","y":0.8}"#),
            ev(r#"{"event":"ElementMoved","id":"E1","col":3}"#),
        ];
        let e1 = &replay(&log).elements[0];
        assert_eq!(e1.col, Some(3));
        assert_eq!(e1.y, Some(0.8), "a col-only nudge must not reset the Y");
    }

    #[test]
    fn a_placed_elements_y_survives_compact() {
        // `compact` folds the projection into ElementAdded lines; without `y` on the add the
        // whole 2D placement would silently flatten on every snapshot.
        let log = [
            ev(r#"{"event":"ElementAdded","id":"E1","type":"event","label":"A","col":0}"#),
            ev(r#"{"event":"ElementMoved","id":"E1","y":0.25}"#),
        ];
        let folded = compact(&log);
        let reparsed = parse_log(&to_jsonl(&folded)).unwrap();
        assert_eq!(replay(&reparsed).elements[0].y, Some(0.25));
    }

    #[test]
    fn move_comment_with_y_only_persists_one_moved_event() {
        let v = json::parse(r#"{"elemId":"E1","kind":"move","y":0.6}"#).unwrap();
        let evs = comment_to_events(&v);
        assert!(
            matches!(&evs[..], [Event::ElementMoved { id, col: None, y: Some(y), .. }]
                if id == "E1" && *y == 0.6),
            "got {evs:?}"
        );
    }

    #[test]
    fn move_comment_with_neither_col_nor_y_is_rejected() {
        let v = json::parse(r#"{"elemId":"E1","kind":"move"}"#).unwrap();
        assert!(
            comment_to_events(&v).is_empty(),
            "a move carrying no target would replay as a no-op"
        );
    }

    #[test]
    fn move_comment_clamps_and_rounds_its_y() {
        // Out-of-band fractions would draw off the lane; float noise would dirty the log.
        for (posted, stored) in [("1.7", 1.0), ("-0.3", 0.0), ("0.333333333333", 0.3333)] {
            let v =
                json::parse(&format!(r#"{{"elemId":"E1","kind":"move","y":{posted}}}"#)).unwrap();
            let evs = comment_to_events(&v);
            assert!(
                matches!(&evs[..], [Event::ElementMoved { y: Some(y), .. }] if *y == stored),
                "posted {posted}: got {evs:?}"
            );
        }
    }

    #[test]
    fn is_log_path_keys_on_extension() {
        assert!(is_log_path(Path::new("event-log.jsonl")));
        assert!(is_log_path(Path::new("a.log")));
        assert!(!is_log_path(Path::new("model.json")));
    }
}
