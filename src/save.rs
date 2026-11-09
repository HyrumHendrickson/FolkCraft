//! Saving and loading worlds, and seed parsing.

use macroquad::prelude::vec3;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::block::Block;
use crate::config::*;
use crate::dropped::DroppedItem;
use crate::inventory::Inventory;
use crate::item::{ItemKind, ItemStack};
use crate::tower::Tower;
use crate::landmarks::{GrandCastle, Graveyard, Lake, Landmarks, Mountain, Rainbow, RainbowState, Well, MAX_LANDMARK_RADIUS, MOUNTAIN_MAX_HEIGHT};
use crate::player::Player;
use crate::rng::clock_seed;
use crate::world::{Cooking, Pos, Pot, World};

#[derive(Clone)]
pub struct WorldInfo {
    pub name: String,
    pub seed: i64,
    pub path: PathBuf,
}

pub fn sanitize_file_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn world_path(name: &str) -> PathBuf {
    PathBuf::from(SAVE_DIR).join(format!("{}.world", sanitize_file_name(name)))
}

/// Everything stored in a world file besides its name and seed.
pub struct SaveData {
    pub player: Player,
    pub removed: HashSet<Pos>,
    pub placed: HashMap<Pos, Block>,
    pub fires: HashMap<Pos, f32>,
    pub cooking: HashMap<Pos, Cooking>,
    pub glow: HashMap<Pos, f32>,
    pub pots: HashMap<Pos, Pot>,
    pub berry_picked: HashSet<Pos>,
    pub night: bool,
    /// Lakes, mountains and the rainbow.
    pub landmarks: Landmarks,
    pub inventory: Inventory,
    pub items: Vec<DroppedItem>,
    pub health: u32,
    /// Dev mode: the inventory lists every item to take. Chosen when the world is created.
    pub dev_mode: bool,
    /// The one thing the player wears (a pelt cloak or the royal armour).
    pub worn: Option<ItemKind>,
    /// The backpack of a save made when it had a place of its own (read, never written: it is `worn` now).
    pub back: Option<ItemKind>,
    /// Whether the player is in the Horizen. The fields above then describe the overworld, where they fell in.
    pub in_horizen: bool,
    /// The Horizen's own edits, items on the ground and the player's place in it, once the player has been there.
    pub horizen: Option<Box<SaveData>>,
    /// Whether the player is in the Skylands (the fields above then describe the overworld, where they left it).
    pub in_skylands: bool,
    /// The Skylands' own edits, items on the ground and the player's place in it, once the player has been there.
    pub skylands: Option<Box<SaveData>>,
    /// How many quail the player has killed (an Iron Quail comes for every `QUAIL_KILLS_PER_IRON_QUAIL`).
    pub quail_kills: u32,
    /// How many parrots the player has killed (Bronze Parrots come for every `PARROTS_PER_BRONZE_PARROT`).
    pub parrot_kills: u32,
    /// How many bats the player has killed in the Horizen's caves (a vampire bat comes for every `BATS_PER_VAMPIRE`).
    pub bat_kills: u32,
    /// How many day/night switches the player has made (the Dragon comes after `DRAGON_SWITCHES`), and big crawfish killed (the Giant Crawfish
    /// comes for every `BIG_CRAWFISH_KILLS_PER_GIANT`).
    pub day_night_switches: u32,
    pub big_crawfish_kills: u32,
}

impl SaveData {
    /// A brand-new world: default player, nothing removed, empty inventory.
    pub fn new() -> Self {
        SaveData {
            player: Player::new(),
            removed: HashSet::new(),
            placed: HashMap::new(),
            fires: HashMap::new(),
            cooking: HashMap::new(),
            glow: HashMap::new(),
            pots: HashMap::new(),
            berry_picked: HashSet::new(),
            night: false,
            landmarks: Landmarks::default(),
            inventory: Inventory::new(),
            items: Vec::new(),
            health: MAX_HEALTH,
            dev_mode: false,
            quail_kills: 0,
            parrot_kills: 0,
            bat_kills: 0,
            day_night_switches: 0,
            big_crawfish_kills: 0,
            worn: None,
            back: None,
            in_horizen: false,
            horizen: None,
            in_skylands: false,
            skylands: None,
        }
    }
}

/// File format (plain text, one record per line):
///   line 1: world name
///   line 2: seed
///   line 3: "px py pz yaw pitch"
///   then any number of:
///     "x y z"                     a block the player removed
///     "inv <slot> <item> <count>" an inventory stack
///     "item <item> <x> <y> <z>"   an item lying on the ground
///     "place <x> <y> <z> <block>" a block the player placed (log, pebble, stone, fire, charcoal)
///     "fire <x> <y> <z> <secs>"   seconds of burning left in the fire block at that position
///     "cook <x> <y> <z> <item> <secs>"  raw meat, a stone or a pebble on the fire at that position,
///                                 with the seconds of heat still needed (0 once it is done)
///     "glow <x> <y> <z> <secs>"   seconds of bellows glow left on the embers at that position
///     "health <n>"                the player's life points (1..=MAX_HEALTH)
///     "berry <x> <y> <z>"          a bush whose berries have been harvested
///     "time day|night"             the saved time of day
///     "mode dev"                  a dev-mode world (no line: normal mode)
///     "lake <x> <z> <radius> <phase> <from x> <from z>"   a lake and the channel that joins it to its river
///     "mountain <x> <z> <radius> <height> <phase>"        a mountain
///     "rainbow spent"            the rainbow's gold has been taken: there is never another rainbow
///     "rainbow <from x> <from z> <x> <y> <z>"  the rainbow is out there, its gold at (x, y, z)
///     "rainbowend <x> <y> <z>"   where the gold of a spent rainbow lay (the peep stone still leads there)
///     "grandcastle <x> <z>"      the grand castle, centred on that column
///     "well <x> <z>"            the well (its water's corner with the smallest coordinates)
///     "graveyard <x> <z>"       the graveyard, centred on that column
///     "ghostrest"               the graveyard's ghost has heard its song and rests
///     "won"                     the player has beaten the Wild Hunt and won the game
///     "phoenixslain"            the Phoenix of the Skylands is dead (written in the Skylands' records, with the `s` prefix)
///     "phoenixdead <seconds>"   how long it has been dead; it rises again after `PHOENIX_RESPAWN_SECONDS`
///     "worn <item>"             what the player wears: a cloak, the armour, the crown or a backpack (while it is a backpack,
///                               slots 36 to 53 of "inv" belong to the pack)
///     "back <item>"             read only: saves made when the backpack had a place of its own; it is worn now
///     "quailkills <n>"          how many quail the player has killed
///     "parrotkills <n>"         how many parrots the player has killed
///     "batkills <n>"            how many bats the player has killed in the Horizen's caves
///     "switches <n>"            how many times the player has turned day to night or night to day
///     "crawfishkills <n>"       how many big crawfish the player has killed
///     "orangutanslain"          the Orangutan on the jungle temple has been killed (it never returns)
///     "unicornsummoned"         the Cursed Idol has been used: the Unicorn has come (it returns whole when the world is loaded, until killed)
///     "unicornslain"            the Unicorn has been killed (it never returns)
///     "ruinedcastle <x> <z>"    the ruined castle with the sword in the stone, centred on that column
///     "dimension horizen"       the player is in the Horizen (everything above is then the overworld)
///     "dimension skylands"     the player is in the Skylands (everything above is then the overworld)
///     "yetislain"              the Yeti has been killed (a cyclone stands on the summit)
///     "nessie <x> <z>"         the monster of the lake centred there has been killed
///     "unlocked <x> <z>"       the silver tower centred there has had its door unlocked
///     "tower <x> <z>"          the Horizen's silver tower (only in the Horizen's records)
///     "skytower <cx> <cz>"     the Skylands' silver tower stands on the island of that grid cell (Skylands' records)
///     "skyvisit <cx> <cz>"     the Skylands island of that grid cell has been walked on (Skylands' records)
///     "s <record>"              a record of the Skylands, as for \"h\"
///     "h <record>"              a record of the Horizen: any of the above that makes sense there (blocks, fires,
///                                items, \"pos <x> <y> <z> <yaw> <pitch>\" for the player's place in it)
/// Worlds saved before the inventory existed simply have none of the last six, and worlds saved before the
/// landmarks existed have none of the last three. Animals, arrows and treks in progress are not saved.
pub fn serialize_save(
    name: &str,
    seed: i64,
    world: &World,
    player: &Player,
    inventory: &Inventory,
    items: &[DroppedItem],
    health: u32,
    dev_mode: bool,
) -> String {
    serialize_save_full(name, seed, world, player, inventory, items, health, dev_mode, &SaveExtras::default())
}

