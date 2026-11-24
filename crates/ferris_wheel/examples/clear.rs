//! Clears the window to a color. Space cycles colors, Escape quits.
//!
//! Run with: `cargo run --example clear`
//! Set `RUST_LOG=info` to see which GPU is used. In a browser: `cargo xtask web clear`.

mod support;

use ferris_wheel::{Color, Config, Context, Frame, Game, Key};

const COLORS: [Color; 4] = [
    Color::hex(0x2e3440),
    Color::hex(0xbf616a),
    Color::hex(0xa3be8c),
    Color::hex(0x5e81ac),
];

struct ClearDemo {
    index: usize,
}

impl Game for ClearDemo {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.input().key_pressed(Key::Space) {
            self.index = (self.index + 1) % COLORS.len();
        }
        if ctx.input().key_pressed(Key::Escape) {
            ctx.quit();
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.clear(COLORS[self.index]);
    }
}

fn main() -> Result<(), ferris_wheel::Error> {
    support::init_logging();
    let config = Config {
        title: "ferris-wheel: clear (space = next color, esc = quit)".into(),
        ..Config::default()
    };
    ferris_wheel::run(config, ClearDemo { index: 0 })
}
