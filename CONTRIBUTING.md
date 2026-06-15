# Contributing to faceto

Thanks for taking a look. faceto is small on purpose; the bar for changes is
"keeps it simple and keeps it honest."

## The one hard rule: zero dependencies

faceto builds from the Rust standard library alone — no crates, ever. This is a
product decision (trivial, offline install), enforced by the `zero dependencies`
CI job. If a change seems to need a crate, implement it in `std` or open an issue
to discuss first. The same applies to dev-dependencies: tests use `std` too.

## Local checks (mirror of CI)

Run these before pushing; CI runs the same gates on macOS, Windows, and Linux:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
npx markdownlint-cli2 "**/*.md"   # docs; matches the CI markdown job
```

The toolchain is pinned in `rust-toolchain.toml` (currently 1.95.0); `rustup`
picks it up automatically. Keep that file, `Cargo.toml`'s `rust-version`, and the
CI `toolchain:` inputs in lockstep when bumping.

## Design principles

- **CUPID** — Composable, Unix-philosophy, Predictable, Idiomatic, Domain-based.
- Each source file is one stage of the `JSON → Model → SVG → HTML` pipeline; keep
  it that way (see `CLAUDE.md` for the architecture and the domain invariants).
- The model's `id` is stable identity — the comment join key and the diff key.
  Never derive identity from text or position.

## Tests

Unit tests live in-file under `#[cfg(test)] mod tests`. New behaviour needs a
test; bug fixes need a regression test. The pure stages (`json`, `model`,
`render`) are the easiest and most valuable to cover.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
`fix:`, `chore:`, `docs:`, `test:`, `ci:`, `build:`). Keep behavioural and
structural (refactor/format) changes in separate commits.

Record user-visible changes under `## [Unreleased]` in `CHANGELOG.md`.
