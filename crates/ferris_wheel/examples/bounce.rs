//! Keep the balls in the air with a paddle.
//!
//! - Arrow keys / A, D move the paddle. Where a ball hits the paddle sets the
//!   angle it bounces off at.
//! - Space launches another ball. Losing every ball resets the hit count.
//! - I turns interpolation ("Smooth" in the top-right corner) on and off. On
//!   a screen faster than 60 Hz, motion stutters when it is off.
//! - M turns the music on and off.
//! - Escape quits.
//!
//! Every sound is made in code (see `Sounds`), so the example needs no audio
//! files. To use your own, load WAV or Ogg Vorbis files with
//! `Sound::from_bytes(include_bytes!("..."))`. In a browser, sound starts
//! after the first click or key press.
//!
//! Run with: `cargo run --example bounce`

mod support;

use ferris_wheel::{
    Color, Config, Context, Font, Frame, Game, Key, Rect, Sound, SoundParams, Vec2,
};

const VIEW: Vec2 = Vec2::new(320.0, 180.0);
const PADDLE_SIZE: Vec2 = Vec2::new(48.0, 6.0);
const PADDLE_Y: f32 = 164.0;
const PADDLE_SPEED: f32 = 220.0;
const BALL_SIZE: Vec2 = Vec2::new(6.0, 6.0);
const BALL_SPEED: f32 = 150.0;
const MAX_BALLS: usize = 64;
/// How sideways a ball leaves the paddle when it hits the very edge.
const MAX_BOUNCE_SLANT: f32 = 1.2;

struct Ball {
    /// Position before the last update, for interpolation.
    prev: Vec2,
    pos: Vec2,
    vel: Vec2,
}

impl Ball {
    fn rect_at(pos: Vec2) -> Rect {
        Rect {
            pos,
            size: BALL_SIZE,
        }
    }
}

/// Sample rate of the sounds made below.
const RATE: u32 = 22050;

/// The game's sound effects and music, synthesized at startup.
struct Sounds {
    paddle: Sound,
    wall: Sound,
    launch: Sound,
    lost: Sound,
    music: Sound,
}

impl Sounds {
    fn new() -> Result<Self, ferris_wheel::Error> {
        Ok(Self {
            paddle: Sound::from_samples(RATE, &tone(0.09, 660.0, 880.0, square, 0.35))?,
            wall: Sound::from_samples(RATE, &tone(0.04, 330.0, 300.0, square, 0.2))?,
            launch: Sound::from_samples(RATE, &tone(0.15, 300.0, 900.0, triangle, 0.5))?,
            lost: Sound::from_samples(RATE, &tone(0.45, 440.0, 110.0, triangle, 0.6))?,
            music: Sound::from_samples(RATE, &music())?,
        })
    }
}

/// One cycle of a wave, for `phase` from 0 to 1.
fn square(phase: f32) -> f32 {
    if phase < 0.5 { 1.0 } else { -1.0 }
}

fn triangle(phase: f32) -> f32 {
    1.0 - 4.0 * (phase - 0.5).abs()
}

/// A note that slides from `start_hz` to `end_hz` and fades out.
fn tone(seconds: f32, start_hz: f32, end_hz: f32, wave: fn(f32) -> f32, volume: f32) -> Vec<f32> {
    let len = (seconds * RATE as f32) as usize;
    let mut phase = 0.0;
    (0..len)
        .map(|i| {
            let t = i as f32 / len as f32;
            phase = (phase + (start_hz + (end_hz - start_hz) * t) / RATE as f32).fract();
            // A 5 ms fade-in avoids a click; then fade out to the end.
            let attack = (i as f32 / (0.005 * RATE as f32)).min(1.0);
            wave(phase) * volume * attack * (1.0 - t) * (1.0 - t)
        })
        .collect()
}

