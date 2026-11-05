//! Player state, input and physics.

use macroquad::prelude::*;

use crate::config::*;
use crate::world::World;

#[derive(Default)]
pub struct MoveInput {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sprint: bool,
}

impl MoveInput {
    pub fn from_keyboard() -> Self {
        MoveInput {
            forward: is_key_down(KeyCode::W),
            back: is_key_down(KeyCode::S),
            left: is_key_down(KeyCode::A),
            right: is_key_down(KeyCode::D),
            jump: is_key_down(KeyCode::Space),
            sprint: is_key_down(KeyCode::LeftShift),
        }
    }
}

pub struct Player {
    pub pos: Vec3, // feet, centre of the bounding box
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    /// Horizontal push from a blow (or a burn), added to the player's own movement and dying away.
    pub knock: Vec3,
}

impl Player {
    pub fn new() -> Self {
        Player {
            pos: vec3(CHUNK_SIZE as f32 / 2.0, (GROUND_TOP + 1) as f32 + 0.01, CHUNK_SIZE as f32 / 2.0),
            vel: Vec3::ZERO,
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            on_ground: false,
            knock: Vec3::ZERO,
        }
    }

    /// Throw the player away from whatever hurt them (`away` points from it to the player; only its horizontal
    /// part counts, and with none the player is thrown backwards), with a little hop.
    pub fn knock_back(&mut self, away: Vec3) {
        let flat = vec3(away.x, 0.0, away.z);
        let dir = if flat.length() > 1e-3 { flat.normalize() } else { -vec3(self.yaw.cos(), 0.0, self.yaw.sin()) };
        self.knock = dir * PLAYER_KNOCKBACK_SPEED;
        self.vel.y = KNOCKBACK_HOP;
        self.on_ground = false;
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + vec3(0.0, EYE_HEIGHT, 0.0)
    }

    pub fn look_dir(&self) -> Vec3 {
        vec3(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
    }

    pub fn collides(world: &World, p: Vec3) -> bool {
        let eps = 1e-4;
        let lo = vec3(p.x - PLAYER_HALF_WIDTH + eps, p.y + eps, p.z - PLAYER_HALF_WIDTH + eps);
        let hi = vec3(p.x + PLAYER_HALF_WIDTH - eps, p.y + PLAYER_HEIGHT - eps, p.z + PLAYER_HALF_WIDTH - eps);
        world.box_blocked(lo, hi)
    }

    /// Lift the player straight up until they are no longer inside a block. A world saved before dirt became
    /// unbreakable can have the player standing in a hole that is now filled in again.
    pub fn lift_out_of_blocks(&mut self, world: &World) {
        while Self::collides(world, self.pos) && self.pos.y < WORLD_HEIGHT as f32 {
            self.pos.y += 1.0;
        }
    }

    /// Move the player out of a block that has appeared where they stand (a door that shut): to the nearest free spot
    /// beside them, or failing that straight up.
    pub fn eject_from_blocks(&mut self, world: &World) {
        if !Self::collides(world, self.pos) {
            return;
        }
        for r in 1..=4 {
            for (dx, dz) in [(r, 0), (-r, 0), (0, r), (0, -r)] {
                let p = self.pos + vec3(dx as f32, 0.0, dz as f32);
                if !Self::collides(world, p) {
                    self.pos = p;
                    return;
                }
            }
        }
        self.lift_out_of_blocks(world);
    }

    pub fn update(&mut self, world: &World, dt: f32, input: &MoveInput) {
        // Horizontal input.
        let forward = vec3(self.yaw.cos(), 0.0, self.yaw.sin());
        let right = vec3(-self.yaw.sin(), 0.0, self.yaw.cos());
        let mut wish = Vec3::ZERO;
        if input.forward { wish += forward; }
        if input.back { wish -= forward; }
        if input.right { wish += right; }
        if input.left { wish -= right; }
        let speed = if input.sprint { SPRINT_SPEED } else { WALK_SPEED };
        let wish = if wish.length_squared() > 0.0 { wish.normalize() * speed } else { Vec3::ZERO };
        self.vel.x = wish.x + self.knock.x;
        self.vel.z = wish.z + self.knock.z;
        self.knock *= (1.0 - KNOCKBACK_DECAY * dt).max(0.0);

        if input.jump && self.on_ground {
            self.vel.y = JUMP_SPEED;
            self.on_ground = false;
        }
        self.vel.y = (self.vel.y - GRAVITY * dt).max(-MAX_FALL_SPEED);

        // Sub-stepped, axis-separated movement to avoid tunnelling.
        const STEPS: i32 = 4;
        let h = dt / STEPS as f32;
        self.on_ground = false;
        for _ in 0..STEPS {
            for axis in 0..3 {
                let mut next = self.pos;
                match axis {
                    0 => next.x += self.vel.x * h,
                    1 => next.y += self.vel.y * h,
                    _ => next.z += self.vel.z * h,
                }
                if Self::collides(world, next) {
                    match axis {
                        0 => self.vel.x = 0.0,
                        1 => {
                            if self.vel.y < 0.0 {
                                self.on_ground = true;
                            }
                            self.vel.y = 0.0;
                        }
                        _ => self.vel.z = 0.0,
                    }
                } else {
                    self.pos = next;
                }
            }
        }
    }
}
