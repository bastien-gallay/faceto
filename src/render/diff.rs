//! The diff overlay — a *render* artifact, never a domain fact (F-board-vs-diff).
//!
//! [`diff_boards`] joins two boards on the stable `id` and hands back two things the caller keeps
//! apart: the **union board** — a plain [`Model`], laid out on the new side with the old side's
//! ghosts appended — and the [`Overlay`], which says what changed, keyed by those same stable ids.
//! The board type carries no `diff` / `was` / `status` optionals, so "a board" and "a diff of two
//! boards" are no longer one product type: a `Model` is always a board, and an overlay only exists
//! where two boards were compared.
//!
//! The join rules are unchanged — identity is the `id` (never text, never position), and layout
//! follows the *new* side.

use crate::model::Lane;
use crate::model::{y_key, Edge, Model, Phase};
use std::collections::{HashMap, HashSet};

/// The four-tone vocabulary the board paints a change in — shared by elements and regions, so a
/// renamed region reads like a relabelled sticky and a resized one like a relocated one. `style`
/// maps a tone to its colour and badge; nothing else names those strings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Added,
    Removed,
    Changed,
    Moved,
}

impl Tone {
    /// The wire word — the class suffix (`diff-added`), the legend caption, the tooltip stem.
    pub fn as_str(self) -> &'static str {
        match self {
            Tone::Added => "added",
            Tone::Removed => "removed",
            Tone::Changed => "changed",
            Tone::Moved => "moved",
        }
    }
}

/// The old side of an element that survived into the new board — what the tooltip reads back as
/// "was". Only a `Changed` / `Moved` verdict carries one: an added element has no past, and a
/// removed one *is* its past.
#[derive(Clone, PartialEq, Debug)]
pub struct Was {
    pub label: String,
    pub col: Option<i64>,
    pub kind: Lane,
    pub y: Option<f64>,
}

/// What happened to one element between the two boards.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum ElementVerdict {
    Added,
    Removed,
    /// Relabelled.
    Changed(Was),
    /// Same label, different place: another column, another lane, or another `y_key` slot.
    Moved(Was),
    #[default]
    Unchanged,
}

impl ElementVerdict {
    /// The verdict's wire word — the sticky's `diff-*` class. Unlike [`Self::tone`] this is total:
    /// `unchanged` is a verdict the class list still names.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ElementVerdict::Unchanged => "unchanged",
            _ => self.tone().expect("only Unchanged has no tone").as_str(),
        }
    }

    /// The tone this verdict paints in — `None` when there is nothing to paint.
    pub(crate) fn tone(&self) -> Option<Tone> {
        match self {
            ElementVerdict::Added => Some(Tone::Added),
            ElementVerdict::Removed => Some(Tone::Removed),
            ElementVerdict::Changed(_) => Some(Tone::Changed),
            ElementVerdict::Moved(_) => Some(Tone::Moved),
            ElementVerdict::Unchanged => None,
        }
    }
}

/// What happened to one region between the two boards. Its own four words (a region is *renamed*
/// and *resized*, not "changed" and "moved") map onto the shared [`Tone`] for painting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RegionVerdict {
    Added,
    Removed,
    Renamed,
    Resized,
    #[default]
    Unchanged,
}

impl RegionVerdict {
    pub(crate) fn tone(self) -> Option<Tone> {
        match self {
            RegionVerdict::Added => Some(Tone::Added),
            RegionVerdict::Removed => Some(Tone::Removed),
            RegionVerdict::Renamed => Some(Tone::Changed),
            RegionVerdict::Resized => Some(Tone::Moved),
            RegionVerdict::Unchanged => None,
        }
    }
}

/// What happened to one connection. An edge has no identity of its own — it is keyed by its
/// `(src, dst)` pair — so it can only appear, vanish, or stay.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EdgeVerdict {
    Added,
    Removed,
    #[default]
    Unchanged,
}

/// The verdicts of one board-to-board comparison, keyed on the same stable ids the union board
/// carries. Rendered *beside* a board, never stored on one: `serve` re-derives it per request and
/// the log never sees it.
#[derive(Clone, PartialEq, Debug)]
pub struct Overlay {
    /// How the two sides are named on the board's diff subtitle — `(old, new)`, e.g.
    /// `("last seen", "now")`.
    pub meta: (String, String),
    elements: HashMap<String, ElementVerdict>,
    regions: HashMap<String, RegionVerdict>,
    edges: HashMap<(String, String), EdgeVerdict>,
}

impl Overlay {
    /// One element's verdict. An id the overlay never saw reads as `Unchanged` — the board is the
    /// truth about what exists, the overlay only about what changed.
    pub(crate) fn element(&self, id: &str) -> &ElementVerdict {
        self.elements.get(id).unwrap_or(&ElementVerdict::Unchanged)
    }

