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
use std::collections::{HashMap, HashSet};

use super::mermaid::render_mermaid;
use super::style::LANES;
use super::text::split_label;
use crate::model::Lane;

/// Human-facing plural headings, **index-aligned with `LANES`**: the `[_; LANES.len()]` size means
/// adding a lane to `LANES` forces this array to grow too or the crate fails to compile — no silent
/// heading drift.
const LANE_HEADINGS: [&str; LANES.len()] = [
    "Actors",
    "Commands",
    "Aggregates",
    "Events",
    "Policies",
    "Read models",
    "Systems",
    "Hotspots",
];

/// The plural heading for a lane `type`. `LANES` is the single source of truth for the order;
/// `"Other"` is unreachable in practice (callers only pass on-grammar kinds) but keeps this total.
fn lane_heading(lane: Lane) -> &'static str {
    LANE_HEADINGS[LANES
        .iter()
        .position(|&l| l == lane)
        .expect("LANES is total")]
}

/// Escape the markdown-active characters that would otherwise break inline prose or bullet text.
/// Board labels are authored freely (they can contain `*`, `_`, backticks, brackets, and even
/// newlines), and they land inside `**bold**` spans, `# ` headings and `- ` bullets — so an
/// un-escaped `*`/`[` could corrupt inline markup, and a raw newline would split the heading or
/// bullet across lines. Newlines collapse to a space (as `mermaid_esc` does); the metachars are
/// backslash-escaped, with `\` doubled first so it can never combine with a following escape. This
/// is *not* `text::esc` (HTML entities) nor `mermaid_esc` (Mermaid quoted strings) — a third context.
fn md_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' | '\r' => out.push(' '),
            '\\' | '`' | '*' | '_' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Escape a URL for a CommonMark angle-bracket link destination (`(<...>)`). Such a destination may
