//! The typed board model + the stable-id diff.
//!
//! A board is elements (coloured stickies) on a shared left→right column axis, grouped
//! into lanes by `type`, connected by directed edges. Identity is the stable `id`
//! (never text or position) — that is the contract the comment sidecar and the diff rely on.

use crate::json::{self, Json};
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

/// The board format a log or model file declares — which projector replays it. Sealed on
/// purpose (`docs/multi-format-architecture.md` §"The Format seam"): dispatch is one `match`, not
/// a `dyn Format`. One variant today; the tag exists so a *foreign* log is rejected loudly instead
/// of replaying as a silently empty event-storming board (F-format-tag, constraint 1 of the
/// canvas spike).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Format {
    #[default]
    EventStorming,
}

/// Parse a board `format` string, or `None` if this build does not speak it. Unlike
/// [`level_from_str`], an unrecognised value is **not** folded into the default: a board whose
/// format we cannot project is the one case where a lenient read produces a confidently wrong
/// empty board. The single parse point shared by `from_json` (model.json) and the log codec.
pub fn format_from_str(s: &str) -> Option<Format> {
    match s {
        "event-storming" => Some(Format::EventStorming),
        _ => None,
    }
}

/// The wire string for a `Format` — the reverse of [`format_from_str`], exhaustive for the same
/// reason [`level_to_str`] is: a future variant must declare its wire form or fail to compile.
pub fn format_to_str(format: Format) -> &'static str {
    match format {
        Format::EventStorming => "event-storming",
    }
}

/// Resolve a declared format tag at a read boundary. Absent → the default (`event-storming`), the
/// same additive rule `level` uses, so every file written before the tag existed reads unchanged.
/// Present but unrecognised → an error naming the format, because continuing would render a board
/// this build cannot project as an empty one.
pub fn format_declared(tag: Option<&str>) -> Result<Format, String> {
    match tag {
        None => Ok(Format::default()),
        Some(s) => format_from_str(s).ok_or_else(|| {
            format!(
                "board format {:?} is not one this faceto speaks (it reads {}) — \
                 the file is from another format, or from a newer faceto",
                s,
                format_to_str(Format::default())
            )
        }),
    }
}

/// The eight-lane event-storming grammar — a sticky's `type`, which selects **both** its lane and
/// its colour. Closed on purpose: an off-grammar element has no lane to occupy, and every reader
/// that placed one had to carry a fallback for a state the board could not draw. As a type, the
/// state is gone: `colour`, `lane_index` and `lane_prefix` are total, and the three "drop the
/// lane-less stickies" filters `render` used to run before drawing are unreachable code.
///
/// Ordering here is declaration order, not board order — the visual top-to-bottom sequence is
/// `render::style::LANES`, which is a render concern.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lane {
    Actor,
    Command,
    Aggregate,
    Event,
    Policy,
    ReadModel,
    External,
    Hotspot,
}

/// Every lane, in the canonical event-storming order (`actor` first, `hotspot` last) — the board
/// draws them top to bottom in exactly this sequence. One array, so the grammar's *set* and its
/// *order* can never drift apart the way a separate render-side list used to allow.
pub const LANES: [Lane; 8] = [
    Lane::Actor,
    Lane::Command,
    Lane::Aggregate,
    Lane::Event,
    Lane::Policy,
    Lane::ReadModel,
    Lane::External,
    Lane::Hotspot,
];

/// Parse a sticky's `type`, or `None` if it names no lane this build knows. The single parse point
/// shared by `from_json` (model.json), the log codec, `extract`'s `--type` selector and `serve`'s
/// `add` guard, so none of them can disagree about what the grammar is.
///
/// A `None` is **skipped, never fatal**: field *values* evolve additively exactly as fields do, so
/// a log naming a lane a future faceto adds must still read here (F-es-vocabulary is the one that
/// will add `timer` / `process`). The element is dropped — the same thing that visibly happened
/// before, one seam earlier.
pub fn lane_from_str(s: &str) -> Option<Lane> {
    Some(match s {
        "actor" => Lane::Actor,
        "command" => Lane::Command,
        "aggregate" => Lane::Aggregate,
        "event" => Lane::Event,
        "policy" => Lane::Policy,
        "readmodel" => Lane::ReadModel,
        "external" => Lane::External,
        "hotspot" => Lane::Hotspot,
        _ => return None,
    })
}

/// The wire string for a `Lane` — the reverse of [`lane_from_str`], and the only place a lane
/// becomes text again (JSON, SVG `data-*`, the context pack). Exhaustive, so a new variant must
/// declare its wire form or fail to compile.
pub fn lane_to_str(lane: Lane) -> &'static str {
    match lane {
        Lane::Actor => "actor",
        Lane::Command => "command",
        Lane::Aggregate => "aggregate",
        Lane::Event => "event",
        Lane::Policy => "policy",
        Lane::ReadModel => "readmodel",
        Lane::External => "external",
        Lane::Hotspot => "hotspot",
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
}

