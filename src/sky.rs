//! The sky: a sun that hangs still a little past noon, and slabs of cloud drifting high above the trees.
//!
//! The sun is a fixed direction (`sun_direction`); it is drawn at the same bearing and height whatever the
//! player does, far enough away that walking never moves it. Clouds are cells of a grid (`CLOUD_CELL` blocks
//! across) floating at `CLOUD_HEIGHT`: a cell is cloud where smooth value noise (`cloud_density`) is high, so
//! they gather in banks with gaps between. Neighbouring cloud cells along x are merged into one run
//! (`cloud_runs`) so a cloud bank costs a few boxes, not one per cell. Everything that decides what is where is
//! a pure function of the world seed and the cell; only `draw_sky` touches macroquad.

use macroquad::prelude::*;

use crate::config::*;
use crate::rng::hash3;

/// The unit vector from the eye towards the sun.
pub fn sun_direction() -> Vec3 {
    let (elevation, bearing) = (SUN_ELEVATION_DEGREES.to_radians(), SUN_BEARING_DEGREES.to_radians());
    vec3(elevation.cos() * bearing.cos(), elevation.sin(), elevation.cos() * bearing.sin())
}

pub fn moon_direction() -> Vec3 {
    let (elevation, bearing) = (MOON_ELEVATION_DEGREES.to_radians(), MOON_BEARING_DEGREES.to_radians());
    vec3(elevation.cos() * bearing.cos(), elevation.sin(), elevation.cos() * bearing.sin())
}

/// Quintic ease, so the noise has no creases.
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Smooth value noise in 0..1 at the point (x, z) given in lattice units.
fn value_noise(seed: i64, salt: i32, x: f32, z: f32) -> f32 {
    let (ix, iz) = (x.floor(), z.floor());
    let (tx, tz) = (fade(x - ix), fade(z - iz));
    let (ix, iz) = (ix as i32, iz as i32);
    let corner = |dx: i32, dz: i32| (hash3(seed, ix + dx, iz + dz, salt) >> 40) as f32 / (1u64 << 24) as f32;
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    lerp(lerp(corner(0, 0), corner(1, 0), tx), lerp(corner(0, 1), corner(1, 1), tx), tz)
}

/// How cloudy the cell (cx, cz) is, 0 to 1: a broad swell for the banks and a finer one for ragged edges.
pub fn cloud_density(seed: i64, cx: i32, cz: i32) -> f32 {
    let (x, z) = (cx as f32 + 0.5, cz as f32 + 0.5);
    let broad = value_noise(seed, 0xC10D, x / CLOUD_NOISE_CELLS, z / CLOUD_NOISE_CELLS);
    let fine = value_noise(seed, 0xC10E, x / (CLOUD_NOISE_CELLS * 0.4), z / (CLOUD_NOISE_CELLS * 0.4));
    broad * 0.7 + fine * 0.3
}

/// Whether the cell (cx, cz) holds cloud.
pub fn is_cloud(seed: i64, cx: i32, cz: i32) -> bool {
    cloud_density(seed, cx, cz) > CLOUD_THRESHOLD
}

/// A row of neighbouring cloud cells along x: cells `x0..=x1` of row `z`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CloudRun {
    pub x0: i32,
    pub x1: i32,
    pub z: i32,
}

/// The cloud runs inside the square of cells within `radius` of (`cx`, `cz`).
pub fn cloud_runs(seed: i64, cx: i32, cz: i32, radius: i32) -> Vec<CloudRun> {
    let mut runs = Vec::new();
    for z in cz - radius..=cz + radius {
        let mut start: Option<i32> = None;
        for x in cx - radius..=cx + radius + 1 {
            let cloud = x <= cx + radius && is_cloud(seed, x, z);
            match (cloud, start) {
                (true, None) => start = Some(x),
                (false, Some(x0)) => {
                    runs.push(CloudRun { x0, x1: x - 1, z });
                    start = None;
                }
                _ => {}
            }
        }
    }
    runs
}

/// How far the clouds have drifted along x after `time` seconds.
pub fn cloud_offset(time: f32) -> f32 {
    (time * CLOUD_DRIFT_SPEED).rem_euclid(CLOUD_CELL * 4096.0)
}

