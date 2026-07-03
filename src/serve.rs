//! A tiny std-only HTTP server for the live board: serves the page, re-renders the SVG on
//! demand (so an appended event shows without a restart), an in-page diff against a cached
//! baseline, and a click→comment channel appended to the event log as events.
//!
//! The server always operates in event-log mode: `main` resolves any `model.json` to its
//! sibling `event-log.jsonl` before calling [`serve`] (auto-running genesis if needed), so the
//! log is the only file this server ever mutates. There is no legacy `comments.jsonl` path.

use crate::{events, json, model, model::Model, render};
use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

const CACHE_MAX: usize = 12;

/// Upper bound on a `POST /comment` body. Comments and structural ops are a few hundred bytes;
/// 1 MiB is generous headroom while refusing an attacker-sized `Content-Length` before allocating.
const MAX_BODY: usize = 1 << 20;

struct Cache {
    map: HashMap<String, Model>,
    order: VecDeque<String>,
}

struct Ctx {
    /// The event log — the single source of truth this server reads and appends to.
    model_path: PathBuf,
    cache: Mutex<Cache>,
    /// Serializes appends to the log (H4): concurrent `POST /comment` handlers run on
    /// separate threads, so without this two events could interleave mid-line. Holding
    /// this lock around a single `write_all` makes each appended line atomic.
    appends: Mutex<()>,
}

impl Ctx {
    /// A context over an event log, with an empty recent-model ring and a free appends lock.
    fn new(model_path: PathBuf) -> Self {
        Ctx {
            model_path,
            cache: Mutex::new(Cache {
                map: HashMap::new(),
                order: VecDeque::new(),
            }),
            appends: Mutex::new(()),
        }
    }

    /// The current board and its short content hash, recorded in the recent-model ring. The board
    /// is replayed from the event log; the version hashes the raw log bytes, so the cached
    /// projection is rebuilt only when a new event lands.
    fn current(&self) -> Result<(String, Model), String> {
        let raw = std::fs::read(&self.model_path).map_err(|e| e.to_string())?;
        let version = fnv12(&raw);
        let text = String::from_utf8_lossy(&raw);
        let model = events::replay(&events::parse_log(&text)?);
        let mut c = self.cache.lock().unwrap();
        if !c.map.contains_key(&version) {
            c.map.insert(version.clone(), model.clone());
            c.order.push_back(version.clone());
            while c.order.len() > CACHE_MAX {
                if let Some(old) = c.order.pop_front() {
                    c.map.remove(&old);
                }
            }
        }
        Ok((version, model))
    }

    fn cached(&self, version: &str) -> Option<Model> {
        self.cache.lock().unwrap().map.get(version).cloned()
    }

    /// Append a text block (one line, or several newline-joined lines for a multi-event action)
    /// to `path`, serialized against all other appends so concurrent posts cannot interleave
    /// (H4). The block and its trailing newline are written in a single `write_all`, so even
    /// under `O_APPEND` no half-line — and no half of a multi-event action — ever lands.
    fn append_line(&self, path: &Path, line: &str) -> std::io::Result<()> {
        let _guard = self.appends.lock().unwrap();
        Self::write_line(path, line)
    }

    /// The atomic write itself — one line + newline in a single `write_all`. Callers must
    /// already hold `appends`; `append_line` is the locked entry point.
    fn write_line(path: &Path, line: &str) -> std::io::Result<()> {
        let mut f = OpenOptions::new().create(true).append(true).open(path)?;
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        f.write_all(&bytes)
    }

    /// Run a server-minted append under the `appends` lock: read + parse the log once, let `build`
    /// construct the event from it (minting ids, deriving cols, or validating and returning `Err` to
    /// refuse), then write the event atomically. The single home of the read → build → write critical
    /// section (H4/H6) shared by every server-minted command — so the mint-under-lock contract, the
    /// corrupt-log error path, and the atomic write live in exactly one place, not three copies.
    fn append_minted(
        &self,
        build: impl FnOnce(&[events::Event]) -> Result<events::Event, String>,
    ) -> Result<events::Event, String> {
        let _guard = self.appends.lock().unwrap();
        // Mint/validate from the *real* log. A corrupt/unreadable log must fail the append, not
        // silently fold to empty (which would re-mint E1/C1… and collide).
        let raw = std::fs::read(&self.model_path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&raw);
        let log = events::parse_log(&text)?;
        let ev = build(&log)?;
        Self::write_line(&self.model_path, &events::line(&ev)).map_err(|e| e.to_string())?;
        Ok(ev)
    }

    /// Mint a fresh id and append an `ElementAdded` (H6). The id is derived from the log's
    /// `ElementAdded` history (next free `<PREFIX><N>` for this lane) rather than a stored counter,
    /// so the log stays the only durable record. A lane-title `+` (prepend) derives its col from the
    /// live projection *under the lock* — a first-in-lane add aligns to the board's left column, a
    /// prepend into a non-empty lane marches left — so concurrent adds can't race the min.
    fn append_add(
        &self,
        kind: &str,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
        prepend: bool,
    ) -> Result<events::Event, String> {
        self.append_minted(|log| {
            let col = if prepend {
                Some(model::lane_left_col(&events::replay(log), kind))
            } else {
                col
            };
            Ok(events::Event::ElementAdded {
                id: mint_id(kind, log),
                kind: kind.to_string(),
                label,
                col,
                detail,
                y: None,
            })
        })
    }

    /// Mint a fresh region id and append a `PhaseAdded` — the region counterpart of `append_add`.
    fn append_region_add(
        &self,
        label: String,
        from_col: i64,
        to_col: i64,
    ) -> Result<events::Event, String> {
        self.append_minted(|log| {
            Ok(events::Event::PhaseAdded {
                id: Some(mint_region_id(log)),
                label,
                from_col,
                to_col,
            })
        })
    }

