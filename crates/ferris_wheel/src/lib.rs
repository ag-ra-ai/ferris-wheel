//! ferris-wheel: a simple 2D game engine.
//!
//! Open a window and start the engine with [`run`]:
//!
//! ```no_run
//! use ferris_wheel::{Color, Config};
//!
//! ferris_wheel::run(Config {
//!     title: "My game".into(),
//!     clear_color: Color::rgb(0.1, 0.1, 0.15),
//!     ..Config::default()
//! })
//! .unwrap();
//! ```

mod app;
mod color;
mod error;
mod gpu;

pub use color::Color;
pub use error::Error;

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
    /// Color the screen is cleared to every frame.
    pub clear_color: Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: "ferris-wheel".into(),
            width: 1280,
            height: 720,
            clear_color: Color::BLACK,
        }
    }
}

/// Opens a window and runs the engine until the window is closed.
///
/// This blocks the calling thread. It must be called from the main thread.
pub fn run(config: Config) -> Result<(), Error> {
    app::run(config)
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
    }
}
