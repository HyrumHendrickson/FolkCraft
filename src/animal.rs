//! Wild animals: deer, rabbits, quail, bears, moose and will-o-whisps. Their behaviour is plain logic over the `World`; drawing lives
//! in `animal_draw.rs`.
//!
//! - **Deer** wander slowly, pausing now and then. A player (or a bear) within `FLEE_DISTANCE` makes them
//!   bolt directly away, faster than a sprinting player, until `FLEE_CALM_DISTANCE` lies between them.
//! - **Quail** hop about on the ground. A player (or a bear) within `FLEE_DISTANCE` makes them take off and
//!   fly away over the trees, losing a feather each time they do.
//! - **Bears** wander like deer, but within `BEAR_NOTICE_DISTANCE` of the player every walk heads roughly
//!   towards them. Within `BEAR_CHARGE_DISTANCE` a bear charges at `BEAR_RUN_SPEED` (a little slower than a
//!   sprinting player) and keeps charging until the player is `BEAR_CALM_DISTANCE` away. A charging bear that
//!   reaches the player hurts them (`Animal::try_attack`). A bear never chases deer or quail.
//!
//!   A player who wears a pelt cloak is ignored by bears (`Animal::pacified`) unless the player has hit that bear
//!   (`Animal::provoked`).
//! - **Rabbits** run from the player (or a bear) exactly like deer, a little slower.
//! - **Moose** do not run. Left alone they wander; once hurt, one chases the player until one of them is dead or
//!   they are `MOOSE_GIVE_UP_DISTANCE` apart, and hits for `MOOSE_DAMAGE` when it reaches them.
//! - **Will-o-whisps** appear only at night, float above the terrain and flee when the player gets close. They have no
//!   attacks or drops.
//! - A hurt animal, and a hurt player, are knocked back (`Animal::knock_back`, `Player::knock_back`).
//!
//! Animals hop up one-block steps, so hills and logs do not trap them, and turn aside when
//! something taller blocks the way.

use macroquad::prelude::*;

use crate::config::*;
use crate::item::ItemKind;
use crate::rng::Rng;
use crate::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimalKind {
    Deer,
    Quail,
    Bear,
    Rabbit,
    Moose,
    WillOWhisp,
}

impl AnimalKind {
    pub fn max_health(self) -> i32 {
        match self {
            AnimalKind::Deer => DEER_HEALTH,
            AnimalKind::Quail => QUAIL_HEALTH,
            AnimalKind::Bear => BEAR_HEALTH,
            AnimalKind::Rabbit => RABBIT_HEALTH,
            AnimalKind::Moose => MOOSE_HEALTH,
            AnimalKind::WillOWhisp => WILL_O_WHISP_HEALTH,
        }
    }

    /// Half the width of the collision box (square seen from above).
    pub fn half_width(self) -> f32 {
        match self {
            AnimalKind::Deer => 0.45,
            AnimalKind::Quail => 0.18,
            AnimalKind::Bear => 0.55,
            AnimalKind::Rabbit => 0.20,
            AnimalKind::Moose => 0.70,
            AnimalKind::WillOWhisp => WILL_O_WHISP_HALF_WIDTH,
        }
    }

    pub fn height(self) -> f32 {
        match self {
            AnimalKind::Deer => 1.5,
            AnimalKind::Quail => 0.4,
            AnimalKind::Bear => 1.4,
            AnimalKind::Rabbit => 0.5,
            AnimalKind::Moose => 2.1,
            AnimalKind::WillOWhisp => WILL_O_WHISP_HEIGHT,
        }
    }

    fn walk_speed(self) -> f32 {
        match self {
            AnimalKind::Deer => DEER_WALK_SPEED,
            AnimalKind::Quail => QUAIL_WALK_SPEED,
            AnimalKind::Bear => BEAR_WALK_SPEED,
            AnimalKind::Rabbit => RABBIT_WALK_SPEED,
            AnimalKind::Moose => MOOSE_WALK_SPEED,
            AnimalKind::WillOWhisp => 0.0,
        }
    }

    /// How long (seconds) the animal stands still between walks, as (min, max).
    fn idle_time(self) -> (f32, f32) {
        match self {
            AnimalKind::Deer => (2.0, 6.0),
            AnimalKind::Quail => (1.0, 3.0),
            AnimalKind::Bear => (1.5, 4.0),
            AnimalKind::Rabbit => (1.0, 3.5),
            AnimalKind::Moose => (3.0, 8.0),
            AnimalKind::WillOWhisp => (1.0, 1.0),
        }
    }

    /// How long (seconds) one walk lasts, as (min, max).
    fn walk_time(self) -> (f32, f32) {
        match self {
            AnimalKind::Deer => (1.5, 4.0),
            AnimalKind::Quail => (0.8, 2.0),
            AnimalKind::Bear => (2.0, 5.0),
            AnimalKind::Rabbit => (0.8, 2.0),
            AnimalKind::Moose => (2.0, 5.0),
            AnimalKind::WillOWhisp => (1.0, 1.0),
        }
    }

    /// Whether the animal runs or flies from bears (as it does from the player).
    pub fn fears_bears(self) -> bool {
        matches!(self, AnimalKind::Deer | AnimalKind::Quail | AnimalKind::Rabbit)
    }

    /// Speed (blocks per second) of an animal that is running away.
    fn run_speed(self) -> f32 {
        match self {
            AnimalKind::Rabbit => RABBIT_RUN_SPEED,
            AnimalKind::WillOWhisp => WILL_O_WHISP_FLEE_SPEED,
            _ => DEER_RUN_SPEED,
        }
    }

    /// Life taken by one blow of an animal that attacks (bears and moose).
    pub fn attack_damage(self) -> u32 {
        match self {
            AnimalKind::Moose => MOOSE_DAMAGE,
            AnimalKind::Bear => BEAR_DAMAGE,
            _ => 0,
        }
    }

