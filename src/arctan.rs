//! Fast integer-based trigonometric functions for lighting calculations.
//!
//! This module provides optimized integer arithmetic implementations of
//! trigonometric functions needed for the lighting engine. By avoiding
//! floating-point operations, these functions are faster and more predictable
//! on systems without dedicated floating-point units.
//!
//! # Implementation Notes
//!
//! - All angles are represented in "hundredths of radians" (1/100 radian units)
//! - Uses lookup tables for arctan calculations to avoid expensive division
//! - Distance calculations use bit-shifting approximations for performance
//! - Results are deterministic across all platforms and architectures
//!
//! # Coordinate System
//!
//! The functions use a standard mathematical coordinate system where:
//! - Positive X is to the right
//! - Positive Y is upward
//! - Angles are measured counter-clockwise from the positive X axis

#![allow(clippy::all)]

/// Lookup table for arctangent values in hundredths of radians.
///
/// This table contains pre-computed arctangent values for the range [0, 1]
/// with 256 discrete steps. The values are scaled to hundredths of radians
/// to maintain precision while using integer arithmetic.
///
/// The table maps input values from 0 to 255 (representing 0.0 to 1.0)
/// to their corresponding arctangent values in hundredths of radians.
#[allow(non_upper_case_globals)]
const ATAN_TABLE: [i32; 256] = [
    0, 0, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 7, 7, 7, 8, 8, 9, 9, 9, 10, 10, 11, 11, 11,
    12, 12, 12, 13, 13, 14, 14, 14, 15, 15, 16, 16, 16, 17, 17, 17, 18, 18, 19, 19, 19, 20, 20, 20,
    21, 21, 22, 22, 22, 23, 23, 23, 24, 24, 25, 25, 25, 26, 26, 26, 27, 27, 28, 28, 28, 29, 29, 29,
    30, 30, 30, 31, 31, 31, 32, 32, 33, 33, 33, 34, 34, 34, 35, 35, 35, 36, 36, 36, 37, 37, 37, 38,
    38, 38, 39, 39, 39, 40, 40, 40, 41, 41, 41, 42, 42, 42, 43, 43, 43, 44, 44, 44, 45, 45, 45, 46,
    46, 46, 47, 47, 47, 47, 48, 48, 48, 49, 49, 49, 50, 50, 50, 51, 51, 51, 51, 52, 52, 52, 53, 53,
    53, 53, 54, 54, 54, 55, 55, 55, 55, 56, 56, 56, 57, 57, 57, 57, 58, 58, 58, 59, 59, 59, 59, 60,
    60, 60, 60, 61, 61, 61, 61, 62, 62, 62, 63, 63, 63, 63, 64, 64, 64, 64, 65, 65, 65, 65, 66, 66,
    66, 66, 67, 67, 67, 67, 67, 68, 68, 68, 68, 69, 69, 69, 69, 70, 70, 70, 70, 71, 71, 71, 71, 71,
    72, 72, 72, 72, 73, 73, 73, 73, 73, 74, 74, 74, 74, 74, 75, 75, 75, 75, 76, 76, 76, 76, 76, 77,
    77, 77, 77, 77, 78, 78, 78, 78, 78, 79,
];

/// Computes the arctangent of y/x using integer arithmetic.
///
/// This function is equivalent to the standard `atan2` function but operates
/// entirely in integer arithmetic for better performance and determinism.
/// The result is returned in hundredths of radians.
///
/// # Arguments
///
/// * `y` - Y coordinate (vertical component)
/// * `x` - X coordinate (horizontal component)
///
/// # Returns
///
/// The angle in hundredths of radians, ranging from approximately -314 to +314
/// (equivalent to -π to +π radians). The angle is measured counter-clockwise
/// from the positive X axis.
///
/// # Special Cases
///
/// * If both x and y are zero, returns 0
/// * Handles all quadrants correctly
/// * Uses lookup table for fast computation
///
/// # Examples
///
/// ```
/// use bresenham_lighting_engine::arctan::atan2_int;
///
/// // 45-degree angle (northeast direction)
/// let angle = atan2_int(1, 1);
///
/// // 90-degree angle (north direction)
/// let angle = atan2_int(1, 0);
///
/// // 180-degree angle (west direction)
/// let angle = atan2_int(0, -1);
/// ```
pub fn atan2_int(y: i32, x: i32) -> i32 {
    // Handle the degenerate case where both coordinates are zero
    if x == 0 && y == 0 {
        return 0;
    }

    // Work with absolute values to simplify the calculation
    let ax = x.abs();
    let ay = y.abs();
    let angle;

    // Determine which lookup method to use based on the slope
    if ax >= ay {
        // Slope is <= 1, use direct lookup
        if ax == 0 {
            // Special case: vertical line
            return if y >= 0 { 157 } else { -157 }; // ±π/2 in hundredths of radians
        }
        // Calculate table index: (ay/ax) * 256
        let atan_index = (ay as i64 * 256 / ax as i64) as usize;
        angle = ATAN_TABLE[atan_index.min(255)];
    } else {
        // Slope is > 1, use complementary angle
        let atan_index = (ax as i64 * 256 / ay as i64) as usize;
        angle = 157 - ATAN_TABLE[atan_index.min(255)]; // π/2 - atan(ax/ay)
    }

    // Adjust angle based on quadrant
    let mut final_angle = if x < 0 {
        314 - angle // π - angle for quadrants II and III
    } else {
        angle
    };

    // Handle negative Y values (quadrants III and IV)
    if y < 0 {
        final_angle = -final_angle;
    }

    // Normalize to [-π, π] range (in hundredths of radians)
    (final_angle + 314) % 628 - 314
}

