//! The event-sourced spine: an append-only log is the durable record; the `Model` is a
//! projection replayed from it.
//!
//! This inverts the older "model file is truth, comments are a disposable inbox" stance
//! (see `docs/source-of-truth.md`). Here `event-log.jsonl` is the only durable record and
//! the only write path; `model.json` becomes derived output. Each log line is one JSON
//! object discriminated by an `"event"` field; [`replay()`] folds a sequence into a `Model`,
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
//!   backward-incompatible change `upcast` repairs: it is the single place a legacy kind string
//!   is rewritten to today's, so the rest of the pipeline only ever sees current kinds. Detection
//!   is by shape (the old kind's presence), not a stored version counter — an old log replays with
//!   nothing to set.
//! - *A kind's meaning is never silently repurposed.* If semantics must change, introduce a new
//!   kind (additive) and upcast the old one; never redefine an existing kind in place.

use crate::model::Lane;

/// One fact in the log. Variants mirror the board operations a session performs; the
/// `Element*` variants all carry the stable `id` (identity is never text or position).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    BoardTitled {
        title: String,
    },
    /// The board format this log is written in. Holds the raw wire string, as `BoardTitled` holds
    /// a raw title; the codec has already refused any value this build cannot project. Additive —
    /// an old log carries none and replays as the default.
    BoardFormat {
        format: String,
    },
    /// The board's modeling granularity (`"big-picture"` / `"design"`). Stores the raw wire string
    /// like `BoardTitled` stores the raw title; `replay` parses it through `model::level_from_str`,
    /// so the `Level` enum stays internal to the model. Additive kind — an old log never has it and
    /// replays as the default `BigPicture`.
    BoardLeveled {
        level: String,
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
        /// The sticky's lane. An off-grammar `type` never reaches here: the codec skips the line,
        /// the way it skips an unknown event kind.
        kind: Lane,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
        /// Stored vertical sub-position within the lane band (F-2d-placement): a fraction of the
        /// band interior in `[0, 1]`, `None` = auto-stacked. Carried on add so `compact`/genesis
        /// round-trips a placed board (additive field — an old log simply never has it).
        y: Option<f64>,
        /// Attached reference URLs (F-element-links). Carried on add so `compact`/genesis round-trips
        /// them; additive — an old log has no `links` and replays with an empty list.
        links: Vec<String>,
    },
    ElementRenamed {
        id: String,
        label: String,
    },
    ElementMoved {
        id: String,
        col: Option<i64>,
        kind: Option<Lane>,
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
        /// Optional human label for the connection (F-element-links). Additive — an old log has no
        /// `label` and replays with `None`.
        label: Option<String>,
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

mod codec;
mod comments;
mod genesis;
mod log;
mod replay;

pub use codec::{line, to_jsonl};
pub use comments::{comment_to_events, nonblank, valid_span};
pub use genesis::{compact, from_model};
pub use log::{is_log_path, load, parse_log, read_log, read_log_full};
pub use replay::{region_watermark, replay};

#[cfg(test)]
mod testutil;
