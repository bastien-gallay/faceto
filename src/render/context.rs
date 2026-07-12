//! Render a board model into a **context pack** — a structured markdown+Mermaid document a coding
//! agent reads (typically referenced from the repo's `AGENTS.md`) so the domain built in the
//! workshop doesn't have to be re-explained.
//!
//! Unlike the Mermaid export (a strictly *lossy* diagram), the context pack is deliberately the
//! *rich* target: the prose preserves what Mermaid drops — lanes (as the "ubiquitous language"
//! grouping), the timeline columns and regions/phases (the "bounded contexts" section), open
//! hotspots and lint findings (the "open questions" section). It then *embeds* the Mermaid diagram
//! at the end for a visual, honestly carrying that diagram's own `%%` degradation header inside the
//! fence.
//!
//! Pure `Model -> String`, deterministic, no I/O — a sibling of `svg`/`html`/`mermaid`. It reuses
//! the same on-grammar filter and iteration orders as `render_mermaid`, so the pack always
//! describes the same board the SVG and Mermaid draw (the parity invariant).

use crate::lint::lint;
use crate::model::{region_of, Element, Model};

use super::mermaid::render_mermaid;
use super::style::LANES;
use super::text::split_label;

/// Human-facing plural heading for each lane `type`, index-aligned with `LANES`. The context pack
/// speaks the ubiquitous language in words, not the wire `type` string.
fn lane_heading(kind: &str) -> &'static str {
    match kind {
        "actor" => "Actors",
        "command" => "Commands",
        "aggregate" => "Aggregates",
        "event" => "Events",
        "policy" => "Policies",
        "readmodel" => "Read models",
        "external" => "External systems",
        "hotspot" => "Hotspots",
        _ => "Other", // off-grammar types are filtered out before this; be safe
    }
}

/// Escape the markdown-active characters that would otherwise break inline prose or bullet text.
/// Board labels are authored freely (they can contain `*`, `_`, backticks, brackets), and they land
/// inside `**bold**` spans and `- ` bullets, so an un-escaped `*` or `[` could corrupt the rendered
/// markdown. Backslash is doubled first so it can never combine with a following escape. This is
/// *not* `text::esc` (HTML entities) nor `mermaid_esc` (Mermaid quoted strings) — a third context.
fn md_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Render a board to a markdown+Mermaid context pack. Deterministic and pure. Sections, in order:
/// header → ubiquitous language (lanes, in `LANES` order) → flows (edges, model order) → regions
/// (phases) → open questions (unresolved hotspots + lint) → embedded Mermaid diagram.
///
/// Parity with `render_svg`/`render_mermaid`: elements whose `kind` is off the 8-lane grammar are
/// dropped, and flows touching a dropped or undefined endpoint are skipped.
pub fn render_context(model: &Model) -> String {
    // Surviving elements = on-grammar types only (mirrors svg/mermaid's filter).
    let live: Vec<&Element> = model
        .elements
        .iter()
        .filter(|e| LANES.contains(&e.kind.as_str()))
        .collect();

    let mut out = String::new();

    header(&mut out, model);
    ubiquitous_language(&mut out, &live);
    flows(&mut out, model, &live);
    regions(&mut out, model, &live);
    open_questions(&mut out, model, &live);
    diagram(&mut out, model);

    out
}

/// The label of a live element by id (for referring to elements by name in prose).
fn label_of<'a>(live: &[&'a Element], id: &str) -> Option<&'a str> {
    live.iter().find(|e| e.id == id).map(|e| e.label.as_str())
}

fn header(out: &mut String, model: &Model) {
    let title = if model.title.is_empty() {
        "untitled board"
    } else {
        &model.title
    };
    out.push_str(&format!("# Context: {}\n\n", md_esc(title)));
    let level = match model.level {
        crate::model::Level::Design => "Design level",
        crate::model::Level::BigPicture => "Big-picture level",
    };
    out.push_str(&format!("_Event-storming model. {level}._\n\n"));
    // Kept ≤100 cols so the generated file lints clean on the fixed lines a user can't control.
    out.push_str(
        "> Domain model from a faceto event-storming workshop (the log is the source of truth).\n\
         > Reference this file from your `AGENTS.md` so an agent knows the domain without re-explaining it.\n\n",
    );
}

fn ubiquitous_language(out: &mut String, live: &[&Element]) {
    out.push_str("## Ubiquitous language\n\n");
    let mut any = false;
    for &kind in LANES.iter() {
        let in_lane: Vec<&&Element> = live.iter().filter(|e| e.kind == kind).collect();
        if in_lane.is_empty() {
            continue;
        }
        any = true;
        out.push_str(&format!("### {}\n\n", lane_heading(kind)));
        for e in in_lane {
            let (hero, detail) = split_label(&e.label, e.detail.as_deref());
            out.push_str(&format!("- **{}** `{}`", md_esc(&hero), e.id));
            if !detail.is_empty() {
                out.push_str(&format!(" — {}", md_esc(&detail)));
            }
            out.push('\n');
            for link in &e.links {
                out.push_str(&format!("  - <{link}>\n"));
            }
        }
        out.push('\n');
    }
    if !any {
        out.push_str("_(no elements)_\n\n");
    }
}

