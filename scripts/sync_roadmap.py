#!/usr/bin/env python3
"""One-way roadmap reconciler: GitHub Project #2 + issues → ROADMAP.md.

The board is canonical for **priority / order / Horizon**; ROADMAP.md is canonical for the
**narrative** (the per-row `Summary` prose). This script closes the one gap that can't be edited
visually: it rewrites only the `Status` and `Horizon` **columns** of each tracked feature row from
the board, leaving every word of `Summary` untouched.

Rules (deliberately conservative — never lose human intent):
  * A row is touched only if its Summary carries a primary ``Tracked #N`` **and** its current
    Status is not ``✅`` (Shipped is terminal; a shipped row that merely *cites* a follow-up issue
    is never reverted).
  * A column is overwritten only when the board actually holds a value for it, so running this
    before the board's Horizon field is populated is a safe no-op on that column.

Usage:
  sync_roadmap.py            # rewrite ROADMAP.md in place
  sync_roadmap.py --check    # report drift, write nothing, exit 1 if anything would change
                             #   or any validity warning fires

Needs `gh` on PATH, authenticated, with the `project` scope.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

OWNER = "bastien-gallay"
REPO = "bastien-gallay/faceto"
PROJECT = "2"
ROADMAP = Path(__file__).resolve().parent.parent / "ROADMAP.md"

# board Status single-select  -> roadmap Status glyph
STATUS_GLYPH = {"Done": "✅", "In Progress": "🚧", "Todo": "☐"}
# board Horizon single-select -> roadmap Horizon cell
HORIZON_CELL = {
    "Now": "**Now**",
    "Next": "Next",
    "Later": "Later",
    "Parked": "Parked",
    "Shipped": "✅ Shipped",
}
TRACKED_RE = re.compile(r"Tracked:?\s+#(\d+)")
ANY_REF_RE = re.compile(r"#(\d+)")


def gh_json(args: list[str]) -> dict | list:
    out = subprocess.run(["gh", *args], capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def _reason(exc: Exception) -> str:
    """A short, honest cause string for a failed `gh` fetch — so the degradation message
    reflects what actually happened (missing scope vs no network vs no `gh`) instead of
    always blaming `project` scope."""
    if isinstance(exc, FileNotFoundError):
        return "`gh` not found on PATH"
    if isinstance(exc, subprocess.CalledProcessError):
        err = (exc.stderr or "").strip().splitlines()
        tail = err[-1] if err else ""
        low = tail.lower()
        if "project" in low and "scope" in low:
            return "token lacks `project` scope"
        if "tls" in low or "certificate" in low or "dial tcp" in low or "connection" in low:
            return f"network/sandbox blocked `gh` ({tail})"
        return tail or "`gh` exited non-zero"
    if isinstance(exc, json.JSONDecodeError):
        return "`gh` returned non-JSON output"
    return str(exc)


def board_by_issue() -> tuple[dict[int, dict] | None, str]:
    """(issue number -> {'status', 'horizon'} from Project #2, ""), or (None, reason).

    Reading a *user* project needs a token with `project` scope, which CI's default
    GITHUB_TOKEN lacks — so a failure here is expected in CI and degrades to skipping the
    column-sync dimension rather than failing the check.
    """
    try:
        data = gh_json(
            ["project", "item-list", PROJECT, "--owner", OWNER, "--format", "json", "--limit", "300"]
        )
    except (subprocess.CalledProcessError, json.JSONDecodeError, OSError) as exc:
        # OSError covers a missing `gh` (FileNotFoundError) — degrade, don't crash.
        return None, _reason(exc)
    out: dict[int, dict] = {}
    for it in data.get("items", []):
        num = (it.get("content") or {}).get("number")
        if num is None:
            continue
        out[int(num)] = {"status": it.get("status"), "horizon": it.get("horizon")}
    return out, ""


def open_issue_numbers() -> tuple[set[int] | None, str]:
    """(open issue numbers, ""), or (None, reason) if unreachable (repo-scoped token suffices)."""
    try:
        data = gh_json(
            ["issue", "list", "--repo", REPO, "--state", "open", "--limit", "300", "--json", "number"]
        )
    except (subprocess.CalledProcessError, json.JSONDecodeError, OSError) as exc:
        # OSError covers a missing `gh` (FileNotFoundError) — degrade, don't crash.
        return None, _reason(exc)
    return {int(i["number"]) for i in data}, ""


def split_row(line: str) -> list[str] | None:
    """A feature row is `| F-... | Direction | Status | Horizon | Summary |`."""
    if not line.startswith("| F-"):
        return None
    # keep the trailing newline off; rebuild exactly with ' | ' joins + outer pipes
    body = line.rstrip("\n")
    # Cap at 4 splits (5 cells) so an escaped pipe (`\|`) in the Summary stays inside the
    # Summary cell instead of over-splitting the row into >5 cells and being silently dropped.
    cells = [c.strip() for c in body.strip().strip("|").split("|", 4)]
    return cells if len(cells) == 5 else None


def rebuild_row(cells: list[str]) -> str:
    return "| " + " | ".join(cells) + " |\n"


def selftest() -> int:
    """Parser regression guards — run offline, no network. Exit 0 on pass, 1 on failure."""
    esc = (
        "| F-es-lint | linting | ✅ | ✅ Shipped | the `level: big-picture \\| design` knob, "
        "keyed on #14 and #19. |\n"
    )
    cases = [
        # (line, expected cells or None)
        (esc, 5),  # an escaped pipe in Summary must NOT over-split the row
        ("| F-x | UI | ☐ | Next | plain summary Tracked #99. |\n", 5),
        ("not a row\n", None),
        ("| header | not | a | feature | row |\n", None),  # doesn't start with `| F-`
    ]
    fails = []
    for line, want in cases:
        got = split_row(line)
        n = None if got is None else len(got)
        if n != want:
            fails.append(f"split_row expected {want} cells, got {n}: {line!r}")
    # the escaped-pipe row's Summary must round-trip with its pipe intact and its refs readable
    cells = split_row(esc)
    if not cells:
        fails.append("escaped-pipe row failed to parse")
    else:
        if "\\|" not in cells[4]:
            fails.append("escaped pipe lost from Summary cell")
        refs = {int(x) for x in ANY_REF_RE.findall(cells[4])}
        if not {14, 19} <= refs:
            fails.append(f"issue refs not recovered from escaped-pipe Summary: {refs}")

    # `_reason` must name the real cause, not always blame `project` scope
    def _cpe(stderr: str) -> subprocess.CalledProcessError:
        e = subprocess.CalledProcessError(1, ["gh"])
        e.stderr = stderr
        return e

    reason_cases = [
        (FileNotFoundError(), "not found"),
        (_cpe("error: your token has not been granted the required scopes: project"), "scope"),
        (_cpe('Post "https://api.github.com/graphql": tls: failed to verify certificate'), "network"),
    ]
    for exc, want in reason_cases:
        got = _reason(exc)
        if want not in got:
            fails.append(f"_reason({exc!r}) = {got!r}, expected to contain {want!r}")

    for f in fails:
        print("selftest FAIL:", f)
    print("selftest OK" if not fails else f"selftest: {len(fails)} failure(s)")
    return 1 if fails else 0


def main() -> int:
    if "--selftest" in sys.argv[1:]:
        return selftest()
    check = "--check" in sys.argv[1:]
    board, board_err = board_by_issue()
    open_issues, issues_err = open_issue_numbers()
    if board is None:
        print(f"· board unreachable ({board_err}) — skipping column sync.")
    if open_issues is None:
        print(f"· issues unreachable ({issues_err}) — skipping orphan-issue check.")

    lines = ROADMAP.read_text().splitlines(keepends=True)
    referenced: set[int] = set()
    live_rows_without_issue: list[str] = []
    changes: list[str] = []
    new_lines: list[str] = []

    for line in lines:
        cells = split_row(line)
        if cells is None:
            new_lines.append(line)
            continue
        fid, _direction, status, horizon, summary = cells
        referenced.update(int(n) for n in ANY_REF_RE.findall(summary))

        m = TRACKED_RE.search(summary)
        tracked = int(m.group(1)) if m else None

        # validity: a *committed* row (Now / Next) must have a tracker; Later/Parked may defer
        # minting until promoted, so they are not treated as drift.
        if tracked is None and horizon in ("**Now**", "Next"):
            live_rows_without_issue.append(f"{fid} (Horizon: {horizon})")

        if board and tracked is not None and status != "✅" and tracked in board:
            b = board[tracked]
            new_status = STATUS_GLYPH.get(b["status"] or "", status)
            new_horizon = HORIZON_CELL.get(b["horizon"] or "", horizon)
            if new_status != status or new_horizon != horizon:
                changes.append(
                    f"{fid} #{tracked}: "
                    f"[{status} | {horizon}] -> [{new_status} | {new_horizon}]"
                )
                cells[2], cells[3] = new_status, new_horizon
            new_lines.append(rebuild_row(cells))
        else:
            new_lines.append(line)

    orphan_open = sorted(open_issues - referenced) if open_issues is not None else []

    # ---- report ----
    if changes:
        print("column updates:" if not check else "would update:")
        for c in changes:
            print("  " + c)
    else:
        print("columns already in sync.")
    if live_rows_without_issue:
        print("\n⚠ live rows with no `Tracked #N` (mint an issue or mark Parked):")
        for r in live_rows_without_issue:
            print("  " + r)
    if orphan_open:
        print("\n⚠ open issues not referenced by any row (triage into a row):")
        for n in orphan_open:
            print(f"  #{n}")

    if check:
        drift = bool(changes or live_rows_without_issue or orphan_open)
        return 1 if drift else 0

    if changes:
        ROADMAP.write_text("".join(new_lines))
        print(f"\n✓ wrote {len(changes)} row(s) to {ROADMAP.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
