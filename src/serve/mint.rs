//! Carry out a [`Mint`]: the three creation commands whose id only the server can assign (H6 /
//! review #3 / F-region-frontiers). Every field arrived guarded — `parse_command` refused a blank
//! label, an off-grammar lane and an inverted span before the request got here — so this file is
//! the dispatch and nothing else. What it cannot check at the parse it checks under the lock:
//! whether a split column falls inside its phase depends on the replayed board.

use super::Ctx;
use crate::events::{Event, Mint};

/// Append the event `cmd` mints. Returns the HTTP status to fail with: `500` when the append
/// itself fails, which now includes a `phase-split` the board refuses (an out-of-range or stale
/// `atCol` — judged against the log under the lock, never here).
pub(crate) fn append_mint(ctx: &Ctx, cmd: &Mint) -> Result<Event, u16> {
    match cmd {
        Mint::Add {
            lane,
            label,
            col,
            detail,
            prepend,
        } => ctx.append_add(*lane, label.clone(), *col, detail.clone(), *prepend),
        Mint::RegionAdd {
            label,
            from_col,
            to_col,
        } => ctx.append_region_add(label.clone(), *from_col, *to_col),
        Mint::PhaseSplit { id, at_col, label } => {
            ctx.append_phase_split(id.clone(), *at_col, label.clone())
        }
    }
    .map_err(|_| 500u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events;
    use crate::json;
    use crate::model::Lane;
    use crate::serve::testutil::*;

    /// A log holding one element, and a `Ctx` over it.
    fn board(tag: &str) -> (std::path::PathBuf, Ctx) {
        let path = std::env::temp_dir().join(format!("faceto-{tag}-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, events::line(&added("E1", Lane::Event)) + "\n").unwrap();
        let ctx = Ctx::new(path.clone());
        (path, ctx)
    }

    #[test]
    fn a_refused_command_never_reaches_the_log_but_a_real_one_replays() {
        // The whole point of parsing first: a blank rename is refused before the append path
        // exists to be called, so the append-only truth gains nothing to undo.
        let (path, ctx) = board("mint-rn");
        let before = std::fs::read_to_string(&path).unwrap();
        let blank = json::parse(r#"{"elemId":"E1","kind":"rename","text":"   "}"#).unwrap();
        assert!(events::parse_command(&blank).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

        let real = json::parse(r#"{"elemId":"E1","kind":"rename","text":"Reborn"}"#).unwrap();
        let evs = match events::parse_command(&real) {
            Ok(events::Command::Fold(cmd)) => events::fold_to_events(&cmd),
            _ => Vec::new(),
        };
        let block = evs.iter().map(events::line).collect::<Vec<_>>().join("\n");
        ctx.append_line(&ctx.model_path, &block).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let model = events::replay(&events::parse_log(&text).unwrap());
        assert_eq!(
            model.elements.iter().find(|e| e.id == "E1").unwrap().label,
            "Reborn"
        );
    }

    #[test]
    fn a_split_the_board_refuses_burns_no_region_id() {
        // The one guard the parse cannot run: whether `at_col` falls inside the phase depends on
        // the replayed board. A stale split must fail *before* writing, or it spends an id and
        // leaves a permanent dead event while the client reports success.
        let (path, ctx) = board("mint-split");
        let cmd = Mint::PhaseSplit {
            id: "K9".into(),
            at_col: 2,
            label: "Right".into(),
        };
        assert_eq!(append_mint(&ctx, &cmd), Err(500));
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(
            !text.contains("PhaseSplit"),
            "a refused split wrote a line: {text}"
        );
    }
}
