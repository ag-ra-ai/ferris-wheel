//! The trait a game implements, and what the engine passes to it.

use std::time::Duration;

use crate::viewport::Viewport;
use crate::{Color, Input, Rect, Vec2};

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
    pub(crate) view_size: Vec2,
    pub(crate) quit_requested: bool,
}

impl Context {
    pub(crate) fn new(step: Duration, viewport: Viewport) -> Self {
        Self {
            input: Input::new(viewport),
            step,
            view_size: viewport.view_size,
            quit_requested: false,
        }
    }

    /// Size of the game view in world units ([`crate::Config::resolution`]).
    pub fn view_size(&self) -> Vec2 {
        self.view_size
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
///
/// Coordinates are world units: (0, 0) is the top-left of the view and
/// [`Frame::view_size`] is the bottom-right. Shapes are drawn in call order,
/// so later shapes cover earlier ones.
#[derive(Debug)]
pub struct Frame {
    pub(crate) clear_color: Color,
    pub(crate) quads: Vec<Quad>,
    view_size: Vec2,
}

/// A filled rectangle waiting to be drawn.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Quad {
    pub(crate) rect: Rect,
    pub(crate) color: Color,
}

impl Frame {
    pub(crate) fn new(view_size: Vec2) -> Self {
        Self {
            clear_color: Color::BLACK,
            quads: Vec::new(),
            view_size,
        }
    }

    /// Empties the frame for reuse, keeping its allocation.
    pub(crate) fn reset(&mut self) {
        self.clear_color = Color::BLACK;
        self.quads.clear();
    }

    /// Size of the game view in world units.
    pub fn view_size(&self) -> Vec2 {
        self.view_size
    }

    /// Fills the whole view with `color`. Black if not called.
    pub fn clear(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// Draws a filled rectangle.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.quads.push(Quad { rect, color });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Vec2 = Vec2::new(320.0, 180.0);

    #[test]
    fn dt_is_step_in_seconds() {
        let ctx = Context::new(Duration::from_millis(20), Viewport::fit(VIEW, 640, 360));
        assert!((ctx.dt() - 0.02).abs() < 1e-6);
    }

    #[test]
    fn quit_sets_flag() {
        let mut ctx = Context::new(Duration::from_millis(20), Viewport::fit(VIEW, 640, 360));
        ctx.quit();
        assert!(ctx.quit_requested);
    }

    #[test]
    fn frame_defaults_to_black() {
        assert_eq!(Frame::new(VIEW).clear_color, Color::BLACK);
    }

    #[test]
    fn frame_collects_rects_in_order_and_resets() {
        let mut frame = Frame::new(VIEW);
        frame.clear(Color::WHITE);
        frame.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        frame.fill_rect(Rect::new(1.0, 1.0, 1.0, 1.0), Color::BLUE);
        let colors: Vec<_> = frame.quads.iter().map(|q| q.color).collect();
        assert_eq!(colors, [Color::RED, Color::BLUE]);

        frame.reset();
        assert!(frame.quads.is_empty());
        assert_eq!(frame.clear_color, Color::BLACK);
        assert_eq!(frame.view_size(), VIEW);
    }
}
