//! Wrap the SVG in the interactive HTML page (`render_html` + template fill).

use super::style::*;
use super::text::esc;

pub fn render_html(svg: &str, title: &str) -> String {
    // The client reuses these geometry constants to re-place a moved sticky and redraw its edges
    // in the browser — keep render.rs the single source of truth for them. `regionTabH`/
    // `regionTabCharW` do the same for the region-add editor's box (Composable — the client must
    // not invent its own tab size).
    // `rowPitch`/`laneVpad` let the drag preview place the lane-growth guide exactly one row
    // below the lane's current bottom rule — the same numbers this renderer will use on commit.
    let cfg = format!(
        "{{\"colW\":{},\"stickyW\":{},\"stickyH\":{},\"rowPitch\":{},\"laneVpad\":{},\"regionTabH\":{},\"regionTabCharW\":{},\"regionTabPad\":{}}}",
        COL_W, STICKY_W, STICKY_H, ROW_PITCH, LANE_VPAD, REGION_TAB_H, REGION_TAB_CHAR_W, REGION_TAB_PAD
    );
    // Fill the placeholders in a single left-to-right pass, so a *value* that happens to contain
    // another placeholder token (a sticky or region labelled `__CONFIG__`, a title of `__SVG__`)
    // is inserted verbatim and never re-scanned. A naive chain of `.replace` inserts the SVG first
    // and then lets the later `__CONFIG__` pass rewrite that label's text into the config JSON.
    fill_template(
        HTML_TEMPLATE,
        &[
            ("__TITLE__", &esc(title)),
            ("__SVG__", svg),
            ("__CONFIG__", &cfg),
        ],
    )
}

/// Substitute `subs` (token → value) into `template` in one pass: at each step the earliest
/// remaining token is replaced with its value, and inserted values are never re-scanned. This is
/// the difference from chained `.replace` — a value carrying a later token can't be clobbered.
pub(crate) fn fill_template(template: &str, subs: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    loop {
        // The earliest occurrence of any token in the unconsumed tail.
        let next = subs
            .iter()
            .filter_map(|(tok, val)| rest.find(tok).map(|pos| (pos, *tok, *val)))
            .min_by_key(|(pos, _, _)| *pos);
        match next {
            Some((pos, tok, val)) => {
                out.push_str(&rest[..pos]);
                out.push_str(val);
                rest = &rest[pos + tok.len()..];
            }
            None => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

pub(crate) const HTML_TEMPLATE: &str = include_str!("../template.html");
