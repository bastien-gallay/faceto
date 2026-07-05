# Coding Standards

This file is the working agreement for code in this repo. It is meant to be
re-read on a slow day, not skimmed once. Four pillars, in the order you usually
apply them — on top of one hard constraint that frames all of them:

0. **Zero runtime dependencies** — the shipped binary is pure Rust std, no runtime crates ever
   (test-only dev-dependencies are the one exception).
1. **Tidy First** — separate behaviour changes from clean-ups.
2. **CUPID & YAGNI** — properties to aim for in design and refactoring.
3. **TDD (Red → Green → Refactor → Reflect)** — the loop that keeps the above
   honest.
4. **Clean Code** — local taste rules that survive automation.

Repo-specific rules take precedence when they collide. The authoritative
sources, in order, are [`AGENTS.md`](AGENTS.md) (the canonical guide — Claude Code
reads it via `@AGENTS.md` in `CLAUDE.md`) and [`DESIGN.md`](DESIGN.md) /
[`PRODUCT.md`](PRODUCT.md) for UI work. This file expands on the *how*; those
define the *what* and *why*.

---

## 0. Zero dependencies (the hard constraint)

> *A simple typed file you think through with an LLM — and a tool that installs
> offline in one `cargo install`.*

faceto's shipped binary is **pure Rust standard library — no runtime crates,
ever.** This is a product decision (trivial, offline install), not an accident,
and it is enforced by the `zero dependencies` CI job (it fails if any crate
enters the *normal* dependency tree, via `cargo tree -e normal`). **Dev-
dependencies are the one exception** — test-only crates (`proptest`, for the
property-based tests) never enter the binary or the install, so they're allowed;
ask before adding one.

Consequences you must respect:

- JSON is parsed/serialized by the hand-written `src/json.rs` (not serde).
- The HTTP server is `std::net::TcpListener` + threads (`src/serve.rs`), not a
  web framework.
- Dates (`now_iso`) and content hashing (`fnv12`, FNV-1a) are implemented by
  hand in `src/serve.rs`.

If a task seems to need a crate, implement it in `std` or push back. "Add a
crate to avoid twenty lines of `std`" is the wrong trade here, always.

---

## 1. Tidy First (Kent Beck)

> *Make the change easy, then make the easy change.*

Behaviour changes and structural changes are **two different commits**.

- **Tidying** — renames, extractions, dead-code removal, reformatting,
  splitting a long function, adding a missing test that pins existing
  behaviour. Never alters observable output (same SVG, same HTTP responses,
  same replayed `Model`).
- **Behaviour change** — the actual feature, fix, or contract change.

Rules of thumb:

- If the diff to add a feature feels too big, stop. Tidy the surrounding code
  first (in its own commit), then come back. The feature commit shrinks.
- Tests that pin existing behaviour are **must-have**, not nice-to-have. Land
  them *before* the behaviour change, so that change reads as a small,
  intentional diff.
- If a tidy ends up changing observable behaviour, it wasn't a tidy. Revert
  and split.

Acceptable commit shapes:

```text
✅  refactor(render): extract edge_path helper            (tidy)
    feat(events): append ElementMoved on a sticky move    (behaviour)

❌  feat(events): move stickies + tidy the render layout
```

---

## 2. CUPID & YAGNI

Five properties to optimise for, in roughly this order:

| Property            | One-liner                                               | Smell when violated                               |
| ------------------- | ------------------------------------------------------- | ------------------------------------------------- |
| **Composable**      | Plays well with others; small surface, no surprises.    | "I have to mock half the world to test this."     |
| **Unix philosophy** | Does one thing well.                                    | A file/function with `and` in its job statement.  |
| **Predictable**     | Behaves as expected; no hidden state, no spooky action. | "Works on my machine" / order-dependent tests.    |
| **Idiomatic**       | Reads like the language and the codebase.               | Reviewer says "this is clever" with a sigh.       |
| **Domain-based**    | Names match the product's vocabulary.                   | Generic `Manager`/`Helper`/`Util` names.          |

### C — Composable

The pipeline is `JSON file → Model → SVG → HTML`, and each stage plugs into the
next without reaching across it.