#[derive(Clone, PartialEq, Debug)]
pub struct Element {
    pub id: String,
    /// The sticky's lane, from the closed eight-lane grammar — the `"type"` key on the wire.
    pub kind: Lane,
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
}

#[derive(Clone, PartialEq, Debug)]
pub struct Edge {
    pub src: String,
    pub dst: String,
    /// Optional human label for the connection (F-element-links) — drawn at the edge midpoint so
    /// connection kinds stop reading identically. Shares the `Edge` seam with F-typed-edges (a
    /// future additive `type`); touch it once.
    pub label: Option<String>,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Model {
    pub title: String,
    /// The board format this model is projected under. Selects which projector replays the log;
    /// `EventStorming` (the default) is the only one this build ships. See [`Format`].
    pub format: Format,
    /// Modeling granularity — `BigPicture` (default) or `Design`. Read by `crate::lint` to decide
    /// which rules apply; never affects rendering. See [`Level`].
    pub level: Level,
    pub phases: Vec<Phase>,
    pub elements: Vec<Element>,
    pub edges: Vec<Edge>,
}

pub fn load(path: &Path) -> Result<Model, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let j = json::parse(&raw)?;
    format_declared(j.get("format").and_then(|v| v.as_str()))?;
    Ok(from_json(&j))
}

pub fn from_json(j: &Json) -> Model {
    let title = j
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("board")
        .to_string();
    // Lenient here on purpose: `load` is the boundary that rejects a format this build cannot
    // project, so an unrecognised tag reaching this far is a caller building a model in-process.
    let format = format_declared(j.get("format").and_then(|v| v.as_str())).unwrap_or_default();
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
        format,
        level,
        phases,
        elements,
        edges,
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
        // An off-grammar `type` drops the element here rather than downstream: it names no lane,
        // so there is nothing for the board to draw and nothing for lint to judge.
        kind: lane_from_str(j.get("type")?.as_str()?)?,
        label: j.get("label")?.as_str()?.to_string(),
        col: j.get("col").and_then(|v| v.as_f64()).map(|n| n as i64),
        detail: j.get("detail").and_then(|v| v.as_str()).map(String::from),
        y: j.get("y").and_then(|v| v.as_f64()),
        resolved: j.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false),
        links: links_from(j.get("links")),
    })
}

/// An edge is a positional tuple `[src, dst]` **or** an object `{src, dst, label}`
/// (F-element-links: the object form carries the authored `label`). Extra tuple slots are ignored:
/// the third one used to seed the internal diff `status`, which let an authored file paint an
/// overlay wire on an ordinary board — the diff channel belongs to `render`, never to the model
/// file. The object form is the seam F-typed-edges extends with a future `type`.
fn edge_from(j: &Json) -> Option<Edge> {
    if let Some(a) = j.as_array() {
        return Some(Edge {
            src: a.first()?.as_str()?.to_string(),
            dst: a.get(1)?.as_str()?.to_string(),
            label: None,
        });
    }
    Some(Edge {
        src: j.get("src")?.as_str()?.to_string(),
        dst: j.get("dst")?.as_str()?.to_string(),
        label: j.get("label").and_then(|v| v.as_str()).map(String::from),
    })
}

/// Resolve every element's column, filling in the ones the file left out: a missing `col`
/// auto-assigns in **file order**, counting up from 0. Returns one column per element, positionally
/// — the input is untouched, so nothing has to clone a board to ask where its stickies sit.
///
/// This is a domain rule, not a drawing detail (`col` is the global timeline coordinate), and it
/// has two callers that must agree: the renderer places stickies by it, and `extract` selects a
/// region by it. When they disagreed, `--region` silently dropped `col`-less elements the board
/// was visibly drawing *inside* that band — what you saw was not what you cut.
///
/// One divergence is deliberate and harmless: the renderer numbers only the elements it can place
/// (it drops kinds outside the 8-lane grammar first), so a board carrying an off-grammar,
/// `col`-less sticky can number the rest one step higher here. Such an element is not drawn at
/// all, so it cannot be seen inside a band either way.
pub fn resolved_cols(elements: &[Element]) -> Vec<i64> {
    let mut auto = 0;
    elements
        .iter()
        .map(|e| {
            e.col.unwrap_or_else(|| {
                let c = auto;
                auto += 1;
                c
            })
        })
        .collect()
}