    /// Shown when this animal's blow kills the player.
    pub fn death_message(self) -> &'static str {
        match self {
            AnimalKind::Moose => "A moose killed you",
            AnimalKind::Bear => "A bear killed you",
            _ => "An animal killed you",
        }
    }

    /// Whether weapons can target this kind. Will-o-whisps are scenery and flight behaviour for now.
    pub fn is_huntable(self) -> bool {
        self != AnimalKind::WillOWhisp
    }

    /// What the animal leaves behind when killed.
    pub fn drops(self, rng: &mut Rng) -> Vec<ItemKind> {
        let mut out = Vec::new();
        let mut add = |kind: ItemKind, count: u32| out.extend(std::iter::repeat(kind).take(count as usize));
        match self {
            AnimalKind::Deer => {
                add(ItemKind::Venison, DEER_VENISON_DROP);
                add(ItemKind::Leather, DEER_LEATHER_DROP);
            }
            AnimalKind::Quail => {
                add(ItemKind::Poultry, QUAIL_POULTRY_DROP);
                add(ItemKind::Feather, QUAIL_FEATHER_DROP);
            }
            AnimalKind::Bear => {
                add(ItemKind::BearMeat, BEAR_MEAT_DROP);
                add(ItemKind::BearPelt, BEAR_PELT_DROP);
            }
            AnimalKind::Rabbit => {
                add(ItemKind::RabbitMeat, RABBIT_MEAT_DROP);
                if rng.chance(RABBIT_FOOT_CHANCE) {
                    add(ItemKind::RabbitFoot, 1);
                }
            }
            AnimalKind::Moose => {
                add(ItemKind::MooseMeat, MOOSE_MEAT_DROP);
                add(ItemKind::Leather, MOOSE_LEATHER_DROP);
                add(ItemKind::Antlers, MOOSE_ANTLER_DROP);
            }
            AnimalKind::WillOWhisp => {}
        }
        out
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    /// Standing still.
    Idle,
    /// Walking in a random direction (a bear near the player walks roughly towards them).
    Wander,
    /// A deer running from the player or a bear.
    Flee,
    /// A quail in the air.
    Fly,
    /// A bear running at the player.
    Charge,
}

#[derive(Clone, Debug)]
pub struct Animal {
    pub kind: AnimalKind,
    /// Centre of the feet.
    pub pos: Vec3,
    pub vel: Vec3,
    /// Direction of travel and facing, in radians (`x = cos`, `z = sin`).
    pub heading: f32,
    pub mode: Mode,
    pub health: i32,
    /// Distance walked, in blocks; drives the leg and wing animation.
    pub phase: f32,
    timer: f32,
    /// Seconds left of being scared (a bear: of being enraged) regardless of distance, after being hurt.
    alarm: f32,
    on_ground: bool,
    /// Seconds spent pushing against something.
    stuck: f32,
    /// Turn applied to a fleeing or charging animal's heading to get round an obstacle, and how long it lasts.
    detour: f32,
    detour_left: f32,
    /// Height the quail took off from.
    launch_y: f32,
    /// Seconds until a bear or moose can strike again.
    attack_cooldown: f32,
    /// Horizontal push from a blow, added to the animal's own movement and dying away.
    pub knock: Vec3,
    /// A moose that has been hurt: it chases the player until `MOOSE_GIVE_UP_DISTANCE` lies between them.
    pub enraged: bool,
    /// A bear that has been hit by the player. It stays hostile for good, even towards a player in a pelt cloak.
    pub provoked: bool,
    /// Set every frame for a bear by the game: the player wears a pelt cloak and this bear has not been hit, so it takes
    /// no notice of them (wanders as if they were not there and never strikes).
    pub pacified: bool,
    /// Set every frame for a will-o-whisp: where the player stands (feet) if they have a holly rod in the hotbar,
    /// which draws whisps to them, and `None` otherwise.
    pub lure: Option<Vec3>,
    /// A will-o-whisp that has come within `WILL_O_WHISP_BOND_DISTANCE` of a player with a holly rod. It follows them,
    /// day or night, until the rod leaves the hotbar.
    pub bonded: bool,
    /// Where on its circle round the player a bonded whisp floats (radians), different for each.
    orbit: f32,
}

/// What a body bumped into during one step.
struct Contact {
    on_ground: bool,
    hit_wall: bool,
}

/// Move a box (`half` wide, `height` tall, feet at `pos`) by its velocity with the same
/// axis-separated, sub-stepped collision the player uses.
fn step_body(world: &World, pos: &mut Vec3, vel: &mut Vec3, half: f32, height: f32, dt: f32) -> Contact {
    const STEPS: i32 = 4;
    let eps = 1e-4;
    let blocked = |p: Vec3| {
        world.box_blocked(
            vec3(p.x - half + eps, p.y + eps, p.z - half + eps),
            vec3(p.x + half - eps, p.y + height - eps, p.z + half - eps),
        )
    };
    let h = dt / STEPS as f32;
    let mut contact = Contact { on_ground: false, hit_wall: false };
    for _ in 0..STEPS {
        for axis in 0..3 {
            let mut next = *pos;
            match axis {
                0 => next.x += vel.x * h,
                1 => next.y += vel.y * h,
                _ => next.z += vel.z * h,
            }
            if blocked(next) {
                match axis {
                    0 => {
                        vel.x = 0.0;
                        contact.hit_wall = true;
                    }
                    1 => {
                        if vel.y < 0.0 {
                            contact.on_ground = true;
                        }
                        vel.y = 0.0;
                    }
                    _ => {
                        vel.z = 0.0;
                        contact.hit_wall = true;
                    }
                }
            } else {
                *pos = next;
            }
        }
    }
    contact
}

impl Animal {
    /// A new animal standing at `pos` (feet), facing a random way.
    pub fn new(kind: AnimalKind, pos: Vec3, rng: &mut Rng) -> Self {
        let mut a = Animal {
            kind,
            pos,
            vel: Vec3::ZERO,
            heading: rng.unit() * std::f32::consts::TAU,
            mode: Mode::Idle,
            health: kind.max_health(),
            phase: 0.0,
            timer: 0.0,
            alarm: 0.0,
            on_ground: false,
            stuck: 0.0,
            detour: 0.0,
            detour_left: 0.0,
            launch_y: pos.y,
            attack_cooldown: 0.0,
            knock: Vec3::ZERO,
            enraged: false,
            provoked: false,
            pacified: false,
            lure: None,
            bonded: false,
            orbit: 0.0,
        };
        // Taken from the random heading, so that each whisp has its own place on the circle.
        a.orbit = a.heading * 1.7;
        a.rest(rng);
        a
    }

    /// The box around the animal, used for aiming and arrow hits.
    pub fn hitbox(&self) -> (Vec3, Vec3) {
        let (w, h) = (self.kind.half_width(), self.kind.height());
        (self.pos - vec3(w, 0.0, w), self.pos + vec3(w, h, w))
    }

    /// Hurt the animal for `amount` and scare it (a bear is enraged instead: it charges the player
    /// wherever they are). Returns true when it is dead.
    pub fn damage(&mut self, amount: i32) -> bool {
        if !self.kind.is_huntable() {
            return false;
        }
        self.health -= amount;
        self.alarm = ANIMAL_ALARM_SECONDS;
        if self.kind == AnimalKind::Moose {
            self.enraged = true;
        }
        if self.kind == AnimalKind::Bear {
            self.provoked = true;
            self.pacified = false;
        }
        self.health <= 0
    }

    /// Throw the animal away from whatever hurt it (`away` points from the attacker to the animal), with a hop.
    pub fn knock_back(&mut self, away: Vec3) {
        let flat = vec3(away.x, 0.0, away.z);
        let dir = if flat.length() > 1e-3 { flat.normalize() } else { vec3(self.heading.cos(), 0.0, self.heading.sin()) };
        self.knock = dir * ANIMAL_KNOCKBACK_SPEED;
        if self.kind != AnimalKind::Quail || self.on_ground {
            self.vel.y = KNOCKBACK_HOP;
            self.on_ground = false;
        }
    }

