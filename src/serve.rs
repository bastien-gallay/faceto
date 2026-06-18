//! A tiny std-only HTTP server for the live board: serves the page, re-renders the SVG on
//! demand (so an edited model shows without a restart), an in-page diff against a cached
//! baseline, and a click→comment channel appended to `comments.jsonl`.

use crate::{events, json, model, model::Model, render};
use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_MAX: usize = 12;

struct Cache {
    map: HashMap<String, Model>,
    order: VecDeque<String>,
}

struct Ctx {
    model_path: PathBuf,
    comments_path: PathBuf,
    /// When the source is an event log, the model is a projection replayed from it and
    /// comments are appended to the log as events rather than to `comments.jsonl`.
    log_mode: bool,
    cache: Mutex<Cache>,
    /// Serializes appends to the log (H4): concurrent `POST /comment` handlers run on
    /// separate threads, so without this two events could interleave mid-line. Holding
    /// this lock around a single `write_all` makes each appended line atomic.
    appends: Mutex<()>,
}

impl Ctx {
    /// The current board and its short content hash, recorded in the recent-model ring.
    /// In log mode the board is replayed from the event log; otherwise it is the parsed
    /// model file. Either way the version hashes the raw source bytes, so the cached
    /// projection is rebuilt only when a new event (or edit) lands.
    fn current(&self) -> Result<(String, Model), String> {
        let raw = std::fs::read(&self.model_path).map_err(|e| e.to_string())?;
        let version = fnv12(&raw);
        let text = String::from_utf8_lossy(&raw);
        let model = if self.log_mode {
            events::replay(&events::parse_log(&text)?)
        } else {
            model::from_json(&json::parse(&text)?)
        };
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

    /// Append one line to `path`, serialized against all other appends so concurrent
    /// posts cannot interleave (H4). The line and its newline are written in a single
    /// `write_all`, so even under `O_APPEND` no half-line ever lands.
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

    /// Mint a fresh id and append an `ElementAdded` — the answer to H6. The id is derived
    /// from the *current* projection (next free `<PREFIX><N>` for this lane) rather than a
    /// stored counter, so the log stays the only durable record. Both the replay and the
    /// write happen under `appends`, so two concurrent adds can never mint the same id.
    /// Returns the event so the handler can echo the minted id.
    fn append_add(
        &self,
        kind: &str,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
    ) -> std::io::Result<events::Event> {
        let _guard = self.appends.lock().unwrap();
        let raw = std::fs::read(&self.model_path).unwrap_or_default();
        let text = String::from_utf8_lossy(&raw);
        let existing = events::parse_log(&text)
            .map(|e| events::replay(&e))
            .unwrap_or_default();
        let ev = events::Event::ElementAdded {
            id: mint_id(kind, &existing),
            kind: kind.to_string(),
            label,
            col,
            detail,
        };
        Self::write_line(&self.model_path, &events::line(&ev))?;
        Ok(ev)
    }
}

/// The single letter each lane stamps onto a freshly minted id, matching the convention the
/// example boards already use (`actor`→`X`, `aggregate`→`A`, so they don't collide). Kept in
/// sync with the 8-lane grammar in `render.rs`.
fn id_prefix(kind: &str) -> char {
    match kind {
        "actor" => 'X',
        "command" => 'C',
        "aggregate" => 'A',
        "event" => 'E',
        "policy" => 'P',
        "readmodel" => 'R',
        "external" => 'G',
        "hotspot" => 'H',
        // Off-grammar type: fall back to its first letter, upper-cased.
        other => other.chars().next().unwrap_or('Z').to_ascii_uppercase(),
    }
}

/// Next free id for `kind`: `<PREFIX>` followed by one past the highest numeric suffix already
/// used under that prefix (so ids are never renumbered, only added — the model convention).
fn mint_id(kind: &str, model: &Model) -> String {
    let prefix = id_prefix(kind);
    let max = model
        .elements
        .iter()
        .filter_map(|e| {
            let mut chars = e.id.chars();
            (chars.next() == Some(prefix)).then(|| chars.as_str().parse::<u32>().ok())?
        })
        .max()
        .unwrap_or(0);
    format!("{}{}", prefix, max + 1)
}

pub fn serve(model_path: &Path, port: u16) -> Result<(), String> {
    let dir = model_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let log_mode = events::is_log_path(model_path);
    let ctx = Arc::new(Ctx {
        model_path: model_path.to_path_buf(),
        comments_path: dir.join("comments.jsonl"),
        log_mode,
        cache: Mutex::new(Cache {
            map: HashMap::new(),
            order: VecDeque::new(),
        }),
        appends: Mutex::new(()),
    });

    // Validate the model up front so a typo fails loudly, not per-request.
    let (_v, model) = ctx.current()?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    println!(
        "faceto board live → http://127.0.0.1:{}  (Ctrl-C to stop)",
        port
    );
    if log_mode {
        println!(
            "  {} elements · event-sourced · comments append to {}",
            model.elements.len(),
            ctx.model_path.display()
        );
    } else {
        println!(
            "  {} elements · comments → {}",
            model.elements.len(),
            ctx.comments_path.display()
        );
    }

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
                let html = render::render_html(&render::render_svg(&model), &model.title);
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
                let old = base
                    .as_deref()
                    .filter(|b| *b != version)
                    .and_then(|b| ctx.cached(b));
                if let (Some(old), Some(base)) = (old, &base) {
                    let merged =
                        model::diff_models(&old, &model, ("last seen".into(), "now".into()));
                    let svg = render::render_svg(&merged) + "\n";
                    send(
                        &mut out,
                        200,
                        "image/svg+xml",
                        svg.as_bytes(),
                        &[("X-Diff-Base", base.as_str())],
                    )
                } else {
                    let svg = render::render_svg(&model) + "\n";
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
            let body = if ctx.log_mode {
                comments_from_log(&ctx.model_path)
            } else {
                read_comments_json(&ctx.comments_path)
            };
            send(&mut out, 200, "application/json", body.as_bytes(), &[])
        }
        ("GET", "/health") => send(&mut out, 200, "application/json", b"{\"ok\":true}", &[]),
        ("POST", "/comment") => {
            let mut buf = vec![0u8; content_length];
            if reader.read_exact(&mut buf).is_err() {
                return send(&mut out, 400, "application/json", b"{\"ok\":false}", &[]);
            }
            let text = String::from_utf8_lossy(&buf);
            if ctx.log_mode {
                return match json::parse(&text) {
                    Ok(v @ json::Json::Obj(_))
                        if v.get("kind").and_then(|x| x.as_str()) == Some("add") =>
                    {
                        match add_from_comment(&ctx, &v) {
                            Ok(ev) => {
                                println!("  \u{2795} event: {}", events::line(&ev));
                                send(&mut out, 200, "application/json", b"{\"ok\":true}", &[])
                            }
                            Err(code) => {
                                send(&mut out, code, "application/json", b"{\"ok\":false}", &[])
                            }
                        }
                    }
                    Ok(v @ json::Json::Obj(_)) => match comment_to_event(&v) {
                        Some(ev) => {
                            if ctx
                                .append_line(&ctx.model_path, &events::line(&ev))
                                .is_err()
                            {
                                return send(
                                    &mut out,
                                    500,
                                    "application/json",
                                    b"{\"ok\":false}",
                                    &[],
                                );
                            }
                            println!("  \u{1F4AC} event: {}", events::line(&ev));
                            send(&mut out, 200, "application/json", b"{\"ok\":true}", &[])
                        }
                        None => send(&mut out, 400, "application/json", b"{\"ok\":false}", &[]),
                    },
                    _ => send(&mut out, 400, "application/json", b"{\"ok\":false}", &[]),
                };
            }
            match json::parse(&text) {
                Ok(json::Json::Obj(mut o)) => {
                    if !o.iter().any(|(k, _)| k == "status") {
                        o.push(("status".into(), json::Json::Str("open".into())));
                    }
                    o.push(("received".into(), json::Json::Str(now_iso())));
                    let v = json::Json::Obj(o);
                    let line = json::to_string(&v);
                    if ctx.append_line(&ctx.comments_path, &line).is_err() {
                        return send(&mut out, 500, "application/json", b"{\"ok\":false}", &[]);
                    }
                    let kind = v.get("kind").and_then(|x| x.as_str()).unwrap_or("comment");
                    let elem = v.get("elemId").and_then(|x| x.as_str()).unwrap_or("?");
                    let body = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    println!("  \u{1F4AC} [{}] {}: {}", kind, elem, body);
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

/// Handle a `kind:"add"` post: an element-creation command rather than a comment on an
/// existing one. `type` (the lane) is required; `text`→label, optional `col`/`detail`. The
/// server mints the id (H6). Returns the HTTP status to fail with: `400` for a missing/empty
/// type, `500` if the append itself fails.
fn add_from_comment(ctx: &Ctx, v: &json::Json) -> Result<events::Event, u16> {
    let kind = v
        .get("type")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(400u16)?
        .to_string();
    let label = v
        .get("text")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let col = v.get("col").and_then(|x| x.as_f64()).map(|n| n as i64);
    let detail = v
        .get("detail")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    ctx.append_add(&kind, label, col, detail)
        .map_err(|_| 500u16)
}

/// Map a posted comment to the event it persists, in log mode. `move`/`resolve`/`rename`
/// carry structural intent (and fold straight into the projection); anything else is a
/// plain annotation. Returns `None` if the comment names no element.
fn comment_to_event(v: &json::Json) -> Option<events::Event> {
    let id = v.get("elemId").and_then(|x| x.as_str())?.to_string();
    let kind = v.get("kind").and_then(|x| x.as_str()).unwrap_or("comment");
    let text = v
        .get("text")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Some(match kind {
        "move" => events::Event::ElementMoved {
            id,
            col: v.get("col").and_then(|x| x.as_f64()).map(|n| n as i64),
            kind: None,
        },
        "resolve" => events::Event::HotspotResolved {
            id,
            resolution: text,
        },
        "rename" => events::Event::ElementRenamed { id, label: text },
        _ => events::Event::ElementAnnotated { id, text },
    })
}

/// Project the log's *feedback* events (annotations, resolutions, renames) back into the
/// comment shape the client sidebar expects. Structural events (adds, moves, edges) are
/// omitted — they already live in the rendered board.
fn comments_from_log(path: &Path) -> String {
    let log = match events::read_log(path) {
        Ok(e) => e,
        Err(_) => return "[]".to_string(),
    };
    let mut items: Vec<String> = Vec::new();
    for ev in &log {
        let (id, kind, text) = match ev {
            events::Event::ElementAnnotated { id, text } => (id, "comment", text.clone()),
            events::Event::HotspotResolved { id, resolution } => {
                (id, "resolve", resolution.clone())
            }
            events::Event::ElementRenamed { id, label } => (id, "rename", label.clone()),
            _ => continue,
        };
        let obj = json::Json::Obj(vec![
            ("elemId".into(), json::Json::Str(id.clone())),
            ("kind".into(), json::Json::Str(kind.into())),
            ("text".into(), json::Json::Str(text)),
            ("status".into(), json::Json::Str("open".into())),
        ]);
        items.push(json::to_string(&obj));
    }
    format!("[{}]", items.join(","))
}

fn read_comments_json(path: &Path) -> String {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return "[]".to_string(),
    };
    let mut items: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if json::parse(line).is_ok() {
            items.push(line.to_string());
        }
    }
    format!("[{}]", items.join(","))
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

/// Current UTC time as an ISO-8601 string, no chrono. (server-side `received` stamp)
fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
        y, mo, d, h, mi, s
    )
}

/// Days since the Unix epoch → (year, month, day). Howard Hinnant's civil-from-days.
fn civil_from_days(z0: i64) -> (i64, i64, i64) {
    let z = z0 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn civil_from_days_maps_known_epochs() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10957), (2000, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn concurrent_appends_never_interleave() {
        // H4: many threads append to one log through a shared Ctx; every line must land
        // whole and intact, with the expected total count and no torn/merged lines.
        let path = std::env::temp_dir().join(format!("faceto-h4-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let ctx = Arc::new(Ctx {
            model_path: path.clone(),
            comments_path: path.clone(),
            log_mode: true,
            cache: Mutex::new(Cache {
                map: HashMap::new(),
                order: VecDeque::new(),
            }),
            appends: Mutex::new(()),
        });

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
        }
    }

    #[test]
    fn mint_id_picks_next_free_suffix_per_lane() {
        // H6: ids are type-prefixed and never renumbered — minting takes one past the
        // highest suffix already used under that prefix, independently per lane.
        let model = events::replay(&[
            added("E1", "event"),
            added("E3", "event"),
            added("C1", "command"),
        ]);
        assert_eq!(mint_id("event", &model), "E4"); // past the highest E, not filling the E2 gap
        assert_eq!(mint_id("command", &model), "C2");
        assert_eq!(mint_id("hotspot", &model), "H1"); // empty lane starts at 1
        assert_eq!(mint_id("actor", &model), "X1"); // actor stamps X, not A
        assert_eq!(mint_id("aggregate", &model), "A1");
    }

    #[test]
    fn append_add_mints_persists_and_replays() {
        // The minted id round-trips: append_add writes an ElementAdded that replay folds
        // back into a real element, and a second add under the same lane increments.
        let path = std::env::temp_dir().join(format!("faceto-h6-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&added("E1", "event")) + "\n").unwrap();
        let ctx = Ctx {
            model_path: path.clone(),
            comments_path: path.clone(),
            log_mode: true,
            cache: Mutex::new(Cache {
                map: HashMap::new(),
                order: VecDeque::new(),
            }),
            appends: Mutex::new(()),
        };

        let ev = ctx
            .append_add("event", "DayStarted".into(), Some(2), None)
            .unwrap();
        assert!(matches!(&ev, events::Event::ElementAdded { id, .. } if id == "E2"));
        let ev2 = ctx
            .append_add("command", "start".into(), None, None)
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
    fn now_iso_has_iso8601_utc_shape() {
        let s = now_iso();
        assert_eq!(s.len(), "1970-01-01T00:00:00+00:00".len());
        assert!(s.ends_with("+00:00"));
        assert_eq!(s.as_bytes()[4], b'-');
        assert_eq!(s.as_bytes()[10], b'T');
    }
}
