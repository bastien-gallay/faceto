//! The typed board model + the stable-id diff.
//!
//! A board is elements (coloured stickies) on a shared left→right column axis, grouped
//! into lanes by `type`, connected by directed edges. Identity is the stable `id`
//! (never text or position) — that is the contract the comment sidecar and the diff rely on.

use crate::json::{self, Json};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The board's declared modeling granularity, which parameterises lint strictness (never
/// gating — a finding is always warn-only). `BigPicture` is a first-pass sweep where a command
/// sketched before its event is normal incompleteness; `Design` is a filled-in flow where such
/// a gap is a defect. The only difference today is that `Design` activates `command-no-output`
/// (see `crate::lint`). Default is `BigPicture`, so an older board with no `level` is unaffected.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Level {
    #[default]
    BigPicture,
    Design,
}

/// Parse a board `level` string. `"design"` → `Design`; anything else (including the explicit
/// `"big-picture"`, an unknown value, or an absent field via the caller's `unwrap_or_default`)
/// → `BigPicture`. The single parse point shared by `from_json` (model.json) and `replay` (the
/// log), so the two paths can never disagree — mirrors how `resolve_region_id` is shared.
pub fn level_from_str(s: &str) -> Level {
    match s {
        "design" => Level::Design,
        _ => Level::BigPicture,
    }
}

/// The wire string for a `Level` — the reverse of [`level_from_str`], so the log-serialize side
/// (`from_model`) can't drift from the parse side. Exhaustive on purpose: a future variant is a
/// compile error here until its wire form is declared, instead of silently round-tripping as the
/// default.
pub fn level_to_str(level: Level) -> &'static str {
    match level {
        Level::BigPicture => "big-picture",
        Level::Design => "design",
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Phase {
    /// Stable identity (the diff join key and the target of resize/rename/remove). A region is a
    /// labelled vertical band; an element belongs to it spatially (its `col` falls inside the
    /// band) — there is no membership field. See `docs/F-container-scope.md` (D1/D2).
    pub id: String,
    pub label: String,
    pub from_col: i64,
    pub to_col: i64,
    // diff annotation (not in the file): added / removed / renamed / resized / unchanged.
    pub diff: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Was {
    pub label: String,
    pub col: Option<i64>,
    pub kind: String,
    pub y: Option<f64>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Element {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub col: Option<i64>,
    pub detail: Option<String>,
    /// Stored vertical sub-position within the lane band (F-2d-placement): a fraction of the
    /// band interior in `[0, 1]`. `None` = auto-stacked by the renderer. Never part of identity
    /// (`id` is) and never a lane choice (`type` is) — it only places the sticky *within* its band.
    pub y: Option<f64>,
    pub resolved: bool,
    /// Attached reference URLs (F-element-links): tickets, docs, ADRs. Additive and free-form —
    /// never identity, never a lane. Surfaced as clickable chips in the click modal, not painted
    /// into the calm SVG board. Empty when the element carries none.
    pub links: Vec<String>,
    // diff annotations (not in the file)
    pub diff: Option<String>,
    pub was: Option<Was>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Edge {
    pub src: String,
    pub dst: String,
    /// Optional human label for the connection (F-element-links) — drawn at the edge midpoint so
    /// connection kinds stop reading identically. Shares the `Edge` seam with F-typed-edges (a
    /// future additive `type`); touch it once. Distinct from `status`, which is the internal
    /// diff-overlay channel, not an authored field.
    pub label: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Model {
    pub title: String,
    /// Modeling granularity — `BigPicture` (default) or `Design`. Read by `crate::lint` to decide
    /// which rules apply; never affects rendering. See [`Level`].
    pub level: Level,
    pub phases: Vec<Phase>,
    pub elements: Vec<Element>,
    pub edges: Vec<Edge>,
    pub diff_meta: Option<(String, String)>,
}

pub fn load(path: &Path) -> Result<Model, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let j = json::parse(&raw)?;
    Ok(from_json(&j))
}

pub fn from_json(j: &Json) -> Model {
    let title = j
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("board")
        .to_string();
    let level = j
        .get("level")
        .and_then(|v| v.as_str())
        .map(level_from_str)
        .unwrap_or_default();
    let mut phases: Vec<Phase> = j
        .get("phases")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let mut max_region = 0u32;
            arr.iter()
                .filter_map(|p| phase_from(p, &mut max_region))
                .collect()
        })
        .unwrap_or_default();
    // Project onto a contiguous partition (F-region-frontiers) so a bootstrap `model.json` obeys the
    // same invariant a replayed log does — every `Model`'s phases are gap-free and overlap-free.
    normalize(&mut phases);
    let elements = j
        .get("elements")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(element_from).collect())
        .unwrap_or_default();
    let edges = j
        .get("edges")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(edge_from).collect())
        .unwrap_or_default();
    Model {
        title,
        level,
        phases,
        elements,
        edges,
        diff_meta: None,
    }
}

