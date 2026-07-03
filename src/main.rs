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
            let (source, port) = parse_serve(&args[2..]);
            let log = match serve_log_path(Path::new(&source)) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            };
            if let Err(e) = serve::serve(&log, port) {
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

/// Load a board (read-only) from either a `model.json` bootstrap form or an event log, chosen by
/// extension. Used by `render`, which never mutates; `serve` always goes through the log
/// (`serve_log_path`), since serving mutates and the log is the truth.
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

/// What a genesis migration produced, enough to report it. `write_genesis` returns this so both
/// the explicit `genesis` command and the implicit serve-time migration print the same line.
struct GenesisReport {
    /// The event log written (`event-log.jsonl` beside the model).
    out: std::path::PathBuf,
    /// The model migrated from.
    source: std::path::PathBuf,
    /// The sibling comments inbox that was folded in (whether or not it existed).
    comments_path: std::path::PathBuf,
    /// Total events written: `genesis_len + folded_len`.
    total: usize,
    /// Events from the model itself (the genesis batch).
    genesis_len: usize,
    /// Comment lines folded onto the batch (H5).
    folded_len: usize,
    /// Comment lines that could not be migrated — reported, never dropped silently.
    skipped: usize,
}

impl GenesisReport {
    /// One line, with an optional inbox clause: how many comment lines folded in, and how many
    /// could not be migrated.
    fn summary(&self) -> String {
        let inbox = if self.folded_len > 0 || self.skipped > 0 {
            let mut clause = format!(
                " ({} genesis + {} folded",
                self.genesis_len, self.folded_len
            );
            if self.skipped > 0 {
                clause.push_str(&format!(", {} not migrated", self.skipped));
            }
            clause.push_str(&format!(" from {})", self.comments_path.display()));
            clause
        } else {
            String::new()
        };
        format!(
            "seeded {} events from {}{} → {}",
            self.total,
            self.source.display(),
            inbox,
            self.out.display()
        )
    }
}

/// Migrate a `model.json` into the genesis batch of an `event-log.jsonl` written alongside it —
/// the bootstrap path into the event-sourced world. A sibling `comments.jsonl` (the legacy
/// feedback inbox) is folded in too (H5): its annotations/resolutions/renames/moves land as events
/// *after* the genesis batch, which minted the ids they reference — so the inbox is preserved on
/// the board instead of stranded. The caller owns the "refuse to clobber an existing log" policy:
/// `genesis` refuses, serve-time migration only reaches here when no log exists yet.
fn write_genesis(model_path: &Path) -> Result<GenesisReport, String> {
    let model = model::load(model_path)?;
    let dir = dir_of(model_path);
    let out = dir.join("event-log.jsonl");
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

    std::fs::write(&out, events::to_jsonl(&batch))
        .map_err(|e| format!("writing {}: {e}", out.display()))?;
    Ok(GenesisReport {
        out,
        source: model_path.to_path_buf(),
        comments_path,
        total: batch.len(),
        genesis_len,
        folded_len,
        skipped,
    })
}

fn cmd_genesis(model_path: &str) {
    let path = Path::new(model_path);
    let out = dir_of(path).join("event-log.jsonl");
    if out.exists() {
        eprintln!(
            "error: {} already exists — refusing to overwrite",
            out.display()
        );
        exit(1);
    }
    match write_genesis(path) {
        Ok(report) => println!("{}", report.summary()),
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    }
}

/// Resolve the source a `serve` command must mutate to an event log, auto-running genesis for a
/// bare `model.json` (F-auto-genesis). Serving mutates, and every mutation must land in the log —
/// the truth — never in the derived model, so:
///
///   * an event log is served as-is;
///   * a `model.json` beside an existing `event-log.jsonl` redirects to that log (the log already
///     won; the model is a derived/bootstrap form, so it is ignored once a log exists);
///   * a `model.json` with no sibling log is migrated once (genesis, folding a sibling
///     `comments.jsonl`) and the fresh log is served.
///
/// This is what kills legacy mode: `serve` never opens a `model.json` for writing, so the old
/// `comments.jsonl` append path — and its "structural gestures stored as dead comments" defect —
/// cannot be reached.
fn serve_log_path(source: &Path) -> Result<std::path::PathBuf, String> {
    if events::is_log_path(source) {
        return Ok(source.to_path_buf());
    }
    let log = dir_of(source).join("event-log.jsonl");
    if log.exists() {
        println!(
            "{} exists beside {} — serving the log (it is the truth; the model is derived)",
            log.display(),
            source.display()
        );
        return Ok(log);
    }
    let report = write_genesis(source)?;
    println!("{}", report.summary());
    Ok(report.out)
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
