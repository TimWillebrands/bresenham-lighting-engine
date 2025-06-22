/**
 * @require $assignable(1, Type)
 */
module ray(<Type>);

import std::math;

def Vec2 = Type[<2>];

macro @line (Vec2 src, Vec2 dst; @body(Vec2))
{
    Vec2 a = src.xy;
    Vec2 b = dst.xy;
    Type dx = (Type) math::abs((int) (b.x - a.x));
    Type dy = (Type)-math::abs((int) (b.y - a.y)); 
    Type sx = a.x < b.x ? 1 : -1;
    Type sy = a.y < b.y ? 1 : -1;
    Type err = dx + dy;
    Type e2; /* error value e_xy */

    for (;;){  /* loop */
        @body (a);
        if (a.x == b.x && a.y == b.y) break;
        e2 = 2 * err;
        if (e2 >= dy) { err += dy; a.x += sx; } /* e_xy+e_x > 0 */
        if (e2 <= dx) { err += dx; a.y += sy; } /* e_xy+e_y < 0 */
    }
}

fn Vec2 step(Vec2 src, Vec2 dst)
{
    Vec2 a = src.xy;
    Vec2 b = dst.xy;
    Type dx = (Type) math::abs((int) (b.x - a.x));
    Type dy = (Type)-math::abs((int) (b.y - a.y)); 
    Type sx = a.x < b.x ? 1 : -1;
    Type sy = a.y < b.y ? 1 : -1;
    Type err = dx + dy;
    Type e2; /* error value e_xy */

    if (a.x != b.x || a.y != b.y) 
    {
        e2 = 2 * err;
        if (e2 >= dy) { err += dy; a.x += sx; } /* e_xy+e_x > 0 */
        if (e2 <= dx) { err += dx; a.y += sy; } /* e_xy+e_y < 0 */
    }
    return a;
}
