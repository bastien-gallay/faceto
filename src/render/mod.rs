//! Render a board model into a static SVG and an interactive HTML page.
//!
//! Deterministic, pure std. The colour grammar (one type → one colour → one lane) and the
//! whole visual language are ported faithfully from the original Python harness.

mod context;
mod geometry;
mod html;
mod mermaid;
mod style;
mod svg;
mod text;

/// SPIKE (#114, `spike_canvas`) — the **only** line the second format needed to change inside an
/// existing module. `esc` (XML escaping) and `wrap` (greedy label wrapping with CamelCase
/// hump-splitting) are pure text utilities with nothing event-storming about them, but they were
/// reachable only from inside `render`. Widening them was a one-liner; the finding is that they
/// belong in the kernel, not that the widening was hard.
pub(crate) use text::{esc, wrap};

pub use context::render_context;
pub use html::render_html;
pub use mermaid::{render_mermaid, DEGRADATION_NOTICE};
pub use style::lane_prefix;
pub use svg::{render_svg, View};