fn flows(out: &mut String, model: &Model, live: &[&Element]) {
    let mut lines = Vec::new();
    for edge in &model.edges {
        let (Some(src), Some(dst)) = (label_of(live, &edge.src), label_of(live, &edge.dst)) else {
            continue; // skip edges whose endpoints aren't both live (parity with mermaid)
        };
        let arrow = match edge.label.as_deref() {
            Some(l) if !l.is_empty() => format!(" →({}) ", md_esc(l)),
            _ => " → ".to_string(),
        };
        lines.push(format!("- {}{}{}\n", md_esc(src), arrow, md_esc(dst)));
    }
    if lines.is_empty() {
        return;
    }
    out.push_str("## Flows\n\n");
    for l in lines {
        out.push_str(&l);
    }
    out.push('\n');
}

fn regions(out: &mut String, model: &Model, live: &[&Element]) {
    if model.phases.is_empty() {
        return;
    }
    out.push_str("## Regions (bounded contexts)\n\n");
    for p in &model.phases {
        out.push_str(&format!(
            "### {} (cols {}–{})\n\n",
            md_esc(&p.label),
            p.from_col,
            p.to_col
        ));
        let members: Vec<&&Element> = live
            .iter()
            .filter(|e| {
                e.col.is_some_and(|c| {
                    region_of(model, c).map(|r| r.id.as_str()) == Some(p.id.as_str())
                })
            })
            .collect();
        if members.is_empty() {
            out.push_str("_(no elements)_\n\n");
            continue;
        }
        for e in members {
            out.push_str(&format!("- **{}** `{}`\n", md_esc(&e.label), e.id));
        }
        out.push('\n');
    }
}

fn open_questions(out: &mut String, model: &Model, live: &[&Element]) {
    // Feeder 1: unresolved hotspots.
    let hotspots: Vec<&&Element> = live
        .iter()
        .filter(|e| e.kind == "hotspot" && !e.resolved)
        .collect();

    // Feeder 2: lint findings, suppressing any on a resolved element (mirrors serve's sidebar).
    let resolved: std::collections::HashSet<&str> = model
        .elements
        .iter()
        .filter(|e| e.resolved)
        .map(|e| e.id.as_str())
        .collect();
    let findings: Vec<_> = lint(model)
        .into_iter()
        .filter(|f| !resolved.contains(f.element_id.as_str()))
        .collect();

    if hotspots.is_empty() && findings.is_empty() {
        return;
    }

    out.push_str("## Open questions\n\n");
    for e in hotspots {
        out.push_str(&format!(
            "- ⬦ **{}** `{}` — open hotspot\n",
            md_esc(&e.label),
            e.id
        ));
    }
    for f in findings {
        let label = model
            .elements
            .iter()
            .find(|e| e.id == f.element_id)
            .map(|e| e.label.as_str())
            .unwrap_or(f.element_id.as_str());
        out.push_str(&format!(
            "- ⚠ **{}** `{}` — {}\n",
            md_esc(label),
            f.element_id,
            md_esc(f.message)
        ));
    }
    out.push('\n');
}

fn diagram(out: &mut String, model: &Model) {
    out.push_str("## Diagram\n\n");
    out.push_str("```mermaid\n");
    out.push_str(&render_mermaid(model));
    out.push_str("```\n");
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
            r#"{ "title": "t", "elements": [
                { "id": "X1", "type": "actor", "label": "Operator", "col": 0 },
                { "id": "C1", "type": "command", "label": "start", "col": 0 },
                { "id": "E1", "type": "event", "label": "Started", "col": 1 }
            ], "edges": [ ["X1","C1"], ["C1","E1"] ] }"#,
        );
        let expected = "\
# Context: t

_Event-storming model. Big-picture level._

> Domain model from a faceto event-storming workshop (the log is the source of truth).
> Reference this file from your `AGENTS.md` so an agent knows the domain without re-explaining it.

## Ubiquitous language

### Actors

- **Operator** `X1`

### Commands

- **start** `C1`

### Events

- **Started** `E1`

## Flows

- Operator → start
- start → Started

## Open questions

- ⚠ **Started** `E1` — no outbound edge: a dead end unless this event is terminal

## Diagram