/// What `serialize_save` leaves out: what is worn, and the Horizen (with the player in it, if they are).
#[derive(Default)]
pub struct SaveExtras<'a> {
    pub worn: Option<ItemKind>,
    pub in_horizen: bool,
    pub horizen: Option<(&'a World, &'a Player, &'a [DroppedItem])>,
    pub in_skylands: bool,
    pub skylands: Option<(&'a World, &'a Player, &'a [DroppedItem])>,
    pub quail_kills: u32,
    /// How many parrots the player has killed (Bronze Parrots come for every `PARROTS_PER_BRONZE_PARROT`).
    pub parrot_kills: u32,
    pub bat_kills: u32,
    pub day_night_switches: u32,
    pub big_crawfish_kills: u32,
}

/// The records of a world's silver tower state, each starting with `prefix`.
fn tower_lines(landmarks: &Landmarks, prefix: &str, out: &mut String) {
    for (x, z) in &landmarks.towers_unlocked {
        out.push_str(&format!("{prefix}unlocked {x} {z}\n"));
    }
    if let Some(t) = landmarks.horizen_tower {
        out.push_str(&format!("{prefix}tower {} {}\n", t.x, t.z));
    }
    if let Some((x, z)) = landmarks.sky_tower {
        out.push_str(&format!("{prefix}skytower {x} {z}\n"));
    }
    for (x, z) in &landmarks.sky_visited {
        out.push_str(&format!("{prefix}skyvisit {x} {z}\n"));
    }
    if landmarks.game_won {
        out.push_str(&format!("{prefix}won\n"));
    }
    if landmarks.phoenix_slain {
        out.push_str(&format!("{prefix}phoenixslain\n"));
        if landmarks.phoenix_dead_for > 0.0 {
            out.push_str(&format!("{prefix}phoenixdead {:.1}\n", landmarks.phoenix_dead_for));
        }
    }
    if landmarks.graveyard_rested {
        out.push_str(&format!("{prefix}ghostrest\n"));
    }
    for (x, z) in &landmarks.caves_visited {
        out.push_str(&format!("{prefix}cavevisit {x} {z}\n"));
    }
}

/// The records that describe a world's edits, each starting with `prefix`.
fn world_lines(world: &World, prefix: &str, out: &mut String) {
    tower_lines(&world.landmarks, prefix, out);
    out.push_str(&format!("{prefix}time {}\n", if world.night { "night" } else { "day" }));
    for (x, y, z) in &world.removed {
        out.push_str(&format!("{prefix}{} {} {}\n", x, y, z));
    }
    for &(x, y, z) in &world.berry_picked {
        out.push_str(&format!("{prefix}berry {} {} {}\n", x, y, z));
    }
    for (&(x, y, z), block) in &world.placed {
        if let Some(id) = block.id().filter(|_| *block != crate::block::Block::StardustPile) {
            out.push_str(&format!("{prefix}place {} {} {} {}\n", x, y, z, id));
        }
    }
    for (&(x, y, z), secs) in &world.fires {
        out.push_str(&format!("{prefix}fire {} {} {} {}\n", x, y, z, secs));
    }
    for (&(x, y, z), secs) in &world.glow {
        out.push_str(&format!("{prefix}glow {} {} {} {}\n", x, y, z, secs));
    }
    for (&(x, y, z), pot) in &world.pots {
        out.push_str(&format!("{prefix}pot {} {} {} {} {} {} {}\n", x, y, z, pot.water as u8, pot.meat.map_or("-", |m| m.id()), pot.left, pot.served));
    }
    for (&(x, y, z), meat) in &world.cooking {
        out.push_str(&format!("{prefix}cook {} {} {} {} {}\n", x, y, z, meat.raw.id(), meat.left));
    }
}

pub fn serialize_save_full(
    name: &str,
    seed: i64,
    world: &World,
    player: &Player,
    inventory: &Inventory,
    items: &[DroppedItem],
    health: u32,
    dev_mode: bool,
    extras: &SaveExtras,
) -> String {
    let mut out = format!(
        "{}\n{}\n{} {} {} {} {}\n",
        name, seed, player.pos.x, player.pos.y, player.pos.z, player.yaw, player.pitch
    );
    for (x, y, z) in &world.removed {
        out.push_str(&format!("{} {} {}\n", x, y, z));
    }
    for &(x, y, z) in &world.berry_picked {
        out.push_str(&format!("berry {} {} {}\n", x, y, z));
    }
    out.push_str(&format!("time {}\n", if world.night { "night" } else { "day" }));
    out.push_str(&format!("health {}\n", health));
    if dev_mode {
        out.push_str("mode dev\n");
    }
    if let Some(worn) = extras.worn {
        out.push_str(&format!("worn {}\n", worn.id()));
    }
    if extras.parrot_kills > 0 {
        out.push_str(&format!("parrotkills {}\n", extras.parrot_kills));
    }
    if extras.quail_kills > 0 {
        out.push_str(&format!("quailkills {}\n", extras.quail_kills));
    }
    if extras.bat_kills > 0 {
        out.push_str(&format!("batkills {}\n", extras.bat_kills));
    }
    if extras.day_night_switches > 0 {
        out.push_str(&format!("switches {}\n", extras.day_night_switches));
    }
    if extras.big_crawfish_kills > 0 {
        out.push_str(&format!("crawfishkills {}\n", extras.big_crawfish_kills));
    }
    if world.landmarks.unicorn_summoned {
        out.push_str("unicornsummoned\n");
    }
    if world.landmarks.unicorn_slain {
        out.push_str("unicornslain\n");
    }
    if world.landmarks.orangutan_slain {
        out.push_str("orangutanslain\n");
    }
    if extras.in_horizen {
        out.push_str("dimension horizen\n");
    }
    if extras.in_skylands {
        out.push_str("dimension skylands\n");
    }
    if world.landmarks.yeti_slain {
        out.push_str("yetislain\n");
    }
    for (x, z) in &world.landmarks.monsters_slain {
        out.push_str(&format!("nessie {x} {z}\n"));
    }
    tower_lines(&world.landmarks, "", &mut out);
    for l in &world.landmarks.lakes {
        out.push_str(&format!("lake {} {} {} {} {} {}\n", l.x, l.z, l.radius, l.phase, l.from.0, l.from.1));
    }
    for m in &world.landmarks.mountains {
        out.push_str(&format!("mountain {} {} {} {} {}\n", m.x, m.z, m.radius, m.height, m.phase));
    }
    match world.landmarks.rainbow {
        RainbowState::Unseen => {}
        RainbowState::Spent => {
            out.push_str("rainbow spent\n");
            if let Some((x, y, z)) = world.landmarks.spent_gold {
                out.push_str(&format!("rainbowend {x} {y} {z}\n"));
            }
        }
        RainbowState::Active(r) => out.push_str(&format!("rainbow {} {} {} {} {}\n", r.from.0, r.from.1, r.gold.0, r.gold.1, r.gold.2)),
    }
    if let Some(c) = world.landmarks.grand_castle {
        out.push_str(&format!("grandcastle {} {}\n", c.x, c.z));
    }
    if let Some(c) = world.landmarks.ruined_castle {
        out.push_str(&format!("ruinedcastle {} {}\n", c.x, c.z));
    }
    if let Some(w) = world.landmarks.well {
        out.push_str(&format!("well {} {}\n", w.x, w.z));
    }
    if let Some(g) = world.landmarks.graveyard {
        out.push_str(&format!("graveyard {} {}\n", g.x, g.z));
    }
    for (slot, stack) in inventory.slots.iter().enumerate() {
        if let Some(s) = stack {
            out.push_str(&format!("inv {} {} {}\n", slot, s.kind.id(), s.count));
        }
    }
    for item in items {
        out.push_str(&format!("item {} {} {} {}\n", item.kind.id(), item.pos.x, item.pos.y, item.pos.z));
    }
    for (&(x, y, z), block) in &world.placed {
        if let Some(id) = block.id().filter(|_| *block != crate::block::Block::StardustPile) {
            out.push_str(&format!("place {} {} {} {}\n", x, y, z, id));
        }
    }
    for (&(x, y, z), secs) in &world.fires {
        out.push_str(&format!("fire {} {} {} {}\n", x, y, z, secs));
    }
    for (&(x, y, z), secs) in &world.glow {
        out.push_str(&format!("glow {} {} {} {}\n", x, y, z, secs));
    }
    for (&(x, y, z), pot) in &world.pots {
        out.push_str(&format!("pot {} {} {} {} {} {} {}\n", x, y, z, pot.water as u8, pot.meat.map_or("-", |m| m.id()), pot.left, pot.served));
    }
    for (&(x, y, z), meat) in &world.cooking {
        out.push_str(&format!("cook {} {} {} {} {}\n", x, y, z, meat.raw.id(), meat.left));
    }
    if let Some((hw, hp, hitems)) = extras.horizen {
        out.push_str(&format!("h pos {} {} {} {} {}\n", hp.pos.x, hp.pos.y, hp.pos.z, hp.yaw, hp.pitch));
        world_lines(hw, "h ", &mut out);
        for item in hitems {
            out.push_str(&format!("h item {} {} {} {}\n", item.kind.id(), item.pos.x, item.pos.y, item.pos.z));
        }
    }
    if let Some((sw, sp, sitems)) = extras.skylands {
        out.push_str(&format!("s pos {} {} {} {} {}\n", sp.pos.x, sp.pos.y, sp.pos.z, sp.yaw, sp.pitch));
        world_lines(sw, "s ", &mut out);
        for item in sitems {
            out.push_str(&format!("s item {} {} {} {}\n", item.kind.id(), item.pos.x, item.pos.y, item.pos.z));
        }
    }
    out
}

/// Parse a world file. Unknown or malformed records are ignored.
pub fn parse_save(text: &str) -> SaveData {
    let mut data = SaveData::new();
    let mut lines = text.lines().skip(2);
    if let Some(line) = lines.next() {
        let v: Vec<f32> = line.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        if v.len() == 5 {
            data.player.pos = vec3(v[0], v[1], v[2]);
            data.player.yaw = v[3];
            data.player.pitch = v[4];
        }
    }
    for line in lines {
        apply_line(&mut data, line);
    }
    data
}

/// Apply one record of a world file to `data`.
fn apply_line(data: &mut SaveData, line: &str) {
        let mut t = line.split_whitespace();
        match t.next() {
            Some("h") => {
                let rest = line.trim_start().strip_prefix('h').unwrap_or("");
                let horizen = data.horizen.get_or_insert_with(|| Box::new(SaveData::new()));
                apply_line(horizen, rest);
            }
            Some("s") => {
                let rest = line.trim_start().strip_prefix('s').unwrap_or("");
                let skylands = data.skylands.get_or_insert_with(|| Box::new(SaveData::new()));
                apply_line(skylands, rest);
            }
            Some("yetislain") => data.landmarks.yeti_slain = true,
            Some("orangutanslain") => data.landmarks.orangutan_slain = true,
            Some("unicornsummoned") => data.landmarks.unicorn_summoned = true,
            Some("unicornslain") => data.landmarks.unicorn_slain = true,
            Some("batkills") => data.bat_kills = t.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("switches") => data.day_night_switches = t.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("crawfishkills") => data.big_crawfish_kills = t.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("nessie") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    if !data.landmarks.monsters_slain.contains(&(x, z)) {
                        data.landmarks.monsters_slain.push((x, z));
                    }
                }
            }
            Some("unlocked") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    if !data.landmarks.towers_unlocked.contains(&(x, z)) {
                        data.landmarks.towers_unlocked.push((x, z));
                    }
                }
            }
            Some("tower") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.horizen_tower = Some(Tower { x, z });
                }
            }
            Some("skytower") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.sky_tower = Some((x, z));
                }
            }
            Some("skyvisit") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.note_sky_visit((x, z));
                }
            }
            Some("ghostrest") => data.landmarks.graveyard_rested = true,
            Some("won") => data.landmarks.game_won = true,
            Some("phoenixslain") => data.landmarks.phoenix_slain = true,
            Some("phoenixdead") => data.landmarks.phoenix_dead_for = t.next().and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0),
            Some("cavevisit") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    if !data.landmarks.caves_visited.contains(&(x, z)) {
                        data.landmarks.caves_visited.push((x, z));
                    }
                }
            }
            Some("pos") => {
                let v: Vec<f32> = t.filter_map(|s| s.parse().ok()).collect();
                if v.len() == 5 {
                    data.player.pos = vec3(v[0], v[1], v[2]);
                    data.player.yaw = v[3];
                    data.player.pitch = v[4];
                }
            }
            Some("parrotkills") => data.parrot_kills = t.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("quailkills") => data.quail_kills = t.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("back") => data.back = t.next().and_then(ItemKind::from_id).filter(|k| k.is_backpack()),
            Some("worn") => data.worn = t.next().and_then(ItemKind::from_id).filter(|k| k.is_wearable()),
            Some("dimension") => {
                let which = t.next();
                data.in_horizen = which == Some("horizen");
                data.in_skylands = which == Some("skylands");
            }
            Some("well") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.well = Some(Well { x, z });
                }
            }
            Some("graveyard") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.graveyard = Some(Graveyard { x, z });
                }
            }
            Some("mode") => data.dev_mode = t.next() == Some("dev"),
            Some("health") => {
                if let Some(h) = t.next().and_then(|s| s.parse::<u32>().ok()) {
                    if (1..=MAX_HEALTH).contains(&h) {
                        data.health = h;
                    }
                }
            }
            Some("lake") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z, radius, phase, fx, fz] = v[..] {
                    if (1..=MAX_LANDMARK_RADIUS).contains(&radius) {
                        data.landmarks.lakes.push(Lake { x, z, radius, phase, from: (fx, fz) });
                    }
                }
            }
            Some("mountain") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z, radius, height, phase] = v[..] {
                    if (1..=MAX_LANDMARK_RADIUS).contains(&radius) && (1..=MOUNTAIN_MAX_HEIGHT).contains(&height) {
                        data.landmarks.mountains.push(Mountain { x, z, radius, height, phase });
                    }
                }
            }
            Some("rainbow") => {
                let rest: Vec<&str> = t.collect();
                let v: Vec<i32> = rest.iter().filter_map(|s| s.parse().ok()).collect();
                if rest == ["spent"] {
                    data.landmarks.rainbow = RainbowState::Spent;
                } else if let ([fx, fz, x, y, z], 5) = (&v[..], rest.len()) {
                    // A world whose gold was taken never gets its rainbow back, whatever order the lines are in.
                    if data.landmarks.rainbow != RainbowState::Spent {
                        data.landmarks.rainbow = RainbowState::Active(Rainbow { from: (*fx, *fz), gold: (*x, *y, *z) });
                    }
                }
            }
            Some("rainbowend") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, y, z] = v[..] {
                    data.landmarks.spent_gold = Some((x, y, z));
                }
            }
            Some("grandcastle") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.grand_castle = Some(GrandCastle { x, z });
                }
            }
            Some("ruinedcastle") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if let [x, z] = v[..] {
                    data.landmarks.ruined_castle = Some(GrandCastle { x, z });
                }
            }
            Some("berry") => {
                let v: Vec<i32> = t.filter_map(|s| s.parse().ok()).collect();
                if v.len() == 3 {
                    data.berry_picked.insert((v[0], v[1], v[2]));
                }
            }
            Some("time") => {
                match t.next() {
                    Some("night") => data.night = true,
                    Some("day") => data.night = false,
                    _ => {}
                }
            }
            Some("inv") => {
                let slot = t.next().and_then(|s| s.parse::<usize>().ok());
                let kind = t.next().and_then(ItemKind::from_id);
                let count = t.next().and_then(|s| s.parse::<u32>().ok());
                if let (Some(slot), Some(kind), Some(count)) = (slot, kind, count) {
                    if slot < INVENTORY_SLOTS + BACKPACK_SLOTS && count > 0 {
                        if data.inventory.slots.len() <= slot {
                            data.inventory.add_pack_rows();
                        }
                        data.inventory.slots[slot] = Some(ItemStack { kind, count: count.min(MAX_STACK) });
                    }
                }
            }
            Some("item") => {
                let kind = t.next().and_then(ItemKind::from_id);
                let v: Vec<f32> = t.filter_map(|s| s.parse().ok()).collect();
                if let (Some(kind), 3) = (kind, v.len()) {
                    data.items.push(DroppedItem::at_rest(kind, vec3(v[0], v[1], v[2])));
                }
            }
            Some("place") => {
                let v: Vec<i32> = (&mut t).take(3).filter_map(|s| s.parse().ok()).collect();
                let block = t.next().and_then(Block::from_id);
                if let (3, Some(block)) = (v.len(), block) {
                    data.placed.insert((v[0], v[1], v[2]), block);
                }
            }
            Some("fire") => {
                let v: Vec<i32> = (&mut t).take(3).filter_map(|s| s.parse().ok()).collect();
                let secs = t.next().and_then(|s| s.parse::<f32>().ok());
                if let (3, Some(secs)) = (v.len(), secs) {
                    if secs.is_finite() && secs > 0.0 {
                        data.fires.insert((v[0], v[1], v[2]), secs);
                    }
                }
            }
            Some("cook") => {
                let v: Vec<i32> = (&mut t).take(3).filter_map(|s| s.parse().ok()).collect();
                // Older saves call a smelting stone `copper_ore` and a smelting pebble `tin_ore`; `from_id` maps both
                // to the stone and the pebble that smelt into those metals now.
                let raw = t.next().and_then(ItemKind::from_id).filter(|k| k.processed().is_some());
                let left = t.next().and_then(|s| s.parse::<f32>().ok());
                if let (3, Some(raw), Some(left)) = (v.len(), raw, left) {
                    if left.is_finite() && left >= 0.0 {
                        data.cooking.insert((v[0], v[1], v[2]), Cooking { raw, left: left.min(raw.heat_seconds()) });
                    }
                }
            }
            Some("pot") => {
                let v: Vec<i32> = (&mut t).take(3).filter_map(|s| s.parse().ok()).collect();
                let water = t.next().and_then(|s| match s {
                    "0" => Some(false),
                    "1" => Some(true),
                    _ => None,
                });
                let meat = t.next().and_then(|s| if s == "-" { Some(None) } else { ItemKind::from_id(s).filter(|k| k.stewed().is_some()).map(Some) });
                let left = t.next().and_then(|s| s.parse::<f32>().ok());
                // Servings already taken (older saves have none).
                let served = t.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0).min(STEW_SERVINGS);
                if let (3, Some(water), Some(meat), Some(left)) = (v.len(), water, meat, left) {
                    if left.is_finite() && left >= 0.0 {
                        data.pots.insert((v[0], v[1], v[2]), Pot { water, meat, left: left.min(STEW_SECONDS), served });
                    }
                }
            }
            Some("glow") => {
                let v: Vec<i32> = (&mut t).take(3).filter_map(|s| s.parse().ok()).collect();
                let secs = t.next().and_then(|s| s.parse::<f32>().ok());
                if let (3, Some(secs)) = (v.len(), secs) {
                    if secs.is_finite() && secs > 0.0 {
                        data.glow.insert((v[0], v[1], v[2]), secs.min(BELLOWS_GLOW_SECONDS));
                    }
                }
            }
            Some(first) => {
                let v: Vec<i32> = std::iter::once(first).chain(t).filter_map(|s| s.parse().ok()).collect();
                if v.len() == 3 {
                    data.removed.insert((v[0], v[1], v[2]));
                }
            }
            None => {}
        }
}