    /// Stop being enraged (the player died): the animal goes back to wandering.
    pub fn calm(&mut self) {
        self.enraged = false;
        self.alarm = 0.0;
        if self.mode == Mode::Charge && self.kind == AnimalKind::Moose {
            self.mode = Mode::Idle;
            self.timer = 1.0;
        }
    }

    fn random_time(range: (f32, f32), rng: &mut Rng) -> f32 {
        range.0 + rng.unit() * (range.1 - range.0)
    }

    /// Stand still for a while.
    fn rest(&mut self, rng: &mut Rng) {
        self.mode = Mode::Idle;
        self.timer = Self::random_time(self.kind.idle_time(), rng);
    }

    /// Whether this animal's body is within striking distance of the player standing at `player` (feet): a
    /// ring of `BEAR_REACH` around the two bodies, and the two heights overlapping.
    fn within_reach_of(&self, player: Vec3) -> bool {
        let flat = vec2(self.pos.x - player.x, self.pos.z - player.z).length();
        let reach = self.kind.half_width() + PLAYER_HALF_WIDTH + BEAR_REACH;
        flat <= reach && player.y < self.pos.y + self.kind.height() && player.y + PLAYER_HEIGHT > self.pos.y
    }

    /// Whether this animal strikes the player standing at `player` (feet) right now. A bear within reach hits once
    /// every `BEAR_ATTACK_COOLDOWN` seconds; a moose only when it is enraged, once every `MOOSE_ATTACK_COOLDOWN`;
    /// any other animal never does.
    pub fn try_attack(&mut self, player: Vec3) -> bool {
        let cooldown = match self.kind {
            AnimalKind::Bear if self.pacified => return false,
            AnimalKind::Bear => BEAR_ATTACK_COOLDOWN,
            AnimalKind::Moose if self.enraged => MOOSE_ATTACK_COOLDOWN,
            _ => return false,
        };
        if self.attack_cooldown > 0.0 || !self.within_reach_of(player) {
            return false;
        }
        self.attack_cooldown = cooldown;
        true
    }

    /// Whether a will-o-whisp is drawn to the player who holds a holly rod in their hotbar: it is once it is bonded,
    /// and before that when the player is within `WILL_O_WHISP_ATTRACT_DISTANCE`.
    fn drawn_to_lure(&self) -> bool {
        match self.lure {
            Some(player) => self.bonded || (self.pos - player).length() <= WILL_O_WHISP_ATTRACT_DISTANCE,
            None => false,
        }
    }

    /// Float a will-o-whisp that is drawn to the player (feet at `player`) on by `dt` seconds. It drifts towards them,
    /// and once within `WILL_O_WHISP_BOND_DISTANCE` it bonds: from then on it circles the player at about
    /// `WILL_O_WHISP_ORBIT_RADIUS`, keeping up however fast they run.
    fn float_to(&mut self, player: Vec3, dt: f32) {
        let centre = player + vec3(0.0, 1.0, 0.0);
        if !self.bonded && (centre - self.pos).length() <= WILL_O_WHISP_BOND_DISTANCE {
            self.bonded = true;
        }
        self.phase += dt * WILL_O_WHISP_BOB_SPEED;
        let before = self.pos;
        if self.bonded {
            let around = self.phase * 0.35 + self.orbit;
            let bob = 0.15 * self.phase.sin();
            let goal = player + vec3(around.cos() * WILL_O_WHISP_ORBIT_RADIUS, WILL_O_WHISP_ORBIT_HEIGHT + bob, around.sin() * WILL_O_WHISP_ORBIT_RADIUS);
            let gap = goal - self.pos;
            let speed = (gap.length() * 5.0).min(WILL_O_WHISP_FOLLOW_SPEED);
            self.pos += gap.normalize_or_zero() * (speed * dt).min(gap.length());
        } else {
            let gap = centre - self.pos;
            self.pos += gap.normalize_or_zero() * (WILL_O_WHISP_LURE_SPEED * dt).min(gap.length());
        }
        let moved = self.pos - before;
        if moved.x.abs() + moved.z.abs() > 1e-4 {
            self.heading = moved.z.atan2(moved.x);
        }
        self.vel = moved / dt.max(1e-4);
        // Should the rod leave the hotbar, the whisp floats on from here.
        self.launch_y = self.pos.y - self.phase.sin() * WILL_O_WHISP_BOB_AMPLITUDE;
        self.on_ground = false;
    }

