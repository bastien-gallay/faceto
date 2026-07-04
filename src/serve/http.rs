//! The HTTP wire layer: read a capped request line/headers, route the request ([`handle`]),
//! and write the response ([`send`]). The only module that speaks the protocol.

use super::comment::{add_from_comment, add_region_from_comment, split_region_from_comment};
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
/// view params), the `/model-version` poll, the `/comments` sidebar, `/health`, and the
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

/// `GET /` (and `/index.html`) — the board page: current projection → SVG → HTML.
fn route_page(out: &mut TcpStream, ctx: &Ctx) -> std::io::Result<()> {
    match ctx.current() {
        Ok((_v, model)) => {
            let svg = render::render_svg(&model, &render::View::none());
            let html = render::render_html(&svg, &model.title);
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
            let base = query_get(query, "base");
            let view = render::View {
                collapsed: parse_collapse(query),
            };
            let old = base
                .as_deref()
                .filter(|b| *b != version)
                .and_then(|b| ctx.cached(b));
            if let (Some(old), Some(base)) = (old, &base) {
                let merged = model::diff_models(&old, &model, ("last seen".into(), "now".into()));
                let svg = render::render_svg(&merged, &view) + "\n";
                send(
                    out,
                    200,
                    "image/svg+xml",
                    svg.as_bytes(),
                    &[("X-Diff-Base", base.as_str())],
                )
            } else {
                let svg = render::render_svg(&model, &view) + "\n";
                send(out, 200, "image/svg+xml", svg.as_bytes(), &[])
            }
        }
        Err(e) => send(out, 500, "text/plain; charset=utf-8", e.as_bytes(), &[]),
    }
}

/// `GET /model-version` — the log's content hash, the value the ~1 Hz client poll reads (replay-free).
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

/// `POST /comment` — append the event(s) a posted comment implies. `add` / `region-add` /
/// `phase-split` each need a server-minted id (H6 / review #3 / F-region-frontiers) and take the
/// mint-and-respond path; every other kind folds through `comment_to_events` and is appended as one
/// atomic block (a multi-event action lands as consecutive lines under a single append).
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
    match json::parse(&text) {
        Ok(v @ json::Json::Obj(_))
            if matches!(
                v.get_str("kind"),
                Some("add") | Some("region-add") | Some("phase-split")
            ) =>
        {
            // The three server-minted kinds share the mint-and-respond path — only which append fn
            // mints differs.
            let result = match v.get_str("kind") {
                Some("add") => add_from_comment(ctx, &v),
                Some("region-add") => add_region_from_comment(ctx, &v),
                _ => split_region_from_comment(ctx, &v),
            };
            match result {
                Ok(ev) => {
                    println!("  \u{2795} event: {}", events::line(&ev));
                    send(out, 200, "application/json", b"{\"ok\":true}", &[])
                }
                Err(code) => send(out, code, "application/json", b"{\"ok\":false}", &[]),
            }
        }
        Ok(v @ json::Json::Obj(_)) => {
            let evs = events::comment_to_events(&v);
            if evs.is_empty() {
                return send(out, 400, "application/json", b"{\"ok\":false}", &[]);
            }
            let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
            if ctx.append_line(&ctx.model_path, &block).is_err() {
                return send(out, 500, "application/json", b"{\"ok\":false}", &[]);
            }
            println!("  \u{1F4AC} event: {}", block);
            send(out, 200, "application/json", b"{\"ok\":true}", &[])
        }
        _ => send(out, 400, "application/json", b"{\"ok\":false}", &[]),
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
}
