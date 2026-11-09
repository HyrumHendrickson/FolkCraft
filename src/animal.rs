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
//! - **Bats** fly: they hover about the player's body and bite. The pipes call them at night in the normal world; the Horizen's cave
//!   has them as wildlife.
//! - **The Wild Hunt** is ghostly: a swordsman that charges, two archers that keep their distance and shoot (`Game` flies their
//!   arrows), two lancers on horses that charge, strike, gallop off, rest and charge again (`HuntPhase`), and ghostly bears and
//!   crows, the first few of which split in two when killed.
//! - **Ravens** circle the player at night, swoop down for one blow and fly away again (`AirPhase`); the Loch Ness Monster calls
//!   some that behave so by day too. **The Phoenix** hovers over its volcano, throwing fireballs and swooping.
//! - **Bosses** feel for a free way ahead (`Animal::steer`) and can climb two-block ledges, so they do not get stuck on trees,
//!   rocks and cliffs. The Yeti smashes the ground and throws ice, and Nessie keeps out of the shallows (`BossEvent`).
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
    /// A harmless little amphibian of the mossy caves: it wanders and never attacks.
    Newt,
    /// The white boss that comes for a player who has carried the Cursed Idol into the open normal world; it will not attack in the jungle.
    Unicorn,
    /// The Great Bear: a huge, dark bear that comes for a player who kills a bear while wearing the pelt cloak. Hitting it calls every bear.
    BearBoss,
    Deer,
    Quail,
    Bear,
    Rabbit,
    Moose,
    WillOWhisp,
    /// The boss on the mountain's summit.
    Yeti,
    /// The boss of a lake with a castle.
    LochNessMonster,
    /// Like a quail, but black, and it always flies west when it can.
    Crow,
    /// The boss that comes for a player who has killed ten quail. It never gives up.
    IronQuail,
    /// A small flying biter: called by the pipes at night in the normal world, and part of the wildlife of the Horizen's cave.
    Bat,
    /// The Wild Hunt, the last boss, is these three ghostly warriors: one charges with a sword...
    HuntSwordsman,
    /// ...one shoots arrows from afar...
    HuntArcher,
    /// ...two ride ghostly horses, charge with a lance, gallop away, rest and charge again...
    HuntLancer,
    /// ...and ghostly bears and crows come with them.
    HuntBear,
    HuntCrow,
    /// A black bird that circles the player at a distance at night, swoops down for one blow and flies away.
    Raven,
    /// The boss in the volcano of the giant Skylands island.
    Phoenix,
    /// The boss that comes out of the dark for every twenty cave bats killed. Its one attack is the swoop.
    VampireBat,
    /// A friendly ghost archer the king of the Horizen's castle sends, for gold, to follow the player and shoot what they hit.
    SpiritArcher,
    /// The boss on the upper tiers of the jungle temple: it throws rocks and smashes the ground.
    Orangutan,
    /// A jungle cat that stalks the player, crouches and pounces.
    Jaguar,
    /// A jungle snake that flies, twisting in spirals round the player and swooping down to bite.
    FlyingSnake,
    /// A colourful quail of the jungle.
    Parrot,
    /// The boss that comes for a player who has killed ten parrots (three at a time). It fights like an Iron Quail.
    BronzeParrot,
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
            AnimalKind::Yeti => YETI_HEALTH,
            AnimalKind::LochNessMonster => LOCH_NESS_HEALTH,
            AnimalKind::Crow => CROW_HEALTH,
            AnimalKind::IronQuail => IRON_QUAIL_HEALTH,
            AnimalKind::Bat => BAT_HEALTH,
            AnimalKind::HuntSwordsman => HUNT_SWORDSMAN_HEALTH,
            AnimalKind::HuntArcher => HUNT_ARCHER_HEALTH,
            AnimalKind::HuntLancer => HUNT_LANCER_HEALTH,
            AnimalKind::HuntBear => HUNT_BEAR_HEALTH,
            AnimalKind::HuntCrow => HUNT_CROW_HEALTH,
            AnimalKind::Raven => RAVEN_HEALTH,
            AnimalKind::Phoenix => PHOENIX_HEALTH,
            AnimalKind::VampireBat => VAMPIRE_BAT_HEALTH,
            AnimalKind::SpiritArcher => 1,
            AnimalKind::Orangutan => ORANGUTAN_HEALTH,
            AnimalKind::Jaguar => JAGUAR_HEALTH,
            AnimalKind::FlyingSnake => FLYING_SNAKE_HEALTH,
            AnimalKind::Parrot => PARROT_HEALTH,
            AnimalKind::BronzeParrot => BRONZE_PARROT_HEALTH,
            AnimalKind::Newt => NEWT_HEALTH,
            AnimalKind::Unicorn => UNICORN_HEALTH,
            AnimalKind::BearBoss => BEAR_BOSS_HEALTH,
        }
    }

    /// Whether this is one of the two metal birds (the Iron Quail and the Bronze Parrot) that charge, kite and shoot feathers.
    pub fn is_metal_bird(self) -> bool {
        matches!(self, AnimalKind::IronQuail | AnimalKind::BronzeParrot)
    }

    /// Whether this is one of the creatures of the Wild Hunt: the warriors, and the ghostly bears and crows.
    pub fn is_wild_hunt(self) -> bool {
        matches!(self, AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer | AnimalKind::HuntBear | AnimalKind::HuntCrow)
    }

    /// Whether this creature flies (no gravity, and it is drawn with its wings beating).
    pub fn flies_always(self) -> bool {
        matches!(self, AnimalKind::Bat | AnimalKind::HuntCrow | AnimalKind::Raven | AnimalKind::Phoenix | AnimalKind::VampireBat | AnimalKind::SpiritArcher | AnimalKind::FlyingSnake)
    }

    /// Whether this is a bat-like flier: it hovers about the player's body, weaving, and bites.
    fn is_bat_like(self) -> bool {
        matches!(self, AnimalKind::Bat | AnimalKind::HuntCrow)
    }

    /// Whether the game keeps this creature alive and moving wherever the player goes: the Iron Quail and the Wild Hunt hunt
    /// for ever (until the player dies or leaves the world).
    pub fn never_gives_up(self) -> bool {
        matches!(self, AnimalKind::IronQuail | AnimalKind::BronzeParrot | AnimalKind::VampireBat | AnimalKind::SpiritArcher | AnimalKind::Unicorn | AnimalKind::BearBoss) || self.is_wild_hunt()
    }

    /// Whether this is one of the two bosses (they show a health bar, are never spawned as wildlife and are kept
    /// within a leash of where they appeared).
    pub fn is_boss(self) -> bool {
        matches!(self, AnimalKind::Yeti | AnimalKind::LochNessMonster | AnimalKind::IronQuail | AnimalKind::BronzeParrot | AnimalKind::Phoenix | AnimalKind::VampireBat | AnimalKind::Orangutan | AnimalKind::Unicorn | AnimalKind::BearBoss) || self.is_wild_hunt()
    }

    /// Whether this boss stays near where it appeared and is given up on when far away (the others hunt for ever).
    pub fn is_leashed(self) -> bool {
        self.is_boss() && !self.never_gives_up()
    }

    /// The name shown over a boss's health bar.
    pub fn boss_name(self) -> &'static str {
        match self {
            AnimalKind::Yeti => "The Yeti",
            AnimalKind::LochNessMonster => "The Loch Ness Monster",
            AnimalKind::IronQuail => "The Iron Quail",
            AnimalKind::BronzeParrot => "The Bronze Parrot",
            AnimalKind::Phoenix => "The Phoenix",
            AnimalKind::VampireBat => "The Vampire Bat",
            AnimalKind::Orangutan => "The Orangutan",
            AnimalKind::Unicorn => "The Unicorn",
            AnimalKind::BearBoss => "The Great Bear",
            kind if kind.is_wild_hunt() => "The Wild Hunt",
            _ => "",
        }
    }

    /// How far past the two bodies a blow of this animal reaches.
    fn strike_reach(self) -> f32 {
        match self {
            AnimalKind::Yeti => YETI_REACH,
            AnimalKind::LochNessMonster => LOCH_NESS_REACH,
            AnimalKind::IronQuail => IRON_QUAIL_REACH,
            AnimalKind::BronzeParrot => BRONZE_PARROT_REACH,
            AnimalKind::Bat => BAT_REACH,
            AnimalKind::HuntSwordsman => HUNT_SWORD_REACH,
            AnimalKind::HuntLancer => HUNT_LANCER_REACH,
            AnimalKind::HuntCrow => HUNT_CROW_REACH,
            AnimalKind::Raven => RAVEN_REACH,
            AnimalKind::Phoenix => PHOENIX_SWOOP_REACH,
            AnimalKind::VampireBat => VAMPIRE_BAT_REACH,
            AnimalKind::Orangutan => ORANGUTAN_REACH,
            AnimalKind::Jaguar => JAGUAR_REACH,
            AnimalKind::FlyingSnake => SNAKE_REACH,
            AnimalKind::BearBoss => BEAR_BOSS_REACH,
            _ => BEAR_REACH,
        }
    }

    /// Half the width of the collision box (square seen from above).
    pub fn half_width(self) -> f32 {
        match self {
            AnimalKind::Deer => 0.45,
            AnimalKind::Quail | AnimalKind::Parrot => 0.18,
            AnimalKind::Bear => 0.55,
            AnimalKind::Rabbit => 0.20,
            AnimalKind::Moose => 0.70,
            AnimalKind::WillOWhisp => WILL_O_WHISP_HALF_WIDTH,
            AnimalKind::Yeti => 0.9,
            AnimalKind::LochNessMonster => 1.1,
            AnimalKind::Crow => 0.2,
            AnimalKind::IronQuail | AnimalKind::BronzeParrot => 1.0,
            AnimalKind::Bat => BAT_HALF_WIDTH,
            AnimalKind::HuntSwordsman | AnimalKind::HuntArcher => HUNT_HALF_WIDTH,
            AnimalKind::HuntLancer => HUNT_LANCER_HALF_WIDTH,
            AnimalKind::HuntBear => 0.55,
            AnimalKind::HuntCrow => 0.2,
            AnimalKind::Raven => RAVEN_HALF_WIDTH,
            AnimalKind::Phoenix => PHOENIX_HALF_WIDTH,
            AnimalKind::VampireBat => VAMPIRE_BAT_HALF_WIDTH,
            AnimalKind::SpiritArcher => SPIRIT_HALF_WIDTH,
            AnimalKind::Orangutan => ORANGUTAN_HALF_WIDTH,
            AnimalKind::Jaguar => JAGUAR_HALF_WIDTH,
            AnimalKind::FlyingSnake => SNAKE_HALF_WIDTH,
            AnimalKind::Newt => NEWT_HALF_WIDTH,
            AnimalKind::Unicorn => UNICORN_HALF_WIDTH,
            AnimalKind::BearBoss => BEAR_BOSS_HALF_WIDTH,
        }
    }

    pub fn height(self) -> f32 {
        match self {
            AnimalKind::Deer => 1.5,
            AnimalKind::Quail | AnimalKind::Parrot => 0.4,
            AnimalKind::Bear => 1.4,
            AnimalKind::Rabbit => 0.5,
            AnimalKind::Moose => 2.1,
            AnimalKind::WillOWhisp => WILL_O_WHISP_HEIGHT,
            // The summit has four blocks of air above it, so the Yeti is just under three and a half.
            AnimalKind::Yeti => 3.2,
            AnimalKind::LochNessMonster => 3.4,
            AnimalKind::Crow => 0.45,
            AnimalKind::IronQuail | AnimalKind::BronzeParrot => 2.2,
            AnimalKind::Bat => BAT_HEIGHT,
            AnimalKind::HuntSwordsman | AnimalKind::HuntArcher => HUNT_HEIGHT,
            AnimalKind::HuntLancer => HUNT_LANCER_HEIGHT,
            AnimalKind::HuntBear => 1.4,
            AnimalKind::HuntCrow => 0.45,
            AnimalKind::Raven => RAVEN_HEIGHT,
            AnimalKind::Phoenix => PHOENIX_HEIGHT,
            AnimalKind::VampireBat => VAMPIRE_BAT_HEIGHT,
            AnimalKind::SpiritArcher => SPIRIT_HEIGHT,
            AnimalKind::Orangutan => ORANGUTAN_HEIGHT,
            AnimalKind::Jaguar => JAGUAR_HEIGHT,
            AnimalKind::FlyingSnake => SNAKE_HEIGHT,
            AnimalKind::Newt => NEWT_HEIGHT,
            AnimalKind::Unicorn => UNICORN_HEIGHT,
            AnimalKind::BearBoss => BEAR_BOSS_HEIGHT,
        }
    }

    fn walk_speed(self) -> f32 {
        match self {
            AnimalKind::Deer => DEER_WALK_SPEED,
            AnimalKind::Quail | AnimalKind::Parrot => QUAIL_WALK_SPEED,
            AnimalKind::Bear => BEAR_WALK_SPEED,
            AnimalKind::Rabbit => RABBIT_WALK_SPEED,
            AnimalKind::Moose => MOOSE_WALK_SPEED,
            AnimalKind::WillOWhisp => 0.0,
            AnimalKind::Yeti => YETI_WALK_SPEED,
            AnimalKind::LochNessMonster => LOCH_NESS_WALK_SPEED,
            AnimalKind::Crow => CROW_WALK_SPEED,
            AnimalKind::IronQuail => IRON_QUAIL_RUN_SPEED,
            AnimalKind::BronzeParrot => BRONZE_PARROT_RUN_SPEED,
            AnimalKind::Bat => BAT_WANDER_SPEED,
            AnimalKind::HuntSwordsman => HUNT_SWORDSMAN_SPEED,
            AnimalKind::HuntArcher => HUNT_ARCHER_STRAFE_SPEED,
            AnimalKind::HuntLancer => HUNT_LANCER_CHARGE_SPEED,
            AnimalKind::HuntBear => HUNT_BEAR_SPEED,
            AnimalKind::HuntCrow => HUNT_CROW_SPEED,
            AnimalKind::Raven => RAVEN_CIRCLE_SPEED,
            AnimalKind::Phoenix => PHOENIX_CIRCLE_SPEED,
            AnimalKind::VampireBat => VAMPIRE_BAT_CIRCLE_SPEED,
            AnimalKind::SpiritArcher => SPIRIT_SPEED,
            AnimalKind::Orangutan => ORANGUTAN_WALK_SPEED,
            AnimalKind::Jaguar => JAGUAR_STALK_SPEED,
            AnimalKind::FlyingSnake => SNAKE_SPIRAL_SPEED,
            AnimalKind::Newt => NEWT_WALK_SPEED,
            AnimalKind::Unicorn => UNICORN_WALK_SPEED,
            AnimalKind::BearBoss => BEAR_BOSS_WALK_SPEED,
        }
    }

    /// How long (seconds) the animal stands still between walks, as (min, max).
    fn idle_time(self) -> (f32, f32) {
        match self {
            AnimalKind::Deer => (2.0, 6.0),
            AnimalKind::Quail | AnimalKind::Crow | AnimalKind::Parrot => (1.0, 3.0),
            AnimalKind::Bear => (1.5, 4.0),
            AnimalKind::Rabbit => (1.0, 3.5),
            AnimalKind::Moose => (3.0, 8.0),
            AnimalKind::WillOWhisp => (1.0, 1.0),
            AnimalKind::Yeti | AnimalKind::LochNessMonster | AnimalKind::IronQuail | AnimalKind::BronzeParrot | AnimalKind::Orangutan => (2.0, 5.0),
            AnimalKind::Bat => (0.5, 1.5),
            AnimalKind::Newt => (1.5, 4.0),
            AnimalKind::Unicorn | AnimalKind::BearBoss => (2.0, 5.0),
            AnimalKind::Jaguar => (2.0, 5.0),
            AnimalKind::Raven | AnimalKind::Phoenix | AnimalKind::VampireBat | AnimalKind::SpiritArcher | AnimalKind::FlyingSnake => (1.0, 1.0),
            AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer | AnimalKind::HuntBear | AnimalKind::HuntCrow => (1.0, 1.0),
        }
    }

    /// How long (seconds) one walk lasts, as (min, max).
    fn walk_time(self) -> (f32, f32) {
        match self {
            AnimalKind::Deer => (1.5, 4.0),
            AnimalKind::Quail | AnimalKind::Crow | AnimalKind::Parrot => (0.8, 2.0),
            AnimalKind::Bear => (2.0, 5.0),
            AnimalKind::Rabbit => (0.8, 2.0),
            AnimalKind::Moose => (2.0, 5.0),
            AnimalKind::WillOWhisp => (1.0, 1.0),
            AnimalKind::Yeti | AnimalKind::LochNessMonster | AnimalKind::IronQuail | AnimalKind::BronzeParrot | AnimalKind::Orangutan => (3.0, 6.0),
            AnimalKind::Bat => (1.0, 2.5),
            AnimalKind::Newt => (0.8, 2.0),
            AnimalKind::Unicorn | AnimalKind::BearBoss => (2.0, 5.0),
            AnimalKind::Jaguar => (2.0, 4.0),
            AnimalKind::Raven | AnimalKind::Phoenix | AnimalKind::VampireBat | AnimalKind::SpiritArcher | AnimalKind::FlyingSnake => (1.0, 1.0),
            AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer | AnimalKind::HuntBear | AnimalKind::HuntCrow => (1.0, 1.0),
        }
    }

    /// Whether this is ordinary wildlife of the normal world, which does not live in the jungle (only the jungle's own creatures do).
    pub fn is_ordinary_wildlife(self) -> bool {
        matches!(self, AnimalKind::Deer | AnimalKind::Quail | AnimalKind::Bear | AnimalKind::Rabbit | AnimalKind::Moose | AnimalKind::Crow | AnimalKind::WillOWhisp)
    }

    /// Whether the animal runs or flies from bears (as it does from the player).
    pub fn fears_bears(self) -> bool {
        matches!(self, AnimalKind::Deer | AnimalKind::Quail | AnimalKind::Parrot | AnimalKind::Crow | AnimalKind::Rabbit)
    }

    /// Speed (blocks per second) of an animal that is running away.
    fn run_speed(self) -> f32 {
        match self {
            AnimalKind::Rabbit => RABBIT_RUN_SPEED,
            AnimalKind::WillOWhisp => WILL_O_WHISP_FLEE_SPEED,
            AnimalKind::HuntArcher => HUNT_ARCHER_SPEED,
            _ => DEER_RUN_SPEED,
        }
    }

    /// Life taken by one blow of an animal that attacks (bears and moose).
    pub fn attack_damage(self) -> u32 {
        match self {
            AnimalKind::Moose => MOOSE_DAMAGE,
            AnimalKind::Bear => BEAR_DAMAGE,
            AnimalKind::Yeti => YETI_DAMAGE,
            AnimalKind::LochNessMonster => LOCH_NESS_DAMAGE,
            AnimalKind::IronQuail => IRON_QUAIL_DAMAGE,
            AnimalKind::BronzeParrot => BRONZE_PARROT_DAMAGE,
            AnimalKind::Bat => BAT_DAMAGE,
            AnimalKind::HuntSwordsman => HUNT_SWORD_DAMAGE,
            AnimalKind::HuntLancer => HUNT_LANCER_DAMAGE,
            AnimalKind::HuntBear => HUNT_BEAR_DAMAGE,
            AnimalKind::HuntCrow => HUNT_CROW_DAMAGE,
            AnimalKind::Raven => RAVEN_DAMAGE,
            AnimalKind::Phoenix => PHOENIX_SWOOP_DAMAGE,
            AnimalKind::VampireBat => VAMPIRE_BAT_DAMAGE,
            AnimalKind::Orangutan => ORANGUTAN_DAMAGE,
            AnimalKind::Jaguar => JAGUAR_DAMAGE,
            AnimalKind::FlyingSnake => SNAKE_DAMAGE,
            AnimalKind::Unicorn => UNICORN_DAMAGE,
            AnimalKind::BearBoss => BEAR_BOSS_DAMAGE,
            _ => 0,
        }
    }

    /// Shown when this animal's blow kills the player.
    pub fn death_message(self) -> &'static str {
        match self {
            AnimalKind::Moose => "A moose killed you",
            AnimalKind::Bear => "A bear killed you",
            AnimalKind::Yeti => "The Yeti killed you",
            AnimalKind::LochNessMonster => "The Loch Ness Monster killed you",
            AnimalKind::IronQuail => "The Iron Quail killed you",
            AnimalKind::BronzeParrot => "A Bronze Parrot killed you",
            AnimalKind::Bat => "A bat killed you",
            AnimalKind::Raven => "A raven pecked you to death",
            AnimalKind::Phoenix => "The Phoenix burned you to death",
            AnimalKind::VampireBat => "The Vampire Bat drained you",
            AnimalKind::Orangutan => "The Orangutan crushed you",
            AnimalKind::Jaguar => "A jaguar pounced on you",
            AnimalKind::FlyingSnake => "A flying snake bit you to death",
            AnimalKind::Unicorn => "The Unicorn gored you",
            AnimalKind::BearBoss => "The Great Bear mauled you",
            kind if kind.is_wild_hunt() => "The Wild Hunt killed you",
            _ => "An animal killed you",
        }
    }

    /// Whether weapons can target this kind. Will-o-whisps are scenery and flight behaviour for now.
    pub fn is_huntable(self) -> bool {
        !matches!(self, AnimalKind::WillOWhisp | AnimalKind::SpiritArcher)
    }

    /// What the animal leaves behind when killed.
    pub fn drops(self, rng: &mut Rng) -> Vec<ItemKind> {
        let mut out = Vec::new();
        let mut add = |kind: ItemKind, count: u32| out.extend(std::iter::repeat(kind).take(count as usize));
        match self {
            AnimalKind::Deer => {
                add(ItemKind::Venison, DEER_VENISON_DROP);
                add(ItemKind::Leather, DEER_LEATHER_DROP);
                if rng.chance(DEER_BONE_CHANCE) {
                    add(ItemKind::Bone, DEER_BONE_DROP);
                }
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
            AnimalKind::Yeti => {
                add(ItemKind::BearPelt, YETI_PELT_DROP);
                add(ItemKind::Gold, YETI_GOLD_DROP);
            }
            AnimalKind::LochNessMonster => {
                add(ItemKind::Gold, LOCH_NESS_GOLD_DROP);
                add(ItemKind::Iron, LOCH_NESS_IRON_DROP);
            }
            // A crow leaves only feathers, no poultry.
            AnimalKind::Crow => add(ItemKind::Feather, CROW_FEATHER_DROP),
            AnimalKind::IronQuail => add(ItemKind::SilverKey, 1),
            AnimalKind::Parrot => add(ItemKind::Feather, PARROT_FEATHER_DROP),
            AnimalKind::BronzeParrot => add(ItemKind::Bronze, BRONZE_PARROT_BRONZE_DROP),
            AnimalKind::Raven => add(ItemKind::Feather, RAVEN_FEATHER_DROP),
            AnimalKind::Phoenix => add(ItemKind::PhoenixFeather, PHOENIX_FEATHER_DROP),
            AnimalKind::VampireBat => add(ItemKind::Gold, VAMPIRE_BAT_GOLD_DROP),
            AnimalKind::Orangutan => add(ItemKind::Gold, ORANGUTAN_GOLD_DROP),
            AnimalKind::Jaguar => add(ItemKind::Leather, 2),
            AnimalKind::SpiritArcher => {}
            AnimalKind::Newt => {
                if rng.chance(NEWT_EYE_CHANCE) {
                    add(ItemKind::EyeOfNewt, 1);
                }
            }
            AnimalKind::Unicorn => add(ItemKind::Gold, UNICORN_GOLD_DROP),
            AnimalKind::BearBoss => {
                add(ItemKind::BearMeat, BEAR_BOSS_MEAT_DROP);
                add(ItemKind::BearPelt, BEAR_BOSS_PELT_DROP);
                add(ItemKind::Gold, BEAR_BOSS_GOLD_DROP);
            }
            AnimalKind::FlyingSnake => add(ItemKind::SnakeFang, rng.range(SNAKE_FANG_DROP_MIN as i32, SNAKE_FANG_DROP_MAX as i32 + 1) as u32),
            // Bats and the ghostly creatures of the Wild Hunt leave nothing behind.
            AnimalKind::Bat | AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer | AnimalKind::HuntBear | AnimalKind::HuntCrow => {}
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

/// What a lancer of the Wild Hunt is doing: charging the player, galloping away from them, or resting before the next charge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HuntPhase {
    Dash,
    Retreat,
    Rest,
}

/// What a raven (or the Phoenix) is doing in the air: circling at a distance, swooping at the player, or flying away again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AirPhase {
    Circle,
    Dive,
    Flee,
}

/// What a jaguar is doing: creeping up, crouching to spring, in the air on its leap, or slinking off afterwards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pounce {
    Stalk,
    Crouch,
    Leap,
    Retreat,
}

