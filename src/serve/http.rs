//! The HTTP wire layer: read a capped request line/headers, route the request ([`handle`]),
//! and write the response ([`send`]). The only module that speaks the protocol.

use super::mint::append_mint;
use super::sidebar::comments_body;
use super::{Ctx, MAX_BODY, MAX_HEADERS, MAX_HEADER_LINE};
use crate::{events, json, model, render};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

/// Read one line (up to and including `\n`) into `buf`, but refuse to grow past `cap` bytes —
/// unlike `BufRead::read_line`, which buffers until a newline or EOF and so has no ceiling. Returns
/// the byte count (0 = clean EOF before any byte), or `Err(InvalidData)` once `cap` is exceeded, so
/// a client that never sends `\r\n` can't drive an unbounded allocation. Bytes are pushed lossily
/// (headers are ASCII); reads go through the same `BufReader`, so leftover bytes remain for the body.
fn read_line_capped<R: BufRead>(
    reader: &mut R,
    buf: &mut String,
    cap: usize,
) -> std::io::Result<usize> {
    let mut raw = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        if reader.read(&mut byte)? == 0 {
            break;
        }
        raw.push(byte[0]);
        if raw.len() > cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request header line exceeds cap",
            ));
        }
        if byte[0] == b'\n' {
            break;
        }
    }
    buf.push_str(&String::from_utf8_lossy(&raw));
    Ok(raw.len())
}

/// Serve one HTTP connection end to end: read the (capped) request line and headers, then route
/// `(method, path)` to a response. One thread runs this per accepted connection. The routes are the
/// board page (`GET /`), the re-rendered SVG (`GET /board.svg`, with optional `?base=`/`?collapse=`
/// view params), the `/model-version` check, the `/comments` sidebar, `/health`, and the
/// `POST /comment` append. Over-long request lines / header floods short-circuit to `431` before
/// routing; every other failure maps to a `4xx`/`5xx` from the route handler.
pub(crate) fn handle(stream: TcpStream, ctx: Arc<Ctx>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut out = stream;

    let mut req_line = String::new();
    match read_line_capped(&mut reader, &mut req_line, MAX_HEADER_LINE) {
        Ok(0) => return Ok(()),
        Ok(_) => {}
        // An over-long request line (or any read error on it) can't be routed — refuse it.
        Err(_) => {
            return send(
                &mut out,
                431,
                "text/plain; charset=utf-8",
                b"header too large",
                &[],
            )
        }
    }
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    let mut content_length = 0usize;
    let mut header_count = 0usize;
    loop {
        header_count += 1;
        if header_count > MAX_HEADERS {
            return send(
                &mut out,
                431,
                "text/plain; charset=utf-8",
                b"too many headers",
                &[],
            );
        }
        let mut line = String::new();
        match read_line_capped(&mut reader, &mut line, MAX_HEADER_LINE) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => {
                return send(
                    &mut out,
                    431,
                    "text/plain; charset=utf-8",
                    b"header too large",
                    &[],
                )
            }
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
        ("GET", "/") | ("GET", "/index.html") => route_page(&mut out, &ctx),
        ("GET", "/board.svg") => route_board_svg(&mut out, &ctx, query),
        ("GET", "/model-version") => route_model_version(&mut out, &ctx),
        ("GET", "/comments") => route_comments(&mut out, &ctx),
        ("GET", "/health") => send(&mut out, 200, "application/json", b"{\"ok\":true}", &[]),
        ("POST", "/comment") => route_post_comment(&mut out, &mut reader, &ctx, content_length),
        _ => send(
            &mut out,
            404,
            "text/plain; charset=utf-8",
            b"not found",
            &[],
        ),
    }
}

/// Apply a launch-time `--base` overlay (F-variants): when a fixed baseline board is set, return the
/// union board diffed against it *plus* the overlay that says what changed (the pair `render_svg`
/// takes); with no baseline it is the identity — the plain current board and no overlay at all.
/// Returns a `Cow` so the common no-baseline path (every page load / refresh of an ordinary live
/// board) *borrows* `current` instead of deep-cloning a whole `Model`. Pure `(&baseline, &current)`,
/// so it unit-tests without a running server. The single seam both `route_page` and
/// `route_board_svg` funnel through, so the first paint and every later fetch agree.
fn overlay<'a>(
    baseline: &'a Option<(model::Model, (String, String))>,
    current: &'a model::Model,
) -> (std::borrow::Cow<'a, model::Model>, Option<render::Overlay>) {
    match baseline {
        Some((base, meta)) => {
            let (board, diff) = render::diff_boards(base, current, meta.clone());
            (std::borrow::Cow::Owned(board), Some(diff))
        }
        None => (std::borrow::Cow::Borrowed(current), None),
    }
}

