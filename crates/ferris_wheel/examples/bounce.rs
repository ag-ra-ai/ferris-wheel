//! Keep the balls in the air with a paddle.
//!
//! - Arrow keys / A, D move the paddle. Where a ball hits the paddle sets the
//!   angle it bounces off at.
//! - Space launches another ball. Losing every ball resets the hit count
//!   (the dots at the top).
//! - I turns interpolation on and off (the square in the top-right corner is
//!   green when on). On a screen faster than 60 Hz, motion stutters when off.
//! - Escape quits.
//!
//! Run with: `cargo run --example bounce`

mod support;

use ferris_wheel::{Color, Config, Context, Frame, Game, Key, Rect, Vec2};

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

struct Bounce {
    balls: Vec<Ball>,
    paddle_prev_x: f32,
    paddle_x: f32,
    hits: u32,
    interpolate: bool,
    rng: u32,
}

impl Bounce {
    fn new() -> Self {
        let mut game = Self {
            balls: Vec::new(),
            paddle_prev_x: (VIEW.x - PADDLE_SIZE.x) / 2.0,
            paddle_x: (VIEW.x - PADDLE_SIZE.x) / 2.0,
            hits: 0,
            interpolate: true,
            rng: 0x9e37_79b9,
        };
        game.launch_ball();
        game
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

    /// Launches a ball upward from just above the paddle.
    fn launch_ball(&mut self) {
        if self.balls.len() >= MAX_BALLS {
            return;
        }
        let paddle = Self::paddle_rect(self.paddle_x);
        let pos = Vec2::new(paddle.center().x, paddle.top() - BALL_SIZE.y) - BALL_SIZE / 2.0;
        let dir = Vec2::new(self.random() * 0.8, -1.0).normalize_or_zero();
        self.balls.push(Ball {
            prev: pos,
            pos,
            vel: dir * BALL_SPEED,
        });
    }
}

impl Game for Bounce {
    fn update(&mut self, ctx: &mut Context) {
        let input = ctx.input();
        let dt = ctx.dt();

        if input.key_pressed(Key::KeyI) {
            self.interpolate = !self.interpolate;
        }
        if input.key_pressed(Key::Space) {
            self.launch_ball();
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

        for ball in &mut self.balls {
            ball.prev = ball.pos;
            ball.pos += ball.vel * dt;

            // Walls and ceiling: push back inside and point the velocity away.
            if ball.pos.x < 0.0 {
                ball.pos.x = 0.0;
                ball.vel.x = ball.vel.x.abs();
            } else if ball.pos.x > VIEW.x - BALL_SIZE.x {
                ball.pos.x = VIEW.x - BALL_SIZE.x;
                ball.vel.x = -ball.vel.x.abs();
            }
            if ball.pos.y < 0.0 {
                ball.pos.y = 0.0;
                ball.vel.y = ball.vel.y.abs();
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
            }
        }

        // Balls that fall past the bottom are lost.
        self.balls.retain(|b| b.pos.y < VIEW.y);
        if self.balls.is_empty() {
            self.hits = 0;
            self.launch_ball();
        }

        if input.key_pressed(Key::Escape) {
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

        // Hit count as a row of dots.
        for i in 0..self.hits.min(60) {
            let dot = Rect::new(4.0 + i as f32 * 5.0, 4.0, 3.0, 3.0);
            frame.fill_rect(dot, Color::hex(0xebcb8b));
        }
        let indicator = if self.interpolate {
            Color::hex(0xa3be8c)
        } else {
            Color::hex(0xbf616a)
        };
        frame.fill_rect(Rect::new(VIEW.x - 8.0, 3.0, 5.0, 5.0), indicator);

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
        title: "ferris-wheel: bounce (arrows/AD move, space = ball, I = interpolation)".into(),
        resolution: (VIEW.x as u32, VIEW.y as u32),
        ..Config::default()
    };
    ferris_wheel::run(config, Bounce::new())
}