- `src/json.rs` knows nothing of boards; `src/model.rs` builds on `Json` but
  not on rendering; `src/render.rs` is pure layout over a `Model`; `src/serve.rs`
  wires them behind HTTP. `src/events.rs` `replay()` folds a log into a `Model`
  and depends on nothing downstream.
- The client (`src/template.html` shell + `src/client/*.js`) talks to the server
  only through the routes and the geometry it is handed (`__CONFIG__`); it never
  assumes server internals.

**Watch for**: the client or `serve.rs` re-deriving a layout/colour decision
that `render.rs` already owns. If two sites want the same derived value, hoist
it into the stage that owns it (e.g. geometry constants flow out of `render.rs`
once, into the page).

### U — Unix philosophy

Do one thing well — **one file, one stage.**

- `json` parses, `model` types & diffs, `lint` checks ES-grammar, `render` lays
  out & draws, `serve` serves, `events` is the log/replay spine, and the
  `template.html` shell + `client/*.js` are the client.
- `src/main.rs` is CLI dispatch only (`render` / `lint` / `serve` / `genesis` /
  `compact` / `help` / `version`) — no domain logic.

**Watch for**: a stage that "while we're here" takes on a neighbour's job. Push
the decision back to the stage that owns it.

### P — Predictable

Same input, same output, on any machine.

- `render_svg` and `events::replay` are **pure**: a given `Model` (or event log)
  yields the same SVG / same projection anywhere — no I/O, no clock, no panic.
- Deterministic ordering: order within a lane is sort-by-`col`; if order
  matters, sort explicitly — never rely on `HashMap`/`HashSet` iteration order.
- Broken invariants surface as `None`/`Err`, not via a panic in a shipped path.

Non-deterministic work (filesystem, sockets, time) lives in `serve.rs` /
`main.rs`, never in the pure stages.

### I — Idiomatic

Feels like modern Rust to a Rust reader, within the std-only constraint.

- Enums to make impossible states impossible: `Json`, `Event`,
  `Element`/`Edge`/`Phase` as typed structs, not stringly-typed bags.
- `Result<T, E>` and `Option<T>` over sentinel values; pattern matching over
  nested `if let`.
- Iterator chains when they read cleaner than a `for` loop; a `for` loop when
  they don't.
- Avoid `unwrap`/`expect` in shipped paths. Where one is genuinely safe (a
  layout invariant guarantees it, e.g. `e.col.unwrap()` after columns are
  assigned), keep it provably unreachable **and add a one-line comment** saying
  why. `panic!`/`todo!`/`unimplemented!` are clippy-warned and CI-blocking.

### D — Domain-based

The code speaks event storming: boards, stickies, lanes, columns, phases.

- Types map to the domain: `Model`, `Element`, `Edge`, `Phase`, `Event`.
- Names use the board's vocabulary — `col` (the global timeline coordinate),
  `lane`, `sticky`, `hotspot` — not data-structure names.
- Avoid `Manager`/`Helper`/`Util`. A name should say what it is in the domain.

### YAGNI (anti-speculation rule)

CUPID describes what good code *is*; YAGNI protects against building what you
don't need yet.

- No abstraction for a second board format that doesn't exist. Event storming is
  the first format; a concrete renderer is fine until the next format actually
  lands — don't pre-abstract for it.
- No trait with a single implementation just to look extensible.
- No field that duplicates information derivable from another (don't store what
  you can compute from `col` and the lane order; derive it).
- No crate, ever, to remove scaffolding that *is* the contract (see pillar 0).

When a refactor toward CUPID would require speculative work, stop and wait for
the second use case.

---

## 3. TDD with a fourth step — Reflect

The standard Red → Green → Refactor loop, with a deliberate **Reflect** beat at
the end of each cycle. Reflect is what keeps the loop from grinding out lots of
small green tests that don't add up to a coherent design.

