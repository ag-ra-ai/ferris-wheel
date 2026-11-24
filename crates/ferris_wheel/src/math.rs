//! 2D vectors and rectangles.
//!
//! World coordinates have the origin at the top-left of the view, with x
//! growing right and y growing down.

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2D vector or point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// This vector scaled to length 1, or zero if it has no length.
    pub fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len > 0.0 { self / len } else { Self::ZERO }
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        Self::new(self.x / s, self.y / s)
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}

/// An axis-aligned rectangle given by its top-left corner and size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub pos: Vec2,
    pub size: Vec2,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            pos: Vec2::new(x, y),
            size: Vec2::new(w, h),
        }
    }

    /// A rectangle of `size` centered on `center`.
    pub fn from_center(center: Vec2, size: Vec2) -> Self {
        Self {
            pos: center - size / 2.0,
            size,
        }
    }

    pub fn center(&self) -> Vec2 {
        self.pos + self.size / 2.0
    }

    pub fn left(&self) -> f32 {
        self.pos.x
    }

    pub fn right(&self) -> f32 {
        self.pos.x + self.size.x
    }

    pub fn top(&self) -> f32 {
        self.pos.y
    }

    pub fn bottom(&self) -> f32 {
        self.pos.y + self.size.y
    }

    /// True if `point` is inside. The top and left edges count as inside,
    /// the bottom and right edges do not, so neighbouring rects never both
    /// contain the same point.
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.left()
            && point.x < self.right()
            && point.y >= self.top()
            && point.y < self.bottom()
    }

    /// True if the two rectangles share any area. Touching edges do not count.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.left() < other.right()
            && other.left() < self.right()
            && self.top() < other.bottom()
            && other.top() < self.bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_arithmetic() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 5.0);
        assert_eq!(a + b, Vec2::new(4.0, 7.0));
        assert_eq!(b - a, Vec2::new(2.0, 3.0));
        assert_eq!(a * 2.0, Vec2::new(2.0, 4.0));
        assert_eq!(b / 2.0, Vec2::new(1.5, 2.5));
        assert_eq!(-a, Vec2::new(-1.0, -2.0));
        assert_eq!(a.dot(b), 13.0);
    }

    #[test]
    fn length_and_normalize() {
        assert_eq!(Vec2::new(3.0, 4.0).length(), 5.0);
        assert_eq!(Vec2::new(0.0, 2.0).normalize_or_zero(), Vec2::new(0.0, 1.0));
        assert_eq!(Vec2::ZERO.normalize_or_zero(), Vec2::ZERO);
    }

    #[test]
    fn rect_edges_and_center() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(
            (r.left(), r.right(), r.top(), r.bottom()),
            (10.0, 40.0, 20.0, 60.0)
        );
        assert_eq!(r.center(), Vec2::new(25.0, 40.0));
        assert_eq!(Rect::from_center(r.center(), r.size), r);
    }

    #[test]
    fn contains_includes_top_left_only() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Vec2::new(0.0, 0.0)));
        assert!(r.contains(Vec2::new(5.0, 5.0)));
        assert!(!r.contains(Vec2::new(10.0, 5.0)));
        assert!(!r.contains(Vec2::new(5.0, 10.0)));
    }

    #[test]
    fn overlaps() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.overlaps(&Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert!(a.overlaps(&Rect::new(2.0, 2.0, 2.0, 2.0))); // fully inside
        assert!(!a.overlaps(&Rect::new(10.0, 0.0, 5.0, 5.0))); // touching edge
        assert!(!a.overlaps(&Rect::new(20.0, 20.0, 5.0, 5.0)));
    }
}
