#!/usr/bin/env python3
"""Guard the two hand-maintained keyboard sheets against drift.

The board's gestures are listed twice, by hand, with no generator between them:

  * ``src/template.html`` — the in-app ``#help`` dialog (press ``?`` on a board)
  * ``docs/src/board/keyboard.md`` — the published reference page

``AGENTS.md`` names this as a trap the project has already fallen into: change a binding in one
place and the other silently lies. A full generator is the eventual fix; this check is the cheap
durable one. It compares the *key tokens* — what is wrapped in ``<kbd>`` — in both directions, so
adding a binding to the app without documenting it fails, and removing one from the app while the
book still promises it fails too.

It deliberately does **not** compare the descriptions. The app sheet is terse by design and the
book page expands; forcing them to match word for word would make the check a nuisance and get it
deleted. Keys are the part that must not drift.

Asymmetries that are real and intended are declared below, one constant each. Adding to those sets
is a decision, not a workaround — write down why.

Usage:  python3 scripts/check_keyboard_sheet.py
Exit:   0 when the two sheets agree, 1 on drift (the differences are printed).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "src" / "template.html"
BOOK = ROOT / "docs" / "src" / "board" / "keyboard.md"

# The in-app sheet lives inside this dialog; the rest of the template has no <kbd> today, but
# scoping the scan keeps that from becoming a silent dependency.
DIALOG = re.compile(r'<dialog id="help".*?</dialog>', re.DOTALL)
KBD = re.compile(r"<kbd>(.*?)</kbd>", re.DOTALL)

# Tokens the app sheet uses as *group labels* rather than keys — they head a row that bundles
# several gestures, and the book gives each its own section instead.
APP_ONLY = {"region"}

# Keys the book documents that the app sheet folds into a neighbouring row:
#   Backspace — the app row reads "Delete" alone; both are bound to remove.
BOOK_ONLY = {"Backspace"}


def tokens(text: str) -> set[str]:
    """The key tokens in a chunk of markup.

    ``<kbd>⌘/Ctrl</kbd>`` and ``<kbd>⌘</kbd>/<kbd>Ctrl</kbd>`` are the same two keys written two
    ways, so a slash splits. Whitespace is normalised; empty pieces are dropped.
    """
    found: set[str] = set()
    for raw in KBD.findall(text):
        for piece in raw.split("/"):
            key = " ".join(piece.split())
            if key:
                found.add(key)
    return found


def main() -> int:
    app_src = APP.read_text(encoding="utf-8")
    dialog = DIALOG.search(app_src)
    if not dialog:
        print(f"keyboard-check FAIL: no `#help` dialog found in {APP.relative_to(ROOT)}")
        return 1

    app = tokens(dialog.group(0))
    book = tokens(BOOK.read_text(encoding="utf-8"))
    if not app or not book:
        print("keyboard-check FAIL: one of the sheets has no <kbd> tokens at all")
        return 1

    undocumented = app - book - APP_ONLY
    stale = book - app - BOOK_ONLY
    if not undocumented and not stale:
        print(f"keyboard-check OK: {len(app)} keys, both sheets agree")
        return 0

    print("keyboard-check FAIL: the in-app sheet and the book page disagree.")
    if undocumented:
        print(
            f"  in {APP.relative_to(ROOT)} but not {BOOK.relative_to(ROOT)}: "
            + ", ".join(sorted(undocumented))
        )
        print("    → document the binding in the book, or drop it from the app sheet.")
    if stale:
        print(
            f"  in {BOOK.relative_to(ROOT)} but not {APP.relative_to(ROOT)}: "
            + ", ".join(sorted(stale))
        )
        print("    → the book promises a key the app no longer offers, or the app sheet lost it.")
    print("  (A genuinely one-sided key belongs in APP_ONLY / BOOK_ONLY, with a reason.)")
    return 1


if __name__ == "__main__":
    sys.exit(main())
