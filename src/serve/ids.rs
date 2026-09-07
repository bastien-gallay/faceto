//! Server-side id minting: the next free `<PREFIX><N>` for a lane ([`mint_id`]) and the
//! next region id ([`mint_region_id`]), both derived from the log so it stays the only record.

use crate::events;
use crate::model::Lane;
use crate::render;

/// Next free id for `kind`: `<PREFIX>` one past the highest suffix **ever added** under that
/// prefix in the log — scanning every `ElementAdded`, including ids since removed but not yet
/// compacted away. Deriving from the live projection instead would re-mint a removed element's
/// id while leftover events still reference it (e.g. its annotations in `/comments`). `compact`
/// folds removed elements out entirely, so reuse after compaction is safe.
pub(crate) fn mint_id(kind: Lane, log: &[events::Event]) -> String {
    let prefix = render::lane_prefix(kind);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events;
    use crate::serve::testutil::*;

    #[test]
    fn mint_id_picks_next_free_suffix_per_lane() {
        // H6: ids are type-prefixed and never renumbered — minting takes one past the
        // highest suffix already used under that prefix, independently per lane.
        let log = [
            added("E1", Lane::Event),
            added("E3", Lane::Event),
            added("C1", Lane::Command),
        ];
        assert_eq!(mint_id(Lane::Event, &log), "E4"); // past the highest E, not filling the E2 gap
        assert_eq!(mint_id(Lane::Command, &log), "C2");
        assert_eq!(mint_id(Lane::Hotspot, &log), "H1"); // empty lane starts at 1
        assert_eq!(mint_id(Lane::Actor, &log), "X1"); // actor stamps X, not A
        assert_eq!(mint_id(Lane::Aggregate, &log), "A1");
    }

    #[test]
    fn mint_id_does_not_reuse_a_removed_id() {
        // A dropped element's ElementAdded stays in the log (until compaction), so its id must
        // stay reserved — re-minting it would alias leftover events (e.g. its annotations).
        let log = [
            added("E1", Lane::Event),
            added("E2", Lane::Event),
            events::Event::ElementRemoved { id: "E2".into() },
        ];
        assert_eq!(mint_id(Lane::Event, &log), "E3");
    }

    #[test]
    fn mint_region_id_picks_next_free_k_suffix() {
        let log = [region_added("K1", 0, 2), region_added("K3", 3, 5)];
        assert_eq!(mint_region_id(&log), "K4"); // past the highest K, not filling the K2 gap
        assert_eq!(mint_region_id(&[]), "K1"); // empty log starts at 1
    }

    #[test]
    fn mint_region_id_does_not_reuse_a_removed_id() {
        let log = [
            region_added("K1", 0, 2),
            region_added("K2", 3, 5),
            events::Event::PhaseRemoved { id: "K2".into() },
        ];
        assert_eq!(mint_region_id(&log), "K3");
    }

    #[test]
    fn mint_region_id_shares_the_namespace_with_replays_synthetic_ids() {
        // Review #3: a legacy id-less PhaseAdded replays to a synthetic K<n> (resolve_region_id).
        // The mint must reserve that suffix too, or a fresh region could collide with one replay
        // would later synthesize for the same log.
        let log = [events::Event::PhaseAdded {
            id: None,
            label: "Legacy".into(),
            from_col: 0,
            to_col: 2,
        }];
        assert_eq!(mint_region_id(&log), "K2"); // K1 is reserved for the legacy band
        let model = events::replay(&log);
        assert_eq!(model.phases[0].id, "K1");
    }

    #[test]
    fn mint_region_id_reserves_split_ids() {
        // A split's minted right-half id lives in the same namespace; the next mint must skip it.
        let log = [
            region_added("K1", 0, 5),
            events::Event::PhaseSplit {
                id: "K1".into(),
                at_col: 3,
                new_id: "K2".into(),
                new_label: "Right".into(),
            },
        ];
        assert_eq!(mint_region_id(&log), "K3", "K2 is spent by the split");
    }

    #[test]
    fn mint_id_saturates_instead_of_overflowing() {
        // A hand-edited log with a suffix at u32::MAX must not panic/wrap.
        let log = [added("E4294967295", Lane::Event)];
        assert_eq!(mint_id(Lane::Event, &log), "E4294967295");
    }
}
