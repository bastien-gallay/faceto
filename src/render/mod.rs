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

pub use context::render_context;
pub use html::render_html;
pub use mermaid::{render_mermaid, DEGRADATION_NOTICE};
pub use style::lane_prefix;
pub use svg::{render_svg, View};