```mermaid
flowchart LR
  %% Exported from faceto — Mermaid is a lossy target.
  %% These do NOT survive the export: lanes, the timeline columns (col),
  %% regions/phases, the since-you-last-looked diff overlay, and in-lane
  %% y-placement / resolved styling. (Type colours ARE preserved, via the
  %% classDef statements at the end.)
  X1([\"Operator\"])
  C1[\"start\"]
  E1(\"Started\")

  X1 --> C1
  C1 --> E1

  classDef actor fill:#FCEFA1,color:#000
  class X1 actor
  classDef command fill:#1A6FAE,color:#fff
  class C1 command
  classDef event fill:#FF9F1C,color:#000
  class E1 event
```
";
        assert_eq!(render_context(&m), expected);
    }

    #[test]
    fn lanes_grouped_and_named_in_grammar_order() {
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "E1", "type": "event", "label": "Ev", "col": 1 },
                { "id": "X1", "type": "actor", "label": "Act", "col": 0 }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        let actors = out.find("### Actors").unwrap();
        let events = out.find("### Events").unwrap();
        assert!(
            actors < events,
            "Actors before Events (LANES order):\n{out}"
        );
        assert!(!out.contains("### Commands"), "absent lane omitted:\n{out}");
    }

    #[test]
    fn flow_renders_labelled_edge() {
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "C1", "type": "command", "label": "add", "col": 0 },
                { "id": "E1", "type": "event", "label": "Added", "col": 1 }
            ], "edges": [ { "src": "C1", "dst": "E1", "label": "emits" } ] }"#,
        );
        let out = render_context(&m);
        assert!(
            out.contains("- add →(emits) Added"),
            "labelled flow:\n{out}"
        );
    }

    #[test]
    fn region_lists_its_members_by_column() {
        let m = model_of(
            r#"{ "title": "t",
                "phases": [ { "id": "K1", "label": "begin", "fromCol": 0, "toCol": 0 },
                            { "id": "K2", "label": "work", "fromCol": 1, "toCol": 2 } ],
                "elements": [
                    { "id": "C1", "type": "command", "label": "start", "col": 0 },
                    { "id": "E1", "type": "event", "label": "Working", "col": 2 }
                ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(
            out.contains("### begin (cols 0–0)"),
            "region heading:\n{out}"
        );
        assert!(
            out.contains("### work (cols 1–2)"),
            "region heading:\n{out}"
        );
        // `start` (col 0) is in begin; `Working` (col 2) is in work. Search within the Regions
        // section only — both labels also appear once in the vocabulary section above it.
        let sec = &out[out.find("## Regions").unwrap()..];
        let begin = sec.find("### begin").unwrap();
        let work = sec.find("### work").unwrap();
        let start = sec.find("**start**").unwrap();
        let working = sec.find("**Working**").unwrap();
        assert!(begin < start && start < work, "start under begin:\n{out}");
        assert!(work < working, "Working under work:\n{out}");
    }

    #[test]
    fn unresolved_hotspot_shows_resolved_one_hidden() {
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "H1", "type": "hotspot", "label": "open Q", "col": 0 },
                { "id": "H2", "type": "hotspot", "label": "settled", "col": 1, "resolved": true, "detail": "done" }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(
            out.contains("**open Q** `H1` — open hotspot"),
            "unresolved shown:\n{out}"
        );
        // The resolved hotspot still appears in the vocabulary, but never as an open question.
        assert!(
            !out.contains("**settled** `H2` — open hotspot"),
            "resolved hidden:\n{out}"
        );
    }

    #[test]
    fn lint_finding_surfaces_and_resolved_element_is_suppressed() {
        // An event with no producer is a lint finding; a resolved one is suppressed.
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "E1", "type": "event", "label": "Orphan", "col": 0 }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(out.contains("Orphan"), "lint finding surfaced:\n{out}");

        let resolved = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "E1", "type": "event", "label": "Orphan", "col": 0, "resolved": true }
            ], "edges": [] }"#,
        );
        let out2 = render_context(&resolved);
        assert!(
            !out2.contains("## Open questions"),
            "resolved suppresses section:\n{out2}"
        );
    }

    #[test]
    fn embeds_mermaid_fence() {
        let m = model_of(r#"{ "title": "t", "elements": [], "edges": [] }"#);
        let out = render_context(&m);
        assert!(
            out.contains("```mermaid\nflowchart LR"),
            "mermaid fence:\n{out}"
        );
        assert!(out.trim_end().ends_with("```"), "fence closes:\n{out}");
    }

    #[test]
    fn off_grammar_elements_are_filtered() {
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "Z1", "type": "sticky", "label": "junk", "col": 0 }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(!out.contains("junk"), "off-grammar element dropped:\n{out}");
    }

    #[test]
    fn empty_board_does_not_panic() {
        let m = model_of(r#"{ "title": "", "elements": [], "edges": [] }"#);
        let out = render_context(&m);
        assert!(
            out.contains("# Context: untitled board"),
            "empty title fallback:\n{out}"
        );
        assert!(
            out.contains("_(no elements)_"),
            "empty vocabulary marker:\n{out}"
        );
    }

    #[test]
    fn markdown_active_chars_in_label_are_escaped() {
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "C1", "type": "command", "label": "a*b_c[d]", "col": 0 }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(out.contains(r"a\*b\_c\[d\]"), "label escaped:\n{out}");
    }
}
