//! Randomness as an input: rolls and fresh yids come from a seeded stream the host picks.

pub trait Rng {
    fn next_u32(&mut self) -> u32;

    /// Uniform in [0, 1).
    fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4_294_967_296.0
    }

    /// Fresh yid: a non-zero u32, as the hosts' `getRandomNonZeroUInt32`.
    fn next_yid(&mut self) -> String {
        loop {
            let v = self.next_u32();
            if v != 0 {
                return v.to_string();
            }
        }
    }
}

/// SplitMix64: tiny, seedable, good enough for dice.
pub struct SplitMix64(pub u64);

impl Rng for SplitMix64 {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 32) as u32
    }
}

/// Scripted rolls for tests; yids count up from 1.
pub struct Script {
    pub rolls: Vec<f64>,
    pub next: usize,
    pub yid: u32,
}

impl Script {
    /// Every roll reads `v`.
    pub fn always(v: f64) -> Self {
        Script {
            rolls: vec![v],
            next: 0,
            yid: 0,
        }
    }

    pub fn of(rolls: &[f64]) -> Self {
        Script {
            rolls: rolls.to_vec(),
            next: 0,
            yid: 0,
        }
    }
}

impl Rng for Script {
    fn next_u32(&mut self) -> u32 {
        self.yid += 1;
        self.yid
    }

    fn next_f64(&mut self) -> f64 {
        let v = self.rolls[self.next.min(self.rolls.len() - 1)];
        self.next += 1;
        v
    }
}
