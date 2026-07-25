# Installation

`faceto` is a single binary with **no runtime dependencies**. Once installed it never needs the
network again.

## From source

You need a Rust toolchain (the pinned version lives in `rust-toolchain.toml`; `rustup` picks it up
automatically).

```bash
git clone https://github.com/bastien-gallay/faceto
cd faceto
cargo install --path .
```

This puts `faceto` in `~/.cargo/bin`. Check it:

```bash
faceto version
faceto help
```

## Without installing

To try it inside the repository without touching `~/.cargo/bin`:

```bash
cargo build --release
./target/release/faceto render examples/sample.model.json
```

## What gets written where

`faceto` writes only beside the file you point it at, and never outside that directory:

| Command | Writes |
| --- | --- |
| `render SOURCE` | `<name>.svg` and `<name>.html` |
| `genesis MODEL` | `<name>.event-log.jsonl` |
| `serve MODEL` | `<name>.event-log.jsonl` (created once if absent, then appended to) |
| `compact LOG` | the log, in place — after copying the previous one to `<log>.bak` |
| `lint`, `export` | nothing (stdout only) |

`<name>` is the **board name**, derived from the source basename: `orders.model.json` and
`orders.event-log.jsonl` both resolve to `orders`. Sibling boards in one directory therefore never
clobber each other's outputs or logs.

## Optional: PNG output

Raster export is a deliberate non-goal — a PNG encoder and a font rasteriser cannot be written in
pure `std` at a reasonable size. Convert the SVG with a tool you already trust:

```bash
rsvg-convert -o orders.png orders.svg
# or: resvg orders.svg orders.png
# or: chromium --headless --screenshot=orders.png orders.svg
```

## Development tooling

Only needed to work *on* faceto, never to use it. See
[Contributing](../project/contributing.md) for the full gate; the short version is `just ci`.
