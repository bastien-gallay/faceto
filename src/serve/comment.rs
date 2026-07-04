//! Translate a posted `POST /comment` payload into the server-minted event it implies —
//! an `add`, a `region-add`, or a `phase-split`, each validated and minted under the lock.

use super::Ctx;
use crate::{events, json, render};

/// The non-blank `text` label a creation command requires, or `400` — the one place the
/// "a minted element/region must carry a label" rule turns into an HTTP status, shared by every
/// server-minted command so `add`, `region-add`, and `phase-split` can't diverge on it. A blank
/// label would mint a permanent, never-renumbered empty box; the same `nonblank` rule the `rename`
/// guard uses (a direct POST must not slip a blank in even though the client modal guards it).
fn required_label(v: &json::Json) -> Result<String, u16> {
    v.get_str("text").and_then(events::nonblank).ok_or(400u16)
}

/// Handle a `kind:"add"` post: an element-creation command rather than a comment on an
/// existing one. `type` (the lane) and a non-empty `text` (label) are required; optional
/// `col`/`detail`. The server mints the id (H6). Returns the HTTP status to fail with: `400` for
/// a missing/empty type or label, `500` if the append itself fails.
pub(crate) fn add_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    // `type` must be one of the 8 lanes. An off-grammar type would fall back to a first-letter
    // prefix in `id_prefix` and could mint into a real lane's id space (e.g. "epic"→'E'),
    // colliding the diff/comment join key — so reject it here rather than letting it through.
    let kind = v
        .get_str("type")
        .filter(|s| render::lane_prefix(s).is_some())
        .ok_or(400u16)?
        .to_string();
    let label = required_label(v)?;
    let col = v.get_i64("col");
    // The lane-title `+` posts `prepend:true` (no col); the server derives the left-edge col so the
    // rule lives in one place and stays consistent under concurrent adds.
    let prepend = v
        .get("prepend")
        .and_then(json::Json::as_bool)
        .unwrap_or(false);
    let detail = v
        .get_str("detail")
        .filter(|s| !s.is_empty())
        .map(String::from);
    ctx.append_add(&kind, label, col, detail, prepend)
        .map_err(|_| 500u16)
}

/// Handle a `kind:"region-add"` post: a region-creation command, the region counterpart of
/// `add_from_comment`. A non-empty `text` (label) and a well-ordered `[fromCol, toCol]` span
/// (`events::valid_span`) are required; the server mints the id (review #3 / H6 for regions).
/// Returns the HTTP status to fail with: `400` for a missing label or an absent/inverted/
/// zero-width span, `500` if the append itself fails.
pub(crate) fn add_region_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let label = required_label(v)?;
    let from_col = v.get_i64("fromCol").ok_or(400u16)?;
    let to_col = v.get_i64("toCol").ok_or(400u16)?;
    if !events::valid_span(from_col, to_col) {
        return Err(400u16);
    }
    ctx.append_region_add(label, from_col, to_col)
        .map_err(|_| 500u16)
}

/// Handle a `kind:"phase-split"` post: divide the region `regionId` at `atCol` into two, the
/// server minting the right half's id (F-region-frontiers, the partition's "add"). A non-empty
/// `text` (the new right-half label) and an `atCol` are required; whether the column falls strictly
/// inside the phase is validated under the lock in `append_phase_split` (a stale/out-of-range split
/// is refused before writing). Returns the HTTP status to fail with: `400` for a missing label or
/// atCol, `500` if the append itself fails or the split is out of range.
pub(crate) fn split_region_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let id = v.get_str("regionId").map(str::to_string).ok_or(400u16)?;
    let label = required_label(v)?;
    let at_col = v.get_i64("atCol").ok_or(400u16)?;
    ctx.append_phase_split(id, at_col, label)
        .map_err(|_| 500u16)
}
