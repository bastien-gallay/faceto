//! The Scene IR — geometric primitives and the one SVG serializer (F-scene-ir, #116).
//!
//! Every format produces a [`Scene`]; **one** serializer turns it into SVG. That inversion is the
//! point: before it, each format's renderer wrote SVG strings inline, so every downstream layer
//! (diff overlay, tests, the client) had to re-implement per format.
//!
//! The IR is **geometric, never semantic**. `Rect` / `Line` / `Text` / `Circle` / `Path` / `Group`
//! are all it knows; a *sticky*, a *region*, a *lane* are event-storming words and stay inside the
//! format's scene builder. That line is what lets a second format reuse the serializer without
//! inheriting the first format's vocabulary.
//!
//! It is deliberately **not a 1:1 mirror of SVG**: the value is the nesting, the identity tags, and
//! the positions the kernel can reason over — composing two scenes into a diff overlay, hit-testing
//! a click, previewing a drag. Presentation that the kernel never reasons about (`fill`, `stroke`,
//! `font-size`) rides along in [`Attrs`] rather than earning a typed field.
//!
//! Zero runtime dependencies: the serializer is hand-written, like `json.rs`.

use std::fmt::Write as _;

/// An attribute value. Numbers stay numbers so the kernel can read geometry back out of a scene
/// (`Val::Num` on a `data-cx`, say) instead of re-parsing strings.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Num(f64),
    Int(i64),
    Str(String),
}

impl From<f64> for Val {
    fn from(v: f64) -> Self {
        Val::Num(v)
    }
}
impl From<i64> for Val {
    fn from(v: i64) -> Self {
        Val::Int(v)
    }
}
impl From<usize> for Val {
    fn from(v: usize) -> Self {
        Val::Int(v as i64)
    }
}
impl From<bool> for Val {
    fn from(v: bool) -> Self {
        Val::Str(v.to_string())
    }
}
impl From<&str> for Val {
    fn from(v: &str) -> Self {
        Val::Str(v.to_string())
    }
}
impl From<String> for Val {
    fn from(v: String) -> Self {
        Val::Str(v)
    }
}

/// Presentation and identity attributes, in insertion order.
///
/// Ordered rather than a map because the serialized output must be deterministic — the same scene
/// renders byte-for-byte the same SVG on every run, which is what makes rendered boards diffable in
/// git. Lookup is linear; an attribute list is never long enough for that to matter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Attrs(Vec<(String, Val)>);

impl Attrs {
    pub fn new() -> Self {
        Attrs(Vec::new())
    }

    /// Set an attribute, replacing any previous value under the same name.
    pub fn set(&mut self, name: &str, v: impl Into<Val>) -> &mut Self {
        let v = v.into();
        match self.0.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = v,
            None => self.0.push((name.to_string(), v)),
        }
        self
    }

    /// Read one attribute back. The IR's read side has exactly one consumer today — the render
    /// tests, which assert on the *number* the layout computed rather than on its serialized form.
    /// It stays `cfg(test)` until production code reads a scene back (the diff-overlay compose and
    /// the client's hit-test are the two that will).
    #[cfg(test)]
    pub fn get(&self, name: &str) -> Option<&Val> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = &(String, Val)> {
        self.0.iter()
    }
}

/// One drawable node. `Group` nests — a boundary box contains its children and paints behind them —
/// which is the structure a flat list of strings could never express.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        attrs: Attrs,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        attrs: Attrs,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        attrs: Attrs,
    },
    Text {
        x: f64,
        y: f64,
        /// Raw content — the serializer escapes it. Callers pass user data unescaped.
        content: String,
        /// An accessible name / hover tooltip, emitted as a nested `<title>`.
        title: Option<String>,
        attrs: Attrs,
    },
    Path {
        /// An SVG path command string. The one place a raw SVG fragment survives in the IR: a path
        /// is a curve, and a typed segment list would buy the kernel nothing it reasons about.
        d: String,
        attrs: Attrs,
    },
    Group {
        title: Option<String>,
        children: Vec<Shape>,
        attrs: Attrs,
    },
}

impl Shape {
    pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Self {
        Shape::Rect {
            x,
            y,
            w,
            h,
            attrs: Attrs::new(),
        }
    }

