//! The command boundary: [`parse_command`] reads one posted `POST /comment` body into a
//! [`Command`], and [`fold_to_events`] maps the ones the server can persist alone to their events.
//!
//! Every guard on a posted field lives in the parse, so a `Command` in hand is already legal and
//! the functions downstream take it instead of a `Json` they must re-interrogate.
//!
//! A *log* read skips kinds it does not recognise — that tolerance is how an older faceto reads a
//! newer log, and it belongs at the `upcast` seam (`codec.rs`). A command is the opposite
//! situation: a client is waiting for an answer, so an unrecognised one is refused by name. The
//! set below is therefore closed, and `comment`/`question`/`split` are listed, not defaulted to.

use super::Event;
use crate::json::Json;
use crate::model::{lane_from_str, Lane};

/// A parsed `POST /comment` body, split along the server's two paths: a [`Mint`] needs an id only
/// the server can assign (H6) and is appended under the lock; a [`Fold`] carries everything its
/// events need.
#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    Mint(Mint),
    Fold(Fold),
}

/// A creation command. Its event cannot be built from the post alone — the id is the server's.
#[derive(Clone, PartialEq, Debug)]
pub enum Mint {
    /// `prepend` (the lane-title `+`) asks the server to derive the lane's left-edge col under
    /// the lock, which is why `col` is optional beside it.
    Add {
        lane: Lane,
        label: String,
        col: Option<i64>,
        detail: Option<String>,
        prepend: bool,
    },
    RegionAdd {
        label: String,
        from_col: i64,
        to_col: i64,
    },
    /// Whether `at_col` falls strictly inside the phase is judged under the lock, against the
    /// replayed board — state this parse cannot see.
    PhaseSplit {
        id: String,
        at_col: i64,
        label: String,
    },
}

/// A command whose events the post fully determines.
#[derive(Clone, PartialEq, Debug)]
pub enum Fold {
    /// Relocate along the timeline (`col`) and/or within the lane band (`y`); at least one is
    /// present, or there would be nothing to persist. `swap` is a displaced occupant (old clients
    /// / stashed offline moves).
    Move {
        id: String,
        col: Option<i64>,
        y: Option<f64>,
        swap: Option<(String, i64)>,
    },
    Rename {
        id: String,
        label: String,
    },
    /// The resolution may be empty: closing a hotspot is an act, not a note.
    Resolve {
        id: String,
        resolution: String,
    },
    Drop {
        id: String,
    },
    /// `comment`, `question` and `split` all land here — the log stores the text, not which of
    /// the three words framed it.
    Annotate {
        id: String,
        text: String,
    },
    /// An edge's identity is its directed pair, so these name both ends and no `elemId`. Endpoint
    /// *existence* is not checked: this sees the post, never the `Model`.
    Connect {
        src: String,
        dst: String,
    },
    Disconnect {
        src: String,
        dst: String,
    },
    /// The legacy independent-span resize (old clients / stashed offline moves). The live client
    /// posts `FrontierMove`; either way `replay`'s `normalize` re-projects a contiguous partition.
    RegionResize {
        id: String,
        from_col: i64,
        to_col: i64,
    },
    /// Set one border of a region; `normalize` re-borders the neighbour.
    FrontierMove {
        id: String,
        edge: Frontier,
        col: i64,
    },
    RegionRename {
        id: String,
        label: String,
    },
    RegionRemove {
        id: String,
    },
}

/// Which border of a region a [`Fold::FrontierMove`] names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frontier {
    Start,
    End,
}

impl Frontier {
    /// The spelling `Event::FrontierMoved` carries.
    fn as_str(self) -> &'static str {
        match self {
            Frontier::Start => "start",
            Frontier::End => "end",
        }
    }
}

/// Why a posted body is not a command. Both answer `400`; the distinction is for the operator
/// reading the console, where a misspelled kind and a missing field send you to two places.
#[derive(Clone, PartialEq, Debug)]
pub enum Rejection {
    /// A `kind` this build has no command for — a typo, or a client newer than the server.
    UnknownKind(String),
    /// A known kind whose payload is missing, blank, or out of range. Carries the sentence the
    /// console prints, so each guard states its reason where it is enforced.
    Malformed(&'static str),
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rejection::UnknownKind(kind) => write!(f, "no command named {kind:?}"),
            Rejection::Malformed(why) => f.write_str(why),
        }
    }
}

