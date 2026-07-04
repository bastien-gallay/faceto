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
use crate::model::Model;
use crate::{events, model};
use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

mod comment;
mod hash;
mod http;
mod ids;
mod sidebar;

#[cfg(test)]
mod tests;

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

pub(crate) struct Ctx {
    /// The event log — the single source of truth this server reads and appends to.
    pub(crate) model_path: PathBuf,
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
    pub(crate) fn current(&self) -> Result<(String, Model), String> {
        let raw = std::fs::read(&self.model_path).map_err(|e| e.to_string())?;
        let version = fnv12(&raw);
        // Fast path: an unchanged log (same content hash) reuses the cached projection, so
        // `parse_log`+`replay` run only when a new event has actually landed — not on every page
        // load or poll. Before this, the cache was a mere insertion guard and replay ran every call.
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

    /// Just the log's content hash — the value `/model-version` polls for. Hashing the raw bytes is
    /// O(bytes) and deliberately skips `parse_log`+`replay`, so the client's once-a-second poll
    /// never folds the whole log just to learn nothing changed.
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
    ) -> Result<events::Event, String> {
        let _guard = lock(&self.appends);
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
    pub(crate) fn append_add(
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
    pub(crate) fn append_region_add(
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
    pub(crate) fn append_phase_split(
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
