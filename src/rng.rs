//! Deterministic hashing and a tiny seeded RNG (no external crates).

pub fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn hash3(seed: i64, a: i32, b: i32, c: i32) -> u64 {
    let mut h = splitmix64(seed as u64);
    h = splitmix64(h ^ (a as i64 as u64).wrapping_mul(0x9E37_79B1));
    h = splitmix64(h ^ (b as i64 as u64).wrapping_mul(0x85EB_CA77));
    splitmix64(h ^ (c as i64 as u64).wrapping_mul(0xC2B2_AE3D))
}

/// A seed taken from the system clock, for rolls that should differ every run.
pub fn clock_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    splitmix64(nanos)
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = splitmix64(self.0);
        self.0
    }
    /// Uniform integer in `lo..hi` (hi exclusive).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u64) as i32
    }
    /// Returns true with the given probability, in percent.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
    /// Uniform float in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}