/// A label with content: trimmed, or `None` when blank. A blank one would mint or rename into a
/// permanent, never-renumbered empty box.
fn nonblank(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// `from_col < to_col`. An inverted or zero-width span makes `region_of`'s `from_col <= col &&
/// col <= to_col` unsatisfiable, so the model drops the region from every column's membership
/// while the render normalizes the span and draws a real band anyway.
fn valid_span(from_col: i64, to_col: i64) -> bool {
    from_col < to_col
}

/// Normalise a posted vertical sub-position to its stored form: clamped into `[0, 1]` and
/// rounded to 4 decimals so the log carries a clean human-readable number, not a float's full
/// noise. This is the **write-seam** half of the rule; the **read** half — how a stored `y`
/// (or its absence) is interpreted as an ordering key — is `model::y_key`.
fn clamp_y(y: f64) -> f64 {
    (y.clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0
}

/// Read one posted body into the command it names, or say why it is none. An absent `kind` means
/// `comment` — what a bare `{elemId, text}` has always meant.
pub fn parse_command(v: &Json) -> Result<Command, Rejection> {
    let kind = v.get_str("kind").unwrap_or("comment");
    match kind {
        "add" | "region-add" | "phase-split" => parse_mint(v, kind).map(Command::Mint),
        _ => parse_fold(v, kind).map(Command::Fold),
    }
}

/// The three creation commands.
fn parse_mint(v: &Json, kind: &str) -> Result<Mint, Rejection> {
    match kind {
        "add" => Ok(Mint::Add {
            lane: lane(v)?,
            label: label(v, "add: a sticky needs a non-blank label")?,
            col: v.get_i64("col"),
            detail: v
                .get_str("detail")
                .filter(|s| !s.is_empty())
                .map(String::from),
            prepend: v.get("prepend").and_then(Json::as_bool).unwrap_or(false),
        }),
        "region-add" => {
            let label = label(v, "region-add: a region needs a non-blank label")?;
            let (from_col, to_col) = span(v, "region-add")?;
            Ok(Mint::RegionAdd {
                label,
                from_col,
                to_col,
            })
        }
        _ => Ok(Mint::PhaseSplit {
            id: region_id(v, "phase-split: no regionId to split")?,
            at_col: v
                .get_i64("atCol")
                .ok_or(Rejection::Malformed("phase-split: no atCol to split at"))?,
            label: label(v, "phase-split: the right half needs a non-blank label")?,
        }),
    }
}

/// Everything else.
fn parse_fold(v: &Json, kind: &str) -> Result<Fold, Rejection> {
    match kind {
        "connect" | "disconnect" => {
            let (src, dst) = endpoints(v)?;
            Ok(if kind == "connect" {
                Fold::Connect { src, dst }
            } else {
                Fold::Disconnect { src, dst }
            })
        }
        "region-resize" => {
            let id = region_id(v, "region-resize: no regionId to resize")?;
            let (from_col, to_col) = span(v, "region-resize")?;
            Ok(Fold::RegionResize {
                id,
                from_col,
                to_col,
            })
        }
        "frontier-move" => Ok(Fold::FrontierMove {
            id: region_id(v, "frontier-move: no regionId to re-border")?,
            edge: frontier(v)?,
            col: v
                .get_i64("col")
                .ok_or(Rejection::Malformed("frontier-move: no col to move to"))?,
        }),
        "region-rename" => Ok(Fold::RegionRename {
            id: region_id(v, "region-rename: no regionId to rename")?,
            label: label(v, "region-rename: a region needs a non-blank label")?,
        }),
        "region-remove" => Ok(Fold::RegionRemove {
            id: region_id(v, "region-remove: no regionId to remove")?,
        }),
        "move" => parse_move(v),
        "rename" => Ok(Fold::Rename {
            id: elem_id(v, "rename: no elemId to rename")?,
            label: label(v, "rename: an element needs a non-blank label")?,
        }),
        "resolve" => Ok(Fold::Resolve {
            id: elem_id(v, "resolve: no elemId to resolve")?,
            resolution: text(v),
        }),
        "drop" => Ok(Fold::Drop {
            id: elem_id(v, "drop: no elemId to remove")?,
        }),
        "comment" | "question" | "split" => Ok(Fold::Annotate {
            id: elem_id(v, "comment: no elemId to annotate")?,
            text: text(v),
        }),
        other => Err(Rejection::UnknownKind(other.to_string())),
    }
}

/// A `move`'s target — refused when it names none, which would replay as a phantom move.
fn parse_move(v: &Json) -> Result<Fold, Rejection> {
    let id = elem_id(v, "move: no elemId to move")?;
    let col = v.get_i64("col");
    let y = v
        .get("y")
        .and_then(Json::as_f64)
        .filter(|y| y.is_finite())
        .map(clamp_y);
    if col.is_none() && y.is_none() {
        return Err(Rejection::Malformed(
            "move: neither a col nor a y to move to",
        ));
    }
    // A swap needs both halves, and a sticky cannot displace itself.
    let swap = match (v.get_str("swapId"), v.get_i64("swapCol")) {
        (Some(sid), Some(scol)) if sid != id => Some((sid.to_string(), scol)),
        _ => None,
    };
    Ok(Fold::Move { id, col, y, swap })
}

fn text(v: &Json) -> String {
    v.get_str("text").unwrap_or("").to_string()
}

fn label(v: &Json, why: &'static str) -> Result<String, Rejection> {
    v.get_str("text")
        .and_then(nonblank)
        .ok_or(Rejection::Malformed(why))
}

fn elem_id(v: &Json, why: &'static str) -> Result<String, Rejection> {
    v.get_str("elemId")
        .map(str::to_string)
        .ok_or(Rejection::Malformed(why))
}

fn region_id(v: &Json, why: &'static str) -> Result<String, Rejection> {
    v.get_str("regionId")
        .map(str::to_string)
        .ok_or(Rejection::Malformed(why))
}

fn lane(v: &Json) -> Result<Lane, Rejection> {
    v.get_str("type")
        .and_then(lane_from_str)
        .ok_or(Rejection::Malformed(
            "add: `type` must name one of the eight lanes",
        ))
}

fn span(v: &Json, kind: &'static str) -> Result<(i64, i64), Rejection> {
    let why = match kind {
        "region-add" => Rejection::Malformed("region-add: needs a well-ordered [fromCol, toCol]"),
        _ => Rejection::Malformed("region-resize: needs a well-ordered [fromCol, toCol]"),
    };
    match (v.get_i64("fromCol"), v.get_i64("toCol")) {
        (Some(from_col), Some(to_col)) if valid_span(from_col, to_col) => Ok((from_col, to_col)),
        _ => Err(why),
    }
}

/// Both endpoints present, non-blank and distinct. A self-loop has no rendered path and no place
/// in the grammar, and `replay` has no guard against one.
fn endpoints(v: &Json) -> Result<(String, String), Rejection> {
    let blank = Rejection::Malformed("connect: both `src` and `dst` must name an element");
    let (Some(src), Some(dst)) = (
        v.get_str("src").and_then(nonblank),
        v.get_str("dst").and_then(nonblank),
    ) else {
        return Err(blank);
    };
    if src == dst {
        return Err(Rejection::Malformed(
            "connect: an element cannot link to itself",
        ));
    }
    Ok((src, dst))
}

fn frontier(v: &Json) -> Result<Frontier, Rejection> {
    match v.get_str("edge") {
        Some("start") => Ok(Frontier::Start),
        Some("end") => Ok(Frontier::End),
        _ => Err(Rejection::Malformed(
            "frontier-move: `edge` must be \"start\" or \"end\"",
        )),
    }
}

/// The events a [`Fold`] persists — total, because every guard ran at the parse. A `Move` that
/// displaces an occupant yields two `ElementMoved`s, so the swap round-trips.
pub fn fold_to_events(cmd: &Fold) -> Vec<Event> {
    match cmd {
        Fold::Move { id, col, y, swap } => {
            let mut evs = vec![Event::ElementMoved {
                id: id.clone(),
                col: *col,
                kind: None,
                y: *y,
            }];
            if let Some((swap_id, swap_col)) = swap {
                evs.push(Event::ElementMoved {
                    id: swap_id.clone(),
                    col: Some(*swap_col),
                    kind: None,
                    y: None,
                });
            }
            evs
        }
        Fold::Rename { id, label } => vec![Event::ElementRenamed {
            id: id.clone(),
            label: label.clone(),
        }],
        Fold::Resolve { id, resolution } => vec![Event::HotspotResolved {
            id: id.clone(),
            resolution: resolution.clone(),
        }],
        Fold::Drop { id } => vec![Event::ElementRemoved { id: id.clone() }],
        Fold::Annotate { id, text } => vec![Event::ElementAnnotated {
            id: id.clone(),
            text: text.clone(),
        }],
        Fold::Connect { src, dst } => vec![Event::EdgeAdded {
            src: src.clone(),
            dst: dst.clone(),
            label: None,
        }],
        Fold::Disconnect { src, dst } => vec![Event::EdgeRemoved {
            src: src.clone(),
            dst: dst.clone(),
        }],
        Fold::RegionResize {
            id,
            from_col,
            to_col,
        } => vec![Event::PhaseResized {
            id: id.clone(),
            from_col: *from_col,
            to_col: *to_col,
        }],
        Fold::FrontierMove { id, edge, col } => vec![Event::FrontierMoved {
            id: id.clone(),
            edge: edge.as_str().to_string(),
            col: *col,
        }],
        Fold::RegionRename { id, label } => vec![Event::PhaseRenamed {
            id: id.clone(),
            label: label.clone(),
        }],
        Fold::RegionRemove { id } => vec![Event::PhaseRemoved { id: id.clone() }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::testutil::*;
    use crate::events::*;
    use crate::json;
    use proptest::prelude::*;

    fn parse(body: &str) -> Result<Command, Rejection> {
        parse_command(&json::parse(body).unwrap())
    }

    /// The events a posted body persists, the way `serve` gets them.
    fn evs(body: &str) -> Vec<Event> {
        match parse(body) {
            Ok(Command::Fold(cmd)) => fold_to_events(&cmd),
            _ => Vec::new(),
        }
    }

    fn why(body: &str) -> String {
        parse(body).expect_err("expected a rejection").to_string()
    }

    #[test]
    fn a_body_naming_no_kind_is_a_comment() {
        assert_eq!(
            parse(r#"{"elemId":"E1","text":"hm"}"#),
            Ok(Command::Fold(Fold::Annotate {
                id: "E1".into(),
                text: "hm".into()
            }))
        );
    }

    #[test]
    fn the_three_creation_kinds_parse_as_mints() {
        assert!(matches!(
            parse(r#"{"kind":"add","type":"event","text":"Paid"}"#),
            Ok(Command::Mint(Mint::Add {
                lane: Lane::Event,
                ..
            }))
        ));
        assert!(matches!(
            parse(r#"{"kind":"region-add","text":"Checkout","fromCol":0,"toCol":3}"#),
            Ok(Command::Mint(Mint::RegionAdd { .. }))
        ));
        assert!(matches!(
            parse(r#"{"kind":"phase-split","regionId":"K1","atCol":2,"text":"Right"}"#),
            Ok(Command::Mint(Mint::PhaseSplit { .. }))
        ));
    }

    #[test]
    fn add_needs_a_lane_in_the_grammar_and_a_non_blank_label() {
        // An off-grammar lane would mint into a real lane's id space — `epic` starts with the
        // same letter as `event`, so the fallback prefix this refuses was a live id collision.
        assert!(parse(r#"{"kind":"add","type":"epic","text":"Saga"}"#).is_err());
        assert!(parse(r#"{"kind":"add","text":"Saga"}"#).is_err());
        assert!(parse(r#"{"kind":"add","type":"event","text":"   "}"#).is_err());
    }

    #[test]
    fn a_span_must_be_well_ordered() {
        for body in [
            r#"{"kind":"region-add","text":"X","fromCol":5,"toCol":2}"#,
            r#"{"kind":"region-add","text":"X","fromCol":4,"toCol":4}"#,
            r#"{"kind":"region-add","text":"X","fromCol":0}"#,
            r#"{"kind":"region-resize","regionId":"K1","fromCol":9,"toCol":2}"#,
            r#"{"kind":"region-resize","regionId":"K1","fromCol":3,"toCol":3}"#,
            r#"{"kind":"region-resize","regionId":"K1"}"#,
        ] {
            assert!(parse(body).is_err(), "{body}");
        }
    }

    #[test]
    fn phase_split_needs_a_region_an_at_col_and_a_label() {
        for body in [
            r#"{"kind":"phase-split","atCol":2,"text":"R"}"#,
            r#"{"kind":"phase-split","regionId":"K1","text":"R"}"#,
            r#"{"kind":"phase-split","regionId":"K1","atCol":2}"#,
        ] {
            assert!(parse(body).is_err(), "{body}");
        }
    }

    #[test]
    fn region_edits_key_off_region_id_not_elem_id() {
        assert_eq!(
            evs(r#"{"kind":"region-resize","regionId":"K1","fromCol":0,"toCol":5}"#),
            vec![Event::PhaseResized {
                id: "K1".into(),
                from_col: 0,
                to_col: 5
            }]
        );
        assert_eq!(
            evs(r#"{"kind":"region-rename","regionId":"K1","text":"Fulfillment"}"#),
            vec![Event::PhaseRenamed {
                id: "K1".into(),
                label: "Fulfillment".into()
            }]
        );
        assert_eq!(
            evs(r#"{"kind":"region-remove","regionId":"K1"}"#),
            vec![Event::PhaseRemoved { id: "K1".into() }]
        );
        assert!(parse(r#"{"kind":"region-remove","elemId":"E1"}"#).is_err());
        assert!(parse(r#"{"kind":"region-rename","regionId":"K1","text":"   "}"#).is_err());
    }

    #[test]
    fn frontier_move_needs_a_named_border_and_a_col() {
        assert_eq!(
            evs(r#"{"kind":"frontier-move","regionId":"K1","edge":"end","col":5}"#),
            vec![Event::FrontierMoved {
                id: "K1".into(),
                edge: "end".into(),
                col: 5
            }]
        );
        assert!(
            parse(r#"{"kind":"frontier-move","regionId":"K1","edge":"sideways","col":5}"#).is_err()
        );
        assert!(parse(r#"{"kind":"frontier-move","regionId":"K1","edge":"end"}"#).is_err());
    }

    #[test]
    fn a_move_carrying_neither_a_col_nor_a_y_is_refused() {
        assert!(parse(r#"{"elemId":"E1","kind":"move"}"#).is_err());
        assert_eq!(evs(r#"{"elemId":"E1","kind":"move","col":2}"#).len(), 1);
        assert!(matches!(
            &evs(r#"{"elemId":"E1","kind":"move","y":0.6}"#)[..],
            [Event::ElementMoved { col: None, y: Some(y), .. }] if *y == 0.6
        ));
    }

    #[test]
    fn a_move_clamps_and_rounds_its_y() {
        for (posted, stored) in [("1.7", 1.0), ("-0.3", 0.0), ("0.333333333333", 0.3333)] {
            let body = format!(r#"{{"elemId":"E1","kind":"move","y":{posted}}}"#);
            assert!(
                matches!(&evs(&body)[..], [Event::ElementMoved { y: Some(y), .. }] if *y == stored),
                "posted {posted}"
            );
        }
    }

    #[test]
    fn a_move_into_an_occupied_column_persists_both_stickies() {
        // Only logging the primary move reverts the partner on the next replay, and the two
        // stickies then overlap.
        assert!(matches!(
            &evs(r#"{"elemId":"E1","kind":"move","col":3,"swapId":"E2","swapCol":1}"#)[..],
            [
                Event::ElementMoved { id: a, col: Some(3), .. },
                Event::ElementMoved { id: b, col: Some(1), .. },
            ] if a == "E1" && b == "E2"
        ));
    }

    #[test]
    fn a_self_swap_or_a_swap_missing_its_col_is_dropped_not_persisted() {
        assert_eq!(
            evs(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E1","swapCol":0}"#).len(),
            1
        );
        assert_eq!(
            evs(r#"{"elemId":"E1","kind":"move","col":2,"swapId":"E2"}"#).len(),
            1
        );
    }

    #[test]
    fn a_label_is_trimmed_and_never_blank() {
        for blank in ["", "   ", "\t", "\n  "] {
            let body = format!(r#"{{"elemId":"E1","kind":"rename","text":{blank:?}}}"#);
            assert!(parse(&body).is_err(), "blank rename {blank:?}");
        }
        assert_eq!(
            evs(r#"{"elemId":"E1","kind":"rename","text":"  PaymentTaken  "}"#),
            vec![Event::ElementRenamed {
                id: "E1".into(),
                label: "PaymentTaken".into()
            }]
        );
    }

    #[test]
    fn a_resolution_may_be_empty_because_closing_is_the_act() {
        assert_eq!(
            evs(r#"{"elemId":"H1","kind":"resolve"}"#),
            vec![Event::HotspotResolved {
                id: "H1".into(),
                resolution: String::new()
            }]
        );
    }

    #[test]
    fn an_edge_needs_two_present_distinct_non_blank_endpoints() {
        for body in [
            r#"{"kind":"connect","src":"E1","dst":"E1"}"#,
            r#"{"kind":"connect","src":"E1"}"#,
            r#"{"kind":"disconnect","dst":"E2"}"#,
            r#"{"kind":"connect","src":"","dst":"E2"}"#,
            r#"{"kind":"connect","src":"E1","dst":"  "}"#,
        ] {
            assert!(parse(body).is_err(), "{body}");
        }
        assert_eq!(
            evs(r#"{"kind":"connect","src":" E1 ","dst":"E2 "}"#),
            vec![Event::EdgeAdded {
                src: "E1".into(),
                dst: "E2".into(),
                label: None
            }]
        );
        assert_eq!(
            evs(r#"{"kind":"disconnect","src":"E1","dst":"E2"}"#),
            vec![Event::EdgeRemoved {
                src: "E1".into(),
                dst: "E2".into()
            }]
        );
    }

    #[test]
    fn an_element_op_naming_no_element_is_refused() {
        for body in [
            r#"{"kind":"rename","text":"X"}"#,
            r#"{"kind":"drop"}"#,
            r#"{"kind":"move","col":1}"#,
            r#"{"kind":"comment","text":"hi"}"#,
        ] {
            assert!(parse(body).is_err(), "{body}");
        }
    }

    #[test]
    fn a_kind_this_build_has_no_command_for_is_refused_by_name() {
        // It used to be stored as an annotation on the element: a client typo, or a newer
        // client's op, came back 200 and turned into a comment nobody wrote.
        assert_eq!(
            parse(r#"{"elemId":"E1","kind":"renmae","text":"Paid"}"#),
            Err(Rejection::UnknownKind("renmae".into()))
        );
        // The three the modal offers still annotate, and so does a body naming no kind.
        for kind in ["comment", "question", "split"] {
            let body = format!(r#"{{"elemId":"E1","kind":"{kind}","text":"hm"}}"#);
            assert_eq!(
                evs(&body),
                vec![Event::ElementAnnotated {
                    id: "E1".into(),
                    text: "hm".into()
                }],
                "{kind}"
            );
        }
    }

    #[test]
    fn a_rejection_says_which_command_and_why() {
        // The server prints this to its console; "rename: …" and "no command named …" send the
        // operator to two different places.
        assert!(why(r#"{"elemId":"E1","kind":"rename","text":" "}"#).starts_with("rename:"));
        assert!(why(r#"{"kind":"add","type":"epic","text":"S"}"#).starts_with("add:"));
        assert_eq!(
            why(r#"{"elemId":"E1","kind":"nope"}"#),
            r#"no command named "nope""#
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// No sequence of posted comments ever leaves an element with a blank label — the guard
        /// that keeps a rename from emptying a box permanently.
        #[test]
        fn pbt_no_comment_sequence_ever_leaves_a_blank_label(
            comments in prop::collection::vec(comment_strategy(), 1..=8),
        ) {
            let (mut log, _ids) = genesis();
            for v in &comments {
                log.extend(posted(v));
            }
            for e in &replay(&log).elements {
                prop_assert!(!e.label.trim().is_empty(), "element {} got a blank label", e.id);
            }
        }
    }
}