    /// Mint the right-half id and append a `PhaseSplit` — the partition's "add" (F-region-frontiers).
    /// The split target `id` keeps its left half; the minted `new_id` (same namespace as
    /// `append_region_add`, so the two never collide) takes the right half with `new_label`. Validates
    /// the split column against the replayed board: an out-of-range or stale `at_col` (the target
    /// phase moved/merged/vanished since the client hovered) returns `Err` *before* writing —
    /// otherwise it would burn a region id and leave a permanent dead event in the append-only log
    /// while the client falsely reported success. `replay` still guards the fold as a backstop.
    fn append_phase_split(
        &self,
        id: String,
        at_col: i64,
        new_label: String,
    ) -> Result<events::Event, String> {
        self.append_minted(|log| {
            let covers = events::replay(log)
                .phases
                .iter()
                .any(|p| p.id == id && p.from_col < at_col && at_col <= p.to_col);
            if !covers {
                return Err(format!(
                    "phase-split: {at_col} not strictly inside phase {id}"
                ));
            }
            Ok(events::Event::PhaseSplit {
                id,
                at_col,
                new_id: mint_region_id(log),
                new_label,
            })
        })
    }
}

/// The single letter each lane stamps onto a freshly minted id. The 8-lane prefixes come from
/// `render::lane_prefix` (one source of truth, in sync with `LANES`); an off-grammar type falls
/// back to its first letter, upper-cased.
fn id_prefix(kind: &str) -> char {
    render::lane_prefix(kind)
        .unwrap_or_else(|| kind.chars().next().unwrap_or('Z').to_ascii_uppercase())
}

