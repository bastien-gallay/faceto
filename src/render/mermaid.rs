//! Render a board model into **Mermaid** `flowchart` text — a portable, lossy export.
//!
//! Mermaid is a strictly poorer target than faceto's own SVG: it has no lanes, no timeline
//! columns, no regions/phases, no diff overlay, and no in-lane y-placement. So this exporter is
//! deliberately *lossy*, and — the whole point of the feature — it states the loss **explicitly**
//! in a `%%` comment header rather than hiding it. What it *does* preserve: each element as a
//! shaped node (one distinct Mermaid shape per lane `type`, so type survives visually), the edges,
//! and the colour grammar (per-type `classDef`, sourced from the same `style::colour` the SVG uses,
//! so the one-type→one-colour rule stays intact).
//!
//! Pure `Model -> String`, deterministic, no I/O — a sibling of `svg`/`html` that needs only the
//! model types plus `style::{LANES, colour, text_dark}`.

use crate::model::Model;

use super::style::{colour, text_dark, LANES};

/// The Mermaid `flowchart` shape delimiters for a lane `type` — one distinct shape per lane so the
/// 8-lane grammar survives visually without polluting the label text. Kept in one place next to the
/// lane list it mirrors.
fn shape(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "actor" => ("([", "])"),     // stadium
        "command" => ("[", "]"),     // rectangle
        "aggregate" => ("[[", "]]"), // subroutine
        "event" => ("(", ")"),       // rounded
        "policy" => ("{{", "}}"),    // hexagon
        "readmodel" => ("[/", "/]"), // parallelogram
        "external" => ("[(", ")]"),  // cylinder
        "hotspot" => ("{", "}"),     // rhombus
        _ => ("[", "]"),             // off-grammar types are filtered out before this; be safe
    }
}

