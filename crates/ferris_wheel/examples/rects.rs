//! Colored rectangles in a 320x180 view, scaled to fit the window.
//!
//! - Arrow keys / WASD move the box; it turns gold while touching the target.
//! - The tile under the mouse is highlighted.
//! - Resize the window to see the view scale with black bars.
//! - Escape quits.
//!
//! Run with: `cargo run --example rects`

use ferris_wheel::{Color, Config, Context, Frame, Game, Key, Rect, Vec2};

const TILE: f32 = 20.0;
const SPEED: f32 = 90.0; // world units per second

struct RectsDemo {
    player: Rect,
    target: Rect,
    touching: bool,
    hovered_tile: Option<(i32, i32)>,
}

impl RectsDemo {
    fn new() -> Self {
        Self {
            player: Rect::new(40.0, 80.0, 16.0, 16.0),
            target: Rect::new(240.0, 70.0, 30.0, 40.0),
            touching: false,
            hovered_tile: None,
        }
    }
}

impl Game for RectsDemo {
    fn update(&mut self, ctx: &mut Context) {
        let input = ctx.input();
        let held = |a, b| input.key_held(a) || input.key_held(b);
        let mut dir = Vec2::ZERO;
        if held(Key::ArrowLeft, Key::KeyA) {
            dir.x -= 1.0;
        }
        if held(Key::ArrowRight, Key::KeyD) {
            dir.x += 1.0;
        }
        if held(Key::ArrowUp, Key::KeyW) {
            dir.y -= 1.0;
        }
        if held(Key::ArrowDown, Key::KeyS) {
            dir.y += 1.0;
        }
        // Normalize so diagonal movement isn't faster.
        self.player.pos += dir.normalize_or_zero() * SPEED * ctx.dt();

        // Keep the player inside the view.
        let max = ctx.view_size() - self.player.size;
        self.player.pos.x = self.player.pos.x.clamp(0.0, max.x);
        self.player.pos.y = self.player.pos.y.clamp(0.0, max.y);

        self.touching = self.player.overlaps(&self.target);
        self.hovered_tile = input
            .mouse_position()
            .filter(|p| Rect::new(0.0, 0.0, ctx.view_size().x, ctx.view_size().y).contains(*p))
            .map(|p| ((p.x / TILE) as i32, (p.y / TILE) as i32));

        if input.key_pressed(Key::Escape) {
            ctx.quit();
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.clear(Color::hex(0x2e3440));

        // Checkerboard floor.
        let size = frame.view_size();
        let (cols, rows) = ((size.x / TILE).ceil() as i32, (size.y / TILE).ceil() as i32);
        for row in 0..rows {
            for col in 0..cols {
                let color = if self.hovered_tile == Some((col, row)) {
                    Color::hex(0x88c0d0)
                } else if (row + col) % 2 == 0 {
                    Color::hex(0x3b4252)
                } else {
                    continue; // background shows through
                };
                let rect = Rect::new(col as f32 * TILE, row as f32 * TILE, TILE, TILE);
                frame.fill_rect(rect, color);
            }
        }

        frame.fill_rect(self.target, Color::hex(0xa3be8c));
        let player_color = if self.touching {
            Color::hex(0xebcb8b)
        } else {
            Color::hex(0xbf616a)
        };
        frame.fill_rect(self.player, player_color);
        // A translucent bar across the top, to show alpha blending.
        frame.fill_rect(
            Rect::new(0.0, 0.0, size.x, 12.0),
            Color::rgba(0.0, 0.0, 0.0, 0.4),
        );
    }
}

fn main() -> Result<(), ferris_wheel::Error> {
    env_logger::init();
    let config = Config {
        title: "ferris-wheel: rects (arrows/WASD move, esc quits)".into(),
        resolution: (320, 180),
        ..Config::default()
    };
    ferris_wheel::run(config, RectsDemo::new())
}
