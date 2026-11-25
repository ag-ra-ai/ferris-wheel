//! Text in the built-in 5x8 pixel font, in a 320x180 view.
//!
//! Shows scaling, colors, centering with `Font::measure`, multiple lines,
//! every character the font has, and values that change each frame. Uses
//! pixel-perfect scaling so every font pixel is the same size on screen.
//! Escape quits.
//!
//! Run with: `cargo run --example text`

mod support;

use ferris_wheel::{Color, Config, Context, Font, Frame, Game, Key, Scaling, TextParams, Vec2};

const VIEW: Vec2 = Vec2::new(320.0, 180.0);

struct TextDemo {
    font: Font,
    seconds: f32,
    mouse: Option<Vec2>,
}

impl Game for TextDemo {
    fn update(&mut self, ctx: &mut Context) {
        self.seconds += ctx.dt();
        self.mouse = ctx.input().mouse_position();
        if ctx.input().key_pressed(Key::Escape) {
            ctx.quit();
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.clear(Color::hex(0x2e3440));
        let dim = Color::hex(0x81a1c1);

        // A large title, centered, bobbing gently.
        let title = "ferris-wheel";
        let big = TextParams {
            color: Color::hex(0xebcb8b),
            scale: 3.0,
        };
        let size = self.font.measure(title, big.scale);
        let bob = (self.seconds * 2.0).sin() * 2.0;
        let pos = Vec2::new((VIEW.x - size.x) / 2.0, 8.0 + bob);
        frame.draw_text_with(&self.font, title, pos, big);

        let subtitle = "a simple 2D engine in Rust";
        let width = self.font.measure(subtitle, 1.0).x;
        frame.draw_text(subtitle, Vec2::new((VIEW.x - width) / 2.0, 38.0), dim);

        // Every character in the font, 32 per line.
        let all: String = (0x20u8..=0x7e).map(char::from).collect();
        let lines = [&all[0..32], &all[32..64], &all[64..]].join("\n");
        frame.draw_text(&lines, Vec2::new(8.0, 56.0), Color::WHITE);

        // Colors, drawn one word at a time.
        let words = [
            ("red", Color::hex(0xbf616a)),
            ("orange", Color::hex(0xd08770)),
            ("yellow", Color::hex(0xebcb8b)),
            ("green", Color::hex(0xa3be8c)),
            ("purple", Color::hex(0xb48ead)),
        ];
        let mut x = 8.0;
        for (word, color) in words {
            frame.draw_text(word, Vec2::new(x, 94.0), color);
            x += self.font.measure(word, 1.0).x + 6.0;
        }

        // Scale 2, and a translucent color.
        let medium = TextParams {
            color: Color::hex(0x88c0d0),
            scale: 2.0,
        };
        frame.draw_text_with(&self.font, "Scale 2", Vec2::new(8.0, 108.0), medium);
        let faint = TextParams {
            color: Color::rgba(1.0, 1.0, 1.0, 0.35),
            ..medium
        };
        frame.draw_text_with(&self.font, "and faint", Vec2::new(100.0, 108.0), faint);

        // Values that change every frame.
        let mouse = match self.mouse {
            Some(p) => format!("{:.0}, {:.0}", p.x, p.y),
            None => "outside the window".into(),
        };
        let status = format!("Time:  {:.1} s\nMouse: {mouse}", self.seconds);
        frame.draw_text(&status, Vec2::new(8.0, 136.0), Color::WHITE);

        let hint = "Esc quits";
        let size = self.font.measure(hint, 1.0);
        frame.draw_text(hint, VIEW - size - Vec2::new(6.0, 6.0), dim);
    }
}

fn main() -> Result<(), ferris_wheel::Error> {
    support::init_logging();
    let config = Config {
        title: "ferris-wheel: text".into(),
        resolution: (VIEW.x as u32, VIEW.y as u32),
        scaling: Scaling::PixelPerfect,
        ..Config::default()
    };
    let game = TextDemo {
        font: Font::builtin(),
        seconds: 0.0,
        mouse: None,
    };
    ferris_wheel::run(config, game)
}
