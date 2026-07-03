//! Render a board model into a static SVG and an interactive HTML page.
//!
//! Deterministic, pure std. The colour grammar (one type → one colour → one lane) and the
//! whole visual language are ported faithfully from the original Python harness.

mod html;
mod render_core;
mod style;
#[cfg(test)]
mod tests;
mod text;

pub use html::render_html;
pub use render_core::{render_svg, View};
pub use style::lane_prefix;