/// `GET /` (and `/index.html`) — the board page: current projection → SVG → HTML. Under `--base` the
/// projection is the diff overlay, so the first paint already shows the variant divergence.
fn route_page(out: &mut TcpStream, ctx: &Ctx) -> std::io::Result<()> {
    match ctx.current() {
        Ok((_v, model)) => {
            let (board, diff) = overlay(&ctx.baseline, &model);
            let svg = render::render_svg(&board, &render::View::none(), diff.as_ref());
            // A launch-time `--base` makes this a diff overlay → render the page read-only so no
            // edit gesture lands on the ghost-carrying diff DOM.
            let html = render::render_html(&svg, &board.title, ctx.baseline.is_some());
            send(out, 200, "text/html; charset=utf-8", html.as_bytes(), &[])
        }
        Err(e) => send(out, 500, "text/plain; charset=utf-8", e.as_bytes(), &[]),
    }
}

/// `GET /board.svg` — the re-rendered board. With a cached `?base=` baseline it is a diff overlay
/// against that version; the collapsed-region `?collapse=` view (a per-viewer reading lens, never
/// persisted — F-region-collapse) folds the current board *and* the baseline the same way, so the
/// overlay lines up column-for-column.
fn route_board_svg(out: &mut TcpStream, ctx: &Ctx, query: &str) -> std::io::Result<()> {
    match ctx.current() {
        Ok((version, model)) => {
            let view = render::View {
                collapsed: parse_collapse(query),
            };
            // A launch-time `--base` fixes the overlay baseline for the whole session and takes
            // precedence over the client's `?base=` ring diff: in variant-review mode the baseline
            // *is* the given file, not "since you last looked". The collapse `View` still composes
            // (diff → merged → render_svg(merged, view)), exactly as the ring path below does.
            if ctx.baseline.is_some() {
                let (merged, diff) = overlay(&ctx.baseline, &model);
                let svg = render::render_svg(&merged, &view, diff.as_ref()) + "\n";
                return send(out, 200, "image/svg+xml", svg.as_bytes(), &[]);
            }
            let base = query_get(query, "base");
            let old = base
                .as_deref()
                .filter(|b| *b != version)
                .and_then(|b| ctx.cached(b));
            if let (Some(old), Some(base)) = (old, &base) {
                let (merged, diff) =
                    render::diff_boards(&old, &model, ("last seen".into(), "now".into()));
                let svg = render::render_svg(&merged, &view, Some(&diff)) + "\n";
                send(
                    out,
                    200,
                    "image/svg+xml",
                    svg.as_bytes(),
                    &[("X-Diff-Base", base.as_str())],
                )
            } else {
                let svg = render::render_svg(&model, &view, None) + "\n";
                send(out, 200, "image/svg+xml", svg.as_bytes(), &[])
            }
        }
        Err(e) => send(out, 500, "text/plain; charset=utf-8", e.as_bytes(), &[]),
    }
}

/// `GET /model-version` — the log's content hash, the value the client reads to tell a stale board
/// from a fresh one (replay-free); fetched on load and on Reload, not on a timer.
fn route_model_version(out: &mut TcpStream, ctx: &Ctx) -> std::io::Result<()> {
    match ctx.version() {
        Ok(version) => {
            let body = format!("{{\"version\":\"{}\"}}", version);
            send(out, 200, "application/json", body.as_bytes(), &[])
        }
        Err(e) => send(out, 500, "text/plain; charset=utf-8", e.as_bytes(), &[]),
    }
}

/// `GET /comments` — the sidebar payload (stored feedback + live lint findings).
fn route_comments(out: &mut TcpStream, ctx: &Ctx) -> std::io::Result<()> {
    let body = comments_body(ctx);
    send(out, 200, "application/json", body.as_bytes(), &[])
}

