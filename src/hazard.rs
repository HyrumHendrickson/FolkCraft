//! Things the bosses throw and spill: the Yeti's lumps of ice, the Phoenix's fireballs (`Projectile`, which bursts where it lands), and the
//! acid Nessie leaves in her lake (`acid_line`, `AcidCell`). Pure logic over the `World`; `effects.rs` draws them.

use macroquad::prelude::*;

use crate::config::*;
use crate::world::World;

/// A longest distance (blocks) a projectile travels between two collision checks.
const MAX_STEP: f32 = 0.3;
/// A projectile that has flown this long bursts where it is.
const LIFETIME: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectileKind {
    /// A lump of ice thrown by the Yeti: it arcs under gravity.
    IceBall,
    /// A ball of fire thrown by the Phoenix: it flies straight.
    Fireball,
}

impl ProjectileKind {
    /// Blocks per second.
    pub fn speed(self) -> f32 {
        match self {
            ProjectileKind::IceBall => ICE_BALL_SPEED,
            ProjectileKind::Fireball => FIREBALL_SPEED,
        }
    }

    pub fn gravity(self) -> f32 {
        match self {
            ProjectileKind::IceBall => ICE_BALL_GRAVITY,
            ProjectileKind::Fireball => 0.0,
        }
    }

    /// How far the burst reaches, and what it does to a player in it.
    pub fn burst_radius(self) -> f32 {
        match self {
            ProjectileKind::IceBall => ICE_BURST_RADIUS,
            ProjectileKind::Fireball => FIREBALL_RADIUS,
        }
    }

    pub fn burst_damage(self) -> u32 {
        match self {
            ProjectileKind::IceBall => ICE_BURST_DAMAGE,
            ProjectileKind::Fireball => FIREBALL_DAMAGE,
        }
    }

    /// Shown when its burst kills the player.
    pub fn death_message(self) -> &'static str {
        match self {
            ProjectileKind::IceBall => "The Yeti's ice shattered you",
            ProjectileKind::Fireball => "A fireball burned you to death",
        }
    }
}

/// Where and how a projectile burst: the game hurts the player if they are in it, and shows a flash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    pub kind: ProjectileKind,
    pub pos: Vec3,
}

impl Burst {
    /// Whether a player standing at `player` (feet) is inside the burst: the nearest point of their body to its middle is within its radius.
    pub fn reaches(&self, player: Vec3) -> bool {
        let lo = player - vec3(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
        let hi = player + vec3(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH);
        let nearest = self.pos.clamp(lo, hi);
        (nearest - self.pos).length() <= self.kind.burst_radius()
    }
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub kind: ProjectileKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
}

impl Projectile {
    /// A projectile leaving `origin` along the unit vector `dir` at its kind's speed.
    pub fn launch(kind: ProjectileKind, origin: Vec3, dir: Vec3) -> Projectile {
        Projectile { kind, pos: origin, vel: dir * kind.speed(), age: 0.0 }
    }

    /// The unit direction in which to throw `kind` from `origin` to hit a target at `target` moving at `target_vel`, allowing for gravity
    /// and for the target's movement over the flight (a little: it is a throw, not a guided missile).
    pub fn aim(kind: ProjectileKind, origin: Vec3, target: Vec3, target_vel: Vec3) -> Vec3 {
        let flight = (target - origin).length() / kind.speed();
        let lead = vec3(target_vel.x, 0.0, target_vel.z) * flight * 0.5;
        let drop = vec3(0.0, 0.5 * kind.gravity() * flight * flight, 0.0);
        (target + lead + drop - origin).normalize_or_zero()
    }

    /// Fly on by `dt` seconds. `player` (feet) is hit by passing through its body. Returns the burst when it lands on a block or the player,
    /// or runs out of flight; `None` while it is in the air.
    pub fn update(&mut self, world: &World, player: Vec3, dt: f32) -> Option<Burst> {
        self.age += dt;
        let steps = ((self.vel.length() * dt / MAX_STEP).ceil() as i32).max(1);
        let h = dt / steps as f32;
        let (lo, hi) = (player - vec3(PLAYER_HALF_WIDTH + 0.15, 0.0, PLAYER_HALF_WIDTH + 0.15), player + vec3(PLAYER_HALF_WIDTH + 0.15, PLAYER_HEIGHT + 0.15, PLAYER_HALF_WIDTH + 0.15));
        for _ in 0..steps {
            self.vel.y -= self.kind.gravity() * h;
            self.pos += self.vel * h;
            if self.pos.cmpge(lo).all() && self.pos.cmple(hi).all() {
                return Some(Burst { kind: self.kind, pos: self.pos });
            }
            let eps = Vec3::splat(0.05);
            if world.box_blocked(self.pos - eps, self.pos + eps) {
                // It bursts a little back along its path, in the open air in front of what it hit.
                let back = self.vel.normalize_or_zero() * 0.4;
                return Some(Burst { kind: self.kind, pos: self.pos - back });
            }
        }
        (self.age >= LIFETIME).then_some(Burst { kind: self.kind, pos: self.pos })
    }
}

/// One column of tainted lake water: brown and acidic for `left` more seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcidCell {
    pub x: i32,
    pub z: i32,
    pub left: f32,
}

