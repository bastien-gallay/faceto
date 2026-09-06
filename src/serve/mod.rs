//! A tiny std-only HTTP server for the live board: serves the page, re-renders the SVG on
//! demand (so an appended event shows without a restart), an in-page diff against a cached
//! baseline, and a click→comment channel appended to the event log as events.
//!
//! The server always operates in event-log mode: `main` resolves any `model.json` to its
//! sibling `event-log.jsonl` before calling [`serve`] (auto-running genesis if needed), so the
//! log is the only file this server ever mutates. There is no legacy `comments.jsonl` path.

use self::hash::fnv12;
use self::http::handle;
use self::ids::{mint_id, mint_region_id};
use crate::model::{Lane, Model};
use crate::{events, model};
use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

mod hash;
mod http;
mod ids;
mod mint;
mod sidebar;

#[cfg(test)]
mod testutil;

const CACHE_MAX: usize = 12;

/// Upper bound on a `POST /comment` body. Comments and structural ops are a few hundred bytes;
/// 1 MiB is generous headroom while refusing an attacker-sized `Content-Length` before allocating.
pub(crate) const MAX_BODY: usize = 1 << 20;

/// Upper bound on a single request/header line, and on the number of header lines. `MAX_BODY`
/// caps the *body* only; without these a client that dribbles header bytes with no `\r\n` (or an
/// unbounded header count) would grow `read_line` without limit — an OOM before routing ever runs.
pub(crate) const MAX_HEADER_LINE: usize = 16 * 1024;

pub(crate) const MAX_HEADERS: usize = 200;

struct Cache {
    map: HashMap<String, Model>,
    order: VecDeque<String>,
}

/// Lock a mutex, recovering the guard if a prior holder panicked and poisoned it. A panic on one
/// connection thread (say an unexpected panic deep in `replay` while minting under `appends`) must
/// not permanently brick every future request with a poisoned-lock panic: the state these mutexes
/// guard — the append-serialization token and the in-memory model ring — is rebuildable from the
/// log on the next read, so continuing with the recovered guard is correct, not a silent corruption.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Why an append made under the append lock failed, split by **whose** error it is. The board's own
/// refusal is a client error — a stale or out-of-range `atCol`, judged against the replayed log —
/// so answering it `500` would tell an agent the append broke and invite it to retry a request that
/// can never succeed. `Server` is the other half: an unreadable log, a write that did not land.
#[derive(Debug)]
pub(crate) enum Refusal {
    Board(String),
    Server(String),
}

pub(crate) struct Ctx {
    /// The event log — the single source of truth this server reads and appends to.
    pub(crate) model_path: PathBuf,
    /// A launch-time `--base` overlay baseline (F-variants): a fixed board every render is diffed
    /// against, plus its ("was", "now") legend labels. `None` for a plain live board. Loaded once,
    /// read-only — the baseline is a static comparison input, never part of the log.
    pub(crate) baseline: Option<(Model, (String, String))>,
    cache: Mutex<Cache>,
    /// Serializes appends to the log (H4): concurrent `POST /comment` handlers run on
    /// separate threads, so without this two events could interleave mid-line. Holding
    /// this lock around a single `write_all` makes each appended line atomic.
    appends: Mutex<()>,
}

