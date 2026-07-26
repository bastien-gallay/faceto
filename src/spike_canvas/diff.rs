//! The canvas diff — and the answer to "does the diff survive when *moved* has no meaning?"
//!
//! **It half-survives.** The *join* is format-agnostic and transferred verbatim (see
//! [`join_by_id`] below, which is `crate::model::diff_models`'s first ten lines with the ES types
//! lifted out). The *verdicts* are not:
//!
//! | ES verdict | canvas | why |
//! | --- | --- | --- |
//! | `added` / `removed` | ✅ same | pure set membership on the stable id |
//! | `changed` (label differs) | ✅ same | one text field per item |
//! | `moved` (col / kind / y-key) | ❌ **gone** | there is no coordinate to differ |
//! | — | 🆕 `reslotted` | an item changed section: categorical, not spatial |
//! | region `resized` | ❌ gone | no regions, no bounds |
//! | edge `added` / `removed` | ❌ gone | no edges at all |
//!
//! `reslotted` is *not* `moved` renamed. `moved` is a report about a position the viewer can see
//! on the board and compare ("col 4 → col 7"); `reslotted` is a report about a *category* and
//! reads as a semantic re-classification ("PlaceOrder was inbound, is now outbound") — closer to
//! ES's `changed` than to its `moved`. Confirms the note's *"do not extract the diff verdicts —
//! rule of two"*: the verdicts are format semantics, and the second real example disagrees with
//! the first rather than generalising it.
//!
//! One further finding the ES diff hides: `crate::model::diff_models` returns a `Model`, i.e. it
//! re-uses the board type as the overlay type. Doing the same here (`Canvas` with `Item.diff`
//! populated) means the canvas renderer must, like the ES one, branch on "am I drawing a board or
//! an overlay?" at every mark. Neither format wants that; it is a pre-existing design debt the
//! second format simply inherits.

use super::model::{Canvas, Item};
use std::collections::{HashMap, HashSet};

/// The one genuinely generic half of the diff, extracted here as the kernel helper the
/// architecture note calls `join_by_id`. Yields, in new-side order, `(new, old?)` pairs followed
/// by the old-only entries — the verdict is the caller's (i.e. the format's) business.
///
/// Written against a closure rather than a trait so the spike stays zero-ceremony; the real
/// extraction would take `fn id(&T) -> &str`.
pub fn join_by_id<'a, T>(
    old: &'a [T],
    new: &'a [T],
    id: impl Fn(&T) -> &str,
) -> (Vec<(&'a T, Option<&'a T>)>, Vec<&'a T>) {
    let by_id: HashMap<&str, &T> = old.iter().map(|t| (id(t), t)).collect();
    let new_ids: HashSet<&str> = new.iter().map(&id).collect();
    let paired = new.iter().map(|n| (n, by_id.get(id(n)).copied())).collect();
    let dropped = old.iter().filter(|o| !new_ids.contains(id(o))).collect();
    (paired, dropped)
}

/// Merge two canvases into one annotated canvas. Layout follows the new side; a removed item keeps
/// its old section.
pub fn diff_canvases(a: &Canvas, b: &Canvas, meta: (String, String)) -> Canvas {
    let (paired, dropped) = join_by_id(&a.items, &b.items, |i| i.id.as_str());

    let mut items: Vec<Item> = Vec::new();
    for (new, old) in paired {
        let mut it = new.clone();
        match old {
            None => it.diff = Some("added".into()),
            Some(old) => {
                if old.text != new.text {
                    it.diff = Some("changed".into());
                    it.was_text = Some(old.text.clone());
                    // A single edit can be both a relabel and a reslot; record both, and let the
                    // relabel win the headline verdict (ES makes the same call for label-vs-col).
                    if old.slot != new.slot {
                        it.was_slot = Some(old.slot);
                    }
                } else if old.slot != new.slot {
                    it.diff = Some("reslotted".into());
                    it.was_slot = Some(old.slot);
                } else {
                    it.diff = Some("unchanged".into());
                }
            }
        }
        items.push(it);
    }
    for old in dropped {
        let mut it = old.clone();
        it.diff = Some("removed".into());
        items.push(it);
    }

    Canvas {
        name: if b.name.is_empty() {
            a.name.clone()
        } else {
            b.name.clone()
        },
        items,
        diff_meta: Some(meta),
    }
}