/// Next free id for `kind`: `<PREFIX>` one past the highest suffix **ever added** under that
/// prefix in the log — scanning every `ElementAdded`, including ids since removed but not yet
/// compacted away. Deriving from the live projection instead would re-mint a removed element's
/// id while leftover events still reference it (e.g. its annotations in `/comments`). `compact`
/// folds removed elements out entirely, so reuse after compaction is safe.
fn mint_id(kind: &str, log: &[events::Event]) -> String {
    let prefix = id_prefix(kind);
    let max = log
        .iter()
        .filter_map(|ev| match ev {
            events::Event::ElementAdded { id, .. } => id
                .strip_prefix(prefix)
                .and_then(|rest| rest.parse::<u32>().ok()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    // saturating_add: a hand-edited log with a suffix at u32::MAX must not panic (debug) or
    // wrap to 0 (release) and re-mint a colliding low id.
    format!("{}{}", prefix, max.saturating_add(1))
}

/// Next free region id: `K<n>` one past the highest `K` suffix **ever seen** in the log —
/// explicit ids on `PhaseAdded`, *and* the synthetic ids `replay` mints for legacy id-less bands
/// (carry-over review #3, `F-container-scope.md`). Folding through `model::resolve_region_id` for
/// every `PhaseAdded` is exactly what `replay` does to compute its own `max_region`, so this mint
/// shares that namespace by construction — a region id can never collide with one replay would
/// have synthesized, and a removed-but-not-compacted suffix stays reserved (same rule as `mint_id`).
fn mint_region_id(log: &[events::Event]) -> String {
    let mut max_region = 0u32;
    for ev in log {
        match ev {
            events::Event::PhaseAdded { id, .. } => {
                model::resolve_region_id(id.as_deref(), &mut max_region);
            }
            // A split mints a new region id too (F-region-frontiers); fold it through the same
            // watermark so a later add/split never re-mints a suffix already spent by a split.
            events::Event::PhaseSplit { new_id, .. } => {
                model::resolve_region_id(Some(new_id), &mut max_region);
            }
            _ => {}
        }
    }
    format!("K{}", max_region.saturating_add(1))
}

/// Serve the live board for an event log. `main` guarantees `log_path` is an `event-log.jsonl`
/// (resolving any `model.json` through `serve_log_path` first), so this server only ever reads and
/// appends to the log — the single source of truth.
pub fn serve(log_path: &Path, port: u16) -> Result<(), String> {
    let ctx = Arc::new(Ctx::new(log_path.to_path_buf()));

    // Validate the log up front so a typo fails loudly, not per-request.
    let (_v, model) = ctx.current()?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    println!(
        "faceto board live → http://127.0.0.1:{}  (Ctrl-C to stop)",
        port
    );
    println!(
        "  {} elements · event-sourced · edits append to {}",
        model.elements.len(),
        ctx.model_path.display()
    );

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let ctx = ctx.clone();
        thread::spawn(move || {
            let _ = handle(stream, ctx);
        });
    }
    Ok(())
}

fn handle(stream: TcpStream, ctx: Arc<Ctx>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut out = stream;

    let mut req_line = String::new();
    if reader.read_line(&mut req_line)? == 0 {
        return Ok(());
    }
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        let lower = t.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => match ctx.current() {
            Ok((_v, model)) => {
                let svg = render::render_svg(&model, &render::View::none());
                let html = render::render_html(&svg, &model.title);
                send(
                    &mut out,
                    200,
                    "text/html; charset=utf-8",
                    html.as_bytes(),
                    &[],
                )
            }
            Err(e) => send(
                &mut out,
                500,
                "text/plain; charset=utf-8",
                e.as_bytes(),
                &[],
            ),
        },
        ("GET", "/board.svg") => match ctx.current() {
            Ok((version, model)) => {
                let base = query_get(query, "base");
                // A per-viewer reading lens, never persisted: the collapsed-region set the client
                // holds in localStorage (F-region-collapse). Composes with `?base=` — the baseline
                // is folded with the *same* view below so the diff overlay lines up column-for-column.
                let view = render::View {
                    collapsed: parse_collapse(query),
                };
                let old = base
                    .as_deref()
                    .filter(|b| *b != version)
                    .and_then(|b| ctx.cached(b));
                if let (Some(old), Some(base)) = (old, &base) {
                    let merged =
                        model::diff_models(&old, &model, ("last seen".into(), "now".into()));
                    let svg = render::render_svg(&merged, &view) + "\n";
                    send(
                        &mut out,
                        200,
                        "image/svg+xml",
                        svg.as_bytes(),
                        &[("X-Diff-Base", base.as_str())],
                    )
                } else {
                    let svg = render::render_svg(&model, &view) + "\n";
                    send(&mut out, 200, "image/svg+xml", svg.as_bytes(), &[])
                }
            }
            Err(e) => send(
                &mut out,
                500,
                "text/plain; charset=utf-8",
                e.as_bytes(),
                &[],
            ),
        },
        ("GET", "/model-version") => match ctx.current() {
            Ok((version, _)) => {
                let body = format!("{{\"version\":\"{}\"}}", version);
                send(&mut out, 200, "application/json", body.as_bytes(), &[])
            }
            Err(e) => send(
                &mut out,
                500,
                "text/plain; charset=utf-8",
                e.as_bytes(),
                &[],
            ),
        },
        ("GET", "/comments") => {
            let body = comments_body(&ctx);
            send(&mut out, 200, "application/json", body.as_bytes(), &[])
        }
        ("GET", "/health") => send(&mut out, 200, "application/json", b"{\"ok\":true}", &[]),
        ("POST", "/comment") => {
            // Cap the body before allocating: `content_length` is attacker-controlled (a header),
            // and a comment/structural op is tiny, so a huge value is a bug or a DoS, never real.
            if content_length > MAX_BODY {
                return send(&mut out, 413, "application/json", b"{\"ok\":false}", &[]);
            }
            let mut buf = vec![0u8; content_length];
            if reader.read_exact(&mut buf).is_err() {
                return send(&mut out, 400, "application/json", b"{\"ok\":false}", &[]);
            }
            let text = String::from_utf8_lossy(&buf);
            match json::parse(&text) {
                Ok(v @ json::Json::Obj(_))
                    if matches!(
                        v.get_str("kind"),
                        Some("add") | Some("region-add") | Some("phase-split")
                    ) =>
                {
                    // All three need a server-minted id (H6 / review #3 / F-region-frontiers), so
                    // they share the mint-and-respond path — only which append fn mints differs.
                    let result = match v.get_str("kind") {
                        Some("add") => add_from_comment(&ctx, &v),
                        Some("region-add") => add_region_from_comment(&ctx, &v),
                        _ => split_region_from_comment(&ctx, &v),
                    };
                    match result {
                        Ok(ev) => {
                            println!("  \u{2795} event: {}", events::line(&ev));
                            send(&mut out, 200, "application/json", b"{\"ok\":true}", &[])
                        }
                        Err(code) => {
                            send(&mut out, code, "application/json", b"{\"ok\":false}", &[])
                        }
                    }
                }
                Ok(v @ json::Json::Obj(_)) => {
                    let evs = events::comment_to_events(&v);
                    if evs.is_empty() {
                        return send(&mut out, 400, "application/json", b"{\"ok\":false}", &[]);
                    }
                    // Join so a multi-event action (a swap is two `ElementMoved`s) lands as
                    // consecutive lines under one append — never split by another post.
                    let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
                    if ctx.append_line(&ctx.model_path, &block).is_err() {
                        return send(&mut out, 500, "application/json", b"{\"ok\":false}", &[]);
                    }
                    println!("  \u{1F4AC} event: {}", block);
                    send(&mut out, 200, "application/json", b"{\"ok\":true}", &[])
                }
                _ => send(&mut out, 400, "application/json", b"{\"ok\":false}", &[]),
            }
        }
        _ => send(
            &mut out,
            404,
            "text/plain; charset=utf-8",
            b"not found",
            &[],
        ),
    }
}

fn send(
    out: &mut impl Write,
    code: u16,
    ctype: &str,
    body: &[u8],
    extra: &[(&str, &str)],
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        code,
        reason,
        ctype,
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{}: {}\r\n", k, v));
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())?;
    out.write_all(body)?;
    out.flush()
}

