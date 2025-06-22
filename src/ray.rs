use num_traits::{Num, Signed};
use std::ops::{AddAssign, SubAssign};

pub fn step<T>(src: (T, T), dst: (T, T)) -> (T, T)
where
    T: Num + Signed + Copy + PartialOrd + AddAssign + SubAssign,
{
    let (mut x, mut y) = src;
    let (x1, y1) = dst;

    if x == x1 && y == y1 {
        return src;
    }

    let dx = (x1 - x).abs();
    let sx = if x < x1 { T::one() } else { -T::one() };
    let dy = -(y1 - y).abs();
    let sy = if y < y1 { T::one() } else { -T::one() };
    let mut err = dx + dy;

    let e2 = T::one() + T::one();
    let e2 = e2 * err;

    if e2 >= dy {
        err = err + dy;
        x = x + sx;
    }
    if e2 <= dx {
        err = err + dx;
        y = y + sy;
    }
    
    (x, y)
} 