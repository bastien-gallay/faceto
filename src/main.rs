//! faceto — a typed file → a visual workshop board you think through with an LLM.
//!
//!   faceto render  [SOURCE] [--base OTHER]  write <name>.svg + <name>.html (a diff overlay vs OTHER)
//!   faceto lint    [SOURCE]           check the board against the ES-grammar rules (warn-only)
//!   faceto serve   [SOURCE] [-p PORT] [--base OTHER]  serve the live board + comment sidecar
//!   faceto export  [SOURCE] [--format mermaid|context]  print the board (mermaid, or a context pack) to stdout
//!   faceto extract [SOURCE] --region ID | --focus ID [--hops N] | --type KIND  carve a sub-board out
//!   faceto genesis [MODEL]            migrate a model.json into a <name>.event-log.jsonl
//!   faceto compact [LOG]              fold a log to a snapshot, bounding replay length
//!
//! SOURCE is a `model.json` or an event log (`*.jsonl` / `*.log`); it defaults to
//! ./model.json. Zero dependencies, offline.

mod events;
mod extract;
mod json;
mod lint;
mod model;
mod render;
mod scene;
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
            let render_args = parse_render(&args[2..]);
            cmd_render(&render_args);
        }
        "lint" => {
            // lint takes only a source (no `--base` diff); reuse the positional parse and ignore
            // any baseline — `--base` is meaningless for a grammar check of one board.
            let model = parse_render(&args[2..]).source;
            cmd_lint(&model);
        }
        "serve" => {
            let (source, port, base) = parse_serve(&args[2..]);
            let log = match serve_log_path(Path::new(&source)) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            };
            // A launch-time `--base` fixes the overlay baseline for the whole session. Loaded
            // read-only via `load_source` (never genesis'd or mutated) and paired with its stem +
            // the served board's stem for the on-board legend/tooltip labels ("was" → "now").
            let baseline = match base {
                Some(b) => {
                    let bp = Path::new(&b);
                    match load_source(bp) {
                        Ok(m) => {
                            // Warn on an empty/mis-suffixed baseline, same as `render --base` warns
                            // both sides — otherwise a wrong `--base` file silently reads every
                            // current element as "added".
                            warn_if_empty(&m, bp);
                            Some((m, (output_stem(bp), output_stem(&log))))
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            exit(1);
                        }
                    }
                }
                None => None,
            };
            if let Err(e) = serve::serve(&log, port, baseline) {
                eprintln!("error: {e}");
                exit(1);
            }
        }
        "export" => {
            let (source, format) = parse_export(&args[2..]);
            cmd_export(&source, format);
        }
        "extract" => {
            let (source, selector) = parse_extract(&args[2..]);
            cmd_extract(&source, &selector);
        }
        "genesis" => {
            let model = args.get(2).map(String::as_str).unwrap_or("model.json");
            cmd_genesis(model);
        }
        "compact" => {
            // No single canonical log name anymore: default to the log of the default model
            // (`model.json` → `model.event-log.jsonl`), mirroring `genesis`'s default, rather than
            // the retired bare `event-log.jsonl` that nothing produces.
            let default_log = log_beside(Path::new("model.json"));
            match args.get(2) {
                Some(log) => cmd_compact(log),
                None => cmd_compact(&default_log.to_string_lossy()),
            }
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

/// The suffix that turns a board name into its event-log filename. `log_beside` *appends* it and
/// `output_stem` *strips* it — the two halves of one round-trip (a model and its log must resolve
/// to the same stem), so they share this single constant rather than two literals that could drift.
const EVENT_LOG_SUFFIX: &str = ".event-log.jsonl";

/// The event log for a source: `<stem>.event-log.jsonl` in the source's directory, where `<stem>`
/// is the source's board name (`orders.model.json` → `orders.event-log.jsonl`). Deriving the log
/// name from the basename is what lets sibling boards live in one directory: each model owns its
/// own log instead of every model contending for a single shared `event-log.jsonl` (which would
/// make `serve b.json` silently serve a log genesis'd from `a.json`). The one place this convention
/// lives, so the clobber-check path and the write path can never drift apart.
fn log_beside(source: &Path) -> PathBuf {
    dir_of(source).join(format!("{}{EVENT_LOG_SUFFIX}", output_stem(source)))
}

/// Warn (never fail) when a source yields an empty board. `model::from_json` is lenient — it
/// accepts any JSON object — so a file that is not a board, or a `.jsonl` mis-suffixed as a model,
/// replays to zero elements and would otherwise render a silent blank board. Warn-only: an
/// intentionally empty board is legal, so this nudges rather than gates.
fn warn_if_empty(model: &model::Model, source: &Path) {
    if model.elements.is_empty() {
        eprintln!(
            "warning: {} yielded an empty board (0 elements) — wrong source, or a mis-suffixed \
             file? (rendering it anyway)",
            source.display()
        );
    }
}

/// The board name derived from a source filename — the join key for every file that belongs to one
/// board so siblings in a directory coexist instead of clobbering a shared name. A model and *its*
/// log resolve to the *same* stem, so `render` of either writes the same `<stem>.svg`/`.html`:
/// `orders.model.json` → `orders`, `orders.event-log.jsonl` → `orders`, and a legacy bare
/// `event-log.jsonl` → `event-log`; `foo.json` → `foo`. Falls back to `board` for a path with no
/// usable stem.
///
/// Only the *compound* suffixes are peeled explicitly; a single trailing extension (`.json`,
/// `.jsonl`, `.txt`, …) is left to `file_stem` below, which strips exactly one — so the two paths
/// don't duplicate each other.
fn output_stem(source: &Path) -> String {
    let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("");
    for suffix in [".model.json", EVENT_LOG_SUFFIX] {
        if let Some(base) = name.strip_suffix(suffix) {
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    source
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("board")
        .to_string()
}

fn cmd_render(args: &RenderArgs) {
    let path = Path::new(&args.source);
    let model = match load_source(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    warn_if_empty(&model, path);
    let dir = dir_of(path);
    let stem = output_stem(path);

    match &args.base {
        // Plain render: the board as-is.
        None => {
            let svg = render::render_svg(&model, &render::View::none(), None);
            let html = render::render_html(&svg, &model.title, false);
            let (svg_path, html_path) = write_board_files(&dir, &stem, &svg, &html);
            println!(
                "rendered {} elements → {} + {}",
                model.elements.len(),
                svg_path.display(),
                html_path.display()
            );
        }
        // Cross-file diff (F-variants): overlay `source` against the `--base` baseline. Both sides
        // load read-only via `load_source`, so a `model.json` *or* a log works on either side and
        // the baseline is never genesis'd or mutated. Output keeps the *source* stem — the variant
        // is the subject.
        Some(base_source) => {
            let base_path = Path::new(base_source);
            let base_model = match load_source(base_path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            };
            warn_if_empty(&base_model, base_path);
            let meta = (output_stem(base_path), stem.clone());
            let (svg, html, tally) = render_diff(&base_model, &model, meta);
            let (svg_path, html_path) = write_board_files(&dir, &stem, &svg, &html);
            println!(
                "rendered diff of {} vs {} — {} → {} + {}",
                stem,
                output_stem(base_path),
                tally,
                svg_path.display(),
                html_path.display()
            );
        }
    }
}

/// The whole render surface of F-variants' cross-file diff: `diff_boards(base, new)` then SVG + HTML,
/// plus a one-line tally of the change (added/removed/moved/changed element counts) for the summary.
/// Pure `(Model, Model) -> (svg, html, tally)` — no disk — so it unit-tests directly. `meta` labels
/// the two sides for the on-board legend and tooltips (`base` = "was", `new` = "now").
fn render_diff(
    base: &model::Model,
    new: &model::Model,
    meta: (String, String),
) -> (String, String, String) {
    use render::Tone;
    let (merged, diff) = render::diff_boards(base, new, meta);
    let tally = format!(
        "{} added, {} removed, {} moved, {} changed",
        diff.count(Tone::Added),
        diff.count(Tone::Removed),
        diff.count(Tone::Moved),
        diff.count(Tone::Changed),
    );
    let svg = render::render_svg(&merged, &render::View::none(), Some(&diff));
    // A cross-file diff is a review artifact → render read-only (variant = true).
    let html = render::render_html(&svg, &merged.title, true);
    (svg, html, tally)
}

/// Write a rendered board's SVG + HTML beside a source, under `stem` (`<stem>.svg` / `<stem>.html`).
/// Both the plain `render` path and the `--base` diff path funnel through here, so the write and its
/// error/exit handling live in exactly one place. Returns the two paths for the caller's summary line.
fn write_board_files(dir: &Path, stem: &str, svg: &str, html: &str) -> (PathBuf, PathBuf) {
    let svg_path = dir.join(format!("{stem}.svg"));
    let html_path = dir.join(format!("{stem}.html"));
    if let Err(e) = std::fs::write(&svg_path, format!("{svg}\n")) {
        eprintln!("error writing {}: {e}", svg_path.display());
        exit(1);
    }
    if let Err(e) = std::fs::write(&html_path, html) {
        eprintln!("error writing {}: {e}", html_path.display());
        exit(1);
    }
    (svg_path, html_path)
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
    warn_if_empty(&model, path);
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
            .map(|e| format!("{} \"{}\"", model::lane_to_str(e.kind), e.label))
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

/// Export a board to a portable text format on **stdout** (a read-only, non-mutating verb, so it
/// takes any source — a `model.json` or an event log — via `load_source`). Mermaid is the only
/// format today; the degradation is announced both inside the output (a `%%` header) and on stderr,
/// so a piped `stdout` stays clean Mermaid while an interactive user still sees the warning.
fn cmd_export(source: &str, format: Format) {
    let path = Path::new(source);
    let model = match load_source(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    warn_if_empty(&model, path);
    let out = match format {
        Format::Mermaid => render::render_mermaid(&model),
        Format::Context => render::render_context(&model),
    };
    print!("{out}");
    // The stderr notice is format-specific: a Mermaid export is wholesale lossy, while the context
    // pack is lossless prose whose only lossy part is its *embedded* diagram — no need to shout.
    match format {
        Format::Mermaid => eprintln!("{}", render::DEGRADATION_NOTICE),
        Format::Context => {}
    }
}

/// Migrate a `model.json` into the genesis batch of a `<name>.event-log.jsonl` written alongside it —
/// the bootstrap path into the event-sourced world. Returns the log path and a one-line summary
/// (both the explicit `genesis` command and serve-time auto-genesis print the same line).
///
/// The write is an **exclusive create** (`create_new`): if a log already exists it fails rather
/// than truncate it, so the "log is append-only truth" invariant is enforced by the write itself —
/// no caller-side guard to forget, and no check-then-write race can clobber a live log. The model
/// is loaded *before* the write, so a malformed model surfaces its own error even when a log is
/// also present.
/// The log an extract is written to: the source's board name, the selector's slug, and the log
/// suffix (`orders.event-log.jsonl` + `--region K2` → `orders-K2.event-log.jsonl`). A sibling of
/// the source, never a nested path — the slug is sanitised to plain filename characters.
fn extract_out_path(source: &Path, sel: &extract::Selector) -> PathBuf {
    dir_of(source).join(format!(
        "{}-{}{EVENT_LOG_SUFFIX}",
        output_stem(source),
        sel.slug()
    ))
}

/// Carve a sub-board out of a source and write it beside it as a genesis'd log (F-extract).
///
/// The output is a **log**, not a `model.json`: `events::from_model` already exists, so the
/// extract lands directly on the spine and is immediately `render`/`serve`-able with no second
/// migration step. Ids and columns are preserved, so `faceto render sub --base origin` is a
/// meaningful diff — the extract → variant → diff loop this verb exists to open.
///
/// The source is read-only and loads through `load_source`, so a `model.json` works as well as a
/// log. Writing goes through `create_log_exclusive`: an extract never clobbers an existing log.
fn cmd_extract(source: &str, sel: &extract::Selector) {
    let given = Path::new(source);
    // Read the *truth*. Handed a `model.json` that already has a log beside it, the model is a
    // stale bootstrap form — every edit made on the board since genesis lives in the log. `render`
    // getting that wrong costs a re-run; `extract` writes a new board out of it, so the staleness
    // would be persisted. The board name (and so the output filename) still comes from the source
    // the user named: a model and its log resolve to the same stem anyway.
    let owned = existing_log_for(given, "extracting from");
    let path = owned.as_deref().unwrap_or(given);
    let model = match load_source(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    warn_if_empty(&model, path);
    let sub = match extract::extract(&model, sel) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    let out = extract_out_path(path, sel);
    let batch = events::from_model(&sub);
    if let Err(e) = create_log_exclusive(&out, &batch) {
        eprintln!("error: {e}");
        exit(1);
    }
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    println!(
        "extracted {} — {}, {}, {} → {}",
        sel.label(),
        plural(sub.elements.len(), "element"),
        plural(sub.edges.len(), "edge"),
        plural(sub.phases.len(), "region"),
        out.display()
    );
}

/// `extract [SOURCE] --region ID | --focus ID [--hops N] | --type KIND`.
///
/// **Exactly one selector**: a second one is a usage error (exit 2), not an intersection —
/// `--focus E4 --hops 2 --type hotspot` would have to define whether the walk runs before or
/// after the lane filter, and guessing is worse than refusing. `--hops` defaults to 1 and is
/// meaningless without `--focus`, so it says so rather than being silently ignored.
fn parse_extract(args: &[String]) -> (String, extract::Selector) {
    use extract::Selector;
    let mut source = "model.json".to_string();
    let mut selector: Option<Selector> = None;
    let mut hops: Option<usize> = None;
    let mut i = 0;

    // One arm for all three selectors: each needs a value, and each must be the only one.
    let set = |sel: Selector, current: &mut Option<Selector>| {
        if let Some(had) = current {
            eprintln!(
                "extract takes one selector, not two ({} and {})\n(--region / --focus / --type \
                 are alternatives, not filters that combine)",
                had.label(),
                sel.label()
            );
            exit(2);
        }
        *current = Some(sel);
    };

    while i < args.len() {
        // A value is never allowed to start with `-`: `--focus --type hotspot` must not read the
        // next flag as an element id. The message says so, rather than the misleading bare "needs
        // a value" a `-`-leading argument used to get.
        let value = |flag: &str| match args.get(i + 1) {
            Some(v) if !v.starts_with('-') => v.clone(),
            Some(v) => {
                eprintln!("{flag} needs a value, and {v} looks like a flag (values cannot start with '-')");
                exit(2);
            }
            None => {
                eprintln!("{flag} needs a value");
                exit(2);
            }
        };
        match args[i].as_str() {
            "--region" => {
                set(Selector::Region(value("--region")), &mut selector);
                i += 2;
            }
            "--focus" => {
                set(
                    Selector::Focus {
                        id: value("--focus"),
                        hops: 1,
                    },
                    &mut selector,
                );
                i += 2;
            }
            "--type" => {
                // A lane typed on the command line is worth refusing loudly, unlike one met in a
                // log: there is a person here to correct it, and the alternative is an empty cut.
                let v = value("--type");
                match model::lane_from_str(&v) {
                    Some(lane) => set(Selector::Kind(lane), &mut selector),
                    None => {
                        eprintln!(
                            "--type {v} is not one of the eight lanes: {}",
                            model::LANES
                                .iter()
                                .map(|&l| model::lane_to_str(l))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        exit(2);
                    }
                }
                i += 2;
            }
            "--hops" => {
                let v = value("--hops");
                // Repeated, `--hops` is refused rather than last-wins — the same rule a repeated
                // selector gets. Two different depths in one command line is a mistake, and
                // silently honouring the last one hides it behind a plausible result.
                if let Some(had) = hops {
                    eprintln!("--hops given twice ({had} and {v}) — pick one depth");
                    exit(2);
                }
                match v.parse() {
                    Ok(n) => hops = Some(n),
                    Err(_) => {
                        eprintln!("--hops needs a whole number of edges (e.g. --hops 2)");
                        exit(2);
                    }
                }
                i += 2;
            }
            _ => {
                reject_flag(&args[i]);
                source = args[i].clone();
                i += 1;
            }
        }
    }

    let selector = match selector {
        Some(s) => s,
        None => {
            eprintln!(
                "extract needs a selector: --region ID, --focus ID [--hops N], or --type KIND"
            );
            exit(2);
        }
    };
    // `--hops` is applied after the fact so flag order never matters (`--hops 2 --focus E4` reads
    // the same as the reverse), and so a `--hops` on a non-focus selector can be *named* rather
    // than dropped in silence.
    match (hops, selector) {
        (Some(n), Selector::Focus { id, .. }) => (source, Selector::Focus { id, hops: n }),
        (Some(_), other) => {
            eprintln!(
                "--hops only means something with --focus (it bounds the walk out from one \
                 element); {} selects no neighbourhood",
                other.label()
            );
            exit(2);
        }
        (None, selector) => (source, selector),
    }
}

/// Write a batch of events to a **new** log file, or fail. The write *is* the guard: the file is
/// opened with an exclusive create, so "a log is append-only truth" holds even against a
/// concurrent process — there is no check-then-write race and no caller-side check to forget.
/// Shared by `genesis` (migrating a model) and `extract` (emitting a sub-board), so the two paths
/// into a fresh log cannot drift on the one rule that protects an existing one.
fn create_log_exclusive(out: &Path, batch: &[events::Event]) -> Result<(), String> {
    let mut f = match OpenOptions::new().write(true).create_new(true).open(out) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            return Err(format!(
                "{} already exists — refusing to overwrite",
                out.display()
            ));
        }
        Err(e) => return Err(format!("writing {}: {e}", out.display())),
    };
    f.write_all(events::to_jsonl(batch).as_bytes())
        .map_err(|e| format!("writing {}: {e}", out.display()))
}

fn write_genesis(model_path: &Path) -> Result<(PathBuf, String), String> {
    let model = model::load(model_path)?;
    warn_if_empty(&model, model_path);
    let out = log_beside(model_path);
    let batch = events::from_model(&model);
    create_log_exclusive(&out, &batch)?;

    let summary = format!(
        "seeded {} events from {} → {}",
        batch.len(),
        model_path.display(),
        out.display()
    );
    Ok((out, summary))
}

fn cmd_genesis(model_path: &str) {
    // `write_genesis` refuses to clobber intrinsically (exclusive create), so there is no
    // separate exists-check to keep in sync — and loading the model first means a broken model
    // reports *its* error, not "already exists".
    match write_genesis(Path::new(model_path)) {
        Ok((_, summary)) => println!("{summary}"),
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    }
}

/// The log that already holds a source's truth, if there is one: the source itself when it *is* a
/// log, or a `<name>.event-log.jsonl` sitting beside a model. `None` means "no log exists yet" —
/// this never creates one, so a read-only verb can prefer the truth without a write as a side
/// effect.
///
/// Once a log exists the model beside it is a stale bootstrap form: every board edit since genesis
/// landed in the log and none of them went back to the model. A verb that reads the model anyway
/// silently works from an older board — harmless for a render you can re-run, but `extract`
/// *persists* what it read, which is why it resolves through here too. `verb` is the gerund used
/// in the notice ("serving" / "extracting from").
fn existing_log_for(source: &Path, verb: &str) -> Option<PathBuf> {
    if events::is_log_path(source) {
        return Some(source.to_path_buf());
    }
    let log = log_beside(source);
    if log.exists() {
        println!(
            "{} exists beside {} — {} the log (it is the truth; the model is derived)",
            log.display(),
            source.display(),
            verb
        );
        return Some(log);
    }
    None
}

/// Resolve the source a `serve` command must mutate to an event log, auto-running genesis for a
/// bare `model.json` (F-auto-genesis). Serving mutates, and every mutation must land in the log —
/// the truth — never in the derived model, so:
///
///   * an event log is served as-is;
///   * a `model.json` beside its existing `<name>.event-log.jsonl` redirects to that log (the log
///     already won; the model is a derived/bootstrap form, so it is ignored once a log exists);
///   * a `model.json` with no sibling log is migrated once (genesis) and the fresh log is served —
///     but if a *legacy* bare `event-log.jsonl` (pre-F-output-naming name) sits beside it, warn
///     first so the user can rename it rather than have its history silently stranded.
///
/// This is what kills legacy mode: `serve` never opens a `model.json` for writing, so no mutation
/// can ever land outside the log.
fn serve_log_path(source: &Path) -> Result<std::path::PathBuf, String> {
    if let Some(log) = existing_log_for(source, "serving") {
        return Ok(log);
    }
    // Upgrade footgun: pre-F-output-naming logs were the bare `event-log.jsonl`. If one sits beside
    // the model under that old name, it is *not* this model's derived log, so genesis below would
    // mint a fresh empty-history log and silently strand the user's real one. Point them at the
    // rename rather than skip it in silence.
    let expected = log_beside(source);
    let legacy = dir_of(source).join("event-log.jsonl");
    if legacy != expected && legacy.exists() {
        eprintln!(
            "warning: found a legacy {} that is not this model's log — rename it to {} to keep its \
             history (genesis is creating a fresh log instead)",
            legacy.display(),
            expected.display()
        );
    }
    let (out, summary) = write_genesis(source)?;
    println!("{summary}");
    Ok(out)
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

/// Parsed `render [SOURCE] [--base OTHER]`: the positional `source` (the "now" board) and an
/// optional `base` to diff against (F-variants). `--base` names the *baseline* board; the overlay
/// then shows `source`'s added/removed/moved/changed *against* it, so the positional is always the
/// subject and `--base` the "was" side — the same direction `serve --base` uses.
struct RenderArgs {
    source: String,
    base: Option<String>,
}

/// `render [SOURCE] [--base OTHER]`. The positional is the source; `--base` takes the baseline path.
/// A `--base` with no value fails loudly (exit 2), mirroring `parse_export`'s `--format`; any other
/// flag-shaped arg is still rejected by `reject_flag`.
fn parse_render(args: &[String]) -> RenderArgs {
    let mut source = "model.json".to_string();
    let mut base = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--base" {
            match args.get(i + 1) {
                Some(v) => base = Some(v.clone()),
                None => {
                    eprintln!("--base needs a value (the baseline board to diff against)");
                    exit(2);
                }
            }
            i += 2;
        } else {
            reject_flag(&args[i]);
            source = args[i].clone();
            i += 1;
        }
    }
    RenderArgs { source, base }
}

/// `serve [SOURCE] [-p PORT] [--base OTHER]`. Positional source; `-p`/`--port` the port; `--base` an
/// optional baseline board the live overlay diffs against (F-variants) — same flag and same
/// direction as `render --base` (SOURCE is "now", `--base` the "was" side). A `--base` with no value
/// fails loudly (exit 2).
fn parse_serve(args: &[String]) -> (String, u16, Option<String>) {
    let mut model = "model.json".to_string();
    let mut port: u16 = 8753;
    let mut base = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--port" => {
                if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    port = v;
                }
                i += 2;
            }
            "--base" => {
                match args.get(i + 1) {
                    Some(v) => base = Some(v.clone()),
                    None => {
                        eprintln!("--base needs a value (the baseline board to diff against)");
                        exit(2);
                    }
                }
                i += 2;
            }
            _ => {
                reject_flag(&args[i]);
                model = args[i].clone();
                i += 1;
            }
        }
    }
    (model, port, base)
}

/// The output formats `export` can emit. `Mermaid` is a lossy diagram; `Context` is a rich
/// markdown+Mermaid context pack for a coding agent. The enum grows without reshaping —
/// F-model-export's `model` and F-narrative-export's `narrative` slot in here.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    Mermaid,
    Context,
}

