<!-- markdownlint-disable MD060 -->

# CI reference

The authoritative source is [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and the
`main-protection` branch ruleset on GitHub. This document is the *map*: what runs, when, why, and
how to reproduce or restore it. If the two ever disagree, the workflow file wins — fix this doc.

CI has one job: keep `main` green without making the feedback loop slow. Every design choice below
serves that, and several are deliberate trade-offs for a **solo project** (see
[Platform coverage](#platform-coverage-a-deliberate-gap)).

---

## Triggers

| Event               | When it fires                          | What it gates                              |
| ------------------- | -------------------------------------- | ------------------------------------------ |
| `pull_request`      | PR opened/updated targeting `main`     | The merge gate (required checks below)     |
| `push`              | Commits land on `main`                 | Post-merge confirmation + macOS coverage   |
| `workflow_dispatch` | Manual run from the Actions tab        | Ad-hoc full run (behaves like a `push`)    |

**Concurrency.** Runs are grouped by `workflow + ref`. A new push to a PR **cancels** the PR's
in-flight run (`cancel-in-progress` is true only for `pull_request`), so a fresh push never queues
behind its own stale jobs. Pushes to `main` are **never** cancelled — those runs gate the default
branch and must always complete.

**Permissions.** The workflow is least-privilege: `contents: read` at the top level. Only the
`detect changes` job additionally requests `pull-requests: read` (the path filter needs it to read
the PR's file list).

**Shared env.** `RUSTFLAGS: -D warnings` (warnings fail the build), `RUST_BACKTRACE: 1`,
`CARGO_TERM_COLOR: always`.

---

## Path gating (run only what's relevant)

A first job, `detect changes`, classifies the diff with
[`dorny/paths-filter`](https://github.com/dorny/paths-filter) and exposes three boolean outputs.
Every other job carries `needs: changes` + an `if:` on the relevant output, so a docs-only change
skips the Rust jobs, a Rust-only change skips markdownlint/actionlint, and so on.

| Output      | Globs that set it to `true`                                                     |
| ----------- | ------------------------------------------------------------------------------ |
| `rust`      | `**/*.rs`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/template.html` |
| `markdown`  | `**/*.md`, `.markdownlint-cli2.jsonc`                                           |
| `workflows` | `.github/workflows/**`                                                          |
| `just`      | `justfile`                                                                     |
| `js`        | `tests/js/**`, `src/template.html`                                              |

> **Why `src/template.html` counts as a Rust change.** It is `include_str!`'d into the binary by
> `render.rs`, so editing it rebuilds the crate and can break tests — it must trigger the Rust jobs.

**Why gating is safe with required checks.** A job skipped by a job-level `if:` still reports a
check, with conclusion **`skipped`**, which the branch ruleset counts as **passing**. So a
docs-only PR shows the Rust checks as skipped-and-green and merges cleanly. This is *only* true for
job-level skips — see the [static-names gotcha](#the-static-names-gotcha).

---

## Jobs

| Job (check name)          | Runs on         | Runs when                        | Goal                                             |
| ------------------------- | --------------- | -------------------------------- | ------------------------------------------------ |
| `detect changes`          | ubuntu          | always                           | Classify the diff; drive every `if:` below       |
| `rustfmt`                 | ubuntu          | `rust`                           | `cargo fmt --all --check` — formatting is law     |
| `clippy (ubuntu-latest)`  | ubuntu          | `rust`                           | `cargo clippy --all-targets -D warnings`          |
| `clippy (macos-latest)`   | macOS           | `rust` **and not** a PR          | Same clippy, macOS coverage on `main` only        |
| `test (ubuntu-latest)`    | ubuntu          | `rust`                           | `cargo test --all-targets`                        |
| `test (macos-latest)`     | macOS           | `rust` **and not** a PR          | Tests on macOS (platform-sensitive server)        |
| `zero dependencies`       | ubuntu          | `rust`                           | Runtime dep tree faceto-only (`cargo tree -e normal`) |
| `binary size budget`      | ubuntu          | `rust`                           | Release binary stays under the 2 MiB budget       |
| `markdownlint`            | ubuntu          | `markdown`                       | Prose ≤100 cols; `.markdownlint-cli2.jsonc` rules |
| `actionlint`              | ubuntu          | `workflows`                      | Lint the workflow files themselves                |
| `justfile`                | ubuntu          | `just`                           | `just --fmt --check` + `--summary` (guard rot)   |
| `client-logic (node)`     | ubuntu          | `js`                             | `node tests/js/board-logic.test.mjs` — pure client helpers |

Notes on the less-obvious ones:

- **`zero dependencies`** and **`binary size budget`** are faceto's headline-promise firewall,
  reshaped to *runtime-only* (see
  [`CODING_STANDARDS.md` §0](../CODING_STANDARDS.md#0-zero-dependencies-the-hard-constraint)). The
  first runs `cargo tree -e normal --prefix none` — the **normal** (runtime) dependency graph, what
  links into the shipped binary — and fails unless it is faceto alone; **dev-dependencies are
  excluded** by `-e normal`, so a test-only crate like `proptest` is allowed (`Cargo.lock` alone
  can't draw that line, since it lists dev-deps as packages too). The second builds `--release` and
  fails if the binary exceeds a **2 MiB** budget (anchor ~905K) — the guard against *runtime* bloat
  now that dep count no longer is.
- **`actionlint`** installs the linter via the upstream downloader script, pinned to a commit and
  **verified by sha256** before running — never an unverified `curl | bash`.
- **`client-logic (node)`** checks the board client's pure helpers (lifted out of `src/template.html`
  by `tests/js/board-logic.test.mjs`) in plain node. Node is preinstalled on the runner and the
  tests use only std node APIs — no npm, no dependency — so the zero-dependency promise is untouched.
- The macOS jobs (`clippy (macos-latest)`, `test (macos-latest)`) are **not required checks**; they
  run only on `push`/`workflow_dispatch`, never on PRs.

---

## Pipeline (the job DAG)

There is one fan-out from `detect changes`; nothing else has cross-job dependencies. Each leaf runs
in parallel as soon as `detect changes` finishes and its `if:` passes.

```text
detect changes ─┬─► rustfmt
                ├─► clippy (ubuntu-latest)
                ├─► clippy (macos-latest)     [main / dispatch only]
                ├─► test (ubuntu-latest)
                ├─► test (macos-latest)       [main / dispatch only]
                ├─► zero dependencies
                ├─► binary size budget
                ├─► markdownlint
                ├─► actionlint
                ├─► justfile
                └─► client-logic (node)
```

There is intentionally **no aggregate "CI passed" gate job**. The required checks are the granular
leaves themselves (next section), so the merge UI shows exactly which check is red.

---

## Required checks & the branch ruleset

Merging to `main` is governed by the `main-protection` ruleset (GitHub → Settings → Rules), not by
in-repo config. Its rules:

- **Required status checks** (must be green or skipped to merge):
  `clippy (ubuntu-latest)`, `test (ubuntu-latest)`, `rustfmt`, `zero dependencies`,
  `binary size budget`, `actionlint`, `justfile`, `client-logic (node)`. *(`binary size budget` and
  `client-logic (node)` are newer jobs — add each to the ruleset in GitHub → Settings → Rules so it
  actually gates; the in-repo workflow only defines the job, it can't make the check required.)*
- **Pull request required** — no direct pushes to `main`.
- **Required signatures** — every commit must be signed (GPG/SSH). Unsigned commits are rejected.
- **Block force-pushes** (`non_fast_forward`) and **block deletion** of `main`.

The macOS checks are deliberately **absent** from the required list, so PRs (which never run them)
are not left waiting on a check that will never report.

### The static-names gotcha

Required check names must be **static strings** — never contain `${{ matrix.os }}`. When a matrix
job is *skipped* (e.g. a docs-only PR), GitHub does **not** expand the matrix, so the check reports
under the literal name `test (${{ matrix.os }})`, which never matches the required
`test (ubuntu-latest)`. The PR then sits `BLOCKED`, waiting on a context that can never appear.

That is why the per-OS jobs are **split** (`test` + `test-macos`) instead of a single matrixed
`test` job. Each has a hardcoded `name:` so its check context is identical whether it runs or is
skipped. If you reintroduce a matrix on a *required* job, you will re-break merges this way.

---

## Platform coverage (a deliberate gap)

Coverage is narrowed on purpose:

- **PRs** run clippy + test on **ubuntu only**.
- **`main`** additionally runs clippy + test on **macOS**.
- **Windows** is not run **anywhere**.

**Why.** faceto has a single contributor/user who runs clippy locally before pushing; no one runs
faceto on Windows; and Windows is the slowest runner. The residual risk is low and accepted (YAGNI).

**The gap to remember.** A macOS-specific regression is only caught once it reaches `main` (never on
the PR); a Windows-specific one is not caught at all. The platform-sensitive surface is the std-only
HTTP server and threading in `serve.rs` (paths, `now_iso`, `TcpListener`) — a future bug there is
the most likely trigger.

**How to restore coverage** if that day comes:

- *macOS on PRs* — drop the `&& github.event_name != 'pull_request'` guard from the `clippy-macos` /
  `test-macos` jobs (keep their static names).
- *Windows* — add `test-windows` / `clippy-windows` jobs (`runs-on: windows-latest`, static names).
  Add them to the ruleset's required checks only if PRs should block on them.
- Keep required-job names static — re-read [the gotcha](#the-static-names-gotcha) first.

---

## Reproducing CI locally

Every gate has a local equivalent, wired up as [`just`](https://github.com/casey/just) recipes in
the repo's [`justfile`](../justfile). Run the whole set before pushing:

```bash
just ci      # format → lint → test → docs → zero-deps → actionlint → justfile, in order
```

Or run one gate at a time:

| Recipe             | Mirrors CI job       | Command it runs                             |
| ------------------ | -------------------- | ------------------------------------------- |
| `just fmt`         | `rustfmt`            | `cargo fmt --all --check`                   |
| `just clippy`      | `clippy (…)`         | `cargo clippy --all-targets -- -D warnings` |
| `just test`        | `test (…)`           | `cargo test --all-targets`                  |
| `just md`          | `markdownlint`       | `markdownlint-cli2 "**/*.md"`               |
| `just zero-deps`   | `zero dependencies`  | assert `cargo tree -e normal` is faceto-only |
| `just binary-size` | `binary size budget` | assert the release binary is under 2 MiB    |
| `just actionlint`  | `actionlint`         | `actionlint`                                |
| `just lint-justfile` | `justfile`         | `just --fmt --check --unstable` + `--summary` |

The justfile exports `RUSTFLAGS=-D warnings`, matching CI so rustc warnings fail locally too.
`just` (`brew install just`), `markdownlint-cli2`, and `actionlint` are dev tools installed
separately — none is a Rust crate, so the zero-dependency promise is untouched.

The repo's [`.pre-commit-config.yaml`](../.pre-commit-config.yaml) also wires fmt, clippy,
markdownlint and `typos` into a pre-commit hook (tests run pre-push). See
[`CONTRIBUTING.md`](../CONTRIBUTING.md) to install it.

---

## Third-party actions & pinning policy

Every third-party action is pinned to a **commit SHA**, with the human-readable version in a
trailing comment. Bump the SHA and the comment together; never use a mutable tag.

| Action                                 | Version | Used by                    |
| -------------------------------------- | ------- | -------------------------- |
| `actions/checkout`                     | v6.0.2  | every job                  |
| `dtolnay/rust-toolchain`               | master  | fmt, clippy(+macos), test(+macos) |
| `Swatinem/rust-cache`                  | v2.9.1  | clippy(+macos), test(+macos) |
| `dorny/paths-filter`                   | v3.0.3  | detect changes             |
| `DavidAnson/markdownlint-cli2-action`  | v23     | markdownlint               |
| actionlint downloader (sha256-checked) | v1.7.7  | actionlint                 |
| `extractions/setup-just`               | v4.0.0  | justfile                   |

---

## Maintenance

- **Toolchain lockstep.** The Rust version appears in three places that must move together:
  `rust-toolchain.toml` (`1.95.0`), `Cargo.toml`'s `rust-version` (`1.95`), and the `toolchain:`
  inputs in `ci.yml`. Bumping one without the others is a bug — see
  [`CODING_STANDARDS.md` → Toolchain policy](../CODING_STANDARDS.md#toolchain-policy).
- **Renaming a job** that is a required check also requires updating the ruleset's required-check
  list, or merges will hang waiting on the old name.

---

## What CI deliberately does *not* do

- No Windows, and no macOS on PRs (see [Platform coverage](#platform-coverage-a-deliberate-gap)).
- No release / publish / `cargo install` automation — installation is a one-line `cargo install`.
- No external test framework, coverage service, or third-party reporter — `std`'s test harness and
  the zero-dependency firewall are the whole story.
