//! Ray stepping implementation using Bresenham-style line algorithm.
//!
//! This module provides utilities for stepping along lines between points,
//! which is essential for ray casting in the lighting engine. The algorithm
//! is based on Bresenham's line algorithm but adapted for single-step movement.

use num_traits::{Num, Signed};
use std::ops::{AddAssign, SubAssign};

/// Performs a single step along a line from source towards destination.
///
/// This function implements a Bresenham-style line stepping algorithm that
/// moves one pixel at a time along the line connecting two points. It's used
/// in the lighting engine to trace rays backwards from obstacles to light sources.
///
/// # Algorithm
///
/// The function uses Bresenham's line algorithm decision variables to determine
/// whether to step horizontally, vertically, or diagonally. The error term
/// tracks the cumulative error and decides the direction of the next step.
///
/// # Arguments
///
/// * `src` - Starting point (x, y) coordinates
/// * `dst` - Destination point (x, y) coordinates
///
/// # Returns
///
/// The next point along the line from `src` towards `dst`. If `src` equals `dst`,
/// returns `src` unchanged.
///
/// # Type Parameters
///
/// * `T` - Numeric type that supports basic arithmetic operations. Must implement
///   `Num`, `Signed`, `Copy`, `PartialOrd`, `AddAssign`, and `SubAssign`.
///
/// # Examples
///
/// ```
/// use bresenham_lighting_engine::ray::step;
///
/// // Step from (0,0) towards (5,3)
/// let start = (0, 0);
/// let end = (5, 3);
/// let next = step(start, end);
/// // next will be (1, 0) or (1, 1) depending on the line slope
/// ```
///
/// # Performance
///
/// This function performs constant-time operations and is suitable for
/// real-time applications. The algorithm avoids floating-point arithmetic
/// entirely, making it fast and deterministic.
pub fn step<T>(src: (T, T), dst: (T, T)) -> (T, T)
where
    T: Num + Signed + Copy + PartialOrd + AddAssign + SubAssign,
{
    let (mut x, mut y) = src;
    let (x1, y1) = dst;

    // If we're already at the destination, don't move
    if x == x1 && y == y1 {
        return src;
    }

    // Calculate the absolute differences in x and y directions
    let dx = (x1 - x).abs();
    let sx = if x < x1 { T::one() } else { -T::one() };
    let dy = -(y1 - y).abs();
    let sy = if y < y1 { T::one() } else { -T::one() };

    // Initialize the error term (decision variable)
    let mut err = dx + dy;

    // Calculate 2 * err for decision making
    let e2 = T::one() + T::one();
    let e2_err = e2 * err;

    // Decide whether to step in x direction
    if e2_err >= dy {
        err += dy;
        x += sx;
    }

    // Decide whether to step in y direction
    if e2_err <= dx {
        err += dx;
        y += sy;
    }

    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_same_point() {
        let point = (5, 3);
        assert_eq!(step(point, point), point);
    }

    #[test]
    fn test_step_horizontal_line() {
        let start = (0, 0);
        let end = (10, 0);
        let next = step(start, end);
        assert_eq!(next, (1, 0));
    }

    #[test]
    fn test_step_vertical_line() {
        let start = (0, 0);
        let end = (0, 10);
        let next = step(start, end);
        assert_eq!(next, (0, 1));
    }

    #[test]
    fn test_step_diagonal_line() {
        let start = (0, 0);
        let end = (5, 5);
        let next = step(start, end);
        assert_eq!(next, (1, 1));
    }

    #[test]
    fn test_step_negative_direction() {
        let start = (5, 5);
        let end = (0, 0);
        let next = step(start, end);
        assert_eq!(next, (4, 4));
    }
}
