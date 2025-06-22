use std::collections::HashMap;
use once_cell::sync::Lazy;

use crate::{
    arctan,
    is_blocked, ray,
};

const DIST: usize = 60;
const ANGLES: usize = 360;

const LIGHT_ROW: usize = DIST * 2 + 1;
const LIGHT_SIZE: usize = LIGHT_ROW * LIGHT_ROW;

type PtI = (i16, i16);
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

static ALL_RAYS: Lazy<[[Vec<PtI>; ANGLES]; DIST]> = Lazy::new(|| {
    let mut rays: [[Vec<PtI>; ANGLES]; DIST] = std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()));
    
    let center = (0, 0);
    let radius = DIST as i16;
    let top = center.1 - radius;
    let bottom = center.1 + radius;
    let left = center.0 - radius;
    let right = center.0 + radius;

    for y in top..=bottom {
        for x in left..=right {
            let pt = (x, y);
            let dist = arctan::distance(pt);
            if dist <= radius as u16 {
                let angle = arctan::rad_to_deg(arctan::atan2_int(y as i32, x as i32)) as usize;
                if angle < ANGLES && (dist as usize) < DIST {
                    rays[dist as usize][angle].push(pt);
                }
            }
        }
    }
    rays
});

static mut LIGHT_MAP: Option<HashMap<u8, Light>> = None;

struct Light {
    pos: PtI,
    r: i16,
    canvas: [Color; LIGHT_SIZE],
    blocked_angles: [u8; ANGLES],
}

impl Light {
    fn new(pos: PtI, r: i16) -> Self {
        Light {
            pos,
            r,
            canvas: [Color(0, 0, 0, 0); LIGHT_SIZE],
            blocked_angles: [255; ANGLES],
        }
    }

    fn update(&mut self) -> *const Color {
        self.blocked_angles.fill(255);
        self.canvas.iter_mut().for_each(|p| *p = Color(0,0,0,0));

        for d in 0..self.r as u8 {
            'angle_loop: for angle in 0..ANGLES {
                if self.blocked_angles[angle] < d {
                    continue;
                }

                for cell in &ALL_RAYS[d as usize][angle] {
                     if d == 0 && angle % 90 != 0 { continue; }

                    let curr = (cell.0 + self.pos.0, cell.1 + self.pos.1);
                    let prev = ray::step(curr, self.pos);

                    if is_blocked(prev.0, prev.1, curr.0, curr.1) {
                        let (p, n) = match angle {
                            315 => ((-1, 0), (0, -1)),
                            a if a > 315 || a < 45 => ((0, 1), (0, -1)),
                            45 => ((0, 1), (1, 0)),
                            a if a > 45 && a < 135 => ((-1, 0), (1, 0)),
                            135 => ((-1, 0), (0, 1)),
                            a if a > 135 && a < 225 => ((0, -1), (0, 1)),
                            225 => ((0, -1), (-1, 0)),
                            _ => ((1, 0), (-1, 0)),
                        };

                        let p = (cell.0 + p.0, cell.1 + p.1);
                        let n = (cell.0 + n.0, cell.1 + n.1);

                        let prev_angle = arctan::rad_to_deg(arctan::atan2_int(p.1 as i32, p.0 as i32)) as usize;
                        let next_angle = arctan::rad_to_deg(arctan::atan2_int(n.1 as i32, n.0 as i32)) as usize;

                        if prev_angle < next_angle {
                            for a in 0..=prev_angle { self.blocked_angles[a] = d; }
                            for a in next_angle..ANGLES { self.blocked_angles[a] = d; }
                        } else {
                            for a in next_angle..=prev_angle { self.blocked_angles[a % ANGLES] = d; }
                        }
                        continue 'angle_loop;
                    }

                    let c = (cell.0 + LIGHT_ROW as i16 / 2, cell.1 + LIGHT_ROW as i16 / 2);
                    let cell_idx = c.0 as usize + c.1 as usize * LIGHT_ROW;
                    let falloff = 255 - (255 * d as u16) / (self.r as u16);

                    if cell_idx < self.canvas.len() {
                       self.canvas[cell_idx] = hsv2rgb(angle as u8, 255, falloff as u8);
                    }
                }
            }
        }
        self.canvas.as_ptr()
    }
}

fn hsv2rgb(h: u8, s: u8, v: u8) -> Color {
    if s == 0 {
        return Color(v, v, v, 255);
    }
    let sector = h / 43;
    let remainder = (h - (sector * 43)) * 6;
    let p = (v as u16 * (255 - s) as u16 / 255) as u8;
    let q = (v as u16 * (255 - (s as u16 * remainder as u16 / 255)) / 255) as u8;
    let t = (v as u16 * (255 - (s as u16 * (255 - remainder) as u16 / 255)) / 255) as u8;
    match sector {
        0 => Color(v, t, p, 255),
        1 => Color(q, v, p, 255),
        2 => Color(p, v, t, 255),
        3 => Color(p, q, v, 255),
        4 => Color(t, p, v, 255),
        _ => Color(v, p, q, 255),
    }
}

pub fn update_or_add_light(id: u8, r: i16, x: i16, y: i16) -> *const Color {
    unsafe {
        if let Some(light_map) = &mut LIGHT_MAP {
            let light = light_map.entry(id).or_insert_with(|| Light::new((x, y), r));
            light.pos = (x, y);
            light.r = r;
            light.update()
        } else {
            std::ptr::null()
        }
    }
}

pub fn init() {
    Lazy::force(&ALL_RAYS);
    unsafe {
        LIGHT_MAP = Some(HashMap::new());
    }
} 