/// Escape a label for a Mermaid quoted node string. Node text is wrapped in double quotes, so the
/// one character that must not appear raw is `"` — replaced with Mermaid's HTML entity `#quot;`.
/// Raw newlines would break the single-line node statement, so they collapse to a space. (This is
/// *not* `text::esc`, which produces HTML entities for an SVG/HTML context.)
fn mermaid_esc(s: &str) -> String {
    s.replace('"', "#quot;")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

/// The `%%` degradation-warning header. Renders invisibly in Mermaid, so the honest notice travels
/// with the exported text itself, not just the terminal. Enumerates exactly what is lost — colour is
/// pointedly *not* in the list, because the `classDef`s below preserve it.
const DEGRADATION_HEADER: &str = "  %% Exported from faceto — Mermaid is a lossy target.\n  %% These do NOT survive the export: lanes, the timeline columns (col),\n  %% regions/phases, the since-you-last-looked diff overlay, and in-lane\n  %% y-placement / resolved styling. (Type colours ARE preserved, via the\n  %% classDef statements at the end.)";

/// The one-line degradation notice the CLI prints to **stderr**, so an interactive user sees it even
/// when stdout is piped straight into a Mermaid tool. Shares the spirit of the header above.
pub const DEGRADATION_NOTICE: &str =
    "note: Mermaid export is lossy — lanes, timeline columns, regions, the diff overlay and \
     y-placement do not survive (type colours are preserved).";

/// Render a board to Mermaid `flowchart LR` text. Deterministic and pure: header → nodes (model
/// order) → edges (model order) → per-type `classDef` + `class` assignment (in `LANES` order, only
/// for types actually present). Left→right mirrors the board's time axis.
///
/// Parity with `render_svg`: elements whose `kind` is off the 8-lane grammar are dropped, and edges
/// touching a dropped or undefined endpoint are skipped — so the two renderers always draw the same
/// board.
pub fn render_mermaid(model: &Model) -> String {
    let mut out = String::from("flowchart LR\n");
    out.push_str(DEGRADATION_HEADER);
    out.push('\n');

    // Surviving elements = on-grammar types only (mirrors svg.rs's `retain`).
    let live: Vec<&crate::model::Element> = model
        .elements
        .iter()
        .filter(|e| LANES.contains(&e.kind.as_str()))
        .collect();

    // Nodes, in model order.
    for e in &live {
        let (open, close) = shape(&e.kind);
        out.push_str(&format!(
            "  {}{}\"{}\"{}\n",
            e.id,
            open,
            mermaid_esc(&e.label),
            close
        ));
    }

    // Edges, in model order — but only those whose endpoints are both live (parity with the SVG,
    // which resolves each endpoint against the placed elements and skips unresolved ones).
    let is_live = |id: &str| live.iter().any(|e| e.id == id);
    let mut wrote_edge = false;
    for edge in &model.edges {
        if is_live(&edge.src) && is_live(&edge.dst) {
            if !wrote_edge {
                out.push('\n');
                wrote_edge = true;
            }
            out.push_str(&format!("  {} --> {}\n", edge.src, edge.dst));
        }
    }

    // Colour grammar via classDef, one per type present, in LANES order. `class` groups all ids of
    // that type onto one assignment line. This is what keeps colour from degrading.
    let mut wrote_class = false;
    for &kind in LANES.iter() {
        let ids: Vec<&str> = live
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.id.as_str())
            .collect();
        if ids.is_empty() {
            continue;
        }
        if !wrote_class {
            out.push('\n');
            wrote_class = true;
        }
        let text = if text_dark(kind) { "#000" } else { "#fff" };
        out.push_str(&format!(
            "  classDef {} fill:{},color:{}\n",
            kind,
            colour(kind),
            text
        ));
        out.push_str(&format!("  class {} {}\n", ids.join(","), kind));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;
    use crate::model::from_json;

    fn model_of(src: &str) -> Model {
        from_json(&json::parse(src).unwrap())
    }

    #[test]
    fn golden_small_board() {
        let m = model_of(
            r#"{
              "title": "t",
              "elements": [
                { "id": "X1", "type": "actor", "label": "Operator", "col": 0 },
                { "id": "C1", "type": "command", "label": "start the day", "col": 0 },
                { "id": "E1", "type": "event", "label": "DayStarted", "col": 1 }
              ],
              "edges": [ ["X1", "C1"], ["C1", "E1"] ]
            }"#,
        );
        let expected = "\
flowchart LR
  %% Exported from faceto — Mermaid is a lossy target.
  %% These do NOT survive the export: lanes, the timeline columns (col),
  %% regions/phases, the since-you-last-looked diff overlay, and in-lane
  %% y-placement / resolved styling. (Type colours ARE preserved, via the
  %% classDef statements at the end.)
  X1([\"Operator\"])
  C1[\"start the day\"]
  E1(\"DayStarted\")

  X1 --> C1
  C1 --> E1

  classDef actor fill:#FCEFA1,color:#000
  class X1 actor
  classDef command fill:#1A6FAE,color:#fff
  class C1 command
  classDef event fill:#FF9F1C,color:#000
  class E1 event
";
        assert_eq!(render_mermaid(&m), expected);
    }

    #[test]
    fn every_type_emits_its_shape() {
        let m = model_of(
            r#"{
              "elements": [
                { "id": "X1", "type": "actor", "label": "a" },
                { "id": "C1", "type": "command", "label": "c" },
                { "id": "A1", "type": "aggregate", "label": "g" },
                { "id": "E1", "type": "event", "label": "e" },
                { "id": "P1", "type": "policy", "label": "p" },
                { "id": "R1", "type": "readmodel", "label": "r" },
                { "id": "G1", "type": "external", "label": "x" },
                { "id": "H1", "type": "hotspot", "label": "h" }
              ]
            }"#,
        );
        let out = render_mermaid(&m);
        assert!(out.contains("X1([\"a\"])"), "actor stadium");
        assert!(out.contains("C1[\"c\"]"), "command rectangle");
        assert!(out.contains("A1[[\"g\"]]"), "aggregate subroutine");
        assert!(out.contains("E1(\"e\")"), "event rounded");
        assert!(out.contains("P1{{\"p\"}}"), "policy hexagon");
        assert!(out.contains("R1[/\"r\"/]"), "readmodel parallelogram");
        assert!(out.contains("G1[(\"x\")]"), "external cylinder");
        assert!(out.contains("H1{\"h\"}"), "hotspot rhombus");
    }

    #[test]
    fn colour_classdef_only_for_present_types() {
        let m = model_of(r#"{ "elements": [ { "id": "E1", "type": "event", "label": "e" } ] }"#);
        let out = render_mermaid(&m);
        // The one present type carries its exact grammar colour + a class assignment…
        assert!(out.contains("classDef event fill:#FF9F1C,color:#000"));
        assert!(out.contains("class E1 event"));
        // …and absent types emit nothing.
        assert!(!out.contains("classDef command"));
        assert!(!out.contains("classDef policy"));
    }

    #[test]
    fn labels_are_escaped_and_never_break_nodes() {
        let m = model_of(
            r#"{ "elements": [ { "id": "E1", "type": "event", "label": "say \"hi\" <now>" } ] }"#,
        );
        let out = render_mermaid(&m);
        // Quotes become the Mermaid entity; the raw label's inner quotes never leak.
        assert!(out.contains("E1(\"say #quot;hi#quot; <now>\")"), "{out}");
    }

    #[test]
    fn off_grammar_elements_and_their_edges_are_filtered() {
        let m = model_of(
            r#"{
              "elements": [
                { "id": "E1", "type": "event", "label": "e" },
                { "id": "Z1", "type": "widget", "label": "z" }
              ],
              "edges": [ ["E1", "Z1"], ["Z1", "E1"] ]
            }"#,
        );
        let out = render_mermaid(&m);
        assert!(out.contains("E1(\"e\")"));
        assert!(!out.contains("Z1"), "off-grammar node dropped");
        assert!(!out.contains("-->"), "edges touching it are skipped");
    }

    #[test]
    fn header_states_the_degradation_but_not_colour() {
        let out = render_mermaid(&model_of(r#"{ "elements": [] }"#));
        assert!(out.starts_with("flowchart LR\n"));
        assert!(out.contains("%% Exported from faceto"));
        assert!(out.contains("lanes"));
        assert!(out.contains("regions/phases"));
        assert!(out.contains("diff overlay"));
        // Colour is preserved, so it is pointedly framed as such, not listed as a loss.
        assert!(out.contains("Type colours ARE preserved"));
    }

    #[test]
    fn empty_board_is_valid_and_does_not_panic() {
        let out = render_mermaid(&model_of(r#"{ "elements": [] }"#));
        assert!(out.starts_with("flowchart LR\n"));
        // No nodes, no edges, no classDef statements (the header's mention of "classDef" is prose,
        // so match the actual statement form).
        assert!(!out.contains("-->"));
        assert!(!out.contains("classDef actor"));
        assert!(!out.contains("  classDef "));
    }
}