impl Ctx {
    /// A context over an event log, with an empty recent-model ring, a free appends lock, and no
    /// overlay baseline. `serve` sets `baseline` when launched with `--base`.
    fn new(model_path: PathBuf) -> Self {
        Ctx {
            model_path,
            baseline: None,
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
    pub(crate) fn current(&self) -> Result<(String, Model), String> {
        let raw = std::fs::read(&self.model_path).map_err(|e| e.to_string())?;
        let version = fnv12(&raw);
        // Fast path: an unchanged log (same content hash) reuses the cached projection, so
        // `parse_log`+`replay` run only when a new event has actually landed — not on every page
        // load or refresh. Before this, the cache was a mere insertion guard and replay ran every call.
        if let Some(model) = self.cached(&version) {
            return Ok((version, model));
        }
        let text = String::from_utf8_lossy(&raw);
        let model = events::replay(&events::parse_log(&text)?);
        let mut c = lock(&self.cache);
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

    pub(crate) fn cached(&self, version: &str) -> Option<Model> {
        lock(&self.cache).map.get(version).cloned()
    }

    /// Just the log's content hash — the value `/model-version` reports. Hashing the raw bytes is
    /// O(bytes) and deliberately skips `parse_log`+`replay`, so a version check never folds the whole
    /// log just to learn nothing changed. (The client fetches this on load and on Reload — there is
    /// no polling loop; `F-collab-sse` is the un-shipped push path.)
    pub(crate) fn version(&self) -> Result<String, String> {
        let raw = std::fs::read(&self.model_path).map_err(|e| e.to_string())?;
        Ok(fnv12(&raw))
    }

    /// Append a text block (one line, or several newline-joined lines for a multi-event action)
    /// to `path`, serialized against all other appends so concurrent posts cannot interleave
    /// (H4). The block and its trailing newline are written in a single `write_all`, so even
    /// under `O_APPEND` no half-line — and no half of a multi-event action — ever lands.
    pub(crate) fn append_line(&self, path: &Path, line: &str) -> std::io::Result<()> {
        let _guard = lock(&self.appends);
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
    ) -> Result<events::Event, Refusal> {
        let _guard = lock(&self.appends);
        // Mint/validate from the *real* log. A corrupt/unreadable log must fail the append, not
        // silently fold to empty (which would re-mint E1/C1… and collide).
        let raw = std::fs::read(&self.model_path).map_err(|e| Refusal::Server(e.to_string()))?;
        let text = String::from_utf8_lossy(&raw);
        let log = events::parse_log(&text).map_err(Refusal::Server)?;
        let ev = build(&log).map_err(Refusal::Board)?;
        Self::write_line(&self.model_path, &events::line(&ev))
            .map_err(|e| Refusal::Server(e.to_string()))?;
        Ok(ev)
    }

    /// Mint a fresh id and append an `ElementAdded` (H6). The id is derived from the log's
    /// `ElementAdded` history (next free `<PREFIX><N>` for this lane) rather than a stored counter,
    /// so the log stays the only durable record. A lane-title `+` (prepend) derives its col from the
    /// live projection *under the lock* — a first-in-lane add aligns to the board's left column, a
    /// prepend into a non-empty lane marches left — so concurrent adds can't race the min.
    pub(crate) fn append_add(
        &self,
        kind: Lane,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
        prepend: bool,
    ) -> Result<events::Event, Refusal> {
        self.append_minted(|log| {
            let col = if prepend {
                Some(model::lane_left_col(&events::replay(log), kind))
            } else {
                col
            };
            Ok(events::Event::ElementAdded {
                id: mint_id(kind, log),
                kind,
                label,
                col,
                detail,
                y: None,
                links: Vec::new(),
            })
        })
    }

    /// Mint a fresh region id and append a `PhaseAdded` — the region counterpart of `append_add`.
    pub(crate) fn append_region_add(
        &self,
        label: String,
        from_col: i64,
        to_col: i64,
    ) -> Result<events::Event, Refusal> {
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
    pub(crate) fn append_phase_split(
        &self,
        id: String,
        at_col: i64,
        new_label: String,
    ) -> Result<events::Event, Refusal> {
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

/// Serve the live board for an event log. `main` guarantees `log_path` is an `event-log.jsonl`
/// (resolving any `model.json` through `serve_log_path` first), so this server only ever reads and
/// appends to the log — the single source of truth. `baseline` is an optional launch-time `--base`
/// overlay board (F-variants): when set, every rendered board is diffed against it.
pub fn serve(
    log_path: &Path,
    port: u16,
    baseline: Option<(Model, (String, String))>,
) -> Result<(), String> {
    let mut ctx = Ctx::new(log_path.to_path_buf());
    ctx.baseline = baseline;
    let ctx = Arc::new(ctx);

    // Validate the log up front so a typo fails loudly, not per-request.
    let (_v, model) = ctx.current()?;
    if let Some((base, _)) = &ctx.baseline {
        crate::render::comparable(base, &model)?;
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    println!(
        "faceto board live → http://127.0.0.1:{}  (Ctrl-C to stop)",
        port
    );
    if let Some((_, (was, now))) = &ctx.baseline {
        println!("  overlay: diffing “{now}” against baseline “{was}” (--base)");
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events;
    use crate::serve::testutil::*;

    use std::sync::Arc;
    use std::thread;

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
    fn append_add_mints_persists_and_replays() {
        // The minted id round-trips: append_add writes an ElementAdded that replay folds
        // back into a real element, and a second add under the same lane increments.
        let path = std::env::temp_dir().join(format!("faceto-h6-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&added("E1", Lane::Event)) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());

        let ev = ctx
            .append_add(Lane::Event, "DayStarted".into(), Some(2), None, false)
            .unwrap();
        assert!(matches!(&ev, events::Event::ElementAdded { id, .. } if id == "E2"));
        let ev2 = ctx
            .append_add(Lane::Command, "start".into(), None, None, false)
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
    fn append_add_errors_on_a_corrupt_log_rather_than_minting_from_empty() {
        // A malformed log must fail the add — not fold to an empty model and re-mint E1.
        let path =
            std::env::temp_dir().join(format!("faceto-corrupt-{}.jsonl", std::process::id()));
        std::fs::write(&path, "{ this is not json\n").unwrap();
        let ctx = Ctx::new(path.clone());
        let r = ctx.append_add(Lane::Event, "X".into(), None, None, false);
        let _ = std::fs::remove_file(&path);
        assert!(r.is_err());
    }
}