    pub(crate) fn region(&self, id: &str) -> RegionVerdict {
        self.regions.get(id).copied().unwrap_or_default()
    }

    pub(crate) fn edge(&self, e: &Edge) -> EdgeVerdict {
        self.edges
            .get(&(e.src.clone(), e.dst.clone()))
            .copied()
            .unwrap_or_default()
    }

    /// How many elements carry a given tone — the diff subtitle's counts and the CLI's tally.
    pub fn count(&self, tone: Tone) -> usize {
        self.elements
            .values()
            .filter(|v| v.tone() == Some(tone))
            .count()
    }
}

/// Whether a region is a ghost of the old board — the one verdict the *layout* reads (a removed
/// band draws no frontier and cannot be folded). Total on `Option`, because most renders have no
/// overlay at all.
pub(crate) fn region_removed(overlay: Option<&Overlay>, id: &str) -> bool {
    overlay.is_some_and(|o| o.region(id) == RegionVerdict::Removed)
}

/// Merge two boards into the union board plus its overlay: every element / region / edge judged
/// added / removed / changed / moved / unchanged, keyed on stable `id` (never text, never
/// position). Layout follows the *new* side (`b`); removed elements and regions keep their old slot
/// and are appended as ghosts, so the board still knows where they used to sit.
/// Whether two boards may be diffed: only under one format. The join key is `id`, and `id` means
/// something different in each grammar, so a cross-format overlay would judge unrelated stickies
/// `moved`. Checked at the CLI boundary — `--base` takes any file, either side.
pub fn comparable(base: &Model, new: &Model) -> Result<(), String> {
    if base.format == new.format {
        return Ok(());
    }
    Err(format!(
        "cannot diff a {} board against a {} one — a diff joins the two sides on `id`, \
         which names a different thing in each format",
        crate::model::format_to_str(base.format),
        crate::model::format_to_str(new.format)
    ))
}

pub fn diff_boards(a: &Model, b: &Model, meta: (String, String)) -> (Model, Overlay) {
    let old: HashMap<&str, &crate::model::Element> =
        a.elements.iter().map(|e| (e.id.as_str(), e)).collect();
    let new_ids: HashSet<&str> = b.elements.iter().map(|e| e.id.as_str()).collect();

    let mut elements = b.elements.clone();
    let mut verdicts: HashMap<String, ElementVerdict> = HashMap::new();
    for e in &b.elements {
        let verdict = match old.get(e.id.as_str()) {
            None => ElementVerdict::Added,
            Some(o) => {
                let was = Was {
                    label: o.label.clone(),
                    col: o.col,
                    kind: o.kind,
                    y: o.y,
                };
                if o.label != e.label {
                    ElementVerdict::Changed(was)
                } else if o.col != e.col || o.kind != e.kind || y_key(o.y) != y_key(e.y) {
                    // `y` counts: a re-placement within the lane is a position change the
                    // since-you-last-looked overlay must report, same as a col shift. Compared
                    // through `y_key`, so "no y" vs the neutral 0.5 (an undone placement) never
                    // reads as a phantom move — only a key the renderer would order differently.
                    ElementVerdict::Moved(was)
                } else {
                    ElementVerdict::Unchanged
                }
            }
        };
        verdicts.insert(e.id.clone(), verdict);
    }
    for e in &a.elements {
        if !new_ids.contains(e.id.as_str()) {
            elements.push(e.clone());
            verdicts.insert(e.id.clone(), ElementVerdict::Removed);
        }
    }

    let pairs = |m: &Model| -> HashSet<(String, String)> {
        m.edges
            .iter()
            .map(|e| (e.src.clone(), e.dst.clone()))
            .collect()
    };
    let (sa, sb) = (pairs(a), pairs(b));
    let mut edges = b.edges.clone();
    let mut edge_verdicts: HashMap<(String, String), EdgeVerdict> = HashMap::new();
    for e in &b.edges {
        let key = (e.src.clone(), e.dst.clone());
        let verdict = if sa.contains(&key) {
            EdgeVerdict::Unchanged
        } else {
            EdgeVerdict::Added
        };
        edge_verdicts.insert(key, verdict);
    }
    for e in &a.edges {
        let key = (e.src.clone(), e.dst.clone());
        if !sb.contains(&key) {
            edges.push(e.clone());
            edge_verdicts.insert(key, EdgeVerdict::Removed);
        }
    }

    let (phases, regions) = diff_phases(a, b);

    let board = Model {
        title: if !b.title.is_empty() {
            b.title.clone()
        } else {
            a.title.clone()
        },
        // A diff is a render-only artifact (lint never runs on it); layout follows the new side,
        // so the tags do too. Callers pass two boards of one format — see `comparable`.
        format: b.format,
        level: b.level,
        phases,
        elements,
        edges,
    };
    let overlay = Overlay {
        meta,
        elements: verdicts,
        regions,
        edges: edge_verdicts,
    };
    (board, overlay)
}

