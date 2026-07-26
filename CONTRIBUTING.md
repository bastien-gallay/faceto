# Contributing to faceto

Thanks for taking a look. faceto is small on purpose; the bar for changes is
"keeps it simple and keeps it honest."

## The one hard rule: zero runtime dependencies

faceto's shipped binary is pure Rust standard library — no *runtime* crates. This is a
product decision (trivial, offline install), enforced by two CI jobs: `zero dependencies`
(the *normal* dependency tree, via `cargo tree -e normal`, must be faceto alone) and
`binary size budget` (the release binary stays under its ceiling). If runtime code seems to
need a crate, implement it in `std` or open an issue first. **Dev-dependencies are the one
exception** — test-only crates (`proptest` powers the property tests) never enter the binary
or the offline install — but ask before adding one.

## Local checks (mirror of CI)

Run `just ci` before pushing — it runs every CI gate in order (see
[`docs/ci.md`](docs/ci.md)). CI runs these gates on Linux for every PR, and adds macOS on
`main`. The individual commands, if you'd rather run them piecemeal:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
npx markdownlint-cli2 "**/*.md"   # prose; matches the CI markdown job
just docs                         # builds the manual; matches the CI `docs book` job
```

The manual lives in [`docs/src/`](docs/src/) and is published to
<https://bastien-gallay.github.io/faceto/>. A change a user can notice belongs there in the same
PR — see [`AGENTS.md`](AGENTS.md) § *Documentation is part of the feature* for which page each
kind of change lands on. `just docs-serve` previews it locally with live reload.

The toolchain is pinned in `rust-toolchain.toml` (currently 1.95.0); `rustup`
picks it up automatically. Keep that file, `Cargo.toml`'s `rust-version`, and the
CI `toolchain:` inputs in lockstep when bumping.

### Pre-commit setup (optional but recommended)

`.pre-commit-config.yaml` runs the same gates locally — fmt, clippy and
markdownlint + `typos` on commit, the test suite on push. Install it once:

```bash
uvx pre-commit install --hook-type pre-commit --hook-type pre-push
```

(or `pipx run pre-commit …` / `pip install pre-commit` if you don't use `uv`).

## Working agreement

The full working agreement — **Tidy First**, CUPID & YAGNI, TDD+Reflect, Clean
Code, commit style, and the toolchain policy — lives in
[`CODING_STANDARDS.md`](CODING_STANDARDS.md). The essentials:

- **CUPID** — Composable, Unix-philosophy, Predictable, Idiomatic, Domain-based.
- Each module is one stage of the `JSON → Model → SVG → HTML` pipeline; keep
  it that way (see [`AGENTS.md`](AGENTS.md) for the architecture and the domain invariants).
- The model's `id` is stable identity — the comment join key and the diff key.
  Never derive identity from text or position.

## Tests

Unit tests live in-file under `#[cfg(test)] mod tests`. New behaviour needs a
test; bug fixes need a regression test. The pure stages (`json`, `model`,
`render`, `events`) are the easiest and most valuable to cover.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
`fix:`, `chore:`, `docs:`, `test:`, `ci:`, `build:`). Following **Tidy First**,
keep behavioural and structural (refactor/format) changes in separate commits —
see [`CODING_STANDARDS.md`](CODING_STANDARDS.md) §1 for the rationale and the
acceptable commit shapes.

Record user-visible changes under `## [Unreleased]` in `CHANGELOG.md`.