    pub fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            attrs: Attrs::new(),
        }
    }

    pub fn circle(cx: f64, cy: f64, r: f64) -> Self {
        Shape::Circle {
            cx,
            cy,
            r,
            attrs: Attrs::new(),
        }
    }

    pub fn text(x: f64, y: f64, content: impl Into<String>) -> Self {
        Shape::Text {
            x,
            y,
            content: content.into(),
            title: None,
            attrs: Attrs::new(),
        }
    }

    pub fn path(d: impl Into<String>) -> Self {
        Shape::Path {
            d: d.into(),
            attrs: Attrs::new(),
        }
    }

    pub fn group(children: Vec<Shape>) -> Self {
        Shape::Group {
            title: None,
            children,
            attrs: Attrs::new(),
        }
    }

    /// Set an attribute (fluent — shapes are built inline in the format's scene builder).
    pub fn with(mut self, name: &str, v: impl Into<Val>) -> Self {
        self.attrs_mut().set(name, v);
        self
    }

    /// Set an attribute only when `v` is `Some` — the shape of every optional `data-*` tag.
    pub fn maybe(self, name: &str, v: Option<impl Into<Val>>) -> Self {
        match v {
            Some(v) => self.with(name, v),
            None => self,
        }
    }

    /// Attach an accessible name / hover tooltip. Valid on `Text` and `Group`; a no-op elsewhere,
    /// since no other primitive has a place to nest one.
    pub fn titled(mut self, t: impl Into<String>) -> Self {
        match &mut self {
            Shape::Text { title, .. } | Shape::Group { title, .. } => *title = Some(t.into()),
            _ => {}
        }
        self
    }

    /// See [`Attrs::get`] — the read side, exercised by the render tests until production code
    /// reads a scene back.
    #[cfg(test)]
    pub fn attrs(&self) -> &Attrs {
        match self {
            Shape::Rect { attrs, .. }
            | Shape::Line { attrs, .. }
            | Shape::Circle { attrs, .. }
            | Shape::Text { attrs, .. }
            | Shape::Path { attrs, .. }
            | Shape::Group { attrs, .. } => attrs,
        }
    }

    pub fn attrs_mut(&mut self) -> &mut Attrs {
        match self {
            Shape::Rect { attrs, .. }
            | Shape::Line { attrs, .. }
            | Shape::Circle { attrs, .. }
            | Shape::Text { attrs, .. }
            | Shape::Path { attrs, .. }
            | Shape::Group { attrs, .. } => attrs,
        }
    }
}

/// A board rendered to geometry: the canvas size plus the shapes on it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub shapes: Vec<Shape>,
}

/// XML-escape the five special characters. Applied by the serializer to every attribute value and
/// every text node, so scene builders pass user data raw.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Format a number the one canonical way: snap to a 1e-4 grid, then print the shortest form that
/// round-trips (so `240.0` is `240` and `0.02` stays `0.02`).
///
/// One rule for every number — coordinates, opacities, font sizes alike. A fixed number of decimals
/// cannot serve all three: one decimal rounds an opacity of `0.02` away to nothing, four make every
/// coordinate noisy. The snap is what keeps float arithmetic from leaking `240.60000000000002` into
/// the output, which would make a rendered board's git diff churn between builds.
fn num(v: f64) -> String {
    let snapped = (v * 10_000.0).round() / 10_000.0;
    // `-0` and `0` are the same point; print one of them.
    if snapped == 0.0 {
        return "0".to_string();
    }
    format!("{snapped}")
}

fn val(v: &Val) -> String {
    match v {
        Val::Num(n) => num(*n),
        Val::Int(i) => i.to_string(),
        Val::Str(s) => esc(s),
    }
}

/// Serialize one shape's attribute list, geometry already written by the caller.
fn write_attrs(out: &mut String, attrs: &Attrs) {
    for (name, v) in attrs.iter() {
        let _ = write!(out, " {}=\"{}\"", name, val(v));
    }
}

fn write_title(out: &mut String, title: &Option<String>) {
    if let Some(t) = title {
        let _ = write!(out, "<title>{}</title>", esc(t));
    }
}

fn write_shape(out: &mut String, s: &Shape) {
    match s {
        Shape::Rect { x, y, w, h, attrs } => {
            let _ = write!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                num(*x),
                num(*y),
                num(*w),
                num(*h)
            );
            write_attrs(out, attrs);
            out.push_str("/>");
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            attrs,
        } => {
            let _ = write!(
                out,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
                num(*x1),
                num(*y1),
                num(*x2),
                num(*y2)
            );
            write_attrs(out, attrs);
            out.push_str("/>");
        }
        Shape::Circle { cx, cy, r, attrs } => {
            let _ = write!(
                out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
                num(*cx),
                num(*cy),
                num(*r)
            );
            write_attrs(out, attrs);
            out.push_str("/>");
        }
        Shape::Text {
            x,
            y,
            content,
            title,
            attrs,
        } => {
            let _ = write!(out, "<text x=\"{}\" y=\"{}\"", num(*x), num(*y));
            write_attrs(out, attrs);
            out.push('>');
            write_title(out, title);
            out.push_str(&esc(content));
            out.push_str("</text>");
        }
        Shape::Path { d, attrs } => {
            let _ = write!(out, "<path d=\"{}\"", esc(d));
            write_attrs(out, attrs);
            out.push_str("/>");
        }
        Shape::Group {
            title,
            children,
            attrs,
        } => {
            out.push_str("<g");
            write_attrs(out, attrs);
            out.push('>');
            write_title(out, title);
            for c in children {
                out.push('\n');
                write_shape(out, c);
            }
            out.push_str("\n</g>");
        }
    }
}

