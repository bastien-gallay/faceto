//! Fold a legacy `comments.jsonl` into events ([`from_comments`]) and map one posted comment
//! to the events it implies ([`comment_to_events`]) — the single source of truth shared with
//! `serve.rs`'s `POST /comment`.

use super::log::jsonl_records;
use super::Event;
use crate::json::{self, Json};

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