fn phase_from(j: &Json, max_region: &mut u32) -> Option<Phase> {
    // Resolve the id only after the required fields parse, so a malformed band that gets dropped
    // does not advance the synthetic counter (keeps minted ids gap-free and never reused).
    let label = j.get("label")?.as_str()?.to_string();
    let from_col = j.get("fromCol")?.as_f64()? as i64;
    let to_col = j.get("toCol")?.as_f64()? as i64;
    let id = resolve_region_id(j.get("id").and_then(|v| v.as_str()), max_region);
    Some(Phase {
        id,
        label,
        from_col,
        to_col,
        diff: None,
    })
}

/// Resolve a region's id: an explicit id used as-is, otherwise the next free `K<n>` one past the
/// **highest `K` suffix ever seen** (`max_region`, which the caller threads across a band sequence
/// and never decrements). This mirrors `serve::mint_id`'s reservation rule — a synthetic id never
/// reuses a suffix freed by a `PhaseRemoved` or already taken by an explicit id. The single source
/// of truth for region-id minting, shared by `from_json` (model.json) and `replay` (the log).
pub fn resolve_region_id(explicit: Option<&str>, max_region: &mut u32) -> String {
    let id = explicit
        .map(String::from)
        .unwrap_or_else(|| format!("K{}", *max_region + 1));
    if let Some(n) = id.strip_prefix('K').and_then(|r| r.parse::<u32>().ok()) {
        *max_region = (*max_region).max(n);
    }
    id
}

/// The string entries of a JSON `links` array (F-element-links). Absent, non-array, or non-string
/// entries are dropped, never an error — same lenient additive-field posture as the rest of the parser.
pub fn links_from(j: Option<&Json>) -> Vec<String> {
    j.and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn element_from(j: &Json) -> Option<Element> {
    Some(Element {
        id: j.get("id")?.as_str()?.to_string(),
        kind: j.get("type")?.as_str()?.to_string(),
        label: j.get("label")?.as_str()?.to_string(),
        col: j.get("col").and_then(|v| v.as_f64()).map(|n| n as i64),
        detail: j.get("detail").and_then(|v| v.as_str()).map(String::from),
        y: j.get("y").and_then(|v| v.as_f64()),
        resolved: j.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false),
        links: links_from(j.get("links")),
        diff: None,
        was: None,
    })
}

/// An edge is a positional tuple `[src, dst]` / `[src, dst, status]` **or** an object
/// `{src, dst, label}` (F-element-links: the object form carries the authored `label`; the tuple's
/// third slot is the internal diff `status`, not an authored field). The object form is the seam
/// F-typed-edges extends with a future `type`.
fn edge_from(j: &Json) -> Option<Edge> {
    if let Some(a) = j.as_array() {
        return Some(Edge {
            src: a.first()?.as_str()?.to_string(),
            dst: a.get(1)?.as_str()?.to_string(),
            label: None,
            status: a.get(2).and_then(|v| v.as_str()).map(String::from),
        });
    }
    Some(Edge {
        src: j.get("src")?.as_str()?.to_string(),
        dst: j.get("dst")?.as_str()?.to_string(),
        label: j.get("label").and_then(|v| v.as_str()).map(String::from),
        status: None,
    })
}

/// The `col` for a lane-title `+` add (the left-edge gesture). When the target lane is **empty**
/// this is the board's current first column, so the new element aligns to the left edge *without*
/// shoving the other lanes right; when the lane already holds elements it is one column further
/// left (a true prepend, repeat-safe). Falls back to 0 on an empty board.
pub fn lane_left_col(m: &Model, kind: &str) -> i64 {
    match m.elements.iter().filter_map(|e| e.col).min() {
        None => 0,
        Some(first) if m.elements.iter().any(|e| e.kind == kind) => first - 1,
        Some(first) => first,
    }
}

