//! Shared `#[cfg(test)]` harness for the serve suite: `added` / `region_added` event
//! builders and `model_of` (parse a model.json string to a `Model`).

use crate::model::{Lane, Model};
use crate::{events, json, model};

pub(crate) fn added(id: &str, kind: Lane) -> events::Event {
    events::Event::ElementAdded {
        id: id.into(),
        kind,
        label: id.into(),
        col: None,
        detail: None,
        y: None,
        links: Vec::new(),
    }
}

pub(crate) fn region_added(id: &str, from_col: i64, to_col: i64) -> events::Event {
    events::Event::PhaseAdded {
        id: Some(id.into()),
        label: id.into(),
        from_col,
        to_col,
    }
}

pub(crate) fn model_of(src: &str) -> Model {
    model::from_json(&json::parse(src).unwrap())
}
