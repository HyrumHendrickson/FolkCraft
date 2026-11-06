//! The Horizen's fire tornado: one moving hazard, replaced by the next spawn.

use macroquad::prelude::*;

use crate::config::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireTornado {
    /// Feet-level centre of the tornado.
    pub pos: Vec3,
    /// The player's position when this tornado spawned.
    pub target: Vec3,
    /// Once reached, the tornado remains at `target` until replaced.
    pub arrived: bool,
    /// Time spent inside the fire since the last damage tick.
    pub damage_timer: f32,
}

impl FireTornado {
    pub fn new(pos: Vec3, target: Vec3) -> Self {
        FireTornado { pos, target, arrived: false, damage_timer: 0.0 }
    }

    pub fn update(&mut self, dt: f32) {
        if self.arrived {
            return;
        }
        let gap = vec2(self.target.x - self.pos.x, self.target.z - self.pos.z);
        let distance = gap.length();
        if distance <= SKY_TORNADO_SPEED * dt || distance <= f32::EPSILON {
            self.pos.x = self.target.x;
            self.pos.z = self.target.z;
            self.arrived = true;
            return;
        }
        let step = gap / distance * SKY_TORNADO_SPEED * dt;
        self.pos.x += step.x;
        self.pos.z += step.y;
    }

    pub fn hits(&self, player: Vec3) -> bool {
        let dx = player.x - self.pos.x;
        let dz = player.z - self.pos.z;
        dx * dx + dz * dz <= (SKY_TORNADO_RADIUS + PLAYER_HALF_WIDTH).powi(2)
            && player.y < self.pos.y + SKY_TORNADO_HEIGHT
            && player.y + PLAYER_HEIGHT > self.pos.y
    }
}

/// A cyclone that carries the player between dimensions: it stands still and does no harm. Standing in it for
/// `CYCLONE_SECONDS` takes the player through (see `Game::update_cyclone`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cyclone {
    /// Feet-level centre of the cyclone.
    pub pos: Vec3,
}

impl Cyclone {
    /// Whether a player standing at `player` (feet) is inside the cyclone.
    pub fn contains(&self, player: Vec3) -> bool {
        let dx = player.x - self.pos.x;
        let dz = player.z - self.pos.z;
        dx * dx + dz * dz <= (CYCLONE_RADIUS + PLAYER_HALF_WIDTH).powi(2)
            && player.y < self.pos.y + CYCLONE_HEIGHT
            && player.y + PLAYER_HEIGHT > self.pos.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tornado_reaches_its_recorded_spawn_position_and_stays_there() {
        let mut tornado = FireTornado::new(vec3(40.0, 6.0, 0.0), vec3(0.0, 6.0, 0.0));
        tornado.update(10.0);
        assert!(tornado.arrived);
        assert!((tornado.pos.x - 0.0).abs() < 1e-5);
        let position = tornado.pos;
        tornado.update(10.0);
        assert_eq!(tornado.pos, position);
    }

    #[test]
    fn a_cyclone_holds_a_player_standing_in_it_and_nobody_beside_it() {
        let c = Cyclone { pos: vec3(10.0, 100.0, 10.0) };
        assert!(c.contains(vec3(10.5, 100.0, 10.0)));
        assert!(!c.contains(vec3(10.0 + CYCLONE_RADIUS + PLAYER_HALF_WIDTH + 0.2, 100.0, 10.0)));
        assert!(!c.contains(vec3(10.0, 100.0 + CYCLONE_HEIGHT + 0.1, 10.0)), "not above it");
    }

    #[test]
    fn tornado_hits_only_inside_its_fire_column() {
        let tornado = FireTornado::new(Vec3::ZERO, Vec3::ZERO);
        assert!(tornado.hits(vec3(0.0, 0.0, 0.0)));
        assert!(!tornado.hits(vec3(SKY_TORNADO_RADIUS + PLAYER_HALF_WIDTH + 0.1, 0.0, 0.0)));
    }
}