/// Project a phase list to a **contiguous partition** of the timeline — sorted left→right with no
/// gaps, no overlaps, every phase at least one column wide. This is the single rule that makes the
/// frontier model (F-region-frontiers) total: whatever phase state a source carries — new atomic
/// frontier moves/splits, or legacy independent `[from_col, to_col]` spans that predate the feature
/// and could leave holes or overlaps — every `Model` lands on one clean partition. Shared by
/// `events::replay` (the log path) and `from_json` (the `model.json` bootstrap) so the invariant
/// holds whatever the source. Deterministic and **idempotent** — a partition is its own fixed point.
///
/// The rule is one left→right sweep: order phases by `(from_col, to_col, id)`; anchor the board-left
/// bound at the first phase's `from_col`; then start each phase where the previous ended (+1) and
/// keep its own `to_col` as the right edge (clamped so it never precedes the start). A frontier move
/// therefore re-borders its neighbour for free — pulling one phase's `to_col` left drags the next
/// phase's derived `from_col` with it — and a legacy overlap/hole resolves to a defined partition (a
/// clean partition renders byte-identically; an old span board may render with a named diff, the
/// accepted cost per the F-region-frontiers shaping).
pub fn normalize(phases: &mut [Phase]) {
    if phases.is_empty() {
        return;
    }
    phases.sort_by(|a, b| {
        (a.from_col, a.to_col, a.id.as_str()).cmp(&(b.from_col, b.to_col, b.id.as_str()))
    });
    let mut cursor = phases[0].from_col;
    for p in phases.iter_mut() {
        p.from_col = cursor;
        p.to_col = p.to_col.max(cursor);
        // saturating: a crafted/legacy `to_col` at i64::MAX must not panic (debug) or wrap to MIN
        // (release) — the same total-arithmetic rule `serve::mint_id` follows for suffixes.
        cursor = p.to_col.saturating_add(1);
    }
}

/// The region a column belongs to — the band whose `[from_col, to_col]` contains `col`. Membership
/// is **spatial**: there is no membership field, the band's stored bounds are the single source of
/// truth (F-container scope D2). Because every `Model`'s phases are a contiguous partition
/// (`normalize`), at most one band covers a column; the `min_by_key` is a defensive total-order
/// tiebreak that a partition never needs. Pure; `None` when no band covers it (only possible before
/// the first phase exists).
pub fn region_of(m: &Model, col: i64) -> Option<&Phase> {
    m.phases
        .iter()
        .filter(|p| p.from_col <= col && col <= p.to_col)
        .min_by_key(|p| p.to_col - p.from_col)
}

/// The ordering key an element's stored `y` denotes: clamped into `[0, 1]` (an out-of-range log
/// value must still sort *inside* its stack) with `0.5` — the neutral middle — for an unplaced
/// element. The single Rust home of the "y is an ordering key, not a position" rule: the renderer
/// sorts cell members by it and the diff compares through it, so `y: 0.5` and "no y" are one
/// state everywhere (which is also what lets an undo neutralise a placement by posting `0.5`).
pub fn y_key(y: Option<f64>) -> f64 {
    y.map(|y| y.clamp(0.0, 1.0)).unwrap_or(0.5)
}

/// Whether an element is a **pivotal event** — derived from geometry, never a stored flag
/// (F-container scope D3). The rule is type-gated and positional: an `event`-lane element whose
/// `col` sits on a region edge (`from_col` or `to_col` of any band). A pivotal event is the hinge
/// between two contexts; a command / read-model / actor on a border is not pivotal.
pub fn is_pivotal(m: &Model, e: &Element) -> bool {
    e.kind == "event"
        && e.col
            .is_some_and(|c| m.phases.iter().any(|p| c == p.from_col || c == p.to_col))
}

