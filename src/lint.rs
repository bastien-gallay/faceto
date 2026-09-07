//! ES-grammar lint — a pure graph pass over a `Model` that flags the event-storming
//! grammar defects a workshop review would otherwise raise by hand.
//!
//! The base rules here are the ones a real 147-element board's review surfaced (issue #13 §3):
//! every substantive review comment was a mechanical grammar defect. Lint is **warn-only** — a
//! big-picture board is legitimately incomplete, so a finding is a nudge, never a gate (the CLI
//! always exits 0). Identity is the stable `id`, so a `Finding` carries `element_id`: the same
//! join key the comment sidecar uses, which is what later lets findings flow into it.
//!
//! This stage is pure and depends on nothing downstream: `Model -> Vec<Finding>`, no IO, no
//! clocks. The CLI (`main.rs`) owns loading the board and printing the findings.

use crate::model::Lane;
use crate::model::{Level, Model};
use std::collections::HashSet;

/// One grammar finding, keyed on the offending element's stable `id`.
pub struct Finding {
    /// A stable, machine-readable rule id (e.g. `"event-no-producer"`) — never localised, so a
    /// caller can group or suppress by rule without parsing prose.
    pub rule: &'static str,
    /// The stable `id` of the element the finding is about — the comment-sidecar join key.
    pub element_id: String,
    /// A calm, human-readable one-line explanation of the defect. Fixed catalog text, like
    /// `rule` — not built per-element, so it is a `&'static str`, not an owned `String`.
    pub message: &'static str,
}