/// Draw the sun and the clouds. Call right after the 3D camera is set and before the terrain, so the land hides
/// whatever it stands in front of.
pub fn draw_sky(seed: i64, eye: Vec3, time: f32, night: bool) {
    if night {
        draw_stars(seed, eye, time);
        draw_moon(eye);
    } else {
        draw_sun(eye);
        draw_clouds(seed, eye, time);
    }
}

/// The Horizen now follows one normal day/night sky instead of a permanent split horizon. Its night stars use a fast
/// rotation so the dimension feels unstable and otherworldly.
pub fn draw_horizen_sky(seed: i64, eye: Vec3, time: f32, night: bool) {
    if night {
        draw_stars_in(seed, eye, time, HORIZEN_STAR_ROTATION_SPEED);
        draw_moon(eye);
    } else {
        draw_sun(eye);
        draw_clouds(seed, eye, time);
    }
}

/// The sun: a bright disc with two faint halos. Drawn as spheres around a point that follows the eye, so it
/// keeps its place in the sky. The smaller sphere goes first: a larger one in front of it still passes the depth test.
fn draw_sun(eye: Vec3) {
    let centre = eye + sun_direction() * SUN_DISTANCE;
    let params = DrawSphereParams { rings: 28, slices: 28, ..Default::default() };
    draw_sphere_ex(centre, SUN_RADIUS, None, Color::new(1.0, 0.97, 0.80, 1.0), params);
    let halo = DrawSphereParams { rings: 20, slices: 20, ..Default::default() };
    draw_sphere_ex(centre, SUN_RADIUS * 1.35, None, Color::new(1.0, 0.95, 0.70, 0.22), halo.clone());
    draw_sphere_ex(centre, SUN_RADIUS * 1.8, None, Color::new(1.0, 0.95, 0.75, 0.10), halo);
}

fn draw_moon(eye: Vec3) {
    let centre = eye + moon_direction() * SUN_DISTANCE;
    let params = DrawSphereParams { rings: 24, slices: 24, ..Default::default() };
    draw_sphere_ex(centre, MOON_RADIUS, None, Color::new(0.86, 0.89, 0.98, 1.0), params);
    // A smaller grey disc gives the moon a little texture without needing an asset.
    draw_sphere_ex(centre + moon_direction() * 0.3, MOON_RADIUS * 0.22, None, Color::new(0.65, 0.68, 0.78, 1.0), DrawSphereParams::default());
}

fn draw_stars(seed: i64, eye: Vec3, time: f32) {
    draw_stars_in(seed, eye, time, 0.0);
}

/// The stars. `rotation_speed` is zero in the ordinary world and deliberately fast in the Horizen.
fn draw_stars_in(seed: i64, eye: Vec3, time: f32, rotation_speed: f32) {
    for i in 0..STAR_COUNT {
        let h = crate::rng::hash3(seed, i as i32, i as i32 * 17, 0x57A4);
        let bearing = (h % 10_000) as f32 / 10_000.0 * std::f32::consts::TAU + time * rotation_speed;
        let elevation = 0.18 + ((h >> 16) % 8_200) as f32 / 10_000.0 * 1.0;
        let dir = vec3(elevation.cos() * bearing.cos(), elevation.sin(), elevation.cos() * bearing.sin());
        let twinkle = 0.75 + 0.25 * (time * 1.7 + i as f32 * 0.73).sin();
        draw_sphere(eye + dir * STAR_DISTANCE, STAR_RADIUS, None, Color::new(twinkle, twinkle, twinkle, 1.0));
    }
}

