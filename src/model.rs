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
