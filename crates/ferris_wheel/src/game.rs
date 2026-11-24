//! The trait a game implements, and what the engine passes to it.

use std::time::Duration;

use crate::{Color, Input};

/// A game the engine can run. See [`crate::run`].
pub trait Game {
    /// Advances the game by one fixed step of [`Context::dt`] seconds.
    ///
    /// Called at a steady rate ([`crate::Config::updates_per_second`]),
    /// independent of how fast frames are drawn. Put movement, physics and
    /// game rules here.
    fn update(&mut self, ctx: &mut Context);

    /// Draws the current state. Called once per displayed frame.
    fn draw(&mut self, frame: &mut Frame);
}

/// Engine state available during [`Game::update`].
#[derive(Debug)]
pub struct Context {
    pub(crate) input: Input,
    pub(crate) step: Duration,
    pub(crate) quit_requested: bool,
}

impl Context {
    pub(crate) fn new(step: Duration) -> Self {
        Self {
            input: Input::default(),
            step,
            quit_requested: false,
        }
    }

    /// Current keyboard and mouse state.
    pub fn input(&self) -> &Input {
        &self.input
    }

    /// Length of one update in seconds. Always the same value; multiply
    /// speeds by it to get distance per update.
    pub fn dt(&self) -> f32 {
        self.step.as_secs_f32()
    }

    /// Closes the window and returns from [`crate::run`] after this update.
    pub fn quit(&mut self) {
        self.quit_requested = true;
    }
}

/// One frame to draw into, passed to [`Game::draw`].
#[derive(Debug)]
pub struct Frame {
    pub(crate) clear_color: Color,
}

impl Frame {
    pub(crate) fn new() -> Self {
        Self {
            clear_color: Color::BLACK,
        }
    }

    /// Fills the whole screen with `color`. Black if not called.
    pub fn clear(&mut self, color: Color) {
        self.clear_color = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dt_is_step_in_seconds() {
        let ctx = Context::new(Duration::from_millis(20));
        assert!((ctx.dt() - 0.02).abs() < 1e-6);
    }

    #[test]
    fn quit_sets_flag() {
        let mut ctx = Context::new(Duration::from_millis(20));
        ctx.quit();
        assert!(ctx.quit_requested);
    }

    #[test]
    fn frame_defaults_to_black() {
        assert_eq!(Frame::new().clear_color, Color::BLACK);
    }
}