/// Merge two models into one annotated model: every element/edge tagged
/// added / removed / changed / moved / unchanged, keyed on stable `id` (never text or
/// position). Layout follows the *new* side (`b`); removed elements keep their old slot.
pub fn diff_models(a: &Model, b: &Model, meta: (String, String)) -> Model {
    let ea: HashMap<&str, &Element> = a.elements.iter().map(|e| (e.id.as_str(), e)).collect();
    let eb_ids: HashSet<&str> = b.elements.iter().map(|e| e.id.as_str()).collect();

    let mut elements: Vec<Element> = Vec::new();
    for e in &b.elements {
        let mut el = e.clone();
        match ea.get(e.id.as_str()) {
            None => el.diff = Some("added".into()),
            Some(old) => {
                if old.label != e.label {
                    el.diff = Some("changed".into());
                    el.was = Some(Was {
                        label: old.label.clone(),
                        col: old.col,
                        kind: old.kind.clone(),
                        y: old.y,
                    });
                } else if old.col != e.col || old.kind != e.kind || y_key(old.y) != y_key(e.y) {
                    // `y` counts: a re-placement within the lane is a position change the
                    // since-you-last-looked overlay must report, same as a col shift. Compared
                    // through `y_key`, so "no y" vs the neutral 0.5 (an undone placement) never
                    // reads as a phantom move — only a key the renderer would order differently.
                    el.diff = Some("moved".into());
                    el.was = Some(Was {
                        label: old.label.clone(),
                        col: old.col,
                        kind: old.kind.clone(),
                        y: old.y,
                    });
                } else {
                    el.diff = Some("unchanged".into());
                }
            }
        }
        elements.push(el);
    }
    for e in &a.elements {
        if !eb_ids.contains(e.id.as_str()) {
            let mut el = e.clone();
            el.diff = Some("removed".into());
            elements.push(el);
        }
    }

    let sa: HashSet<(String, String)> = a
        .edges
        .iter()
        .map(|e| (e.src.clone(), e.dst.clone()))
        .collect();
    let sb: HashSet<(String, String)> = b
        .edges
        .iter()
        .map(|e| (e.src.clone(), e.dst.clone()))
        .collect();
    let mut edges: Vec<Edge> = Vec::new();
    for e in &b.edges {
        let status = if sa.contains(&(e.src.clone(), e.dst.clone())) {
            "unchanged"
        } else {
            "added"
        };
        edges.push(Edge {
            src: e.src.clone(),
            dst: e.dst.clone(),
            label: e.label.clone(),
            status: Some(status.into()),
        });
    }
    for e in &a.edges {
        if !sb.contains(&(e.src.clone(), e.dst.clone())) {
            edges.push(Edge {
                src: e.src.clone(),
                dst: e.dst.clone(),
                label: e.label.clone(),
                status: Some("removed".into()),
            });
        }
    }

    Model {
        title: if !b.title.is_empty() {
            b.title.clone()
        } else {
            a.title.clone()
        },
        // A diff is a render-only artifact (lint never runs on it); carry the newer board's level.
        level: b.level,
        phases: diff_phases(a, b),
        elements,
        edges,
        diff_meta: Some(meta),
    }
}