/// The clouds: each run is a white slab over a thinner grey underside.
fn draw_clouds(seed: i64, eye: Vec3, time: f32) {
    let offset = cloud_offset(time);
    let (cx, cz) = (((eye.x - offset) / CLOUD_CELL).floor() as i32, (eye.z / CLOUD_CELL).floor() as i32);
    let under = CLOUD_THICKNESS * 0.3;
    let top = CLOUD_THICKNESS - under;
    for run in cloud_runs(seed, cx, cz, CLOUD_RADIUS_CELLS) {
        let width = (run.x1 - run.x0 + 1) as f32 * CLOUD_CELL;
        let x = offset + run.x0 as f32 * CLOUD_CELL + width / 2.0;
        let z = (run.z as f32 + 0.5) * CLOUD_CELL;
        draw_cube(vec3(x, CLOUD_HEIGHT + under / 2.0, z), vec3(width, under, CLOUD_CELL), None, Color::new(0.80, 0.83, 0.88, 1.0));
        draw_cube(vec3(x, CLOUD_HEIGHT + under + top / 2.0, z), vec3(width, top, CLOUD_CELL), None, Color::new(0.98, 0.98, 0.99, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_is_a_fixed_unit_direction_high_and_a_little_past_noon() {
        let d = sun_direction();
        assert!((d.length() - 1.0).abs() < 1e-5);
        assert_eq!(d, sun_direction(), "it never moves");
        let elevation = d.y.asin().to_degrees();
        assert!((elevation - SUN_ELEVATION_DEGREES).abs() < 1e-3);
        assert!(elevation > 45.0 && elevation < 75.0, "high in the sky but not overhead: {elevation}");
    }

    #[test]
    fn clouds_are_a_pure_function_of_the_seed_and_the_cell() {
        for (x, z) in [(0, 0), (-7, 12), (100, -250)] {
            assert_eq!(is_cloud(5, x, z), is_cloud(5, x, z));
            let d = cloud_density(5, x, z);
            assert!((0.0..=1.0).contains(&d), "{d}");
        }
        let differ = (-20..20).any(|x| is_cloud(1, x, 3) != is_cloud(2, x, 3));
        assert!(differ, "another world has other clouds");
    }

    #[test]
    fn about_a_third_of_the_sky_is_cloud_in_banks_with_gaps() {
        let (mut cloudy, mut total) = (0, 0);
        for z in -60..60 {
            for x in -60..60 {
                total += 1;
                cloudy += is_cloud(9, x, z) as i32;
            }
        }
        let share = cloudy as f32 / total as f32;
        assert!(share > 0.15 && share < 0.55, "cloud share {share}");
        // Banks, not confetti: a cloudy cell usually has a cloudy neighbour.
        let (mut with_neighbour, mut seen) = (0, 0);
        for z in -60..60 {
            for x in -60..60 {
                if is_cloud(9, x, z) {
                    seen += 1;
                    with_neighbour += (is_cloud(9, x + 1, z) || is_cloud(9, x - 1, z) || is_cloud(9, x, z + 1) || is_cloud(9, x, z - 1)) as i32;
                }
            }
        }
        assert!(with_neighbour as f32 / seen as f32 > 0.8);
    }

    #[test]
    fn runs_cover_exactly_the_cloud_cells_without_overlap() {
        let (seed, r) = (3, 12);
        let runs = cloud_runs(seed, 4, -9, r);
        assert!(!runs.is_empty());
        let mut covered = std::collections::HashSet::new();
        for run in &runs {
            assert!(run.x0 <= run.x1);
            for x in run.x0..=run.x1 {
                assert!(covered.insert((x, run.z)), "cell {x},{} is in two runs", run.z);
                assert!(is_cloud(seed, x, run.z));
            }
        }
        for z in -9 - r..=-9 + r {
            for x in 4 - r..=4 + r {
                assert_eq!(covered.contains(&(x, z)), is_cloud(seed, x, z), "cell {x},{z}");
            }
        }
        // Merging pays: far fewer boxes than cells.
        assert!(runs.len() * 2 < covered.len());
    }

    #[test]
    fn clouds_drift_along_x_and_stand_still_when_the_speed_is_zero() {
        assert_eq!(cloud_offset(0.0), 0.0);
        assert!((cloud_offset(10.0) - 10.0 * CLOUD_DRIFT_SPEED).abs() < 1e-4);
        assert!(CLOUD_DRIFT_SPEED >= 0.0);
        assert!(cloud_offset(1.0e6) < CLOUD_CELL * 4096.0, "the offset stays small so floats stay precise");
    }

    #[test]
    fn the_clouds_float_above_the_tallest_tree() {
        // The tallest tree stands on the tallest hill. (The mountain is taller than the clouds and rises through them.)
        let tallest_tree = (GROUND_TOP + TREE_LINE_RISE + 1 + 10) as f32;
        assert!(CLOUD_HEIGHT > tallest_tree + 10.0);
        assert!(SUN_DISTANCE > CLOUD_CELL * CLOUD_RADIUS_CELLS as f32, "the sun lies behind the clouds");
    }
}