```text
   ┌──────────┐
   │   RED    │   Write the smallest failing test that names the
   │          │   behaviour you want. Run it. Confirm it fails for
   │          │   the right reason (not a typo, not an import).
   └────┬─────┘
        │
        ▼
   ┌──────────┐
   │  GREEN   │   Write the least code that makes the test pass.
   │          │   Ugly is fine here. Don't generalise yet.
   └────┬─────┘
        │
        ▼
   ┌──────────┐
   │ REFACTOR │   With the test green, clean up — names, duplication,
   │          │   shape. Tests stay green between every keystroke.
   │          │   This is a TIDY (see §1); commit it separately.
   └────┬─────┘
        │
        ▼
   ┌──────────┐
   │ REFLECT  │   Pause. Ask:
   │          │     • What did this cycle teach me?
   │          │     • What surprised me (red took longer? green was
   │          │       trivial? refactor revealed a missing concept)?
   │          │     • Is the *next* test on my list still the right
   │          │       one, or did this cycle change the plan?
   │          │     • Is there a test I should retire because it now
   │          │       overlaps with a stronger one?
   │          │     • Did I learn a domain rule worth pinning in
   │          │       another test, separate from the one I just wrote?
   │          │   Update the test list. Then loop.
   └────┬─────┘
        │
        ▼
       (next test)
```

Reflect rules:

- **Reflect is short.** A minute, sometimes thirty seconds. If it becomes a
  meeting, do it asynchronously between cycles.
- **Reflect updates the plan, not the code.** If reflection reveals code that
  should change, that's the *next* RED test, not an edit smuggled into the
  current cycle.
- **Reflect after Green-but-no-Refactor cycles too.** "There was nothing to
  clean" is itself a signal.
