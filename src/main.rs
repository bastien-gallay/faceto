//! faceto — a typed file → a visual workshop board you think through with an LLM.
//!
//!   faceto render  [SOURCE]           write board.svg + index.html next to SOURCE
//!   faceto serve   [SOURCE] [-p PORT]  serve the live board + comment sidecar
//!   faceto genesis [MODEL]            migrate a model.json into an event-log.jsonl
//!   faceto compact [LOG]              fold a log to a snapshot, bounding replay length
//!
//! SOURCE is a `model.json` or an event log (`*.jsonl` / `*.log`); it defaults to
//! ./model.json. Zero dependencies, offline.

mod events;
mod json;
mod model;
mod render;
mod serve;

use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "render" => {
            let model = parse_render(&args[2..]);
            cmd_render(&model);
        }
        "serve" => {
            let (model, port) = parse_serve(&args[2..]);
            if let Err(e) = serve::serve(Path::new(&model), port) {
                eprintln!("error: {e}");
                exit(1);
            }
        }
        "genesis" => {
            let model = args.get(2).map(String::as_str).unwrap_or("model.json");
            cmd_genesis(model);
        }
        "compact" => {
            let log = args.get(2).map(String::as_str).unwrap_or("event-log.jsonl");
            cmd_compact(log);
        }
        "help" | "-h" | "--help" => print_help(),
        "version" | "-V" | "--version" => println!("faceto {}", env!("CARGO_PKG_VERSION")),
        other => {
            eprintln!("unknown command: {other}\n");
            print_help();
            exit(2);
        }
    }
}

/// Load a board from either a legacy `model.json` or an event log, chosen by extension.
fn load_source(path: &Path) -> Result<model::Model, String> {
    if events::is_log_path(path) {
        events::load(path)
    } else {
        model::load(path)
    }
}