pub fn save_world(
    info: &WorldInfo,
    world: &World,
    player: &Player,
    inventory: &Inventory,
    items: &[DroppedItem],
    health: u32,
    dev_mode: bool,
) {
    let out = serialize_save(&info.name, info.seed, world, player, inventory, items, health, dev_mode);
    if let Err(e) = fs::create_dir_all(SAVE_DIR).and_then(|_| fs::write(&info.path, out)) {
        eprintln!("Failed to save world: {e}");
    }
}

pub fn list_worlds() -> Vec<WorldInfo> {
    let mut worlds = Vec::new();
    if let Ok(entries) = fs::read_dir(SAVE_DIR) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("world") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                let mut lines = text.lines();
                if let (Some(name), Some(seed)) = (lines.next(), lines.next()) {
                    if let Ok(seed) = seed.trim().parse::<i64>() {
                        worlds.push(WorldInfo { name: name.to_string(), seed, path });
                    }
                }
            }
        }
    }
    worlds.sort_by_key(|w| w.name.to_lowercase());
    worlds
}

/// Delete a world's save file. Only a `.world` file is ever removed.
pub fn delete_world(info: &WorldInfo) -> std::io::Result<()> {
    if info.path.extension().and_then(|e| e.to_str()) != Some("world") {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a world file"));
    }
    fs::remove_file(&info.path)
}

