//! Item kinds, item stacks, and what blocks drop when broken.

use macroquad::prelude::Color;

use crate::block::Block;
use crate::config::*;
use crate::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Stick,
    Pebble,
    Stone,
    Log,
    StoneAxe,
    Grass,
    Berries,
    PurpleBerries,
    Twine,
    BowDrill,
    Charcoal,
    /// Made from a log; builds walls and ceilings.
    Plank,
    /// Dropped by a quail when it flies away; fletches arrows.
    Feather,
    /// Shoots arrows (left click). Made of twine and a stick.
    Bow,
    Arrow,
    /// Raw meat. It has to be cooked on a burning log before it can be eaten.
    Venison,
    Poultry,
    CookedVenison,
    CookedPoultry,
    Leather,
    /// Raw bear meat, cooked like the other meats.
    BearMeat,
    CookedBearMeat,
    /// The thick hide of a bear.
    BearPelt,
    /// Pumps air into embers (left click) so they glow hot enough to smelt. Twine, leather, sticks.
    Bellows,
    /// What burnt-out charcoal leaves; it can be broken and picked up.
    Ash,
    /// Smelted from a stone on burning charcoal.
    Copper,
    /// Smelted from a pebble on burning charcoal.
    Tin,
    /// Smelted from iron ore in a lit clay forge worked with bellows.
    Iron,
    Gold,
    /// Dug from the river bed.
    Clay,
    /// Broken out of a boulder with a bronze mallet.
    IronOre,
    GoldOre,
    /// Cut from a piece of leather (three to a piece).
    LeatherCord,
    /// One tin and one copper smelted together in a lit clay forge.
    Bronze,
    /// Two bronze, a leather cord and a stick. Breaks boulders and hurts animals.
    BronzeMallet,
    /// Clay shaped into a forge: put down, filled with charcoal and lit, it smelts bronze and iron.
    ClayForge,
    /// Dropped by a rabbit. Cooked like the other meats.
    RabbitMeat,
    CookedRabbit,
    /// Dropped by a moose.
    MooseMeat,
    CookedMoose,
    /// A rabbit sometimes leaves one. It has no use.
    RabbitFoot,
    /// A rabbit-foot bonus drop. Held in the hand it shows the world's one rainbow by day, which ends in a pile of
    /// gold, and by night the world's one grand castle. A ghostly trail always leads to whichever of them is out there.
    PeepStone,
    /// A rabbit-foot bonus drop. It lures will-o-whisps and can call storm lightning when held.
    HollyRod,
    /// Axes of metal: each hits harder than the one before (stone, copper, bronze, iron).
    CopperAxe,
    BronzeAxe,
    IronAxe,
    /// Tin shaped into a pot, set on a burning log or charcoal to make stew.
    TinPot,
    /// Clay shaped into a jar. Right click water with it to fill it.
    ClayJar,
    WaterJar,
    /// A pot of water and raw meat, cooked. Restores twice as much life as the cooked meat.
    VenisonStew,
    PoultryStew,
    BearStew,
    RabbitStew,
    MooseStew,
    RedBerryStew,
    PurpleBerryStew,
    /// A strip of leather that slings pebbles (left click). Made of one leather.
    Sling,
    /// A small dark lodestone found lying on the ground. With an iron ore it makes a compass.
    Loadstone,
    /// A loadstone and an iron ore. Held in the hand its needle points north, until the world has its one mountain
    /// (following it ten chunks north brings the mountain out); from then on it always points at the mountain.
    Compass,
    /// Dropped by a moose. With two gold it makes a bone totem.
    Antlers,
    /// Antlers and two gold. Held in the hand it shows a ghostly trail: by day to the world's one well, by night to its
    /// one graveyard.
    BoneTotem,
    /// A bear pelt made into a cloak. Worn, it keeps bears from attacking unless they have been hit first.
    PeltCloak,
    /// The armour of the throne room. Worn, it halves all the damage the player takes.
    RoyalArmor,
    /// Found on a stand in the mines under the mountain. Used on the locked door of a silver tower, it opens it for good
    /// (the key is kept). A ghost in the graveyard gives one too.
    SilverKey,
    /// Broken out of the pink salt crystals that stud the walls of the mines. It has no use yet.
    PinkSalt,
    /// The relic at the top of the silver tower on the lake's island. It does nothing yet.
    PhilosophersStone,
    /// The relic at the top of the silver tower in the Horizen. It does nothing yet.
    AncientCrown,
    /// The relic at the top of the silver tower in the Skylands. It does nothing yet.
    HornOfTheHunt,
    /// Sometimes left by a deer (one time in five). Three bones make musical pipes.
    DeerBone,
    /// Pipes cut from deer bones. They do nothing yet.
    MusicalPipes,
    /// Tin shaped into a pan. Held while breaking river clay it sometimes turns up gold.
    TinPan,
    /// The sword from the stone in the ruined castle. The best weapon in the game.
    Excalibur,
    /// Leather and leather cord sewn into a pack. Worn in the one worn place (right click it, or the Worn slot of the inventory), it gives
    /// two more rows of inventory slots, for as long as it is worn.
    LeatherBackpack,
}

