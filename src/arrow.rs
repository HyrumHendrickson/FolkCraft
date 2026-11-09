//! Arrows shot from a bow and pebbles slung from a sling: flight under gravity, and what they hit.

use macroquad::prelude::*;

use crate::animal::Animal;
use crate::config::*;
use crate::item::ItemKind;
use crate::world::{ray_box, World};

/// Longest distance (blocks) an arrow travels between two collision checks.
const MAX_STEP: f32 = 0.25;

#[derive(Clone, Debug)]
pub struct Arrow {
    /// What it is: `ItemKind::Arrow` from a bow or `ItemKind::Pebble` from a sling. It lands as this item.
    pub kind: ItemKind,
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
        Arrow { kind: ItemKind::Arrow, pos: origin, vel: dir * ARROW_SPEED, age: 0.0 }
    }

    /// A pebble leaving `origin` (the eye) along the unit vector `dir`.
    pub fn slung(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::Pebble, pos: origin, vel: dir * SLING_SPEED, age: 0.0 }
    }

    /// A crystal (`kind`: a ruby, sapphire or amethyst) leaving `origin` along the unit vector `dir` from a sling. It lands as that crystal.
    pub fn slung_crystal(origin: Vec3, dir: Vec3, kind: ItemKind) -> Arrow {
        Arrow { kind, pos: origin, vel: dir * SLING_SPEED, age: 0.0 }
    }

    /// A Phoenix Arrow shot from a bow: it flies like an arrow, but is never used up and never lands.
    pub fn fired_phoenix(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::PhoenixArrow, pos: origin, vel: dir * ARROW_SPEED, age: 0.0 }
    }

    /// A poison arrow shot from a bow: it flies and lands like an ordinary arrow (used up only by hitting an animal, picked up again from
    /// the ground if it misses), but does `POISON_ARROW_DAMAGE_FACTOR` times the damage.
    pub fn fired_poison(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::PoisonArrow, pos: origin, vel: dir * ARROW_SPEED, age: 0.0 }
    }

    /// An iron feather thrown by the player from `origin` along the unit vector `dir`, like a throwing star.
    pub fn thrown_feather(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::IronFeather, pos: origin, vel: dir * IRON_FEATHER_SPEED, age: 0.0 }
    }

    /// An ivory knife thrown by the player from `origin` along the unit vector `dir`.
    pub fn thrown_knife(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::IvoryKnife, pos: origin, vel: dir * IVORY_KNIFE_SPEED, age: 0.0 }
    }

    /// An iron feather shot by an Iron Quail from `origin` along the unit vector `dir`. It hurts the player, not animals, and lands as an
    /// item if it misses.
    pub fn from_iron_quail(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::IronFeather, pos: origin, vel: dir * IRON_FEATHER_SPEED, age: 0.0 }
    }

    /// An arrow shot by one of the king's spirit archers from `origin` along the unit vector `dir`: it flies like a bow's arrow, hurts
    /// animals, and (like a Phoenix Arrow) is gone where it lands.
    pub fn from_spirit(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::PhoenixArrow, pos: origin, vel: dir * ARROW_SPEED, age: 0.0 }
    }

    /// Whether the arrow is gone for good where it lands (a Phoenix Arrow), instead of lying there to be picked up again.
    pub fn vanishes_on_landing(&self) -> bool {
        self.kind == ItemKind::PhoenixArrow
    }

    /// An arrow shot by the Wild Hunt's archer from `origin` along the unit vector `dir`. It hurts the player, not animals.
    pub fn from_archer(origin: Vec3, dir: Vec3) -> Arrow {
        Arrow { kind: ItemKind::Arrow, pos: origin, vel: dir * HUNT_ARROW_SPEED, age: 0.0 }
    }

    /// Whether the straight path this arrow has just flown, from `from` to where it is now, passed through the player standing at
    /// `player` (feet).
    pub fn hit_player(&self, from: Vec3, player: Vec3) -> bool {
        let path = self.pos - from;
        let length = path.length();
        let (lo, hi) = (player - vec3(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH), player + vec3(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH));
        if length < 1e-6 {
            return (lo.x..=hi.x).contains(&from.x) && (lo.y..=hi.y).contains(&from.y) && (lo.z..=hi.z).contains(&from.z);
        }
        ray_box(from, path / length, lo.to_array(), hi.to_array()).map_or(false, |t| t <= length)
    }

    /// Life points taken from an animal it hits.
    pub fn damage(&self) -> i32 {
        match self.kind {
            ItemKind::Pebble => SLING_DAMAGE,
            kind if kind.is_crystal() => SLING_DAMAGE * CRYSTAL_DAMAGE_FACTOR,
            ItemKind::IronFeather => IRON_FEATHER_THROW_DAMAGE,
            ItemKind::IvoryKnife => IVORY_KNIFE_DAMAGE,
            ItemKind::PoisonArrow => POISON_ARROW_DAMAGE,
            _ => ARROW_DAMAGE,
        }
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
                        if !a.kind.is_huntable() {
                            return None;
                        }
                        // A flying snake can be hit anywhere along its long body.
                        a.hitboxes()
                            .into_iter()
                            .filter_map(|(lo, hi)| ray_box(self.pos, dir, lo.to_array(), hi.to_array()).filter(|&t| t <= length))
                            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|t| (i, t))
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
    fn an_archers_arrow_that_passes_through_the_player_hits_them_and_one_that_passes_beside_them_does_not() {
        let player = vec3(10.0, 6.0, 10.0);
        let mut arrow = Arrow::from_archer(vec3(0.0, 7.0, 10.0), vec3(1.0, 0.0, 0.0));
        let from = arrow.pos;
        arrow.pos = vec3(12.0, 7.0, 10.0);
        assert!(arrow.hit_player(from, player), "through the chest");
        arrow.pos = vec3(9.0, 7.0, 10.0);
        assert!(!arrow.hit_player(from, player), "stopped short");
        let beside = Arrow::from_archer(vec3(0.0, 7.0, 12.0), vec3(1.0, 0.0, 0.0));
        let mut beside_after = beside.clone();
        beside_after.pos = vec3(20.0, 7.0, 12.0);
        assert!(!beside_after.hit_player(beside.pos, player), "two blocks wide of them");
        let mut over = Arrow::from_archer(vec3(0.0, 9.0, 10.0), vec3(1.0, 0.0, 0.0));
        let start = over.pos;
        over.pos = vec3(20.0, 9.0, 10.0);
        assert!(!over.hit_player(start, player), "over their head");
        assert!((arrow.vel.length() - HUNT_ARROW_SPEED).abs() < 1e-3);
    }

    #[test]
    fn a_poison_arrow_flies_and_lands_like_an_arrow_but_hits_twice_as_hard() {
        let w = open_field();
        assert_eq!(Arrow::fired_poison(Vec3::ZERO, vec3(1.0, 0.0, 0.0)).damage(), 2 * Arrow::fired(Vec3::ZERO, vec3(1.0, 0.0, 0.0)).damage());
        assert_eq!(POISON_ARROW_DAMAGE, ARROW_DAMAGE * 2);
        let poison = Arrow::fired_poison(Vec3::ZERO, vec3(1.0, 0.0, 0.0));
        assert!(!poison.vanishes_on_landing(), "a miss lies on the ground to be picked up, as an ordinary arrow does");
        let mut arrow = Arrow::fired_poison(vec3(2.5, 7.0, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(fly(&mut arrow, &w, &[deer_at(10.5, 14.5)]), Fate::HitBlock, "aimed wide, it lands");
        let mut arrow = Arrow::fired_poison(vec3(2.5, 7.0, 10.5), vec3(1.0, 0.0, 0.0));
        assert_eq!(fly(&mut arrow, &w, &[deer_at(10.5, 10.5)]), Fate::HitAnimal(0));
        assert!(Arrow::fired_phoenix(Vec3::ZERO, Vec3::X).vanishes_on_landing(), "the Phoenix Arrow still never lands");
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
