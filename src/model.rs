//! The typed board model + the stable-id diff.
//!
//! A board is elements (coloured stickies) on a shared left→right column axis, grouped
//! into lanes by `type`, connected by directed edges. Identity is the stable `id`
//! (never text or position) — that is the contract the comment sidecar and the diff rely on.

use crate::json::{self, Json};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone)]
pub struct Phase {
    pub label: String,
    pub from_col: i64,
    pub to_col: i64,
}

#[derive(Clone)]
pub struct Was {
    pub label: String,
    pub col: Option<i64>,
    pub kind: String,
}

#[derive(Clone)]
pub struct Element {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub col: Option<i64>,
    pub detail: Option<String>,
    pub resolved: bool,
    // diff annotations (not in the file)
    pub diff: Option<String>,
    pub was: Option<Was>,
}

#[derive(Clone)]
pub struct Edge {
    pub src: String,
    pub dst: String,
    pub status: Option<String>,
}

#[derive(Clone, Default)]
pub struct Model {
    pub title: String,
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
    let phases = j
        .get("phases")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(phase_from).collect())
        .unwrap_or_default();
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
        phases,
        elements,
        edges,
        diff_meta: None,
    }
}

fn phase_from(j: &Json) -> Option<Phase> {
    Some(Phase {
        label: j.get("label")?.as_str()?.to_string(),
        from_col: j.get("fromCol")?.as_f64()? as i64,
        to_col: j.get("toCol")?.as_f64()? as i64,
    })
}

fn element_from(j: &Json) -> Option<Element> {
    Some(Element {
        id: j.get("id")?.as_str()?.to_string(),
        kind: j.get("type")?.as_str()?.to_string(),
        label: j.get("label")?.as_str()?.to_string(),
        col: j.get("col").and_then(|v| v.as_f64()).map(|n| n as i64),
        detail: j.get("detail").and_then(|v| v.as_str()).map(String::from),
        resolved: j.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false),
        diff: None,
        was: None,
    })
}

fn edge_from(j: &Json) -> Option<Edge> {
    let a = j.as_array()?;
    Some(Edge {
        src: a.first()?.as_str()?.to_string(),
        dst: a.get(1)?.as_str()?.to_string(),
        status: a.get(2).and_then(|v| v.as_str()).map(String::from),
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
                    });
                } else if old.col != e.col || old.kind != e.kind {
                    el.diff = Some("moved".into());
                    el.was = Some(Was {
                        label: old.label.clone(),
                        col: old.col,
                        kind: old.kind.clone(),
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
            status: Some(status.into()),
        });
    }
    for e in &a.edges {
        if !sb.contains(&(e.src.clone(), e.dst.clone())) {
            edges.push(Edge {
                src: e.src.clone(),
                dst: e.dst.clone(),
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
        phases: if !b.phases.is_empty() {
            b.phases.clone()
        } else {
            a.phases.clone()
        },
        elements,
        edges,
        diff_meta: Some(meta),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn model_of(src: &str) -> Model {
        from_json(&json::parse(src).unwrap())
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
    }
}
