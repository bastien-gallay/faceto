//! faceto — a typed file → a visual workshop board you think through with an LLM.
//!
//!   faceto render  [SOURCE]           write board.svg + index.html next to SOURCE
//!   faceto lint    [SOURCE]           check the board against the ES-grammar rules (warn-only)
//!   faceto serve   [SOURCE] [-p PORT]  serve the live board + comment sidecar
//!   faceto genesis [MODEL]            migrate a model.json into an event-log.jsonl
//!   faceto compact [LOG]              fold a log to a snapshot, bounding replay length
//!
//! SOURCE is a `model.json` or an event log (`*.jsonl` / `*.log`); it defaults to
//! ./model.json. Zero dependencies, offline.

mod events;
mod json;
mod lint;
mod model;
mod render;
mod serve;

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "render" => {
            let model = parse_render(&args[2..]);
            cmd_render(&model);
        }
        "lint" => {
            let model = parse_render(&args[2..]);
            cmd_lint(&model);
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

fn dir_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The event log for a source: `event-log.jsonl` in the source's directory. The one place this
/// convention lives, so the clobber-check path and the write path can never drift apart.
fn log_beside(source: &Path) -> PathBuf {
    dir_of(source).join("event-log.jsonl")
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
    let svg = render::render_svg(&model, &render::View::none());
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

/// Check a board against the ES-grammar rules and print any findings. **Warn-only**: a
/// big-picture board is legitimately incomplete, so findings never fail the command (exit 0
/// always) — the tool nudges, it does not gate. Findings are keyed on the stable `id`; the label
/// is looked up here purely for a readable line.
fn cmd_lint(model_path: &str) {
    let path = Path::new(model_path);
    let model = match load_source(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    let findings = lint::lint(&model);
    if findings.is_empty() {
        println!(
            "no grammar findings — {} elements checked in {}",
            model.elements.len(),
            path.display()
        );
        return;
    }
    let label_of = |id: &str| {
        model
            .elements
            .iter()
            .find(|e| e.id == id)
            .map(|e| format!("{} \"{}\"", e.kind, e.label))
            .unwrap_or_else(|| id.to_string())
    };
    let n = findings.len();
    println!(
        "{n} grammar {} (warn-only — a big-picture board is legitimately incomplete):\n",
        if n == 1 { "finding" } else { "findings" }
    );
    for f in &findings {
        println!("  {} [{}] — {}", label_of(&f.element_id), f.rule, f.message);
    }
}

/// What a genesis migration produced, enough to report it. `write_genesis` returns this so both
/// the explicit `genesis` command and the implicit serve-time migration print the same line.
struct GenesisReport {
    /// The event log written (`event-log.jsonl` beside the model).
    out: PathBuf,
    /// The model migrated from.
    source: PathBuf,
    /// Total events written (the genesis batch).
    total: usize,
}

impl GenesisReport {
    /// One line: how many events the model seeded, and where.
    fn summary(&self) -> String {
        format!(
            "seeded {} events from {} → {}",
            self.total,
            self.source.display(),
            self.out.display()
        )
    }
}

/// Migrate a `model.json` into the genesis batch of an `event-log.jsonl` written alongside it —
/// the bootstrap path into the event-sourced world.
///
/// The write is an **exclusive create** (`create_new`): if a log already exists it fails rather
/// than truncate it, so the "log is append-only truth" invariant is enforced by the write itself —
/// no caller-side guard to forget, and no check-then-write race can clobber a live log. The model
/// is loaded *before* the write, so a malformed model surfaces its own error even when a log is
/// also present.
fn write_genesis(model_path: &Path) -> Result<GenesisReport, String> {
    let model = model::load(model_path)?;
    let out = log_beside(model_path);
    let batch = events::from_model(&model);

    // Exclusive create: refuse to overwrite an existing log (append-only truth), race-free.
    let mut f = match OpenOptions::new().write(true).create_new(true).open(&out) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            return Err(format!(
                "{} already exists — refusing to overwrite",
                out.display()
            ));
        }
        Err(e) => return Err(format!("writing {}: {e}", out.display())),
    };
    f.write_all(events::to_jsonl(&batch).as_bytes())
        .map_err(|e| format!("writing {}: {e}", out.display()))?;

    Ok(GenesisReport {
        out,
        source: model_path.to_path_buf(),
        total: batch.len(),
    })
}

fn cmd_genesis(model_path: &str) {
    // `write_genesis` refuses to clobber intrinsically (exclusive create), so there is no
    // separate exists-check to keep in sync — and loading the model first means a broken model
    // reports *its* error, not "already exists".
    match write_genesis(Path::new(model_path)) {
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
///   * a `model.json` with no sibling log is migrated once (genesis) and the fresh log is served.
///
/// This is what kills legacy mode: `serve` never opens a `model.json` for writing, so no mutation
/// can ever land outside the log.
fn serve_log_path(source: &Path) -> Result<std::path::PathBuf, String> {
    if events::is_log_path(source) {
        return Ok(source.to_path_buf());
    }
    let log = log_beside(source);
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
         \x20 faceto lint    [SOURCE]            check the board against the ES-grammar rules (warn-only)\n\
         \x20 faceto serve   [SOURCE] [-p PORT]  serve the live board + comment sidecar (default :8753)\n\
         \x20 faceto genesis [MODEL]             migrate a model.json into an event-log.jsonl\n\
         \x20 faceto compact [LOG]               fold a log to a snapshot, bounding replay (default event-log.jsonl)\n\
         \x20 faceto help | version\n\
         \n\
         SOURCE is a model.json or an event log (*.jsonl / *.log); it defaults to ./model.json.\n\
         lint reads the board's \"level\" (big-picture | design); a design board adds stricter rules.",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str =
        r#"{"title":"T","elements":[{"id":"E1","type":"event","label":"Hello","col":0}]}"#;

    /// A fresh, empty scratch directory unique to this tag (all tests share one process id, so the
    /// tag keeps parallel tests apart).
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("faceto-mt-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn log_beside_is_the_sibling_event_log() {
        assert_eq!(
            log_beside(Path::new("/a/b/model.json")),
            PathBuf::from("/a/b/event-log.jsonl")
        );
        // A bare filename resolves against ".".
        assert_eq!(
            log_beside(Path::new("model.json")),
            PathBuf::from("./event-log.jsonl")
        );
    }

    #[test]
    fn write_genesis_refuses_to_clobber_an_existing_log() {
        // The exclusive-create write must fail rather than truncate the append-only truth log.
        let dir = scratch("clobber");
        let model = dir.join("model.json");
        std::fs::write(&model, MODEL).unwrap();
        let log = dir.join("event-log.jsonl");
        std::fs::write(&log, "PRIOR\n").unwrap();

        assert!(write_genesis(&model).is_err());
        // The prior log is byte-for-byte intact — not truncated.
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "PRIOR\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn serve_log_path_passes_a_log_through_untouched() {
        // An event-log source is served as-is — no filesystem access, no migration.
        let p = Path::new("/nowhere/event-log.jsonl");
        assert_eq!(serve_log_path(p).unwrap(), p.to_path_buf());
    }

    #[test]
    fn serve_log_path_redirects_a_model_to_its_existing_log() {
        // When a log already sits beside the model, it wins and is returned unchanged.
        let dir = scratch("redirect");
        let model = dir.join("model.json");
        std::fs::write(&model, MODEL).unwrap();
        let log = dir.join("event-log.jsonl");
        std::fs::write(&log, "PRIOR\n").unwrap();

        assert_eq!(serve_log_path(&model).unwrap(), log);
        // The redirect must not rewrite the log.
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "PRIOR\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn serve_log_path_migrates_a_bare_model() {
        // No sibling log → genesis runs once and the fresh log is what gets served.
        let dir = scratch("migrate");
        let model = dir.join("model.json");
        std::fs::write(&model, MODEL).unwrap();

        let served = serve_log_path(&model).unwrap();
        assert_eq!(served, dir.join("event-log.jsonl"));
        assert!(served.exists());
        // It replays back to the one-element board.
        assert_eq!(events::load(&served).unwrap().elements.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