impl ItemKind {
    pub const ALL: [ItemKind; 73] = [
        ItemKind::Stick,
        ItemKind::Pebble,
        ItemKind::Stone,
        ItemKind::Log,
        ItemKind::StoneAxe,
        ItemKind::Grass,
        ItemKind::Berries,
        ItemKind::PurpleBerries,
        ItemKind::Twine,
        ItemKind::BowDrill,
        ItemKind::Charcoal,
        ItemKind::Plank,
        ItemKind::Feather,
        ItemKind::Bow,
        ItemKind::Arrow,
        ItemKind::Venison,
        ItemKind::Poultry,
        ItemKind::CookedVenison,
        ItemKind::CookedPoultry,
        ItemKind::Leather,
        ItemKind::BearMeat,
        ItemKind::CookedBearMeat,
        ItemKind::BearPelt,
        ItemKind::Bellows,
        ItemKind::Ash,
        ItemKind::Copper,
        ItemKind::Tin,
        ItemKind::Iron,
        ItemKind::Gold,
        ItemKind::Clay,
        ItemKind::IronOre,
        ItemKind::GoldOre,
        ItemKind::LeatherCord,
        ItemKind::Bronze,
        ItemKind::BronzeMallet,
        ItemKind::ClayForge,
        ItemKind::RabbitMeat,
        ItemKind::CookedRabbit,
        ItemKind::MooseMeat,
        ItemKind::CookedMoose,
        ItemKind::RabbitFoot,
        ItemKind::PeepStone,
        ItemKind::HollyRod,
        ItemKind::CopperAxe,
        ItemKind::BronzeAxe,
        ItemKind::IronAxe,
        ItemKind::TinPot,
        ItemKind::ClayJar,
        ItemKind::WaterJar,
        ItemKind::VenisonStew,
        ItemKind::PoultryStew,
        ItemKind::BearStew,
        ItemKind::RabbitStew,
        ItemKind::MooseStew,
        ItemKind::RedBerryStew,
        ItemKind::PurpleBerryStew,
        ItemKind::Sling,
        ItemKind::Loadstone,
        ItemKind::Compass,
        ItemKind::Antlers,
        ItemKind::BoneTotem,
        ItemKind::PeltCloak,
        ItemKind::RoyalArmor,
        ItemKind::SilverKey,
        ItemKind::PinkSalt,
        ItemKind::PhilosophersStone,
        ItemKind::AncientCrown,
        ItemKind::HornOfTheHunt,
        ItemKind::DeerBone,
        ItemKind::MusicalPipes,
        ItemKind::TinPan,
        ItemKind::Excalibur,
        ItemKind::LeatherBackpack,
    ];

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Stick => "Stick",
            ItemKind::Pebble => "Pebble",
            ItemKind::Stone => "Stone",
            ItemKind::Log => "Log",
            ItemKind::StoneAxe => "Stone Axe",
            ItemKind::Grass => "Grass",
            ItemKind::Berries => "Berries",
            ItemKind::PurpleBerries => "Purple Berries",
            ItemKind::Twine => "Twine",
            ItemKind::BowDrill => "Bow Drill",
            ItemKind::Charcoal => "Charcoal",
            ItemKind::Plank => "Wooden Plank",
            ItemKind::Feather => "Feather",
            ItemKind::Bow => "Bow",
            ItemKind::Arrow => "Arrow",
            ItemKind::Venison => "Venison",
            ItemKind::Poultry => "Poultry",
            ItemKind::CookedVenison => "Cooked Venison",
            ItemKind::CookedPoultry => "Cooked Poultry",
            ItemKind::Leather => "Leather",
            ItemKind::BearMeat => "Bear Meat",
            ItemKind::CookedBearMeat => "Cooked Bear Meat",
            ItemKind::BearPelt => "Bear Pelt",
            ItemKind::Bellows => "Bellows",
            ItemKind::Ash => "Ash",
            ItemKind::Copper => "Copper",
            ItemKind::Tin => "Tin",
            ItemKind::Iron => "Iron",
            ItemKind::Gold => "Gold",
            ItemKind::Clay => "Clay",
            ItemKind::IronOre => "Iron Ore",
            ItemKind::GoldOre => "Gold Ore",
            ItemKind::LeatherCord => "Leather Cord",
            ItemKind::Bronze => "Bronze",
            ItemKind::BronzeMallet => "Bronze Mallet",
            ItemKind::ClayForge => "Clay Forge",
            ItemKind::RabbitMeat => "Rabbit Meat",
            ItemKind::CookedRabbit => "Cooked Rabbit",
            ItemKind::MooseMeat => "Moose Meat",
            ItemKind::CookedMoose => "Cooked Moose",
            ItemKind::RabbitFoot => "Rabbit's Foot",
            ItemKind::PeepStone => "Peep Stone",
            ItemKind::HollyRod => "Holly Rod",
            ItemKind::CopperAxe => "Copper Axe",
            ItemKind::BronzeAxe => "Bronze Axe",
            ItemKind::IronAxe => "Iron Axe",
            ItemKind::TinPot => "Tin Pot",
            ItemKind::ClayJar => "Clay Jar",
            ItemKind::WaterJar => "Jar of Water",
            ItemKind::VenisonStew => "Venison Stew",
            ItemKind::PoultryStew => "Poultry Stew",
            ItemKind::BearStew => "Bear Stew",
            ItemKind::RabbitStew => "Rabbit Stew",
            ItemKind::MooseStew => "Moose Stew",
            ItemKind::RedBerryStew => "Red Berry Stew",
            ItemKind::PurpleBerryStew => "Purple Berry Stew",
            ItemKind::Sling => "Sling",
            ItemKind::Loadstone => "Loadstone",
            ItemKind::Compass => "Compass",
            ItemKind::Antlers => "Antlers",
            ItemKind::BoneTotem => "Bone Totem",
            ItemKind::PeltCloak => "Pelt Cloak",
            ItemKind::RoyalArmor => "Royal Armor",
            ItemKind::SilverKey => "Silver Key",
            ItemKind::PinkSalt => "Pink Salt",
            ItemKind::PhilosophersStone => "Philosopher's Stone",
            ItemKind::AncientCrown => "King's Crown",
            ItemKind::HornOfTheHunt => "Horn of the Hunt",
            ItemKind::DeerBone => "Deer Bone",
            ItemKind::MusicalPipes => "Musical Pipes",
            ItemKind::TinPan => "Tin Pan",
            ItemKind::Excalibur => "Excalibur",
            ItemKind::LeatherBackpack => "Leather Backpack",
        }
    }

    /// Stable identifier used in save files.
    pub fn id(self) -> &'static str {
        match self {
            ItemKind::Stick => "stick",
            ItemKind::Pebble => "pebble",
            ItemKind::Stone => "stone",
            ItemKind::Log => "log",
            ItemKind::StoneAxe => "stone_axe",
            ItemKind::Grass => "grass",
            ItemKind::Berries => "berries",
            ItemKind::PurpleBerries => "purple_berries",
            ItemKind::Twine => "twine",
            ItemKind::BowDrill => "bow_drill",
            ItemKind::Charcoal => "charcoal",
            ItemKind::Plank => "plank",
            ItemKind::Feather => "feather",
            ItemKind::Bow => "bow",
            ItemKind::Arrow => "arrow",
            ItemKind::Venison => "venison",
            ItemKind::Poultry => "poultry",
            ItemKind::CookedVenison => "cooked_venison",
            ItemKind::CookedPoultry => "cooked_poultry",
            ItemKind::Leather => "leather",
            ItemKind::BearMeat => "bear_meat",
            ItemKind::CookedBearMeat => "cooked_bear_meat",
            ItemKind::BearPelt => "bear_pelt",
            ItemKind::Bellows => "bellows",
            ItemKind::Ash => "ash",
            ItemKind::Copper => "copper",
            ItemKind::Tin => "tin",
            ItemKind::Iron => "iron",
            ItemKind::Gold => "gold",
            ItemKind::Clay => "clay",
            ItemKind::IronOre => "iron_ore",
            ItemKind::GoldOre => "gold_ore",
            ItemKind::LeatherCord => "leather_cord",
            ItemKind::Bronze => "bronze",
            ItemKind::BronzeMallet => "bronze_mallet",
            ItemKind::ClayForge => "clay_forge",
            ItemKind::RabbitMeat => "rabbit_meat",
            ItemKind::CookedRabbit => "cooked_rabbit",
            ItemKind::MooseMeat => "moose_meat",
            ItemKind::CookedMoose => "cooked_moose",
            ItemKind::RabbitFoot => "rabbit_foot",
            ItemKind::PeepStone => "peep_stone",
            ItemKind::HollyRod => "holly_rod",
            ItemKind::CopperAxe => "copper_axe",
            ItemKind::BronzeAxe => "bronze_axe",
            ItemKind::IronAxe => "iron_axe",
            ItemKind::TinPot => "tin_pot",
            ItemKind::ClayJar => "clay_jar",
            ItemKind::WaterJar => "water_jar",
            ItemKind::VenisonStew => "venison_stew",
            ItemKind::PoultryStew => "poultry_stew",
            ItemKind::BearStew => "bear_stew",
            ItemKind::RabbitStew => "rabbit_stew",
            ItemKind::MooseStew => "moose_stew",
            ItemKind::RedBerryStew => "red_berry_stew",
            ItemKind::PurpleBerryStew => "purple_berry_stew",
            ItemKind::Sling => "sling",
            ItemKind::Loadstone => "loadstone",
            ItemKind::Compass => "compass",
            ItemKind::Antlers => "antlers",
            ItemKind::BoneTotem => "bone_totem",
            ItemKind::PeltCloak => "pelt_cloak",
            ItemKind::RoyalArmor => "royal_armor",
            ItemKind::SilverKey => "silver_key",
            ItemKind::PinkSalt => "pink_salt",
            ItemKind::PhilosophersStone => "philosophers_stone",
            ItemKind::AncientCrown => "ancient_crown",
            ItemKind::HornOfTheHunt => "horn_of_the_hunt",
            ItemKind::DeerBone => "deer_bone",
            ItemKind::MusicalPipes => "musical_pipes",
            ItemKind::TinPan => "tin_pan",
            ItemKind::Excalibur => "excalibur",
            ItemKind::LeatherBackpack => "leather_backpack",
        }
    }

    /// The item with this save id. The veined stones of older saves come back as the plain stone and pebble
    /// that smelt into the same metal now.
    pub fn from_id(id: &str) -> Option<ItemKind> {
        match id {
            "copper_ore" => Some(ItemKind::Stone),
            "tin_ore" => Some(ItemKind::Pebble),
            id => ItemKind::ALL.into_iter().find(|k| k.id() == id),
        }
    }

    pub fn color(self) -> Color {
        match self {
            ItemKind::Stick => Color::new(0.55, 0.38, 0.18, 1.0),
            ItemKind::Pebble => Color::new(0.64, 0.64, 0.66, 1.0),
            ItemKind::Stone => Color::new(0.42, 0.42, 0.46, 1.0),
            ItemKind::Log => Color::new(0.40, 0.27, 0.13, 1.0),
            ItemKind::StoneAxe => Color::new(0.50, 0.50, 0.54, 1.0),
            ItemKind::Grass => Color::new(0.36, 0.66, 0.24, 1.0),
            ItemKind::Berries => Color::new(0.72, 0.14, 0.30, 1.0),
            ItemKind::PurpleBerries => Color::new(0.48, 0.12, 0.76, 1.0),
            ItemKind::Twine => Color::new(0.80, 0.70, 0.45, 1.0),
            ItemKind::BowDrill => Color::new(0.52, 0.36, 0.20, 1.0),
            ItemKind::Charcoal => Color::new(0.14, 0.14, 0.15, 1.0),
            ItemKind::Plank => Color::new(0.72, 0.54, 0.31, 1.0),
            ItemKind::Feather => Color::new(0.88, 0.84, 0.76, 1.0),
            ItemKind::Bow => Color::new(0.50, 0.33, 0.16, 1.0),
            ItemKind::Arrow => Color::new(0.62, 0.50, 0.30, 1.0),
            ItemKind::Venison => Color::new(0.66, 0.20, 0.24, 1.0),
            ItemKind::Poultry => Color::new(0.90, 0.64, 0.58, 1.0),
            ItemKind::CookedVenison => Color::new(0.45, 0.25, 0.14, 1.0),
            ItemKind::CookedPoultry => Color::new(0.76, 0.52, 0.24, 1.0),
            ItemKind::Leather => Color::new(0.58, 0.38, 0.22, 1.0),
            ItemKind::BearMeat => Color::new(0.56, 0.16, 0.20, 1.0),
            ItemKind::CookedBearMeat => Color::new(0.38, 0.20, 0.12, 1.0),
            ItemKind::BearPelt => Color::new(0.26, 0.18, 0.12, 1.0),
            ItemKind::Bellows => Color::new(0.55, 0.38, 0.22, 1.0),
            ItemKind::Ash => Color::new(0.66, 0.64, 0.62, 1.0),
            ItemKind::Copper => Color::new(0.80, 0.46, 0.25, 1.0),
            ItemKind::Tin => Color::new(0.72, 0.76, 0.82, 1.0),
            ItemKind::Iron => Color::new(0.40, 0.42, 0.47, 1.0),
            ItemKind::Gold => Color::new(0.92, 0.68, 0.18, 1.0),
            ItemKind::Clay => Color::new(0.68, 0.54, 0.45, 1.0),
            ItemKind::IronOre => Color::new(0.62, 0.24, 0.14, 1.0),
            ItemKind::GoldOre => Color::new(0.73, 0.52, 0.13, 1.0),
            ItemKind::LeatherCord => Color::new(0.48, 0.30, 0.16, 1.0),
            ItemKind::Bronze => Color::new(0.74, 0.55, 0.22, 1.0),
            ItemKind::BronzeMallet => Color::new(0.70, 0.52, 0.24, 1.0),
            ItemKind::ClayForge => Color::new(0.68, 0.54, 0.45, 1.0),
            ItemKind::RabbitMeat => Color::new(0.80, 0.35, 0.35, 1.0),
            ItemKind::CookedRabbit => Color::new(0.65, 0.42, 0.22, 1.0),
            ItemKind::MooseMeat => Color::new(0.60, 0.14, 0.18, 1.0),
            ItemKind::CookedMoose => Color::new(0.40, 0.22, 0.12, 1.0),
            ItemKind::RabbitFoot => Color::new(0.82, 0.76, 0.66, 1.0),
            ItemKind::PeepStone => Color::new(0.34, 0.62, 0.78, 1.0),
            ItemKind::HollyRod => Color::new(0.24, 0.56, 0.20, 1.0),
            ItemKind::CopperAxe => Color::new(0.80, 0.46, 0.25, 1.0),
            ItemKind::BronzeAxe => Color::new(0.74, 0.55, 0.22, 1.0),
            ItemKind::IronAxe => Color::new(0.34, 0.36, 0.42, 1.0),
            ItemKind::TinPot => Color::new(0.72, 0.76, 0.82, 1.0),
            ItemKind::ClayJar => Color::new(0.68, 0.54, 0.45, 1.0),
            ItemKind::WaterJar => Color::new(0.55, 0.62, 0.78, 1.0),
            ItemKind::VenisonStew => Color::new(0.55, 0.30, 0.16, 1.0),
            ItemKind::PoultryStew => Color::new(0.80, 0.62, 0.30, 1.0),
            ItemKind::BearStew => Color::new(0.42, 0.22, 0.14, 1.0),
            ItemKind::RabbitStew => Color::new(0.62, 0.42, 0.26, 1.0),
            ItemKind::MooseStew => Color::new(0.48, 0.30, 0.20, 1.0),
            ItemKind::RedBerryStew => Color::new(0.72, 0.14, 0.30, 1.0),
            ItemKind::PurpleBerryStew => Color::new(0.48, 0.12, 0.76, 1.0),
            ItemKind::Sling => Color::new(0.58, 0.38, 0.22, 1.0),
            ItemKind::Loadstone => Color::new(0.20, 0.21, 0.26, 1.0),
            ItemKind::Compass => Color::new(0.72, 0.55, 0.24, 1.0),
            ItemKind::Antlers => Color::new(0.84, 0.78, 0.64, 1.0),
            ItemKind::BoneTotem => Color::new(0.90, 0.88, 0.80, 1.0),
            ItemKind::PeltCloak => Color::new(0.32, 0.22, 0.15, 1.0),
            ItemKind::RoyalArmor => Color::new(0.66, 0.68, 0.74, 1.0),
            ItemKind::SilverKey => Color::new(0.82, 0.84, 0.92, 1.0),
            ItemKind::PinkSalt => Color::new(0.95, 0.64, 0.70, 1.0),
            ItemKind::PhilosophersStone => Color::new(0.76, 0.12, 0.16, 1.0),
            ItemKind::AncientCrown => Color::new(0.90, 0.68, 0.16, 1.0),
            ItemKind::HornOfTheHunt => Color::new(0.90, 0.84, 0.68, 1.0),
            ItemKind::DeerBone => Color::new(0.93, 0.90, 0.82, 1.0),
            ItemKind::MusicalPipes => Color::new(0.88, 0.82, 0.68, 1.0),
            ItemKind::TinPan => Color::new(0.72, 0.76, 0.82, 1.0),
            ItemKind::Excalibur => Color::new(0.86, 0.90, 0.98, 1.0),
            ItemKind::LeatherBackpack => Color::new(0.50, 0.32, 0.18, 1.0),
        }
    }

    /// The block this item becomes when placed on the ground, if it can be placed.
    pub fn placeable_block(self) -> Option<Block> {
        match self {
            ItemKind::Log => Some(Block::PlacedLog),
            ItemKind::Pebble => Some(Block::PlacedPebble),
            ItemKind::Stone => Some(Block::PlacedStone),
            ItemKind::Plank => Some(Block::Planks),
            ItemKind::Charcoal => Some(Block::Charcoal),
            ItemKind::ClayForge => Some(Block::ClayForge),
            _ => None,
        }
    }

    /// For raw meat, what it becomes once cooked on a fire.
    pub fn cooked(self) -> Option<ItemKind> {
        match self {
            ItemKind::Venison => Some(ItemKind::CookedVenison),
            ItemKind::Poultry => Some(ItemKind::CookedPoultry),
            ItemKind::BearMeat => Some(ItemKind::CookedBearMeat),
            ItemKind::RabbitMeat => Some(ItemKind::CookedRabbit),
            ItemKind::MooseMeat => Some(ItemKind::CookedMoose),
            _ => None,
        }
    }

    /// For raw meat, the stew it makes in a pot of water on a fire.
    pub fn stewed(self) -> Option<ItemKind> {
        match self {
            ItemKind::Venison => Some(ItemKind::VenisonStew),
            ItemKind::Poultry => Some(ItemKind::PoultryStew),
            ItemKind::BearMeat => Some(ItemKind::BearStew),
            ItemKind::RabbitMeat => Some(ItemKind::RabbitStew),
            ItemKind::MooseMeat => Some(ItemKind::MooseStew),
            ItemKind::Berries => Some(ItemKind::RedBerryStew),
            ItemKind::PurpleBerries => Some(ItemKind::PurpleBerryStew),
            _ => None,
        }
    }

    /// For a stew, the cooked meat it is made of.
    pub fn stew_base(self) -> Option<ItemKind> {
        match self {
            ItemKind::VenisonStew => Some(ItemKind::CookedVenison),
            ItemKind::PoultryStew => Some(ItemKind::CookedPoultry),
            ItemKind::BearStew => Some(ItemKind::CookedBearMeat),
            ItemKind::RabbitStew => Some(ItemKind::CookedRabbit),
            ItemKind::MooseStew => Some(ItemKind::CookedMoose),
            _ => None,
        }
    }

    /// Whether this is an axe: it chops trees and hurts animals.
    pub fn is_axe(self) -> bool {
        matches!(self, ItemKind::StoneAxe | ItemKind::CopperAxe | ItemKind::BronzeAxe | ItemKind::IronAxe)
    }

    /// Whether this can be worn. A player wears one thing at a time, all in the same place: a pelt cloak, the royal armour,
    /// the king's crown or a leather backpack.
    pub fn is_wearable(self) -> bool {
        matches!(self, ItemKind::PeltCloak | ItemKind::RoyalArmor | ItemKind::AncientCrown | ItemKind::LeatherBackpack)
    }

    /// Whether this is a backpack: it shares the one worn place with the cloak, the armour and the crown, and while it is on
    /// the inventory has its extra rows.
    pub fn is_backpack(self) -> bool {
        self == ItemKind::LeatherBackpack
    }

    /// Whether this shoots something when the left button is clicked: a bow (arrows) or a sling (pebbles).
    pub fn is_launcher(self) -> bool {
        matches!(self, ItemKind::Bow | ItemKind::Sling)
    }

    /// What a launcher shoots, taken from the inventory (`None` for anything that is not a launcher).
    pub fn ammo(self) -> Option<ItemKind> {
        match self {
            ItemKind::Bow => Some(ItemKind::Arrow),
            ItemKind::Sling => Some(ItemKind::Pebble),
            _ => None,
        }
    }

    /// Whether this can be added to a burning log or embers to extend the fire by `FIRE_SECONDS`.
    pub fn is_fuel(self) -> bool {
        matches!(self, ItemKind::Log | ItemKind::Charcoal)
    }

    /// The metal this item smelts into: a stone makes copper and a pebble tin (on burning charcoal), tin
    /// (with a copper beside it) makes bronze and iron ore makes iron (both in a lit clay forge).
    pub fn smelted(self) -> Option<ItemKind> {
        match self {
            ItemKind::Stone => Some(ItemKind::Copper),
            ItemKind::Pebble => Some(ItemKind::Tin),
            ItemKind::Tin => Some(ItemKind::Bronze),
            ItemKind::IronOre => Some(ItemKind::Iron),
            ItemKind::GoldOre => Some(ItemKind::Gold),
            _ => None,
        }
    }

    /// What this item turns into when left on a fire: cooked meat, or smelted metal.
    pub fn processed(self) -> Option<ItemKind> {
        self.cooked().or(self.smelted())
    }

    /// The kind of fire this item must lie on to make progress (`None` if it does not cook or smelt).
    pub fn hearth(self) -> Option<Hearth> {
        if self.cooked().is_some() {
            return Some(Hearth::AnyFire);
        }
        match self {
            ItemKind::Stone | ItemKind::Pebble => Some(Hearth::Embers),
            ItemKind::Tin => Some(Hearth::Forge),
            ItemKind::IronOre | ItemKind::GoldOre => Some(Hearth::GlowingForge),
            _ => None,
        }
    }

    /// Whether the fire must glow brighter (a forge worked with bellows) for this item to make progress.
    pub fn needs_glow(self) -> bool {
        self.hearth() == Some(Hearth::GlowingForge)
    }

    /// Seconds this item needs on a fire to cook or smelt.
    pub fn heat_seconds(self) -> f32 {
        if self.smelted().is_some() { SMELT_SECONDS } else { COOK_SECONDS }
    }

    /// Damage of one swing when this is held, for the items that hurt animals up close.
    pub fn melee_damage(self) -> Option<i32> {
        match self {
            ItemKind::StoneAxe => Some(AXE_DAMAGE),
            ItemKind::CopperAxe => Some(COPPER_AXE_DAMAGE),
            ItemKind::BronzeAxe => Some(BRONZE_AXE_DAMAGE),
            ItemKind::IronAxe => Some(IRON_AXE_DAMAGE),
            ItemKind::BronzeMallet => Some(MALLET_DAMAGE),
            ItemKind::Excalibur => Some(EXCALIBUR_DAMAGE),
            _ => None,
        }
    }

    /// Seconds between swings of a melee item.
    pub fn melee_cooldown(self) -> f32 {
        match self {
            ItemKind::BronzeMallet => MALLET_COOLDOWN,
            ItemKind::Excalibur => EXCALIBUR_COOLDOWN,
            _ => AXE_COOLDOWN,
        }
    }

    /// How far a swing of this melee item reaches.
    pub fn melee_reach(self) -> f32 {
        if self == ItemKind::Excalibur { EXCALIBUR_REACH } else { AXE_REACH }
    }

    /// Life points restored by eating this item, if it can be eaten. Raw meat cannot.
    pub fn heal_amount(self) -> Option<u32> {
        match self {
            ItemKind::Berries => Some(BERRY_HEAL),
            ItemKind::RedBerryStew => Some(BERRY_STEW_HEAL),
            ItemKind::CookedVenison => Some(VENISON_HEAL),
            ItemKind::CookedPoultry => Some(POULTRY_HEAL),
            ItemKind::CookedBearMeat => Some(BEAR_MEAT_HEAL),
            ItemKind::CookedRabbit => Some(RABBIT_MEAT_HEAL),
            ItemKind::CookedMoose => Some(MOOSE_MEAT_HEAL),
            stew => stew.stew_base().and_then(ItemKind::heal_amount).map(|h| h * STEW_HEAL_FACTOR),
        }
    }
}

