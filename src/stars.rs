//! Falling stars: on the Skylands' nights a star streaks down out of the sky about every `FALLING_STAR_INTERVAL` seconds and lands on
//! an island the player can see (never the one they stand on), where it bursts in a flash of light and leaves a chunk of stardust.
//!
//! What is chosen (`pick_landing`) is a pure function of the world, the player's eye and line of sight, and a random roll; only
//! `effects::draw_falling_stars` touches macroquad.

use macroquad::prelude::*;

use crate::config::*;
use crate::rng::Rng;
use crate::skylands;

/// A star falling, or flashing where it has landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FallingStar {
    /// Where it began high in the sky, and where it lands.
    pub from: Vec3,
    pub to: Vec3,
    /// Seconds since it began to fall.
    pub age: f32,
    /// Whether its chunk of stardust has been set down yet.
    pub pile_made: bool,
}

impl FallingStar {
    pub fn new(from: Vec3, to: Vec3) -> FallingStar {
        FallingStar { from, to, age: 0.0, pile_made: false }
    }

    /// How far along its fall it is, 0 to 1 (it speeds up as it comes down).
    pub fn progress(&self) -> f32 {
        let t = (self.age / FALLING_STAR_FALL_SECONDS).clamp(0.0, 1.0);
        t * t * 0.5 + t * 0.5
    }

    /// Where the star is now (at its landing place once it has landed).
    pub fn pos(&self) -> Vec3 {
        self.from.lerp(self.to, self.progress())
    }

    pub fn landed(&self) -> bool {
        self.age >= FALLING_STAR_FALL_SECONDS
    }

    /// Seconds since it landed (0 while it is still falling).
    pub fn flash_age(&self) -> f32 {
        (self.age - FALLING_STAR_FALL_SECONDS).max(0.0)
    }

    pub fn done(&self) -> bool {
        self.flash_age() >= FALLING_STAR_FLASH_SECONDS
    }

    /// The direction it falls in (unit, pointing down along its path).
    pub fn direction(&self) -> Vec3 {
        (self.to - self.from).normalize_or_zero()
    }
}

/// Where a new star lands: a point on the surface of an island the player at `eye` can see, which is one in front of them (within
/// `FALLING_STAR_VIEW_DEGREES` of the way they look, `look`) and within `range` blocks, but never the island they stand on. Each such
/// island is as likely as another. `None` when there is no such island.
pub fn pick_landing(seed: i64, eye: Vec3, look: Vec3, range: f32, rng: &mut Rng) -> Option<Vec3> {
    let flat_look = vec2(look.x, look.z).normalize_or_zero();
    let cos_limit = FALLING_STAR_VIEW_DEGREES.to_radians().cos();
    let (ex, ez) = (eye.x.floor() as i32, eye.z.floor() as i32);
    let seen: Vec<_> = skylands::islands_near(seed, ex, ez, range as i32)
        .into_iter()
        .filter(|island| {
            let to = vec2(island.x as f32 + 0.5 - eye.x, island.z as f32 + 0.5 - eye.z);
            if island.contains(ex, ez) {
                return false;
            }
            let gap = to.length();
            gap <= range && gap > 1.0 && (flat_look == Vec2::ZERO || to.normalize_or_zero().dot(flat_look) >= cos_limit)
        })
        .collect();
    if seen.is_empty() {
        return None;
    }
    let island = seen[(rng.unit() * seen.len() as f32) as usize % seen.len()];
    // Somewhere on the island, on its ground (a few tries to find a column that is on it).
    for _ in 0..16 {
        let angle = rng.unit() * std::f32::consts::TAU;
        let dist = island.reach(angle) * 0.75 * rng.unit().sqrt();
        let (x, z) = ((island.x as f32 + 0.5 + angle.cos() * dist).floor() as i32, (island.z as f32 + 0.5 + angle.sin() * dist).floor() as i32);
        if island.contains(x, z) {
            return Some(vec3(x as f32 + 0.5, island.surface_y(x, z) as f32 + 1.0, z as f32 + 0.5));
        }
    }
    Some(vec3(island.x as f32 + 0.5, island.surface_y(island.x, island.z) as f32 + 1.0, island.z as f32 + 0.5))
}

/// A star that falls to `landing`: it starts high above it and to one side (`rng` picks which), so it crosses the sky on a slant.
pub fn star_for(landing: Vec3, rng: &mut Rng) -> FallingStar {
    let side = rng.unit() * std::f32::consts::TAU;
    FallingStar::new(landing + vec3(side.cos() * FALLING_STAR_SLANT, FALLING_STAR_START_HEIGHT, side.sin() * FALLING_STAR_SLANT), landing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_star_falls_from_high_up_to_its_landing_place_and_then_flashes_and_is_done() {
        let star = FallingStar::new(vec3(50.0, 200.0, 0.0), vec3(0.0, 60.0, 0.0));
        assert_eq!(star.pos(), star.from);
        let mut s = star;
        s.age = FALLING_STAR_FALL_SECONDS;
        assert!(s.landed() && (s.pos() - s.to).length() < 1e-3 && !s.done());
        s.age = FALLING_STAR_FALL_SECONDS + FALLING_STAR_FLASH_SECONDS + 0.01;
        assert!(s.done());
        let mid = FallingStar { age: FALLING_STAR_FALL_SECONDS / 2.0, ..star };
        assert!(mid.pos().y < star.from.y && mid.pos().y > star.to.y);
    }

    #[test]
    fn stars_land_on_islands_in_view_and_never_on_the_one_the_player_is_on() {
        let seed = 12;
        let eye = vec3(skylands::spawn_point().x, skylands::spawn_point().y + 1.6, skylands::spawn_point().z);
        let mut rng = Rng::new(5);
        let mut landed = 0;
        for i in 0..200 {
            let look = vec3((i as f32 * 0.7).cos(), 0.0, (i as f32 * 0.7).sin());
            let Some(at) = pick_landing(seed, eye, look, FALLING_STAR_RANGE, &mut rng) else { continue };
            landed += 1;
            let island = skylands::island_at(seed, at.x.floor() as i32, at.z.floor() as i32).expect("it lands on an island");
            assert!((at.y - (island.surface_y(at.x.floor() as i32, at.z.floor() as i32) as f32 + 1.0)).abs() < 1e-3, "on the ground");
            assert!(!island.contains(eye.x.floor() as i32, eye.z.floor() as i32), "not the player's own island");
            let to = vec2(island.x as f32 + 0.5 - eye.x, island.z as f32 + 0.5 - eye.z);
            assert!(to.length() <= FALLING_STAR_RANGE + 1.0);
            assert!(to.normalize().dot(vec2(look.x, look.z)) >= FALLING_STAR_VIEW_DEGREES.to_radians().cos() - 1e-3, "in front of the player");
        }
        assert!(landed > 0, "some stars landed");
    }
}
