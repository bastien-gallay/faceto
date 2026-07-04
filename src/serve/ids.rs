//! Server-side id minting: the next free `<PREFIX><N>` for a lane ([`mint_id`]) and the
//! next region id ([`mint_region_id`]), both derived from the log so it stays the only record.

use crate::events;
use crate::render;

/// The single letter each lane stamps onto a freshly minted id. The 8-lane prefixes come from
/// `render::lane_prefix` (one source of truth, in sync with `LANES`); an off-grammar type falls
/// back to its first letter, upper-cased.
fn id_prefix(kind: &str) -> char {
    render::lane_prefix(kind)
        .unwrap_or_else(|| kind.chars().next().unwrap_or('Z').to_ascii_uppercase())
}

/// Next free id for `kind`: `<PREFIX>` one past the highest suffix **ever added** under that
/// prefix in the log — scanning every `ElementAdded`, including ids since removed but not yet
/// compacted away. Deriving from the live projection instead would re-mint a removed element's
/// id while leftover events still reference it (e.g. its annotations in `/comments`). `compact`
/// folds removed elements out entirely, so reuse after compaction is safe.
pub(crate) fn mint_id(kind: &str, log: &[events::Event]) -> String {
    let prefix = id_prefix(kind);
    let max = log
        .iter()
        .filter_map(|ev| match ev {
            events::Event::ElementAdded { id, .. } => id
                .strip_prefix(prefix)
                .and_then(|rest| rest.parse::<u32>().ok()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    // saturating_add: a hand-edited log with a suffix at u32::MAX must not panic (debug) or
    // wrap to 0 (release) and re-mint a colliding low id.
    format!("{}{}", prefix, max.saturating_add(1))
}

/// Next free region id: `K<n>` one past the highest `K` suffix **ever seen** in the log —
/// explicit ids on `PhaseAdded`, *and* the synthetic ids `replay` mints for legacy id-less bands
/// (carry-over review #3, `F-container-scope.md`). Folding through `model::resolve_region_id` for
/// every `PhaseAdded` is exactly what `replay` does to compute its own `max_region`, so this mint
/// shares that namespace by construction — a region id can never collide with one replay would
/// have synthesized, and a removed-but-not-compacted suffix stays reserved (same rule as `mint_id`).
pub(crate) fn mint_region_id(log: &[events::Event]) -> String {
    // The namespace fold lives in the event spine (`events::region_watermark`), the same rule
    // `replay` uses — so a mint here can never collide with an id `replay` would synthesize. Server
    // side we just take one past the highest suffix ever spent.
    format!("K{}", events::region_watermark(log).saturating_add(1))
}
