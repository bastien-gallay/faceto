# Local mirror of the CI gates — see docs/ci.md for the full pipeline.
# `just ci` runs the whole set a contributor should pass before pushing.
#
# Extra tools (installed separately; none are Rust crates, so the zero-dependency
# promise is untouched): just, markdownlint-cli2, actionlint.

# Warnings fail the build, exactly as CI's RUSTFLAGS does.
export RUSTFLAGS := "-D warnings"

# List the available recipes.
default:
    @just --list

# Run every CI gate in order (format → lint → test → docs → firewall → workflows → justfile).
ci: fmt clippy test md zero-deps actionlint lint-justfile
    @echo "✓ all local CI gates passed"

# Formatting is law: cargo fmt --all --check.
fmt:
    cargo fmt --all --check

# Clippy over all targets; every warning is an error.
clippy:
    cargo clippy --all-targets -- -D warnings

# The test suite over all targets.
test:
    cargo test --all-targets

# markdownlint (prose ≤100 cols; rules in .markdownlint-cli2.jsonc).
md:
    markdownlint-cli2 "**/*.md"

# Zero-dependency firewall: Cargo.lock must list exactly one package (faceto).
zero-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    count=$(grep -c '^name = ' Cargo.lock)
    if [ "$count" -ne 1 ]; then
      echo "zero-deps FAIL: Cargo.lock lists $count packages:"
      grep '^name = ' Cargo.lock
      exit 1
    fi
    echo "zero-deps OK: exactly one package (faceto)"

# Lint the GitHub Actions workflow files.
actionlint:
    actionlint

# Guard this justfile against rot: check formatting and that it parses.
lint-justfile:
    just --fmt --check --unstable
    just --summary > /dev/null
