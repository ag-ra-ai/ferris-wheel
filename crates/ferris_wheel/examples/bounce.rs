//! Bounce: keep the balls in the air with a paddle. A small, complete game
//! with a title screen, lives, pause, game over and a best score.
//!
//! - Arrow keys / A, D move the paddle. Where a ball hits the paddle sets the
//!   angle it bounces off at.
//! - Space (or a click) starts. While playing, Space launches another ball,
//!   up to three: more hits, but more to keep up.
//! - Lose every ball in play and you lose a life; after three, the game is
//!   over. Balls speed up as your hits add up.
//! - P pauses, and so does switching to another window.
//! - M turns the music on and off. I turns smooth motion (interpolation) on
//!   and off; on a screen faster than 60 Hz, motion stutters when it is off.
//! - Escape goes back to the title screen, and quits from there (desktop).
//!
//! The best score lasts until the game closes.
//!
//! Every sound is made in code (see `Sounds`), so the example needs no audio
//! files. To use your own, load WAV or Ogg Vorbis files with
//! `Sound::from_bytes(include_bytes!("..."))`. In a browser, sound starts
//! after the first click or key press.
//!
//! Run with: `cargo run --example bounce`

mod support;

use ferris_wheel::{
    Color, Config, Context, Font, Frame, Game, Key, MouseButton, Rect, Sound, SoundParams,
    TextParams, Vec2,
};

const VIEW: Vec2 = Vec2::new(320.0, 180.0);
const PADDLE_SIZE: Vec2 = Vec2::new(48.0, 6.0);
const PADDLE_Y: f32 = 164.0;
const PADDLE_SPEED: f32 = 220.0;
const BALL_SIZE: Vec2 = Vec2::new(6.0, 6.0);
/// Ball speed at the start of a game, in world units per second.
const BALL_SPEED: f32 = 150.0;
/// Each hit makes balls this much faster (1%), up to `MAX_SPEEDUP`.
const SPEEDUP_PER_HIT: f32 = 0.01;
const MAX_SPEEDUP: f32 = 1.6;
const MAX_BALLS: usize = 3;
/// How sideways a ball leaves the paddle when it hits the very edge.
const MAX_BOUNCE_SLANT: f32 = 1.2;
const LIVES: u32 = 3;
/// Seconds a new ball waits on the paddle before it launches.
const SERVE_DELAY: f32 = 1.2;
/// Seconds before "play again" works on the game over screen, so a player
/// still pressing Space doesn't skip it.
const GAME_OVER_DELAY: f32 = 1.0;
const PAUSED_MUSIC_VOLUME: f32 = 0.3;
/// Seconds a message like "Music: off" stays on screen.
const TOAST_TIME: f32 = 1.5;
/// Browsers can't close their tab, so Escape only quits on desktop.
const CAN_QUIT: bool = !cfg!(target_arch = "wasm32");

const BACKGROUND: Color = Color::hex(0x2e3440);
const TEXT: Color = Color::hex(0xeceff4);
const DIM: Color = Color::hex(0x81a1c1);
const GOLD: Color = Color::hex(0xebcb8b);
const RED: Color = Color::hex(0xbf616a);
const PADDLE: Color = Color::hex(0x88c0d0);
const BALL: Color = Color::hex(0xeceff4);

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
    game_over: Sound,
    new_best: Sound,
    /// Menu actions: start, pause, resume, toggles.
    blip: Sound,
    music: Sound,
}

