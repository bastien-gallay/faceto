//! Render a board model into a static SVG and an interactive HTML page.
//!
//! Deterministic, pure std. The colour grammar (one type → one colour → one lane) and the
//! whole visual language are ported faithfully from the original Python harness.

mod geometry;
mod html;
mod style;
mod svg;
#[cfg(test)]
mod tests;
mod text;

pub use html::render_html;
pub use style::lane_prefix;
pub use svg::{render_svg, View};
