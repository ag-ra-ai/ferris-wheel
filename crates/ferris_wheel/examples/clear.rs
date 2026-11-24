//! Opens a window and clears it to a solid color.
//!
//! Run with: `cargo run --example clear`
//! Set `RUST_LOG=info` to see which GPU is used.

use ferris_wheel::{Color, Config};

fn main() -> Result<(), ferris_wheel::Error> {
    env_logger::init();
    ferris_wheel::run(Config {
        title: "ferris-wheel: clear".into(),
        clear_color: Color::hex(0x2e3440),
        ..Config::default()
    })
}