/// The single SVG serializer — written once, for every format.
///
/// The `<defs>` prelude carries the shared arrowhead marker. Its fill is `context-stroke`, so an
/// arrowhead always takes its own connector's colour and no format has to define a marker per hue.
pub fn render_scene(scene: &Scene) -> String {
    let (w, h) = (num(scene.width), num(scene.height));
    let mut out = String::with_capacity(4096);
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" font-family=\"-apple-system,Segoe UI,Roboto,sans-serif\">"
    );
    out.push_str(
        "\n<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
         markerWidth=\"7\" markerHeight=\"7\" orient=\"auto\">\
         <path d=\"M0,0 L10,5 L0,10 z\" fill=\"context-stroke\"/></marker></defs>",
    );
    for s in &scene.shapes {
        out.push('\n');
        write_shape(&mut out, s);
    }
    out.push_str("\n</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The serializer's whole numeric contract in one place: one rule, no per-attribute exceptions.
    /// A board's SVG has to be stable across builds for a rendered board to diff cleanly in git.
    #[test]
    fn numbers_print_the_shortest_round_tripping_form() {
        assert_eq!(num(240.0), "240");
        assert_eq!(num(240.5), "240.5");
        assert_eq!(num(-3.0), "-3");
        assert_eq!(num(0.02), "0.02"); // an opacity keeps its precision; a fixed 1 decimal ate it
        assert_eq!(num(-0.0), "0");
    }

    /// Float arithmetic must not leak into the output, or a rendered board's git diff churns
    /// between builds over digits no one can see.
    #[test]
    fn numbers_snap_to_a_grid_instead_of_printing_float_noise() {
        assert_eq!(num(0.1 + 0.2), "0.3");
        assert_eq!(num(240.60000000000002), "240.6");
    }

    #[test]
    fn attribute_values_and_text_are_escaped() {
        let s = Shape::text(0.0, 0.0, "a < b & \"c\"").with("data-label", "x'y");
        let mut out = String::new();
        write_shape(&mut out, &s);
        assert!(out.contains("data-label=\"x&#x27;y\""));
        assert!(out.contains(">a &lt; b &amp; &quot;c&quot;</text>"));
    }

    /// `set` replaces in place rather than appending a second pair, so a builder that overrides a
    /// default cannot emit the attribute twice (invalid XML, and the last one silently wins).
    #[test]
    fn setting_an_attribute_twice_replaces_it() {
        let s = Shape::rect(0.0, 0.0, 1.0, 1.0)
            .with("fill", "#fff")
            .with("fill", "#000");
        assert_eq!(s.attrs().get("fill"), Some(&Val::Str("#000".into())));
        assert_eq!(s.attrs().iter().count(), 1);
    }

    #[test]
    fn maybe_skips_a_none_attribute() {
        let s = Shape::rect(0.0, 0.0, 1.0, 1.0)
            .maybe("data-y", None::<f64>)
            .maybe("data-col", Some(3i64));
        assert!(s.attrs().get("data-y").is_none());
        assert_eq!(s.attrs().get("data-col"), Some(&Val::Int(3)));
    }

    /// Nesting is the structure a flat list of SVG strings could not express — and the reason
    /// `Group` exists as a tree node rather than an open/close pair of strings.
    #[test]
    fn groups_nest_and_carry_a_title() {
        let s = Shape::group(vec![Shape::group(vec![Shape::circle(1.0, 2.0, 3.0)])
            .with("class", "inner")
            .titled("tip")])
        .with("class", "outer");
        let mut out = String::new();
        write_shape(&mut out, &s);
        assert!(out.starts_with("<g class=\"outer\">"));
        assert!(out.contains("<g class=\"inner\"><title>tip</title>"));
        assert!(out.contains("<circle cx=\"1\" cy=\"2\" r=\"3\"/>"));
        assert_eq!(out.matches("</g>").count(), 2);
    }

    /// Geometry is typed, so the kernel can read a scene back — the property the diff overlay and
    /// the client's hit-test rely on, and the reason `Shape` is not a bag of string attributes.
    #[test]
    fn geometry_is_readable_back_off_a_shape() {
        let scene = Scene {
            width: 100.0,
            height: 50.0,
            shapes: vec![Shape::rect(10.0, 20.0, 30.0, 40.0).with("class", "card")],
        };
        let read = scene.shapes.iter().find_map(|s| match s {
            Shape::Rect { x, y, w, h, .. } => Some((*x, *y, *w, *h)),
            _ => None,
        });
        assert_eq!(read, Some((10.0, 20.0, 30.0, 40.0)));
    }

    #[test]
    fn render_scene_wraps_the_shapes_in_a_sized_svg_root() {
        let scene = Scene {
            width: 120.0,
            height: 60.0,
            shapes: vec![Shape::rect(0.0, 0.0, 120.0, 60.0).with("fill", "#fbfbfd")],
        };
        let svg = render_scene(&scene);
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"120\" height=\"60\" viewBox=\"0 0 120 60\""));
        assert!(svg.contains("<marker id=\"arrow\""));
        assert!(svg.ends_with("</svg>"));
    }
}