/// A short looping tune: a bass note and an arpeggio for each of four chords.
fn music() -> Vec<f32> {
    // MIDI note numbers: (bass, arpeggio) for A minor, F, C and G.
    let chords: [(u8, [u8; 4]); 4] = [
        (45, [69, 72, 76, 72]),
        (41, [65, 69, 72, 69]),
        (48, [67, 72, 76, 72]),
        (43, [67, 71, 74, 71]),
    ];
    let hz = |note: u8| 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
    let eighth = 0.25; // seconds, at 120 beats per minute
    let mut out = Vec::new();
    for (bass, arpeggio) in chords {
        let bar_start = out.len();
        // Eight eighth notes per bar: the arpeggio twice.
        for note in arpeggio.iter().chain(&arpeggio) {
            out.extend(tone(eighth, hz(*note), hz(*note), triangle, 0.12));
        }
        // The bass rings for the whole bar, mixed in under the arpeggio.
        let bass = tone(eighth * 8.0, hz(bass), hz(bass), triangle, 0.25);
        for (sample, b) in out[bar_start..].iter_mut().zip(bass) {
            *sample += b;
        }
    }
    out
}

struct Bounce {
    sounds: Sounds,
    music_on: bool,
    /// False until the first update, which starts the music.
    started: bool,
    balls: Vec<Ball>,
    paddle_prev_x: f32,
    paddle_x: f32,
    hits: u32,
    interpolate: bool,
    rng: u32,
}

impl Bounce {
    fn new() -> Result<Self, ferris_wheel::Error> {
        let mut game = Self {
            sounds: Sounds::new()?,
            music_on: true,
            started: false,
            balls: Vec::new(),
            paddle_prev_x: (VIEW.x - PADDLE_SIZE.x) / 2.0,
            paddle_x: (VIEW.x - PADDLE_SIZE.x) / 2.0,
            hits: 0,
            interpolate: true,
            rng: 0x9e37_79b9,
        };
        game.launch_ball();
        Ok(game)
    }

    fn paddle_rect(x: f32) -> Rect {
        Rect {
            pos: Vec2::new(x, PADDLE_Y),
            size: PADDLE_SIZE,
        }
    }

    /// A number from -1 to 1 (xorshift; good enough for a demo).
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Launches a ball upward from just above the paddle. Returns false if
    /// there are already as many balls as allowed.
    fn launch_ball(&mut self) -> bool {
        if self.balls.len() >= MAX_BALLS {
            return false;
        }
        let paddle = Self::paddle_rect(self.paddle_x);
        let pos = Vec2::new(paddle.center().x, paddle.top() - BALL_SIZE.y) - BALL_SIZE / 2.0;
        let dir = Vec2::new(self.random() * 0.8, -1.0).normalize_or_zero();
        self.balls.push(Ball {
            prev: pos,
            pos,
            vel: dir * BALL_SPEED,
        });
        true
    }
}

/// Plays sounds from where they happen: left side of the view on the left.
fn pan_at(x: f32) -> f32 {
    (x / VIEW.x * 2.0 - 1.0) * 0.6
}