/// `POST /comment` — append the event(s) a posted command implies. The body is read into a typed
/// `Command` **once** (`events::parse_command`), and the two arms of that type are the two paths
/// this route has always had: a `Mint` needs a server-assigned id and goes to `append_mint`; a
/// `Fold` maps to its events and is appended as one atomic block (a multi-event action lands as
/// consecutive lines under a single append). A body that names no command is answered `400` and
/// the reason is printed, rather than being stored as a comment nobody asked for. The one guard a
/// parse cannot make — whether a split column still falls inside its phase — is judged under the
/// lock, and it is a client error too: `400`, not `500`, so an agent does not retry it.
fn route_post_comment(
    out: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    ctx: &Ctx,
    content_length: usize,
) -> std::io::Result<()> {
    // Cap the body before allocating: `content_length` is attacker-controlled (a header), and a
    // comment/structural op is tiny, so a huge value is a bug or a DoS, never real.
    if content_length > MAX_BODY {
        return send(out, 413, "application/json", b"{\"ok\":false}", &[]);
    }
    let mut buf = vec![0u8; content_length];
    if reader.read_exact(&mut buf).is_err() {
        return send(out, 400, "application/json", b"{\"ok\":false}", &[]);
    }
    let text = String::from_utf8_lossy(&buf);
    let Ok(v @ json::Json::Obj(_)) = json::parse(&text) else {
        return send(out, 400, "application/json", b"{\"ok\":false}", &[]);
    };
    match events::parse_command(&v) {
        Ok(events::Command::Mint(cmd)) => match append_mint(ctx, &cmd) {
            Ok(ev) => {
                println!("  \u{2795} event: {}", events::line(&ev));
                send(out, 200, "application/json", b"{\"ok\":true}", &[])
            }
            Err(why) => {
                let code = match &why {
                    super::Refusal::Board(_) => 400,
                    super::Refusal::Server(_) => 500,
                };
                let (super::Refusal::Board(msg) | super::Refusal::Server(msg)) = &why;
                println!("  \u{26A0} refused: {msg}");
                send(out, code, "application/json", b"{\"ok\":false}", &[])
            }
        },
        Ok(events::Command::Fold(cmd)) => {
            let evs = events::fold_to_events(&cmd);
            let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
            if ctx.append_line(&ctx.model_path, &block).is_err() {
                return send(out, 500, "application/json", b"{\"ok\":false}", &[]);
            }
            println!("  \u{1F4AC} event: {}", block);
            send(out, 200, "application/json", b"{\"ok\":true}", &[])
        }
        Err(why) => {
            println!("  \u{26A0} refused: {why}");
            send(out, 400, "application/json", b"{\"ok\":false}", &[])
        }
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
                return Some(url_decode(v));
            }
        }
    }
    None
}

/// Percent-decode a URL query value (`%XX` → byte, `+` → space) — std-only, no crate. The client
/// builds board fetches with `URLSearchParams`, which percent-encodes reserved characters: the
/// comma separating `?collapse=` ids becomes `%2C`, so without decoding a multi-region fold would
/// arrive as a single unmatchable id. A malformed escape (`%` not followed by two hex digits) is
/// left verbatim rather than dropped, so a stray `%` never eats following bytes.
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => match (
                i.checked_add(1)
                    .and_then(|j| bytes.get(j))
                    .map(|b| *b as char),
                i.checked_add(2)
                    .and_then(|j| bytes.get(j))
                    .map(|b| *b as char),
            ) {
                (Some(hi), Some(lo)) => match (hi.to_digit(16), lo.to_digit(16)) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                },
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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
        // The browser builds the query with `URLSearchParams`, which percent-encodes the comma
        // that separates ids (`,` → `%2C`). The server must decode it, or a two-region fold
        // arrives as one bogus id (`K2%2CK5`) that matches no phase and the whole lens silently
        // un-folds (regression: multi-region collapse was broken end-to-end).
        assert_eq!(parse_collapse("collapse=K2%2CK5"), vec!["K2", "K5"]);
        assert_eq!(
            parse_collapse("base=abc&collapse=K2%2CK5%2CK9"),
            vec!["K2", "K5", "K9"]
        );
    }

    #[test]
    fn overlay_is_the_identity_without_a_baseline_and_diffs_with_one() {
        use crate::serve::testutil::model_of;
        let current = model_of(
            r#"{"title":"T","elements":[
                {"id":"E1","type":"event","label":"Order placed","col":0},
                {"id":"E2","type":"event","label":"Order shipped","col":1}
            ]}"#,
        );

        // No baseline → the plain current board, untouched, and no overlay at all.
        let (plain, none) = overlay(&None, &current);
        assert!(none.is_none());
        assert_eq!(*plain, current);

        // With a baseline (E2 is new vs the base) → a diff overlay: E2 is tagged `added`.
        let base = model_of(
            r#"{"title":"T","elements":[
                {"id":"E1","type":"event","label":"Order placed","col":0}
            ]}"#,
        );
        let baseline = Some((base, ("before".to_string(), "after".to_string())));
        let (merged, diff) = overlay(&baseline, &current);
        let diff = diff.expect("a baseline produces an overlay");
        assert_eq!(diff.meta, ("before".to_string(), "after".to_string()));
        assert_eq!(diff.count(render::Tone::Added), 1);
        assert!(merged.elements.iter().any(|e| e.id == "E2"));
    }

    #[test]
    fn query_get_percent_decodes_the_value() {
        assert_eq!(query_get("k=a%2Cb", "k").as_deref(), Some("a,b"));
        assert_eq!(query_get("k=a+b", "k").as_deref(), Some("a b"));
        // A malformed escape is left verbatim rather than dropped.
        assert_eq!(query_get("k=100%", "k").as_deref(), Some("100%"));
        assert_eq!(query_get("k=%zz", "k").as_deref(), Some("%zz"));
    }
}
