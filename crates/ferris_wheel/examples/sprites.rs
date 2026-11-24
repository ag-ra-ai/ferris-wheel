//! An animated sprite on tiled ground, in a 320x180 view.
//!
//! - Arrow keys / A, D walk; Space jumps. Walking plays the hop animation and
//!   the sprite flips to face the way it moves.
//! - The row at the top shows tinting and transparency.
//! - Escape quits.
//!
//! Run with: `cargo run --example sprites`

use ferris_wheel::{Color, Config, Context, Frame, Game, Key, Rect, SpriteParams, Texture, Vec2};

/// 4 frames of 16x16, side by side. Embedded so the example runs from any
/// working directory; use `Texture::from_png_file` to load at runtime.
const BLOB_PNG: &[u8] = include_bytes!("assets/blob.png");
const FRAME_SIZE: f32 = 16.0;
const FRAMES: usize = 4;
const FRAMES_PER_SECOND: f32 = 10.0;

const SCALE: f32 = 2.0; // draw the 16px sprite at 32 world units
const TILE: f32 = 16.0;
const GROUND_Y: f32 = 180.0 - 2.0 * TILE;
const WALK_SPEED: f32 = 80.0;
const JUMP_SPEED: f32 = 260.0;
const GRAVITY: f32 = 700.0;

struct SpritesDemo {
    blob: Texture,
    ground: Texture,
    /// Position before the last update, for smooth drawing.
    prev_pos: Vec2,
    pos: Vec2,
    velocity_y: f32,
    facing_left: bool,
    walking: bool,
    anim_time: f32,
}

impl SpritesDemo {
    fn new() -> Result<Self, ferris_wheel::Error> {
        Ok(Self {
            blob: Texture::from_png_bytes(BLOB_PNG)?,
            ground: ground_tiles()?,
            prev_pos: Vec2::new(144.0, GROUND_Y - FRAME_SIZE * SCALE),
            pos: Vec2::new(144.0, GROUND_Y - FRAME_SIZE * SCALE),
            velocity_y: 0.0,
            facing_left: false,
            walking: false,
            anim_time: 0.0,
        })
    }

    fn on_ground(&self) -> bool {
        self.pos.y >= GROUND_Y - FRAME_SIZE * SCALE
    }

    /// Source rect of animation frame `index` in the sheet.
    fn frame_rect(index: usize) -> Rect {
        Rect::new(index as f32 * FRAME_SIZE, 0.0, FRAME_SIZE, FRAME_SIZE)
    }
}

/// Two 16x16 ground tiles stacked vertically (grass-topped dirt, then plain
/// dirt), built from code to show `Texture::from_rgba8`.
fn ground_tiles() -> Result<Texture, ferris_wheel::Error> {
    let mut pixels = Vec::with_capacity(16 * 32 * 4);
    for y in 0..32u32 {
        for x in 0..16u32 {
            let rgb = match y {
                0..=2 => [0x6f, 0x8f, 0x4e],                           // grass
                _ if (x * 7 + y * 13) % 11 == 0 => [0x5e, 0x50, 0x46], // pebbles
                _ => [0x7a, 0x67, 0x58],                               // dirt
            };
            pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    Texture::from_rgba8(16, 32, pixels)
}

impl Game for SpritesDemo {
    fn update(&mut self, ctx: &mut Context) {
        let input = ctx.input();
        let dt = ctx.dt();
        self.prev_pos = self.pos;

        let mut dir = 0.0;
        if input.key_held(Key::ArrowLeft) || input.key_held(Key::KeyA) {
            dir -= 1.0;
        }
        if input.key_held(Key::ArrowRight) || input.key_held(Key::KeyD) {
            dir += 1.0;
        }
        self.walking = dir != 0.0;
        if self.walking {
            self.facing_left = dir < 0.0;
            self.anim_time += dt;
        } else {
            self.anim_time = 0.0;
        }
        let max_x = ctx.view_size().x - FRAME_SIZE * SCALE;
        self.pos.x = (self.pos.x + dir * WALK_SPEED * dt).clamp(0.0, max_x);

        if input.key_pressed(Key::Space) && self.on_ground() {
            self.velocity_y = -JUMP_SPEED;
        }
        self.velocity_y += GRAVITY * dt;
        self.pos.y += self.velocity_y * dt;
        if self.on_ground() {
            self.pos.y = GROUND_Y - FRAME_SIZE * SCALE;
            self.velocity_y = 0.0;
        }

        if input.key_pressed(Key::Escape) {
            ctx.quit();
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.clear(Color::hex(0x2e3440));
        let view = frame.view_size();

        // Ground: a grass row, then a dirt row, each picked from the tileset.
        let cols = (view.x / TILE).ceil() as i32;
        for row in 0..2 {
            let params = SpriteParams {
                source: Some(Rect::new(0.0, row as f32 * TILE, TILE, TILE)),
                ..Default::default()
            };
            for col in 0..cols {
                let dest = Rect::new(col as f32 * TILE, GROUND_Y + row as f32 * TILE, TILE, TILE);
                frame.draw_sprite_with(&self.ground, dest, params);
            }
        }

        // Tint and transparency samples.
        let tints = [
            Color::WHITE,
            Color::hex(0xbf616a),
            Color::hex(0x88c0d0),
            Color::rgba(1.0, 1.0, 1.0, 0.4),
        ];
        for (i, tint) in tints.into_iter().enumerate() {
            let dest = Rect::new(8.0 + i as f32 * 20.0, 8.0, FRAME_SIZE, FRAME_SIZE);
            let params = SpriteParams {
                source: Some(Self::frame_rect(1)),
                tint,
                ..Default::default()
            };
            frame.draw_sprite_with(&self.blob, dest, params);
        }

        // The player. Frame 1 is the resting pose.
        let index = if self.walking {
            (self.anim_time * FRAMES_PER_SECOND) as usize % FRAMES
        } else {
            1
        };
        // Draw between the last two updates so motion is smooth on fast screens.
        let dest = Rect {
            pos: self.prev_pos.lerp(self.pos, frame.interpolation()),
            size: Vec2::new(FRAME_SIZE, FRAME_SIZE) * SCALE,
        };
        let params = SpriteParams {
            source: Some(Self::frame_rect(index)),
            flip_x: self.facing_left,
            ..Default::default()
        };
        frame.draw_sprite_with(&self.blob, dest, params);
    }
}

fn main() -> Result<(), ferris_wheel::Error> {
    env_logger::init();
    let config = Config {
        title: "ferris-wheel: sprites (arrows/AD walk, space jumps, esc quits)".into(),
        resolution: (320, 180),
        ..Config::default()
    };
    ferris_wheel::run(config, SpritesDemo::new()?)
}
