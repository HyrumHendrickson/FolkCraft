//! Arrows shot from a bow: flight under gravity, and what they hit.

use macroquad::prelude::*;

use crate::animal::Animal;
use crate::config::*;
use crate::world::{ray_box, World};

/// Longest distance (blocks) an arrow travels between two collision checks.
const MAX_STEP: f32 = 0.25;

#[derive(Clone, Debug)]
pub struct Arrow {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Seconds in flight.
    pub age: f32,
}

/// What became of an arrow during one update.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fate {
    InFlight,
    /// Hit the animal at this index of the slice passed to `update`.
    HitAnimal(usize),
    /// Struck the ground, a tree, a wall or the edge of the loaded world.
    HitBlock,
    /// Flew for `ARROW_LIFETIME` without hitting anything.
    Expired,
}

impl Arrow {
    /// An arrow leaving `origin` (the eye) along the unit vector `dir`.
    pub fn fired(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { pos: origin, vel: dir * ARROW_SPEED, age: 0.0 }
    }

    /// Advance the arrow by `dt` seconds. The path is checked in short segments, so a fast arrow
    /// cannot skip over an animal or a thin trunk.
    pub fn update(&mut self, world: &World, animals: &[Animal], dt: f32) -> Fate {
        self.age += dt;
        let steps = ((self.vel.length() * dt / MAX_STEP).ceil() as i32).max(1);
        let h = dt / steps as f32;
        for _ in 0..steps {
            self.vel.y -= ARROW_GRAVITY * h;
            let travel = self.vel * h;
            let length = travel.length();
            if length > 1e-6 {
                let dir = travel / length;
                let nearest = animals
                    .iter()
                    .enumerate()
                    .filter_map(|(i, a)| {
                        let (lo, hi) = a.hitbox();
                        ray_box(self.pos, dir, lo.to_array(), hi.to_array()).filter(|&t| t <= length).map(|t| (i, t))
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                if let Some((i, _)) = nearest {
                    return Fate::HitAnimal(i);
                }
            }
            self.pos += travel;
            let eps = Vec3::splat(0.02);
            if world.box_blocked(self.pos - eps, self.pos + eps) {
                return Fate::HitBlock;
            }
        }
        if self.age >= ARROW_LIFETIME {
            return Fate::Expired;
        }
        Fate::InFlight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animal::AnimalKind;
    use crate::block::Block;
    use crate::chunk::idx;
    use crate::rng::Rng;
    use std::collections::HashSet;

    const DT: f32 = 1.0 / 60.0;

    fn open_field() -> World {
        let mut w = World::new(1, HashSet::new());
        w.stream_chunks(vec3(10.0, 6.0, 10.0));
        for chunk in w.chunks.values_mut() {
            for y in GROUND_TOP + 1..WORLD_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        chunk.blocks[idx(x, y, z)] = Block::Air;
                    }
                }
            }
        }
        w
    }

    fn deer_at(x: f32, z: f32) -> Animal {
        Animal::new(AnimalKind::Deer, vec3(x, (GROUND_TOP + 1) as f32, z), &mut Rng::new(1))
    }

    fn fly(arrow: &mut Arrow, w: &World, animals: &[Animal]) -> Fate {
        for _ in 0..600 {
            match arrow.update(w, animals, DT) {
                Fate::InFlight => {}
                other => return other,
            }
        }
        Fate::InFlight
    }

    #[test]
    fn an_arrow_aimed_at_an_animal_hits_it() {
        let w = open_field();
        let animals = [deer_at(10.5, 10.5), deer_at(18.5, 10.5)];
        let mut arrow = Arrow::fired(vec3(2.5, 7.0, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(fly(&mut arrow, &w, &animals), Fate::HitAnimal(0), "the nearer deer is hit first");
        assert!(arrow.pos.x < 11.0);
    }

    #[test]
    fn an_arrow_sags_under_gravity_and_can_fall_short_or_pass_beside() {
        let w = open_field();
        let mut arrow = Arrow::fired(vec3(2.5, 7.0, 10.5), vec3(1.0, 0.0, 0.0));
        let before = arrow.pos.y;
        for _ in 0..30 {
            arrow.update(&w, &[], DT);
        }
        assert!(arrow.pos.y < before - 0.5, "dropped from {before} to {}", arrow.pos.y);
        // Aimed well to the side of the deer: no hit, it lands on the ground.
        let animals = [deer_at(10.5, 14.5)];
        let mut arrow = Arrow::fired(vec3(2.5, 7.0, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(fly(&mut arrow, &w, &animals), Fate::HitBlock);
    }

    #[test]
    fn an_arrow_stops_at_walls_and_at_the_end_of_the_world() {
        let mut w = open_field();
        w.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(12, GROUND_TOP + 2, 10)] = Block::Dirt;
        let animals = [deer_at(16.5, 10.5)];
        let mut arrow = Arrow::fired(vec3(2.5, 7.5, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(fly(&mut arrow, &w, &animals), Fate::HitBlock, "the wall is in the way of the deer");
        assert!(arrow.pos.x < 13.0);
    }

    #[test]
    fn an_arrow_that_hits_nothing_eventually_expires() {
        let w = open_field();
        // Straight up: still in the air when its time is up.
        let mut arrow = Arrow::fired(vec3(10.5, 7.0, 10.5), vec3(0.0, 1.0, 0.0));
        assert_eq!(fly(&mut arrow, &w, &[]), Fate::Expired);
        assert!(arrow.age >= ARROW_LIFETIME);
    }

    #[test]
    fn a_fast_arrow_cannot_skip_a_thin_target() {
        let w = open_field();
        // A quail is only 0.36 wide; in one 0.1 s update the arrow covers over three blocks.
        let quail = Animal::new(AnimalKind::Quail, vec3(10.5, (GROUND_TOP + 1) as f32, 10.5), &mut Rng::new(1));
        let mut over = Arrow::fired(vec3(8.0, 6.7, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(over.update(&w, &[quail.clone()], 0.1), Fate::InFlight, "passes above its head");
        let mut through = Arrow::fired(vec3(8.0, 6.3, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(through.update(&w, &[quail], 0.1), Fate::HitAnimal(0));
    }
}
