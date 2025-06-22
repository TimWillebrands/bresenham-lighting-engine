import std::collections::list;
import std::collections::map;
import std::math;
import constants;
import arctan;
import ray;

const int DIST = 60;
const int ANGLES = 360;
const int CELLS = 2;

const int LIGHT_ROW = DIST * 2 + 1; // Add 1 to account for the origin point of the light
const int LIGHT_SIZE = LIGHT_ROW * LIGHT_ROW;

const PtI ORIGIN = {0,0};
const PtI LEFT = {-1,0};
const PtI RIGHT = {1,0};
const PtI UP = {0,1};
const PtI DOWN = {0,-1};

PtIList [ANGLES][DIST] all_rays;
Color[constants::CELLS_TOTAL] canvas;
CharLightMap light_map;

struct Light
{
    char id;
    short r;
    PtI pos;

    Color[LIGHT_SIZE] canvas;

    // Every element is at what distance from origin a ray is blocked
    //  it's index is the angle of that ray.
    char[ANGLES] blocked_angles; 
}

fn Color* Light.update(Light* self) {
    // RESET ALL BLOCKED ANGLES ON UPDATE, CACHE NOTHIN
    mem::set(&self.blocked_angles, 255, self.blocked_angles.len * bool.sizeof);
    mem::set(&self.canvas, 0, self.canvas.len * Color.sizeof);

    for DISTANCE: (char d = 0; d < self.r; ++d) {
    for ANGLE: (usz angle = 0; angle < ANGLES; ++angle) {
    foreach CELLS: (q, cell : all_rays[d][angle])
    {
        // Weird, can't log/detect the cells I filter here on init
        if(d == 0 && angle % 90 != 0) continue;
        if(self.blocked_angles[angle] < d) continue;

        PtI curr = cell + self.pos; 
        PtI prev = ray::step(<short>)(curr, self.pos); // Go back one tile towards origin
        if(check_blocked(prev.x, prev.y, curr.x, curr.y))
        {
            PtI p,n; // prev, next

            if(angle == 315){
                p = cell + LEFT;
                n = cell + DOWN;
            }else if(angle > 315 || angle < 45) {
                p = cell + UP;
                n = cell + DOWN;
            }else if(angle == 45) {
                p = cell + UP;
                n = cell + RIGHT;
            }else if(angle > 45 && angle < 135) {
                p = cell + LEFT;
                n = cell + RIGHT;
            }else if(angle == 135) {
                p = cell + LEFT;
                n = cell + UP;
            }else if(angle > 135 && angle < 225) {
                p = cell + DOWN;
                n = cell + UP;
            }else if(angle == 225) {
                p = cell + DOWN;
                n = cell + LEFT;
            }else if(angle > 225 && angle < 315) {
                p = cell + RIGHT;
                n = cell + LEFT;
            }

            int prev_angle = arctan::rad_to_deg(arctan::atan2_int(p.y, p.x));
            int next_angle = arctan::rad_to_deg(arctan::atan2_int(n.y, n.x));

            PtI blocked_domain = {(short)prev_angle, (short)next_angle};

            if(prev_angle < next_angle) {
                // The range crosses the 0/360 boundary
                for (int a = 0; a <= blocked_domain.min(); ++a) {
                    self.blocked_angles[a] = d;
                }
                for (int a = blocked_domain.max(); a < 360; ++a) {
                    self.blocked_angles[a] = d;
                }
            }else{
                for(int a = blocked_domain.min(); a <= blocked_domain.max(); ++a){
                    self.blocked_angles[a % 360] = d;
                }
            }

            continue ANGLE;
        }

        PtI c = cell + (LIGHT_ROW/2);
        int cell_idx = c.x + (c.y * LIGHT_ROW);
        char falloff = 255 - (255 * d) / ((char) self.r );

        self.canvas[cell_idx] = hsv2rgb((char) angle, 255, falloff);
    }}}

    return &self.canvas[0];
}

fn Color hsv2rgb(char h, char s, char v) {
    Color color = {0,0,0,255};
    
    if (s == 0) {
        // If saturation is 0, the color is grayscale
        color.r = color.g = color.b = v;
        return color;
    }
    
    // Sector 0 to 5
    char sector = h / 43;
    // Fraction within sector
    char remainder = (h - (sector * 43)) * 6;
    
    char p = (v * (255 - s)) >> 8;
    char q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    char t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    
    switch (sector) {
        case 0:  color.r = v; color.g = t; color.b = p; break;
        case 1:  color.r = q; color.g = v; color.b = p; break;
        case 2:  color.r = p; color.g = v; color.b = t; break;
        case 3:  color.r = p; color.g = q; color.b = v; break;
        case 4:  color.r = t; color.g = p; color.b = v; break;
        default: color.r = v; color.g = p; color.b = q; break;
    }
    
    return color;
}

fn Color* update_or_add_light(char id, short r, short x, short y) @extern("put") @wasm
{
    Light* l = light_map.@get_or_set(id, mem::new(Light));
    l.id = id;
    l.pos = { x, y };
    l.r = r;
    return l.update();
}

fn void init() @extern("init") @wasm
{
    light_map.new_init();

    PtI center = { 0, 0 };
    int radius = DIST;
    short top    = (short)(center.y - radius);
    short bottom = (short)(center.y + radius);
    short left   = (short)(center.x - radius);
    short right  = (short)(center.x + radius);

    for (short y = top; y <= bottom; y++) {
    for (short x = left; x <= right; x++)
    {
        PtI pt = { x, y };
        ushort dist = arctan::distance(pt);
        if(dist <= radius)
        {
            int angle = arctan::rad_to_deg(arctan::atan2_int(y, x));

            // The cells at this angle & distance
            PtIList* cells = &all_rays[dist][angle];
            if(cells.capacity == 0)
            {
                cells.new_init(2);
            }

            cells.push(pt);
        }
    }}
}

def PtI = short[<2>];
def Color = char[<4>];
def CharLightMap = HashMap(<char, Light*>);
def PtIList = List(<PtI>);

extern fn bool check_blocked(short x0, short y0, short x1, short y1) @extern("IsBlocked");
extern fn bool jslog(int l0, int l1, int l2) @extern("Log");