/// Diff the regions of two boards, keyed on stable `id` (mirroring the element diff): each tagged
/// added / removed / renamed (label differs) / resized (bounds differ) / unchanged. Layout follows
/// the **new** side (`b`); a region only in the old side keeps its slot, tagged removed and appended.
fn diff_phases(a: &Model, b: &Model) -> Vec<Phase> {
    let old: HashMap<&str, &Phase> = a.phases.iter().map(|p| (p.id.as_str(), p)).collect();
    let new_ids: HashSet<&str> = b.phases.iter().map(|p| p.id.as_str()).collect();

    let mut phases: Vec<Phase> = Vec::new();
    for p in &b.phases {
        let mut ph = p.clone();
        ph.diff = Some(
            match old.get(p.id.as_str()) {
                None => "added",
                Some(o) if o.label != p.label => "renamed",
                Some(o) if o.from_col != p.from_col || o.to_col != p.to_col => "resized",
                Some(_) => "unchanged",
            }
            .into(),
        );
        phases.push(ph);
    }
    for p in &a.phases {
        if !new_ids.contains(p.id.as_str()) {
            let mut ph = p.clone();
            ph.diff = Some("removed".into());
            phases.push(ph);
        }
    }
    phases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn model_of(src: &str) -> Model {
        from_json(&json::parse(src).unwrap())
    }

    // ---- F-element-links: element `links` + edge `label` ----------------------------------

    #[test]
    fn element_links_parse_as_a_string_list_and_default_empty() {
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","links":["https://t/1","doc://x"]},
                {"id":"E2","type":"event","label":"B"}
            ]}"#,
        );
        assert_eq!(m.elements[0].links, vec!["https://t/1", "doc://x"]);
        assert!(
            m.elements[1].links.is_empty(),
            "no links → empty, not an error"
        );
    }

    #[test]
    fn a_malformed_links_field_is_ignored_not_fatal() {
        // Non-array `links`, and non-string entries within an array, drop silently (additive posture).
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","links":"nope"},
                {"id":"E2","type":"event","label":"B","links":["ok",7,null,"two"]}
            ]}"#,
        );
        assert!(m.elements[0].links.is_empty());
        assert_eq!(m.elements[1].links, vec!["ok", "two"]);
    }

    #[test]
    fn an_edge_object_form_carries_a_label_the_tuple_form_has_none() {
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A"},
                {"id":"E2","type":"event","label":"B"},
                {"id":"E3","type":"event","label":"C"}
            ],"edges":[
                ["E1","E2"],
                {"src":"E2","dst":"E3","label":"then"}
            ]}"#,
        );
        assert_eq!(m.edges[0].src, "E1");
        assert_eq!(m.edges[0].label, None, "tuple form → no authored label");
        assert_eq!(m.edges[1].src, "E2");
        assert_eq!(m.edges[1].label.as_deref(), Some("then"));
    }

    // ---- F-es-lint: board level ------------------------------------------------------------

    #[test]
    fn level_defaults_to_big_picture_when_absent() {
        let m = model_of(r#"{"elements":[]}"#);
        assert_eq!(m.level, Level::BigPicture);
    }

    #[test]
    fn level_design_is_parsed_from_the_top_level_field() {
        let m = model_of(r#"{"level":"design","elements":[]}"#);
        assert_eq!(m.level, Level::Design);
    }

    #[test]
    fn an_unknown_or_explicit_big_picture_level_falls_back_to_big_picture() {
        assert_eq!(level_from_str("big-picture"), Level::BigPicture);
        assert_eq!(level_from_str("nonsense"), Level::BigPicture);
        assert_eq!(model_of(r#"{"level":"whatever"}"#).level, Level::BigPicture);
    }

    #[test]
    fn level_to_str_is_the_inverse_of_level_from_str() {
        for level in [Level::BigPicture, Level::Design] {
            assert_eq!(level_from_str(level_to_str(level)), level);
        }
    }

    // ---- F-container Stage 2: spatial membership + derived pivotal -------------------------
    // Membership and pivotal are read from geometry, not stored. These pin the two rules the
    // later render/UI stages lean on: which band a col is in, and whether an event sits on a border.

    #[test]
    fn from_json_normalizes_overlapping_bands_into_a_partition() {
        // Overlaps are unrepresentable under F-region-frontiers: `from_json` runs `normalize`, so an
        // "Inner" band nested inside "Outer" resolves deterministically to a contiguous partition —
        // Outer keeps its own [0,9], Inner is swept to the column just past it. Every column is then
        // in exactly one band (no innermost tiebreak needed).
        let m = model_of(
            r#"{"phases":[
                {"id":"K1","label":"Outer","fromCol":0,"toCol":9},
                {"id":"K2","label":"Inner","fromCol":3,"toCol":5}]}"#,
        );
        assert_eq!(m.phases.len(), 2);
        let bounds = |id: &str| {
            m.phases
                .iter()
                .find(|p| p.id == id)
                .map(|p| (p.from_col, p.to_col))
        };
        assert_eq!(bounds("K1"), Some((0, 9)), "outer keeps its span");
        assert_eq!(
            bounds("K2"),
            Some((10, 10)),
            "nested inner swept past outer"
        );
        assert_eq!(
            region_of(&m, 4).map(|p| p.id.as_str()),
            Some("K1"),
            "4 in K1"
        );
        assert_eq!(
            region_of(&m, 10).map(|p| p.id.as_str()),
            Some("K2"),
            "10 in K2"
        );
        assert_eq!(
            region_of(&m, 12).map(|p| p.id.as_str()),
            None,
            "no band covers 12"
        );
        assert_eq!(
            region_of(&Model::default(), 0).map(|p| p.id.as_str()),
            None,
            "no bands"
        );
    }

    #[test]
    fn normalize_is_idempotent_and_gap_free() {
        // A partition is normalize's fixed point; a gapped/overlapping input becomes contiguous.
        let mut ps = vec![
            Phase {
                id: "K1".into(),
                label: "A".into(),
                from_col: 0,
                to_col: 3,
                diff: None,
            },
            Phase {
                id: "K3".into(),
                label: "C".into(),
                from_col: 8,
                to_col: 10,
                diff: None,
            },
            Phase {
                id: "K2".into(),
                label: "B".into(),
                from_col: 2,
                to_col: 5,
                diff: None,
            },
        ];
        normalize(&mut ps);
        let spans: Vec<_> = ps
            .iter()
            .map(|p| (p.id.as_str(), p.from_col, p.to_col))
            .collect();
        assert_eq!(
            spans,
            vec![("K1", 0, 3), ("K2", 4, 5), ("K3", 6, 10)],
            "contiguous, ordered"
        );
        let before = ps.clone();
        normalize(&mut ps);
        let after: Vec<_> = ps
            .iter()
            .map(|p| (p.id.clone(), p.from_col, p.to_col))
            .collect();
        let was: Vec<_> = before
            .iter()
            .map(|p| (p.id.clone(), p.from_col, p.to_col))
            .collect();
        assert_eq!(after, was, "idempotent");
    }

    #[test]
    fn is_pivotal_is_an_event_on_a_band_edge_only() {
        // K1 spans cols 0..=3. An event ON an edge (0 or 3) is pivotal; one inside is not.
        let m = model_of(
            r#"{"phases":[{"id":"K1","label":"A","fromCol":0,"toCol":3}],
                "elements":[
                    {"id":"E1","type":"event","label":"OnEdge","col":3},
                    {"id":"E2","type":"event","label":"Inside","col":1},
                    {"id":"C1","type":"command","label":"AlsoOnEdge","col":3}]}"#,
        );
        let by = |id: &str| m.elements.iter().find(|e| e.id == id).unwrap();
        assert!(
            is_pivotal(&m, by("E1")),
            "event on the band edge is pivotal"
        );
        assert!(!is_pivotal(&m, by("E2")), "event inside the band is not");
        assert!(
            !is_pivotal(&m, by("C1")),
            "type-gated: a command on the edge is not pivotal"
        );
    }

    #[test]
    fn diff_tags_regions_by_stable_id() {
        let a = model_of(
            r#"{"phases":[
                {"id":"K1","label":"Same","fromCol":0,"toCol":2},
                {"id":"K2","label":"Old","fromCol":3,"toCol":4},
                {"id":"K3","label":"Grows","fromCol":5,"toCol":6},
                {"id":"K4","label":"GoneSoon","fromCol":7,"toCol":8}]}"#,
        );
        let b = model_of(
            r#"{"phases":[
                {"id":"K1","label":"Same","fromCol":0,"toCol":2},
                {"id":"K2","label":"New","fromCol":3,"toCol":4},
                {"id":"K3","label":"Grows","fromCol":5,"toCol":9},
                {"id":"K5","label":"BrandNew","fromCol":10,"toCol":11}]}"#,
        );
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        let tag = |id: &str| {
            d.phases
                .iter()
                .find(|p| p.id == id)
                .and_then(|p| p.diff.as_deref())
        };
        assert_eq!(tag("K1"), Some("unchanged"));
        assert_eq!(tag("K2"), Some("renamed"), "label differs");
        assert_eq!(tag("K3"), Some("resized"), "bounds differ");
        assert_eq!(tag("K4"), Some("removed"));
        assert_eq!(tag("K5"), Some("added"));
    }

    // The lane-title `+` aligns a lane's *first* element to the board's existing left column (no
    // shift of the other lanes), but a *prepend* into a non-empty lane marches one column further
    // left (repeat-safe). Empty board falls back to 0.
    #[test]
    fn lane_left_col_aligns_a_first_element_but_prepends_within_a_lane() {
        assert_eq!(lane_left_col(&Model::default(), "event"), 0, "empty board");
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","col":3},
                {"id":"E2","type":"event","label":"B","col":5}]}"#,
        );
        // first element of an *empty* lane lands in the board's first column — no shift.
        assert_eq!(
            lane_left_col(&m, "actor"),
            3,
            "empty lane aligns to first col"
        );
        // a *non-empty* lane prepends one column further left.
        assert_eq!(lane_left_col(&m, "event"), 2, "non-empty lane prepends");
        // after one prepend the lowest col is 2; the next must march to 1, not back to 3.
        let m2 = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","col":2},
                {"id":"E2","type":"event","label":"B","col":3}]}"#,
        );
        assert_eq!(lane_left_col(&m2, "event"), 1, "repeat marches left");
    }

    fn tag<'a>(m: &'a Model, id: &str) -> Option<&'a str> {
        m.elements
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.diff.as_deref())
    }

    // The whole comment/diff contract hinges on `id` being identity, never text
    // or position. This pins each diff verdict to the right join.
    #[test]
    fn diff_tags_elements_by_stable_id() {
        let a = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"Created","col":1},
                {"id":"E2","type":"event","label":"GoneSoon","col":2},
                {"id":"E3","type":"event","label":"Same","col":3},
                {"id":"E4","type":"event","label":"MoveMe","col":4}
            ]}"#,
        );
        let b = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"Created v2","col":1},
                {"id":"E3","type":"event","label":"Same","col":3},
                {"id":"E4","type":"event","label":"MoveMe","col":5},
                {"id":"E5","type":"event","label":"BrandNew","col":6}
            ]}"#,
        );
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        assert_eq!(tag(&d, "E1"), Some("changed"));
        assert_eq!(tag(&d, "E2"), Some("removed"));
        assert_eq!(tag(&d, "E3"), Some("unchanged"));
        assert_eq!(tag(&d, "E4"), Some("moved"));
        assert_eq!(tag(&d, "E5"), Some("added"));
    }

    #[test]
    fn changed_element_remembers_its_former_label() {
        let a = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"Old","col":1}]}"#);
        let b = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"New","col":1}]}"#);
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        let e1 = d.elements.iter().find(|e| e.id == "E1").unwrap();
        assert_eq!(e1.was.as_ref().map(|w| w.label.as_str()), Some("Old"));
    }

    // Same label, different lane: a relocation, not an edit.
    #[test]
    fn changing_type_alone_reads_as_moved() {
        let a = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"X","col":1}]}"#);
        let b = model_of(r#"{"elements":[{"id":"E1","type":"command","label":"X","col":1}]}"#);
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        assert_eq!(tag(&d, "E1"), Some("moved"));
    }

    #[test]
    fn edges_diff_on_their_endpoints() {
        let a = model_of(r#"{"elements":[],"edges":[["E1","E3"]]}"#);
        let b = model_of(r#"{"elements":[],"edges":[["E1","E3"],["E1","E5"]]}"#);
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        let status = |s: &str, t: &str| {
            d.edges
                .iter()
                .find(|e| e.src == s && e.dst == t)
                .and_then(|e| e.status.clone())
        };
        assert_eq!(status("E1", "E3"), Some("unchanged".into()));
        assert_eq!(status("E1", "E5"), Some("added".into()));
    }

    #[test]
    fn optional_fields_fall_back_to_defaults() {
        let m = model_of(r#"{"title":"t","elements":[{"id":"E1","type":"event","label":"L"}]}"#);
        assert_eq!(m.title, "t");
        let e = &m.elements[0];
        assert_eq!(e.col, None);
        assert!(!e.resolved);
        assert!(e.detail.is_none());
        assert!(e.y.is_none());
    }

    // F-2d-placement: `y` reads from the file and a y-only re-placement is a *position* change —
    // the overlay must report it as moved, keyed on the same stable id as every other verdict.
    #[test]
    fn a_y_only_change_reads_as_moved() {
        let a = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"X","col":1}]}"#);
        let b =
            model_of(r#"{"elements":[{"id":"E1","type":"event","label":"X","col":1,"y":0.75}]}"#);
        assert_eq!(b.elements[0].y, Some(0.75), "y reads from the file");
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        assert_eq!(tag(&d, "E1"), Some("moved"));
    }

    // The diff compares y through `y_key`, where "no y" and the neutral 0.5 are one state (an
    // undone placement posts 0.5) — the overlay must not announce a phantom "repositioned".
    #[test]
    fn a_neutral_y_vs_no_y_reads_as_unchanged() {
        let a = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"X","col":1}]}"#);
        let b =
            model_of(r#"{"elements":[{"id":"E1","type":"event","label":"X","col":1,"y":0.5}]}"#);
        let d = diff_models(&a, &b, ("old".into(), "new".into()));
        assert_eq!(tag(&d, "E1"), Some("unchanged"));
    }

    #[test]
    fn y_key_clamps_and_defaults_to_the_neutral_middle() {
        assert_eq!(y_key(None), 0.5);
        assert_eq!(y_key(Some(0.2)), 0.2);
        assert_eq!(y_key(Some(7.0)), 1.0, "out-of-range clamps into the stack");
        assert_eq!(y_key(Some(-3.0)), 0.0);
    }
}
