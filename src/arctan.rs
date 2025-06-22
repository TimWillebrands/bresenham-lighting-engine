#![allow(clippy::all)]

#[allow(non_upper_case_globals)]
const ATAN_TABLE: [i32; 256] = [
    0, 0, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 
    6, 7, 7, 7, 8, 8, 9, 9, 9, 10, 10, 11, 11, 11, 12, 12, 
    12, 13, 13, 14, 14, 14, 15, 15, 16, 16, 16, 17, 17, 17, 18, 18, 
    19, 19, 19, 20, 20, 20, 21, 21, 22, 22, 22, 23, 23, 23, 24, 24, 
    25, 25, 25, 26, 26, 26, 27, 27, 28, 28, 28, 29, 29, 29, 30, 30, 
    30, 31, 31, 31, 32, 32, 33, 33, 33, 34, 34, 34, 35, 35, 35, 36, 
    36, 36, 37, 37, 37, 38, 38, 38, 39, 39, 39, 40, 40, 40, 41, 41, 
    41, 42, 42, 42, 43, 43, 43, 44, 44, 44, 45, 45, 45, 46, 46, 46, 
    47, 47, 47, 47, 48, 48, 48, 49, 49, 49, 50, 50, 50, 51, 51, 51, 
    51, 52, 52, 52, 53, 53, 53, 53, 54, 54, 54, 55, 55, 55, 55, 56, 
    56, 56, 57, 57, 57, 57, 58, 58, 58, 59, 59, 59, 59, 60, 60, 60, 
    60, 61, 61, 61, 61, 62, 62, 62, 63, 63, 63, 63, 64, 64, 64, 64, 
    65, 65, 65, 65, 66, 66, 66, 66, 67, 67, 67, 67, 67, 68, 68, 68, 
    68, 69, 69, 69, 69, 70, 70, 70, 70, 71, 71, 71, 71, 71, 72, 72, 
    72, 72, 73, 73, 73, 73, 73, 74, 74, 74, 74, 74, 75, 75, 75, 75, 
    76, 76, 76, 76, 76, 77, 77, 77, 77, 77, 78, 78, 78, 78, 78, 79
];

pub fn atan2_int(y: i32, x: i32) -> i32 {
    if x == 0 && y == 0 { return 0; }

    let ax = x.abs();
    let ay = y.abs();
    let angle;

    if ax >= ay {
        if ax == 0 { return if y >= 0 { 157 } else { -157 }; }
        let atan_index = (ay as i64 * 256 / ax as i64) as usize;
        angle = ATAN_TABLE[atan_index.min(255)];
    } else {
        let atan_index = (ax as i64 * 256 / ay as i64) as usize;
        angle = 157 - ATAN_TABLE[atan_index.min(255)];
    }

    let mut final_angle = if x < 0 { 314 - angle } else { angle };
    if y < 0 { final_angle = -final_angle; }

    (final_angle + 314) % 628 - 314
}

pub fn rad_to_deg(hundredths_radians: i32) -> i32 {
    let degrees = (hundredths_radians * 180) / 314;
    let mut mapped = degrees % 360;
    if mapped < 0 {
        mapped += 360;
    }
    mapped
}

pub fn distance(v: (i16, i16)) -> u16 {
    let x = (v.0.abs()) as u16;
    let y = (v.1.abs()) as u16;

    let (min, max) = if x < y { (x, y) } else { (y, x) };

    (((max << 8) + (max << 3) - (max << 4) - (max << 1) +
      (min << 7) - (min << 5) + (min << 3) - (min << 1)) >> 8)
} 