/// A boss's wind-up for one of its special attacks: it stands still (the Yeti rears up) until the timer runs out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossAction {
    None,
    /// The Yeti raises its fists to hit the ground.
    Smash,
    /// The Yeti lifts a lump of ice to throw.
    IceThrow,
    /// The Orangutan lifts a rock to hurl.
    RockThrow,
    /// The Orangutan is in the air on its leap, to come down beside the player and smash the ground.
    Leap,
}

/// Something a boss has done that the game has to carry out (it owns the player, the projectiles and the other creatures).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BossEvent {
    /// The Yeti's fists have hit the ground here.
    Smash { at: Vec3 },
    /// The Yeti throws ice from here (its hands).
    IceThrow { from: Vec3 },
    /// The Phoenix throws a fireball from here.
    Fireball { from: Vec3 },
    /// Nessie calls this many ravens against a player far from her.
    CallRavens { count: usize },
    /// Nessie taints a line of water from here towards the player.
    Acid { from: Vec3 },
    /// The Orangutan's fists have hit the ground here.
    OrangutanSmash { at: Vec3 },
    /// The Orangutan hurls a rock from here.
    RockThrow { from: Vec3 },
}

/// The turns (radians) `Animal::steer` tries, in order, when something is in a boss's way: straight on, then ever wider to either side.
const STEER_OFFSETS: [f32; 9] = [0.0, 0.5, -0.5, 1.0, -1.0, 1.6, -1.6, 2.4, -2.4];

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
    pub(crate) timer: f32,
    /// Seconds left of being scared (a bear: of being enraged) regardless of distance, after being hurt.
    alarm: f32,
    pub(crate) on_ground: bool,
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
    /// Where a boss appeared: it never strays further than its leash from here.
    pub home: Vec3,
    /// A crow: blocks flown west so far with the player following (see `Game::update_crows`).
    pub west_followed: f32,
    /// A quail or crow called by the pipes: where it is flying to, to land.
    pub lure_target: Option<Vec3>,
    /// Seconds a bird that has come to the pipes still stands quite still.
    pub calm: f32,
    /// A bat called by the pipes: it hunts the player wherever they are, until the dawn.
    pub hunting: bool,
    /// A lancer of the Wild Hunt: what it is doing now, and the seconds left of the charge, the gallop away or the rest.
    pub hunt_phase: HuntPhase,
    pub hunt_timer: f32,
    /// The Wild Hunt's archer: which way (1 or -1) it side-steps.
    strafe: f32,
    /// A raven or the Phoenix: what it is doing in the air, and the seconds left of it.
    pub air_phase: AirPhase,
    air_timer: f32,
    /// Where on its circle round the player an airborne creature is (radians).
    circle: f32,
    /// A raven called by the Loch Ness Monster: it behaves as if it were night, by day as well, and does not leave at dawn.
    pub summoned: bool,
    /// A ghostly bear or crow of the Wild Hunt that splits in two when killed.
    pub splits: bool,
    /// A bat that lives in the mineshafts: it stays by day, and never goes up into the open air.
    pub lives_in_mine: bool,
    /// Seconds until the next arrow (an archer) or iron feather (an Iron Quail) is ready.
    shoot_timer: f32,
    /// An Iron Quail that is keeping its distance to shoot feathers instead of closing in, and for how many more seconds.
    kiting: bool,
    kite_timer: f32,
    /// A boss's special attack in its wind-up, the seconds left of it, and the seconds before each special can be used again.
    pub action: BossAction,
    action_timer: f32,
    special_timer: f32,
    ranged_timer: f32,
    /// What the boss has done that the game must carry out (see `Animal::take_events`).
    events: Vec<BossEvent>,
    /// Steering round obstacles (bosses): the turn applied to the heading, and for how long it is kept.
    steer_turn: f32,
    steer_left: f32,
    steer_force: bool,
    /// Where the boss was `progress_timer` seconds ago: a boss that has not got anywhere is stuck, and takes a stride the other way.
    progress_from: Vec3,
    progress_timer: f32,
    /// Nessie: seconds left of running to the middle of the lake after a shot, and until she next calls ravens or taints water.
    flee_timer: f32,
    raven_timer: f32,
    /// Where she is heading while she runs (recomputed now and then), and seconds until it is.
    flee_heading: f32,
    flee_think: f32,
    /// How fast a push from a blow dies away (`KNOCKBACK_DECAY`, or `LAUNCH_DECAY` after a hard knock).
    knock_decay: f32,
    /// A spirit archer: shots it has left before it vanishes, and (for any creature) seconds left of being marked as something the player
    /// has hit, which is what the spirits shoot at.
    pub shots_left: u32,
    pub marked: f32,
    /// A jaguar: what it is doing, and the seconds left of it.
    pub pounce: Pounce,
    pounce_timer: f32,
    /// Nessie: seconds left of turning back into the lake after meeting the edge of her water, and the way she is turning.
    shore_turn: f32,
    shore_heading: f32,
    /// The Orangutan: it has gone beyond its leash and is walking back (until it is well within it).
    homing: bool,
    /// The Orangutan's leap: its horizontal velocity and the seconds it has been in the air.
    leap_vel: Vec3,
    leap_age: f32,
    /// A flying snake: the places its head has been, newest first, `SNAKE_SEGMENT_SPACING` blocks apart. Its body follows them.
    pub trail: Vec<Vec3>,
}