pub fn parse_seed(text: &str) -> i64 {
    let t = text.trim();
    if t.is_empty() {
        return (clock_seed() >> 1) as i64 % 1_000_000_000;
    }
    if let Ok(n) = t.parse::<i64>() {
        return n;
    }
    // FNV-1a for text seeds.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in t.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    (h >> 1) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_world(tag: &str) -> WorldInfo {
        let dir = std::env::temp_dir().join(format!("folkcraft_test_{}_{}", std::process::id(), tag));
        fs::create_dir_all(&dir).unwrap();
        WorldInfo { name: "Doomed".into(), seed: 5, path: dir.join("Doomed.world") }
    }

    #[test]
    fn deleting_a_world_removes_its_file_and_only_that_file() {
        let info = temp_world("delete");
        let other = info.path.with_file_name("Kept.world");
        for p in [&info.path, &other] {
            fs::write(p, "W\n1\n0 6 0 0 0\n").unwrap();
        }
        assert!(delete_world(&info).is_ok());
        assert!(!info.path.exists() && other.exists());
        assert!(delete_world(&info).is_err(), "already gone");
        fs::remove_dir_all(info.path.parent().unwrap()).unwrap();
    }

    #[test]
    fn only_world_files_can_be_deleted() {
        let info = temp_world("guard");
        let note = info.path.with_file_name("notes.txt");
        fs::write(&note, "keep me").unwrap();
        let wrong = WorldInfo { path: note.clone(), ..info.clone() };
        assert!(delete_world(&wrong).is_err());
        assert!(note.exists());
        fs::remove_dir_all(info.path.parent().unwrap()).unwrap();
    }

    #[test]
    fn save_round_trips_inventory_items_and_edits() {
        let mut removed = HashSet::new();
        removed.insert((3, 5, -7));
        removed.insert((-1, 2, 9));
        let world = World::new(42, removed.clone());
        let mut player = Player::new();
        player.pos = vec3(12.5, 6.0, -3.25);
        player.yaw = 1.5;
        player.pitch = -0.25;
        let mut inv = Inventory::new();
        inv.add(ItemKind::Stick, 7);
        inv.add(ItemKind::Pebble, 70); // spills into a second stack
        inv.slots[20] = Some(ItemStack { kind: ItemKind::Stone, count: 2 });
        let items = vec![
            DroppedItem::at_rest(ItemKind::Stick, vec3(1.5, 6.04, 2.5)),
            DroppedItem::at_rest(ItemKind::Stone, vec3(-4.25, 1.11, 8.75)),
        ];

        let text = serialize_save("My World", 42, &world, &player, &inv, &items, MAX_HEALTH, false);
        let data = parse_save(&text);
        assert_eq!(data.removed, removed);
        assert_eq!(data.player.pos, player.pos);
        assert_eq!((data.player.yaw, data.player.pitch), (1.5, -0.25));
        assert_eq!(data.inventory, inv);
        assert_eq!(data.items.len(), 2);
        assert_eq!((data.items[1].kind, data.items[1].pos), (ItemKind::Stone, vec3(-4.25, 1.11, 8.75)));
        assert!(data.items.iter().all(|i| i.can_pickup(i.pos - vec3(0.0, 0.05, 0.0))));
    }

    #[test]
    fn lakes_mountains_and_the_rainbow_survive_a_save_round_trip() {
        let mut world = World::new(3, HashSet::new());
        world.landmarks.lakes.push(Lake { x: 300, z: -40, radius: 34, phase: 12, from: (10, 11) });
        world.landmarks.mountains.push(Mountain { x: -20, z: -400, radius: 46, height: 17, phase: 5 });
        world.landmarks.rainbow = RainbowState::Active(Rainbow { from: (4, 5), gold: (30, 6, 12) });
        let text = serialize_save("W", 3, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("lake 300 -40 34 12 10 11") && text.contains("mountain -20 -400 46 17 5") && text.contains("rainbow 4 5 30 6 12"));
        assert_eq!(parse_save(&text).landmarks, world.landmarks);
        world.landmarks.rainbow = RainbowState::Spent;
        let text = serialize_save("W", 3, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("rainbow spent"));
        assert_eq!(parse_save(&text).landmarks, world.landmarks);
        assert_eq!(parse_save("W\n1\n0 6 0 0 0\n").landmarks, Landmarks::default(), "older worlds have none");
    }

    #[test]
    fn the_silver_towers_survive_a_save_round_trip_in_each_dimension() {
        let mut world = World::new(1, HashSet::new());
        world.landmarks.towers_unlocked.push((100, -50));
        let mut horizen = World::new_horizen(1);
        horizen.landmarks.horizen_tower = Some(Tower { x: 300, z: -80 });
        horizen.landmarks.towers_unlocked.push((300, -80));
        let mut sky = World::new_skylands(1);
        sky.landmarks.sky_tower = Some((4, -2));
        sky.landmarks.sky_visited = vec![(0, 0), (1, 0), (1, 1)];
        let player = Player::new();
        let extras = SaveExtras { horizen: Some((&horizen, &player, &[])), skylands: Some((&sky, &player, &[])), ..Default::default() };
        let text = serialize_save_full("w", 1, &world, &player, &Inventory::new(), &[], 10, false, &extras);
        let data = parse_save(&text);
        assert_eq!(data.landmarks.towers_unlocked, vec![(100, -50)]);
        assert_eq!(data.landmarks.horizen_tower, None, "the Horizen's tower is not the overworld's");
        assert_eq!(data.horizen.unwrap().landmarks, horizen.landmarks);
        assert_eq!(data.skylands.unwrap().landmarks, sky.landmarks);
    }

    #[test]
    fn the_grand_castle_and_the_end_of_a_spent_rainbow_survive_a_save_round_trip() {
        let mut world = World::new(3, HashSet::new());
        world.landmarks.grand_castle = Some(GrandCastle { x: -150, z: 220 });
        world.landmarks.rainbow = RainbowState::Spent;
        world.landmarks.spent_gold = Some((30, 6, 12));
        let text = serialize_save("W", 3, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("grandcastle -150 220") && text.contains("rainbowend 30 6 12"));
        assert_eq!(parse_save(&text).landmarks, world.landmarks);
        // A world saved before the end was kept is still spent, with nowhere to lead to.
        let old = parse_save("W\n1\n0 6 0 0 0\nrainbow spent\n");
        assert_eq!((old.landmarks.rainbow, old.landmarks.rainbow_end()), (RainbowState::Spent, None));
        // Damaged lines are ignored.
        assert_eq!(parse_save("W\n1\n0 6 0 0 0\ngrandcastle 1\nrainbowend a b c\n").landmarks, Landmarks::default());
    }

    #[test]
    fn a_spent_rainbow_stays_spent_and_damaged_landmark_lines_are_ignored() {
        let text = "W\n1\n0 6 0 0 0\nrainbow spent\nrainbow 1 2 3 4 5\nlake 1 2\nlake 0 0 0 0 0 0\nlake 0 0 9999 0 0 0\nmountain 0 0 40 9999 0\nmountain a b c d e\nrainbow x\n";
        let data = parse_save(text);
        assert_eq!(data.landmarks.rainbow, RainbowState::Spent);
        assert!(data.landmarks.lakes.is_empty() && data.landmarks.mountains.is_empty());
        let late = parse_save("W\n1\n0 6 0 0 0\nrainbow 1 2 3 4 5\nrainbow spent\n");
        assert_eq!(late.landmarks.rainbow, RainbowState::Spent);
    }

    #[test]
    fn logs_and_stone_axes_survive_a_save_round_trip() {
        let world = World::new(1, HashSet::new());
        let mut inv = Inventory::new();
        inv.add(ItemKind::Log, 9);
        inv.slots[3] = Some(ItemStack { kind: ItemKind::StoneAxe, count: 1 });
        let items = vec![DroppedItem::at_rest(ItemKind::Log, vec3(1.5, 6.1, 2.5))];
        let text = serialize_save("W", 1, &world, &Player::new(), &inv, &items, MAX_HEALTH, false);
        assert!(text.contains("inv 3 stone_axe 1") && text.contains("item log "));
        let data = parse_save(&text);
        assert_eq!(data.inventory, inv);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].kind, ItemKind::Log);
    }

    #[test]
    fn placed_blocks_and_fires_survive_a_save_round_trip() {
        let mut world = World::new(9, HashSet::new());
        world.placed.insert((4, 6, 5), Block::PlacedLog);
        world.placed.insert((-3, 6, 8), Block::PlacedPebble);
        world.placed.insert((7, 6, -2), Block::PlacedStone);
        world.placed.insert((1, 6, 1), Block::Fire);
        world.placed.insert((2, 6, 2), Block::Charcoal);
        world.fires.insert((1, 6, 1), 42.5);
        let text = serialize_save("W", 9, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("place 4 6 5 placed_log") && text.contains("fire 1 6 1 42.5"));
        let data = parse_save(&text);
        assert_eq!(data.placed, world.placed);
        assert_eq!(data.fires, world.fires);
        assert!(data.removed.is_empty(), "place and fire lines are not removed blocks");
    }

    #[test]
    fn a_saved_world_rebuilds_its_placed_blocks_and_fire_timers() {
        let mut world = World::new(1, HashSet::new());
        world.ensure_chunk(0, 0);
        world.set_block(10, 6, 10, Block::PlacedLog);
        world.ignite((10, 6, 10), 60.0);
        world.set_block(11, 6, 10, Block::PlacedStone);
        // A tree trunk cut down is a removal of generated terrain that is saved along with the rest.
        let trunk = (0..WORLD_HEIGHT)
            .flat_map(|y| (0..CHUNK_SIZE).flat_map(move |x| (0..CHUNK_SIZE).map(move |z| (x, y, z))))
            .find(|&(x, y, z)| world.block_at(x, y, z) == Block::Wood)
            .expect("a tree");
        world.break_block(trunk.0, trunk.1, trunk.2);
        world.update_fires(20.0);
        let text = serialize_save("W", 1, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        let data = parse_save(&text);
        let mut loaded = World::with_edits(1, data.removed, data.placed, data.fires);
        loaded.ensure_chunk(0, 0);
        assert_eq!(loaded.block_at(10, 6, 10), Block::Fire);
        assert!((loaded.fires[&(10, 6, 10)] - 40.0).abs() < 1e-3);
        assert_eq!(loaded.block_at(11, 6, 10), Block::PlacedStone);
        assert_eq!(loaded.block_at(trunk.0, trunk.1, trunk.2), Block::Air);
    }

    #[test]
    fn planks_and_new_items_survive_a_save_round_trip() {
        let mut world = World::new(9, HashSet::new());
        world.placed.insert((4, 6, 5), Block::Planks);
        let mut inv = Inventory::new();
        for (i, kind) in [
            ItemKind::Plank, ItemKind::Feather, ItemKind::Bow, ItemKind::Arrow, ItemKind::Venison,
            ItemKind::Poultry, ItemKind::CookedVenison, ItemKind::CookedPoultry, ItemKind::Leather,
        ]
        .into_iter()
        .enumerate()
        {
            inv.slots[i] = Some(ItemStack { kind, count: i as u32 + 1 });
        }
        let items = vec![DroppedItem::at_rest(ItemKind::Feather, vec3(1.5, 6.02, 2.5))];
        let text = serialize_save("W", 9, &world, &Player::new(), &inv, &items, MAX_HEALTH, false);
        assert!(text.contains("place 4 6 5 planks") && text.contains("inv 4 venison 5"));
        let data = parse_save(&text);
        assert_eq!(data.placed, world.placed);
        assert_eq!(data.inventory, inv);
        assert_eq!(data.items[0].kind, ItemKind::Feather);
    }

    #[test]
    fn meat_on_a_fire_survives_a_save_round_trip() {
        let mut world = World::new(9, HashSet::new());
        world.placed.insert((1, 6, 1), Block::Fire);
        world.fires.insert((1, 6, 1), 40.0);
        world.cooking.insert((1, 6, 1), Cooking { raw: ItemKind::Poultry, left: 2.5 });
        let text = serialize_save("W", 9, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("cook 1 6 1 poultry 2.5"));
        let data = parse_save(&text);
        assert_eq!(data.cooking, world.cooking);
        assert!(data.removed.is_empty(), "a cook line is not a removed block");
        let loaded = World::with_edits(9, data.removed, data.placed, data.fires).with_cooking(data.cooking);
        assert_eq!(loaded.cooking[&(1, 6, 1)].item(), ItemKind::Poultry);
    }

    #[test]
    fn smelting_pieces_forges_and_glow_survive_a_save_round_trip() {
        let mut world = World::new(9, HashSet::new());
        world.placed.insert((1, 6, 1), Block::ForgeLit);
        world.placed.insert((3, 6, 1), Block::ClayForge);
        world.placed.insert((5, 6, 1), Block::ForgeCharged);
        world.placed.insert((7, 6, 1), Block::Embers);
        world.fires.insert((1, 6, 1), 40.0);
        world.fires.insert((7, 6, 1), 30.0);
        world.glow.insert((1, 6, 1), 12.5);
        world.cooking.insert((1, 6, 1), Cooking { raw: ItemKind::Tin, left: 4.0 });
        world.cooking.insert((7, 6, 1), Cooking { raw: ItemKind::Stone, left: 6.0 });
        let text = serialize_save("W", 9, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("place 1 6 1 forge_lit") && text.contains("place 3 6 1 clay_forge") && text.contains("place 5 6 1 forge_charged"));
        assert!(text.contains("glow 1 6 1 12.5") && text.contains("cook 1 6 1 tin 4") && text.contains("cook 7 6 1 stone 6"));
        let data = parse_save(&text);
        assert_eq!((data.glow.clone(), data.cooking.clone()), (world.glow.clone(), world.cooking.clone()));
        assert_eq!(data.placed, world.placed);
        assert!(data.removed.is_empty());
        let loaded = World::with_edits(9, data.removed, data.placed, data.fires).with_cooking(data.cooking).with_glow(data.glow);
        assert!(loaded.is_glowing((1, 6, 1)));
        assert_eq!(loaded.cooking[&(1, 6, 1)].raw, ItemKind::Tin);
        assert_eq!(loaded.cooking[&(7, 6, 1)].raw, ItemKind::Stone);
    }

    #[test]
    fn new_items_survive_a_save_round_trip() {
        let mut inv = Inventory::new();
        let kinds = [ItemKind::LeatherCord, ItemKind::Bronze, ItemKind::BronzeMallet, ItemKind::ClayForge, ItemKind::IronOre];
        for (i, kind) in kinds.into_iter().enumerate() {
            inv.slots[i] = Some(ItemStack { kind, count: i as u32 + 2 });
        }
        let world = World::new(9, HashSet::new());
        let text = serialize_save("W", 9, &world, &Player::new(), &inv, &[], MAX_HEALTH, false);
        assert!(text.contains("inv 0 leather_cord 2") && text.contains("inv 2 bronze_mallet 4") && text.contains("inv 3 clay_forge 5"));
        let data = parse_save(&text);
        assert_eq!(data.inventory, inv);
    }

    #[test]
    fn veined_stones_in_an_older_save_come_back_as_the_stone_and_pebble_that_smelt_into_the_same_metal() {
        let text = "W\n1\n0 6 0 0 0\ninv 0 copper_ore 3\ninv 1 tin_ore 2\ninv 2 iron_ore 1\nplace 1 6 1 embers\nfire 1 6 1 40\ncook 1 6 1 copper_ore 4\n";
        let data = parse_save(text);
        assert_eq!(data.inventory.slots[0], Some(ItemStack { kind: ItemKind::Stone, count: 3 }));
        assert_eq!(data.inventory.slots[1], Some(ItemStack { kind: ItemKind::Pebble, count: 2 }));
        assert_eq!(data.inventory.slots[2], Some(ItemStack { kind: ItemKind::IronOre, count: 1 }));
        assert_eq!(data.cooking[&(1, 6, 1)].raw, ItemKind::Stone);
        let data = parse_save(&text.replace("cook 1 6 1 copper_ore 4", "cook 1 6 1 tin_ore 4"));
        assert_eq!(data.cooking[&(1, 6, 1)].raw, ItemKind::Pebble);
        // A stone or pebble that was smelting before veined stones existed is still one.
        let data = parse_save(&text.replace("cook 1 6 1 copper_ore 4", "cook 1 6 1 pebble 4"));
        assert_eq!(data.cooking[&(1, 6, 1)].raw, ItemKind::Pebble);
    }

    #[test]
    fn malformed_glow_records_and_glow_without_embers_are_ignored() {
        let text = "W\n1\n0 6 0 0 0\nglow 1 2 3\nglow 1 2 3 -4\nglow 1 2 3 inf\nglow a 2 3 5\nglow 1 2 3 0\n";
        let data = parse_save(text);
        assert!(data.glow.is_empty() && data.removed.is_empty());
        let w = World::new(1, HashSet::new()).with_glow(HashMap::from([((1, 6, 1), 5.0)]));
        assert!(w.glow.is_empty(), "nothing burns there");
    }

    #[test]
    fn malformed_cook_records_are_ignored() {
        let text = "W\n1\n0 6 0 0 0\ncook 1 2 poultry 3\ncook 1 2 3 stick 3\ncook 1 2 3 cooked_poultry 3\ncook 1 2 3 venison -1\ncook 1 2 3 venison inf\ncook a 2 3 venison 3\ncook 1 2 3 venison\n";
        let data = parse_save(text);
        assert!(data.cooking.is_empty() && data.removed.is_empty());
    }

    #[test]
    fn malformed_place_and_fire_records_are_ignored() {
        let text = "W\n1\n0 6 0 0 0\nplace 1 2 placed_log\nplace 1 2 3 dirt\nplace 1 2 3 nonsense\nplace a b c fire\nfire 1 2 3\nfire 1 2 3 -4\nfire 1 2 3 inf\nfire x 2 3 5\n";
        let data = parse_save(text);
        assert!(data.placed.is_empty() && data.fires.is_empty() && data.removed.is_empty());
    }

    #[test]
    fn health_round_trips_and_defaults_to_full() {
        let world = World::new(1, HashSet::new());
        let text = serialize_save("W", 1, &world, &Player::new(), &Inventory::new(), &[], 4, false);
        assert!(text.contains("health 4"));
        assert_eq!(parse_save(&text).health, 4);
        assert_eq!(parse_save("W\n1\n0 6 0 0 0\n").health, MAX_HEALTH, "old saves are at full health");
        for bad in ["health 0", "health 11", "health -1", "health x", "health"] {
            let data = parse_save(&format!("W\n1\n0 6 0 0 0\n{bad}\n"));
            assert_eq!(data.health, MAX_HEALTH, "{bad}");
            assert!(data.removed.is_empty());
        }
    }

    #[test]
    fn old_saves_without_inventory_still_load() {
        let text = "Old World\n7\n10 6 10 0 0\n4 5 6\n7 1 2\n";
        let data = parse_save(text);
        assert_eq!(data.removed.len(), 2);
        assert_eq!(data.inventory, Inventory::new());
        assert!(data.items.is_empty());
    }

    #[test]
    fn malformed_records_are_ignored() {
        let text = "W\n1\n0 6 0 0 0\ninv 99 stick 3\ninv 0 diamond 3\ninv 0 stick 0\nitem stick 1 2\nitem nothing 1 2 3\nhello world\ninv 1 stick 500\n";
        let data = parse_save(text);
        assert_eq!(data.inventory.count(ItemKind::Stick), MAX_STACK);
        assert!(data.items.is_empty() && data.removed.is_empty());
    }

    #[test]
    fn pots_survive_a_save_round_trip_and_bad_pot_records_are_ignored() {
        let mut world = World::new(9, HashSet::new());
        world.placed.insert((1, 6, 1), Block::Fire);
        world.fires.insert((1, 6, 1), 40.0);
        world.pots.insert((1, 6, 1), Pot { water: true, meat: Some(ItemKind::RabbitMeat), left: 4.5, served: 0 });
        let text = serialize_save("W", 9, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(text.contains("pot 1 6 1 1 rabbit_meat 4.5"));
        let data = parse_save(&text);
        assert_eq!(data.pots, world.pots);
        let loaded = World::with_edits(9, data.removed, data.placed, data.fires).with_pots(data.pots);
        assert_eq!(loaded.pots[&(1, 6, 1)].meat, Some(ItemKind::RabbitMeat));
        let empty = parse_save("W\n1\n0 6 0 0 0\npot 2 6 2 0 - 12\n");
        assert_eq!(empty.pots[&(2, 6, 2)], Pot::empty());
        let bad = "W\n1\n0 6 0 0 0\npot 1 2 3 1 cooked_venison 3\npot 1 2 3 2 venison 3\npot 1 2 3 1 venison -1\npot 1 2 3 1 venison inf\npot a 2 3 1 venison 3\npot 1 2 3 1 venison\n";
        let data = parse_save(bad);
        assert!(data.pots.is_empty() && data.removed.is_empty());
    }

    #[test]
    fn dev_mode_is_saved_and_a_world_without_the_line_is_normal() {
        let world = World::new(3, HashSet::new());
        let dev = serialize_save("D", 3, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, true);
        assert!(dev.lines().any(|l| l == "mode dev"));
        assert!(parse_save(&dev).dev_mode);
        let normal = serialize_save("N", 3, &world, &Player::new(), &Inventory::new(), &[], MAX_HEALTH, false);
        assert!(!normal.contains("mode"));
        assert!(!parse_save(&normal).dev_mode);
        assert!(!parse_save("Old\n1\n0 6 0 0 0\nhealth 5\n").dev_mode, "older saves are normal");
        assert!(!parse_save("W\n1\n0 6 0 0 0\nmode nonsense\n").dev_mode);
        assert!(!SaveData::new().dev_mode);
    }
}