/// The collapsed-region ids from `?collapse=K2,K5` — the client's reading lens (F-region-collapse),
/// never persisted. Comma-separated, empty segments dropped, so `?collapse=` (or an absent key) is
/// the empty set = the identity render. Unknown ids are harmless: `render_svg` ignores an id that
/// matches no region.
fn parse_collapse(query: &str) -> Vec<String> {
    query_get(query, "collapse")
        .map(|s| {
            s.split(',')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn query_get(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// The non-blank `text` label a creation command requires, or `400` — the one place the
/// "a minted element/region must carry a label" rule turns into an HTTP status, shared by every
/// server-minted command so `add`, `region-add`, and `phase-split` can't diverge on it. A blank
/// label would mint a permanent, never-renumbered empty box; the same `nonblank` rule the `rename`
/// guard uses (a direct POST must not slip a blank in even though the client modal guards it).
fn required_label(v: &json::Json) -> Result<String, u16> {
    v.get_str("text").and_then(events::nonblank).ok_or(400u16)
}

/// Handle a `kind:"add"` post: an element-creation command rather than a comment on an
/// existing one. `type` (the lane) and a non-empty `text` (label) are required; optional
/// `col`/`detail`. The server mints the id (H6). Returns the HTTP status to fail with: `400` for
/// a missing/empty type or label, `500` if the append itself fails.
fn add_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    // `type` must be one of the 8 lanes. An off-grammar type would fall back to a first-letter
    // prefix in `id_prefix` and could mint into a real lane's id space (e.g. "epic"→'E'),
    // colliding the diff/comment join key — so reject it here rather than letting it through.
    let kind = v
        .get_str("type")
        .filter(|s| render::lane_prefix(s).is_some())
        .ok_or(400u16)?
        .to_string();
    let label = required_label(v)?;
    let col = v.get_i64("col");
    // The lane-title `+` posts `prepend:true` (no col); the server derives the left-edge col so the
    // rule lives in one place and stays consistent under concurrent adds.
    let prepend = v
        .get("prepend")
        .and_then(json::Json::as_bool)
        .unwrap_or(false);
    let detail = v
        .get_str("detail")
        .filter(|s| !s.is_empty())
        .map(String::from);
    ctx.append_add(&kind, label, col, detail, prepend)
        .map_err(|_| 500u16)
}

/// Handle a `kind:"region-add"` post: a region-creation command, the region counterpart of
/// `add_from_comment`. A non-empty `text` (label) and a well-ordered `[fromCol, toCol]` span
/// (`events::valid_span`) are required; the server mints the id (review #3 / H6 for regions).
/// Returns the HTTP status to fail with: `400` for a missing label or an absent/inverted/
/// zero-width span, `500` if the append itself fails.
fn add_region_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let label = required_label(v)?;
    let from_col = v.get_i64("fromCol").ok_or(400u16)?;
    let to_col = v.get_i64("toCol").ok_or(400u16)?;
    if !events::valid_span(from_col, to_col) {
        return Err(400u16);
    }
    ctx.append_region_add(label, from_col, to_col)
        .map_err(|_| 500u16)
}

/// Handle a `kind:"phase-split"` post: divide the region `regionId` at `atCol` into two, the
/// server minting the right half's id (F-region-frontiers, the partition's "add"). A non-empty
/// `text` (the new right-half label) and an `atCol` are required; whether the column falls strictly
/// inside the phase is validated under the lock in `append_phase_split` (a stale/out-of-range split
/// is refused before writing). Returns the HTTP status to fail with: `400` for a missing label or
/// atCol, `500` if the append itself fails or the split is out of range.
fn split_region_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let id = v.get_str("regionId").map(str::to_string).ok_or(400u16)?;
    let label = required_label(v)?;
    let at_col = v.get_i64("atCol").ok_or(400u16)?;
    ctx.append_phase_split(id, at_col, label)
        .map_err(|_| 500u16)
}

/// The full `/comments` response body: stored feedback first, then the live lint findings,
/// framed as one JSON array. The lint merge is **best-effort** — if the log doesn't parse, the
/// stored comments still come back on their own: a malformed / half-written log degrades to
/// comments-only (here, empty) rather than hiding the sidebar behind a 500, the resilience the
/// endpoint had before lint was merged in.
///
/// The log is read + replayed **once**: the single projection feeds both the comment fold
/// ([`comments_from_log`]) and the lint pass, so the comment set and the findings always reflect
/// the same snapshot (and the log isn't read/replayed twice per request).
fn comments_body(ctx: &Ctx) -> String {
    let mut items = Vec::new();
    if let Ok(log) = events::read_log(&ctx.model_path) {
        let model = events::replay(&log);
        items.extend(comments_from_log(&log, &model));
        items.extend(lint_items(&model));
    }
    format!("[{}]", items.join(","))
}

/// One sidebar comment item — the `{elemId, kind, text, status:"open"}` JSON string the client
/// renders. The single definition of the sidebar wire-shape, shared by the log projection
/// ([`comments_from_log`]) and the lint merge ([`lint_items`]) so the two lanes can never drift.
fn comment_item(elem_id: &str, kind: &str, text: &str) -> String {
    let obj = json::Json::Obj(vec![
        ("elemId".into(), json::Json::Str(elem_id.to_string())),
        ("kind".into(), json::Json::Str(kind.to_string())),
        ("text".into(), json::Json::Str(text.to_string())),
        ("status".into(), json::Json::Str("open".into())),
    ]);
    json::to_string(&obj)
}

/// Project the log's *feedback* events (annotations, resolutions, renames) back into the
/// comment shape the client sidebar expects. Structural events (adds, moves, edges) are
/// omitted — they already live in the rendered board. Feedback on an element that was later
/// removed is dropped too, so the sidebar never lists a comment for a box that's off the board.
///
/// Takes the already-parsed `log` and its `model` projection ([`comments_body`] reads and replays
/// the source once, then feeds both here and to [`lint_items`]). Returns the item JSON strings, not
/// the joined array.
fn comments_from_log(log: &[events::Event], model: &Model) -> Vec<String> {
    let present: std::collections::HashSet<&str> =
        model.elements.iter().map(|e| e.id.as_str()).collect();
    let mut items: Vec<String> = Vec::new();
    for ev in log {
        let (id, kind, text) = match ev {
            events::Event::ElementAnnotated { id, text } => (id, "comment", text.clone()),
            events::Event::HotspotResolved { id, resolution } => {
                (id, "resolve", resolution.clone())
            }
            events::Event::ElementRenamed { id, label } => (id, "rename", label.clone()),
            _ => continue,
        };
        if !present.contains(id.as_str()) {
            continue;
        }
        items.push(comment_item(id, kind, &text));
    }
    items
}

/// The live lint findings for a board, in the same comment shape the sidebar renders — a
/// `kind:"lint"` entry keyed on the offending element's stable `id`. Computed on read (never
/// persisted): a finding is *derived* from the current graph, so recomputing it each request
/// keeps it always-fresh and can never go stale against an edited board. A finding on an element
/// the reviewer has already **resolved** (a `HotspotResolved` set `resolved:true`) is suppressed
/// — that is the whole "reuse serve→review→resolve" story, keyed on `Finding.element_id` == the
/// same stable id `HotspotResolved.id` uses. Per-finding acknowledgement is F-comment-lifecycle's.
///
/// This resolve-suppression is deliberately serve-only: the `faceto lint` CLI runs `lint()`
/// unfiltered (a full audit reports on resolved elements too). The divergence is intended — the
/// sidebar is the interactive review loop, the CLI is the complete check — and safe, since lint is
/// warn-only (exit 0) at both surfaces, so a suppressed nudge can never gate a build.
fn lint_items(model: &Model) -> Vec<String> {
    // Build the resolved-id set once (O(V)) so the per-finding suppression check is O(1) — the
    // same present-set idiom `comments_from_log` uses, instead of an O(findings × elements) rescan.
    let resolved: std::collections::HashSet<&str> = model
        .elements
        .iter()
        .filter(|e| e.resolved)
        .map(|e| e.id.as_str())
        .collect();
    crate::lint::lint(model)
        .into_iter()
        .filter(|f| !resolved.contains(f.element_id.as_str()))
        .map(|f| comment_item(&f.element_id, "lint", f.message))
        .collect()
}

/// FNV-1a 64-bit, first 12 hex chars — a stable, dependency-free content version token.
fn fnv12(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_collapse_splits_ids_and_treats_empty_as_the_identity_set() {
        assert_eq!(parse_collapse("collapse=K2,K5"), vec!["K2", "K5"]);
        assert_eq!(parse_collapse("base=abc&collapse=K2"), vec!["K2"]);
        // Absent key, an empty value, and stray empty segments all fold to the empty (identity) set.
        assert!(parse_collapse("base=abc").is_empty());
        assert!(parse_collapse("collapse=").is_empty());
        assert_eq!(parse_collapse("collapse=,K2,"), vec!["K2"]);
    }

    #[test]
    fn fnv12_is_deterministic_and_twelve_hex_chars() {
        // FNV-1a offset basis, for empty input.
        assert_eq!(fnv12(b""), "cbf29ce48422");
        let h = fnv12(b"faceto");
        assert_eq!(h.len(), 12);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(fnv12(b"faceto"), h);
        assert_ne!(fnv12(b"faceto"), fnv12(b"faceto "));
    }

    #[test]
    fn concurrent_appends_never_interleave() {
        // H4: many threads append to one log through a shared Ctx; every line must land
        // whole and intact, with the expected total count and no torn/merged lines.
        let path = std::env::temp_dir().join(format!("faceto-h4-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let ctx = Arc::new(Ctx::new(path.clone()));

        const THREADS: usize = 8;
        const PER_THREAD: usize = 50;
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let ctx = Arc::clone(&ctx);
                let path = path.clone();
                thread::spawn(move || {
                    for i in 0..PER_THREAD {
                        // A long payload makes a torn write easy to detect if the lock fails.
                        let line = format!("t{t}-i{i}-{}", "x".repeat(200));
                        ctx.append_line(&path, &line).unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        let contents = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), THREADS * PER_THREAD);
        // Every line is whole: matches the exact shape we wrote, nothing spliced.
        for line in &lines {
            assert!(
                line.starts_with('t') && line.ends_with(&"x".repeat(200)),
                "torn line: {line}"
            );
        }
    }

    fn added(id: &str, kind: &str) -> events::Event {
        events::Event::ElementAdded {
            id: id.into(),
            kind: kind.into(),
            label: id.into(),
            col: None,
            detail: None,
            y: None,
        }
    }

    #[test]
    fn mint_id_picks_next_free_suffix_per_lane() {
        // H6: ids are type-prefixed and never renumbered — minting takes one past the
        // highest suffix already used under that prefix, independently per lane.
        let log = [
            added("E1", "event"),
            added("E3", "event"),
            added("C1", "command"),
        ];
        assert_eq!(mint_id("event", &log), "E4"); // past the highest E, not filling the E2 gap
        assert_eq!(mint_id("command", &log), "C2");
        assert_eq!(mint_id("hotspot", &log), "H1"); // empty lane starts at 1
        assert_eq!(mint_id("actor", &log), "X1"); // actor stamps X, not A
        assert_eq!(mint_id("aggregate", &log), "A1");
    }

    #[test]
    fn mint_id_does_not_reuse_a_removed_id() {
        // A dropped element's ElementAdded stays in the log (until compaction), so its id must
        // stay reserved — re-minting it would alias leftover events (e.g. its annotations).
        let log = [
            added("E1", "event"),
            added("E2", "event"),
            events::Event::ElementRemoved { id: "E2".into() },
        ];
        assert_eq!(mint_id("event", &log), "E3");
    }

    fn region_added(id: &str, from_col: i64, to_col: i64) -> events::Event {
        events::Event::PhaseAdded {
            id: Some(id.into()),
            label: id.into(),
            from_col,
            to_col,
        }
    }

    #[test]
    fn mint_region_id_picks_next_free_k_suffix() {
        let log = [region_added("K1", 0, 2), region_added("K3", 3, 5)];
        assert_eq!(mint_region_id(&log), "K4"); // past the highest K, not filling the K2 gap
        assert_eq!(mint_region_id(&[]), "K1"); // empty log starts at 1
    }

    #[test]
    fn mint_region_id_does_not_reuse_a_removed_id() {
        let log = [
            region_added("K1", 0, 2),
            region_added("K2", 3, 5),
            events::Event::PhaseRemoved { id: "K2".into() },
        ];
        assert_eq!(mint_region_id(&log), "K3");
    }

    #[test]
    fn mint_region_id_shares_the_namespace_with_replays_synthetic_ids() {
        // Review #3: a legacy id-less PhaseAdded replays to a synthetic K<n> (resolve_region_id).
        // The mint must reserve that suffix too, or a fresh region could collide with one replay
        // would later synthesize for the same log.
        let log = [events::Event::PhaseAdded {
            id: None,
            label: "Legacy".into(),
            from_col: 0,
            to_col: 2,
        }];
        assert_eq!(mint_region_id(&log), "K2"); // K1 is reserved for the legacy band
        let model = events::replay(&log);
        assert_eq!(model.phases[0].id, "K1");
    }

    #[test]
    fn append_region_add_mints_persists_and_replays() {
        let path =
            std::env::temp_dir().join(format!("faceto-region-h6-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&region_added("K1", 0, 2)) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());

        let ev = ctx.append_region_add("Checkout".into(), 3, 6).unwrap();
        assert!(matches!(&ev, events::Event::PhaseAdded { id: Some(id), .. } if id == "K2"));
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let model = events::replay(&events::parse_log(&text).unwrap());
        let k2 = model.phases.iter().find(|p| p.id == "K2").unwrap();
        assert_eq!(k2.label, "Checkout");
        assert_eq!((k2.from_col, k2.to_col), (3, 6));
    }

    #[test]
    fn mint_region_id_reserves_split_ids() {
        // A split's minted right-half id lives in the same namespace; the next mint must skip it.
        let log = [
            region_added("K1", 0, 5),
            events::Event::PhaseSplit {
                id: "K1".into(),
                at_col: 3,
                new_id: "K2".into(),
                new_label: "Right".into(),
            },
        ];
        assert_eq!(mint_region_id(&log), "K3", "K2 is spent by the split");
    }

    #[test]
    fn append_phase_split_mints_the_right_half_and_replays_to_a_partition() {
        let path =
            std::env::temp_dir().join(format!("faceto-split-h6-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&region_added("K1", 0, 5)) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());

        let ev = ctx
            .append_phase_split("K1".into(), 3, "Right".into())
            .unwrap();
        assert!(matches!(
            &ev,
            events::Event::PhaseSplit { id, at_col: 3, new_id, new_label }
                if id == "K1" && new_id == "K2" && new_label == "Right"
        ));
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let model = events::replay(&events::parse_log(&text).unwrap());
        let spans: Vec<_> = model
            .phases
            .iter()
            .map(|p| (p.id.as_str(), p.from_col, p.to_col))
            .collect();
        assert_eq!(
            spans,
            vec![("K1", 0, 2), ("K2", 3, 5)],
            "split carves K1 in two, contiguous partition preserved"
        );
    }

    #[test]
    fn append_phase_split_rejects_an_out_of_range_split_without_writing() {
        // Review #2: a stale/out-of-range split (atCol not strictly inside the target phase) must
        // Err *before* writing — no dead event in the append-only log, no burned region id, no false
        // success. Here atCol=9 is past K1[0,5]'s to_col.
        let path =
            std::env::temp_dir().join(format!("faceto-split-oor-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let seed = events::line(&region_added("K1", 0, 5)) + "\n";
        std::fs::write(&path, &seed).unwrap();
        let ctx = Ctx::new(path.clone());

        assert!(
            ctx.append_phase_split("K1".into(), 9, "Right".into())
                .is_err(),
            "out-of-range split is rejected"
        );
        let after = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(after, seed, "nothing was appended");
        // The next mint is still K2 — no id was burned by the rejected split.
        assert_eq!(mint_region_id(&events::parse_log(&after).unwrap()), "K2");
    }

    #[test]
    fn region_resize_rename_remove_map_to_phase_events() {
        let resize =
            json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":0,"toCol":5}"#)
                .unwrap();
        let evs = events::comment_to_events(&resize);
        assert_eq!(evs.len(), 1);
        assert!(matches!(
            &evs[0],
            events::Event::PhaseResized { id, from_col: 0, to_col: 5 } if id == "K1"
        ));

        let rename =
            json::parse(r#"{"kind":"region-rename","regionId":"K1","text":"Fulfillment"}"#)
                .unwrap();
        let evs = events::comment_to_events(&rename);
        assert_eq!(evs.len(), 1);
        assert!(
            matches!(&evs[0], events::Event::PhaseRenamed { id, label } if id == "K1" && label == "Fulfillment")
        );

        let remove = json::parse(r#"{"kind":"region-remove","regionId":"K1"}"#).unwrap();
        let evs = events::comment_to_events(&remove);
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], events::Event::PhaseRemoved { id } if id == "K1"));
    }

    #[test]
    fn region_edits_with_missing_data_are_rejected() {
        let no_span = json::parse(r#"{"kind":"region-resize","regionId":"K1"}"#).unwrap();
        assert!(events::comment_to_events(&no_span).is_empty());

        let blank_rename =
            json::parse(r#"{"kind":"region-rename","regionId":"K1","text":"   "}"#).unwrap();
        assert!(events::comment_to_events(&blank_rename).is_empty());

        let no_region = json::parse(r#"{"kind":"region-remove"}"#).unwrap();
        assert!(events::comment_to_events(&no_region).is_empty());
    }

    #[test]
    fn region_resize_rejects_an_inverted_or_zero_width_span() {
        // A resize into fromCol >= toCol would make region_of's `from_col <= col <= to_col`
        // test unsatisfiable for any col, silently dropping the region from every column's
        // membership while render still draws a (normalized) visible band for it.
        let inverted =
            json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":9,"toCol":2}"#)
                .unwrap();
        assert!(events::comment_to_events(&inverted).is_empty());

        let zero_width =
            json::parse(r#"{"kind":"region-resize","regionId":"K1","fromCol":3,"toCol":3}"#)
                .unwrap();
        assert!(events::comment_to_events(&zero_width).is_empty());
    }

    #[test]
    fn add_region_from_comment_rejects_an_inverted_or_zero_width_span() {
        let ctx = Ctx::new(std::env::temp_dir().join("faceto-nonexistent-region.jsonl"));
        // The span check runs before any file access, same as the label check — no file needed.
        let inverted =
            json::parse(r#"{"kind":"region-add","text":"X","fromCol":5,"toCol":2}"#).unwrap();
        assert_eq!(add_region_from_comment(&ctx, &inverted), Err(400));

        let zero_width =
            json::parse(r#"{"kind":"region-add","text":"X","fromCol":4,"toCol":4}"#).unwrap();
        assert_eq!(add_region_from_comment(&ctx, &zero_width), Err(400));
    }

    #[test]
    fn drop_maps_to_element_removed() {
        let v = json::parse(r#"{"elemId":"E2","kind":"drop","text":"never happened"}"#).unwrap();
        let evs = events::comment_to_events(&v);
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], events::Event::ElementRemoved { id } if id == "E2"));
    }

    #[test]
    fn mint_id_saturates_instead_of_overflowing() {
        // A hand-edited log with a suffix at u32::MAX must not panic/wrap.
        let log = [added("E4294967295", "event")];
        assert_eq!(mint_id("event", &log), "E4294967295");
    }

    #[test]
    fn move_without_a_col_is_rejected() {
        let v = json::parse(r#"{"elemId":"E1","kind":"move"}"#).unwrap();
        assert!(events::comment_to_events(&v).is_empty());
    }

    #[test]
    fn move_ignores_a_self_swap_or_a_swap_missing_its_col() {
        // Self-swap → just the primary move; swapId without swapCol → no phantom partner move.
        let selfswap =
            json::parse(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E1","swapCol":0}"#)
                .unwrap();
        assert_eq!(events::comment_to_events(&selfswap).len(), 1);
        let nocol = json::parse(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E2"}"#).unwrap();
        assert_eq!(events::comment_to_events(&nocol).len(), 1);
    }

    #[test]
    fn comments_from_log_skips_a_removed_elements_feedback() {
        // Annotate E2, then drop it — its comment must not surface for a box off the board.
        let log = [
            added("E2", "event"),
            events::Event::ElementAnnotated {
                id: "E2".into(),
                text: "is this right?".into(),
            },
            events::Event::ElementRemoved { id: "E2".into() },
        ];
        let model = events::replay(&log);
        assert!(
            comments_from_log(&log, &model).is_empty(),
            "feedback on a removed element is dropped"
        );
    }

    // ---- F-es-lint: lint findings merged into the sidebar (derived on read) ----------------

    fn model_of(src: &str) -> Model {
        model::from_json(&json::parse(src).unwrap())
    }

    #[test]
    fn lint_items_surfaces_a_finding_as_a_lint_kind_comment() {
        // An orphan event (no producer, no consumer) yields two lint entries, both keyed on E1.
        let m = model_of(r#"{"elements":[{"id":"E1","type":"event","label":"Lonely","col":0}]}"#);
        let items = lint_items(&m);
        assert_eq!(items.len(), 2);
        for item in &items {
            assert!(item.contains(r#""kind":"lint""#));
            assert!(item.contains(r#""elemId":"E1""#));
            assert!(item.contains(r#""status":"open""#));
        }
    }

    #[test]
    fn lint_items_is_empty_for_a_grammar_clean_board() {
        let m = model_of(
            r#"{"elements":[
                {"id":"C1","type":"command","label":"do","col":0},
                {"id":"E1","type":"event","label":"Done","col":1},
                {"id":"R1","type":"readmodel","label":"view","col":2}],
              "edges":[["C1","E1"],["E1","R1"]]}"#,
        );
        assert!(lint_items(&m).is_empty());
    }

    #[test]
    fn a_finding_on_a_resolved_element_is_suppressed() {
        // Reuse the existing resolve path: once E1 carries resolved:true its findings drop out —
        // no new endpoint, just the HotspotResolved-driven `resolved` flag the model already has.
        let src = r#"{"elements":[{"id":"E1","type":"event","label":"Lonely","col":0RESOLVED}]}"#;
        let live = model_of(&src.replace("RESOLVED", ""));
        assert_eq!(
            lint_items(&live).len(),
            2,
            "an unresolved orphan still nudges"
        );
        let resolved = model_of(&src.replace("RESOLVED", r#","resolved":true"#));
        assert!(
            lint_items(&resolved).is_empty(),
            "a resolved element's findings are suppressed"
        );
    }

    #[test]
    fn a_design_board_surfaces_the_command_rule_through_lint_items() {
        // The merge honours the board's level for free: lint_items reads model.level.
        let m = model_of(
            r#"{"level":"design","elements":[
                {"id":"C1","type":"command","label":"orphan","col":0}]}"#,
        );
        let items = lint_items(&m);
        assert_eq!(items.len(), 1);
        assert!(items[0].contains(r#""elemId":"C1""#) && items[0].contains(r#""kind":"lint""#));
    }

    #[test]
    fn comments_body_degrades_to_comments_only_on_a_malformed_source() {
        // A corrupt log must not 500 the sidebar: comments_body still returns a valid (here empty)
        // JSON array instead of failing, so a malformed source can't hide the stored comments.
        let path = std::env::temp_dir().join(format!("faceto-cb-bad-{}.jsonl", std::process::id()));
        std::fs::write(&path, "not json at all\n").unwrap();
        let ctx = Ctx::new(path.clone());
        let body = comments_body(&ctx);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            body, "[]",
            "a malformed log degrades to comments-only, never a 500"
        );
    }

    #[test]
    fn comments_body_merges_lint_findings_when_the_board_parses() {
        // A valid log with an orphan event: no stored comments, two lint nudges, framed as an array.
        let path = std::env::temp_dir().join(format!("faceto-cb-ok-{}.jsonl", std::process::id()));
        std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());
        let body = comments_body(&ctx);
        let _ = std::fs::remove_file(&path);
        assert!(body.starts_with('[') && body.ends_with(']'));
        assert_eq!(body.matches(r#""kind":"lint""#).count(), 2);
    }

    #[test]
    fn append_add_mints_persists_and_replays() {
        // The minted id round-trips: append_add writes an ElementAdded that replay folds
        // back into a real element, and a second add under the same lane increments.
        let path = std::env::temp_dir().join(format!("faceto-h6-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());

        let ev = ctx
            .append_add("event", "DayStarted".into(), Some(2), None, false)
            .unwrap();
        assert!(matches!(&ev, events::Event::ElementAdded { id, .. } if id == "E2"));
        let ev2 = ctx
            .append_add("command", "start".into(), None, None, false)
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let model = events::replay(&events::parse_log(&text).unwrap());
        let e2 = model.elements.iter().find(|e| e.id == "E2").unwrap();
        assert_eq!(e2.label, "DayStarted");
        assert_eq!(e2.col, Some(2));
        assert!(matches!(&ev2, events::Event::ElementAdded { id, .. } if id == "C1"));
    }

    #[test]
    fn move_with_swap_persists_both_stickies() {
        // A move into an occupied column swaps two stickies; both relocations must be logged,
        // else the partner reverts on the next replay and the two overlap.
        let v = json::parse(r#"{"elemId":"E1","kind":"move","col":3,"swapId":"E2","swapCol":1}"#)
            .unwrap();
        let evs = events::comment_to_events(&v);
        assert_eq!(evs.len(), 2);
        assert!(
            matches!(&evs[0], events::Event::ElementMoved { id, col: Some(3), .. } if id == "E1")
        );
        assert!(
            matches!(&evs[1], events::Event::ElementMoved { id, col: Some(1), .. } if id == "E2")
        );
    }

    #[test]
    fn plain_move_is_one_event_and_no_elem_id_is_rejected() {
        let mv = json::parse(r#"{"elemId":"E1","kind":"move","col":2}"#).unwrap();
        assert_eq!(events::comment_to_events(&mv).len(), 1);
        let orphan = json::parse(r#"{"kind":"comment","text":"hi"}"#).unwrap();
        assert!(events::comment_to_events(&orphan).is_empty());
    }

    #[test]
    fn append_add_errors_on_a_corrupt_log_rather_than_minting_from_empty() {
        // A malformed log must fail the add — not fold to an empty model and re-mint E1.
        let path =
            std::env::temp_dir().join(format!("faceto-corrupt-{}.jsonl", std::process::id()));
        std::fs::write(&path, "{ this is not json\n").unwrap();
        let ctx = Ctx::new(path.clone());
        let r = ctx.append_add("event", "X".into(), None, None, false);
        let _ = std::fs::remove_file(&path);
        assert!(r.is_err());
    }

    #[test]
    fn add_with_a_blank_label_is_rejected() {
        // The label check fires before any file access, so a bare Ctx is enough.
        let ctx = Ctx::new(std::env::temp_dir().join("faceto-nonexistent.jsonl"));
        let v = json::parse(r#"{"kind":"add","type":"event","text":"   "}"#).unwrap();
        assert_eq!(add_from_comment(&ctx, &v), Err(400));

        // An off-grammar type would mint into a real lane's id space — reject it too.
        let off = json::parse(r#"{"kind":"add","type":"epic","text":"Saga"}"#).unwrap();
        assert_eq!(add_from_comment(&ctx, &off), Err(400));
    }

    #[test]
    fn blank_rename_appends_nothing_but_a_real_one_persists() {
        // Integration over the log-mode POST /comment path: a comment is mapped to events exactly
        // as `handle` does, and only a non-empty block is appended. A blank inline rename must
        // leave the log byte-for-byte unchanged; a real one appends one ElementRenamed that
        // replays into the new label.
        let path = std::env::temp_dir().join(format!("faceto-rn-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());
        let before = std::fs::read_to_string(&path).unwrap();

        // Blank rename → empty event vec → the handler appends nothing.
        let blank = json::parse(r#"{"elemId":"E1","kind":"rename","text":"   "}"#).unwrap();
        let evs = events::comment_to_events(&blank);
        if !evs.is_empty() {
            let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
            ctx.append_line(&ctx.model_path, &block).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            before,
            "a blank inline rename must not touch the log"
        );

        // Real rename → one ElementRenamed → replays to the new label.
        let real = json::parse(r#"{"elemId":"E1","kind":"rename","text":"Reborn"}"#).unwrap();
        let evs = events::comment_to_events(&real);
        let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
        ctx.append_line(&ctx.model_path, &block).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let model = events::replay(&events::parse_log(&text).unwrap());
        let e1 = model.elements.iter().find(|e| e.id == "E1").unwrap();
        assert_eq!(e1.label, "Reborn");
    }
}
