// A spike deliberately builds the *whole* seam (mint prefixes, `compact`, the genesis batch) even
// where the CLI fork does not yet call it — the unused surface is part of the finding, not dead
// weight to be trimmed. A merged format would wire it up; this one is throwaway.
#![allow(dead_code)]

//! **THROWAWAY SPIKE — not for merge.** Issue #114 (`F-spike-canvas`).
//!
//! A second board format — the DDD-crew [Bounded Context Canvas][bcc] — implemented *beside* the
//! event-storming format rather than behind an abstraction, so the question the spike exists to
//! answer stays honest: **which event-storming assumptions are welded into the kernel?**
//!
//! [bcc]: https://github.com/ddd-crew/bounded-context-canvas
//!
//! The BCC is a **slot template**: a fixed list of named sections, each holding a short list of
//! items. It has no `col`, no lane, no `y`, no phase, no timeline — every event-storming
//! coordinate concept is dead weight. Layout is a fixed grid, so the spike measures *the seam*,
//! not the drawing.
//!
//! Deliberately **not** built (per the issue's timebox): `serve` routes, the client, lint. The
//! findings those would have produced are argued on paper in `docs/notes/f-spike-canvas.md`.
//!
//! Structure mirrors the shape the real thing would take, so the diff between "what I could
//! reuse" and "what I had to copy" is readable:
//!
//! | file | what it is | reused from the kernel? |
//! | --- | --- | --- |
//! | [`model`] | the slot-template board + parse | `json` only |
//! | [`events`] | `CanvasEvent` + `replay` + `from_canvas` | log *framing* copied, not called |
//! | [`diff`] | id-keyed diff with canvas verdicts | pattern copied, code not shared |
//! | [`render`] | fixed-grid → SVG | nothing — `render::*` is `pub(crate)` to `render` |

pub mod diff;
pub mod events;
pub mod model;
pub mod render;

pub use events::load_log;
pub use model::{load, Canvas};
pub use render::render_svg;