/// hold spaces and most punctuation, but not an unescaped `<`/`>` or a line break — so a malformed
/// link can never leak into and corrupt the surrounding markdown. Backslash-escapes are recognised
/// inside `<...>`, so `\`/`<`/`>` are backslash-escaped and line breaks dropped (a valid URL has
/// none of these raw anyway).
fn md_url_dest(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' | '\r' => {}
            '\\' | '<' | '>' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// The shortest backtick fence (≥3) that can wrap `content` without a run of backticks *inside* it
/// prematurely closing the fence. CommonMark requires the opening fence be longer than any internal
/// run, so a label carrying ``` ``` ``` can't break the embedded ```` ```mermaid ```` block.
fn backtick_fence(content: &str) -> String {
    let mut longest = 0usize;
    let mut run = 0usize;
    for c in content.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

/// Render a board to a markdown+Mermaid context pack. Deterministic and pure. Sections, in order:
/// header → ubiquitous language (lanes, in `LANES` order) → flows (edges, model order) → regions
/// (phases) → open questions (unresolved hotspots + lint) → embedded Mermaid diagram.
///
/// Parity with `render_svg`/`render_mermaid`: a flow touching an endpoint the board does not
/// define is skipped. There is no off-grammar element to drop — `Lane` closed that at the read
/// boundary (F-lane-enum), so every element the model carries has a lane.
pub fn render_context(model: &Model) -> String {
    let live: &[Element] = &model.elements;

    // Index elements by id once, so flows resolve endpoints in O(1) rather than scanning.
    let by_id: HashMap<&str, &Element> = live.iter().map(|e| (e.id.as_str(), e)).collect();

    let mut out = String::new();

    header(&mut out, model);
    ubiquitous_language(&mut out, live);
    flows(&mut out, model, &by_id);
    regions(&mut out, model, live);
    open_questions(&mut out, model, live);
    diagram(&mut out, model);

    out
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

fn ubiquitous_language(out: &mut String, live: &[Element]) {
    out.push_str("## Ubiquitous language\n\n");
    let mut any = false;
    for &kind in LANES.iter() {
        // Peek before printing the heading so an empty lane is skipped without allocating a Vec.
        let mut in_lane = live.iter().filter(|e| e.kind == kind).peekable();
        if in_lane.peek().is_none() {
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
                // A proper `[text](<dest>)` link: the angle-bracket destination tolerates spaces and
                // punctuation, and both halves are sanitised so a malformed link can't break the doc.
                out.push_str(&format!(
                    "  - [{}](<{}>)\n",
                    md_esc(link),
                    md_url_dest(link)
                ));
            }
        }
        out.push('\n');
    }
    if !any {
        out.push_str("_(no elements)_\n\n");
    }
}

fn flows(out: &mut String, model: &Model, by_id: &HashMap<&str, &Element>) {
    // Build the body first so the "## Flows" heading is only emitted when at least one edge is live.
    let mut body = String::new();
    for edge in &model.edges {
        let (Some(src), Some(dst)) = (by_id.get(edge.src.as_str()), by_id.get(edge.dst.as_str()))
        else {
            continue; // skip edges whose endpoints aren't both live (parity with mermaid)
        };
        let arrow = match edge.label.as_deref() {
            Some(l) if !l.is_empty() => format!(" →({}) ", md_esc(l)),
            _ => " → ".to_string(),
        };
        body.push_str(&format!(
            "- {}{}{}\n",
            md_esc(&src.label),
            arrow,
            md_esc(&dst.label)
        ));
    }
    if body.is_empty() {
        return;
    }
    out.push_str("## Flows\n\n");
    out.push_str(&body);
    out.push('\n');
}

fn regions(out: &mut String, model: &Model, live: &[Element]) {
    if model.phases.is_empty() {
        return;
    }
    // Assign each placed element to its region in one pass: region_of is O(phases), so doing it
    // once per element here is O(V·P) instead of O(V·P²) if re-derived inside the per-phase loop.
    // Insertion preserves live (model) order within each region, matching the vocabulary ordering.
    let mut members: HashMap<&str, Vec<&Element>> = HashMap::new();
    for e in live {
        if let Some(c) = e.col {
            if let Some(r) = region_of(model, c) {
                members.entry(r.id.as_str()).or_default().push(e);
            }
        }
    }

    out.push_str("## Regions (bounded contexts)\n\n");
    for p in &model.phases {
        out.push_str(&format!(
            "### {} (cols {}–{})\n\n",
            md_esc(&p.label),
            p.from_col,
            p.to_col
        ));
        match members.get(p.id.as_str()) {
            None => out.push_str("_(no elements)_\n\n"),
            Some(members) => {
                for e in members {
                    out.push_str(&format!("- **{}** `{}`\n", md_esc(&e.label), e.id));
                }
                out.push('\n');
            }
        }
    }
}

fn open_questions(out: &mut String, model: &Model, live: &[Element]) {
    // Feeder 1: unresolved hotspots.
    let hotspots: Vec<&Element> = live
        .iter()
        .filter(|e| e.kind == Lane::Hotspot && !e.resolved)
        .collect();

    // Feeder 2: lint findings, suppressing any on a resolved element (mirrors serve's sidebar).
    let resolved: HashSet<&str> = model
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
    let mermaid = render_mermaid(model);
    // Mermaid node text isn't markdown-escaped, so a label carrying backticks could otherwise close
    // the fence early; size the fence to the longest internal run so it always wraps cleanly.
    let fence = backtick_fence(&mermaid);
    out.push_str("## Diagram\n\n");
    out.push_str(&format!("{fence}mermaid\n"));
    out.push_str(&mermaid);
    out.push_str(&format!("{fence}\n"));
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

    #[test]
    fn newline_in_label_or_title_collapses_to_space() {
        // A newline would otherwise split the heading / bullet across lines.
        let m = model_of(
            r#"{ "title": "line1\nline2", "elements": [
                { "id": "C1", "type": "command", "label": "add\nitem", "col": 0 }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(
            out.contains("# Context: line1 line2"),
            "title one line:\n{out}"
        );
        assert!(
            out.contains("- **add item** `C1`"),
            "bullet one line:\n{out}"
        );
    }

    #[test]
    fn backticks_in_label_do_not_break_the_mermaid_fence() {
        // A triple-backtick label must not prematurely close the embedded ```mermaid fence.
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "C1", "type": "command", "label": "see ```code```", "col": 0 }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        // The fence must be longer than the 3-backtick run inside, i.e. at least ````.
        assert!(out.contains("````mermaid\n"), "widened fence:\n{out}");
        assert!(
            out.trim_end().ends_with("````"),
            "matching close fence:\n{out}"
        );
    }

    #[test]
    fn link_with_spaces_becomes_a_valid_angle_bracket_link() {
        let m = model_of(
            r#"{ "title": "t", "elements": [
                { "id": "C1", "type": "command", "label": "add", "col": 0, "links": ["https://x.com/a b"] }
            ], "edges": [] }"#,
        );
        let out = render_context(&m);
        assert!(
            out.contains("- [https://x.com/a b](<https://x.com/a b>)"),
            "sanitised link:\n{out}"
        );
    }
}