impl Game for Bounce {
    fn update(&mut self, ctx: &mut Context) {
        // Read the input first: `input` borrows `ctx`, which must be free
        // before playing sounds or quitting.
        let input = ctx.input();
        let dt = ctx.dt();
        let launch = input.key_pressed(Key::Space);
        let toggle_music = input.key_pressed(Key::KeyM);
        let quit = input.key_pressed(Key::Escape);
        if input.key_pressed(Key::KeyI) {
            self.interpolate = !self.interpolate;
        }

        let mut dir = 0.0;
        if input.key_held(Key::ArrowLeft) || input.key_held(Key::KeyA) {
            dir -= 1.0;
        }
        if input.key_held(Key::ArrowRight) || input.key_held(Key::KeyD) {
            dir += 1.0;
        }
        self.paddle_prev_x = self.paddle_x;
        self.paddle_x =
            (self.paddle_x + dir * PADDLE_SPEED * dt).clamp(0.0, VIEW.x - PADDLE_SIZE.x);
        let paddle = Self::paddle_rect(self.paddle_x);

        // Start the music on the first update. In a browser the engine holds
        // it until the first click or key press, when sound is allowed.
        if toggle_music {
            self.music_on = !self.music_on;
        }
        if toggle_music || !self.started {
            self.started = true;
            if self.music_on {
                ctx.play_music(&self.sounds.music);
            } else {
                ctx.stop_music();
            }
        }

        if launch && self.launch_ball() {
            let pan = pan_at(self.paddle_x + PADDLE_SIZE.x / 2.0);
            let params = SoundParams {
                pan,
                ..SoundParams::default()
            };
            ctx.play_sound_with(&self.sounds.launch, params);
        }

        for ball in &mut self.balls {
            ball.prev = ball.pos;
            ball.pos += ball.vel * dt;

            // Walls and ceiling: push back inside and point the velocity away.
            let mut hit_wall = false;
            if ball.pos.x < 0.0 {
                ball.pos.x = 0.0;
                ball.vel.x = ball.vel.x.abs();
                hit_wall = true;
            } else if ball.pos.x > VIEW.x - BALL_SIZE.x {
                ball.pos.x = VIEW.x - BALL_SIZE.x;
                ball.vel.x = -ball.vel.x.abs();
                hit_wall = true;
            }
            if ball.pos.y < 0.0 {
                ball.pos.y = 0.0;
                ball.vel.y = ball.vel.y.abs();
                hit_wall = true;
            }
            if hit_wall {
                let params = SoundParams {
                    pan: pan_at(ball.pos.x),
                    ..SoundParams::default()
                };
                ctx.play_sound_with(&self.sounds.wall, params);
            }

            // Paddle: only while falling, so a ball can't get stuck inside.
            let rect = Ball::rect_at(ball.pos);
            if ball.vel.y > 0.0 && rect.overlaps(&paddle) {
                // -1 at the paddle's left edge, 1 at its right edge.
                let offset = ((rect.center().x - paddle.center().x) / (PADDLE_SIZE.x / 2.0))
                    .clamp(-1.0, 1.0);
                ball.vel =
                    Vec2::new(offset * MAX_BOUNCE_SLANT, -1.0).normalize_or_zero() * BALL_SPEED;
                ball.pos.y = paddle.top() - BALL_SIZE.y;
                self.hits += 1;
                // Higher pitched toward the paddle's edges.
                let params = SoundParams {
                    speed: 1.0 + offset.abs() * 0.3,
                    pan: pan_at(ball.pos.x),
                    ..SoundParams::default()
                };
                ctx.play_sound_with(&self.sounds.paddle, params);
            }
        }

        // Balls that fall past the bottom are lost.
        let before = self.balls.len();
        self.balls.retain(|b| b.pos.y < VIEW.y);
        if self.balls.len() < before {
            ctx.play_sound(&self.sounds.lost);
        }
        if self.balls.is_empty() {
            self.hits = 0;
            self.launch_ball();
        }

        if quit {
            ctx.quit();
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        // With interpolation off, everything is drawn where the last update
        // left it.
        let t = if self.interpolate {
            frame.interpolation()
        } else {
            1.0
        };

        frame.clear(Color::hex(0x2e3440));

        let hud = format!("Hits {}   Balls {}", self.hits, self.balls.len());
        frame.draw_text(&hud, Vec2::new(4.0, 4.0), Color::hex(0xebcb8b));

        // Interpolation state, right-aligned.
        let (label, color) = if self.interpolate {
            ("Smooth: on (I)", Color::hex(0xa3be8c))
        } else {
            ("Smooth: off (I)", Color::hex(0xbf616a))
        };
        let width = Font::builtin().measure(label, 1.0).x;
        frame.draw_text(label, Vec2::new(VIEW.x - 4.0 - width, 4.0), color);

        let (label, color) = if self.music_on {
            ("Music: on (M)", Color::hex(0xa3be8c))
        } else {
            ("Music: off (M)", Color::hex(0xbf616a))
        };
        let width = Font::builtin().measure(label, 1.0).x;
        frame.draw_text(label, Vec2::new(VIEW.x - 4.0 - width, 14.0), color);

        let paddle_x = self.paddle_prev_x + (self.paddle_x - self.paddle_prev_x) * t;
        frame.fill_rect(Self::paddle_rect(paddle_x), Color::hex(0x88c0d0));

        for ball in &self.balls {
            let pos = ball.prev.lerp(ball.pos, t);
            frame.fill_rect(Ball::rect_at(pos), Color::hex(0xeceff4));
        }
    }
}

fn main() -> Result<(), ferris_wheel::Error> {
    support::init_logging();
    let config = Config {
        title: "ferris-wheel: bounce (arrows/AD move, space = ball, I = smooth, M = music)".into(),
        resolution: (VIEW.x as u32, VIEW.y as u32),
        ..Config::default()
    };
    ferris_wheel::run(config, Bounce::new()?)
}
