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
    // Two-stage fill. `__CONFIG__` lives *inside* the client script, so it must be resolved before
    // the script is inserted into the shell: `fill_template` never re-scans an inserted value (so a
    // sticky/region labelled `__CONFIG__` can't be clobbered), which means a `__CONFIG__` left in the
    // `__SCRIPT__` value would survive un-substituted. Stage 1 folds the config into the concatenated
    // modules; stage 2 drops the resolved script (plus style/svg/title) into the shell in one pass.
    let script = fill_template(CLIENT_JS, &[("__CONFIG__", &cfg)]);
    fill_template(
        HTML_TEMPLATE,
        &[
            ("__TITLE__", &esc(title)),
            ("__SVG__", svg),
            ("__STYLE__", STYLE_CSS),
            ("__SCRIPT__", &script),
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

/// The board's CSS, extracted from the inline `<style>` block into its own file (F-js-modules).
pub(crate) const STYLE_CSS: &str = include_str!("../client/style.css");

/// The board's client, split into cohesive modules and concatenated back into one classic script at
/// build time (F-js-modules). No bundler ships — `concat!` glues the `include_str!`'d modules in
/// source order, so the result is one shared top-level scope, byte-identical to the former inline
/// `<script>`. Order is load-bearing: top-level `const`/`let` (e.g. `const CFG = __CONFIG__`) are
/// TDZ-bound and the boot `load()` in `main.js` must run last — keep this list in file order.
pub(crate) const CLIENT_JS: &str = concat!(
    include_str!("../client/core.js"),
    include_str!("../client/layout.js"),
    include_str!("../client/drag.js"),
    include_str!("../client/edit.js"),
    include_str!("../client/region.js"),
    include_str!("../client/sync.js"),
    include_str!("../client/graph.js"),
    include_str!("../client/main.js"),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_html_injects_the_geometry_config() {
        let html = render_html("<svg></svg>", "t");
        assert!(!html.contains("__CONFIG__"));
        assert!(html.contains("\"colW\":210"));
        assert!(html.contains("\"stickyW\":176"));
    }

    #[test]
    fn a_label_equal_to_a_template_token_is_not_clobbered() {
        // A sticky labelled `__CONFIG__` reaches the SVG verbatim (esc leaves underscores). The
        // single-pass fill must insert it as-is, not rewrite it into the geometry JSON.
        let html = render_html("<text>__CONFIG__</text>", "t");
        assert!(html.contains("<text>__CONFIG__</text>")); // label survived
        assert!(html.contains("\"colW\":")); // real config JSON still landed
    }
}