/// Where something has to lie to cook or smelt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hearth {
    /// Any burning log or charcoal (meat).
    AnyFire,
    /// Burning charcoal in the open (stones and pebbles).
    Embers,
    /// A lit clay forge (bronze).
    Forge,
    /// A lit clay forge that bellows keep glowing (iron).
    GlowingForge,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ItemStack {
    pub kind: ItemKind,
    pub count: u32,
}

/// Roll the items dropped by breaking `block`.
///
/// - Leaves and pine needles: one 50% chance of a stick.
/// - Wood: always one log (the caller checks that the player holds an axe).
/// - Grass: a `GRASS_DROP_CHANCE` percent chance of one grass. Flowers drop nothing.
/// - Bush: five independent 50% chances of a berry (0 to 5 berries).
/// - Placed logs, pebbles, stones, charcoal and planks drop themselves back, always one; so do the
///   pebbles and stones lying under water. Clay drops `CLAY_DROP` lumps.
/// - A boulder drops a stone, a pebble and 1 to 3 iron ore. A clay forge drops `FORGE_CLAY_DROP` clay (and the
///   charcoal in it, if it was filled but not lit). The rainbow's pile of gold drops `RAINBOW_GOLD_DROP` gold.
#[cfg(test)]
pub fn drops_for(block: Block, rng: &mut Rng) -> Vec<ItemKind> {
    drops_for_with_berries(block, ItemKind::Berries, rng)
}