/// The `col` for a lane-title `+` add (the left-edge gesture). When the target lane is **empty**
/// this is the board's current first column, so the new element aligns to the left edge *without*
/// shoving the other lanes right; when the lane already holds elements it is one column further
/// left (a true prepend, repeat-safe). Falls back to 0 on an empty board.
pub fn lane_left_col(m: &Model, kind: Lane) -> i64 {
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
    e.kind == Lane::Event
        && e.col
            .is_some_and(|c| m.phases.iter().any(|p| c == p.from_col || c == p.to_col))
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

    // ---- F-format-tag: board format ---------------------------------------------------------

    #[test]
    fn an_off_grammar_type_drops_the_element_at_the_parse_boundary() {
        // `type` picks the lane, and there is no ninth lane to put a sticky in. Such an element
        // used to enter the model and be filtered out again by each renderer; now it never becomes
        // an `Element`, which is what makes `colour` / `lane_index` / `lane_prefix` total.
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A"},
                {"id":"W1","type":"widget","label":"B"}]}"#,
        );
        assert_eq!(m.elements.len(), 1);
        assert_eq!(m.elements[0].id, "E1");
        assert_eq!(lane_from_str("widget"), None);
    }

    #[test]
    fn format_defaults_to_event_storming_when_absent() {
        assert_eq!(format_declared(None), Ok(Format::EventStorming));
        assert_eq!(model_of(r#"{"elements":[]}"#).format, Format::EventStorming);
    }

    #[test]
    fn an_explicit_event_storming_format_is_parsed() {
        let m = model_of(r#"{"format":"event-storming","elements":[]}"#);
        assert_eq!(m.format, Format::EventStorming);
    }

    #[test]
    fn an_unrecognised_format_is_an_error_not_the_default() {
        let err = format_declared(Some("bounded-context-canvas")).unwrap_err();
        assert!(err.contains("bounded-context-canvas"), "{}", err);
        assert_eq!(format_from_str("bounded-context-canvas"), None);
    }

    #[test]
    fn load_refuses_a_model_file_declaring_a_format_this_build_cannot_project() {
        // The model.json half of the same guard `parse_log` gives the log: without it, a foreign
        // board renders as an empty event-storming one and exits 0.
        let dir = std::env::temp_dir().join(format!("faceto-fmt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("foreign.model.json");
        std::fs::write(
            &path,
            r#"{"format":"bounded-context-canvas","elements":[]}"#,
        )
        .unwrap();
        let err = load(&path).unwrap_err();
        assert!(err.contains("bounded-context-canvas"), "{}", err);

        std::fs::write(&path, r#"{"format":"event-storming","elements":[]}"#).unwrap();
        assert_eq!(load(&path).unwrap().format, Format::EventStorming);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_to_str_is_the_inverse_of_format_from_str() {
        // One variant today, so this reads as a single assertion rather than the loop its `level`
        // sibling uses; `format_to_str` is exhaustive, so a second format cannot skip it.
        let format = Format::EventStorming;
        assert_eq!(format_from_str(format_to_str(format)), Some(format));
    }

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
            },
            Phase {
                id: "K3".into(),
                label: "C".into(),
                from_col: 8,
                to_col: 10,
            },
            Phase {
                id: "K2".into(),
                label: "B".into(),
                from_col: 2,
                to_col: 5,
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

    // The lane-title `+` aligns a lane's *first* element to the board's existing left column (no
    // shift of the other lanes), but a *prepend* into a non-empty lane marches one column further
    // left (repeat-safe). Empty board falls back to 0.
    #[test]
    fn lane_left_col_aligns_a_first_element_but_prepends_within_a_lane() {
        assert_eq!(
            lane_left_col(&Model::default(), Lane::Event),
            0,
            "empty board"
        );
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","col":3},
                {"id":"E2","type":"event","label":"B","col":5}]}"#,
        );
        // first element of an *empty* lane lands in the board's first column — no shift.
        assert_eq!(
            lane_left_col(&m, Lane::Actor),
            3,
            "empty lane aligns to first col"
        );
        // a *non-empty* lane prepends one column further left.
        assert_eq!(lane_left_col(&m, Lane::Event), 2, "non-empty lane prepends");
        // after one prepend the lowest col is 2; the next must march to 1, not back to 3.
        let m2 = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","col":2},
                {"id":"E2","type":"event","label":"B","col":3}]}"#,
        );
        assert_eq!(lane_left_col(&m2, Lane::Event), 1, "repeat marches left");
    }

    // The tuple's third slot was the internal diff channel, reachable from an authored file: a
    // hand-written `["E1","E3","added"]` painted a green overlay wire on a plain board. It is read
    // no more — a tuple is two ids and nothing else.
    #[test]
    fn an_edge_tuple_is_two_ids_and_nothing_else() {
        let m = model_of(r#"{"edges":[["E1","E3","added"],["E1","E5"]]}"#);
        assert_eq!(m.edges.len(), 2);
        assert!(m.edges.iter().all(|e| e.label.is_none()));
        assert_eq!(
            (m.edges[0].src.as_str(), m.edges[0].dst.as_str()),
            ("E1", "E3")
        );
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

    #[test]
    fn y_key_clamps_and_defaults_to_the_neutral_middle() {
        assert_eq!(y_key(None), 0.5);
        assert_eq!(y_key(Some(0.2)), 0.2);
        assert_eq!(y_key(Some(7.0)), 1.0, "out-of-range clamps into the stack");
        assert_eq!(y_key(Some(-3.0)), 0.0);
    }
}
