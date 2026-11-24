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
Wayland (`libwayland-client`) or X11 (`libX11`, `libXcursor`, `libXrandr`, `libXi`,
`libxkbcommon-x11`). No `-dev` packages are needed to build.

Some tests draw on the GPU. Without a GPU, install a software driver
(`mesa-vulkan-drivers` on Debian/Ubuntu) and run with `WGPU_BACKEND=vulkan`, as
CI does.

## Examples

```sh
cargo run --locked --example clear   # space cycles the background color, esc quits
cargo run --locked --example rects   # move a box with arrows/WASD in a 320x180 view
cargo run --locked --example sprites # animated sprite: arrows/AD walk, space jumps
```

Set `RUST_LOG=info` to see which GPU and graphics backend are used, and
`WGPU_BACKEND=vulkan` (or `gl`, `metal`, `dx12`) to force a backend. On NVIDIA
with Wayland, the `gl` backend can't present to the window; use Vulkan there.

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
│   ├── lib.rs        public API: Config, run()
│   ├── game.rs       Game trait, Context (update), Frame (draw)
│   ├── app.rs        window, event loop, fixed-timestep game loop
│   ├── time.rs       fixed-timestep accumulator
│   ├── input.rs      keyboard and mouse state
│   ├── math.rs       Vec2, Rect
│   ├── viewport.rs   fits the fixed-size view into the window (letterboxing)
│   ├── gpu.rs        GPU device and window surface (wgpu)
│   ├── renderer.rs   batched sprite and rectangle drawing
│   ├── texture.rs    Texture: PNG loading, raw RGBA
│   ├── shaders/      WGSL shaders
│   ├── color.rs      Color type
│   └── error.rs      Error type
└── examples/         runnable demos
    └── assets/       images used by the examples
```

## License

MIT