/// Run the ES-grammar rules over a board, returning findings in a deterministic order
/// (element file-order, then rule order) so the output never churns between runs.
///
/// The rules, all warn-only. The first four apply at every level; the last only when the board
/// declares `level: design` (see [`crate::model::Level`]) — a first-pass big-picture board
/// legitimately sketches commands before their events, so gating it there avoids false positives.
/// - **event-no-producer** — an `event` with no incoming edge: nothing emits it.
/// - **policy-no-input** — a `policy` with no incoming edge: nothing triggers it.
/// - **policy-no-output** — a `policy` with no outgoing edge: it triggers nothing.
/// - **event-dead-end** — an `event` with no outgoing edge: a dead end unless it is terminal.
/// - **command-no-output** *(design only)* — a `command` with no outgoing edge: it emits no event.
pub fn lint(m: &Model) -> Vec<Finding> {
    // O(V + E): an element has a producer (resp. consumer) iff some *real* edge ends (resp.
    // starts) at it. A real edge connects two distinct existing elements — a self-loop
    // (`src == dst`) is not a producer/consumer of itself, and a dangling edge to a since-deleted
    // or mistyped id is not one either. Without this guard a naive endpoint test would see the
    // stray edge and silently mask the very missing-producer / dead-end defect lint exists to catch.
    let ids: HashSet<&str> = m.elements.iter().map(|e| e.id.as_str()).collect();
    let has_inbound: HashSet<&str> = m
        .edges
        .iter()
        .filter(|e| e.src != e.dst && ids.contains(e.src.as_str()))
        .map(|e| e.dst.as_str())
        .collect();
    let has_outbound: HashSet<&str> = m
        .edges
        .iter()
        .filter(|e| e.src != e.dst && ids.contains(e.dst.as_str()))
        .map(|e| e.src.as_str())
        .collect();

    let mut findings = Vec::new();
    for e in &m.elements {
        let id = e.id.as_str();
        let inbound = has_inbound.contains(id);
        let outbound = has_outbound.contains(id);
        match e.kind {
            Lane::Event => {
                if !inbound {
                    findings.push(Finding {
                        rule: "event-no-producer",
                        element_id: e.id.clone(),
                        message: "no producer: nothing emits this event (no incoming edge)",
                    });
                }
                if !outbound {
                    findings.push(Finding {
                        rule: "event-dead-end",
                        element_id: e.id.clone(),
                        message: "no outbound edge: a dead end unless this event is terminal",
                    });
                }
            }
            Lane::Policy => {
                if !inbound {
                    findings.push(Finding {
                        rule: "policy-no-input",
                        element_id: e.id.clone(),
                        message: "no input: nothing triggers this policy (no incoming edge)",
                    });
                }
                if !outbound {
                    findings.push(Finding {
                        rule: "policy-no-output",
                        element_id: e.id.clone(),
                        message: "no output: this policy triggers nothing (no outgoing edge)",
                    });
                }
            }
            // Design-level only: a command that emits no event. At `big-picture` a command
            // sketched before its event is legitimate incompleteness, so the rule is gated to a
            // filled-in `design` board — the same producer/consumer obligation the event/policy
            // rules enforce, applied to a command's outbound side.
            Lane::Command if m.level == Level::Design && !outbound => {
                findings.push(Finding {
                    rule: "command-no-output",
                    element_id: e.id.clone(),
                    message: "no output: this command emits no event (no outgoing edge)",
                });
            }
            _ => {}
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn model_of(src: &str) -> Model {
        crate::model::from_json(&json::parse(src).unwrap())
    }

    fn rules_for<'a>(fs: &'a [Finding], id: &str) -> Vec<&'a str> {
        fs.iter()
            .filter(|f| f.element_id == id)
            .map(|f| f.rule)
            .collect()
    }

    // A grammar-clean chain (the shape of examples/sample.model.json): every event has a
    // producer and an outbound edge, every policy is wired both sides. Lint must stay silent —
    // the calm first impression the tool depends on.
    #[test]
    fn a_well_formed_chain_yields_no_findings() {
        let m = model_of(
            r#"{"elements":[
                {"id":"X1","type":"actor","label":"Op","col":0},
                {"id":"C1","type":"command","label":"do","col":0},
                {"id":"E1","type":"event","label":"Done","col":1},
                {"id":"P1","type":"policy","label":"when Done","col":2},
                {"id":"E2","type":"event","label":"Next","col":3},
                {"id":"R1","type":"readmodel","label":"view","col":4}],
              "edges":[["X1","C1"],["C1","E1"],["E1","P1"],["P1","E2"],["E2","R1"]]}"#,
        );
        assert!(lint(&m).is_empty(), "a well-formed chain is silent");
    }

    #[test]
    fn an_event_with_no_incoming_edge_has_no_producer() {
        // E1 is emitted by nothing; it does flow onward (E1 -> R1), so only the producer rule fires.
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"Appears","col":1},
                {"id":"R1","type":"readmodel","label":"view","col":2}],
              "edges":[["E1","R1"]]}"#,
        );
        assert_eq!(rules_for(&lint(&m), "E1"), vec!["event-no-producer"]);
    }

    #[test]
    fn a_non_terminal_event_with_no_outgoing_edge_is_a_dead_end() {
        // C1 -> E1 gives E1 a producer; nothing consumes E1, so only the dead-end rule fires.
        let m = model_of(
            r#"{"elements":[
                {"id":"C1","type":"command","label":"do","col":0},
                {"id":"E1","type":"event","label":"Stuck","col":1}],
              "edges":[["C1","E1"]]}"#,
        );
        assert_eq!(rules_for(&lint(&m), "E1"), vec!["event-dead-end"]);
    }

    // A self-loop is not a real connection: an element wired only to itself has no external
    // producer or consumer, so both event rules must still fire. Regression — a naive endpoint
    // membership test would see inbound+outbound and stay silent.
    #[test]
    fn a_self_looped_event_is_not_its_own_producer_or_consumer() {
        let m = model_of(
            r#"{"elements":[{"id":"E1","type":"event","label":"Loop","col":0}],
              "edges":[["E1","E1"]]}"#,
        );
        assert_eq!(
            rules_for(&lint(&m), "E1"),
            vec!["event-no-producer", "event-dead-end"]
        );
    }

    // A dangling edge whose other end is not a real element is not a producer: GHOST does not
    // exist, so E1 still has no producer (E1 -> R1 gives it a real consumer, isolating the rule).
    #[test]
    fn a_dangling_edge_from_a_missing_id_is_not_a_producer() {
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"X","col":0},
                {"id":"R1","type":"readmodel","label":"v","col":1}],
              "edges":[["GHOST","E1"],["E1","R1"]]}"#,
        );
        assert_eq!(rules_for(&lint(&m), "E1"), vec!["event-no-producer"]);
    }

    #[test]
    fn a_policy_missing_a_side_is_flagged_on_that_side() {
        // P_in: triggered (E->P) but triggers nothing -> no-output only.
        // P_out: triggers something (P->E) but nothing triggers it -> no-input only.
        let m = model_of(
            r#"{"elements":[
                {"id":"E1","type":"event","label":"A","col":0},
                {"id":"PIN","type":"policy","label":"in-only","col":1},
                {"id":"POUT","type":"policy","label":"out-only","col":1},
                {"id":"E2","type":"event","label":"B","col":2}],
              "edges":[["E1","PIN"],["POUT","E2"]]}"#,
        );
        let fs = lint(&m);
        assert_eq!(rules_for(&fs, "PIN"), vec!["policy-no-output"]);
        assert_eq!(rules_for(&fs, "POUT"), vec!["policy-no-input"]);
    }

    // An isolated event trips two distinct rules at once — both a missing producer and a dead end.
    #[test]
    fn an_isolated_event_trips_both_event_rules() {
        let m = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"Lonely","col":0}]}"#);
        assert_eq!(
            rules_for(&lint(&m), "E1"),
            vec!["event-no-producer", "event-dead-end"],
            "producer rule before dead-end rule, deterministically"
        );
    }

    // An isolated policy trips both policy rules; the ordering is stable.
    #[test]
    fn an_isolated_policy_trips_both_policy_rules() {
        let m = model_of(r#"{"elements":[{"id":"P1","type":"policy","label":"Orphan","col":0}]}"#);
        assert_eq!(
            rules_for(&lint(&m), "P1"),
            vec!["policy-no-input", "policy-no-output"]
        );
    }

    // These rules are event/policy-only: an unconnected actor, command, aggregate, read-model,
    // external, or hotspot is not a grammar defect (a lone actor or a bare hotspot is normal).
    #[test]
    fn other_lanes_are_never_flagged_by_these_rules() {
        let m = model_of(
            r#"{"elements":[
                {"id":"X1","type":"actor","label":"a","col":0},
                {"id":"C1","type":"command","label":"c","col":0},
                {"id":"A1","type":"aggregate","label":"g","col":0},
                {"id":"R1","type":"readmodel","label":"r","col":0},
                {"id":"Z1","type":"external","label":"z","col":0},
                {"id":"H1","type":"hotspot","label":"?","col":0}]}"#,
        );
        assert!(
            lint(&m).is_empty(),
            "only events and policies carry grammar obligations"
        );
    }

    // Findings come out in element file-order, so the report is stable across runs.
    #[test]
    fn findings_follow_element_file_order() {
        let m = model_of(
            r#"{"elements":[
                {"id":"P1","type":"policy","label":"first","col":0},
                {"id":"E1","type":"event","label":"second","col":1}]}"#,
        );
        let fs = lint(&m);
        let ids: Vec<&str> = fs.iter().map(|f| f.element_id.as_str()).collect();
        assert_eq!(ids.first(), Some(&"P1"), "P1 (first in file) reports first");
        assert!(ids.contains(&"E1"));
    }

    // ---- F-es-lint: the design-level `command-no-output` rule ------------------------------

    // A command that emits nothing is a defect only at `design` granularity. C1 -> E1 gives it
    // an output; C2 dangles. The same board is silent at big-picture, flagged at design.
    #[test]
    fn a_command_with_no_output_is_flagged_only_at_design_level() {
        // big-picture (default): a command sketched before its event is legitimate — silent.
        let big = model_of(
            r#"{"elements":[
                {"id":"C1","type":"command","label":"emits","col":0},
                {"id":"E1","type":"event","label":"E","col":1},
                {"id":"C2","type":"command","label":"dangles","col":0}],
              "edges":[["C1","E1"]]}"#,
        );
        assert!(
            rules_for(&lint(&big), "C2").is_empty(),
            "no command rule at big-picture"
        );

        // design: the dangling command fires, the wired one stays clean.
        let design = model_of(
            r#"{"level":"design","elements":[
                {"id":"C1","type":"command","label":"emits","col":0},
                {"id":"E1","type":"event","label":"E","col":1},
                {"id":"C2","type":"command","label":"dangles","col":0}],
              "edges":[["C1","E1"]]}"#,
        );
        assert_eq!(rules_for(&lint(&design), "C2"), vec!["command-no-output"]);
        assert!(
            rules_for(&lint(&design), "C1").is_empty(),
            "a command with an outbound edge is clean at design too"
        );
    }

    // Determinism holds across lanes: at design level a command finding and an event finding come
    // out in element file-order (C1 before E1 here).
    #[test]
    fn design_findings_follow_element_file_order() {
        let m = model_of(
            r#"{"level":"design","elements":[
                {"id":"C1","type":"command","label":"c","col":0},
                {"id":"E1","type":"event","label":"e","col":1}]}"#,
        );
        let fs = lint(&m);
        let ids: Vec<&str> = fs.iter().map(|f| f.element_id.as_str()).collect();
        // C1's command finding precedes E1's two event findings — element file-order holds across
        // the new design-level lane just as it does for the base rules.
        assert_eq!(ids, vec!["C1", "E1", "E1"]);
    }
}
