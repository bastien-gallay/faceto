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
    // runtime / diff annotations (not in the file)
    pub x: f64,
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
        x: 0.0,
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