/// The overlay's per-item tooltip. Compare `render::svg::diff_tooltip`: that one has a
/// three-branch body for `moved` (lane / col / y) because a move is a compound spatial fact. Here
/// the whole vocabulary fits on one line each.
pub fn diff_tooltip(i: &Item, meta: &(String, String)) -> String {
    let (a, b) = (&meta.0, &meta.1);
    match i.diff.as_deref() {
        Some("added") => format!("added in {b}"),
        Some("removed") => format!("removed \u{2014} was in {a}"),
        Some("reslotted") => match i.was_slot {
            Some(s) => format!("reslotted: {} \u{2192} {}", s.title(), i.slot.title()),
            None => "reslotted".to_string(),
        },
        Some("changed") => {
            let was = i.was_text.as_deref().unwrap_or("");
            match i.was_slot {
                Some(s) => format!("was: {was} (in {})", s.title()),
                None => format!("was: {was}"),
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::Slot;
    use super::*;

    fn canvas(items: Vec<Item>) -> Canvas {
        Canvas {
            name: "Orders".into(),
            items,
            diff_meta: None,
        }
    }

    fn verdict<'a>(c: &'a Canvas, id: &str) -> Option<&'a str> {
        c.items
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.diff.as_deref())
    }

    #[test]
    fn verdicts_cover_added_removed_changed_reslotted_unchanged() {
        let a = canvas(vec![
            Item::new("U1", Slot::Purpose, "same"),
            Item::new("B1", Slot::Decisions, "old wording"),
            Item::new("I1", Slot::Inbound, "PlaceOrder"),
            Item::new("Q1", Slot::Questions, "goes away"),
        ]);
        let b = canvas(vec![
            Item::new("U1", Slot::Purpose, "same"),
            Item::new("B1", Slot::Decisions, "new wording"),
            Item::new("I1", Slot::Outbound, "PlaceOrder"),
            Item::new("M1", Slot::Metrics, "brand new"),
        ]);
        let d = diff_canvases(&a, &b, ("old".into(), "new".into()));
        assert_eq!(verdict(&d, "U1"), Some("unchanged"));
        assert_eq!(verdict(&d, "B1"), Some("changed"));
        assert_eq!(verdict(&d, "I1"), Some("reslotted"));
        assert_eq!(verdict(&d, "Q1"), Some("removed"));
        assert_eq!(verdict(&d, "M1"), Some("added"));
    }

    #[test]
    fn a_reslot_reports_the_former_section_not_a_coordinate() {
        let a = canvas(vec![Item::new("I1", Slot::Inbound, "PlaceOrder")]);
        let b = canvas(vec![Item::new("I1", Slot::Outbound, "PlaceOrder")]);
        let d = diff_canvases(&a, &b, ("old".into(), "new".into()));
        let i = &d.items[0];
        assert_eq!(i.was_slot, Some(Slot::Inbound));
        assert_eq!(
            diff_tooltip(i, &("old".into(), "new".into())),
            "reslotted: Inbound Communication \u{2192} Outbound Communication"
        );
    }

    #[test]
    fn a_simultaneous_edit_and_reslot_reads_as_changed_but_keeps_both_facts() {
        let a = canvas(vec![Item::new("I1", Slot::Inbound, "PlaceOrder")]);
        let b = canvas(vec![Item::new("I1", Slot::Outbound, "OrderPlaced")]);
        let d = diff_canvases(&a, &b, ("old".into(), "new".into()));
        assert_eq!(verdict(&d, "I1"), Some("changed"));
        assert_eq!(d.items[0].was_slot, Some(Slot::Inbound));
    }

    #[test]
    fn join_by_id_is_format_agnostic() {
        // The extractable half: the same helper joins ES elements with no canvas types in sight.
        let old = ["E1", "E2"];
        let new = ["E2", "E3"];
        let (paired, dropped) = join_by_id(&old, &new, |s: &&str| s);
        assert_eq!(
            paired
                .iter()
                .map(|(n, o)| (**n, o.is_some()))
                .collect::<Vec<_>>(),
            vec![("E2", true), ("E3", false)]
        );
        assert_eq!(dropped, vec![&"E1"]);
    }
}