/// Roll block drops and the extra discoveries enabled by a rabbit foot in the hotbar.
pub fn drops_for_with_bonuses(
    block: Block,
    berry_kind: ItemKind,
    rabbit_foot: bool,
    rng: &mut Rng,
) -> Vec<ItemKind> {
    let mut drops = drops_for_with_berries(block, berry_kind, rng);
    if !rabbit_foot {
        return drops;
    }

    match block {
        Block::Leaves | Block::Needles if rng.chance(RABBIT_FOOT_HOLLY_ROD_CHANCE) => {
            drops.push(ItemKind::HollyRod);
        }
        Block::PlacedPebble | Block::WetPebble if rng.chance(RABBIT_FOOT_PEEP_STONE_CHANCE) => {
            drops.push(ItemKind::PeepStone);
        }
        _ => {}
    }
    drops
}

/// Roll drops using the berry colour currently visible on a bush.
pub fn drops_for_with_berries(block: Block, berry_kind: ItemKind, rng: &mut Rng) -> Vec<ItemKind> {
    let mut drops = Vec::new();
    match block {
        Block::Leaves | Block::Needles => {
            if rng.chance(LEAF_STICK_CHANCE) {
                drops.push(ItemKind::Stick);
            }
        }
        Block::Wood | Block::PlacedLog => drops.push(ItemKind::Log),
        Block::PlacedPebble | Block::WetPebble => drops.push(ItemKind::Pebble),
        Block::PlacedStone | Block::WetStone => drops.push(ItemKind::Stone),
        Block::WetClay => drops.extend(std::iter::repeat(ItemKind::Clay).take(CLAY_DROP as usize)),
        Block::Boulder => {
            drops.push(ItemKind::Stone);
            drops.push(ItemKind::Pebble);
            for _ in 0..rng.range(BOULDER_IRON_MIN, BOULDER_IRON_MAX + 1) {
                drops.push(ItemKind::IronOre);
            }
        }
        Block::GoldPile => drops.extend(std::iter::repeat(ItemKind::Gold).take(RAINBOW_GOLD_DROP as usize)),
        Block::ArmorStand => drops.push(ItemKind::RoyalArmor),
        Block::PinkSalt => drops.extend(std::iter::repeat(ItemKind::PinkSalt).take(PINK_SALT_DROP as usize)),
        Block::KeyStand => drops.push(ItemKind::SilverKey),
        Block::GoldBoulder => drops.extend(std::iter::repeat(ItemKind::Gold).take(GOLD_BOULDER_GOLD_DROP as usize)),
        Block::AltarLoadstone => drops.push(ItemKind::Loadstone),
        Block::StonePlinth => drops.push(ItemKind::PhilosophersStone),
        Block::HornPlinth => drops.push(ItemKind::HornOfTheHunt),
        Block::CrownPlinth => drops.push(ItemKind::AncientCrown),
        Block::ClayForge => drops.extend(std::iter::repeat(ItemKind::Clay).take(FORGE_CLAY_DROP as usize)),
        Block::ForgeCharged => {
            drops.extend(std::iter::repeat(ItemKind::Clay).take(FORGE_CLAY_DROP as usize));
            drops.push(ItemKind::Charcoal);
        }
        Block::Charcoal => drops.push(ItemKind::Charcoal),
        Block::Planks => drops.push(ItemKind::Plank),
        Block::Grass => {
            if rng.chance(GRASS_DROP_CHANCE) {
                drops.push(ItemKind::Grass);
            }
        }
        Block::Bush => {
            for _ in 0..BUSH_BERRY_ROLLS {
                if rng.chance(BUSH_BERRY_CHANCE) {
                    drops.push(berry_kind);
                }
            }
        }
        Block::Ash => drops.push(ItemKind::Ash),
        Block::Air
        | Block::Dirt
        | Block::Stone
        | Block::Snow
        | Block::CastleStone
        | Block::CastleDoor
        | Block::Gravestone
        | Block::BridgePlank
        | Block::BridgePost
        | Block::SilverStone
        | Block::TowerDoor
        | Block::TowerDoorTop
        | Block::AncientAltar
        | Block::SwordInStone
        | Block::SwordHilt
        | Block::PulledStone
        | Block::MayaStone
        | Block::Water
        | Block::Flower
        | Block::Fire
        | Block::Embers
        | Block::ForgeLit => {}
    }
    drops
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIALS: usize = 40_000;

    fn rate(hits: usize) -> f64 {
        hits as f64 / TRIALS as f64
    }

    #[test]
    fn leaves_drop_a_stick_half_the_time_and_nothing_else() {
        let mut rng = Rng::new(11);
        let mut sticks = 0;
        for block in [Block::Leaves, Block::Needles] {
            for _ in 0..TRIALS {
                let d = drops_for(block, &mut rng);
                assert!(d.len() <= 1 && d.iter().all(|&k| k == ItemKind::Stick));
                sticks += d.len();
            }
        }
        let r = sticks as f64 / (2 * TRIALS) as f64;
        assert!((r - 0.50).abs() < 0.01, "stick rate {r}");
    }

    #[test]
    fn rabbit_foot_adds_peep_stones_and_holly_rods_one_time_in_five() {
        let mut rng = Rng::new(12);
        let mut peep_stones = 0;
        let mut holly_rods = 0;
        for _ in 0..TRIALS {
            let pebble = drops_for_with_bonuses(Block::PlacedPebble, ItemKind::Berries, true, &mut rng);
            peep_stones += pebble.iter().filter(|&&kind| kind == ItemKind::PeepStone).count();
            let leaves = drops_for_with_bonuses(Block::Leaves, ItemKind::Berries, true, &mut rng);
            holly_rods += leaves.iter().filter(|&&kind| kind == ItemKind::HollyRod).count();
        }
        assert!((rate(peep_stones) - 0.20).abs() < 0.015, "peep stone rate {}", rate(peep_stones));
        assert!((rate(holly_rods) - 0.20).abs() < 0.015, "holly rod rate {}", rate(holly_rods));
        assert!(drops_for_with_bonuses(Block::PlacedPebble, ItemKind::Berries, false, &mut rng).iter().all(|&kind| kind != ItemKind::PeepStone));
        assert!(drops_for_with_bonuses(Block::Leaves, ItemKind::Berries, false, &mut rng).iter().all(|&kind| kind != ItemKind::HollyRod));
    }

    #[test]
    fn unbreakable_air_flower_and_fire_blocks_drop_nothing() {
        let mut rng = Rng::new(13);
        for block in [Block::Air, Block::Dirt, Block::Stone, Block::Snow, Block::CastleStone, Block::Water, Block::Flower, Block::Fire] {
            for _ in 0..1000 {
                assert!(drops_for(block, &mut rng).is_empty());
            }
        }
    }

    #[test]
    fn wood_always_drops_exactly_one_log() {
        let mut rng = Rng::new(14);
        for _ in 0..1000 {
            assert_eq!(drops_for(Block::Wood, &mut rng), vec![ItemKind::Log]);
        }
    }

    #[test]
    fn drops_are_deterministic_for_a_seed() {
        let a: Vec<_> = (0..50).map(|_| drops_for(Block::Bush, &mut Rng::new(5))).collect();
        assert!(a.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn ids_round_trip() {
        for k in ItemKind::ALL {
            assert_eq!(ItemKind::from_id(k.id()), Some(k));
        }
        assert_eq!(ItemKind::from_id("diamond"), None);
    }

    #[test]
    fn grass_drops_one_grass_most_of_the_time() {
        let mut rng = Rng::new(15);
        let mut hits = 0;
        for _ in 0..TRIALS {
            let d = drops_for(Block::Grass, &mut rng);
            assert!(d.len() <= 1 && d.iter().all(|&k| k == ItemKind::Grass));
            hits += d.len();
        }
        assert!((rate(hits) - GRASS_DROP_CHANCE as f64 / 100.0).abs() < 0.012, "grass rate {}", rate(hits));
    }

    #[test]
    fn bushes_roll_five_berry_chances_at_fifty_percent() {
        let mut rng = Rng::new(16);
        let mut counts = [0usize; 6];
        for _ in 0..TRIALS {
            let d = drops_for(Block::Bush, &mut rng);
            assert!(d.len() <= 5 && d.iter().all(|&k| k == ItemKind::Berries));
            counts[d.len()] += 1;
        }
        // Binomial(5, 0.5): 1, 5, 10, 10, 5, 1 out of 32.
        for (n, ways) in [1.0, 5.0, 10.0, 10.0, 5.0, 1.0].iter().enumerate() {
            let expected = ways / 32.0;
            assert!((rate(counts[n]) - expected).abs() < 0.012, "P({n} berries) = {}", rate(counts[n]));
        }
    }

    #[test]
    fn placed_blocks_drop_exactly_themselves_back() {
        let mut rng = Rng::new(17);
        for (block, item) in [
            (Block::PlacedLog, ItemKind::Log),
            (Block::PlacedPebble, ItemKind::Pebble),
            (Block::PlacedStone, ItemKind::Stone),
            (Block::Charcoal, ItemKind::Charcoal),
            (Block::Planks, ItemKind::Plank),
            (Block::Ash, ItemKind::Ash),
        ] {
            for _ in 0..100 {
                assert_eq!(drops_for(block, &mut rng), vec![item]);
            }
        }
    }

    #[test]
    fn logs_pebbles_and_stones_are_placeable_and_round_trip() {
        for (item, block) in [
            (ItemKind::Log, Block::PlacedLog),
            (ItemKind::Pebble, Block::PlacedPebble),
            (ItemKind::Stone, Block::PlacedStone),
            (ItemKind::Plank, Block::Planks),
            (ItemKind::Charcoal, Block::Charcoal),
        ] {
            assert_eq!(item.placeable_block(), Some(block));
            assert_eq!(drops_for(block, &mut Rng::new(1)), vec![item]);
        }
        for item in [ItemKind::Stick, ItemKind::StoneAxe, ItemKind::Twine, ItemKind::BowDrill, ItemKind::Ash] {
            assert_eq!(item.placeable_block(), None, "{item:?}");
        }
        for item in [
            ItemKind::Bow,
            ItemKind::Arrow,
            ItemKind::Feather,
            ItemKind::Venison,
            ItemKind::Leather,
            ItemKind::BearMeat,
            ItemKind::CookedBearMeat,
            ItemKind::BearPelt,
            ItemKind::Bellows,
            ItemKind::Copper,
            ItemKind::Tin,
            ItemKind::Iron,
            ItemKind::Clay,
            ItemKind::IronOre,
            ItemKind::LeatherCord,
            ItemKind::Bronze,
            ItemKind::BronzeMallet,
        ] {
            assert_eq!(item.placeable_block(), None, "{item:?}");
        }
        assert_eq!(ItemKind::ClayForge.placeable_block(), Some(Block::ClayForge));
    }

    #[test]
    fn only_cooked_meat_and_berries_can_be_eaten() {
        assert_eq!(ItemKind::Berries.heal_amount(), Some(BERRY_HEAL));
        assert_eq!(ItemKind::CookedVenison.heal_amount(), Some(VENISON_HEAL));
        assert_eq!(ItemKind::CookedPoultry.heal_amount(), Some(POULTRY_HEAL));
        assert_eq!(ItemKind::CookedBearMeat.heal_amount(), Some(BEAR_MEAT_HEAL));
        for k in [ItemKind::Venison, ItemKind::Poultry, ItemKind::BearMeat, ItemKind::Leather, ItemKind::BearPelt, ItemKind::Feather, ItemKind::Stick] {
            assert_eq!(k.heal_amount(), None, "{k:?}");
        }
    }

    #[test]
    fn stones_pebbles_tin_and_iron_ore_smelt_each_in_their_own_place() {
        assert_eq!(ItemKind::Stone.smelted(), Some(ItemKind::Copper));
        assert_eq!(ItemKind::Pebble.smelted(), Some(ItemKind::Tin));
        assert_eq!(ItemKind::Tin.smelted(), Some(ItemKind::Bronze));
        assert_eq!(ItemKind::IronOre.smelted(), Some(ItemKind::Iron));
        assert_eq!(ItemKind::GoldOre.smelted(), Some(ItemKind::Gold));
        for k in ItemKind::ALL {
            let smelts = matches!(k, ItemKind::Stone | ItemKind::Pebble | ItemKind::Tin | ItemKind::IronOre | ItemKind::GoldOre);
            assert_eq!(k.smelted().is_some(), smelts, "{k:?}");
            assert_eq!(k.needs_glow(), matches!(k, ItemKind::IronOre | ItemKind::GoldOre), "only deep ores need the bellows: {k:?}");
        }
        assert_eq!(ItemKind::Stone.hearth(), Some(Hearth::Embers));
        assert_eq!(ItemKind::Pebble.hearth(), Some(Hearth::Embers));
        assert_eq!(ItemKind::Tin.hearth(), Some(Hearth::Forge));
        assert_eq!(ItemKind::IronOre.hearth(), Some(Hearth::GlowingForge));
        assert_eq!(ItemKind::GoldOre.hearth(), Some(Hearth::GlowingForge));
        assert_eq!(ItemKind::Venison.hearth(), Some(Hearth::AnyFire));
        assert_eq!(ItemKind::Copper.hearth(), None, "copper is only ever smelted along with a tin");
        assert_eq!(ItemKind::Venison.processed(), Some(ItemKind::CookedVenison));
        assert!(!ItemKind::Venison.needs_glow(), "meat cooks on any fire");
        assert_eq!((ItemKind::Venison.heat_seconds(), ItemKind::Pebble.heat_seconds()), (COOK_SECONDS, SMELT_SECONDS));
        for k in [ItemKind::Copper, ItemKind::Iron, ItemKind::Bronze, ItemKind::Clay, ItemKind::Ash, ItemKind::Charcoal, ItemKind::Log] {
            assert_eq!(k.processed(), None, "{k:?}");
            assert_eq!(k.hearth(), None, "{k:?}");
            assert_eq!(k.heal_amount(), None, "{k:?}");
        }
    }

    #[test]
    fn the_axe_and_the_bronze_mallet_are_the_melee_items() {
        assert_eq!(ItemKind::StoneAxe.melee_damage(), Some(AXE_DAMAGE));
        assert_eq!(ItemKind::BronzeMallet.melee_damage(), Some(MALLET_DAMAGE));
        assert_eq!((ItemKind::StoneAxe.melee_cooldown(), ItemKind::BronzeMallet.melee_cooldown()), (AXE_COOLDOWN, MALLET_COOLDOWN));
        for k in ItemKind::ALL {
            assert_eq!(k.melee_damage().is_some(), k.is_axe() || matches!(k, ItemKind::BronzeMallet | ItemKind::Excalibur), "{k:?}");
        }
    }

    #[test]
    fn old_veined_stones_in_a_save_come_back_as_the_stone_and_pebble_that_smelt_into_the_same_metal() {
        assert_eq!(ItemKind::from_id("copper_ore"), Some(ItemKind::Stone));
        assert_eq!(ItemKind::from_id("tin_ore"), Some(ItemKind::Pebble));
        assert_eq!(ItemKind::from_id("iron_ore"), Some(ItemKind::IronOre));
        assert_eq!(ItemKind::Stone.smelted(), Some(ItemKind::Copper));
        assert_eq!(ItemKind::Pebble.smelted(), Some(ItemKind::Tin));
    }

    #[test]
    fn a_boulder_drops_a_stone_a_pebble_and_one_to_three_iron_ore() {
        let mut rng = Rng::new(19);
        let mut seen = [0usize; 4];
        for _ in 0..2000 {
            let d = drops_for(Block::Boulder, &mut rng);
            assert_eq!(&d[..2], &[ItemKind::Stone, ItemKind::Pebble]);
            let iron = d.len() - 2;
            assert!(d[2..].iter().all(|&k| k == ItemKind::IronOre));
            assert!((1..=3).contains(&iron), "{iron}");
            seen[iron] += 1;
        }
        assert!(seen[1] > 400 && seen[2] > 400 && seen[3] > 400, "each count turns up: {seen:?}");
    }

    #[test]
    fn a_clay_forge_gives_back_four_clay_and_no_forge_and_a_lit_one_gives_nothing() {
        let mut rng = Rng::new(20);
        assert_eq!(drops_for(Block::ClayForge, &mut rng), vec![ItemKind::Clay; 4]);
        let mut filled = drops_for(Block::ForgeCharged, &mut rng);
        filled.sort_by_key(|k| k.id());
        assert_eq!(filled, vec![ItemKind::Charcoal, ItemKind::Clay, ItemKind::Clay, ItemKind::Clay, ItemKind::Clay]);
        assert!(drops_for(Block::ForgeLit, &mut rng).is_empty());
    }

    #[test]
    fn things_under_the_water_drop_what_they_are_and_clay_drops_lumps() {
        let mut rng = Rng::new(18);
        assert_eq!(drops_for(Block::WetPebble, &mut rng), vec![ItemKind::Pebble]);
        assert_eq!(drops_for(Block::WetStone, &mut rng), vec![ItemKind::Stone]);
        assert_eq!(drops_for(Block::WetClay, &mut rng), vec![ItemKind::Clay; CLAY_DROP as usize]);
        assert!(CLAY_DROP >= 1);
        assert!(drops_for(Block::Water, &mut rng).is_empty());
    }

    #[test]
    fn raw_meat_cooks_into_its_cooked_form_and_nothing_else_cooks() {
        assert_eq!(ItemKind::Venison.cooked(), Some(ItemKind::CookedVenison));
        assert_eq!(ItemKind::Poultry.cooked(), Some(ItemKind::CookedPoultry));
        assert_eq!(ItemKind::BearMeat.cooked(), Some(ItemKind::CookedBearMeat));
        assert_eq!(ItemKind::RabbitMeat.cooked(), Some(ItemKind::CookedRabbit));
        assert_eq!(ItemKind::MooseMeat.cooked(), Some(ItemKind::CookedMoose));
        for k in ItemKind::ALL {
            if !matches!(k, ItemKind::Venison | ItemKind::Poultry | ItemKind::BearMeat | ItemKind::RabbitMeat | ItemKind::MooseMeat) {
                assert_eq!(k.cooked(), None, "{k:?}");
            }
        }
    }

    #[test]
    fn rabbit_and_moose_meat_cook_and_every_raw_meat_makes_a_stew_that_heals_double() {
        assert_eq!(ItemKind::RabbitMeat.cooked(), Some(ItemKind::CookedRabbit));
        assert_eq!(ItemKind::MooseMeat.cooked(), Some(ItemKind::CookedMoose));
        assert_eq!(ItemKind::RabbitMeat.hearth(), Some(Hearth::AnyFire));
        assert_eq!(ItemKind::CookedRabbit.heal_amount(), Some(RABBIT_MEAT_HEAL));
        assert_eq!(ItemKind::CookedMoose.heal_amount(), Some(MOOSE_MEAT_HEAL));
        assert_eq!(STEW_HEAL_FACTOR, 2);
        let pairs = [
            (ItemKind::Venison, ItemKind::VenisonStew, ItemKind::CookedVenison),
            (ItemKind::Poultry, ItemKind::PoultryStew, ItemKind::CookedPoultry),
            (ItemKind::BearMeat, ItemKind::BearStew, ItemKind::CookedBearMeat),
            (ItemKind::RabbitMeat, ItemKind::RabbitStew, ItemKind::CookedRabbit),
            (ItemKind::MooseMeat, ItemKind::MooseStew, ItemKind::CookedMoose),
        ];
        for (raw, stew, cooked) in pairs {
            assert_eq!(raw.stewed(), Some(stew));
            assert_eq!(stew.stew_base(), Some(cooked));
            assert_eq!(stew.heal_amount(), Some(cooked.heal_amount().unwrap() * 2), "{stew:?}");
            assert_eq!(raw.heal_amount(), None, "raw meat cannot be eaten");
            assert_eq!(stew.stewed(), None);
        }
        for k in ItemKind::ALL {
            assert_eq!(k.stewed().is_some(), pairs.iter().any(|p| p.0 == k) || matches!(k, ItemKind::Berries | ItemKind::PurpleBerries), "{k:?}");
        }
        assert_eq!(ItemKind::Berries.stewed(), Some(ItemKind::RedBerryStew));
        assert_eq!(ItemKind::PurpleBerries.stewed(), Some(ItemKind::PurpleBerryStew));
        assert_eq!(ItemKind::RedBerryStew.heal_amount(), Some(BERRY_STEW_HEAL));
        assert_eq!(ItemKind::PurpleBerryStew.heal_amount(), None);
        for k in [ItemKind::TinPot, ItemKind::ClayJar, ItemKind::WaterJar, ItemKind::RabbitFoot] {
            assert_eq!((k.heal_amount(), k.placeable_block(), k.melee_damage()), (None, None, None), "{k:?}");
        }
    }

    #[test]
    fn each_axe_hits_harder_than_the_one_before_and_all_chop() {
        let axes = [ItemKind::StoneAxe, ItemKind::CopperAxe, ItemKind::BronzeAxe, ItemKind::IronAxe];
        let damage = [AXE_DAMAGE, COPPER_AXE_DAMAGE, BRONZE_AXE_DAMAGE, IRON_AXE_DAMAGE];
        assert!(damage.windows(2).all(|w| w[0] < w[1]), "every kind of axe deals a different amount");
        for (i, axe) in axes.into_iter().enumerate() {
            assert_eq!(axe.melee_damage(), Some(damage[i]), "{axe:?}");
            assert!(axe.is_axe() && Block::Wood.can_break_with(Some(axe)), "{axe:?}");
        }
        assert!(!ItemKind::BronzeMallet.is_axe() && !Block::Wood.can_break_with(Some(ItemKind::BronzeMallet)));
    }

    #[test]
    fn loadstone_is_an_item_and_launchers_have_their_ammunition() {
        assert_eq!(ItemKind::Bow.ammo(), Some(ItemKind::Arrow));
        assert_eq!(ItemKind::Sling.ammo(), Some(ItemKind::Pebble));
        assert!(ItemKind::Bow.is_launcher() && ItemKind::Sling.is_launcher());
        for k in ItemKind::ALL {
            assert_eq!(k.is_launcher(), k.ammo().is_some(), "{k:?}");
        }
        for k in [ItemKind::Sling, ItemKind::Loadstone, ItemKind::Compass] {
            assert!(k.placeable_block().is_none() && k.melee_damage().is_none() && k.heal_amount().is_none(), "{k:?}");
            assert_eq!(ItemKind::from_id(k.id()), Some(k));
        }
    }

    #[test]
    fn every_axe_and_the_mallet_have_a_melee_damage_and_each_axe_hits_harder() {
        let axes = [ItemKind::StoneAxe, ItemKind::CopperAxe, ItemKind::BronzeAxe, ItemKind::IronAxe];
        let damage = [AXE_DAMAGE, COPPER_AXE_DAMAGE, BRONZE_AXE_DAMAGE, IRON_AXE_DAMAGE];
        for (i, axe) in axes.iter().enumerate() {
            assert!(axe.is_axe());
            assert_eq!(axe.melee_damage(), Some(damage[i]), "{axe:?}");
        }
        assert_eq!(ItemKind::BronzeMallet.melee_damage(), Some(MALLET_DAMAGE));
        for k in ItemKind::ALL {
            assert_eq!(k.melee_damage().is_some(), k.is_axe() || matches!(k, ItemKind::BronzeMallet | ItemKind::Excalibur), "{k:?}");
        }
    }
}