fn dir_of(path: &Path) -> std::path::PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn cmd_render(model_path: &str) {
    let path = Path::new(model_path);
    let model = match load_source(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    let svg = render::render_svg(&model);
    let html = render::render_html(&svg, &model.title);
    let dir = dir_of(path);
    if let Err(e) = std::fs::write(dir.join("board.svg"), format!("{svg}\n")) {
        eprintln!("error writing board.svg: {e}");
        exit(1);
    }
    if let Err(e) = std::fs::write(dir.join("index.html"), html) {
        eprintln!("error writing index.html: {e}");
        exit(1);
    }
    println!(
        "rendered {} elements → {} + {}",
        model.elements.len(),
        dir.join("board.svg").display(),
        dir.join("index.html").display()
    );
}

/// Migrate a legacy `model.json` into the genesis batch of an `event-log.jsonl` written
/// alongside it — the bootstrap path into the event-sourced world. A sibling `comments.jsonl`
/// (the legacy feedback inbox) is folded in too (H5): its annotations/resolutions/renames/moves
/// land as events *after* the genesis batch, which minted the ids they reference — so the inbox
/// is preserved on the board instead of stranded. Refuses to clobber an existing log.
fn cmd_genesis(model_path: &str) {
    let path = Path::new(model_path);
    let model = match model::load(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    let dir = dir_of(path);
    let out = dir.join("event-log.jsonl");
    if out.exists() {
        eprintln!(
            "error: {} already exists — refusing to overwrite",
            out.display()
        );
        exit(1);
    }
    let mut batch = events::from_model(&model);
    let genesis_len = batch.len();

    // Fold a sibling comments.jsonl, if one exists, into the same migration. Reading it is
    // best-effort: a missing file is the common case (nothing to fold), and the inbox itself
    // tolerates stray lines (see `events::from_comments`).
    let comments_path = dir.join("comments.jsonl");
    let (folded, skipped) = std::fs::read_to_string(&comments_path)
        .ok()
        .map(|text| events::from_comments(&text))
        .unwrap_or_default();
    let folded_len = folded.len();
    batch.extend(folded);

    if let Err(e) = std::fs::write(&out, events::to_jsonl(&batch)) {
        eprintln!("error writing {}: {e}", out.display());
        exit(1);
    }
    // One message, with an optional inbox clause: how many comment lines folded in, and how many
    // could not be migrated (reported, never dropped silently).
    let inbox = if folded_len > 0 || skipped > 0 {
        let mut clause = format!(" ({genesis_len} genesis + {folded_len} folded");
        if skipped > 0 {
            clause.push_str(&format!(", {skipped} not migrated"));
        }
        clause.push_str(&format!(" from {})", comments_path.display()));
        clause
    } else {
        String::new()
    };
    println!(
        "seeded {} events from {}{} → {}",
        batch.len(),
        path.display(),
        inbox,
        out.display()
    );
}

/// Fold an event log to a minimal snapshot — a `LogCompacted` marker plus the genesis batch of
/// the current projection — bounding how long replay has to run (H1's snapshot escape hatch).
/// The board is preserved exactly; compaction is lossy only in dropping comment *history*, so
/// the prior log is copied to `<log>.bak` before the source of truth is overwritten in place.
fn cmd_compact(log_path: &str) {
    let path = Path::new(log_path);
    if !events::is_log_path(path) {
        eprintln!(
            "error: compact operates on an event log (*.jsonl / *.log), not {}",
            path.display()
        );
        exit(1);
    }
    let original = match events::read_log(path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    let folded = events::compact(&original);

    // Preserve the prior log alongside before overwriting the truth file.
    let mut bak = path.as_os_str().to_owned();
    bak.push(".bak");
    let bak = std::path::PathBuf::from(bak);
    if let Err(e) = std::fs::copy(path, &bak) {
        eprintln!(
            "error backing up {} → {}: {e}",
            path.display(),
            bak.display()
        );
        exit(1);
    }
    if let Err(e) = std::fs::write(path, events::to_jsonl(&folded)) {
        eprintln!("error writing {}: {e}", path.display());
        exit(1);
    }
    println!(
        "compacted {} events → {} (1 marker + {} genesis) in {} · prior log saved to {}",
        original.len(),
        folded.len(),
        folded.len() - 1,
        path.display(),
        bak.display()
    );
}

/// A flag this build no longer knows fails loudly by name — never silently misread as the source
/// path (`faceto render model.json --pack rows` must not try to open a file called `rows`). The
/// `--pack` modes went with F-2d-placement: each element stores its own sub-position now.
fn reject_flag(arg: &str) {
    if arg.starts_with('-') {
        eprintln!(
            "unknown flag: {arg}\n(the --pack modes were removed — each element now stores its \
             own position; see `faceto help`)"
        );
        exit(2);
    }
}

/// `render [SOURCE]`. The positional is the source; anything flag-shaped is rejected loudly.
fn parse_render(args: &[String]) -> String {
    let mut model = "model.json".to_string();
    for arg in args {
        reject_flag(arg);
        model = arg.clone();
    }
    model
}

fn parse_serve(args: &[String]) -> (String, u16) {
    let mut model = "model.json".to_string();
    let mut port: u16 = 8753;
    let mut i = 0;
    while i < args.len() {
        if matches!(args[i].as_str(), "-p" | "--port") {
            if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                port = v;
            }
            i += 2;
        } else {
            reject_flag(&args[i]);
            model = args[i].clone();
            i += 1;
        }
    }
    (model, port)
}

fn print_help() {
    println!(
        "faceto {} — a typed file → a visual workshop board you think through with an LLM\n\
         \n\
         USAGE:\n\
         \x20 faceto render  [SOURCE]            write board.svg + index.html next to SOURCE\n\
         \x20 faceto serve   [SOURCE] [-p PORT]  serve the live board + comment sidecar (default :8753)\n\
         \x20 faceto genesis [MODEL]             migrate a model.json into an event-log.jsonl\n\
         \x20 faceto compact [LOG]               fold a log to a snapshot, bounding replay (default event-log.jsonl)\n\
         \x20 faceto help | version\n\
         \n\
         SOURCE is a model.json or an event log (*.jsonl / *.log); it defaults to ./model.json.",
        env!("CARGO_PKG_VERSION")
    );
}