/// `export [SOURCE] [--format FMT]`. Positional source defaults to `model.json`; `--format` defaults
/// to `mermaid`. An unknown format fails loudly (exit 2) rather than being misread as a source path,
/// mirroring `parse_serve`'s handling of `-p`.
fn parse_export(args: &[String]) -> (String, Format) {
    let mut source = "model.json".to_string();
    let mut format = Format::Mermaid;
    let mut i = 0;
    while i < args.len() {
        if matches!(args[i].as_str(), "-f" | "--format") {
            match args.get(i + 1).map(String::as_str) {
                Some("mermaid") => format = Format::Mermaid,
                Some("context") => format = Format::Context,
                Some(other) => {
                    eprintln!("unknown export format: {other}\n(supported: mermaid, context)");
                    exit(2);
                }
                None => {
                    eprintln!("--format needs a value\n(supported: mermaid, context)");
                    exit(2);
                }
            }
            i += 2;
        } else {
            reject_flag(&args[i]);
            source = args[i].clone();
            i += 1;
        }
    }
    (source, format)
}

fn print_help() {
    println!(
        "faceto {} — a typed file → a visual workshop board you think through with an LLM\n\
         \n\
         USAGE:\n\
         \x20 faceto render  [SOURCE] [--base OTHER]  write <name>.svg + <name>.html (diff overlay vs OTHER)\n\
         \x20 faceto lint    [SOURCE]            check the board against the ES-grammar rules (warn-only)\n\
         \x20 faceto serve   [SOURCE] [-p PORT] [--base OTHER]  serve the live board + comment sidecar (default :8753)\n\
         \x20 faceto export  [SOURCE] [--format mermaid|context]  print the board to stdout (mermaid diagram, or a markdown context pack for a coding agent)\n\
         \x20 faceto extract [SOURCE] (--region ID | --focus ID [--hops N] | --type KIND)  carve a sub-board out into a sibling log\n\
         \x20 faceto genesis [MODEL]             migrate a model.json into a <name>.event-log.jsonl\n\
         \x20 faceto compact [LOG]               fold a log to a snapshot, bounding replay (default model.event-log.jsonl)\n\
         \x20 faceto help | version\n\
         \n\
         SOURCE is a model.json or an event log (*.jsonl / *.log); it defaults to ./model.json.\n\
         lint reads the board's \"level\" (big-picture | design); a design board adds stricter rules.\n\
         extract takes exactly one selector and keeps ids + columns, so the sub-board diffs\n\
         cleanly against the board it came from (faceto render SUB --base ORIGIN).",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Lane;

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

    /// Build a `Model` straight from a JSON board literal (no disk), for the diff-render tests.
    fn model_of(json: &str) -> model::Model {
        model::from_json(&crate::json::parse(json).unwrap())
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_render_reads_source_and_optional_base() {
        // No args → default source, no baseline (plain render).
        let r = parse_render(&args(&[]));
        assert_eq!(r.source, "model.json");
        assert!(r.base.is_none());

        // A bare positional is the source; still no baseline.
        let r = parse_render(&args(&["after.jsonl"]));
        assert_eq!(r.source, "after.jsonl");
        assert!(r.base.is_none());

        // `--base OTHER` sets the baseline; order-independent from the positional.
        let r = parse_render(&args(&["after.jsonl", "--base", "before.jsonl"]));
        assert_eq!(r.source, "after.jsonl");
        assert_eq!(r.base.as_deref(), Some("before.jsonl"));
        let r = parse_render(&args(&["--base", "before.jsonl", "after.jsonl"]));
        assert_eq!(r.source, "after.jsonl");
        assert_eq!(r.base.as_deref(), Some("before.jsonl"));
    }

    #[test]
    fn parse_extract_reads_each_selector_in_any_order() {
        use extract::Selector;

        let (src, sel) = parse_extract(&args(&["orders.jsonl", "--region", "K2"]));
        assert_eq!(src, "orders.jsonl");
        assert_eq!(sel, Selector::Region("K2".into()));

        // No positional → the same default every other verb uses.
        let (src, sel) = parse_extract(&args(&["--type", "hotspot"]));
        assert_eq!(src, "model.json");
        assert_eq!(sel, Selector::Kind(Lane::Hotspot));

        // `--hops` defaults to 1 and applies whichever side of `--focus` it is written on.
        let (_, sel) = parse_extract(&args(&["--focus", "E4"]));
        assert_eq!(
            sel,
            Selector::Focus {
                id: "E4".into(),
                hops: 1
            }
        );
        let (_, sel) = parse_extract(&args(&["--hops", "3", "--focus", "E4"]));
        assert_eq!(
            sel,
            Selector::Focus {
                id: "E4".into(),
                hops: 3
            }
        );
    }

    #[test]
    fn extract_out_path_is_a_sibling_log_named_for_the_selector() {
        use extract::Selector;
        let src = Path::new("boards/orders.event-log.jsonl");
        assert_eq!(
            extract_out_path(src, &Selector::Region("K2".into())),
            PathBuf::from("boards/orders-K2.event-log.jsonl")
        );
        // A model source resolves to the same board name, so both entry points name one file.
        assert_eq!(
            extract_out_path(
                Path::new("boards/orders.model.json"),
                &Selector::Focus {
                    id: "E4".into(),
                    hops: 2
                }
            ),
            PathBuf::from("boards/orders-E4-h2.event-log.jsonl")
        );
    }

    #[test]
    fn existing_log_for_prefers_the_log_without_ever_creating_one() {
        let dir = scratch("truth");
        let model = dir.join("orders.model.json");
        std::fs::write(&model, MODEL).unwrap();

        // No log yet: `None`, and — unlike `serve_log_path` — nothing is written. A read-only verb
        // must not mint a log as a side effect of being pointed at a model.
        assert!(existing_log_for(&model, "extracting from").is_none());
        assert!(!dir.join("orders.event-log.jsonl").exists());

        // Once a log exists it is the truth, and the model beside it is a stale bootstrap form.
        let log = dir.join("orders.event-log.jsonl");
        std::fs::write(&log, "PRIOR\n").unwrap();
        assert_eq!(
            existing_log_for(&model, "extracting from"),
            Some(log.clone())
        );
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "PRIOR\n");

        // A log source is itself, untouched, with no filesystem lookup.
        let direct = Path::new("/nowhere/x.event-log.jsonl");
        assert_eq!(
            existing_log_for(direct, "extracting from"),
            Some(direct.to_path_buf())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_extracted_log_replays_to_the_sub_board() {
        // The end-to-end contract of the verb: what lands on disk is a log that replays to
        // exactly the sub-board `extract` computed — ids and columns intact, so the result is a
        // legitimate `--base` for a diff against the origin.
        let board = model_of(
            r#"{"title":"Orders",
                "phases":[{"id":"K1","label":"a","fromCol":0,"toCol":1},
                          {"id":"K2","label":"b","fromCol":2,"toCol":3}],
                "elements":[{"id":"C1","type":"command","label":"Do","col":2},
                            {"id":"E1","type":"event","label":"Done","col":3},
                            {"id":"A1","type":"actor","label":"Who","col":0}],
                "edges":[["A1","C1"],["C1","E1"]]}"#,
        );
        let sub = extract::extract(&board, &extract::Selector::Region("K2".into())).unwrap();

        let dir = scratch("extract");
        let out = dir.join("orders-K2.event-log.jsonl");
        create_log_exclusive(&out, &events::from_model(&sub)).unwrap();

        let replayed = events::load(&out).unwrap();
        assert_eq!(replayed, sub, "the log round-trips the extracted board");
        assert_eq!(
            replayed
                .elements
                .iter()
                .map(|e| (e.id.as_str(), e.col))
                .collect::<Vec<_>>(),
            [("C1", Some(2)), ("E1", Some(3))],
            "ids and columns survive the trip to disk"
        );

        // Same refusal as genesis: an extract never clobbers a log that already exists.
        let err = create_log_exclusive(&out, &events::from_model(&sub)).unwrap_err();
        assert!(err.contains("refusing to overwrite"), "{err}");
    }

    #[test]
    fn parse_serve_reads_port_and_optional_base() {
        // Defaults: model.json, :8753, no overlay baseline.
        let (src, port, base) = parse_serve(&args(&[]));
        assert_eq!((src.as_str(), port), ("model.json", 8753));
        assert!(base.is_none());

        // `--base` sets the baseline and composes with `-p`, in any order.
        let (src, port, base) = parse_serve(&args(&[
            "after.jsonl",
            "-p",
            "9000",
            "--base",
            "before.jsonl",
        ]));
        assert_eq!((src.as_str(), port), ("after.jsonl", 9000));
        assert_eq!(base.as_deref(), Some("before.jsonl"));
        let (src, port, base) = parse_serve(&args(&["--base", "before.jsonl", "after.jsonl"]));
        assert_eq!((src.as_str(), port), ("after.jsonl", 8753));
        assert_eq!(base.as_deref(), Some("before.jsonl"));
    }

    #[test]
    fn render_diff_tags_added_removed_and_changed_in_the_svg() {
        // A base board and a variant on stable ids: E2 added, E1 relabeled (changed), C1 removed.
        let base = model_of(
            r#"{"title":"T","elements":[
                {"id":"E1","type":"event","label":"Placed","col":0},
                {"id":"C1","type":"command","label":"Place","col":0}
            ]}"#,
        );
        let variant = model_of(
            r#"{"title":"T","elements":[
                {"id":"E1","type":"event","label":"Order placed","col":0},
                {"id":"E2","type":"event","label":"Order shipped","col":1}
            ]}"#,
        );
        let (svg, html, tally) = render_diff(&base, &variant, ("before".into(), "after".into()));

        // The overlay carries every diff verdict as an SVG class the stylesheet colours.
        assert!(svg.contains("added"), "added element missing from overlay");
        assert!(
            svg.contains("removed"),
            "removed element missing from overlay"
        );
        assert!(
            svg.contains("changed"),
            "changed element missing from overlay"
        );
        // The tally counts them for the CLI summary line.
        assert_eq!(tally, "1 added, 1 removed, 0 moved, 1 changed");
        // HTML wraps the same SVG (the file `render --base` writes).
        assert!(html.contains("<svg"));
    }

    #[test]
    fn parse_export_selects_source_and_format() {
        // Default: model.json, mermaid.
        assert_eq!(parse_export(&[]), ("model.json".into(), Format::Mermaid));
        // Positional source only.
        assert_eq!(
            parse_export(&["board.jsonl".into()]),
            ("board.jsonl".into(), Format::Mermaid)
        );
        // --format context (and the -f alias) select the context pack; source keeps its default.
        assert_eq!(
            parse_export(&["--format".into(), "context".into()]),
            ("model.json".into(), Format::Context)
        );
        assert_eq!(
            parse_export(&["b.jsonl".into(), "-f".into(), "mermaid".into()]),
            ("b.jsonl".into(), Format::Mermaid)
        );
    }

    #[test]
    fn log_beside_is_the_sibling_event_log() {
        // The log name is derived from the model basename so siblings own separate logs.
        assert_eq!(
            log_beside(Path::new("/a/b/orders.model.json")),
            PathBuf::from("/a/b/orders.event-log.jsonl")
        );
        // A bare filename resolves against ".".
        assert_eq!(
            log_beside(Path::new("orders.model.json")),
            PathBuf::from("./orders.event-log.jsonl")
        );
    }

    #[test]
    fn output_stem_derives_a_board_name_per_source() {
        // Sibling boards coexist: the basename comes from the source, minus its model/log suffix.
        assert_eq!(output_stem(Path::new("/a/b/orders.model.json")), "orders");
        assert_eq!(output_stem(Path::new("orders.model.json")), "orders");
        // A model and *its* log share one stem, so `render` of either names the same board.
        assert_eq!(
            output_stem(Path::new("/a/orders.event-log.jsonl")),
            "orders"
        );
        // A legacy bare log keeps its own name; a plain `.json`/`.jsonl` strips one extension.
        assert_eq!(output_stem(Path::new("/a/event-log.jsonl")), "event-log");
        assert_eq!(output_stem(Path::new("foo.json")), "foo");
        // No recognised suffix → the plain stem; an empty/odd path → the `board` fallback.
        assert_eq!(output_stem(Path::new("plain.txt")), "plain");
        assert_eq!(output_stem(Path::new("")), "board");
    }

    #[test]
    fn write_genesis_refuses_to_clobber_an_existing_log() {
        // The exclusive-create write must fail rather than truncate the append-only truth log.
        let dir = scratch("clobber");
        let model = dir.join("orders.model.json");
        std::fs::write(&model, MODEL).unwrap();
        let log = dir.join("orders.event-log.jsonl");
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
        let model = dir.join("orders.model.json");
        std::fs::write(&model, MODEL).unwrap();
        let log = dir.join("orders.event-log.jsonl");
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
        let model = dir.join("orders.model.json");
        std::fs::write(&model, MODEL).unwrap();

        let served = serve_log_path(&model).unwrap();
        assert_eq!(served, dir.join("orders.event-log.jsonl"));
        assert!(served.exists());
        // It replays back to the one-element board.
        assert_eq!(events::load(&served).unwrap().elements.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
