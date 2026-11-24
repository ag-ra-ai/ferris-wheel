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

### System requirements

Rendering uses [wgpu](https://wgpu.rs), so you need a GPU driver with Vulkan,
Metal, DirectX 12 or OpenGL ES support. On Linux, the window system libraries are
loaded at runtime and are present on any desktop install: `libxkbcommon` plus
Wayland (`libwayland-client`) or X11 (`libX11`, `libXcursor`, `libXrandr`, `libXi`).
No `-dev` packages are needed to build.

## Examples

```sh
cargo run --locked --example clear   # open a window and clear it to a color
```

Set `RUST_LOG=info` to see which GPU and graphics backend are used, and
`WGPU_BACKEND=gl` (or `vulkan`, `metal`, `dx12`) to force a backend.

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
crates/ferris_wheel/
├── src/
│   ├── lib.rs     public API: Config, run()
│   ├── app.rs     window and event loop (winit)
│   ├── gpu.rs     GPU setup and drawing (wgpu)
│   ├── color.rs   Color type
│   └── error.rs   Error type
└── examples/      runnable demos
```

## License

MIT
