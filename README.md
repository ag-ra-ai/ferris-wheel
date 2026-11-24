# ferris-wheel

A simple 2D game engine written in Rust.

## Setup

1. Install [rustup](https://rustup.rs). The exact compiler version is pinned in
   `rust-toolchain.toml`; rustup installs it automatically on first build.
2. Clone the repo and build:

   ```sh
   cargo build --locked
   ```

`Cargo.lock` is committed and every command uses `--locked`, so all machines
build with the same dependency versions.

## Development

```sh
cargo fmt --all                                  # format
cargo clippy --workspace --all-targets --locked  # lint
cargo test --workspace --locked                  # test
```

CI runs the same commands on every push and pull request
(`.github/workflows/ci.yml`).

## Layout

```
crates/ferris_wheel/   the engine library
```

## License

MIT