/// Diff the regions of two boards, keyed on stable `id` (mirroring the element diff). Layout
/// follows the **new** side (`b`); a region only in the old side keeps its slot and is appended as
/// a `Removed` ghost.
fn diff_phases(a: &Model, b: &Model) -> (Vec<Phase>, HashMap<String, RegionVerdict>) {
    let old: HashMap<&str, &Phase> = a.phases.iter().map(|p| (p.id.as_str(), p)).collect();
    let new_ids: HashSet<&str> = b.phases.iter().map(|p| p.id.as_str()).collect();

    let mut phases = b.phases.clone();
    let mut verdicts: HashMap<String, RegionVerdict> = HashMap::new();
    for p in &b.phases {
        let verdict = match old.get(p.id.as_str()) {
            None => RegionVerdict::Added,
            Some(o) if o.label != p.label => RegionVerdict::Renamed,
            Some(o) if o.from_col != p.from_col || o.to_col != p.to_col => RegionVerdict::Resized,
            Some(_) => RegionVerdict::Unchanged,
        };
        verdicts.insert(p.id.clone(), verdict);
    }
    for p in &a.phases {
        if !new_ids.contains(p.id.as_str()) {
            phases.push(p.clone());
            verdicts.insert(p.id.clone(), RegionVerdict::Removed);
        }
    }
    (phases, verdicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;
    use crate::model::from_json;

    fn model_of(src: &str) -> Model {
        from_json(&json::parse(src).unwrap())
    }

    fn meta() -> (String, String) {
        ("old".into(), "new".into())
    }

    #[test]
    fn two_boards_of_one_format_are_comparable() {
        let m = model_of(r#"{"elements":[]}"#);
        assert_eq!(comparable(&m, &m), Ok(()));
    }

    // The whole comment/diff contract hinges on `id` being identity, never text or position.
    // This pins each verdict to the right join.
    #[test]
    fn elements_are_judged_by_stable_id() {
        let a = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"Old","col":0},
                {"id":"E2","type":"event","label":"Same","col":1},
                {"id":"E3","type":"event","label":"Gone","col":2},
                {"id":"E4","type":"event","label":"Shifts","col":3}
            ]}"#,
        );
        let b = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"New","col":0},
                {"id":"E2","type":"event","label":"Same","col":1},
                {"id":"E4","type":"event","label":"Shifts","col":9},
                {"id":"E5","type":"event","label":"Fresh","col":4}
            ]}"#,
        );
        let (board, o) = diff_boards(&a, &b, meta());

        assert!(matches!(o.element("E1"), ElementVerdict::Changed(_)));
        assert_eq!(*o.element("E2"), ElementVerdict::Unchanged);
        assert_eq!(*o.element("E3"), ElementVerdict::Removed);
        assert!(matches!(o.element("E4"), ElementVerdict::Moved(_)));
        assert_eq!(*o.element("E5"), ElementVerdict::Added);
        // The removed element is kept as a ghost so the union board can still draw it.
        assert!(board.elements.iter().any(|e| e.id == "E3"));
    }

    // A relabel reads back its old text; a move reads back its old place.
    #[test]
    fn a_surviving_element_carries_its_old_side() {
        let a = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"Old","col":0},
                {"id":"E2","type":"command","label":"Same","col":1}
            ]}"#,
        );
        let b = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"New","col":0},
                {"id":"E2","type":"event","label":"Same","col":1}
            ]}"#,
        );
        let (_, o) = diff_boards(&a, &b, meta());
        // A relabel reads back the old text; a lane change is a *move* that reads back the old lane.
        let changed = match o.element("E1") {
            ElementVerdict::Changed(w) => Some(w.label.as_str()),
            _ => None,
        };
        assert_eq!(changed, Some("Old"), "E1 was relabelled");
        let moved = match o.element("E2") {
            ElementVerdict::Moved(w) => Some(w.kind),
            _ => None,
        };
        assert_eq!(moved, Some(Lane::Command), "E2 changed lane");
    }

    // `y` is an ordering key, not a position: "no y" and the neutral 0.5 are one state, so an
    // undone placement must not read as a phantom move.
    #[test]
    fn a_neutral_y_is_not_a_move() {
        let a = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"L","col":0}]}"#);
        let b =
            model_of(r#"{"elements":[{"id":"E1","type":"event","label":"L","col":0,"y":0.5}]}"#);
        let (_, o) = diff_boards(&a, &b, meta());
        assert_eq!(*o.element("E1"), ElementVerdict::Unchanged);

        let c =
            model_of(r#"{"elements":[{"id":"E1","type":"event","label":"L","col":0,"y":0.9}]}"#);
        let (_, o) = diff_boards(&a, &c, meta());
        assert!(matches!(o.element("E1"), ElementVerdict::Moved(_)));
    }

    #[test]
    fn regions_are_judged_by_stable_id_too() {
        let a = model_of(
            r#"{"phases":[
                {"id":"K1","label":"Alpha","fromCol":0,"toCol":1},
                {"id":"K2","label":"Beta","fromCol":2,"toCol":3},
                {"id":"K3","label":"Gamma","fromCol":4,"toCol":5},
                {"id":"K4","label":"Delta","fromCol":6,"toCol":7}
            ]}"#,
        );
        let b = model_of(
            r#"{"phases":[
                {"id":"K1","label":"Alpha","fromCol":0,"toCol":1},
                {"id":"K2","label":"Beta renamed","fromCol":2,"toCol":3},
                {"id":"K3","label":"Gamma","fromCol":4,"toCol":9},
                {"id":"K5","label":"Epsilon","fromCol":10,"toCol":11}
            ]}"#,
        );
        let (board, o) = diff_boards(&a, &b, meta());
        assert_eq!(o.region("K1"), RegionVerdict::Unchanged);
        assert_eq!(o.region("K2"), RegionVerdict::Renamed);
        assert_eq!(o.region("K3"), RegionVerdict::Resized);
        assert_eq!(o.region("K4"), RegionVerdict::Removed);
        assert_eq!(o.region("K5"), RegionVerdict::Added);
        assert!(board.phases.iter().any(|p| p.id == "K4"), "ghost band kept");
        // The layout question the region loop actually asks.
        assert!(region_removed(Some(&o), "K4"));
        assert!(!region_removed(Some(&o), "K1"));
        assert!(
            !region_removed(None, "K4"),
            "no overlay ⇒ nothing is a ghost"
        );
    }

    #[test]
    fn edges_are_judged_by_their_endpoints() {
        let a = model_of(r#"{"edges":[{"src":"E1","dst":"E2"},{"src":"E2","dst":"E3"}]}"#);
        let b = model_of(r#"{"edges":[{"src":"E1","dst":"E2"},{"src":"E3","dst":"E4"}]}"#);
        let (board, o) = diff_boards(&a, &b, meta());
        let edge = |src: &str, dst: &str| Edge {
            src: src.into(),
            dst: dst.into(),
            label: None,
        };
        assert_eq!(o.edge(&edge("E1", "E2")), EdgeVerdict::Unchanged);
        assert_eq!(o.edge(&edge("E3", "E4")), EdgeVerdict::Added);
        assert_eq!(o.edge(&edge("E2", "E3")), EdgeVerdict::Removed);
        assert_eq!(board.edges.len(), 3, "the removed wire is kept as a ghost");
    }

    // The subtitle's counts and the CLI's tally read the same accessor.
    #[test]
    fn tone_counts_the_elements_that_carry_it() {
        let a = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"Old","col":0},
                {"id":"E2","type":"event","label":"Gone","col":1}
            ]}"#,
        );
        let b = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"New","col":0},
                {"id":"E3","type":"event","label":"Fresh","col":2},
                {"id":"E4","type":"event","label":"Also fresh","col":3}
            ]}"#,
        );
        let (_, o) = diff_boards(&a, &b, meta());
        assert_eq!(o.count(Tone::Added), 2);
        assert_eq!(o.count(Tone::Removed), 1);
        assert_eq!(o.count(Tone::Changed), 1);
        assert_eq!(o.count(Tone::Moved), 0);
    }

    // The board keeps the newer title, falling back to the older when the new side has none.
    #[test]
    fn the_union_board_titles_itself_from_the_new_side() {
        let a = model_of(r#"{"title":"Was"}"#);
        let b = model_of(r#"{"title":"Now"}"#);
        assert_eq!(diff_boards(&a, &b, meta()).0.title, "Now");

        let blank = Model::default();
        assert_eq!(diff_boards(&a, &blank, meta()).0.title, "Was");
    }

    // Verdict → tone → wire word: the one place the painted vocabulary is pinned.
    #[test]
    fn verdicts_speak_the_shared_four_tone_vocabulary() {
        assert_eq!(ElementVerdict::Added.as_str(), "added");
        assert_eq!(ElementVerdict::Unchanged.as_str(), "unchanged");
        assert_eq!(
            ElementVerdict::Changed(Was {
                label: "x".into(),
                col: None,
                kind: Lane::Event,
                y: None,
            })
            .as_str(),
            "changed"
        );
        assert_eq!(RegionVerdict::Renamed.tone(), Some(Tone::Changed));
        assert_eq!(RegionVerdict::Resized.tone(), Some(Tone::Moved));
        assert_eq!(RegionVerdict::Unchanged.tone(), None);
        assert_eq!(Tone::Moved.as_str(), "moved");
    }
}
