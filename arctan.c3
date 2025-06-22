module arctan;

import std::math;
import std::io;

// Define a 2D vector type
def Vec2 = short[<2>];

// Lookup table for arctangent values (multiplied by 100 for precision)
const int[256] ATAN_TABLE = {
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
};
// const int[256] ATAN_TABLE = {
//     0, 4, 8, 12, 16, 20, 24, 27, 31, 35, 39, 43, 47, 51, 55, 59, 
//     63, 67, 70, 74, 78, 82, 86, 90, 94, 98, 102, 105, 109, 113, 117, 121, 
//     125, 129, 133, 136, 140, 144, 148, 152, 156, 159, 163, 167, 171, 175, 178, 182, 
//     186, 190, 194, 197, 201, 205, 209, 212, 216, 220, 224, 227, 231, 235, 239, 242, 
//     246, 250, 253, 257, 261, 264, 268, 272, 275, 279, 282, 286, 290, 293, 297, 300, 
//     304, 308, 311, 315, 318, 322, 325, 329, 332, 336, 339, 343, 346, 350, 353, 357, 
//     360, 363, 367, 370, 374, 377, 381, 384, 387, 391, 394, 397, 401, 404, 407, 411, 
//     414, 417, 420, 424, 427, 430, 433, 437, 440, 443, 446, 449, 453, 456, 459, 462, 
//     465, 468, 471, 475, 478, 481, 484, 487, 490, 493, 496, 499, 502, 505, 508, 511, 
//     514, 517, 520, 523, 526, 529, 532, 535, 538, 540, 543, 546, 549, 552, 555, 558, 
//     560, 563, 566, 569, 572, 574, 577, 580, 583, 585, 588, 591, 593, 596, 599, 601, 
//     604, 607, 609, 612, 615, 617, 620, 622, 625, 628, 630, 633, 635, 638, 640, 643, 
//     645, 648, 650, 653, 655, 658, 660, 663, 665, 668, 670, 672, 675, 677, 680, 682, 
//     684, 687, 689, 691, 694, 696, 698, 700, 703, 705, 707, 710, 712, 714, 716, 719, 
//     721, 723, 725, 727, 730, 732, 734, 736, 738, 740, 742, 745, 747, 749, 751, 753, 
//     755, 757, 759, 761, 763, 765, 767, 769, 771, 773, 775, 777, 779, 781, 783, 785
// };

// Function to calculate integer square root
fn int int_sqrt(int x)
{
    int r = 0;
    int t, b;

    for (b = 0x40000000; b != 0; b >>= 2)
    {
        t = r + b;
        r >>= 1;
        if (x >= t)
        {
            x -= t;
            r += b;
        }
    }
    return r;
}

// Function to calculate arctangent using integer lookup table
fn int atan2_int(int y, int x)
{
    if (x == 0 && y == 0) return 0;

    int ax = math::abs(x);
    int ay = math::abs(y);
    int angle;

    if (ax >= ay)
    {
        if (ax == 0) return (y >= 0) ? 157 : -157;
        int atan_index = (int)((long)ay * 256 / ax);
        if (atan_index >= 256) atan_index = 255;  // Prevent overflow
        angle = ATAN_TABLE[atan_index];
    }
    else
    {
        int atan_index = (int)((long)ax * 256 / ay);
        if (atan_index >= 256) atan_index = 255;  // Prevent overflow
        angle = 157 - ATAN_TABLE[atan_index];
    }

    if (x < 0) angle = 314 - angle;
    if (y < 0) angle = -angle;

    return (angle + 314) % 628 - 314;  // Keep within range [-314, 314]
}

// Function to calculate the angle between two Vec2
fn int angle_between(Vec2 p1, Vec2 p2)
{
    return atan2_int(p2.y, p2.x) - atan2_int(p1.y, p1.x);
    // // Calculate dot product
    // int dot_product = p1.x * p2.x + p1.y * p2.y;
    // 
    // // Calculate squared magnitudes
    // int mag1_squared = p1.x * p1.x + p1.y * p1.y;
    // int mag2_squared = p2.x * p2.x + p2.y * p2.y;
    // 
    // // Calculate magnitudes
    // int mag1 = int_sqrt(mag1_squared);
    // int mag2 = int_sqrt(mag2_squared);
    // 
    // // Calculate cross product to determine direction
    // int cross_product = p1.x * p2.y - p1.y * p2.x;

    // // Use atan2 to get the angle
    // return atan2_int(cross_product, dot_product);
}

/**
 * @require hundredths_radians > -314, hundredths_radians < 314, "radians out of bounds"
 **/
fn int rad_to_deg(int hundredths_radians)
{
    // Convert hundredths of radians to degrees
    int degrees = (hundredths_radians * 180) / 314;

    // Map to the range [0, 359] using modulo
    degrees = degrees % 360;
    
    // Handle negative values (to ensure they are positive in the range [0, 359])
    if (degrees < 0) degrees += 360;

    return degrees;
}

/**
 * Fast approximate distance
 * https://www.flipcode.com/archives/Fast_Approximate_Distance_Functions.shtml
 **/
fn ushort distance(Vec2 v)
{
   ushort min, max;
   ushort x = (ushort) math::abs(v.x);
   ushort y = (ushort) math::abs(v.y);

   if ( x < y )
   {
      min = x;
      max = y;
   } else {
      min = y;
      max = x;
   }

   return ((( max << 8 ) + ( max << 3 ) - ( max << 4 ) - ( max << 1 ) +
            ( min << 7 ) - ( min << 5 ) + ( min << 3 ) - ( min << 1 )) >> 8 );
}
