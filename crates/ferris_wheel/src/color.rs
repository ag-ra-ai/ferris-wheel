/// An RGBA color with components from `0.0` to `1.0`.
///
/// Values are in sRGB space, the same space as colors picked in an image editor
/// or written as hex codes. The engine converts to linear space for the GPU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const RED: Self = Self::rgb(1.0, 0.0, 0.0);
    pub const GREEN: Self = Self::rgb(0.0, 1.0, 0.0);
    pub const BLUE: Self = Self::rgb(0.0, 0.0, 1.0);
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

    /// An opaque color.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// A color from a hex value such as `0xff8800`. Alpha is 1.0.
    pub const fn hex(rgb: u32) -> Self {
        Self::rgb(
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
        )
    }

    /// The same color in linear space. Alpha is unchanged.
    pub fn to_linear(self) -> Self {
        Self {
            r: srgb_to_linear(self.r),
            g: srgb_to_linear(self.g),
            b: srgb_to_linear(self.b),
            a: self.a,
        }
    }
}

/// The standard sRGB transfer function (IEC 61966-2-1).
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl From<Color> for wgpu::Color {
    fn from(c: Color) -> Self {
        Self {
            r: c.r.into(),
            g: c.g.into(),
            b: c.b.into(),
            a: c.a.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn hex_splits_channels() {
        assert_eq!(Color::hex(0xff0000), Color::RED);
        assert_eq!(Color::hex(0x00ff00), Color::GREEN);
        assert_eq!(Color::hex(0x0000ff), Color::BLUE);
    }

    #[test]
    fn linear_keeps_endpoints_and_alpha() {
        let c = Color::rgba(0.0, 1.0, 1.0, 0.5).to_linear();
        assert_eq!(c, Color::rgba(0.0, 1.0, 1.0, 0.5));
    }

    #[test]
    fn linear_matches_reference_values() {
        // sRGB 0.5 is about 0.214 linear; the low end is a straight line.
        assert!(close(srgb_to_linear(0.5), 0.21404));
        assert!(close(srgb_to_linear(0.04), 0.04 / 12.92));
    }
}
