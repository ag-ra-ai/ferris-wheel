//! Fitting the fixed-size game view into a window of any size.

use crate::Vec2;

/// Where the game view sits inside the window, in physical pixels.
///
/// The view is scaled uniformly to the largest size that fits, and centered.
/// Leftover space becomes black bars (letterboxing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Viewport {
    /// Size of the game view in world units.
    pub(crate) view_size: Vec2,
    /// Top-left corner of the view inside the window.
    pub(crate) offset: Vec2,
    /// Physical pixels per world unit.
    pub(crate) scale: f32,
}

impl Viewport {
    pub(crate) fn fit(view_size: Vec2, window_width: u32, window_height: u32) -> Self {
        let window = Vec2::new(window_width as f32, window_height as f32);
        let scale = (window.x / view_size.x).min(window.y / view_size.y);
        let offset = (window - view_size * scale) / 2.0;
        Self {
            view_size,
            offset,
            scale,
        }
    }

    /// Converts a window position in physical pixels to world coordinates.
    pub(crate) fn window_to_world(&self, physical: Vec2) -> Vec2 {
        (physical - self.offset) / self.scale
    }

    /// The view's size inside the window, in physical pixels.
    pub(crate) fn physical_size(&self) -> Vec2 {
        self.view_size * self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Vec2 = Vec2::new(320.0, 180.0);

    #[test]
    fn exact_multiple_fills_window() {
        let vp = Viewport::fit(VIEW, 1280, 720);
        assert_eq!(vp.scale, 4.0);
        assert_eq!(vp.offset, Vec2::ZERO);
        assert_eq!(vp.physical_size(), Vec2::new(1280.0, 720.0));
    }

    #[test]
    fn wide_window_gets_side_bars() {
        let vp = Viewport::fit(VIEW, 1000, 360);
        assert_eq!(vp.scale, 2.0);
        assert_eq!(vp.offset, Vec2::new(180.0, 0.0));
    }

    #[test]
    fn tall_window_gets_top_and_bottom_bars() {
        let vp = Viewport::fit(VIEW, 640, 600);
        assert_eq!(vp.scale, 2.0);
        assert_eq!(vp.offset, Vec2::new(0.0, 120.0));
    }

    #[test]
    fn window_to_world_round_trip() {
        let vp = Viewport::fit(VIEW, 1000, 360);
        assert_eq!(vp.window_to_world(Vec2::new(180.0, 0.0)), Vec2::ZERO);
        assert_eq!(vp.window_to_world(Vec2::new(820.0, 360.0)), VIEW);
        // Points in the side bars map outside the view.
        assert!(vp.window_to_world(Vec2::new(0.0, 0.0)).x < 0.0);
    }
}