/// How many blocks (up to `limit`) lie between the point (x, z) and the nearest column that is not water: how far from any shore
/// the open water is there. Zero on dry land.
fn shore_clearance(world: &World, x: f32, z: f32, limit: f32) -> f32 {
    if !world.is_wet_column(x.floor() as i32, z.floor() as i32) {
        return 0.0;
    }
    let mut r = 1.0;
    while r <= limit {
        for i in 0..16 {
            let a = i as f32 * std::f32::consts::TAU / 16.0;
            if !world.is_wet_column((x + a.cos() * r).floor() as i32, (z + a.sin() * r).floor() as i32) {
                return r;
            }
        }
        r += 1.0;
    }
    limit
}

/// What a body bumped into during one step.
struct Contact {
    pub(crate) on_ground: bool,
    hit_wall: bool,
    hit_ceiling: bool,
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
    let mut contact = Contact { on_ground: false, hit_wall: false, hit_ceiling: false };
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
                        } else if vel.y > 0.0 {
                            contact.hit_ceiling = true;
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
            home: pos,
            west_followed: 0.0,
            lure_target: None,
            calm: 0.0,
            hunting: false,
            hunt_phase: HuntPhase::Dash,
            hunt_timer: 0.0,
            strafe: 1.0,
            air_phase: AirPhase::Circle,
            air_timer: 0.0,
            circle: 0.0,
            summoned: false,
            splits: false,
            lives_in_mine: false,
            shoot_timer: 0.0,
            kiting: false,
            kite_timer: 0.0,
            action: BossAction::None,
            action_timer: 0.0,
            special_timer: 0.0,
            ranged_timer: 0.0,
            events: Vec::new(),
            steer_turn: 0.0,
            steer_left: 0.0,
            steer_force: false,
            progress_from: pos,
            progress_timer: 0.0,
            flee_timer: 0.0,
            raven_timer: 0.0,
            flee_heading: 0.0,
            flee_think: 0.0,
            knock_decay: KNOCKBACK_DECAY,
            shots_left: 0,
            marked: 0.0,
            pounce: Pounce::Stalk,
            pounce_timer: 0.0,
            shore_turn: 0.0,
            shore_heading: 0.0,
            homing: false,
            leap_vel: Vec3::ZERO,
            leap_age: 0.0,
            trail: Vec::new(),
        };
        a.circle = a.heading;
        match kind {
            // The archer takes a moment to nock its first arrow.
            AnimalKind::HuntArcher => a.shoot_timer = HUNT_ARCHER_INTERVAL / 2.0,
            // An Iron Quail shoots its first feather a moment after it comes, and each starts at a different point of its cycle.
            AnimalKind::IronQuail | AnimalKind::BronzeParrot => {
                a.shoot_timer = IRON_QUAIL_FEATHER_INTERVAL * (0.3 + 0.7 * rng.unit());
                a.kiting = rng.chance(50);
                a.kite_timer = 3.0 + 4.0 * rng.unit();
            }
            // Lancers start resting for a different moment each, so that two do not always charge together.
            AnimalKind::HuntLancer => {
                a.hunt_phase = HuntPhase::Rest;
                a.hunt_timer = 3.0 * rng.unit();
            }
            AnimalKind::Raven => a.air_timer = RAVEN_ATTACK_MIN + (RAVEN_ATTACK_MAX - RAVEN_ATTACK_MIN) * rng.unit(),
            AnimalKind::Phoenix => a.air_timer = PHOENIX_SWOOP_INTERVAL,
            AnimalKind::VampireBat => a.air_timer = VAMPIRE_BAT_ATTACK_MIN + (VAMPIRE_BAT_ATTACK_MAX - VAMPIRE_BAT_ATTACK_MIN) * rng.unit(),
            AnimalKind::FlyingSnake => a.air_timer = SNAKE_ATTACK_MIN + (SNAKE_ATTACK_MAX - SNAKE_ATTACK_MIN) * rng.unit(),
            AnimalKind::SpiritArcher => a.shots_left = SPIRIT_SHOTS,
            AnimalKind::Orangutan => {
                a.special_timer = 2.0;
                a.ranged_timer = 1.0;
            }
            AnimalKind::LochNessMonster => a.raven_timer = 1.5,
            AnimalKind::Yeti => {
                a.special_timer = 2.0;
                a.ranged_timer = 1.0;
            }
            _ => {}
        }
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

    /// Every box that can be hit: the one round the animal, and for a flying snake also the boxes along its long body.
    pub fn hitboxes(&self) -> Vec<(Vec3, Vec3)> {
        if self.kind != AnimalKind::FlyingSnake {
            return vec![self.hitbox()];
        }
        // A snake's head and wings reach well past its collision box, and its body is long: hit-test all of it.
        let w = SNAKE_HIT_HALF_WIDTH;
        let mut boxes = vec![(self.pos - vec3(w, 0.2, w), self.pos + vec3(w, SNAKE_HIT_HEIGHT, w))];
        let (bw, bh) = (SNAKE_BODY_HIT_HALF, SNAKE_BODY_HIT_HALF_HEIGHT);
        for p in self.body_points().into_iter().skip(1).step_by(2) {
            boxes.push((p - vec3(bw, bh, bw), p + vec3(bw, bh, bw)));
        }
        boxes
    }

    /// A flying snake's body: `SNAKE_SEGMENTS` points from the head back to the tail, along the path the head has flown (and straight
    /// back from where the path begins when it has not flown that far yet).
    pub fn body_points(&self) -> Vec<Vec3> {
        let head = self.pos + vec3(0.0, SNAKE_HEIGHT * 0.5, 0.0);
        let mut pts = Vec::with_capacity(SNAKE_SEGMENTS);
        pts.push(head);
        for i in 1..SNAKE_SEGMENTS {
            if let Some(&p) = self.trail.get(i - 1) {
                pts.push(p + vec3(0.0, SNAKE_HEIGHT * 0.5, 0.0));
            } else {
                let last = *pts.last().unwrap_or(&head);
                let back = match pts.len() {
                    0 | 1 => vec3(-self.heading.cos(), 0.0, -self.heading.sin()),
                    n => (pts[n - 1] - pts[n - 2]).normalize_or_zero(),
                };
                let back = if back == Vec3::ZERO { vec3(-self.heading.cos(), 0.0, -self.heading.sin()) } else { back };
                pts.push(last + back * SNAKE_SEGMENT_SPACING);
            }
        }
        pts
    }

    /// Record where the head has got to: a point goes on the trail each time it is `SNAKE_SEGMENT_SPACING` blocks from the last.
    fn lay_trail(&mut self) {
        let mut last = self.trail.first().copied().unwrap_or(self.pos);
        if self.trail.is_empty() {
            self.trail.push(last);
        }
        let mut guard = 0;
        while (self.pos - last).length() >= SNAKE_SEGMENT_SPACING && guard < 16 {
            last += (self.pos - last).normalize_or_zero() * SNAKE_SEGMENT_SPACING;
            self.trail.insert(0, last);
            guard += 1;
        }
        self.trail.truncate(SNAKE_SEGMENTS);
    }

    /// Hurt the animal for `amount` and scare it (a bear is enraged instead: it charges the player
    /// wherever they are). Returns true when it is dead.
    pub fn damage(&mut self, amount: i32) -> bool {
        if !self.kind.is_huntable() {
            return false;
        }
        self.health -= amount;
        self.alarm = ANIMAL_ALARM_SECONDS;
        // A blow breaks the spell of the pipes.
        self.lure_target = None;
        self.calm = 0.0;
        if self.kind == AnimalKind::Moose {
            self.enraged = true;
        }
        if self.kind == AnimalKind::Bear {
            self.provoked = true;
            self.pacified = false;
        }
        self.health <= 0
    }

    /// Call a bear to the player: it is provoked (so a pelt cloak no longer calms it) and charges from any distance until its alarm runs out,
    /// so a caller that keeps doing this keeps it charging. Does nothing to other animals.
    pub fn enrage_bear(&mut self) {
        if self.kind == AnimalKind::Bear {
            self.provoked = true;
            self.pacified = false;
            self.alarm = ANIMAL_ALARM_SECONDS;
        }
    }

    /// Throw the animal away from whatever hurt it (`away` points from the attacker to the animal), with a hop.
    pub fn knock_back(&mut self, away: Vec3) {
        let flat = vec3(away.x, 0.0, away.z);
        let dir = if flat.length() > 1e-3 { flat.normalize() } else { vec3(self.heading.cos(), 0.0, self.heading.sin()) };
        self.knock = dir * ANIMAL_KNOCKBACK_SPEED;
        self.knock_decay = KNOCKBACK_DECAY;
        if !matches!(self.kind, AnimalKind::Quail | AnimalKind::Crow | AnimalKind::Parrot) || self.on_ground {
            self.vel.y = KNOCKBACK_HOP;
            self.on_ground = false;
        }
    }

    /// Throw the animal `blocks` away from whatever hit it (`away` points from the attacker to the animal): a long, hard throw, like
    /// the Yeti's smash. The push dies away at `LAUNCH_DECAY`, so it carries `blocks` (less what is in the way).
    pub fn launch(&mut self, away: Vec3, blocks: f32) {
        let flat = vec3(away.x, 0.0, away.z);
        let dir = if flat.length() > 1e-3 { flat.normalize() } else { vec3(self.heading.cos(), 0.0, self.heading.sin()) };
        self.knock = dir * blocks * LAUNCH_DECAY;
        self.knock_decay = LAUNCH_DECAY;
        if !self.kind.flies_always() {
            self.vel.y = KNOCKBACK_HOP;
            self.on_ground = false;
        }
        self.lure_target = None;
        self.calm = 0.0;
    }

    /// What the boss has done since this was last called, for the game to carry out.
    pub fn take_events(&mut self) -> Vec<BossEvent> {
        std::mem::take(&mut self.events)
    }

    /// Nessie has heard a shot: she runs for the middle of the lake for `seconds`.
    pub fn startle(&mut self, seconds: f32) {
        if self.kind == AnimalKind::LochNessMonster {
            self.flee_timer = seconds;
            self.flee_think = 0.0;
        }
    }

    /// Whether Nessie is running from a shot.
    pub fn is_fleeing_shot(&self) -> bool {
        self.flee_timer > 0.0
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

    /// Call a quail or a crow (the pipes): it flies to `target` and lands there, then stands still for `PIPES_CALM_SECONDS`.
    /// Other animals take no notice.
    pub fn call_to(&mut self, target: Vec3) {
        if matches!(self.kind, AnimalKind::Quail | AnimalKind::Crow | AnimalKind::Parrot) {
            self.lure_target = Some(target);
            self.west_followed = 0.0;
            self.calm = 0.0;
            self.alarm = 0.0;
        }
    }

    /// Whether a crow taking off here could fly west: nothing solid in the way for `CROW_WEST_LOOKAHEAD` blocks at flying height.
    fn can_fly_west(&self, world: &World) -> bool {
        let (half, h) = (self.kind.half_width(), self.kind.height());
        let y = self.pos.y + QUAIL_FLY_HEIGHT;
        (1..=CROW_WEST_LOOKAHEAD).all(|i| {
            let x = self.pos.x - i as f32;
            !world.box_blocked(vec3(x - half, y, self.pos.z - half), vec3(x + half, y + h, self.pos.z + half))
        })
    }

    /// Whether the animal is off the ground (drawn with its wings beating, for the Iron Quail).
    pub fn is_airborne(&self) -> bool {
        !self.on_ground
    }

    /// Whether a crow is coming down to land (the last part of a flight).
    pub fn is_landing(&self) -> bool {
        self.mode == Mode::Fly && self.timer <= QUAIL_DESCENT_TIME
    }

    /// Set the animal down at `pos` and let it rest there (a crow at an altar).
    pub fn land_at(&mut self, pos: Vec3, rng: &mut Rng) {
        self.pos = pos;
        self.vel = Vec3::ZERO;
        self.on_ground = true;
        self.west_followed = 0.0;
        self.rest(rng);
    }

    /// Whether a boss is below half health (the Yeti then runs and strikes faster).
    pub fn boss_enraged(&self) -> bool {
        self.kind.is_boss() && self.health * 2 <= self.kind.max_health()
    }

    /// The heading from this animal to its home.
    fn heading_home(&self) -> f32 {
        (self.home.z - self.pos.z).atan2(self.home.x - self.pos.x)
    }

    /// Whether this animal's body is within striking distance of the player standing at `player` (feet): a
    /// ring of `BEAR_REACH` around the two bodies, and the two heights overlapping.
    fn within_reach_of(&self, player: Vec3) -> bool {
        let flat = vec2(self.pos.x - player.x, self.pos.z - player.z).length();
        let reach = self.kind.half_width() + PLAYER_HALF_WIDTH + self.kind.strike_reach();
        flat <= reach && player.y < self.pos.y + self.kind.height() && player.y + PLAYER_HEIGHT > self.pos.y
    }

    /// Whether this animal strikes the player standing at `player` (feet) right now. A bear within reach hits once
    /// every `BEAR_ATTACK_COOLDOWN` seconds; a moose only when it is enraged, once every `MOOSE_ATTACK_COOLDOWN`;
    /// any other animal never does.
    pub fn try_attack(&mut self, player: Vec3) -> bool {
        let cooldown = match self.kind {
            AnimalKind::Bear | AnimalKind::Unicorn if self.pacified => return false,
            AnimalKind::Unicorn => UNICORN_ATTACK_COOLDOWN,
            // The pelt cloak does not calm the Great Bear; below half health it strikes faster.
            AnimalKind::BearBoss if self.boss_enraged() => BEAR_BOSS_ATTACK_COOLDOWN * BEAR_BOSS_ENRAGED_COOLDOWN_FACTOR,
            AnimalKind::BearBoss => BEAR_BOSS_ATTACK_COOLDOWN,
            AnimalKind::Bear => BEAR_ATTACK_COOLDOWN,
            AnimalKind::Moose if self.enraged => MOOSE_ATTACK_COOLDOWN,
            AnimalKind::Yeti if self.boss_enraged() => YETI_ATTACK_COOLDOWN * YETI_ENRAGED_COOLDOWN_FACTOR,
            AnimalKind::Yeti => YETI_ATTACK_COOLDOWN,
            AnimalKind::LochNessMonster => LOCH_NESS_ATTACK_COOLDOWN,
            AnimalKind::IronQuail => IRON_QUAIL_ATTACK_COOLDOWN,
            AnimalKind::BronzeParrot => BRONZE_PARROT_ATTACK_COOLDOWN,
            AnimalKind::Bat => BAT_ATTACK_COOLDOWN,
            AnimalKind::HuntSwordsman => HUNT_SWORD_COOLDOWN,
            // A lancer strikes only while charging, and once: then it gallops away.
            AnimalKind::HuntLancer if self.hunt_phase == HuntPhase::Dash => HUNT_LANCER_COOLDOWN,
            AnimalKind::HuntBear => HUNT_BEAR_COOLDOWN,
            AnimalKind::HuntCrow => HUNT_CROW_COOLDOWN,
            // A raven pecks only on its dive, once: then it flies away.
            AnimalKind::Raven if self.air_phase == AirPhase::Dive => RAVEN_ATTACK_MIN,
            // The Phoenix burns only on a swoop.
            AnimalKind::Phoenix if self.air_phase == AirPhase::Dive => 1.0,
            // The vampire bat and the flying snake bite only on the swoop; the jaguar only in the air on its leap.
            AnimalKind::VampireBat if self.air_phase == AirPhase::Dive => VAMPIRE_BAT_ATTACK_MIN,
            AnimalKind::FlyingSnake if self.air_phase == AirPhase::Dive => SNAKE_ATTACK_MIN,
            AnimalKind::Jaguar if self.pounce == Pounce::Leap => JAGUAR_ATTACK_COOLDOWN,
            AnimalKind::Orangutan if self.boss_enraged() => ORANGUTAN_ATTACK_COOLDOWN * ORANGUTAN_ENRAGED_FACTOR,
            AnimalKind::Orangutan => ORANGUTAN_ATTACK_COOLDOWN,
            _ => return false,
        };
        // A boss winding up a special attack does not also strike.
        if self.attack_cooldown > 0.0 || self.action != BossAction::None || !self.within_reach_of(player) {
            return false;
        }
        self.attack_cooldown = cooldown;
        match self.kind {
            AnimalKind::HuntLancer => {
                self.hunt_phase = HuntPhase::Retreat;
                self.hunt_timer = HUNT_LANCER_RETREAT_SECONDS;
            }
            AnimalKind::Raven => {
                self.air_phase = AirPhase::Flee;
                self.air_timer = RAVEN_FLEE_SECONDS;
            }
            AnimalKind::Phoenix => {
                self.air_phase = AirPhase::Flee;
                self.air_timer = 2.0;
            }
            AnimalKind::VampireBat => {
                self.air_phase = AirPhase::Flee;
                self.air_timer = VAMPIRE_BAT_FLEE_SECONDS;
            }
            AnimalKind::FlyingSnake => {
                self.air_phase = AirPhase::Flee;
                self.air_timer = SNAKE_FLEE_SECONDS;
            }
            AnimalKind::Jaguar => {
                self.pounce = Pounce::Retreat;
                self.pounce_timer = JAGUAR_RETREAT_SECONDS;
            }
            _ => {}
        }
        true
    }

    /// A spirit archer's own place on the circle it drifts round the player on.
    pub fn orbit_angle(&self) -> f32 {
        self.orbit
    }

    /// Give a spirit archer its own place on the circle it drifts round the player on.
    pub fn set_orbit(&mut self, angle: f32) {
        self.orbit = angle;
    }

    /// Whether a spirit archer has an arrow nocked: it has shots left and its interval is over.
    pub fn spirit_ready(&self) -> bool {
        self.kind == AnimalKind::SpiritArcher && self.shots_left > 0 && self.shoot_timer <= 0.0
    }

    /// A spirit archer has shot: its interval starts, and one of its `SPIRIT_SHOTS` is used. Returns whether that was the last (it then vanishes).
    pub fn spirit_fired(&mut self) -> bool {
        self.shoot_timer = SPIRIT_SHOOT_INTERVAL;
        self.shots_left = self.shots_left.saturating_sub(1);
        self.shots_left == 0
    }

    /// Whether this creature has a missile ready for a player standing at `player` (feet): the Wild Hunt's archer has nocked an
    /// arrow and is in range, or an Iron Quail has an iron feather ready and is between `IRON_QUAIL_FEATHER_MIN_RANGE` and `MAX_RANGE`.
    /// The game then checks it can see the player and calls `shot_fired`.
    pub fn ready_to_shoot(&self, player: Vec3) -> bool {
        let gap = (self.pos - player).length();
        match self.kind {
            AnimalKind::HuntArcher => self.shoot_timer <= 0.0 && gap <= HUNT_ARCHER_RANGE,
            AnimalKind::IronQuail | AnimalKind::BronzeParrot => self.shoot_timer <= 0.0 && (IRON_QUAIL_FEATHER_MIN_RANGE..=IRON_QUAIL_FEATHER_MAX_RANGE).contains(&gap),
            _ => false,
        }
    }

    /// The creature has shot: it is ready again after its interval (`HUNT_ARCHER_INTERVAL` or `IRON_QUAIL_FEATHER_INTERVAL`), times
    /// `slowdown` (2 when the player has a Rabbit Foot in the hotbar, so that an archer fires half as often).
    pub fn shot_fired(&mut self, slowdown: f32) {
        self.shoot_timer = match self.kind {
            AnimalKind::IronQuail | AnimalKind::BronzeParrot => IRON_QUAIL_FEATHER_INTERVAL,
            _ => HUNT_ARCHER_INTERVAL * slowdown,
        };
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
        self.shoot_timer = (self.shoot_timer - dt).max(0.0);
        self.special_timer = (self.special_timer - dt).max(0.0);
        self.ranged_timer = (self.ranged_timer - dt).max(0.0);
        self.raven_timer = (self.raven_timer - dt).max(0.0);
        self.flee_timer = (self.flee_timer - dt).max(0.0);
        self.marked = (self.marked - dt).max(0.0);
        self.timer -= dt;
        self.calm = (self.calm - dt).max(0.0);
        self.free_itself(world);
        if matches!(self.kind, AnimalKind::Raven | AnimalKind::Phoenix | AnimalKind::VampireBat | AnimalKind::FlyingSnake | AnimalKind::SpiritArcher) {
            self.update_flier(world, threat, dt, rng);
            return None;
        }
        // A bird that the pipes have called (or are calming) is not frightened by the player, only by a blow.
        let charmed = self.lure_target.is_some() || self.calm > 0.0;
        let scared = self.alarm > 0.0 || (dist < FLEE_DISTANCE && !charmed);
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
            AnimalKind::Quail | AnimalKind::Crow | AnimalKind::Parrot => {
                if scared && self.mode != Mode::Fly && self.on_ground {
                    self.mode = Mode::Fly;
                    self.timer = Self::random_time((QUAIL_FLIGHT_MIN, QUAIL_FLIGHT_MAX), rng);
                    self.launch_y = self.pos.y;
                    // A crow always flies west, if it can; otherwise it flies away like a quail.
                    self.heading = if self.kind == AnimalKind::Crow && self.can_fly_west(world) { std::f32::consts::PI } else { away };
                    dropped = Some(ItemKind::Feather);
                }
                if let Some(t) = self.lure_target {
                    let gap = vec2(t.x - self.pos.x, t.z - self.pos.z).length();
                    if self.mode != Mode::Fly && self.on_ground {
                        self.mode = Mode::Fly;
                        self.timer = 60.0;
                        self.launch_y = self.pos.y;
                    }
                    if self.mode == Mode::Fly {
                        self.heading = (t.z - self.pos.z).atan2(t.x - self.pos.x);
                        if gap < 0.6 && self.timer > QUAIL_DESCENT_TIME {
                            self.timer = QUAIL_DESCENT_TIME;
                        }
                        if self.timer <= QUAIL_DESCENT_TIME && self.on_ground {
                            self.lure_target = None;
                            self.calm = PIPES_CALM_SECONDS;
                        }
                    }
                }
                let landing = self.timer <= QUAIL_DESCENT_TIME;
                if self.mode == Mode::Fly && (self.timer <= 0.0 || (landing && self.on_ground)) {
                    self.rest(rng);
                    if self.calm > 0.0 {
                        self.timer = self.calm;
                    }
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
            AnimalKind::Newt => {}
            AnimalKind::BearBoss => {
                // It charges from any distance and is not calmed by a pelt cloak.
                self.mode = Mode::Charge;
                if flat_gap > 1e-3 {
                    self.heading = towards + detour;
                }
            }
            AnimalKind::Unicorn => {
                // At peace while the player is in the jungle (the game sets `pacified`); otherwise it charges from any distance.
                if self.pacified {
                    if self.mode == Mode::Charge {
                        self.rest(rng);
                    }
                } else {
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
            AnimalKind::IronQuail | AnimalKind::BronzeParrot => {
                // She hunts the player wherever they are, for ever: closing in to strike for a while, then keeping her distance
                // (strafing round the player) to shoot iron feathers, and so on.
                self.mode = Mode::Charge;
                self.kite_timer -= dt;
                if self.kite_timer <= 0.0 {
                    self.kiting = !self.kiting;
                    self.kite_timer = if self.kiting { 5.0 + 2.0 * rng.unit() } else { 5.0 + 3.0 * rng.unit() };
                    self.strafe = if rng.chance(50) { 1.0 } else { -1.0 };
                }
                if flat_gap > 1e-3 {
                    self.heading = if !self.kiting {
                        towards
                    } else if flat_gap < IRON_QUAIL_KITE_MIN {
                        away
                    } else if flat_gap > IRON_QUAIL_KITE_MAX {
                        towards
                    } else {
                        towards + self.strafe * std::f32::consts::FRAC_PI_2
                    };
                }
            }
            AnimalKind::Bat => {
                // A bat of the mines notices only what is near it in all three directions (the rock is thick).
                let noticed = if self.lives_in_mine { dist < BAT_MINE_NOTICE_DISTANCE } else { flat_gap < BAT_NOTICE_DISTANCE };
                if self.hunting || noticed || self.alarm > 0.0 {
                    self.mode = Mode::Charge;
                } else if self.mode == Mode::Charge {
                    self.rest(rng);
                }
                if self.mode == Mode::Charge && flat_gap > 1e-3 {
                    // A bat's flight is jinking: it weaves from side to side as it comes.
                    self.heading = towards + detour + (self.phase * 3.0).sin() * 0.5;
                }
            }
            AnimalKind::HuntSwordsman => {
                self.mode = Mode::Charge;
                if flat_gap > 1e-3 {
                    self.heading = towards + detour;
                }
            }
            AnimalKind::HuntArcher => {
                self.hunt_timer -= dt;
                if self.hunt_timer <= 0.0 {
                    self.strafe = -self.strafe;
                    self.hunt_timer = 2.5;
                }
                if flat_gap < HUNT_ARCHER_MIN_RANGE {
                    self.mode = Mode::Flee;
                    self.heading = away + detour;
                } else if flat_gap > HUNT_ARCHER_MAX_RANGE {
                    self.mode = Mode::Charge;
                    self.heading = towards + detour;
                } else {
                    self.mode = Mode::Wander;
                    self.heading = towards + self.strafe * std::f32::consts::FRAC_PI_2;
                }
            }
            AnimalKind::HuntLancer => {
                // Charge straight at the player, gallop away after the blow (or a charge that missed), rest, and charge again.
                self.mode = Mode::Charge;
                self.hunt_timer -= dt;
                match self.hunt_phase {
                    HuntPhase::Dash => {
                        if self.hunt_timer <= 0.0 {
                            self.hunt_phase = HuntPhase::Retreat;
                            self.hunt_timer = HUNT_LANCER_RETREAT_SECONDS;
                        } else if flat_gap > 1e-3 {
                            self.heading = towards;
                        }
                    }
                    HuntPhase::Retreat => {
                        if self.hunt_timer <= 0.0 {
                            self.hunt_phase = HuntPhase::Rest;
                            self.hunt_timer = HUNT_LANCER_REST_SECONDS;
                        } else if flat_gap > 1e-3 {
                            self.heading = away;
                        }
                    }
                    HuntPhase::Rest => {
                        if self.hunt_timer <= 0.0 {
                            self.hunt_phase = HuntPhase::Dash;
                            self.hunt_timer = HUNT_LANCER_CHARGE_SECONDS;
                        } else if flat_gap > 1e-3 {
                            self.heading = towards;
                        }
                    }
                }
            }
            AnimalKind::HuntBear => {
                self.mode = Mode::Charge;
                if flat_gap > 1e-3 {
                    self.heading = towards;
                }
            }
            AnimalKind::HuntCrow => {
                self.mode = Mode::Charge;
                if flat_gap > 1e-3 {
                    self.heading = towards + (self.phase * 3.0).sin() * 0.5;
                }
            }
            AnimalKind::Yeti => {
                let home_gap = vec2(self.pos.x - self.home.x, self.pos.z - self.home.z).length();
                if home_gap > YETI_LEASH {
                    // Too far from home: give up the chase and go back.
                    self.alarm = 0.0;
                    self.mode = Mode::Wander;
                    self.heading = self.heading_home();
                    self.timer = 1.0;
                    self.action = BossAction::None;
                } else if flat_gap < YETI_NOTICE_DISTANCE || self.alarm > 0.0 {
                    self.mode = Mode::Charge;
                } else if self.mode == Mode::Charge {
                    self.rest(rng);
                    self.action = BossAction::None;
                }
                if self.mode == Mode::Charge && flat_gap > 1e-3 {
                    self.heading = towards;
                }
                self.update_yeti_attacks(flat_gap, towards, dt);
            }
            AnimalKind::LochNessMonster => {
                let home_gap = vec2(self.pos.x - self.home.x, self.pos.z - self.home.z).length();
                if self.flee_timer > 0.0 {
                    // A shot was heard: she runs for the open middle of the lake.
                    self.mode = Mode::Charge;
                    self.flee_think -= dt;
                    if self.flee_think <= 0.0 {
                        self.flee_think = 0.5;
                        self.flee_heading = self.open_water_heading(world);
                    }
                    self.heading = self.flee_heading;
                } else if home_gap > LOCH_NESS_LEASH {
                    self.alarm = 0.0;
                    self.mode = Mode::Wander;
                    self.heading = self.heading_home();
                    self.timer = 1.0;
                } else if flat_gap < LOCH_NESS_NOTICE_DISTANCE || self.alarm > 0.0 {
                    self.mode = Mode::Charge;
                } else if self.mode == Mode::Charge {
                    self.rest(rng);
                }
                if self.flee_timer <= 0.0 && self.mode == Mode::Charge && flat_gap > 1e-3 {
                    self.heading = towards;
                }
                if self.flee_timer <= 0.0 && flat_gap < LOCH_NESS_RAVEN_DISTANCE {
                    // Far from her, the player is set on by ravens; and a line of her water is made acid.
                    if flat_gap > LOCH_NESS_RAVEN_RANGE && self.raven_timer <= 0.0 {
                        self.events.push(BossEvent::CallRavens { count: LOCH_NESS_RAVENS_PER_CALL });
                        self.raven_timer = LOCH_NESS_RAVEN_INTERVAL;
                    }
                    if flat_gap <= LOCH_NESS_ACID_RANGE && self.ranged_timer <= 0.0 {
                        self.events.push(BossEvent::Acid { from: self.pos });
                        self.ranged_timer = LOCH_NESS_ACID_COOLDOWN;
                    }
                }
            }
            AnimalKind::Orangutan => {
                let home_gap = vec2(self.pos.x - self.home.x, self.pos.z - self.home.z).length();
                // Beyond the leash it walks back, and keeps walking until it is well inside it (so that it does not turn round again
                // at the very edge, which is where the foot of the temple is).
                if home_gap > ORANGUTAN_LEASH {
                    self.homing = true;
                } else if home_gap <= ORANGUTAN_LEASH * ORANGUTAN_HOME_RETURN_SHARE {
                    self.homing = false;
                }
                if self.action == BossAction::Leap {
                    // In the air: nothing changes its mind until it has come down.
                } else if self.homing {
                    // Too far from the temple top: give up the chase and go back.
                    self.alarm = 0.0;
                    self.mode = Mode::Wander;
                    self.heading = self.heading_home();
                    self.timer = 1.0;
                    self.action = BossAction::None;
                } else if flat_gap < ORANGUTAN_NOTICE_DISTANCE || self.alarm > 0.0 {
                    self.mode = Mode::Charge;
                } else if self.mode == Mode::Charge {
                    self.rest(rng);
                    self.action = BossAction::None;
                }
                if self.mode == Mode::Charge && flat_gap > 1e-3 {
                    self.heading = towards;
                }
                self.update_orangutan_attacks(flat_gap, towards, dt, threat);
            }
            AnimalKind::Jaguar => {
                // It creeps up on a player it has noticed, crouches, springs, and slinks off after the leap (hit or miss).
                self.pounce_timer -= dt;
                match self.pounce {
                    Pounce::Stalk => {
                        if flat_gap < JAGUAR_NOTICE_DISTANCE || self.alarm > 0.0 {
                            self.mode = Mode::Charge;
                            if flat_gap > 1e-3 {
                                self.heading = towards + detour;
                            }
                            if flat_gap <= JAGUAR_POUNCE_RANGE && self.on_ground {
                                self.pounce = Pounce::Crouch;
                                self.pounce_timer = JAGUAR_CROUCH_SECONDS;
                            }
                        } else if self.mode == Mode::Charge {
                            self.rest(rng);
                        }
                    }
                    Pounce::Crouch => {
                        self.mode = Mode::Charge;
                        if flat_gap > 1e-3 {
                            self.heading = towards;
                        }
                        if self.pounce_timer <= 0.0 {
                            self.pounce = Pounce::Leap;
                            self.pounce_timer = JAGUAR_LEAP_SECONDS;
                            self.vel.y = JAGUAR_LEAP_RISE;
                            self.on_ground = false;
                        }
                    }
                    Pounce::Leap => {
                        self.mode = Mode::Charge;
                        if self.pounce_timer <= 0.0 || (self.on_ground && self.pounce_timer < JAGUAR_LEAP_SECONDS - 0.25) {
                            self.pounce = Pounce::Retreat;
                            self.pounce_timer = JAGUAR_RETREAT_SECONDS;
                        }
                    }
                    Pounce::Retreat => {
                        self.mode = Mode::Charge;
                        if flat_gap > 1e-3 {
                            self.heading = away + detour;
                        }
                        if self.pounce_timer <= 0.0 {
                            self.pounce = Pounce::Stalk;
                            self.rest(rng);
                        }
                    }
                }
            }
            // Fliers with their own movement never get here (see `update_flier`).
            AnimalKind::Raven | AnimalKind::Phoenix | AnimalKind::VampireBat | AnimalKind::FlyingSnake | AnimalKind::SpiritArcher => {}
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

        if self.kind != AnimalKind::WillOWhisp && !self.kind.is_wild_hunt() && self.calm <= 0.0 && matches!(self.mode, Mode::Idle | Mode::Wander) && self.timer <= 0.0 {
            if self.mode == Mode::Idle {
                self.mode = Mode::Wander;
                let straying = self.kind.is_leashed() && vec2(self.pos.x - self.home.x, self.pos.z - self.home.z).length() > 12.0;
                self.heading = if straying {
                    self.heading_home()
                } else if self.kind == AnimalKind::Bear && !self.pacified && dist < BEAR_NOTICE_DISTANCE {
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

        let mut speed = match self.mode {
            Mode::Idle => 0.0,
            Mode::Wander => self.kind.walk_speed(),
            Mode::Flee => self.kind.run_speed(),
            // A bird flying to the pipes slows as it comes to the place it is to land at.
            Mode::Fly if self.lure_target.is_some() => {
                let t = self.lure_target.unwrap();
                QUAIL_FLY_SPEED.min(vec2(t.x - self.pos.x, t.z - self.pos.z).length() * 4.0)
            }
            Mode::Fly => QUAIL_FLY_SPEED,
            // A boss winding up a special attack stands still.
            _ if self.action != BossAction::None => 0.0,
            // A lancer at rest stands.
            Mode::Charge if self.kind == AnimalKind::HuntLancer && self.hunt_phase == HuntPhase::Rest => 0.0,
            // A charging bear stops once it stands against the player, instead of running through them.
            Mode::Charge if flat_gap <= self.kind.half_width() + PLAYER_HALF_WIDTH && !(self.kind == AnimalKind::HuntLancer && self.hunt_phase == HuntPhase::Retreat) && !(self.kind == AnimalKind::Jaguar && self.pounce == Pounce::Retreat) && self.flee_timer <= 0.0 => 0.0,
            Mode::Charge if self.kind == AnimalKind::Unicorn => UNICORN_RUN_SPEED,
            Mode::Charge if self.kind == AnimalKind::BearBoss => BEAR_BOSS_RUN_SPEED * if self.boss_enraged() { BEAR_BOSS_ENRAGED_SPEED_FACTOR } else { 1.0 },
            Mode::Charge if self.kind == AnimalKind::Moose => MOOSE_RUN_SPEED,
            Mode::Charge if self.kind == AnimalKind::Orangutan => ORANGUTAN_RUN_SPEED * if self.boss_enraged() { 1.25 } else { 1.0 },
            Mode::Charge if self.kind == AnimalKind::Jaguar => match self.pounce {
                Pounce::Stalk => JAGUAR_STALK_SPEED,
                Pounce::Crouch => 0.0,
                Pounce::Leap => JAGUAR_LEAP_SPEED,
                Pounce::Retreat => JAGUAR_RETREAT_SPEED,
            },
            Mode::Charge if self.kind == AnimalKind::Yeti => YETI_RUN_SPEED * if self.boss_enraged() { YETI_ENRAGED_SPEED_FACTOR } else { 1.0 },
            Mode::Charge if self.kind == AnimalKind::LochNessMonster => LOCH_NESS_RUN_SPEED * if self.flee_timer > 0.0 { 1.6 } else { 1.0 },
            Mode::Charge if self.kind == AnimalKind::IronQuail => IRON_QUAIL_RUN_SPEED,
            Mode::Charge if self.kind == AnimalKind::BronzeParrot => BRONZE_PARROT_RUN_SPEED,
            Mode::Charge if self.kind == AnimalKind::Bat => BAT_SPEED,
            Mode::Charge if self.kind == AnimalKind::HuntCrow => HUNT_CROW_SPEED,
            Mode::Charge if self.kind == AnimalKind::HuntBear => HUNT_BEAR_SPEED,
            Mode::Charge if self.kind == AnimalKind::HuntSwordsman => HUNT_SWORDSMAN_SPEED,
            Mode::Charge if self.kind == AnimalKind::HuntArcher => HUNT_ARCHER_SPEED,
            Mode::Charge if self.kind == AnimalKind::HuntLancer => match self.hunt_phase {
                HuntPhase::Dash => HUNT_LANCER_CHARGE_SPEED,
                HuntPhase::Retreat => HUNT_LANCER_RETREAT_SPEED,
                HuntPhase::Rest => 0.0,
            },
            Mode::Charge => BEAR_RUN_SPEED,
        };
        if self.kind == AnimalKind::LochNessMonster {
            // The monster stays in the water, and keeps `LOCH_NESS_SHORE_MARGIN` blocks from the shore. When her way ahead meets the
            // edge of her water she does not stop against it: she turns round and swims back towards the open middle of the lake for
            // `LOCH_NESS_TURN_SECONDS` before she tries for her goal again. (One that finds herself nearer the shore may still move
            // to open water.)
            self.knock = Vec3::ZERO;
            self.shore_turn = (self.shore_turn - dt).max(0.0);
            if speed > 0.0 {
                let reach = speed * dt + self.kind.half_width() + 1.0;
                let here = shore_clearance(world, self.pos.x, self.pos.z, LOCH_NESS_SHORE_MARGIN);
                let ok = |h: f32| {
                    let (x, z) = (self.pos.x + h.cos() * reach, self.pos.z + h.sin() * reach);
                    world.is_wet_column(x.floor() as i32, z.floor() as i32) && {
                        let there = shore_clearance(world, x, z, LOCH_NESS_SHORE_MARGIN);
                        there >= LOCH_NESS_SHORE_MARGIN || there >= here
                    }
                };
                let wanted = if self.shore_turn > 0.0 { self.shore_heading } else { self.heading };
                if !ok(wanted) {
                    // Meeting the boundary: turn back into the lake and keep to that for a while.
                    let back = self.open_water_heading(world);
                    let found = STEER_OFFSETS.into_iter().map(|t| back + t).find(|&h| ok(h));
                    match found {
                        Some(h) => {
                            self.shore_heading = h;
                            self.shore_turn = LOCH_NESS_TURN_SECONDS;
                            self.heading = h;
                        }
                        None => speed = 0.0,
                    }
                } else {
                    self.heading = wanted;
                }
            }
        } else if self.kind.is_boss() && !self.kind.flies_always() && matches!(self.mode, Mode::Charge | Mode::Flee | Mode::Wander) && speed > 0.0 {
            // A boss feels for a free way ahead instead of walking into a tree, a rock or a cliff.
            self.heading = self.steer(world, self.heading, dt);
        }
        self.vel.x = self.heading.cos() * speed;
        self.vel.z = self.heading.sin() * speed;
        if self.action == BossAction::Leap {
            self.vel.x = self.leap_vel.x;
            self.vel.z = self.leap_vel.z;
        }
        self.vel.x += self.knock.x;
        self.vel.z += self.knock.z;
        self.knock *= (1.0 - self.knock_decay * dt).max(0.0);
        if self.kind.is_bat_like() {
            // A bat never touches the ground: it flutters up and down about the player's body as it attacks, and about its
            // own height when it wanders.
            let mut goal = if self.mode == Mode::Charge { threat.y + 0.9 + 0.4 * (self.phase * 2.3).sin() + if self.stuck > 0.0 { 3.0 } else { 0.0 } } else { self.launch_y + 0.5 * (self.phase * 1.7).sin() };
            if let Some(top) = self.ceiling_limit(world, self.pos.x, self.pos.z) {
                goal = goal.min(top.max(self.floor_hint(world)));
            }
            self.vel.y = ((goal - self.pos.y) * 3.0).clamp(-4.0, 4.0);
        } else if self.kind.is_metal_bird() && self.mode == Mode::Charge {
            // She flies: up to just over the player's head while she is on her way (higher still if something is in the way),
            // and down to their level once she is beside them.
            let goal = threat.y + if flat_gap > 3.0 { 2.0 } else { 0.0 } + if self.stuck > 0.0 { 8.0 } else { 0.0 };
            self.vel.y = ((goal - self.pos.y) * 3.0).clamp(-6.0, 6.0);
        } else if self.mode == Mode::Fly {
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
            // Hop up a step (a boss, two blocks of it); if that does not help, turn aside after a moment.
            if self.on_ground && self.mode != Mode::Fly {
                // A boss jumps only at something it can get over (not at a wall that is too high).
                if !self.kind.is_boss() {
                    self.vel.y = ANIMAL_JUMP_SPEED;
                } else if self.can_hop(world, self.heading) {
                    self.vel.y = BOSS_JUMP_SPEED;
                }
            }
            self.stuck += dt;
            if self.stuck > 0.5 {
                self.stuck = 0.0;
                let sign = if rng.chance(50) { 1.0 } else { -1.0 };
                let turn = sign * (1.0 + rng.unit() * 0.8);
                if self.kind.is_boss() {
                    // Keep going round the way it already was (a new random side each time makes it dither along a wall).
                    if self.steer_turn == 0.0 {
                        self.steer_turn = turn;
                    }
                    self.steer_left = BOSS_DETOUR_SECONDS;
                } else if matches!(self.mode, Mode::Flee | Mode::Charge) {
                    self.detour = turn;
                    self.detour_left = 1.5;
                } else {
                    self.heading += turn;
                }
            }
        } else {
            self.stuck = (self.stuck - dt).max(0.0);
        }
        self.check_progress(speed, dt);
        dropped
    }

    /// A boss that is trying to move but has got nowhere in `BOSS_UNSTICK_SECONDS` (wedged between two things, or in a pit) takes
    /// a few strides in another direction, with a jump, whatever is ahead.
    fn check_progress(&mut self, speed: f32, dt: f32) {
        if !self.kind.is_boss() || speed <= 0.0 || self.kind == AnimalKind::LochNessMonster {
            self.progress_timer = 0.0;
            self.progress_from = self.pos;
            return;
        }
        self.progress_timer += dt;
        if self.progress_timer < BOSS_UNSTICK_SECONDS {
            return;
        }
        let moved = vec2(self.pos.x - self.progress_from.x, self.pos.z - self.progress_from.z).length();
        if moved < BOSS_UNSTICK_PROGRESS {
            self.steer_turn = if self.steer_turn >= 0.0 { -2.0 } else { 2.0 };
            self.steer_left = 1.5;
            self.steer_force = true;
            if self.on_ground {
                self.vel.y = BOSS_JUMP_SPEED;
            }
        }
        self.progress_from = self.pos;
        self.progress_timer = 0.0;
    }

    /// Whether what is ahead along `heading` is a ledge a jump gets over (one block, or two for a boss).
    fn can_hop(&self, world: &World, heading: f32) -> bool {
        let (half, height) = (self.kind.half_width(), self.kind.height());
        let reach = half + 0.6;
        let p = self.pos + vec3(heading.cos() * reach, 0.0, heading.sin() * reach);
        let eps = 1e-3;
        let blocked_at = |lift: f32| world.box_blocked(vec3(p.x - half + eps, p.y + lift + eps, p.z - half + eps), vec3(p.x + half - eps, p.y + lift + height - eps, p.z + half - eps));
        !blocked_at(1.0) || (self.kind.is_boss() && !blocked_at(2.0))
    }

    /// Whether a body of this animal's size could go along `heading` for `BOSS_FEEL_DISTANCE` blocks: nothing is in the way, or
    /// what is in the way is a ledge it can climb (one block for anyone, two for a boss on the ground).
    fn way_is_free(&self, world: &World, heading: f32) -> bool {
        let (half, height) = (self.kind.half_width(), self.kind.height());
        let reach = BOSS_FEEL_DISTANCE + half;
        let p = self.pos + vec3(heading.cos() * reach, 0.0, heading.sin() * reach);
        let eps = 1e-3;
        let blocked_at = |lift: f32| {
            world.box_blocked(vec3(p.x - half + eps, p.y + lift + eps, p.z - half + eps), vec3(p.x + half - eps, p.y + lift + height - eps, p.z + half - eps))
        };
        if !blocked_at(0.0) {
            return true;
        }
        let walker = !self.kind.flies_always() && !self.kind.is_metal_bird();
        // (In the air, mid-jump, the ledge counts too: it is no reason to turn back.)
        walker && (!blocked_at(1.0) || (self.kind.is_boss() && !blocked_at(2.0)))
    }

    /// The heading a boss should take to get towards `want`: `want` itself if the way is free, otherwise the nearest free heading to
    /// either side (the side it last went round by, so it does not dither), kept for `BOSS_DETOUR_SECONDS` so that it gets round
    /// the thing before it tries the straight line again. Boxed in on every side, it turns back.
    fn steer(&mut self, world: &World, want: f32, dt: f32) -> f32 {
        self.steer_left = (self.steer_left - dt).max(0.0);
        if self.steer_left <= 0.0 {
            self.steer_force = false;
        }
        if self.steer_left > 0.0 {
            let h = want + self.steer_turn;
            if self.steer_force || self.way_is_free(world, h) {
                return h;
            }
            self.steer_left = 0.0;
        }
        if self.way_is_free(world, want) {
            self.steer_turn = 0.0;
            return want;
        }
        // Stay on the side it went round by (if it has one): nearest turns first, and only if none of that side is free the other.
        let prefer = if self.steer_turn >= 0.0 { 1.0 } else { -1.0 };
        for side in [prefer, -prefer] {
            for off in STEER_OFFSETS.into_iter().filter(|o| *o > 0.0) {
                let turn = off * side;
                if self.way_is_free(world, want + turn) {
                    self.steer_turn = turn;
                    self.steer_left = BOSS_DETOUR_SECONDS;
                    return want + turn;
                }
            }
        }
        self.steer_turn = std::f32::consts::PI * 0.9;
        self.steer_left = BOSS_DETOUR_SECONDS;
        want + self.steer_turn
    }

    /// The Yeti's special attacks. In its charge, within `YETI_SMASH_TRIGGER_RANGE` it rears up and then hits the ground (a smash,
    /// `BossEvent::Smash`); beyond `YETI_ICE_MIN_RANGE` it lifts a lump of ice and throws it (`BossEvent::IceThrow`). Either one
    /// holds it still for its wind-up. Below half health the cooldowns are shorter.
    fn update_yeti_attacks(&mut self, flat_gap: f32, towards: f32, dt: f32) {
        let quick = if self.boss_enraged() { YETI_ENRAGED_COOLDOWN_FACTOR } else { 1.0 };
        match self.action {
            BossAction::None => {
                if self.mode == Mode::Charge && self.on_ground {
                    if flat_gap <= YETI_SMASH_TRIGGER_RANGE && self.special_timer <= 0.0 {
                        self.action = BossAction::Smash;
                        self.action_timer = YETI_SMASH_WINDUP;
                    } else if flat_gap > YETI_ICE_MIN_RANGE && flat_gap <= YETI_ICE_MAX_RANGE && self.ranged_timer <= 0.0 {
                        self.action = BossAction::IceThrow;
                        self.action_timer = YETI_ICE_WINDUP;
                    }
                }
            }
            BossAction::Smash => {
                self.action_timer -= dt;
                if self.action_timer <= 0.0 {
                    self.events.push(BossEvent::Smash { at: self.pos });
                    self.special_timer = YETI_SMASH_COOLDOWN * quick;
                    self.attack_cooldown = self.attack_cooldown.max(0.9);
                    self.action = BossAction::None;
                }
            }
            BossAction::RockThrow | BossAction::Leap => {}
            BossAction::IceThrow => {
                self.action_timer -= dt;
                if self.action_timer <= 0.0 {
                    let from = self.pos + vec3(self.heading.cos() * 0.9, 2.4, self.heading.sin() * 0.9);
                    self.events.push(BossEvent::IceThrow { from });
                    self.ranged_timer = YETI_ICE_COOLDOWN * quick;
                    self.action = BossAction::None;
                }
            }
        }
        if self.action != BossAction::None && flat_gap > 1e-3 {
            // It faces the player while it winds up.
            self.heading = towards;
        }
    }

    /// Spring into the air to come down `ORANGUTAN_LEAP_LAND_GAP` blocks from the player (feet at `threat`), on the side it leaps from.
    /// The jump is a plain ballistic arc under gravity whose top is `ORANGUTAN_LEAP_APEX` above the higher of the two ends.
    fn begin_leap(&mut self, threat: Vec3) {
        let away = vec2(self.pos.x - threat.x, self.pos.z - threat.z);
        let dir = if away.length() > 1e-3 { away.normalize() } else { vec2(self.heading.cos(), self.heading.sin()) };
        let target = vec3(threat.x + dir.x * ORANGUTAN_LEAP_LAND_GAP, threat.y, threat.z + dir.y * ORANGUTAN_LEAP_LAND_GAP);
        let rise = target.y - self.pos.y;
        let vy = (2.0 * GRAVITY * (rise.max(0.0) + ORANGUTAN_LEAP_APEX)).sqrt();
        let time = (vy + (vy * vy - 2.0 * GRAVITY * rise).max(0.0).sqrt()) / GRAVITY;
        self.leap_vel = vec3((target.x - self.pos.x) / time, 0.0, (target.z - self.pos.z) / time);
        self.vel.y = vy;
        self.on_ground = false;
        self.action = BossAction::Leap;
        self.action_timer = ORANGUTAN_LEAP_MAX_SECONDS;
        self.leap_age = 0.0;
    }

    /// The Orangutan's special attacks. In its charge, within `ORANGUTAN_LEAP_MAX_RANGE` it crouches, leaps and lands beside the player,
    /// smashing the ground as it comes down (`BossEvent::OrangutanSmash`); between `ORANGUTAN_ROCK_MIN_RANGE` and `MAX_RANGE` it lifts a rock and hurls it (`BossEvent::RockThrow`).
    /// Either holds it still for its wind-up; below half health the cooldowns are shorter.
    fn update_orangutan_attacks(&mut self, flat_gap: f32, towards: f32, dt: f32, threat: Vec3) {
        let quick = if self.boss_enraged() { ORANGUTAN_ENRAGED_FACTOR } else { 1.0 };
        match self.action {
            BossAction::None => {
                if self.mode == Mode::Charge && self.on_ground {
                    if flat_gap <= ORANGUTAN_LEAP_MAX_RANGE && self.special_timer <= 0.0 {
                        self.action = BossAction::Smash;
                        self.action_timer = ORANGUTAN_LEAP_WINDUP;
                    } else if flat_gap >= ORANGUTAN_ROCK_MIN_RANGE && flat_gap <= ORANGUTAN_ROCK_MAX_RANGE && self.ranged_timer <= 0.0 {
                        self.action = BossAction::RockThrow;
                        self.action_timer = ORANGUTAN_ROCK_WINDUP;
                    }
                }
            }
            // The crouch before the leap.
            BossAction::Smash => {
                self.action_timer -= dt;
                if self.action_timer <= 0.0 {
                    self.begin_leap(threat);
                }
            }
            BossAction::Leap => {
                self.action_timer -= dt;
                self.leap_age += dt;
                // Down again (or it never got off the ground, or has been up too long): it smashes the ground where it is.
                if (self.on_ground && self.leap_age > 0.2) || self.action_timer <= 0.0 {
                    self.events.push(BossEvent::OrangutanSmash { at: self.pos });
                    self.special_timer = ORANGUTAN_SMASH_COOLDOWN * quick;
                    self.attack_cooldown = self.attack_cooldown.max(0.9);
                    self.action = BossAction::None;
                    self.vel.x = 0.0;
                    self.vel.z = 0.0;
                }
            }
            BossAction::RockThrow | BossAction::IceThrow => {
                self.action_timer -= dt;
                if self.action_timer <= 0.0 {
                    let from = self.pos + vec3(self.heading.cos() * 0.9, 2.4, self.heading.sin() * 0.9);
                    self.events.push(BossEvent::RockThrow { from });
                    self.ranged_timer = ORANGUTAN_ROCK_COOLDOWN * quick;
                    self.action = BossAction::None;
                }
            }
        }
        if self.action != BossAction::None && flat_gap > 1e-3 {
            self.heading = towards;
        }
    }

    /// The direction from Nessie towards the open middle of her lake: the heading, of twelve round the compass, whose point
    /// ten blocks off lies farthest from any shore.
    fn open_water_heading(&self, world: &World) -> f32 {
        let home = self.heading_home();
        let mut best = (self.heading, f32::MIN);
        for i in 0..12 {
            let h = i as f32 * std::f32::consts::TAU / 12.0;
            let (x, z) = (self.pos.x + h.cos() * 10.0, self.pos.z + h.sin() * 10.0);
            // Where the open water is equally wide, the way towards where she came up (the middle of her lake) is the better.
            let off = (h - home).sin().atan2((h - home).cos()).abs();
            let c = shore_clearance(world, x, z, 24.0) - 0.5 * off;
            if c > best.1 {
                best = (h, c);
            }
        }
        best.0
    }

    /// A spirit archer keeps company with the player: it drifts round them at `SPIRIT_FOLLOW_RADIUS`, `SPIRIT_FOLLOW_HEIGHT` up, at its own
    /// angle (so that several do not crowd one spot), passing through anything in its way, and joins them at once if left far behind.
    fn follow_player(&mut self, player: Vec3, dt: f32) {
        self.circle += dt * 0.6;
        let around = self.circle + self.orbit;
        let goal = player + vec3(around.cos() * SPIRIT_FOLLOW_RADIUS, SPIRIT_FOLLOW_HEIGHT + 0.2 * (self.phase * 0.5).sin(), around.sin() * SPIRIT_FOLLOW_RADIUS);
        let gap = goal - self.pos;
        if gap.length() > SPIRIT_TELEPORT_DISTANCE {
            self.pos = goal;
        } else {
            let step = gap.normalize_or_zero() * (gap.length() * 4.0).min(SPIRIT_SPEED) * dt;
            self.pos += step.clamp_length_max(gap.length());
        }
        self.vel = gap;
        self.phase += dt * 2.0;
        self.on_ground = false;
        let to_player = player - self.pos;
        if to_player.x.abs() + to_player.z.abs() > 1e-3 {
            self.heading = to_player.z.atan2(to_player.x);
        }
    }

    /// The highest feet-height this creature may fly to over the column (x, z) without its head going into a ceiling:
    /// the lowest solid block within `FLIER_CEILING_LOOKUP` above its body, less its height and `FLIER_CEILING_CLEARANCE`. `None` where
    /// the air is open. A column that is solid at the height of the body (a wall) is no ceiling, so it gives `None`.
    fn ceiling_limit(&self, world: &World, x: f32, z: f32) -> Option<f32> {
        let (half, height) = (self.kind.half_width(), self.kind.height());
        let start = self.pos.y + height;
        for k in 0..=(FLIER_CEILING_LOOKUP as i32) {
            let y = start + k as f32;
            if world.box_blocked(vec3(x - half, y, z - half), vec3(x + half, y + 0.01, z + half)) {
                if (x, z) != (self.pos.x, self.pos.z) && k == 0 {
                    return None;
                }
                return Some(y.floor() - height - FLIER_CEILING_CLEARANCE);
            }
        }
        None
    }

    /// The lowest height a flier is made to aim for under a ceiling: not lower than a block over the floor below it.
    fn floor_hint(&self, world: &World) -> f32 {
        (world.ground_top(self.pos.x.floor() as i32, self.pos.z.floor() as i32) + 1) as f32 + 0.5
    }

    /// A walking boss whose body overlaps solid blocks (it was set down too close to a wall, or the ground was made again under it) cannot
    /// move on any axis; it is moved the shortest way to a place where its box is clear, sideways first and then up.
    fn free_itself(&mut self, world: &World) {
        if !self.kind.is_boss() || self.kind.flies_always() || self.kind.is_metal_bird() {
            return;
        }
        let (half, height, eps) = (self.kind.half_width(), self.kind.height(), 1e-3);
        let blocked = |p: Vec3| world.box_blocked(vec3(p.x - half + eps, p.y + eps, p.z - half + eps), vec3(p.x + half - eps, p.y + height - eps, p.z + half - eps));
        if !blocked(self.pos) {
            return;
        }
        for ring in 1..=25 {
            let r = ring as f32 * 0.1;
            for i in 0..16 {
                let a = i as f32 * std::f32::consts::TAU / 16.0;
                for lift in [0.0, 0.5, 1.0] {
                    let p = self.pos + vec3(a.cos() * r, lift, a.sin() * r);
                    if !blocked(p) {
                        self.pos = p;
                        return;
                    }
                }
            }
        }
    }

    /// A flier whose body has ended up inside solid rock (a ceiling that was pressed into) is set down until it is clear.
    fn unwedge(&mut self, world: &World) {
        let (half, height) = (self.kind.half_width(), self.kind.height());
        let eps = 1e-3;
        for _ in 0..12 {
            let p = self.pos;
            if !world.box_blocked(vec3(p.x - half + eps, p.y + eps, p.z - half + eps), vec3(p.x + half - eps, p.y + height - eps, p.z + half - eps)) {
                return;
            }
            self.pos.y -= FLIER_UNWEDGE_STEP;
        }
    }

    /// A creature that flies under its own power: a raven (circling the player at a distance, swooping down for one peck and flying
    /// off again) or the Phoenix (hovering over its volcano, throwing fireballs and swooping now and then).
    fn update_flier(&mut self, world: &World, threat: Vec3, dt: f32, rng: &mut Rng) {
        if self.kind == AnimalKind::SpiritArcher {
            self.follow_player(threat, dt);
            return;
        }
        let flat_gap = vec2(threat.x - self.pos.x, threat.z - self.pos.z).length();
        let away = (self.pos.z - threat.z).atan2(self.pos.x - threat.x);
        self.air_timer -= dt;
        let mut goal;
        let speed;
        if self.kind == AnimalKind::VampireBat {
            // Its one attack is the swoop: circle the player at a distance, dive for one bite, fly off, and come again.
            match self.air_phase {
                AirPhase::Circle => {
                    self.circle += dt * VAMPIRE_BAT_CIRCLE_SPEED / VAMPIRE_BAT_ORBIT_RADIUS;
                    goal = threat + vec3(self.circle.cos() * VAMPIRE_BAT_ORBIT_RADIUS, VAMPIRE_BAT_ORBIT_HEIGHT + 0.6 * (self.phase * 0.7).sin(), self.circle.sin() * VAMPIRE_BAT_ORBIT_RADIUS);
                    speed = VAMPIRE_BAT_CIRCLE_SPEED * 1.6;
                    if self.air_timer <= 0.0 && flat_gap < VAMPIRE_BAT_ORBIT_RADIUS * 2.5 {
                        self.air_phase = AirPhase::Dive;
                        self.air_timer = VAMPIRE_BAT_DIVE_SECONDS;
                    }
                }
                AirPhase::Dive => {
                    goal = threat + vec3(0.0, 1.0, 0.0);
                    speed = VAMPIRE_BAT_DIVE_SPEED;
                    if self.air_timer <= 0.0 {
                        self.air_phase = AirPhase::Flee;
                        self.air_timer = VAMPIRE_BAT_FLEE_SECONDS;
                    }
                }
                AirPhase::Flee => {
                    goal = threat + vec3(away.cos() * VAMPIRE_BAT_ORBIT_RADIUS * 1.3, VAMPIRE_BAT_ORBIT_HEIGHT + 1.0, away.sin() * VAMPIRE_BAT_ORBIT_RADIUS * 1.3);
                    speed = VAMPIRE_BAT_CIRCLE_SPEED * 2.0;
                    if self.air_timer <= 0.0 {
                        self.air_phase = AirPhase::Circle;
                        self.air_timer = VAMPIRE_BAT_ATTACK_MIN + (VAMPIRE_BAT_ATTACK_MAX - VAMPIRE_BAT_ATTACK_MIN) * rng.unit();
                        self.circle = away;
                    }
                }
            }
        } else if self.kind == AnimalKind::FlyingSnake {
            // It drifts slowly through the air in wide loops: a big circle round the player if one is near (round its home otherwise),
            // with a smaller circle riding on it, which makes the loops. It swoops down on the player, and loops up and away again.
            let near = flat_gap < SNAKE_NOTICE_DISTANCE;
            let centre = if near { threat } else { self.home };
            self.circle += dt * SNAKE_LOOP_RATE;
            let swirl = |t: f32| {
                let radius = SNAKE_SPIRAL_RADIUS * (1.0 + 0.30 * (t * 0.37).sin());
                let height = SNAKE_SPIRAL_HEIGHT + 3.5 * (t * 1.9).sin();
                let coil = t * (SNAKE_COIL_RATE / SNAKE_LOOP_RATE);
                vec3(t.cos() * radius + coil.cos() * SNAKE_COIL_RADIUS, height + 2.0 * coil.sin(), t.sin() * radius + coil.sin() * SNAKE_COIL_RADIUS)
            };
            match self.air_phase {
                AirPhase::Circle => {
                    goal = centre + swirl(self.circle);
                    speed = SNAKE_SPIRAL_SPEED * 1.8;
                    if self.air_timer <= 0.0 && near {
                        self.air_phase = AirPhase::Dive;
                        self.air_timer = SNAKE_DIVE_SECONDS;
                    }
                }
                AirPhase::Dive => {
                    let side = vec3(-(away.sin()), 0.0, away.cos());
                    goal = threat + vec3(0.0, 1.0, 0.0) + side * (self.circle * 9.0).sin() * 1.2;
                    speed = SNAKE_DIVE_SPEED;
                    if self.air_timer <= 0.0 {
                        self.air_phase = AirPhase::Flee;
                        self.air_timer = SNAKE_FLEE_SECONDS;
                    }
                }
                AirPhase::Flee => {
                    goal = self.pos + vec3(away.cos() * 6.0, SNAKE_SPIRAL_HEIGHT, away.sin() * 6.0) + swirl(self.circle) * 0.5;
                    speed = SNAKE_SPIRAL_SPEED * 2.0;
                    if self.air_timer <= 0.0 {
                        self.air_phase = AirPhase::Circle;
                        self.air_timer = SNAKE_ATTACK_MIN + (SNAKE_ATTACK_MAX - SNAKE_ATTACK_MIN) * rng.unit();
                    }
                }
            }
        } else if self.kind == AnimalKind::Raven {
            match self.air_phase {
                AirPhase::Circle => {
                    self.circle += dt * RAVEN_CIRCLE_SPEED / RAVEN_ORBIT_RADIUS;
                    goal = threat + vec3(self.circle.cos() * RAVEN_ORBIT_RADIUS, RAVEN_ORBIT_HEIGHT, self.circle.sin() * RAVEN_ORBIT_RADIUS);
                    speed = RAVEN_CIRCLE_SPEED * 1.6;
                    if self.air_timer <= 0.0 && flat_gap < RAVEN_ORBIT_RADIUS * 2.0 {
                        self.air_phase = AirPhase::Dive;
                        self.air_timer = RAVEN_DIVE_SECONDS;
                    }
                }
                AirPhase::Dive => {
                    goal = threat + vec3(0.0, 1.1, 0.0);
                    speed = RAVEN_DIVE_SPEED;
                    if self.air_timer <= 0.0 {
                        self.air_phase = AirPhase::Flee;
                        self.air_timer = RAVEN_FLEE_SECONDS;
                    }
                }
                AirPhase::Flee => {
                    goal = threat + vec3(away.cos() * RAVEN_ORBIT_RADIUS * 1.4, RAVEN_ORBIT_HEIGHT, away.sin() * RAVEN_ORBIT_RADIUS * 1.4);
                    speed = RAVEN_FLEE_SPEED;
                    if self.air_timer <= 0.0 {
                        self.air_phase = AirPhase::Circle;
                        self.air_timer = RAVEN_ATTACK_MIN + (RAVEN_ATTACK_MAX - RAVEN_ATTACK_MIN) * rng.unit();
                        self.circle = away;
                    }
                }
            }
        } else {
            let quick = if self.boss_enraged() { 0.7 } else { 1.0 };
            let home_gap = vec2(self.pos.x - self.home.x, self.pos.z - self.home.z).length();
            let hover_y = (threat.y + PHOENIX_HOVER_HEIGHT).max(self.home.y + 8.0);
            if home_gap > PHOENIX_LEASH {
                // Too far from its volcano: it goes home.
                self.air_phase = AirPhase::Circle;
                goal = self.home + vec3(0.0, 10.0, 0.0);
                speed = PHOENIX_CIRCLE_SPEED * 1.4;
            } else {
                match self.air_phase {
                    AirPhase::Circle => {
                        self.circle += dt * PHOENIX_CIRCLE_SPEED / PHOENIX_HOVER_RADIUS;
                        goal = vec3(threat.x + self.circle.cos() * PHOENIX_HOVER_RADIUS, hover_y, threat.z + self.circle.sin() * PHOENIX_HOVER_RADIUS);
                        speed = PHOENIX_CIRCLE_SPEED * 1.5;
                        if self.ranged_timer <= 0.0 && flat_gap <= PHOENIX_FIREBALL_RANGE && (self.pos - threat).length() > 5.0 {
                            self.events.push(BossEvent::Fireball { from: self.pos + vec3(self.heading.cos() * 1.2, 1.0, self.heading.sin() * 1.2) });
                            self.ranged_timer = PHOENIX_FIREBALL_INTERVAL * quick;
                        }
                        if self.air_timer <= 0.0 && flat_gap <= PHOENIX_FIREBALL_RANGE {
                            self.air_phase = AirPhase::Dive;
                            self.air_timer = PHOENIX_SWOOP_SECONDS;
                        }
                    }
                    AirPhase::Dive => {
                        goal = threat + vec3(0.0, 1.0, 0.0);
                        speed = PHOENIX_SWOOP_SPEED;
                        if self.air_timer <= 0.0 {
                            self.air_phase = AirPhase::Flee;
                            self.air_timer = 1.5;
                        }
                    }
                    AirPhase::Flee => {
                        goal = vec3(self.pos.x + away.cos() * 8.0, hover_y + 4.0, self.pos.z + away.sin() * 8.0);
                        speed = PHOENIX_CIRCLE_SPEED * 1.4;
                        if self.air_timer <= 0.0 {
                            self.air_phase = AirPhase::Circle;
                            self.air_timer = PHOENIX_SWOOP_INTERVAL * quick;
                            self.circle = away;
                        }
                    }
                }
            }
        }
        // Something in the way (a tree, the rim of the crater) sends the flier up and over it, if there is room above; under a
        // ceiling (the cave) it slides sideways instead, and it never aims into the rock overhead.
        if self.stuck > 0.0 {
            let room = self.ceiling_limit(world, self.pos.x, self.pos.z).map_or(8.0, |top| (top - goal.y).clamp(0.0, 8.0));
            if room >= 2.0 {
                goal.y += room;
            } else {
                let side = vec3(-self.heading.sin(), 0.0, self.heading.cos());
                goal += side * 6.0 * self.strafe;
                self.circle += 0.05;
            }
        }
        for (x, z) in [(self.pos.x, self.pos.z), (goal.x, goal.z)] {
            if let Some(top) = self.ceiling_limit(world, x, z) {
                goal.y = goal.y.min(top.max(self.floor_hint(world)));
            }
        }
        self.unwedge(world);
        let to = goal - self.pos;
        let dist = to.length();
        let v = to.normalize_or_zero() * speed.min(dist * 3.0 + 1.0);
        self.vel = v + vec3(self.knock.x, 0.0, self.knock.z);
        self.knock *= (1.0 - self.knock_decay * dt).max(0.0);
        let before = self.pos;
        let contact = step_body(world, &mut self.pos, &mut self.vel, self.kind.half_width(), self.kind.height(), dt);
        self.on_ground = false;
        if contact.hit_wall && self.stuck <= 0.0 {
            self.strafe = -self.strafe;
        }
        self.stuck = if contact.hit_wall { 0.6 } else { (self.stuck - dt).max(0.0) };
        let moved = self.pos - before;
        self.phase += moved.length();
        if moved.x.abs() + moved.z.abs() > 1e-4 {
            self.heading = moved.z.atan2(moved.x);
        } else if flat_gap > 1e-3 {
            self.heading = (threat.z - self.pos.z).atan2(threat.x - self.pos.x);
        }
        if self.kind == AnimalKind::FlyingSnake {
            self.lay_trail();
        }
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
        let blows = (BEAR_HEALTH + AXE_DAMAGE - 1) / AXE_DAMAGE;
        for _ in 0..blows - 1 {
            assert!(!bear.damage(AXE_DAMAGE));
        }
        assert!(bear.damage(AXE_DAMAGE), "dead on blow {blows}");
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