/// Converts angle from hundredths of radians to degrees.
///
/// This function converts the custom angle representation used internally
/// (hundredths of radians) to standard degrees for easier interpretation
/// and debugging.
///
/// # Arguments
///
/// * `hundredths_radians` - Angle in hundredths of radians (π ≈ 314)
///
/// # Returns
///
/// The equivalent angle in degrees, normalized to the range [0, 360).
///
/// # Examples
///
/// ```
/// use bresenham_lighting_engine::arctan::rad_to_deg;
///
/// // Convert π/2 (90 degrees)
/// let degrees = rad_to_deg(157); // ≈ 90
///
/// // Convert π (180 degrees)
/// let degrees = rad_to_deg(314); // ≈ 180
/// ```
pub fn rad_to_deg(hundredths_radians: i32) -> i32 {
    // Convert using the ratio: 180° / π ≈ 180 / 3.14159 ≈ 180 / 314 (hundredths)
    let degrees = (hundredths_radians * 180) / 314;

    // Normalize to [0, 360) range
    let mut mapped = degrees % 360;
    if mapped < 0 {
        mapped += 360;
    }
    mapped
}

/// Calculates the approximate distance from origin to a point using integer arithmetic.
///
/// This function provides a fast approximation of the Euclidean distance
/// without using expensive square root operations. The approximation is
/// based on bit manipulation and is suitable for lighting distance calculations
/// where perfect accuracy is less important than performance.
///
/// # Algorithm
///
/// The function uses a clever bit-shifting approximation that provides
/// reasonable accuracy (typically within 5-10% of the true distance)
/// while being much faster than `sqrt(x² + y²)`.
///
/// # Arguments
///
/// * `v` - Point coordinates as (x, y) tuple
///
/// # Returns
///
/// Approximate distance from origin as an unsigned 16-bit integer.
/// The maximum representable distance is 65,535 units.
///
/// # Examples
///
/// ```
/// use bresenham_lighting_engine::arctan::distance;
///
/// // Distance to (3, 4) should be approximately 5
/// let dist = distance((3, 4));
///
/// // Distance to (0, 0) is 0
/// let dist = distance((0, 0));
/// ```
///
/// # Performance
///
/// This function is significantly faster than floating-point distance
/// calculations and provides consistent results across all platforms.
pub fn distance(v: (i16, i16)) -> u16 {
    // Work with absolute values to avoid issues with negative coordinates
    let x = (v.0.abs()) as u16;
    let y = (v.1.abs()) as u16;

    // Order coordinates so that max >= min
    let (min, max) = if x < y { (x, y) } else { (y, x) };

    // Fast distance approximation using bit operations
    // This approximates: distance ≈ max + 0.5 * min
    // The bit operations implement: max + 0.428 * min (a more accurate coefficient)
    (max << 8) + (max << 3) - (max << 4) - (max << 1) + (min << 7) - (min << 5) + (min << 3)
        - (min << 1)
        >> 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atan2_int_basic_directions() {
        // Test cardinal directions
        assert_eq!(rad_to_deg(atan2_int(0, 1)), 0); // East: 0°
        assert_eq!(rad_to_deg(atan2_int(1, 0)), 90); // North: 90°
        assert_eq!(rad_to_deg(atan2_int(0, -1)), 180); // West: 180°
        assert_eq!(rad_to_deg(atan2_int(-1, 0)), 270); // South: 270°
    }

    #[test]
    fn test_atan2_int_diagonal_directions() {
        // Test diagonal directions (45° increments)
        let ne = rad_to_deg(atan2_int(1, 1)); // Northeast: ~45°
        let nw = rad_to_deg(atan2_int(1, -1)); // Northwest: ~135°
        let sw = rad_to_deg(atan2_int(-1, -1)); // Southwest: ~225°
        let se = rad_to_deg(atan2_int(-1, 1)); // Southeast: ~315°

        assert!((ne - 45).abs() <= 2); // Allow small error
        assert!((nw - 135).abs() <= 2);
        assert!((sw - 225).abs() <= 2);
        assert!((se - 315).abs() <= 2);
    }

    #[test]
    fn test_atan2_int_origin() {
        // Origin should return 0
        assert_eq!(atan2_int(0, 0), 0);
    }

    #[test]
    fn test_distance_approximation() {
        // Test known distances
        assert_eq!(distance((0, 0)), 0);
        assert_eq!(distance((3, 4)), 5); // Classic 3-4-5 triangle
        assert_eq!(distance((5, 12)), 13); // 5-12-13 triangle

        // Test symmetry
        assert_eq!(distance((3, 4)), distance((-3, 4)));
        assert_eq!(distance((3, 4)), distance((3, -4)));
        assert_eq!(distance((3, 4)), distance((-3, -4)));
    }

    #[test]
    fn test_rad_to_deg_conversion() {
        // Test common angles
        assert_eq!(rad_to_deg(0), 0);
        assert_eq!(rad_to_deg(157), 90); // π/2 ≈ 157 hundredths
        assert_eq!(rad_to_deg(314), 180); // π ≈ 314 hundredths

        // Test negative angles
        assert_eq!(rad_to_deg(-157), 270); // -π/2 → 270°
    }
}
