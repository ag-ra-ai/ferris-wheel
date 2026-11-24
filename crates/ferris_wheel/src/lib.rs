//! ferris-wheel: a simple 2D game engine.
//!
//! Implement [`Game`] and start it with [`run`]:
//!
//! ```no_run
//! use ferris_wheel::{Color, Config, Context, Frame, Game, Key};
//!
//! struct MyGame;
//!
//! impl Game for MyGame {
//!     fn update(&mut self, ctx: &mut Context) {
//!         if ctx.input().key_pressed(Key::Escape) {
//!             ctx.quit();
//!         }
//!     }
//!
//!     fn draw(&mut self, frame: &mut Frame) {
//!         frame.clear(Color::rgb(0.1, 0.1, 0.15));
//!     }
//! }
//!
//! ferris_wheel::run(Config::default(), MyGame).unwrap();
//! ```

mod app;
mod color;
mod error;
mod game;
mod gpu;
mod input;
mod math;
mod renderer;
mod time;
mod viewport;

pub use color::Color;
pub use error::Error;
pub use game::{Context, Frame, Game};
pub use input::{Input, Key, MouseButton};
pub use math::{Rect, Vec2};

/// The engine version, taken from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Settings for the game window.
#[derive(Debug, Clone)]
pub struct Config {
    /// Text shown in the window's title bar.
    pub title: String,
    /// Initial window width in logical pixels.
    pub width: u32,
    /// Initial window height in logical pixels.
    pub height: u32,
    /// Size of the game view in world units (width, height).
    ///
    /// The view is scaled to fit the window while keeping its shape; extra
    /// space shows as black bars. For a pixel-art look use a small size such
    /// as `(320, 180)`.
    pub resolution: (u32, u32),
    /// How many times per second [`Game::update`] runs.
    pub updates_per_second: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: "ferris-wheel".into(),
            width: 1280,
            height: 720,
            resolution: (1280, 720),
            updates_per_second: 60,
        }
    }
}

/// Opens a window and runs `game` until the window is closed or the game
/// calls [`Context::quit`].
///
/// This blocks the calling thread. It must be called from the main thread.
pub fn run<G: Game>(config: Config, game: G) -> Result<(), Error> {
    app::run(config, game)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn default_config_has_a_usable_size() {
        let config = Config::default();
        assert!(config.width > 0 && config.height > 0);
        assert!(config.resolution.0 > 0 && config.resolution.1 > 0);
    }
}
