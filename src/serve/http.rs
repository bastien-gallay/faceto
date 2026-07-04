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
        ("GET", "/model-version") => match ctx.version() {
            Ok(version) => {
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
pub(crate) fn parse_collapse(query: &str) -> Vec<String> {
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
