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

# Run every CI gate in order (format → lint → test → docs → firewall → size → workflows → justfile).
ci: fmt clippy test test-js md zero-deps binary-size actionlint lint-justfile
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

# Client-logic tests: pure helpers lifted out of src/template.html, checked in plain node (no deps).
test-js:
    node tests/js/board-logic.test.mjs

# markdownlint (prose ≤100 cols; rules in .markdownlint-cli2.jsonc).
md:
    markdownlint-cli2 "**/*.md"

# Runtime zero-dependency firewall: the normal dep tree must be faceto-only (dev-deps are free).
zero-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    # Runtime (normal) dep tree only — dev-dependencies (e.g. proptest) are excluded by
    # `-e normal` and allowed, since they never enter the shipped binary or the install.
    deps=$(cargo tree -e normal --prefix none | awk 'NF{print $1}' | sort -u)
    if [ "$deps" != "faceto" ]; then
      echo "zero-deps FAIL: runtime dependency tree must be std-only, but found:"
      echo "$deps"
      exit 1
    fi
    echo "zero-deps OK: runtime dependency tree is faceto-only"

# Runtime-bloat guard: the shipped release binary must stay under the size budget (2 MiB).
binary-size:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release
    size=$(stat -f%z target/release/faceto 2>/dev/null || stat -c%s target/release/faceto)
    ceiling=$((2 * 1024 * 1024))
    printf 'faceto release binary: %d bytes (ceiling %d)\n' "$size" "$ceiling"
    if [ "$size" -gt "$ceiling" ]; then
      echo "binary-size FAIL: $size B exceeds the $ceiling B budget"
      exit 1
    fi
    echo "binary-size OK: under the ${ceiling}-byte budget"

# Lint the GitHub Actions workflow files.
actionlint:
    actionlint

# Guard this justfile against rot: check formatting and that it parses.
lint-justfile:
    just --fmt --check --unstable
    just --summary > /dev/null