impl Sounds {
    fn new() -> Result<Self, ferris_wheel::Error> {
        // C, E, G and high C, rising.
        let jingle: Vec<f32> = [523.0, 659.0, 784.0, 1047.0]
            .iter()
            .flat_map(|&hz| tone(0.1, hz, hz, square, 0.25))
            .collect();
        Ok(Self {
            paddle: Sound::from_samples(RATE, &tone(0.09, 660.0, 880.0, square, 0.35))?,
            wall: Sound::from_samples(RATE, &tone(0.04, 330.0, 300.0, square, 0.2))?,
            launch: Sound::from_samples(RATE, &tone(0.15, 300.0, 900.0, triangle, 0.5))?,
            lost: Sound::from_samples(RATE, &tone(0.45, 440.0, 110.0, triangle, 0.6))?,
            game_over: Sound::from_samples(RATE, &tone(1.2, 330.0, 55.0, triangle, 0.7))?,
            new_best: Sound::from_samples(RATE, &jingle)?,
            blip: Sound::from_samples(RATE, &tone(0.06, 520.0, 520.0, square, 0.2))?,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Playing,
    Paused,
    GameOver,
}

/// This update's input, read up front: `ctx.input()` borrows `ctx`, which
/// must be free to play sounds or quit.
struct Controls {
    /// -1 left, 1 right, 0 neither (or both).
    dir: f32,
    space: bool,
    click: bool,
    pause: bool,
    back: bool,
    toggle_music: bool,
    toggle_smooth: bool,
    focused: bool,
}

impl Controls {
    fn read(ctx: &Context) -> Self {
        let input = ctx.input();
        let mut dir = 0.0;
        if input.key_held(Key::ArrowLeft) || input.key_held(Key::KeyA) {
            dir -= 1.0;
        }
        if input.key_held(Key::ArrowRight) || input.key_held(Key::KeyD) {
            dir += 1.0;
        }
        Self {
            dir,
            space: input.key_pressed(Key::Space),
            click: input.mouse_pressed(MouseButton::Left),
            pause: input.key_pressed(Key::KeyP),
            back: input.key_pressed(Key::Escape),
            toggle_music: input.key_pressed(Key::KeyM),
            toggle_smooth: input.key_pressed(Key::KeyI),
            focused: input.has_focus(),
        }
    }
}

struct Bounce {
    sounds: Sounds,
    font: Font,
    screen: Screen,
    /// Seconds spent on the current screen, for blinking text and delays.
    screen_time: f32,

    balls: Vec<Ball>,
    paddle_prev_x: f32,
    paddle_x: f32,
    /// Seconds until the waiting ball launches; `None` when none waits.
    serve_in: Option<f32>,
    hits: u32,
    lives: u32,
    best: u32,
    /// Whether the game that just ended set a new best.
    new_best: bool,

    /// A short message at the top and how many seconds it has left.
    toast: Option<(&'static str, f32)>,
    music_on: bool,
    /// False until the first update, which starts the music.
    music_started: bool,
    interpolate: bool,
    rng: u32,
}

impl Bounce {
    fn new() -> Result<Self, ferris_wheel::Error> {
        Ok(Self {
            sounds: Sounds::new()?,
            font: Font::builtin(),
            screen: Screen::Title,
            screen_time: 0.0,
            balls: Vec::new(),
            paddle_prev_x: (VIEW.x - PADDLE_SIZE.x) / 2.0,
            paddle_x: (VIEW.x - PADDLE_SIZE.x) / 2.0,
            serve_in: None,
            hits: 0,
            lives: LIVES,
            best: 0,
            new_best: false,
            toast: None,
            music_on: true,
            music_started: false,
            interpolate: true,
            rng: 0x9e37_79b9,
        })
    }

    fn paddle_rect(x: f32) -> Rect {
        Rect {
            pos: Vec2::new(x, PADDLE_Y),
            size: PADDLE_SIZE,
        }
    }

    /// Where a ball waiting to launch sits: centered on top of the paddle.
    fn serve_pos(paddle_x: f32) -> Vec2 {
        let paddle = Self::paddle_rect(paddle_x);
        Vec2::new(
            paddle.center().x - BALL_SIZE.x / 2.0,
            paddle.top() - BALL_SIZE.y,
        )
    }

    /// A number from -1 to 1 (xorshift; good enough for a game like this).
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Ball speed for the current number of hits.
    fn ball_speed(&self) -> f32 {
        BALL_SPEED * (1.0 + self.hits as f32 * SPEEDUP_PER_HIT).min(MAX_SPEEDUP)
    }

    fn set_screen(&mut self, ctx: &mut Context, screen: Screen) {
        self.screen = screen;
        self.screen_time = 0.0;
        let music_volume = if screen == Screen::Paused {
            PAUSED_MUSIC_VOLUME
        } else {
            1.0
        };
        ctx.set_music_volume(music_volume);
        // Updates stop on other screens, so stop interpolating too, or the
        // last step would replay over and over.
        self.paddle_prev_x = self.paddle_x;
        for ball in &mut self.balls {
            ball.prev = ball.pos;
        }
    }

    fn start_game(&mut self, ctx: &mut Context) {
        self.balls.clear();
        self.hits = 0;
        self.lives = LIVES;
        self.new_best = false;
        self.serve_in = Some(SERVE_DELAY);
        ctx.play_sound(&self.sounds.blip);
        self.set_screen(ctx, Screen::Playing);
    }

    /// Launches a ball upward from the paddle, unless the most are in play.
    fn launch_ball(&mut self, ctx: &mut Context) {
        if self.balls.len() >= MAX_BALLS {
            return;
        }
        let pos = Self::serve_pos(self.paddle_x);
        let dir = Vec2::new(self.random() * 0.8, -1.0).normalize_or_zero();
        self.balls.push(Ball {
            prev: pos,
            pos,
            vel: dir * self.ball_speed(),
        });
        let params = SoundParams {
            pan: pan_at(pos.x),
            ..SoundParams::default()
        };
        ctx.play_sound_with(&self.sounds.launch, params);
    }

    /// One update of the game itself.
    fn play(&mut self, ctx: &mut Context, controls: &Controls) {
        let dt = ctx.dt();
        self.paddle_prev_x = self.paddle_x;
        self.paddle_x =
            (self.paddle_x + controls.dir * PADDLE_SPEED * dt).clamp(0.0, VIEW.x - PADDLE_SIZE.x);
        let paddle = Self::paddle_rect(self.paddle_x);

        match &mut self.serve_in {
            Some(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    self.serve_in = None;
                    self.launch_ball(ctx);
                }
            }
            None if controls.space => self.launch_ball(ctx),
            None => {}
        }

        let speed = self.ball_speed();
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
                ball.vel = Vec2::new(offset * MAX_BOUNCE_SLANT, -1.0).normalize_or_zero() * speed;
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

        // Balls that fall past the bottom are gone; losing the last one
        // costs a life.
        self.balls.retain(|b| b.pos.y < VIEW.y);
        if self.balls.is_empty() && self.serve_in.is_none() {
            self.lives -= 1;
            if self.lives > 0 {
                ctx.play_sound(&self.sounds.lost);
                self.serve_in = Some(SERVE_DELAY);
            } else {
                self.game_over(ctx);
            }
        }
    }

    fn game_over(&mut self, ctx: &mut Context) {
        self.new_best = self.hits > self.best;
        self.best = self.best.max(self.hits);
        ctx.play_sound(&self.sounds.game_over);
        if self.new_best {
            ctx.play_sound(&self.sounds.new_best);
        }
        self.set_screen(ctx, Screen::GameOver);
    }

    /// M and I work on every screen.
    fn handle_toggles(&mut self, ctx: &mut Context, controls: &Controls) {
        if controls.toggle_music {
            self.music_on = !self.music_on;
            let text = if self.music_on {
                "Music: on"
            } else {
                "Music: off"
            };
            self.toast = Some((text, TOAST_TIME));
        }
        // Start the music on the first update. In a browser the engine holds
        // it until the first click or key press, when sound is allowed.
        if controls.toggle_music || !self.music_started {
            self.music_started = true;
            if self.music_on {
                ctx.play_music(&self.sounds.music);
            } else {
                ctx.stop_music();
            }
        }
        if controls.toggle_smooth {
            self.interpolate = !self.interpolate;
            let text = if self.interpolate {
                "Smooth motion: on"
            } else {
                "Smooth motion: off"
            };
            self.toast = Some((text, TOAST_TIME));
            ctx.play_sound(&self.sounds.blip);
        }
    }

    /// Draws `text` centered across the view with its top at `y`.
    fn text_centered(&self, frame: &mut Frame, text: &str, y: f32, scale: f32, color: Color) {
        let width = self.font.measure(text, scale).x;
        let pos = Vec2::new((VIEW.x - width) / 2.0, y);
        frame.draw_text_with(&self.font, text, pos, TextParams { color, scale });
    }

    /// Blinks: on for 60% of every half second.
    fn blink_on(&self) -> bool {
        (self.screen_time * 2.0).fract() < 0.6
    }

    fn draw_title(&self, frame: &mut Frame) {
        let bob = (self.screen_time * 2.5).sin() * 2.0;
        self.text_centered(frame, "BOUNCE", 18.0 + bob, 4.0, GOLD);
        self.text_centered(frame, "Keep the balls in the air", 58.0, 1.0, DIM);

        let controls = [
            ("Arrows, A D", "move"),
            ("Space", "another ball"),
            ("P", "pause"),
            ("M", "music on/off"),
        ];
        for (i, (keys, action)) in controls.into_iter().enumerate() {
            let y = 80.0 + i as f32 * 11.0;
            frame.draw_text(keys, Vec2::new(84.0, y), TEXT);
            frame.draw_text(action, Vec2::new(170.0, y), DIM);
        }

        if self.best > 0 {
            self.text_centered(frame, &format!("Best: {}", self.best), 130.0, 1.0, GOLD);
        }
        if self.blink_on() {
            self.text_centered(frame, "Press Space or click to start", 148.0, 1.0, TEXT);
        }
        if CAN_QUIT {
            let hint = "Esc quits";
            let width = self.font.measure(hint, 1.0).x;
            frame.draw_text(hint, Vec2::new(VIEW.x - 4.0 - width, 168.0), DIM);
        }
    }

    /// The paddle, balls and score, for every screen but the title.
    fn draw_field(&self, frame: &mut Frame, t: f32) {
        let paddle_x = self.paddle_prev_x + (self.paddle_x - self.paddle_prev_x) * t;
        frame.fill_rect(Self::paddle_rect(paddle_x), PADDLE);
        for ball in &self.balls {
            let pos = ball.prev.lerp(ball.pos, t);
            frame.fill_rect(Ball::rect_at(pos), BALL);
        }
        if self.serve_in.is_some() {
            frame.fill_rect(Ball::rect_at(Self::serve_pos(paddle_x)), BALL);
            if self.screen == Screen::Playing && self.blink_on() {
                self.text_centered(frame, "Ready?", 84.0, 1.0, TEXT);
            }
        }

        frame.draw_text(&format!("Hits {}", self.hits), Vec2::new(4.0, 4.0), GOLD);
        frame.draw_text(&format!("Best {}", self.best), Vec2::new(64.0, 4.0), DIM);
        // Lives left, as small balls in the top-right corner.
        for i in 0..self.lives {
            let x = VIEW.x - 4.0 - BALL_SIZE.x - i as f32 * (BALL_SIZE.x + 4.0);
            frame.fill_rect(Ball::rect_at(Vec2::new(x, 5.0)), RED);
        }
    }

    /// Darkens the field behind a pause or game over message.
    fn draw_shade(frame: &mut Frame) {
        frame.fill_rect(
            Rect::new(0.0, 0.0, VIEW.x, VIEW.y),
            Color::rgba(0.0, 0.0, 0.0, 0.55),
        );
    }

    fn draw_paused(&self, frame: &mut Frame) {
        Self::draw_shade(frame);
        self.text_centered(frame, "PAUSED", 56.0, 3.0, TEXT);
        self.text_centered(frame, "P or Space: resume", 100.0, 1.0, DIM);
        self.text_centered(frame, "Esc: title screen", 112.0, 1.0, DIM);
    }

    fn draw_game_over(&self, frame: &mut Frame) {
        Self::draw_shade(frame);
        self.text_centered(frame, "GAME OVER", 36.0, 3.0, RED);
        self.text_centered(frame, &format!("Hits {}", self.hits), 72.0, 2.0, TEXT);
        if self.new_best {
            if self.blink_on() {
                self.text_centered(frame, "New best!", 98.0, 1.0, GOLD);
            }
        } else {
            self.text_centered(frame, &format!("Best {}", self.best), 98.0, 1.0, DIM);
        }
        if self.screen_time >= GAME_OVER_DELAY {
            self.text_centered(frame, "Space or click: play again", 128.0, 1.0, TEXT);
            self.text_centered(frame, "Esc: title screen", 140.0, 1.0, DIM);
        }
    }

    fn draw_toast(&self, frame: &mut Frame) {
        if let Some((text, left)) = self.toast {
            // Fade out over the last half second.
            let color = Color {
                a: (left / 0.5).min(1.0),
                ..DIM
            };
            // Below the score while playing; at the very top on the title.
            let y = if self.screen == Screen::Title {
                4.0
            } else {
                16.0
            };
            self.text_centered(frame, text, y, 1.0, color);
        }
    }
}

/// Plays sounds from where they happen: left side of the view on the left.
fn pan_at(x: f32) -> f32 {
    (x / VIEW.x * 2.0 - 1.0) * 0.6
}

impl Game for Bounce {
    fn update(&mut self, ctx: &mut Context) {
        let controls = Controls::read(ctx);
        let dt = ctx.dt();
        self.screen_time += dt;
        if let Some((_, left)) = &mut self.toast {
            *left -= dt;
            if *left <= 0.0 {
                self.toast = None;
            }
        }
        self.handle_toggles(ctx, &controls);

        match self.screen {
            Screen::Title => {
                if controls.space || controls.click {
                    self.start_game(ctx);
                } else if controls.back && CAN_QUIT {
                    ctx.quit();
                }
            }
            Screen::Playing => {
                if controls.back {
                    self.set_screen(ctx, Screen::Title);
                } else if controls.pause || !controls.focused {
                    ctx.play_sound(&self.sounds.blip);
                    self.set_screen(ctx, Screen::Paused);
                } else {
                    self.play(ctx, &controls);
                }
            }
            Screen::Paused => {
                if controls.back {
                    self.set_screen(ctx, Screen::Title);
                } else if controls.pause || controls.space {
                    ctx.play_sound(&self.sounds.blip);
                    self.set_screen(ctx, Screen::Playing);
                }
            }
            Screen::GameOver => {
                if controls.back {
                    self.set_screen(ctx, Screen::Title);
                } else if (controls.space || controls.click) && self.screen_time >= GAME_OVER_DELAY
                {
                    self.start_game(ctx);
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        // With interpolation off (or while not playing), everything is drawn
        // where the last update left it.
        let t = if self.interpolate && self.screen == Screen::Playing {
            frame.interpolation()
        } else {
            1.0
        };

        frame.clear(BACKGROUND);
        match self.screen {
            Screen::Title => self.draw_title(frame),
            Screen::Playing => self.draw_field(frame, t),
            Screen::Paused => {
                self.draw_field(frame, t);
                self.draw_paused(frame);
            }
            Screen::GameOver => {
                self.draw_field(frame, t);
                self.draw_game_over(frame);
            }
        }
        self.draw_toast(frame);
    }
}

fn main() -> Result<(), ferris_wheel::Error> {
    support::init_logging();
    let config = Config {
        title: "ferris-wheel: bounce".into(),
        resolution: (VIEW.x as u32, VIEW.y as u32),
        ..Config::default()
    };
    ferris_wheel::run(config, Bounce::new()?)
}