- **Always surface findings to the user with a recommendation.** Every
  reflection that produces a finding gets a one-line decision prompt: *"apply
  now / add to today / add to the changelog or docs / forget it"*. Recommend the
  best move per the principles and say *why* in one short clause. Default leans
  toward *apply now* when the finding is small and directly tied to the cycle
  that surfaced it (Tidy First: keep the diff coherent); lean toward
  *docs/later* when it is larger than the cycle it interrupted (CUPID-Composable:
  don't bundle unrelated work).

### Testing in faceto

- **Unit tests** live in the same file, under `#[cfg(test)] mod tests`. The pure
  stages (`json`, `model`, `render`, `events`) have no I/O excuse — cover them
  exhaustively. A behaviour with an invariant (an id-keyed diff tag, a layout
  data-attribute, a replay rule) gets a positive, a negative, and an edge case.
- **Server helpers** that are pure (`fnv12`, the civil-date maths behind
  `now_iso`) are unit-tested in `src/serve.rs`.
- **Tests may use `unwrap`/`expect`/`panic`** — `clippy.toml` allows it in tests
  so the discipline doesn't fight the harness. Shipped paths may not.
- Run the suite with `cargo test --all-targets`. There is no external test
  framework — `std`'s is enough.
- For board behaviour not covered by tests, render `examples/sample.model.json`
  or run `serve` and interact.

---

## 4. Clean Code

Local taste rules. None are absolute; they exist to be broken *on purpose*, not
by accident.

### Names

- A name should let a reader skip the implementation. If they have to read the
  body to understand the name, rename it.
- Domain words beat generic ones (`hotspot`, not `flagged_item`).
- Boolean names read as predicates: `is_resolved`, `has_note`, `should_carry`.
- Types: `PascalCase`; functions/vars: `snake_case`; constants:
  `SCREAMING_SNAKE_CASE` (`COL_W`, `LANE_H`); enum variants: `PascalCase`;
  files: `snake_case.rs`.

### Functions

- One purpose per function. If you'd need "and" to describe it, split.
- Short by default — long when the alternative is a tangle of helpers no one
  reads in order.
- Arguments: 0–3 is fine; 4+ wants a struct.
- No flag argument that changes *what* the function does. A `dry_run: bool` that
  toggles a side-effect is fine; a `mode` that picks behaviour is usually two
  functions.

### Comments & Documentation

- Default to **no inline comments**. Code says *what*; commit messages say *why*.
- Write an inline comment only when the *why* is non-obvious: a hidden
  constraint, a surprising invariant, a workaround (e.g. why the client mirrors
  `render.rs`'s `edge_path` instead of round-tripping to the server).
- Public items get a doc comment: a one-line summary, then detail if needed.

### Errors

- Validate at boundaries (CLI args, file I/O). Trust internal callers.
- **Fail loudly and early.** A swallowed error is a future bug report. Never
  `let _ = some_result;` to silence a failure — surface it.
- Return `Result` from fallible work; the CLI/server boundary turns it into a
  clear message on stderr and a non-zero exit. There is **no logging stack** —
  std `eprintln!` at the boundary is the whole story (no `println!` for
  diagnostics, and none in the pure stages).

---

## Commit style

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>(<optional scope>): <short summary>

<optional body>

<optional footer>
```

Types:

- `feat`: new feature or capability
- `fix`: bug fix
- `docs`: documentation only
- `refactor`: code change that neither fixes a bug nor adds a feature (a tidy)
- `test`: adding or fixing tests
- `chore`: housekeeping
- `perf`: performance improvement
- `ci`: CI configuration
- `build`: build/tooling configuration

Scopes are a source stage or area: `json`, `model`, `render`, `serve`,
`events`, `template`, `ci`, `docs`.

Breaking changes include `BREAKING CHANGE:` in the footer or `!` after the type.
Commit messages carry **no "Claude" signature** (per the global user
instruction). Record user-visible changes under `## [Unreleased]` in
[`CHANGELOG.md`](CHANGELOG.md).

---

## Toolchain & Layout

### Layout

There is one binary crate. The "layout" is the pipeline, not a workspace — each
source file is exactly one stage:

```text
src/
├── json.rs       # hand-written JSON parser/serializer (the Json enum)
├── model.rs      # typed board: Model/Element/Edge/Phase, from_json, diff_models
├── lint.rs       # ES-grammar lint: lint(&Model) → Vec<Finding>, warn-only, pure (level-aware)
├── events.rs     # event log: Event enum, replay() → Model, from_model() genesis
├── render.rs     # pure layout + SVG (render_svg) and HTML wrapping (render_html)
├── serve.rs      # std-only HTTP server (TcpListener + threads)
├── template.html # the client's thin shell (placeholders), embedded via include_str! in render.rs
├── client/       # the client's CSS + JS modules, concat!'d into the shell at build (no bundler)
└── main.rs       # CLI dispatch only
```

When adding code, ask *which stage does this belong in?* If the answer is "a
pure stage should do I/O," the answer is wrong — keep I/O at the `serve`/`main`
boundary.

### Toolchain policy

- `rust-toolchain.toml` pins **exactly `1.95.0`, edition 2021**. Do not bump it
  without updating the pin and re-checking CI in the same commit.
- `Cargo.toml` declares `rust-version = "1.95"`. Keep it in lockstep with the
  toolchain pin and the CI `toolchain:` inputs; bumping one without the others
  is a bug.

### Lints

Declared in `Cargo.toml`:

- `[lints.rust]`: `unsafe_code = "forbid"` — a std-only tool never needs it.
- `[lints.clippy]`: `panic`, `todo`, `unimplemented` = `warn`.
- `clippy.toml` opts tests out of the unwrap/expect/panic rules.
- CI runs `cargo clippy --all-targets -- -D warnings`: **every warning is
  blocking.** Mirror it locally before pushing.

### CI gates (all blocking)

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
markdownlint-cli2                      # prose ≤100 cols; see .markdownlint-cli2.jsonc
# plus, in CI: actionlint, and the `zero dependencies` firewall job
```

These are the local mirrors of the CI checks. For the full pipeline — triggers,
path-based job gating, the per-OS coverage trade-off (ubuntu-only on PRs, macOS
on `main`, no Windows), the required-checks ruleset, and why required job names
must stay static — see [`docs/ci.md`](docs/ci.md).

Markdown prose wraps at **100 columns** (tables and code blocks are exempt).
This file obeys that rule; keep it that way when you edit. The local
[`.pre-commit-config.yaml`](.pre-commit-config.yaml) runs fmt, clippy,
markdownlint and `typos` before commit (tests on pre-push) — see
[`CONTRIBUTING.md`](CONTRIBUTING.md) to install it.

---

## Review mindset

When reviewing a change, ask:

1. Does it meet the design principles above?
2. Does it respect **zero runtime dependencies** (nothing new in the `cargo tree -e normal`
   graph; a dev-dependency is fine)?
3. Does it honour the three domain invariants — `id` is stable identity, `col`
   is a global timeline coordinate, `type` selects the lane and colour?
4. Are there impossible states the types now allow?
5. Is any error silently swallowed?
6. Is the logic tested, and is the documentation up to date — and would a future
   maintainer understand *why*, not just *what*?

Kindness over pedantry. The goal is a better codebase, not a perfect one.
