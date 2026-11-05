//! Rivers: winding bands of shallow water that run across the world.
//!
//! A river follows a zero contour of a smooth noise field (`field`): where the field crosses zero the
//! land is wet. Contours of a smooth field are continuous curves that bend, branch and never end,
//! which is what a river should do. Two lattices of value noise are mixed, a wide one that decides
//! where the rivers run and a narrow one that makes them wind. A column is wet when it is within
//! the river's half-width of the contour; the distance is estimated as `|field| / |gradient|`, which
//! keeps the width even instead of letting the band swell into a pond where the field is flat.
//!
//! The field is lifted around the spawn point, so every contour (and so every river) bends round it and
//! the spawn area stays dry. Everything here is a pure function of the seed and the column, so chunks
//! agree on where the water is without asking each other.

use crate::config::*;
use crate::rng::hash3;

/// Quintic ease, so the noise has a continuous slope and the rivers have no corners.
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Smooth value noise in -1..1 on a square lattice with `scale` blocks between the points.
fn value_noise(seed: i64, salt: i32, x: f32, z: f32, scale: f32) -> f32 {
    let (fx, fz) = (x / scale, z / scale);
    let (ix, iz) = (fx.floor(), fz.floor());
    let (tx, tz) = (fade(fx - ix), fade(fz - iz));
    let (ix, iz) = (ix as i32, iz as i32);
    let corner = |dx: i32, dz: i32| (hash3(seed, ix + dx, iz + dz, salt) >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0;
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    lerp(lerp(corner(0, 0), corner(1, 0), tx), lerp(corner(0, 1), corner(1, 1), tx), tz)
}

/// How far the field is lifted at the spawn point. More than the field's own range (about 1.5), so no
/// contour can pass where the lift is full.
const SPAWN_LIFT: f32 = 3.0;

/// The noise field whose zero contours are the rivers, at the point (x, z).
fn field(seed: i64, x: f32, z: f32) -> f32 {
    let base = value_noise(seed, 0x51E1, x, z, RIVER_BIG_SCALE) + RIVER_WIGGLE * value_noise(seed, 0x51E2, x, z, RIVER_SMALL_SCALE);
    let spawn = CHUNK_SIZE as f32 / 2.0;
    let dist = ((x - spawn).powi(2) + (z - spawn).powi(2)).sqrt();
    // 1 within the clearance of the spawn point, easing to 0 at the fade distance.
    let t = ((RIVER_SPAWN_FADE - dist) / (RIVER_SPAWN_FADE - RIVER_SPAWN_CLEARANCE)).clamp(0.0, 1.0);
    base + SPAWN_LIFT * t * t * (3.0 - 2.0 * t)
}

/// Half the width of the river at (x, z): it swells and narrows slowly along its length.
fn half_width(seed: i64, x: f32, z: f32) -> f32 {
    let t = value_noise(seed, 0x51E3, x, z, RIVER_WIDTH_SCALE) * 0.5 + 0.5;
    RIVER_MIN_HALF_WIDTH + (RIVER_MAX_HALF_WIDTH - RIVER_MIN_HALF_WIDTH) * t
}

/// Whether the column (wx, wz) is part of a river: its bed lies `RIVER_DEPTH` below the ground around it
/// and water fills it.
pub fn is_river(seed: i64, wx: i32, wz: i32) -> bool {
    let (x, z) = (wx as f32 + 0.5, wz as f32 + 0.5);
    let f = field(seed, x, z);
    let gx = (field(seed, x + 1.0, z) - field(seed, x - 1.0, z)) / 2.0;
    let gz = (field(seed, x, z + 1.0) - field(seed, x, z - 1.0)) / 2.0;
    let slope = (gx * gx + gz * gz).sqrt();
    slope > 1e-6 && f.abs() / slope < half_width(seed, x, z)
}

/// Whether any river column lies within `radius` blocks of the column (wx, wz).
pub fn river_near(seed: i64, wx: i32, wz: i32, radius: i32) -> bool {
    (wz - radius..=wz + radius).any(|z| {
        (wx - radius..=wx + radius).any(|x| (x - wx).pow(2) + (z - wz).pow(2) <= radius * radius && is_river(seed, x, z))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Number of river columns in the square of `side` columns whose corner is (x0, z0).
    fn count(seed: i64, x0: i32, z0: i32, side: i32) -> i32 {
        (z0..z0 + side).flat_map(|z| (x0..x0 + side).map(move |x| (x, z))).filter(|&(x, z)| is_river(seed, x, z)).count() as i32
    }

    #[test]
    fn rivers_are_deterministic_and_differ_between_seeds() {
        for (x, z) in [(100, 40), (-250, 310), (7, -90)] {
            assert_eq!(is_river(5, x, z), is_river(5, x, z));
        }
        let map = |seed| (0..60).flat_map(|z| (0..60).map(move |x| is_river(seed, x * 10 - 300, z * 10 - 300))).collect::<Vec<_>>();
        assert_ne!(map(1), map(2));
    }

    #[test]
    fn rivers_cover_a_modest_share_of_the_land() {
        for seed in 0..4 {
            let share = count(seed, -400, -400, 800) as f64 / (800.0 * 800.0);
            assert!((0.01..0.12).contains(&share), "seed {seed}: {share:.3} of the columns are river");
        }
    }

    #[test]
    fn the_spawn_area_is_dry_for_every_seed() {
        for seed in 0..300 {
            for (x, z) in [(10, 10), (0, 0), (19, 19), (-20, 10), (10, -20), (40, 10), (10, 40), (-24, -24), (44, 44)] {
                let d = ((x - 10) as f32).hypot((z - 10) as f32);
                if d < RIVER_SPAWN_CLEARANCE - 1.0 {
                    assert!(!is_river(seed, x, z), "seed {seed}: river at {x},{z}");
                }
            }
        }
    }

    #[test]
    fn rivers_are_continuous_bands_not_specks() {
        for seed in 0..3 {
            let (mut wet, mut lonely) = (0, 0);
            for z in -200..200 {
                for x in -200..200 {
                    if !is_river(seed, x, z) {
                        continue;
                    }
                    wet += 1;
                    let neighbours = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|&&(dx, dz)| is_river(seed, x + dx, z + dz)).count();
                    lonely += (neighbours == 0) as i32;
                }
            }
            assert!(wet > 500, "seed {seed}: some river in 400x400 ({wet} columns)");
            assert!(lonely * 100 < wet, "seed {seed}: {lonely} of {wet} river columns stand alone");
        }
    }

    #[test]
    fn rivers_keep_an_even_width() {
        // Through every river column, take the shorter of the east-west and north-south runs of water: it is
        // the river's width, seen a little aslant. Nearly all of them lie between the narrowest and widest
        // river (a few at the meeting of two rivers are wider).
        let (mut fine, mut total) = (0, 0);
        for seed in 0..2 {
            for z in -250..250 {
                for x in -250..250 {
                    if !is_river(seed, x, z) {
                        continue;
                    }
                    let run = |dx: i32, dz: i32| {
                        let along = |s: i32| (1..).take_while(|&k| is_river(seed, x + s * dx * k, z + s * dz * k)).count() as i32;
                        1 + along(1) + along(-1)
                    };
                    let width = run(1, 0).min(run(0, 1));
                    total += 1;
                    fine += ((2.0 * RIVER_MIN_HALF_WIDTH) as i32..=(2.0 * RIVER_MAX_HALF_WIDTH * 1.7) as i32).contains(&width) as i32;
                }
            }
        }
        assert!(total > 5000, "enough river to measure ({total})");
        assert!(fine as f64 > 0.97 * total as f64, "{fine} of {total} river columns have a width in range");
    }

    /// The field's slope at (x, z), by central differences.
    fn slope(seed: i64, x: f32, z: f32) -> (f32, f32) {
        ((field(seed, x + 1.0, z) - field(seed, x - 1.0, z)) / 2.0, (field(seed, x, z + 1.0) - field(seed, x, z - 1.0)) / 2.0)
    }

    /// Walk `length` blocks along the river's centre line starting at (x, z) and return how many radians the
    /// course turned in total.
    fn turning_along_river(seed: i64, (mut x, mut z): (f32, f32), length: f32) -> f32 {
        const STEP: f32 = 2.0;
        let direction = |x: f32, z: f32| {
            let (gx, gz) = slope(seed, x, z);
            let n = (gx * gx + gz * gz).sqrt().max(1e-9);
            (-gz / n, gx / n)
        };
        let mut dir = direction(x, z);
        let mut turned = 0.0;
        for _ in 0..(length / STEP) as i32 {
            (x, z) = (x + dir.0 * STEP, z + dir.1 * STEP);
            // Settle back onto the centre line, where the field is zero.
            for _ in 0..3 {
                let (gx, gz) = slope(seed, x, z);
                let g2 = (gx * gx + gz * gz).max(1e-12);
                let f = field(seed, x, z);
                (x, z) = (x - f * gx / g2, z - f * gz / g2);
            }
            let mut next = direction(x, z);
            if next.0 * dir.0 + next.1 * dir.1 < 0.0 {
                next = (-next.0, -next.1);
            }
            turned += (dir.0 * next.1 - dir.1 * next.0).asin().abs();
            dir = next;
        }
        turned
    }

    #[test]
    fn rivers_wind_instead_of_running_straight() {
        let mut followed = 0;
        for seed in 0..10 {
            let Some(start) = (-300..300).find(|&x| is_river(seed, x, 150)) else { continue };
            followed += 1;
            let turned = turning_along_river(seed, (start as f32 + 0.5, 150.5), 800.0);
            assert!(turned > std::f32::consts::PI, "seed {seed}: the river turns only {turned:.1} radians in 800 blocks");
        }
        assert!(followed >= 6, "rivers found for {followed} of 10 seeds");
    }

    #[test]
    fn river_near_finds_a_river_within_the_radius_only() {
        let seed = 3;
        let (x, z) = (0..400).flat_map(|z| (0..400).map(move |x| (x, z))).find(|&(x, z)| is_river(seed, x, z)).expect("a river");
        assert!(river_near(seed, x, z, 0));
        assert!(river_near(seed, x + 3, z, 3));
        // Somewhere with no river within the radius.
        let dry = (0..400).flat_map(|z| (0..400).map(move |x| (x, z))).find(|&(x, z)| !river_near(seed, x, z, 6)).expect("dry land");
        assert!(!is_river(seed, dry.0, dry.1));
    }
}