    /// Advance the animal by `dt` seconds. `threat` is the point (feet) it reacts to: the player for a bear;
    /// for deer and quail the nearer of the player and any bear.
    /// Returns an item the animal dropped on this frame: the feather of a quail that just took off.
    pub fn update(&mut self, world: &World, threat: Vec3, dt: f32, rng: &mut Rng) -> Option<ItemKind> {
        let dist = (self.pos - threat).length();
        let flat_gap = vec2(self.pos.x - threat.x, self.pos.z - threat.z).length();
        self.alarm = (self.alarm - dt).max(0.0);
        self.detour_left = (self.detour_left - dt).max(0.0);
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.timer -= dt;
        let scared = dist < FLEE_DISTANCE || self.alarm > 0.0;
        let away = (self.pos.z - threat.z).atan2(self.pos.x - threat.x);
        let towards = away + std::f32::consts::PI;
        let detour = if self.detour_left > 0.0 { self.detour } else { 0.0 };
        let mut dropped = None;

        match self.kind {
            AnimalKind::Deer | AnimalKind::Rabbit => {
                if scared {
                    self.mode = Mode::Flee;
                } else if self.mode == Mode::Flee && dist >= FLEE_CALM_DISTANCE {
                    self.rest(rng);
                }
                if self.mode == Mode::Flee && flat_gap > 1e-3 {
                    self.heading = away + detour;
                }
            }
            AnimalKind::Quail => {
                if scared && self.mode != Mode::Fly && self.on_ground {
                    self.mode = Mode::Fly;
                    self.timer = Self::random_time((QUAIL_FLIGHT_MIN, QUAIL_FLIGHT_MAX), rng);
                    self.launch_y = self.pos.y;
                    self.heading = away;
                    dropped = Some(ItemKind::Feather);
                }
                let landing = self.timer <= QUAIL_DESCENT_TIME;
                if self.mode == Mode::Fly && (self.timer <= 0.0 || (landing && self.on_ground)) {
                    self.rest(rng);
                }
            }
            AnimalKind::Bear => {
                if self.pacified {
                    // A player in a pelt cloak is of no interest to a bear that has not been hit.
                    if self.mode == Mode::Charge {
                        self.rest(rng);
                    }
                } else if self.mode == Mode::Charge {
                    if dist >= BEAR_CALM_DISTANCE && self.alarm <= 0.0 {
                        self.rest(rng);
                    }
                } else if dist < BEAR_CHARGE_DISTANCE || self.alarm > 0.0 {
                    self.mode = Mode::Charge;
                }
                if self.mode == Mode::Charge && flat_gap > 1e-3 {
                    self.heading = towards + detour;
                }
            }
            AnimalKind::Moose => {
                if self.enraged && dist >= MOOSE_GIVE_UP_DISTANCE {
                    self.enraged = false;
                    self.alarm = 0.0;
                }
                if self.enraged {
                    self.mode = Mode::Charge;
                } else if self.mode == Mode::Charge {
                    self.rest(rng);
                }
                if self.mode == Mode::Charge && flat_gap > 1e-3 {
                    self.heading = towards + detour;
                }
            }
            AnimalKind::WillOWhisp => {
                if self.lure.is_none() {
                    self.bonded = false;
                }
                if self.drawn_to_lure() {
                    // Drawn to the holly rod: no fleeing (see `float_to`).
                    self.mode = Mode::Idle;
                } else if flat_gap < WILL_O_WHISP_FLEE_DISTANCE {
                    self.mode = Mode::Flee;
                    if flat_gap > 1e-3 {
                        self.heading = away;
                    }
                } else if self.mode == Mode::Flee && flat_gap >= WILL_O_WHISP_CALM_DISTANCE {
                    self.mode = Mode::Idle;
                }
            }
        }

        if self.kind != AnimalKind::WillOWhisp && matches!(self.mode, Mode::Idle | Mode::Wander) && self.timer <= 0.0 {
            if self.mode == Mode::Idle {
                self.mode = Mode::Wander;
                self.heading = if self.kind == AnimalKind::Bear && !self.pacified && dist < BEAR_NOTICE_DISTANCE {
                    // A bear that has noticed the player strays from the straight line to them, but only so far.
                    towards + (rng.unit() * 2.0 - 1.0) * BEAR_WANDER_SPREAD
                } else {
                    rng.unit() * std::f32::consts::TAU
                };
                self.timer = Self::random_time(self.kind.walk_time(), rng);
            } else {
                self.rest(rng);
            }
        }

        if self.kind == AnimalKind::WillOWhisp {
            if let (true, Some(player)) = (self.drawn_to_lure(), self.lure) {
                self.float_to(player, dt);
                return dropped;
            }
            let speed = if self.mode == Mode::Flee { WILL_O_WHISP_FLEE_SPEED } else { 0.0 };
            self.vel = vec3(self.heading.cos() * speed, 0.0, self.heading.sin() * speed);
            self.pos.x += self.vel.x * dt;
            self.pos.z += self.vel.z * dt;
            self.phase += dt * WILL_O_WHISP_BOB_SPEED;
            self.pos.y = self.launch_y + self.phase.sin() * WILL_O_WHISP_BOB_AMPLITUDE;
            self.on_ground = false;
            return dropped;
        }

        let speed = match self.mode {
            Mode::Idle => 0.0,
            Mode::Wander => self.kind.walk_speed(),
            Mode::Flee => self.kind.run_speed(),
            Mode::Fly => QUAIL_FLY_SPEED,
            // A charging bear stops once it stands against the player, instead of running through them.
            Mode::Charge if flat_gap <= self.kind.half_width() + PLAYER_HALF_WIDTH => 0.0,
            Mode::Charge if self.kind == AnimalKind::Moose => MOOSE_RUN_SPEED,
            Mode::Charge => BEAR_RUN_SPEED,
        };
        self.vel.x = self.heading.cos() * speed;
        self.vel.z = self.heading.sin() * speed;
        self.vel.x += self.knock.x;
        self.vel.z += self.knock.z;
        self.knock *= (1.0 - KNOCKBACK_DECAY * dt).max(0.0);
        if self.mode == Mode::Fly {
            self.vel.y = if self.timer <= QUAIL_DESCENT_TIME {
                -QUAIL_DESCENT_SPEED
            } else if self.pos.y < self.launch_y + QUAIL_FLY_HEIGHT {
                QUAIL_CLIMB_SPEED
            } else {
                0.0
            };
        } else {
            self.vel.y = (self.vel.y - GRAVITY * dt).max(-MAX_FALL_SPEED);
        }

        let before = self.pos;
        let contact = step_body(world, &mut self.pos, &mut self.vel, self.kind.half_width(), self.kind.height(), dt);
        self.on_ground = contact.on_ground;
        self.phase += (self.pos - before).length();

        if contact.hit_wall && speed > 0.0 {
            // Hop up a one-block step; if that does not help, turn aside after a moment.
            if self.on_ground && self.mode != Mode::Fly {
                self.vel.y = ANIMAL_JUMP_SPEED;
            }
            self.stuck += dt;
            if self.stuck > 0.5 {
                self.stuck = 0.0;
                let sign = if rng.chance(50) { 1.0 } else { -1.0 };
                let turn = sign * (1.0 + rng.unit() * 0.8);
                if matches!(self.mode, Mode::Flee | Mode::Charge) {
                    self.detour = turn;
                    self.detour_left = 1.5;
                } else {
                    self.heading += turn;
                }
            }
        } else {
            self.stuck = (self.stuck - dt).max(0.0);
        }
        dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::idx;
    use crate::block::Block;
    use std::collections::HashSet;

    const DT: f32 = 1.0 / 60.0;

    /// Chunks around the spawn point with everything above the ground cleared away.
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

    fn place(w: &mut World, x: i32, y: i32, z: i32, b: Block) {
        let key = World::chunk_of(x, z);
        w.chunks.get_mut(&key).unwrap().blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))] = b;
    }

    fn at(kind: AnimalKind, x: f32, z: f32) -> Animal {
        let mut a = Animal::new(kind, vec3(x, (GROUND_TOP + 1) as f32, z), &mut Rng::new(3));
        a.on_ground = true;
        a
    }

    fn run(a: &mut Animal, w: &World, player: Vec3, seconds: f32, rng: &mut Rng) -> Vec<ItemKind> {
        let mut dropped = Vec::new();
        for _ in 0..(seconds / DT) as i32 {
            dropped.extend(a.update(w, player, DT, rng));
        }
        dropped
    }

    fn horizontal_speed(a: &Animal) -> f32 {
        vec2(a.vel.x, a.vel.z).length()
    }

    #[test]
    fn a_deer_bolts_directly_away_from_a_player_within_ten_blocks() {
        let w = open_field();
        let mut rng = Rng::new(1);
        let mut deer = at(AnimalKind::Deer, 20.5, 10.5);
        let player = vec3(14.5, 6.0, 10.5); // 6 blocks west
        let start = deer.pos;
        run(&mut deer, &w, player, 0.5, &mut rng);
        assert_eq!(deer.mode, Mode::Flee);
        assert!((horizontal_speed(&deer) - DEER_RUN_SPEED).abs() < 0.01);
        assert!(deer.pos.x > start.x + 3.0, "ran east, away from the player");
        assert!((deer.pos.z - start.z).abs() < 0.2);
        assert!(DEER_RUN_SPEED > WALK_SPEED, "a deer outruns a walking player");
    }

    #[test]
    fn a_deer_does_not_react_to_a_player_beyond_ten_blocks() {
        let w = open_field();
        let mut rng = Rng::new(2);
        for gap in [10.5, 12.0, 25.0] {
            let mut deer = at(AnimalKind::Deer, 10.5 + gap, 10.5);
            let player = vec3(10.5, 6.0, 10.5);
            run(&mut deer, &w, player, 0.2, &mut rng);
            assert_ne!(deer.mode, Mode::Flee, "gap {gap}");
            assert!(horizontal_speed(&deer) <= DEER_WALK_SPEED + 0.01);
        }
        let mut deer = at(AnimalKind::Deer, 10.5 + 9.5, 10.5);
        run(&mut deer, &w, vec3(10.5, 6.0, 10.5), 0.1, &mut rng);
        assert_eq!(deer.mode, Mode::Flee, "9.5 blocks is within ten");
    }

    #[test]
    fn a_fleeing_deer_calms_down_once_it_is_far_enough() {
        let w = open_field();
        let mut rng = Rng::new(4);
        let mut deer = at(AnimalKind::Deer, 20.5, 10.5);
        let player = vec3(14.5, 6.0, 10.5);
        run(&mut deer, &w, player, 3.0, &mut rng);
        let gap = (deer.pos - player).length();
        assert!(gap >= FLEE_CALM_DISTANCE, "gap {gap}");
        assert_ne!(deer.mode, Mode::Flee);
    }

    #[test]
    fn a_deer_left_alone_wanders_randomly_and_pauses() {
        let w = open_field();
        let mut rng = Rng::new(5);
        let mut deer = at(AnimalKind::Deer, 30.5, 30.5);
        let player = vec3(-200.0, 6.0, -200.0);
        let (mut modes, mut headings) = (HashSet::new(), HashSet::new());
        let mut walked = 0.0;
        for _ in 0..(40.0 / DT) as i32 {
            let before = deer.pos;
            deer.update(&w, player, DT, &mut rng);
            modes.insert(deer.mode);
            if deer.mode == Mode::Wander {
                headings.insert((deer.heading * 100.0) as i32);
            }
            walked += (deer.pos - before).length();
        }
        assert!(modes.contains(&Mode::Idle) && modes.contains(&Mode::Wander) && !modes.contains(&Mode::Flee));
        assert!(headings.len() >= 3, "picks new directions: {headings:?}");
        assert!(walked > 3.0, "walked {walked}");
        assert!(walked < 40.0 * DEER_WALK_SPEED, "a stroll, not a run");
    }

    #[test]
    fn a_quail_takes_off_and_drops_a_feather_when_the_player_comes_within_ten_blocks() {
        let w = open_field();
        let mut rng = Rng::new(6);
        let mut quail = at(AnimalKind::Quail, 20.5, 10.5);
        let player = vec3(15.5, 6.0, 10.5);
        let start = quail.pos;
        let dropped = run(&mut quail, &w, player, 0.05, &mut rng);
        assert_eq!(dropped, vec![ItemKind::Feather]);
        assert_eq!(quail.mode, Mode::Fly);
        let dropped = run(&mut quail, &w, player, 1.0, &mut rng);
        assert!(dropped.is_empty(), "one feather per take-off");
        assert!(quail.pos.y > start.y + 3.0, "climbs: {}", quail.pos.y);
        assert!(quail.pos.x > start.x + 4.0, "flies away from the player");
    }

    #[test]
    fn a_quail_far_from_the_player_stays_calm_and_keeps_its_feathers() {
        let w = open_field();
        let mut rng = Rng::new(7);
        let mut quail = at(AnimalKind::Quail, 30.5, 30.5);
        let dropped = run(&mut quail, &w, vec3(-200.0, 6.0, -200.0), 30.0, &mut rng);
        assert!(dropped.is_empty());
        assert_ne!(quail.mode, Mode::Fly);
    }

    #[test]
    fn a_quail_lands_again_and_drops_another_feather_each_time_it_is_startled() {
        let w = open_field();
        let mut rng = Rng::new(8);
        let mut quail = at(AnimalKind::Quail, 20.5, 10.5);
        let mut feathers = 0;
        for round in 0..3 {
            // The player walks up to within five blocks of wherever the quail is.
            let player = quail.pos - vec3(5.0, 0.0, 0.0);
            feathers += run(&mut quail, &w, player, 0.05, &mut rng).len();
            // Then everything is quiet and the quail comes down.
            run(&mut quail, &w, vec3(-200.0, 6.0, -200.0), QUAIL_FLIGHT_MAX + 3.0, &mut rng);
            assert_ne!(quail.mode, Mode::Fly, "round {round}");
            assert!((quail.pos.y - (GROUND_TOP + 1) as f32).abs() < 0.05, "back on the ground, y = {}", quail.pos.y);
        }
        assert_eq!(feathers, 3);
    }

    #[test]
    fn animals_hop_up_a_one_block_step_but_turn_aside_from_a_wall() {
        let mut w = open_field();
        for x in 14..=18 {
            for z in 0..CHUNK_SIZE {
                place(&mut w, x, GROUND_TOP + 1, z, Block::Dirt);
            }
        }
        let mut rng = Rng::new(9);
        let mut deer = at(AnimalKind::Deer, 10.5, 10.5);
        let player = vec3(4.5, 6.0, 10.5);
        let mut highest = deer.pos.y;
        for _ in 0..(1.5 / DT) as i32 {
            deer.update(&w, player, DT, &mut rng);
            highest = highest.max(deer.pos.y);
        }
        assert!(highest >= (GROUND_TOP + 2) as f32 - 1e-3, "got up on the step, y = {highest}");
        assert!(deer.pos.x > 14.0);

        // A wall three blocks high cannot be hopped: the deer works its way sideways.
        let mut w = open_field();
        for z in 0..CHUNK_SIZE {
            for y in GROUND_TOP + 1..GROUND_TOP + 4 {
                place(&mut w, 14, y, z, Block::Dirt);
            }
        }
        let mut deer = at(AnimalKind::Deer, 10.5, 10.5);
        run(&mut deer, &w, player, 5.0, &mut rng);
        assert!(deer.pos.x < 14.0, "did not pass through the wall");
        assert!((deer.pos.z - 10.5).abs() > 1.0, "slid along the wall, z = {}", deer.pos.z);
    }

    #[test]
    fn a_hurt_animal_flees_however_far_the_player_is() {
        let w = open_field();
        let mut rng = Rng::new(10);
        let mut deer = at(AnimalKind::Deer, 40.5, 10.5);
        let far_player = vec3(10.5, 6.0, 10.5);
        assert!(!deer.damage(2), "6 health survives one swing");
        assert_eq!(deer.health, DEER_HEALTH - 2);
        run(&mut deer, &w, far_player, 0.1, &mut rng);
        assert_eq!(deer.mode, Mode::Flee);
        run(&mut deer, &w, far_player, ANIMAL_ALARM_SECONDS + 4.0, &mut rng);
        assert_ne!(deer.mode, Mode::Flee, "calms down after the alarm and the distance");
    }

    #[test]
    fn a_will_o_whisp_floats_and_flees_without_attacking_or_dropping() {
        let w = open_field();
        let mut rng = Rng::new(10);
        let mut whisp = Animal::new(AnimalKind::WillOWhisp, vec3(20.5, 9.0, 10.5), &mut rng);
        let start = whisp.pos;
        let player = vec3(15.5, 6.0, 10.5);
        assert_eq!(whisp.update(&w, player, 0.1, &mut rng), None);
        assert_eq!(whisp.mode, Mode::Flee);
        assert!(whisp.pos.x > start.x, "it flees away from the player");
        assert!((whisp.pos.y - start.y).abs() < WILL_O_WHISP_BOB_AMPLITUDE);
        assert!(!whisp.try_attack(player));
        assert!(!whisp.kind.is_huntable());
        assert!(whisp.kind.drops(&mut rng).is_empty());

        whisp.update(&w, vec3(-200.0, 6.0, -200.0), 0.1, &mut rng);
        assert_eq!(whisp.mode, Mode::Idle);
        assert!(whisp.vel.x.abs() < 1e-5 && whisp.vel.z.abs() < 1e-5);
    }

    #[test]
    fn health_decides_when_an_animal_dies_and_what_it_drops() {
        let mut deer = at(AnimalKind::Deer, 10.5, 10.5);
        let mut quail = at(AnimalKind::Quail, 12.5, 10.5);
        let blows = (DEER_HEALTH + AXE_DAMAGE - 1) / AXE_DAMAGE;
        for _ in 1..blows {
            assert!(!deer.damage(AXE_DAMAGE));
        }
        assert!(deer.damage(AXE_DAMAGE), "a deer dies after {blows} axe blows");
        assert!(quail.damage(AXE_DAMAGE), "a quail dies in one blow");
        let venison = AnimalKind::Deer.drops(&mut Rng::new(1)).iter().filter(|&&k| k == ItemKind::Venison).count();
        let leather = AnimalKind::Deer.drops(&mut Rng::new(1)).iter().filter(|&&k| k == ItemKind::Leather).count();
        assert_eq!((venison as u32, leather as u32), (DEER_VENISON_DROP, DEER_LEATHER_DROP));
        assert_eq!(AnimalKind::Deer.drops(&mut Rng::new(1)).len(), venison + leather);
        let mut quail_loot = vec![ItemKind::Poultry; QUAIL_POULTRY_DROP as usize];
        quail_loot.extend(vec![ItemKind::Feather; QUAIL_FEATHER_DROP as usize]);
        assert_eq!(AnimalKind::Quail.drops(&mut Rng::new(1)), quail_loot, "a quail drops poultry and several feathers");
    }

    #[test]
    fn the_hitbox_surrounds_the_animal_from_its_feet_up() {
        let a = at(AnimalKind::Deer, 10.5, 10.5);
        let (lo, hi) = a.hitbox();
        assert_eq!(lo.y, a.pos.y);
        assert!((hi.y - lo.y - AnimalKind::Deer.height()).abs() < 1e-5);
        assert!(lo.x < a.pos.x && hi.x > a.pos.x && lo.z < a.pos.z && hi.z > a.pos.z);
    }

    // --- Bears ----------------------------------------------------------------------------

    fn angle_between(a: f32, b: f32) -> f32 {
        let d = (a - b).rem_euclid(std::f32::consts::TAU);
        d.min(std::f32::consts::TAU - d)
    }

    #[test]
    fn a_bear_runs_slightly_slower_than_a_sprinting_player_and_faster_than_a_walking_one() {
        assert!(BEAR_RUN_SPEED < SPRINT_SPEED && BEAR_RUN_SPEED > 0.6 * SPRINT_SPEED, "slower than a sprint");
        assert!(SPRINT_SPEED > WALK_SPEED, "sprinting is faster than walking");
        assert!(BEAR_RUN_SPEED > WALK_SPEED && BEAR_WALK_SPEED < WALK_SPEED);
    }

    #[test]
    fn a_bear_within_forty_blocks_wanders_towards_the_player_but_not_straight_at_them() {
        let w = open_field();
        let mut rng = Rng::new(21);
        let player = vec3(10.5, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 40.5, 10.5); // 30 blocks east
        let (mut walks, mut last_mode) = (0, Mode::Idle);
        let mut headings = Vec::new();
        for _ in 0..(40.0 / DT) as i32 {
            bear.update(&w, player, DT, &mut rng);
            if bear.mode == Mode::Wander && last_mode != Mode::Wander {
                walks += 1;
                headings.push(angle_between(bear.heading, std::f32::consts::PI));
            }
            last_mode = bear.mode;
            assert_ne!(bear.mode, Mode::Charge, "still {:.1} away", (bear.pos - player).length());
            if (bear.pos - player).length() < BEAR_CHARGE_DISTANCE {
                break;
            }
        }
        assert!(walks >= 3, "walks {walks}");
        assert!(headings.iter().all(|&h| h <= BEAR_WANDER_SPREAD + 1e-3), "every walk points towards the player: {headings:?}");
        assert!(headings.iter().any(|&h| h > 0.05), "but not exactly: {headings:?}");
        assert!((bear.pos - player).length() < 30.0 - 4.0, "came closer, now {:.1}", (bear.pos - player).length());
        assert!(BEAR_WALK_SPEED * 40.0 >= 1.0);
    }

    #[test]
    fn a_bear_beyond_forty_blocks_wanders_every_which_way() {
        let w = open_field();
        let mut rng = Rng::new(22);
        let player = vec3(-200.0, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 30.5, 10.5);
        let mut away_walks = 0;
        let mut last_mode = Mode::Idle;
        for _ in 0..(120.0 / DT) as i32 {
            bear.update(&w, player, DT, &mut rng);
            if bear.mode == Mode::Wander && last_mode != Mode::Wander && angle_between(bear.heading, std::f32::consts::PI) > std::f32::consts::FRAC_PI_2 {
                away_walks += 1;
            }
            last_mode = bear.mode;
            bear.pos = vec3(30.5, bear.pos.y, 10.5); // keep it in the middle of the field
        }
        assert!(away_walks >= 3, "a far bear also wanders away from the player ({away_walks} walks)");
    }

    #[test]
    fn a_bear_within_ten_blocks_charges_at_its_run_speed() {
        let w = open_field();
        let mut rng = Rng::new(23);
        let player = vec3(10.5, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 19.0, 10.5); // 8.5 blocks
        let start = (bear.pos - player).length();
        run(&mut bear, &w, player, 0.5, &mut rng);
        assert_eq!(bear.mode, Mode::Charge);
        assert!((horizontal_speed(&bear) - BEAR_RUN_SPEED).abs() < 0.01);
        assert!(angle_between(bear.heading, std::f32::consts::PI) < 0.01, "straight at the player");
        assert!(start - (bear.pos - player).length() > 2.5, "closing in");
        // Just beyond ten blocks it does not charge.
        let mut calm = at(AnimalKind::Bear, 21.0, 10.5);
        run(&mut calm, &w, player, 0.2, &mut rng);
        assert_ne!(calm.mode, Mode::Charge);
    }

    #[test]
    fn a_charging_bear_keeps_running_until_the_player_is_twenty_blocks_away_then_wanders_again() {
        let w = open_field();
        let mut rng = Rng::new(24);
        let mut player = vec3(10.5, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 19.5, 10.5);
        let (mut charging_at_15, mut charging_at_19) = (false, false);
        let mut calmed_at = None;
        for step in 0..(60.0 / DT) as i32 {
            player.x -= SPRINT_SPEED * DT; // sprinting directly away
            bear.update(&w, player, DT, &mut rng);
            let gap = (bear.pos - player).length();
            if (15.0..16.0).contains(&gap) {
                charging_at_15 |= bear.mode == Mode::Charge;
            }
            if (19.0..19.9).contains(&gap) {
                charging_at_19 |= bear.mode == Mode::Charge;
            }
            if bear.mode != Mode::Charge && calmed_at.is_none() {
                calmed_at = Some((step as f32 * DT, gap));
            }
        }
        assert!(charging_at_15 && charging_at_19, "keeps charging although it is more than ten blocks away");
        let (_, gap) = calmed_at.expect("it gave up");
        assert!(gap >= BEAR_CALM_DISTANCE - 0.2, "gave up at {gap:.1} blocks");
        // Out of reach for good: the player outruns it and the bear is back to wandering.
        assert!(matches!(bear.mode, Mode::Idle | Mode::Wander));
    }

    #[test]
    fn a_bear_that_has_given_up_wanders_towards_the_player_again() {
        let w = open_field();
        let mut rng = Rng::new(25);
        let player = vec3(10.5, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 35.5, 10.5);
        bear.mode = Mode::Charge; // it had been charging and the player stands 25 blocks away
        run(&mut bear, &w, player, 0.1, &mut rng);
        assert!(matches!(bear.mode, Mode::Idle | Mode::Wander), "calm beyond twenty blocks");
        let before = (bear.pos - player).length();
        run(&mut bear, &w, player, 8.0, &mut rng);
        assert!((bear.pos - player).length() < before, "wandering towards the player again");
    }

    #[test]
    fn a_hurt_bear_charges_from_afar_and_does_not_flee() {
        let w = open_field();
        let mut rng = Rng::new(26);
        let player = vec3(10.5, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 40.5, 10.5);
        assert!(!bear.damage(AXE_DAMAGE));
        run(&mut bear, &w, player, 0.5, &mut rng);
        assert_eq!(bear.mode, Mode::Charge);
        assert!(bear.pos.x < 40.0, "ran at the player");
    }

    #[test]
    fn a_bear_stops_against_the_player_and_strikes_once_per_cooldown() {
        let w = open_field();
        let mut rng = Rng::new(27);
        let player = vec3(10.5, 6.0, 10.5);
        let mut bear = at(AnimalKind::Bear, 12.5, 10.5);
        let mut blows = 0;
        for _ in 0..(5.0 / DT) as i32 {
            bear.update(&w, player, DT, &mut rng);
            blows += bear.try_attack(player) as i32;
        }
        assert!((bear.pos - player).length() > 0.5, "does not run through the player");
        assert_eq!(horizontal_speed(&bear), 0.0, "standing against them");
        let expected = (5.0 / BEAR_ATTACK_COOLDOWN).ceil() as i32;
        assert!((expected - 1..=expected).contains(&blows), "{blows} blows in 5 s");
    }

    #[test]
    fn a_bear_strikes_only_within_reach_and_other_animals_never_do() {
        let mut bear = at(AnimalKind::Bear, 12.5, 10.5);
        assert!(!bear.try_attack(vec3(10.5, 6.0, 10.5)), "2 blocks is out of reach");
        assert!(!bear.try_attack(vec3(12.5, 9.0, 10.5)), "well above the bear");
        assert!(bear.try_attack(vec3(11.5, 6.0, 10.5)), "1 block away");
        assert!(!bear.try_attack(vec3(11.5, 6.0, 10.5)), "the next blow has to wait");
        let (mut deer, mut quail) = (at(AnimalKind::Deer, 11.5, 10.5), at(AnimalKind::Quail, 11.5, 10.5));
        assert!(!deer.try_attack(vec3(11.6, 6.0, 10.5)) && !quail.try_attack(vec3(11.6, 6.0, 10.5)));
    }

    #[test]
    fn bears_are_tough_and_drop_meat_and_a_pelt() {
        let mut bear = at(AnimalKind::Bear, 10.5, 10.5);
        for _ in 0..(BEAR_HEALTH / AXE_DAMAGE - 1) {
            assert!(!bear.damage(AXE_DAMAGE));
        }
        assert!(bear.damage(AXE_DAMAGE));
        assert!(BEAR_HEALTH > 2 * DEER_HEALTH);
        let drops = AnimalKind::Bear.drops(&mut Rng::new(1));
        assert_eq!(drops.iter().filter(|&&k| k == ItemKind::BearMeat).count() as u32, BEAR_MEAT_DROP);
        assert_eq!(drops.iter().filter(|&&k| k == ItemKind::BearPelt).count() as u32, BEAR_PELT_DROP);
        assert_eq!(drops.len() as u32, BEAR_MEAT_DROP + BEAR_PELT_DROP);
    }

    #[test]
    fn deer_and_quail_run_from_a_bear_exactly_as_they_run_from_the_player() {
        let w = open_field();
        let mut rng = Rng::new(28);
        let bear_at = vec3(14.5, 6.0, 10.5);
        let mut deer = at(AnimalKind::Deer, 20.5, 10.5);
        run(&mut deer, &w, bear_at, 0.5, &mut rng);
        assert_eq!(deer.mode, Mode::Flee);
        assert!(deer.pos.x > 23.0, "ran directly away from the bear");
        let mut quail = at(AnimalKind::Quail, 20.5, 10.5);
        assert_eq!(run(&mut quail, &w, bear_at, 0.05, &mut rng), vec![ItemKind::Feather]);
        assert_eq!(quail.mode, Mode::Fly);
        assert!(AnimalKind::Deer.fears_bears() && AnimalKind::Quail.fears_bears() && !AnimalKind::Bear.fears_bears());
    }

    #[test]
    fn a_rabbit_runs_from_the_player_like_a_deer() {
        let w = open_field();
        let mut rng = Rng::new(31);
        let mut rabbit = at(AnimalKind::Rabbit, 20.5, 10.5);
        let player = vec3(14.5, 6.0, 10.5);
        let start = rabbit.pos;
        run(&mut rabbit, &w, player, 0.5, &mut rng);
        assert_eq!(rabbit.mode, Mode::Flee);
        assert!((horizontal_speed(&rabbit) - RABBIT_RUN_SPEED).abs() < 0.01);
        assert!(rabbit.pos.x > start.x + 2.0, "ran east, away from the player");
        assert_eq!(RABBIT_RUN_SPEED, DEER_RUN_SPEED, "it runs like a deer");
        let mut calm = at(AnimalKind::Rabbit, 10.5 + 12.0, 10.5);
        run(&mut calm, &w, vec3(10.5, 6.0, 10.5), 0.2, &mut rng);
        assert_ne!(calm.mode, Mode::Flee, "twelve blocks away is out of its sight");
        assert!(AnimalKind::Rabbit.fears_bears() && AnimalKind::Deer.fears_bears() && !AnimalKind::Moose.fears_bears());
    }

    #[test]
    fn an_unhurt_moose_ignores_the_player_even_up_close() {
        let w = open_field();
        let mut rng = Rng::new(32);
        let mut moose = at(AnimalKind::Moose, 11.5, 10.5);
        let player = vec3(10.5, 6.0, 10.5);
        run(&mut moose, &w, player, 3.0, &mut rng);
        assert!(!moose.enraged && !matches!(moose.mode, Mode::Charge | Mode::Flee));
        assert!(!moose.try_attack(player), "a moose that has not been hurt never strikes");
    }

    #[test]
    fn a_hurt_moose_chases_until_the_player_is_forty_blocks_away_and_then_gives_up() {
        let w = open_field();
        let mut rng = Rng::new(33);
        let mut moose = at(AnimalKind::Moose, 30.5, 10.5);
        let player = vec3(10.5, 6.0, 10.5);
        assert!(!moose.damage(1));
        assert!(moose.enraged);
        let start = moose.pos;
        run(&mut moose, &w, player, 1.0, &mut rng);
        assert_eq!(moose.mode, Mode::Charge);
        assert!((horizontal_speed(&moose) - MOOSE_RUN_SPEED).abs() < 0.01);
        assert!(moose.pos.x < start.x - 3.0, "running at the player");
        assert!(MOOSE_RUN_SPEED < SPRINT_SPEED, "a sprinting player can get away");
        // Long after the scare has worn off it is still after the player.
        run(&mut moose, &w, player, 8.0, &mut rng);
        assert!(moose.enraged && moose.mode == Mode::Charge, "it does not give up while the player is close");
        // The player gets 40 blocks away: it stops.
        let far = vec3(moose.pos.x + MOOSE_GIVE_UP_DISTANCE + 1.0, 6.0, moose.pos.z);
        run(&mut moose, &w, far, 0.1, &mut rng);
        assert!(!moose.enraged && moose.mode != Mode::Charge);
        // ...and it only gets angry again when hurt again.
        let near = vec3(moose.pos.x + 2.0, 6.0, moose.pos.z);
        run(&mut moose, &w, near, 1.0, &mut rng);
        assert!(!moose.enraged);
        moose.damage(1);
        assert!(moose.enraged);
        moose.calm();
        assert!(!moose.enraged && moose.mode != Mode::Charge);
    }

    #[test]
    fn an_enraged_moose_hits_for_two_once_per_cooldown() {
        let w = open_field();
        let mut rng = Rng::new(34);
        let mut moose = at(AnimalKind::Moose, 11.5, 10.5);
        let player = vec3(10.5, 6.0, 10.5);
        moose.damage(1);
        assert_eq!(AnimalKind::Moose.attack_damage(), 2);
        assert!(moose.try_attack(player));
        assert!(!moose.try_attack(player), "the next blow has to wait");
        run(&mut moose, &w, player, MOOSE_ATTACK_COOLDOWN + 0.1, &mut rng);
        assert!(moose.try_attack(player));
        assert!(!at(AnimalKind::Moose, 40.5, 10.5).try_attack(player), "out of reach");
    }

    #[test]
    fn a_hurt_animal_is_thrown_back_with_a_hop_and_the_push_dies_away() {
        let w = open_field();
        let mut rng = Rng::new(35);
        let mut moose = at(AnimalKind::Moose, 20.5, 10.5);
        moose.timer = 100.0;
        moose.knock_back(vec3(5.0, 0.0, 0.0));
        assert!((moose.knock.x - ANIMAL_KNOCKBACK_SPEED).abs() < 1e-4 && moose.knock.z.abs() < 1e-4);
        assert_eq!(moose.vel.y, KNOCKBACK_HOP);
        let start = moose.pos;
        run(&mut moose, &w, vec3(-50.0, 6.0, 10.5), 1.0, &mut rng);
        assert!(moose.pos.x > start.x + 1.5 && moose.pos.x < start.x + 2.6, "pushed about two blocks: {}", moose.pos.x - start.x);
        assert!(moose.knock.length() < 0.2, "the push has died away");
        // With nothing to push away from it is thrown the way it faces.
        let mut other = at(AnimalKind::Deer, 20.5, 10.5);
        other.knock_back(Vec3::ZERO);
        assert!((other.knock.length() - ANIMAL_KNOCKBACK_SPEED).abs() < 1e-4);
    }

    #[test]
    fn a_rabbit_drops_meat_and_a_foot_one_time_in_four_and_a_moose_drops_meat_and_leather() {
        let n = 20_000;
        let mut feet = 0;
        for i in 0..n {
            let d = AnimalKind::Rabbit.drops(&mut Rng::new(i));
            assert_eq!(d[0], ItemKind::RabbitMeat);
            assert!(d.len() == 1 || (d.len() == 2 && d[1] == ItemKind::RabbitFoot), "{d:?}");
            feet += (d.len() == 2) as usize;
        }
        let rate = feet as f64 / n as f64;
        assert!((rate - 0.25).abs() < 0.02 && RABBIT_FOOT_CHANCE == 25, "foot rate {rate}");
        let d = AnimalKind::Moose.drops(&mut Rng::new(1));
        assert_eq!(d.iter().filter(|&&k| k == ItemKind::MooseMeat).count(), MOOSE_MEAT_DROP as usize);
        assert_eq!(d.iter().filter(|&&k| k == ItemKind::Leather).count(), MOOSE_LEATHER_DROP as usize);
        assert_eq!(d.iter().filter(|&&k| k == ItemKind::Antlers).count(), MOOSE_ANTLER_DROP as usize, "a moose leaves antlers");
        assert_eq!(d.len(), (MOOSE_MEAT_DROP + MOOSE_LEATHER_DROP + MOOSE_ANTLER_DROP) as usize);
        assert_eq!((AnimalKind::Rabbit.max_health(), AnimalKind::Moose.max_health()), (RABBIT_HEALTH, MOOSE_HEALTH));
    }
}
