//! Fitting the fixed-size game view into a window of any size.

use crate::Vec2;

/// How the game view is scaled to fit the window. Set it with
/// [`crate::Config::scaling`] or change it while running with
/// [`crate::Context::set_scaling`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scaling {
    /// Scale the view as large as the window allows. Fills the most space,
    /// but at in-between sizes (say 2.5x) some game pixels come out one
    /// screen pixel wider than others.
    #[default]
    Fit,
    /// Scale only by whole numbers (1x, 2x, 3x, ...) so every game pixel is
    /// the same size on screen: best for pixel art. Leaves wider black
    /// borders. If the window is smaller than the view, it shrinks to fit
    /// like [`Scaling::Fit`].
    PixelPerfect,
}

/// Where the game view sits inside the window, in physical pixels.
///
/// The view is scaled uniformly, centered, and placed on whole pixels.
/// Leftover space becomes black bars (letterboxing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Viewport {
    /// Size of the game view in world units.
    pub(crate) view_size: Vec2,
    /// Top-left corner of the view inside the window.
    pub(crate) offset: Vec2,
    /// Physical pixels per world unit.
    pub(crate) scale: f32,
    /// The view's size inside the window, in physical pixels.
    size: Vec2,
    /// The mode used to pick `scale`.
    pub(crate) scaling: Scaling,
}

impl Viewport {
    pub(crate) fn fit(
        view_size: Vec2,
        window_width: u32,
        window_height: u32,
        scaling: Scaling,
    ) -> Self {
        let window = Vec2::new(window_width as f32, window_height as f32);
        let fit = (window.x / view_size.x).min(window.y / view_size.y);
        let scale = match scaling {
            // Below 1x there is no whole-number scale, so shrink smoothly.
            Scaling::PixelPerfect if fit >= 1.0 => fit.floor(),
            _ => fit,
        };
        // Whole-pixel offset, so view pixels line up with screen pixels.
        // Rounding can make `view_size * scale` a hair larger than the
        // window, which would push the offset below zero or the view past
        // the window's edge; the GPU rejects both, so clamp them.
        let offset = (window - view_size * scale) / 2.0;
        let offset = Vec2::new(offset.x.floor().max(0.0), offset.y.floor().max(0.0));
        let size = view_size * scale;
        let size = Vec2::new(
            size.x.min(window.x - offset.x),
            size.y.min(window.y - offset.y),
        );
        Self {
            view_size,
            offset,
            scale,
            size,
            scaling,
        }
    }

    /// Converts a window position in physical pixels to world coordinates.
    pub(crate) fn window_to_world(&self, physical: Vec2) -> Vec2 {
        (physical - self.offset) / self.scale
    }

    /// The view's size inside the window, in physical pixels. Never
    /// reaches past the window's edge.
    pub(crate) fn physical_size(&self) -> Vec2 {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Vec2 = Vec2::new(320.0, 180.0);

    fn fit(w: u32, h: u32) -> Viewport {
        Viewport::fit(VIEW, w, h, Scaling::Fit)
    }

    fn pixel_perfect(w: u32, h: u32) -> Viewport {
        Viewport::fit(VIEW, w, h, Scaling::PixelPerfect)
    }

    #[test]
    fn exact_multiple_fills_window() {
        for vp in [fit(1280, 720), pixel_perfect(1280, 720)] {
            assert_eq!(vp.scale, 4.0);
            assert_eq!(vp.offset, Vec2::ZERO);
            assert_eq!(vp.physical_size(), Vec2::new(1280.0, 720.0));
        }
    }

    #[test]
    fn wide_window_gets_side_bars() {
        let vp = fit(1000, 360);
        assert_eq!(vp.scale, 2.0);
        assert_eq!(vp.offset, Vec2::new(180.0, 0.0));
    }

    #[test]
    fn tall_window_gets_top_and_bottom_bars() {
        let vp = fit(640, 600);
        assert_eq!(vp.scale, 2.0);
        assert_eq!(vp.offset, Vec2::new(0.0, 120.0));
    }

    #[test]
    fn fit_uses_in_between_scales() {
        let vp = fit(800, 450);
        assert_eq!(vp.scale, 2.5);
        assert_eq!(vp.offset, Vec2::ZERO);
    }

    #[test]
    fn pixel_perfect_rounds_scale_down() {
        let vp = pixel_perfect(800, 450); // 2.5x fits; use 2x
        assert_eq!(vp.scale, 2.0);
        assert_eq!(vp.physical_size(), Vec2::new(640.0, 360.0));
        assert_eq!(vp.offset, Vec2::new(80.0, 45.0));
    }

    #[test]
    fn offsets_are_whole_pixels() {
        let vp = pixel_perfect(1001, 543); // 3x: 960x540, 41x3 left over
        assert_eq!(vp.scale, 3.0);
        assert_eq!(vp.offset, Vec2::new(20.0, 1.0));
        let vp = fit(641, 360); // 2x: 1 pixel left over
        assert_eq!(vp.offset, Vec2::ZERO);
    }

    #[test]
    fn pixel_perfect_shrinks_in_small_windows() {
        let vp = pixel_perfect(160, 90);
        assert_eq!(vp.scale, 0.5);
        assert_eq!(vp.offset, Vec2::ZERO);
    }

    #[test]
    fn view_stays_inside_window() {
        let mut sizes = vec![(1001, 543), (333, 777), (800, 450), (100, 50), (1, 1)];
        // Plus every size in a range, to catch rounding edge cases.
        sizes.extend((1..400).map(|w| (w, (w * 3 + 7) % 500 + 1)));
        for (w, h) in sizes {
            for vp in [fit(w, h), pixel_perfect(w, h)] {
                let end = vp.offset + vp.physical_size();
                assert!(vp.offset.x >= 0.0 && vp.offset.y >= 0.0, "{vp:?}");
                // Exact: the GPU rejects a viewport even slightly outside.
                assert!(end.x <= w as f32 && end.y <= h as f32, "{vp:?}");
            }
        }
    }

    #[test]
    fn window_to_world_round_trip() {
        let vp = fit(1000, 360);
        assert_eq!(vp.window_to_world(Vec2::new(180.0, 0.0)), Vec2::ZERO);
        assert_eq!(vp.window_to_world(Vec2::new(820.0, 360.0)), VIEW);
        // Points in the side bars map outside the view.
        assert!(vp.window_to_world(Vec2::new(0.0, 0.0)).x < 0.0);

        let vp = pixel_perfect(800, 450);
        assert_eq!(vp.window_to_world(Vec2::new(80.0, 45.0)), Vec2::ZERO);
        assert_eq!(vp.window_to_world(Vec2::new(82.0, 47.0)), Vec2::ONE);
    }
}