/// The water columns of a strip `LOCH_NESS_ACID_WIDTH` wide and up to `LOCH_NESS_ACID_LENGTH` long that runs from `from` towards `towards`
/// (flat directions only): the line Nessie taints. Only water is included (a shore or an island breaks the line).
pub fn acid_line(is_water: &dyn Fn(i32, i32) -> bool, from: Vec3, towards: Vec3) -> Vec<(i32, i32)> {
    let flat = vec2(towards.x - from.x, towards.z - from.z);
    if flat.length() < 1e-3 {
        return Vec::new();
    }
    let dir = flat.normalize();
    let side = vec2(-dir.y, dir.x);
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let half = LOCH_NESS_ACID_WIDTH / 2.0;
    let mut t = 0.0;
    while t <= LOCH_NESS_ACID_LENGTH {
        let mut w = -half + 0.5;
        while w <= half - 0.5 + 1e-3 {
            let p = vec2(from.x, from.z) + dir * t + side * w;
            let cell = (p.x.floor() as i32, p.y.floor() as i32);
            if is_water(cell.0, cell.1) && !cells.contains(&cell) {
                cells.push(cell);
            }
            w += 1.0;
        }
        t += 0.5;
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::chunk::idx;
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

    #[test]
    fn an_ice_ball_arcs_and_bursts_on_the_ground_where_it_lands() {
        let w = open_field();
        let origin = vec3(8.5, 9.0, 8.5);
        let target = vec3(8.5, 6.0, 8.5 + 20.0);
        let dir = Projectile::aim(ProjectileKind::IceBall, origin, target, Vec3::ZERO);
        let mut ball = Projectile::launch(ProjectileKind::IceBall, origin, dir);
        let mut burst = None;
        for _ in 0..600 {
            burst = ball.update(&w, vec3(-100.0, 6.0, -100.0), DT);
            if burst.is_some() {
                break;
            }
        }
        let burst = burst.expect("it came down");
        assert!((burst.pos.z - target.z).abs() < 3.0, "aimed with the arc allowed for: {:?}", burst.pos);
        assert!(burst.pos.y < 8.0, "on the ground, not in the air");
    }

    #[test]
    fn a_burst_reaches_a_player_within_its_radius_and_not_beyond() {
        let burst = Burst { kind: ProjectileKind::IceBall, pos: vec3(10.0, 6.0, 10.0) };
        assert!(burst.reaches(vec3(10.0 + ICE_BURST_RADIUS - 0.3, 6.0, 10.0)));
        assert!(!burst.reaches(vec3(10.0 + ICE_BURST_RADIUS + 1.0, 6.0, 10.0)));
        assert!(!burst.reaches(vec3(10.0, 6.0 + ICE_BURST_RADIUS + 1.0, 10.0)), "not through the sky above");
    }

    #[test]
    fn a_fireball_flies_straight_and_bursts_on_the_player() {
        let w = open_field();
        let player = vec3(8.5, 6.0, 28.5);
        let origin = vec3(8.5, 7.0, 8.5);
        let dir = Projectile::aim(ProjectileKind::Fireball, origin, player + vec3(0.0, 1.0, 0.0), Vec3::ZERO);
        let mut ball = Projectile::launch(ProjectileKind::Fireball, origin, dir);
        let mut burst = None;
        for _ in 0..300 {
            burst = ball.update(&w, player, DT);
            if burst.is_some() {
                break;
            }
        }
        let burst = burst.expect("it hit");
        assert!(burst.reaches(player), "it burst on the player: {:?}", burst.pos);
    }

    #[test]
    fn the_acid_line_is_a_strip_of_water_columns_towards_the_player() {
        // A strip of lake: water in the columns x 8..=12 for z 8..=40, with dry ground around it.
        let water = |x: i32, z: i32| (8..=12).contains(&x) && (8..=40).contains(&z);
        let cells = acid_line(&water, vec3(10.5, 6.0, 9.0), vec3(10.5, 6.0, 30.0));
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|&(x, z)| (9..=11).contains(&x) && z >= 8), "a strip three wide: {cells:?}");
        assert!(cells.iter().any(|&(_, z)| z >= 30), "it runs on to the player and beyond");
        assert!(cells.iter().map(|&(_, z)| z).max().unwrap() <= 40, "and stops where the water does");
        assert!(acid_line(&water, vec3(10.5, 6.0, 9.0), vec3(10.5, 6.0, 9.0)).is_empty());
        assert!(acid_line(&|_, _| false, vec3(10.5, 6.0, 9.0), vec3(10.5, 6.0, 30.0)).is_empty(), "no water, no acid");
    }
}
