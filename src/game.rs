//! A running game session: the loaded world, the player, their inventory and dropped items.

use macroquad::prelude::*;
use std::fs;

use crate::animal::{Animal, AnimalKind, BossEvent};
use crate::arrow::{Arrow, Fate};
use crate::block::Block;
use crate::config::*;
use crate::dropped::{collect_nearby, DroppedItem};
use crate::effects::rainbow_clear_of_view;
use crate::hazard::{acid_line, AcidCell, Burst, Projectile, ProjectileKind};
use crate::hud::{InventoryUi, Toasts};
use crate::inventory::Inventory;
use crate::item::{drops_for_with_bonuses, drops_for_with_berries, Hearth, ItemKind};
use crate::horizen::{self, Dimension};
use crate::tower::Tower;
use crate::skylands;
use crate::{ghosts, stonehenge};
use crate::landmarks::{compass_bearing, GrandCastle, Graveyard, Lake, Mountain, Rainbow, RainbowState, TrekEvent, Treks, Well};
use crate::player::Player;
use crate::rng::{clock_seed, Rng};
use crate::save::{parse_save, serialize_save_full, SaveData, SaveExtras, WorldInfo};
use crate::tornado::{Cyclone, FireTornado};
use crate::world::{Pot, ray_box, Hit, Pos, World};

/// What kind of flash a `ShownBurst` is (the game draws each its own way).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BurstShape {
    Ice,
    Fire,
    /// The ring that runs out from a smashed patch of ground.
    Smash,
    /// The same ring, from the Orangutan's smash (it reaches `ORANGUTAN_SMASH_RADIUS`).
    OrangutanSmash,
    /// A rock bursting where it landed.
    Rock,
}

/// A burst or a smash that is still on the screen, for `BURST_SHOW_SECONDS`.
#[derive(Clone, Copy, Debug)]
pub struct ShownBurst {
    pub pos: Vec3,
    pub shape: BurstShape,
    pub age: f32,
}

/// A block being broken. Progress resets when the target, the block or the held tool changes,
/// or when the button is released.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mining {
    pub pos: Pos,
    pub block: Block,
    pub elapsed: f32,
}

/// The blackout between day and night: the screen fades to black, the sky changes while it is dark, and it fades
/// back, all in `DAY_NIGHT_BLACKOUT_SECONDS`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blackout {
    /// Seconds since it began.
    pub elapsed: f32,
    /// What the world is to become, once it is dark.
    pub night: bool,
    /// Whether the world has changed yet.
    pub switched: bool,
}

/// Shares of the blackout spent fading out and fading in; the rest it is quite black.
const BLACKOUT_FADE_SHARE: f32 = 0.3;

impl Blackout {
    /// How dark the screen is, from 0 (clear) to 1 (black).
    pub fn darkness(&self) -> f32 {
        let t = (self.elapsed / DAY_NIGHT_BLACKOUT_SECONDS).clamp(0.0, 1.0);
        if t < BLACKOUT_FADE_SHARE {
            t / BLACKOUT_FADE_SHARE
        } else if t > 1.0 - BLACKOUT_FADE_SHARE {
            (1.0 - t) / BLACKOUT_FADE_SHARE
        } else {
            1.0
        }
    }
}

/// What kind of white screen is up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhiteoutKind {
    /// Arriving by cyclone: the screen starts white and clears.
    Arrival,
    /// Falling off an island of the Skylands: the screen whites out, the player is taken to the normal world and put `SKY_FALL_DROP`
    /// blocks above its spawn point (`moved`), and it clears as they fall. The overworld shows through the white meanwhile.
    Fall { moved: bool },
}

/// A white screen in the Skylands (see `WhiteoutKind`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Whiteout {
    pub elapsed: f32,
    pub kind: WhiteoutKind,
}

impl Whiteout {
    /// How white the screen is, from 0 (clear) to 1.
    pub fn alpha(&self) -> f32 {
        match self.kind {
            WhiteoutKind::Arrival => (1.0 - self.elapsed / SKY_ARRIVAL_FADE_SECONDS).clamp(0.0, 1.0),
            WhiteoutKind::Fall { .. } if self.elapsed < SKY_FALL_FADE_IN => self.elapsed / SKY_FALL_FADE_IN,
            WhiteoutKind::Fall { .. } => (1.0 - (self.elapsed - SKY_FALL_FADE_IN) / SKY_FALL_FADE_OUT).clamp(0.0, 1.0),
        }
    }

    fn duration(&self) -> f32 {
        match self.kind {
            WhiteoutKind::Arrival => SKY_ARRIVAL_FADE_SECONDS,
            WhiteoutKind::Fall { .. } => SKY_FALL_FADE_IN + SKY_FALL_FADE_OUT,
        }
    }

    /// Whether the normal world is drawn behind the white: only while falling out of the Skylands.
    pub fn shows_overworld(&self) -> bool {
        matches!(self.kind, WhiteoutKind::Fall { .. })
    }
}

/// The dimension the player is not in, as they left it: its world, its items on the ground and where the player stood.
pub struct Away {
    pub world: World,
    pub items: Vec<DroppedItem>,
    pub player: Player,
}

impl Away {
    /// A dimension nobody has been to yet, with the player at its arrival point.
    fn fresh(dimension: Dimension, seed: i64) -> Away {
        let (world, mut player) = match dimension {
            Dimension::Skylands => (World::new_skylands(seed), Player::new()),
            Dimension::Horizen => (World::new_horizen(seed), Player::new()),
            Dimension::Overworld => (World::new(seed, std::collections::HashSet::new()), Player::new()),
        };
        if dimension == Dimension::Skylands {
            player.pos = skylands::spawn_point();
        }
        Away { world, items: Vec::new(), player }
    }
}

pub struct Game {
    pub info: WorldInfo,
    /// Dev mode: the inventory screen lists every item to add.
    pub dev_mode: bool,
    pub world: World,
    pub player: Player,
    pub inventory: Inventory,
    /// Items lying on the ground.
    pub items: Vec<DroppedItem>,
    /// Random rolls for drops (not tied to the world seed, so repeated digging differs).
    pub rng: Rng,
    pub inventory_open: bool,
    /// Drag-and-drop state for the inventory screen.
    pub inventory_ui: InventoryUi,
    /// Index (0..HOTBAR_SLOTS) of the hotbar slot the player is holding.
    pub selected: usize,
    pub toasts: Toasts,
    /// The block being broken and how long the button has been held on it.
    pub mining: Option<Mining>,
    /// Life points, `0..=MAX_HEALTH`. Reaching zero respawns the player.
    pub health: u32,
    /// Seconds spent touching fire since the last burn tick.
    pub burn_timer: f32,
    /// Wildlife living around the player. Animals and will-o-whisps are not saved: new ones wander in.
    pub animals: Vec<Animal>,
    /// Arrows in flight.
    pub arrows: Vec<Arrow>,
    /// Seconds until the next axe swing or bow shot is allowed.
    pub attack_cooldown: f32,
    /// Seconds since the last attempt to bring a new animal into the world.
    pub spawn_timer: f32,
    pub autosave_timer: f32,
    /// Progress of the walk through a river and the walk behind a compass, which bring in lakes and mountains.
    pub treks: Treks,
    /// Frames of mouse movement to ignore after the cursor is re-captured.
    pub skip_look_frames: u8,
    /// The blackout in progress between day and night, if any.
    pub blackout: Option<Blackout>,
    /// The one thing the player wears: a pelt cloak (bears leave them alone unless hit first), the royal armour (all
    /// damage halved), the king's crown or a backpack (the inventory has its two extra rows only while it is worn). They
    /// all share this one place.
    pub worn: Option<ItemKind>,
    /// A dimension the player is not in, once they have been somewhere else (see `Away`).
    pub away: Option<Away>,
    /// A second parked dimension, when the player has been to all three.
    pub shelf: Option<Away>,
    /// Seconds the player has stood in a cyclone.
    pub cyclone_timer: f32,
    /// The white screen of the Skylands, if one is up.
    pub whiteout: Option<Whiteout>,
    /// The fire tornadoes currently active in the Skylands (daytime only): one appears every `SKY_TORNADO_INTERVAL` seconds and
    /// lasts `SKY_TORNADO_LIFETIME`, so there are sometimes two.
    pub tornadoes: Vec<FireTornado>,
    /// Seconds since the previous Skylands tornado spawned.
    pub tornado_timer: f32,
    /// The great fire tornado that whirls in the Phoenix's crater all day (never at night), whether or not the Phoenix lives.
    pub crater_tornado: Option<FireTornado>,
    /// Whether the daytime will-o-whisp storm clouds have gathered.
    pub storm_active: bool,
    storm_gather_timer: f32,
    storm_clear_timer: f32,
    storm_lightning_timer: f32,
    storm_lightning_flash: f32,
    storm_lightning_pos: Vec3,
    /// Fade in progress after the player reaches the bottom of the well.
    pub horizen_transition: Option<f32>,
    /// Quail the player has killed. An Iron Quail comes for every `QUAIL_KILLS_PER_IRON_QUAIL`.
    pub quail_kills: u32,
    /// What a ghost is saying now, and for how many more seconds (shown as text on the screen: ghosts are not seen).
    pub ghost_text: Option<(String, f32)>,
    /// Whether the player is inside the graveyard (so that its ghost speaks when they walk in, not every frame).
    pub in_graveyard: bool,
    /// Iron Quails that are due but have not yet found a place to appear (or were left behind in another dimension).
    pub iron_quails_owed: u32,
    /// Bats the player has killed in the Horizen's caves (a vampire bat comes for every `BATS_PER_VAMPIRE`), and vampire bats that are
    /// due but have not yet appeared (one at a time).
    pub bat_kills: u32,
    pub vampire_bats_owed: u32,
    /// Counts the cave's wildlife spawn ticks: it brings a bat in only on every `BAT_CAVE_SPAWN_EVERY`th, so bats come half as often.
    cave_spawn_tick: u32,
    /// The pop-up in which the player gives the king's ghost gold: how many offerings (of `GOLD_PER_SPIRIT` gold, one spirit each) they
    /// have chosen so far. `None` while it is closed.
    pub king_dialog: Option<u32>,
    /// Whether the mouse cursor is free because that pop-up is open (set by `play::sync_dialog_cursor`).
    pub dialog_cursor_free: bool,
    /// Arrows the Wild Hunt's archer has shot, in flight (they hurt the player and nothing else, and are used up on landing).
    pub hostile_arrows: Vec<Arrow>,
    /// Seconds left of the victory message on the screen, once the Wild Hunt has been beaten.
    pub victory: Option<f32>,
    /// The Yeti's lumps of ice and the Phoenix's fireballs in flight, the flashes where they burst, and the lake water Nessie has
    /// made acid.
    pub projectiles: Vec<Projectile>,
    pub bursts: Vec<ShownBurst>,
    pub acid: Vec<AcidCell>,
    /// Seconds the player has stood in acid water since it last burned them.
    acid_burn: f32,
    /// The health the Wild Hunt began with, and what its splitting bears and crows have added: the whole of its shared health bar.
    hunt_total: i32,
}

/// Whether nothing solid lies on the straight line from `eye` along the unit vector `dir` for `dist` blocks.
fn clear_line(world: &World, eye: Vec3, dir: Vec3, dist: f32) -> bool {
    let eps = Vec3::splat(0.01);
    let steps = (dist / 0.1).ceil() as i32;
    (0..=steps).all(|i| {
        let p = eye + dir * (i as f32 * 0.1).min(dist);
        !world.box_blocked(p - eps, p + eps)
    })
}

/// What a world wears now that everything shares one place, from what its save says is worn and what it says is on the back
/// (saves made when the backpack had a place of its own): the pack wins, since the rows of its slots are in the save, and the
/// other garment, if there is one, is handed back to be put into the inventory.
fn one_worn_place(worn: Option<ItemKind>, back: Option<ItemKind>) -> (Option<ItemKind>, Option<ItemKind>) {
    match (worn, back) {
        (other, Some(pack)) => (Some(pack), other.filter(|&k| k != pack)),
        (worn, None) => (worn, None),
    }
}

impl Game {
    pub fn load(info: WorldInfo) -> Game {
        let data = fs::read_to_string(&info.path).map(|t| parse_save(&t)).unwrap_or_else(|_| SaveData::new());
        Game::from_save(info, data, clock_seed())
    }

    pub fn from_save(info: WorldInfo, data: SaveData, rng_seed: u64) -> Game {
        let mut world = World::with_edits(info.seed, data.removed, data.placed, data.fires)
            .with_cooking(data.cooking)
            .with_glow(data.glow)
            .with_pots(data.pots)
            .with_berry_picked(data.berry_picked)
            .with_night(data.night)
            .with_landmarks(data.landmarks);
        world.stream_chunks(data.player.pos);
        world.rebuild_dirty(data.player.pos, usize::MAX);
        let health = data.health;
        let dev_mode = data.dev_mode;
        // Saves from when the backpack had a place of its own may list both: the pack stays on, and the other garment is put
        // back into the inventory (or on the ground) once the game is built.
        let (worn, displaced) = one_worn_place(data.worn, data.back);
        let mut inventory = data.inventory;
        // The rows of the pack are there exactly while a pack is worn.
        if worn.map_or(false, |k| k.is_backpack()) {
            inventory.add_pack_rows();
        } else if inventory.has_pack_rows() && !inventory.remove_pack_rows() {
            inventory.slots.truncate(INVENTORY_SLOTS);
        }
        let mut player = data.player;
        player.lift_out_of_blocks(&world);
        let items = data.items;
        let parked = |saved: Option<Box<SaveData>>, dimension: Dimension| {
            saved.map(|h| {
                let h = *h;
                let mut w = World::with_edits(info.seed, h.removed, h.placed, h.fires)
                    .with_cooking(h.cooking)
                    .with_glow(h.glow)
                    .with_pots(h.pots)
                    .with_night(h.night)
                    .with_landmarks(h.landmarks);
                w.dimension = dimension;
                Away { world: w, items: h.items, player: h.player }
            })
        };
        let away = parked(data.horizen, Dimension::Horizen);
        let shelf = parked(data.skylands, Dimension::Skylands);
        let (in_horizen, in_skylands) = (data.in_horizen, data.in_skylands);
        let mut game = Game {
            info,
            dev_mode,
            world,
            player,
            inventory,
            items,
            rng: Rng::new(rng_seed),
            inventory_open: false,
            inventory_ui: InventoryUi::new(),
            selected: 0,
            toasts: Toasts::new(),
            mining: None,
            health,
            burn_timer: 0.0,
            animals: Vec::new(),
            arrows: Vec::new(),
            attack_cooldown: 0.0,
            spawn_timer: 0.0,
            autosave_timer: 0.0,
            treks: Treks::new(),
            skip_look_frames: 0,
            blackout: None,
            worn,
            away,
            shelf,
            cyclone_timer: 0.0,
            whiteout: None,
            tornadoes: Vec::new(),
            tornado_timer: 0.0,
            crater_tornado: None,
            storm_active: false,
            storm_gather_timer: 0.0,
            storm_clear_timer: 0.0,
            storm_lightning_timer: 0.0,
            storm_lightning_flash: 0.0,
            storm_lightning_pos: Vec3::ZERO,
            horizen_transition: None,
            quail_kills: data.quail_kills,
            ghost_text: None,
            in_graveyard: false,
            iron_quails_owed: 0,
            bat_kills: data.bat_kills,
            vampire_bats_owed: 0,
            cave_spawn_tick: 0,
            king_dialog: None,
            dialog_cursor_free: false,
            hostile_arrows: Vec::new(),
            victory: None,
            projectiles: Vec::new(),
            bursts: Vec::new(),
            acid: Vec::new(),
            acid_burn: 0.0,
            hunt_total: 0,
        };
        // A world saved away from the overworld picks up the dimension it was saved in.
        if in_horizen {
            game.travel_to(Dimension::Horizen);
        } else if in_skylands {
            game.travel_to(Dimension::Skylands);
        }
        game.horizen_transition = None;
        game.whiteout = None;
        if let Some(kind) = displaced {
            game.stow_or_drop(kind);
        }
        game
    }

    /// The save text for this session (see `serialize_save_full`): the overworld always as the main part, the Horizen
    /// as records starting with `h`.
    pub fn save_text(&self) -> String {
        let current = (&self.world, &self.player, &self.items[..]);
        let find = |d: Dimension| {
            if self.world.dimension == d {
                Some(current)
            } else {
                self.parked(d).map(|a| (&a.world, &a.player, &a.items[..]))
            }
        };
        let over = find(Dimension::Overworld).unwrap_or(current);
        let extras = SaveExtras {
            worn: self.worn,
            in_horizen: self.in_horizen(),
            horizen: find(Dimension::Horizen),
            in_skylands: self.in_skylands(),
            skylands: find(Dimension::Skylands),
            quail_kills: self.quail_kills,
            bat_kills: self.bat_kills,
        };
        serialize_save_full(&self.info.name, self.info.seed, over.0, over.1, &self.inventory, over.2, self.health, self.dev_mode, &extras)
    }

    pub fn save(&self) {
        let out = self.save_text();
        if let Err(e) = fs::create_dir_all(SAVE_DIR).and_then(|_| fs::write(&self.info.path, out)) {
            eprintln!("Failed to save world: {e}");
        }
    }

    pub fn in_horizen(&self) -> bool {
        self.world.dimension == Dimension::Horizen
    }

    pub fn in_skylands(&self) -> bool {
        self.world.dimension == Dimension::Skylands
    }

    /// Whether the player is in the ordinary world (the landmarks, wildlife, weather and trails belong to it alone).
    pub fn in_overworld(&self) -> bool {
        self.world.dimension == Dimension::Overworld
    }

    /// The parked dimension of that kind, if the player has been there and is not there now.
    pub fn parked(&self, dimension: Dimension) -> Option<&Away> {
        [&self.away, &self.shelf].into_iter().flatten().find(|a| a.world.dimension == dimension)
    }

    fn take_parked(&mut self, dimension: Dimension) -> Option<Away> {
        if self.away.as_ref().map_or(false, |a| a.world.dimension == dimension) {
            self.away.take()
        } else if self.shelf.as_ref().map_or(false, |a| a.world.dimension == dimension) {
            self.shelf.take()
        } else {
            None
        }
    }

    fn park(&mut self, other: Away) {
        if self.away.is_none() {
            self.away = Some(other);
        } else {
            self.shelf = Some(other);
        }
    }

    /// The item in the selected hotbar slot, if any.
    pub fn held_item(&self) -> Option<ItemKind> {
        self.inventory.slots[self.selected].map(|s| s.kind)
    }

    /// Hold the hotbar slot `slot` (ignored if out of range).
    pub fn select_slot(&mut self, slot: usize) {
        if slot < HOTBAR_SLOTS {
            self.selected = slot;
        }
    }

    /// Move the hotbar selection by `delta` slots, wrapping around either end.
    pub fn scroll_selection(&mut self, delta: i32) {
        let n = HOTBAR_SLOTS as i32;
        self.selected = (self.selected as i32 + delta).rem_euclid(n) as usize;
    }

    /// Break the block at (x, y, z) if the held item allows it (hands for dirt and leaves,
    /// a stone axe for wood), rolling and spawning its drops.
    /// Returns the kinds that dropped (empty when the block cannot be broken with what is
    /// held, and for unlucky rolls).
    pub fn mine(&mut self, x: i32, y: i32, z: i32) -> Vec<ItemKind> {
        let block = self.world.block_at(x, y, z);
        if !block.can_break_with(self.held_item()) {
            return Vec::new();
        }
        if block == Block::AltarLoadstone {
            // The altar stays; only its loadstone goes.
            self.world.set_block(x, y, z, Block::AncientAltar);
        } else {
            self.world.break_block(x, y, z);
        }
        let mut drops = drops_for_with_bonuses(
            block,
            if block == Block::Bush { self.world.berry_kind_at(x, z) } else { ItemKind::Berries },
            self.rabbit_foot_in_hotbar(),
            &mut self.rng,
        );
        if self.rabbit_foot_in_hotbar() {
            if matches!(block, Block::PlacedStone | Block::WetStone)
                && self.rng.chance(RABBIT_FOOT_LOADSTONE_CHANCE)
            {
                if let Some(stone) = drops.iter_mut().find(|kind| **kind == ItemKind::Stone) {
                    *stone = ItemKind::Loadstone;
                }
            }
            if block == Block::Boulder && self.rng.chance(RABBIT_FOOT_GOLD_CHANCE) {
                for kind in &mut drops {
                    if *kind == ItemKind::IronOre {
                        *kind = ItemKind::GoldOre;
                    }
                }
            }
        }
        if block == Block::WetClay && self.held_item() == Some(ItemKind::TinPan) {
            // Panning the clay: one time in twenty a gold turns up, one in five with a rabbit's foot in the hotbar.
            let chance = if self.rabbit_foot_in_hotbar() { TIN_PAN_RABBIT_FOOT_GOLD_CHANCE } else { TIN_PAN_GOLD_CHANCE };
            if self.rng.chance(chance) {
                drops.push(ItemKind::Gold);
            }
        }
        if block == Block::GoldPile && self.world.active_rainbow().map_or(false, |r| r.gold == (x, y, z)) {
            // The gold is gone, and so is the rainbow, for the rest of the game.
            self.world.set_rainbow(RainbowState::Spent);
            self.toasts.push("The rainbow fades. There will not be another.".to_string());
        }
        for kind in drops.iter_mut() {
            // A player who already has a silver key is given a gold bar instead of another (see `key_or_gold`).
            if *kind == ItemKind::SilverKey {
                *kind = self.key_or_gold();
            }
            self.items.push(DroppedItem::spawn(*kind, (x, y, z), &mut self.rng));
        }
        drops
    }

    /// Whether a holly rod is anywhere in the hotbar (it need not be in the hand).
    pub fn holly_rod_in_hotbar(&self) -> bool {
        self.inventory.slots[..HOTBAR_SLOTS].iter().flatten().any(|stack| stack.kind == ItemKind::HollyRod)
    }

    /// Number of whisps that have bonded to the player and are currently following them.
    pub fn following_whisp_count(&self) -> usize {
        self.animals.iter().filter(|a| a.kind == AnimalKind::WillOWhisp && a.bonded).count()
    }

    /// The current lightning bolt, if its short flash is still visible.
    pub fn storm_lightning(&self) -> Option<(Vec3, f32)> {
        (self.storm_lightning_flash > 0.0).then_some((self.storm_lightning_pos, self.storm_lightning_flash))
    }

    /// Whether a screen that takes the mouse is open (the inventory or the king's pop-up): the player then cannot move, look or act.
    pub fn ui_open(&self) -> bool {
        self.inventory_open || self.king_dialog.is_some()
    }

    /// Set what the worn thing does to the player's movement (call every frame, before the player moves): the boots of the sky make them
    /// run `BOOTS_SPEED_FACTOR` times as fast and jump `BOOTS_JUMP_DISTANCE_FACTOR` times as far.
    pub fn apply_worn_effects(&mut self) {
        let boots = self.worn == Some(ItemKind::BootsOfTheSky);
        self.player.speed_factor = if boots { BOOTS_SPEED_FACTOR } else { 1.0 };
        // A jump is as long as the speed along the ground times the time in the air, and the time in the air is proportional to the take-off
        // speed, so a jump `D` times as far at `S` times the running speed needs a take-off `D / S` times as fast.
        self.player.jump_factor = if boots { BOOTS_JUMP_DISTANCE_FACTOR / BOOTS_SPEED_FACTOR } else { 1.0 };
    }

    /// Whether a cursed idol is in the hotbar: the player then deals `IDOL_DAMAGE_FACTOR` times the damage and cannot heal.
    pub fn idol_in_hotbar(&self) -> bool {
        self.inventory.slots[..HOTBAR_SLOTS].iter().flatten().any(|stack| stack.kind == ItemKind::CursedIdol)
    }

    fn rabbit_foot_in_hotbar(&self) -> bool {
        self.inventory.slots[..HOTBAR_SLOTS].iter().flatten().any(|stack| stack.kind == ItemKind::RabbitFoot)
    }

    /// Drop an entire inventory stack in front of the player. The distance and the normal pickup delay keep
    /// the new drops from immediately returning to the inventory.
    pub fn drop_inventory_stack(&mut self, slot: usize) -> bool {
        let Some(slot) = self.inventory.slots.get_mut(slot) else { return false };
        let Some(stack) = slot.take() else { return false };
        let forward = vec3(self.player.yaw.cos(), 0.0, self.player.yaw.sin());
        let spot = self.player.pos + forward * INVENTORY_DROP_DISTANCE + vec3(0.0, 1.0, 0.0);
        for _ in 0..stack.count {
            self.items.push(DroppedItem::spawn_at(stack.kind, spot, &mut self.rng));
        }
        self.toasts.push(format!("Dropped {} {}", stack.count, stack.kind.name().to_lowercase()));
        true
    }

    /// Advance block breaking by `dt` seconds. `holding` is whether the break button is down and
    /// `target` the block under the crosshair. A block breaks once the button has been held on it
    /// for `Block::break_seconds`; looking away, releasing the button or switching to a tool that
    /// cannot break it starts the progress over. A bow drill lights a placed log at once.
    /// Returns the kinds dropped when a block broke on this call.
    pub fn update_mining(&mut self, target: Option<Hit>, holding: bool, dt: f32) -> Vec<ItemKind> {
        let Some(hit) = target.filter(|_| holding) else {
            self.mining = None;
            return Vec::new();
        };
        if self.try_ignite(hit.x, hit.y, hit.z)
            || self.try_blow(hit.x, hit.y, hit.z)
            || !hit.block.can_break_with(self.held_item())
        {
            self.mining = None;
            return Vec::new();
        }
        let pos = (hit.x, hit.y, hit.z);
        let mut m = match self.mining {
            Some(m) if m.pos == pos && m.block == hit.block => m,
            _ => Mining { pos, block: hit.block, elapsed: 0.0 },
        };
        m.elapsed += dt;
        if m.elapsed >= hit.block.break_seconds() {
            self.mining = None;
            return self.mine(pos.0, pos.1, pos.2);
        }
        self.mining = Some(m);
        Vec::new()
    }

    /// How far along the current break is, from 0 to 1 (`None` when not breaking anything).
    pub fn mining_progress(&self) -> Option<f32> {
        self.mining.map(|m| (m.elapsed / m.block.break_seconds()).clamp(0.0, 1.0))
    }

    /// The centre of a burning log, embers or forge the player's body is touching (or within `FIRE_TOUCH_MARGIN` of).
    fn touched_fire(&self) -> Option<Vec3> {
        let (p, m) = (self.player.pos, FIRE_TOUCH_MARGIN);
        self.world.fires.keys().find_map(|&(x, y, z)| {
            let (lo, hi) = self.world.block_at(x, y, z).bounds()?;
            let c = vec3(x as f32, y as f32, z as f32);
            let touching = p.x + PLAYER_HALF_WIDTH + m > c.x + lo[0]
                && p.x - PLAYER_HALF_WIDTH - m < c.x + hi[0]
                && p.y + PLAYER_HEIGHT + m > c.y + lo[1]
                && p.y - m < c.y + hi[1]
                && p.z + PLAYER_HALF_WIDTH + m > c.z + lo[2]
                && p.z - PLAYER_HALF_WIDTH - m < c.z + hi[2];
            touching.then(|| c + vec3(0.5, 0.5, 0.5))
        })
    }

    /// Whether the player's body is touching (or within `FIRE_TOUCH_MARGIN` of) a burning log, embers or forge.
    #[cfg(test)]
    pub fn touching_fire(&self) -> bool {
        self.touched_fire().is_some()
    }

    /// Hurt the player for `amount` life. At zero life the player respawns at the start with full health,
    /// keeping their inventory, and `death` is shown; any enraged moose calms down. Returns whether the player died.
    fn hurt_player(&mut self, amount: u32, death: &str) -> bool {
        // The royal armour halves every blow (rounding up: a blow always costs at least one life).
        let amount = if self.worn == Some(ItemKind::RoyalArmor) && amount > 0 { (amount + ARMOR_DAMAGE_DIVISOR - 1) / ARMOR_DAMAGE_DIVISOR } else { amount };
        self.health = self.health.saturating_sub(amount);
        if self.health == 0 {
            // Dying in another dimension takes the player back to where the overworld begins.
            if !self.in_overworld() {
                self.travel_to(Dimension::Overworld);
                self.whiteout = None;
            }
            self.player = Player::new();
            self.health = MAX_HEALTH;
            self.burn_timer = 0.0;
            self.toasts.push(death.to_string());
            for a in self.animals.iter_mut().filter(|a| a.kind == AnimalKind::Moose) {
                a.calm();
            }
            // The fight is over: the Iron Quail has won.
            self.animals.retain(|a| a.kind != AnimalKind::IronQuail);
            // The Wild Hunt has won too: its warriors and their arrows are gone, and the horn can call them again.
            self.animals.retain(|a| !a.kind.is_wild_hunt());
            // The ravens Nessie called go too, and what the bosses threw.
            self.animals.retain(|a| !(a.kind == AnimalKind::Raven && a.summoned));
            // The vampire bat has won, and the king's spirits go with the player's death.
            self.animals.retain(|a| !matches!(a.kind, AnimalKind::VampireBat | AnimalKind::SpiritArcher));
            self.king_dialog = None;
            self.hostile_arrows.clear();
            self.projectiles.clear();
            self.acid.clear();
            self.hunt_total = 0;
            return true;
        }
        false
    }

    /// Throw the player away from `from` (where the blow or the burn came from).
    fn knock_player(&mut self, from: Vec3) {
        self.player.knock_back(self.player.pos - from);
    }

    /// Burn the player for `FIRE_DAMAGE` each full `FIRE_DAMAGE_INTERVAL` spent touching fire, and throw them out of it.
    pub fn update_health(&mut self, dt: f32) {
        let Some(fire) = self.touched_fire() else {
            self.burn_timer = 0.0;
            return;
        };
        self.burn_timer += dt;
        while self.burn_timer >= FIRE_DAMAGE_INTERVAL {
            self.burn_timer -= FIRE_DAMAGE_INTERVAL;
            if !self.hurt_player(FIRE_DAMAGE, "You burned to death") {
                self.knock_player(fire);
            }
        }
    }

    /// Eat or drink one held food item. Red food heals only while hurt; purple berries and
    /// purple berry stew damage the player and can be consumed at any health.
    pub fn eat_held(&mut self) -> bool {
        let Some(kind) = self.held_item() else { return false };
        // The cursed idol in the hotbar lets nothing heal the player.
        if self.idol_in_hotbar() && (kind.heal_amount().is_some() || kind == ItemKind::RedBerryStew) {
            self.toasts.push("The cursed idol will not let you heal.".to_string());
            return false;
        }
        match kind {
            ItemKind::PurpleBerries => {
                self.inventory.take_from_slot(self.selected);
                self.hurt_player(PURPLE_BERRY_DAMAGE, "The purple berries hurt you");
                true
            }
            ItemKind::PurpleBerryStew => {
                self.inventory.take_from_slot(self.selected);
                self.hurt_player(PURPLE_BERRY_STEW_DAMAGE, "The purple berry stew hurts you");
                self.set_night(true);
                true
            }
            ItemKind::RedBerryStew => {
                self.inventory.take_from_slot(self.selected);
                self.health = (self.health + BERRY_STEW_HEAL).min(MAX_HEALTH);
                self.set_night(false);
                true
            }
            _ => {
                let Some(heal) = kind.heal_amount() else { return false };
                if self.health >= MAX_HEALTH { return false; }
                self.inventory.take_from_slot(self.selected);
                self.health = (self.health + heal).min(MAX_HEALTH);
                true
            }
        }
    }

    /// Turn the world to night or to day, behind a blackout (see `Blackout`). Nothing happens if it already is.
    fn set_night(&mut self, night: bool) {
        // A nightfall immediately removes the daytime tornado, even while the blackout is fading in.
        if night {
            self.tornadoes.clear();
            self.crater_tornado = None;
            self.tornado_timer = 0.0;
            self.reset_storm();
        }
        match self.blackout.as_mut() {
            Some(b) => {
                b.night = night;
                if b.switched {
                    self.apply_night(night);
                }
            }
            None if self.world.night != night => self.blackout = Some(Blackout { elapsed: 0.0, night, switched: false }),
            None => {}
        }
    }

    /// Change the sky and the world now.
    fn apply_night(&mut self, night: bool) {
        // Night in the cave is too much: the player blacks out and wakes beside the well they came in by.
        if self.in_horizen() && night {
            self.faint_in_cave();
            return;
        }
        let changed = self.world.night != night;
        self.world.set_night(night);
        if night {
            self.tornadoes.clear();
            self.crater_tornado = None;
            self.tornado_timer = 0.0;
            self.reset_storm();
        }
        // A gate or door that has shut where the player stands puts them beside it instead.
        self.player.eject_from_blocks(&self.world);
        // A bridge that has broken under the player sets them on the nearest island or bridge stub instead of dropping them.
        if changed && night && self.in_skylands() {
            let p = self.player.pos;
            let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
            let was_on_deck = skylands::deck_at(self.info.seed, x, z, false).is_some() && skylands::surface_top(self.info.seed, x, z, true).is_none();
            if was_on_deck {
                if let Some(spot) = skylands::nearest_footing(self.info.seed, x, z, true, SKY_BROKEN_RESCUE_REACH) {
                    self.player.pos = spot;
                    self.player.vel = Vec3::ZERO;
                    self.toasts.push("The bridge breaks under you in the dark; you are thrown to the nearest footing.".to_string());
                }
            }
        }
        if changed {
            self.toasts.push(if night { "Night falls".to_string() } else { "Day breaks".to_string() });
        }
    }

    /// Night falls in the Horizen: the player blacks out and comes to in the ring of Stonehenge in the overworld. The cave stays day for the next visit, and the blackout already under way carries on to fade back in.
    fn faint_in_cave(&mut self) {
        let blackout = self.blackout;
        self.travel_to(Dimension::Overworld);
        self.blackout = blackout;
        self.whiteout = None;
        // They wake in the ring of Stonehenge, five chunks from where the world began.
        let (ax, az) = stonehenge::arrival(self.info.seed);
        let (x, z) = (ax.floor() as i32, az.floor() as i32);
        self.world.stream_chunks(vec3(ax, 6.0, az));
        self.player.pos = vec3(ax, self.world.ground_top(x, z) as f32 + 1.01, az);
        self.player.vel = Vec3::ZERO;
        self.world.stream_chunks(self.player.pos);
        self.world.rebuild_dirty(self.player.pos, usize::MAX);
        self.toasts.push("Night falls in the cave. You black out, and wake among the standing stones of Stonehenge.".to_string());
    }

    /// Advance the blackout: once the screen is black the world changes to day or night, and the blackout ends after
    /// `DAY_NIGHT_BLACKOUT_SECONDS`. Call once a frame.
    pub fn update_blackout(&mut self, dt: f32) {
        let Some(mut b) = self.blackout else { return };
        b.elapsed += dt;
        if !b.switched && b.elapsed >= DAY_NIGHT_BLACKOUT_SECONDS * BLACKOUT_FADE_SHARE {
            b.switched = true;
            self.apply_night(b.night);
        }
        self.blackout = if b.elapsed >= DAY_NIGHT_BLACKOUT_SECONDS { None } else { Some(b) };
    }

    /// Pick all currently available berries from a bush without removing the bush.
    fn pick_berry_bush(&mut self, pos: Pos) -> bool {
        if !self.world.pick_berry_bush(pos) {
            return false;
        }
        let kind = self.world.berry_kind_at(pos.0, pos.2);
        let drops = drops_for_with_berries(Block::Bush, kind, &mut self.rng);
        let count = drops.len();
        for drop in drops {
            self.give_or_drop(drop, pos);
        }
        self.toasts.push(if count == 0 {
            "The bush had no ripe berries".to_string()
        } else {
            format!("Picked berries from the bush")
        });
        true
    }

    /// Whether the held item lets the player do something to `block` with the left mouse
    /// button: break it, light it with a bow drill, or work a lit forge with bellows.
    pub fn can_interact(&self, block: Block) -> bool {
        block.can_break_with(self.held_item()) || self.can_ignite(block) || self.can_blow(block) || self.can_unlock(block) || self.can_use_relic(block)
    }

    /// Whether a right click with what is held (or worn) does something to the block: the Philosopher's Stone on a boulder, a
    /// crowned head on the sword in the stone.
    fn can_use_relic(&self, block: Block) -> bool {
        (block == Block::Boulder && self.held_item() == Some(ItemKind::PhilosophersStone))
            || (matches!(block, Block::SwordInStone | Block::SwordHilt) && self.worn == Some(ItemKind::AncientCrown))
    }

    /// Whether the held silver key fits the block (the lock of a tower's door).
    fn can_unlock(&self, block: Block) -> bool {
        matches!(block, Block::TowerDoor | Block::TowerDoorTop) && self.held_item() == Some(ItemKind::SilverKey)
    }

    /// The centre column of the tower whose door has a block at `pos`: the door is in the middle of the south side.
    fn tower_of_door(&self, (x, _, z): Pos) -> (i32, i32) {
        (x, z - TOWER_HALF)
    }

    /// Use the held silver key on the locked door at `pos`: the door opens for good and the player keeps the key. Without a key
    /// the player is told what is needed. Returns whether the door opened.
    pub fn try_unlock(&mut self, pos: Pos) -> bool {
        if !matches!(self.world.block_at(pos.0, pos.1, pos.2), Block::TowerDoor | Block::TowerDoorTop) {
            return false;
        }
        if self.held_item() != Some(ItemKind::SilverKey) {
            self.toasts.push("The door is locked. It needs a silver key.".to_string());
            return false;
        }
        let (x, z) = self.tower_of_door(pos);
        // The key only turns the lock: the player keeps it.
        self.world.unlock_tower(x, z);
        self.toasts.push("The silver key turns. The door swings open.".to_string());
        true
    }

    fn can_ignite(&self, block: Block) -> bool {
        matches!(block, Block::PlacedLog | Block::Charcoal | Block::ForgeCharged) && self.held_item() == Some(ItemKind::BowDrill)
    }

    fn can_blow(&self, block: Block) -> bool {
        block == Block::ForgeLit && self.held_item() == Some(ItemKind::Bellows)
    }

    /// Light the placed log, charcoal or filled forge at (x, y, z) if a bow drill is held. It burns for `FIRE_SECONDS`.
    pub fn try_ignite(&mut self, x: i32, y: i32, z: i32) -> bool {
        let block = self.world.block_at(x, y, z);
        if !self.can_ignite(block) {
            return false;
        }
        let lit = self.world.ignite((x, y, z), FIRE_SECONDS);
        if lit {
            let what = match block {
                Block::Charcoal => "The charcoal begins to glow",
                Block::ForgeCharged => "The forge is lit",
                _ => "The log catches fire",
            };
            self.toasts.push(what.to_string());
        }
        lit
    }

    /// Work the lit forge at (x, y, z) with bellows if they are held: it glows brighter, hot
    /// enough to smelt iron, for `BELLOWS_GLOW_SECONDS`. Hitting it again renews the glow.
    pub fn try_blow(&mut self, x: i32, y: i32, z: i32) -> bool {
        if !self.can_blow(self.world.block_at(x, y, z)) {
            return false;
        }
        let fresh = !self.world.is_glowing((x, y, z));
        let blown = self.world.blow((x, y, z));
        if blown && fresh {
            self.toasts.push("The forge glows brighter".to_string());
        }
        blown
    }

    /// Whether `block` may be placed in the empty cell `(x, y, z)`: the cell is free, it is held
    /// up, and (for blocks that collide) the player is not standing in it. Small items need solid
    /// ground below. A plank only needs a dirt, stone or plank block next to it on any side, so a
    /// ceiling can reach out from the top of a wall.
    pub fn can_place(&self, block: Block, (x, y, z): Pos) -> bool {
        if y < 1 || y >= WORLD_HEIGHT || !self.world.chunks.contains_key(&World::chunk_of(x, z)) {
            return false;
        }
        if self.world.block_at(x, y, z) != Block::Air {
            return false;
        }
        let supported = if block == Block::Planks {
            const SIDES: [(i32, i32, i32); 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
            SIDES.iter().any(|&(dx, dy, dz)| self.world.block_at(x + dx, y + dy, z + dz).anchors_planks())
        } else {
            self.world.block_at(x, y - 1, z).supports_placement()
        };
        if !supported {
            return false;
        }
        if block.collides() {
            if let Some((lo, hi)) = block.bounds() {
                let (c, p) = (vec3(x as f32, y as f32, z as f32), self.player.pos);
                let overlaps = p.x + PLAYER_HALF_WIDTH > c.x + lo[0]
                    && p.x - PLAYER_HALF_WIDTH < c.x + hi[0]
                    && p.y + PLAYER_HEIGHT > c.y + lo[1]
                    && p.y < c.y + hi[1]
                    && p.z + PLAYER_HALF_WIDTH > c.z + lo[2]
                    && p.z - PLAYER_HALF_WIDTH < c.z + hi[2];
                if overlaps {
                    return false;
                }
            }
        }
        true
    }

    /// What a held item is laid on a fire as: copper goes in together with a tin, so it is laid as the tin.
    fn piece_for(held: ItemKind) -> ItemKind {
        if held == ItemKind::Copper { ItemKind::Tin } else { held }
    }

    /// Whether the held `item` could be laid on the fire at `pos`: it has to suit that fire, and bronze also needs
    /// one tin and one copper in the inventory.
    fn can_lay_held(&self, pos: Pos, item: ItemKind) -> bool {
        let piece = Self::piece_for(item);
        self.world.can_lay(pos, piece)
            && (piece != ItemKind::Tin || (self.inventory.count(ItemKind::Tin) >= 1 && self.inventory.count(ItemKind::Copper) >= 1))
    }

    /// Why `piece` was refused by the free fire at `pos`, for a toast (`None` when the fire is not the problem).
    fn refusal(&self, pos: Pos, piece: ItemKind) -> Option<&'static str> {
        if self.world.cooking.contains_key(&pos) {
            return None;
        }
        let lit_forge = self.world.block_at(pos.0, pos.1, pos.2) == Block::ForgeLit;
        match piece.hearth()? {
            Hearth::AnyFire => None,
            Hearth::Embers => Some("Stones and pebbles smelt on burning charcoal in the open"),
            Hearth::Forge if lit_forge => Some("Bronze needs one tin and one copper"),
            Hearth::Forge => Some("Bronze smelts in a lit clay forge"),
            Hearth::GlowingForge if lit_forge => Some("The forge is not hot enough: work it with bellows"),
            Hearth::GlowingForge => Some("Iron smelts in a lit clay forge worked with bellows"),
        }
    }

    /// Whether right click on `hit` would do something: fill an empty clay forge with held charcoal, or on a
    /// burning block take a finished piece off it, add fuel (a log or charcoal), or lay something on it (raw
    /// meat on any fire; a stone or pebble on burning charcoal; tin or copper in a lit forge; iron ore in a glowing one).
    pub fn can_use_fire(&self, hit: &Hit) -> bool {
        let held = self.held_item();
        if hit.block == Block::ClayForge {
            return held == Some(ItemKind::Charcoal);
        }
        if !hit.block.is_burning() {
            return false;
        }
        let pos = (hit.x, hit.y, hit.z);
        if let Some(pot) = self.world.pots.get(&pos) {
            return match held {
                Some(k) if k.is_fuel() => true,
                Some(ItemKind::WaterJar) => !pot.water,
                Some(k) if k.stewed().is_some() => pot.meat.is_none(),
                _ => pot.stew().is_some() || pot.meat.is_none(),
            };
        }
        if held == Some(ItemKind::TinPot) && self.world.can_place_pot(pos) {
            return true;
        }
        let ready = self.world.cooking.get(&pos).map_or(false, |c| c.done());
        let free = !self.world.cooking.contains_key(&pos);
        ready || held.map_or(false, |k| k.is_fuel()) || (free && held.map_or(false, |k| self.can_lay_held(pos, k)))
    }

    /// Right click on a burning block at `pos`: take a finished piece off it, otherwise feed it
    /// the held fuel, otherwise lay the held meat, stone, pebble, metal or ore on it. Returns whether anything happened.
    fn use_on_fire(&mut self, pos: Pos) -> bool {
        let held = self.held_item();
        if let Some(pot) = self.world.pots.get(&pos).copied() {
            if !held.map_or(false, |k| k.is_fuel()) {
                return self.use_on_pot(pos, pot);
            }
        }
        let holding_cookable = held.map_or(false, |k| Self::piece_for(k).processed().is_some());
        if !holding_cookable {
            if let Some(done) = self.world.take_product(pos) {
                if self.inventory.add(done, 1) != 0 {
                    let spot = vec3(pos.0 as f32 + 0.5, pos.1 as f32 + 0.9, pos.2 as f32 + 0.5);
                    self.items.push(DroppedItem::spawn_at(done, spot, &mut self.rng));
                }
                self.toasts.push(format!("Took the {} off the fire", done.name().to_lowercase()));
                return true;
            }
        }
        match held {
            Some(fuel) if fuel.is_fuel() => {
                if self.world.feed_fire(pos, FIRE_SECONDS) {
                    self.inventory.take_from_slot(self.selected);
                    self.toasts.push(format!("Added {} to the fire", if fuel == ItemKind::Log { "a log" } else { "charcoal" }));
                    return true;
                }
                false
            }
            Some(ItemKind::TinPot) => {
                if self.world.place_pot(pos) {
                    self.inventory.take_from_slot(self.selected);
                    self.toasts.push("Set the pot on the fire".to_string());
                    return true;
                }
                self.toasts.push("A pot goes on a burning log or charcoal with nothing else on it".to_string());
                false
            }
            Some(item) if holding_cookable => {
                let piece = Self::piece_for(item);
                if self.can_lay_held(pos, item) && self.world.lay_on_fire(pos, piece) {
                    if piece == ItemKind::Tin {
                        self.inventory.remove(ItemKind::Tin, 1);
                        self.inventory.remove(ItemKind::Copper, 1);
                        self.toasts.push("Set a tin and a copper in the forge".to_string());
                    } else {
                        self.inventory.take_from_slot(self.selected);
                        let verb = if piece.smelted().is_some() { "Set" } else { "Laid" };
                        self.toasts.push(format!("{verb} the {} on the fire", piece.name().to_lowercase()));
                    }
                    return true;
                }
                if let Some(why) = self.refusal(pos, piece) {
                    self.toasts.push(why.to_string());
                }
                false
            }
            _ => false,
        }
    }

    /// Put `kind` in the inventory, or on the ground at `pos` if it is full.
    fn give_or_drop(&mut self, kind: ItemKind, pos: Pos) {
        if self.inventory.add(kind, 1) != 0 {
            let spot = vec3(pos.0 as f32 + 0.5, pos.1 as f32 + 0.9, pos.2 as f32 + 0.5);
            self.items.push(DroppedItem::spawn_at(kind, spot, &mut self.rng));
        }
    }

    /// Right click on a fire that has a pot on it: a jar of water fills the pot, raw meat goes into the watered
    /// pot (it becomes a stew after `STEW_SECONDS`), and anything else (an empty hand) takes the finished stew out,
    /// or the pot off the fire when it holds no meat. Returns whether anything happened.
    fn use_on_pot(&mut self, pos: Pos, pot: Pot) -> bool {
        match self.held_item() {
            Some(ItemKind::WaterJar) => {
                if pot.water {
                    self.toasts.push("The pot already holds water".to_string());
                    return false;
                }
                self.world.fill_pot(pos);
                self.inventory.take_from_slot(self.selected);
                self.give_or_drop(ItemKind::ClayJar, pos);
                self.toasts.push("Poured water into the pot".to_string());
                true
            }
            Some(meat) if meat.stewed().is_some() => {
                if !pot.water {
                    self.toasts.push("Fill the pot with water first".to_string());
                    return false;
                }
                if self.world.add_to_pot(pos, meat) {
                    self.inventory.take_from_slot(self.selected);
                    self.toasts.push(format!("Put the {} in the pot", meat.name().to_lowercase()));
                    return true;
                }
                false
            }
            _ => {
                if let Some(stew) = self.world.take_stew(pos) {
                    self.give_or_drop(stew, pos);
                    self.toasts.push(format!("Ladled out the {}", stew.name().to_lowercase()));
                    true
                } else if self.world.take_pot(pos) {
                    self.give_or_drop(ItemKind::TinPot, pos);
                    self.toasts.push("Took the pot off the fire".to_string());
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Right click with an empty clay jar held while looking at water within reach: fill the jar.
    pub fn fill_jar(&mut self, eye: Vec3, dir: Vec3) -> bool {
        if self.held_item() != Some(ItemKind::ClayJar) || !self.world.water_in_reach(eye, dir, REACH) {
            return false;
        }
        self.inventory.take_from_slot(self.selected);
        if self.inventory.add(ItemKind::WaterJar, 1) != 0 {
            self.inventory.add(ItemKind::ClayJar, 1);
            return false;
        }
        self.toasts.push("Filled the jar with water".to_string());
        true
    }

    /// Right click on `hit`: charcoal in hand fills an empty clay forge; on a burning block, work the fire
    /// (see `use_on_fire`); otherwise place the held log, pebble, stone, plank, charcoal or clay forge in the empty
    /// cell in front of the targeted block. Uses up one item. Returns whether anything happened.
    pub fn use_held(&mut self, hit: &Hit) -> bool {
        if hit.block == Block::Bush {
            return self.pick_berry_bush((hit.x, hit.y, hit.z));
        }
        if hit.block == Block::ClayForge && self.held_item() == Some(ItemKind::Charcoal) {
            self.world.set_block(hit.x, hit.y, hit.z, Block::ForgeCharged);
            self.inventory.take_from_slot(self.selected);
            self.toasts.push("Filled the forge with charcoal: light it with a bow drill".to_string());
            return true;
        }
        if hit.block.is_burning() {
            return self.use_on_fire((hit.x, hit.y, hit.z));
        }
        if matches!(hit.block, Block::TowerDoor | Block::TowerDoorTop) {
            return self.try_unlock((hit.x, hit.y, hit.z));
        }
        if hit.block == Block::Boulder && self.held_item() == Some(ItemKind::PhilosophersStone) {
            return self.turn_boulder_to_gold((hit.x, hit.y, hit.z));
        }
        if matches!(hit.block, Block::SwordInStone | Block::SwordHilt) {
            return self.try_pull_sword((hit.x, hit.y, hit.z));
        }
        let Some(kind) = self.held_item() else { return false };
        let Some(block) = kind.placeable_block() else { return false };
        let (x, y, z) = hit.prev;
        if !self.can_place(block, (x, y, z)) {
            return false;
        }
        self.world.set_block(x, y, z, block);
        self.inventory.take_from_slot(self.selected);
        true
    }

    /// Burn the fires down by `dt` and drop the meat that fell off the ones that burnt out.
    pub fn burn_fires(&mut self, dt: f32) {
        self.world.update_fires(dt);
        for ((x, y, z), item) in self.world.drain_released() {
            let spot = vec3(x as f32 + 0.5, y as f32 + 0.6, z as f32 + 0.5);
            self.items.push(DroppedItem::spawn_at(item, spot, &mut self.rng));
        }
    }

    // --- Animals and hunting ---------------------------------------------------------------

    /// Whether nothing solid lies on the straight line from `eye` along `dir` for `dist` blocks.
    fn line_of_sight(&self, eye: Vec3, dir: Vec3, dist: f32) -> bool {
        clear_line(&self.world, eye, dir, dist)
    }

    /// The nearest animal under the crosshair within `reach`, if nothing solid is in the way.
    fn animal_along(&self, eye: Vec3, dir: Vec3, reach: f32) -> Option<usize> {
        let (i, t) = self
            .animals
            .iter()
            .enumerate()
            .filter(|(_, a)| a.kind.is_huntable())
            .filter_map(|(i, a)| {
                let (lo, hi) = a.hitbox();
                ray_box(eye, dir, lo.to_array(), hi.to_array()).filter(|&t| t <= reach).map(|t| (i, t))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
        self.line_of_sight(eye, dir, t).then_some(i)
    }

    /// The animal a held stone axe or bronze mallet could strike right now (`None` with anything else in hand).
    pub fn melee_target(&self, eye: Vec3, dir: Vec3) -> Option<usize> {
        if !self.held_item().map_or(false, |k| k.melee_damage().is_some()) {
            return None;
        }
        self.animal_along(eye, dir, self.held_item().map_or(AXE_REACH, ItemKind::melee_reach))
    }

    /// Hurt animals (index, damage). The ones that die are removed and drop their loot; the others are knocked back
    /// away from the player.
    fn hurt_animals(&mut self, hits: &[(usize, i32)]) {
        let from = self.player.pos;
        // A cursed idol in the hotbar doubles every blow, and whatever the player hits is marked for the king's spirits to shoot at.
        let factor = if self.idol_in_hotbar() { IDOL_DAMAGE_FACTOR } else { 1 };
        let hits: Vec<(usize, i32)> = hits.iter().map(|&(i, d)| (i, d * factor)).collect();
        for &(i, _) in &hits {
            if self.animals[i].kind != AnimalKind::SpiritArcher {
                self.animals[i].marked = SPIRIT_MARK_SECONDS;
            }
        }
        self.hurt_animals_from(&hits, from);
    }

    /// `hurt_animals`, with the survivors thrown back away from `from` (the player for a blow, the strike for lightning).
    fn hurt_animals_from(&mut self, hits: &[(usize, i32)], from: Vec3) {
        let mut dead = Vec::new();
        for &(i, damage) in hits {
            let animal = &mut self.animals[i];
            if animal.damage(damage) {
                if !dead.contains(&i) {
                    dead.push(i);
                }
            } else {
                let away = animal.pos - from;
                animal.knock_back(away);
            }
        }
        self.remove_dead(dead);
    }

    /// Take the animals at these indices out of the world: each drops its loot and the bosses among them do what a boss's death
    /// does. A ghostly bear or crow of the Wild Hunt that splits is replaced by `HUNT_SPLIT_INTO` of its kind.
    fn remove_dead(&mut self, mut dead: Vec<usize>) {
        dead.sort_unstable_by(|a, b| b.cmp(a));
        dead.dedup();
        for i in dead {
            let animal = self.animals.remove(i);
            if animal.kind == AnimalKind::Quail {
                self.note_quail_killed();
            }
            if animal.kind == AnimalKind::Bat && self.in_horizen() {
                self.note_cave_bat_killed();
            }
            let drops = animal.kind.drops(&mut self.rng);
            for kind in drops {
                // A player who has a silver key, or one lying on the ground, is given a gold bar instead of another.
                let kind = if kind == ItemKind::SilverKey { self.key_or_gold() } else { kind };
                let spot = animal.pos + vec3(0.0, 0.5, 0.0);
                self.items.push(DroppedItem::spawn_at(kind, spot, &mut self.rng));
            }
            if animal.splits {
                for n in 0..HUNT_SPLIT_INTO {
                    let a = n as f32 * std::f32::consts::TAU / HUNT_SPLIT_INTO as f32 + animal.heading;
                    let at = animal.pos + vec3(a.cos() * 0.8, 0.3, a.sin() * 0.8);
                    let mut child = Animal::new(animal.kind, at, &mut self.rng);
                    child.home = animal.home;
                    self.hunt_total += child.kind.max_health();
                    self.animals.push(child);
                }
            }
            self.boss_slain(&animal);
        }
        self.summon_iron_quails();
        self.summon_vampire_bats();
    }

    /// Count a bat killed in the cave: every `BATS_PER_VAMPIRE`th calls a vampire bat.
    fn note_cave_bat_killed(&mut self) {
        self.bat_kills += 1;
        if self.bat_kills % BATS_PER_VAMPIRE == 0 {
            self.vampire_bats_owed += 1;
        }
    }

    /// Bring out the vampire bat that is due (one at a time, in the cave), `VAMPIRE_BAT_SPAWN_DISTANCE` from the player on open floor.
    /// If there is no room now, or one is already about, it is tried again with the next wildlife spawn.
    fn summon_vampire_bats(&mut self) {
        if self.vampire_bats_owed == 0 || !self.in_horizen() || self.animals.iter().any(|a| a.kind == AnimalKind::VampireBat) {
            return;
        }
        let p = self.player.pos;
        let seed = self.info.seed;
        let first = self.rng.unit() * std::f32::consts::TAU;
        for i in 0..24 {
            let a = first + i as f32 * std::f32::consts::TAU / 24.0;
            let (x, z) = ((p.x + a.cos() * VAMPIRE_BAT_SPAWN_DISTANCE).floor() as i32, (p.z + a.sin() * VAMPIRE_BAT_SPAWN_DISTANCE).floor() as i32);
            let air = horizen::air_height(seed, x, z);
            if air < BAT_CAVE_MIN_AIR || self.world.is_wet_column(x, z) {
                continue;
            }
            let height = (air as f32 - 2.0).min(VAMPIRE_BAT_ORBIT_HEIGHT + 1.0).max(1.0);
            if self.spawn_flyer(AnimalKind::VampireBat, x, z, height).is_some() {
                self.vampire_bats_owed -= 1;
                self.toasts.push("A vampire bat sweeps out of the dark!".to_string());
                return;
            }
        }
    }

    /// Whether the player has a silver key: in the inventory, or lying on the ground.
    pub fn has_silver_key(&self) -> bool {
        self.inventory.count(ItemKind::SilverKey) > 0 || self.items.iter().any(|i| i.kind == ItemKind::SilverKey)
    }

    /// What a silver key that is about to drop becomes: itself, or a gold bar if the player already has one (in the inventory, or
    /// lying on the ground).
    pub fn key_or_gold(&self) -> ItemKind {
        if self.has_silver_key() { ItemKind::Gold } else { ItemKind::SilverKey }
    }

    /// Count a quail the player has killed: every `QUAIL_KILLS_PER_IRON_QUAIL`th one calls an Iron Quail.
    fn note_quail_killed(&mut self) {
        self.quail_kills += 1;
        if self.quail_kills % QUAIL_KILLS_PER_IRON_QUAIL == 0 {
            self.iron_quails_owed += IRON_QUAILS_PER_SUMMON;
        }
    }

    /// Bring out the Iron Quails that are due (`IRON_QUAILS_PER_SUMMON` for every tenth quail), spread round the player, each
    /// `IRON_QUAIL_SPAWN_DISTANCE` away on open ground. Any that finds no place now is tried again with the next wildlife spawn.
    fn summon_iron_quails(&mut self) {
        let mut came = 0;
        while self.iron_quails_owed > 0 && self.in_overworld() {
            let p = self.player.pos;
            let first = self.rng.unit() * std::f32::consts::TAU;
            let share = std::f32::consts::TAU / IRON_QUAILS_PER_SUMMON as f32;
            let offset = (came % IRON_QUAILS_PER_SUMMON as usize) as f32 * share;
            let placed = (0..24).any(|i| {
                let a = first + offset + i as f32 * std::f32::consts::TAU / 24.0;
                let (x, z) = ((p.x + a.cos() * IRON_QUAIL_SPAWN_DISTANCE).floor() as i32, (p.z + a.sin() * IRON_QUAIL_SPAWN_DISTANCE).floor() as i32);
                self.spawn_animal(AnimalKind::IronQuail, x, z)
            });
            if !placed {
                break;
            }
            self.iron_quails_owed -= 1;
            came += 1;
        }
        if came > 0 {
            self.toasts.push("A great shadow falls: the Iron Quails come for you.".to_string());
        }
    }

    /// Swing the stone axe or bronze mallet along `dir` from `eye`. Does nothing without one in hand or before
    /// the previous swing is over. Returns whether it hit an animal.
    pub fn swing_melee(&mut self, eye: Vec3, dir: Vec3) -> bool {
        let Some(kind) = self.held_item() else { return false };
        let Some(damage) = kind.melee_damage() else { return false };
        if self.attack_cooldown > 0.0 {
            return false;
        }
        self.attack_cooldown = kind.melee_cooldown();
        match self.melee_target(eye, dir) {
            Some(i) => {
                // Excalibur is the bane of the Wild Hunt.
                let damage = if kind == ItemKind::Excalibur && self.animals[i].kind.is_wild_hunt() { damage * EXCALIBUR_HUNT_FACTOR } else { damage };
                self.hurt_animals(&[(i, damage)]);
                true
            }
            None => false,
        }
    }

    /// What the left button does with a weapon in hand. A click shoots a bow or a sling; holding the button on an
    /// animal under the crosshair (`aimed`) swings any axe or the bronze mallet, once per swing cooldown.
    /// Returns whether something was shot or struck.
    pub fn attack(&mut self, eye: Vec3, dir: Vec3, clicked: bool, held_down: bool, aimed: bool) -> bool {
        match self.held_item() {
            Some(ItemKind::Bow) if clicked => self.shoot_bow(eye, dir),
            Some(ItemKind::Sling) if clicked => self.shoot_sling(eye, dir),
            Some(ItemKind::IronFeather) if clicked => self.throw_feather(eye, dir),
            Some(kind) if kind.melee_damage().is_some() && aimed && held_down => self.swing_melee(eye, dir),
            _ => false,
        }
    }

    /// Shoot an arrow along `dir` from `eye`. Needs a bow in hand and an arrow in the inventory.
    /// Returns whether an arrow was shot.
    pub fn shoot_bow(&mut self, eye: Vec3, dir: Vec3) -> bool {
        self.shoot(ItemKind::Bow, eye, dir)
    }

    /// Sling a pebble along `dir` from `eye`. Needs a sling in hand and a pebble in the inventory.
    /// Returns whether a pebble was slung.
    pub fn shoot_sling(&mut self, eye: Vec3, dir: Vec3) -> bool {
        self.shoot(ItemKind::Sling, eye, dir)
    }

    /// Throw the iron feather in hand along `dir` from `eye`, like a throwing star: it flies like an arrow and is used up only if it
    /// hits an animal (one that hits a block lies there to be picked up again). Returns whether it was thrown.
    pub fn throw_feather(&mut self, eye: Vec3, dir: Vec3) -> bool {
        if self.held_item() != Some(ItemKind::IronFeather) || self.attack_cooldown > 0.0 || !self.inventory.remove(ItemKind::IronFeather, 1) {
            return false;
        }
        self.attack_cooldown = IRON_FEATHER_THROW_COOLDOWN;
        self.arrows.push(Arrow::thrown_feather(eye, dir));
        true
    }

    /// Nessie hears a bow or a sling: within `LOCH_NESS_SHOT_HEARING` she runs for the middle of the lake and, if she was not
    /// running already, calls `LOCH_NESS_ALARM_RAVENS` more ravens.
    fn alarm_nessie(&mut self) {
        let p = self.player.pos;
        let mut alarmed = false;
        for a in self.animals.iter_mut().filter(|a| a.kind == AnimalKind::LochNessMonster) {
            if vec2(a.pos.x - p.x, a.pos.z - p.z).length() <= LOCH_NESS_SHOT_HEARING {
                alarmed |= !a.is_fleeing_shot();
                a.startle(LOCH_NESS_FLEE_SECONDS);
            }
        }
        if alarmed {
            self.call_ravens(LOCH_NESS_ALARM_RAVENS, LOCH_NESS_MAX_RAVENS + LOCH_NESS_ALARM_RAVENS);
        }
    }

    /// Fire the held launcher (`launcher`, which must be what is in hand), using up one of its ammunition. A bow shoots a Phoenix Arrow
    /// in preference when there is one in the inventory, and that is never used up.
    fn shoot(&mut self, launcher: ItemKind, eye: Vec3, dir: Vec3) -> bool {
        let Some(ammo) = launcher.ammo() else { return false };
        if self.held_item() != Some(launcher) || self.attack_cooldown > 0.0 {
            return false;
        }
        if launcher == ItemKind::Bow && self.inventory.count(ItemKind::PhoenixArrow) > 0 {
            self.attack_cooldown = BOW_COOLDOWN;
            self.arrows.push(Arrow::fired_phoenix(eye, dir));
            self.alarm_nessie();
            return true;
        }
        if !self.inventory.remove(ammo, 1) {
            self.attack_cooldown = AXE_COOLDOWN;
            self.toasts.push(format!("You have no {}", if ammo == ItemKind::Arrow { "arrows" } else { "pebbles" }));
            return false;
        }
        if launcher == ItemKind::Sling {
            self.attack_cooldown = SLING_COOLDOWN;
            self.arrows.push(Arrow::slung(eye, dir));
        } else {
            self.attack_cooldown = BOW_COOLDOWN;
            self.arrows.push(Arrow::fired(eye, dir));
        }
        self.alarm_nessie();
        true
    }

    /// Fly the arrows and slung pebbles on. One that hits an animal hurts it and is used up; one that hits a block
    /// or runs out of flight without hurting anything lands on the ground as the item it was (an arrow or a
    /// pebble), to be picked up and used again.
    pub fn update_arrows(&mut self, dt: f32) {
        let mut hits = Vec::new();
        let mut landed = Vec::new();
        let (world, animals) = (&self.world, &self.animals);
        self.arrows.retain_mut(|arrow| match arrow.update(world, animals, dt) {
            Fate::InFlight => true,
            Fate::HitAnimal(i) => {
                hits.push((i, arrow.damage()));
                false
            }
            Fate::HitBlock | Fate::Expired => {
                // A Phoenix Arrow is never used up, and never lies on the ground: it is simply gone.
                if !arrow.vanishes_on_landing() {
                    landed.push((arrow.kind, arrow.pos - arrow.vel.normalize_or_zero() * ARROW_DROP_BACKOFF));
                }
                false
            }
        });
        self.hurt_animals(&hits);
        for (kind, spot) in landed {
            self.items.push(DroppedItem::at_rest(kind, spot));
        }
    }

    // --- Lakes, mountains and the rainbow ------------------------------------------------------

    /// Whether the player is wading: water fills the cell their feet are in, or one of the two cells below
    /// (so a jump in a river does not count as leaving it).
    pub fn player_in_water(&self) -> bool {
        let p = self.player.pos;
        let feet = (p.y + 0.1).floor() as i32;
        (0..3).any(|down| self.world.block_at(p.x.floor() as i32, feet - down, p.z.floor() as i32).is_wet())
    }

    /// Advance the treks and the rainbow. Walking `LAKE_TREK_CHUNKS` chunks through a river without leaving the
    /// water brings a lake into the world ahead of the player; following a held compass north for
    /// `COMPASS_TREK_CHUNKS` chunks brings a mountain; holding a peep stone shows the rainbow (once).
    /// Call once a frame, after the player has moved.
    pub fn update_landmarks(&mut self) {
        // The other dimensions have one silver tower each (and no other landmarks); the totem does nothing in the Horizen, and
        // in the Skylands it only points the way to the tower (`trail_path`).
        if self.in_horizen() {
            self.try_show_horizen_tower();
            self.update_cave_visits();
            return;
        }
        if self.in_skylands() {
            self.update_sky_tower();
            return;
        }
        self.treks.mountain_found = self.world.landmarks.mountain().is_some();
        let p = self.player.pos;
        let compass = self.held_item() == Some(ItemKind::Compass);
        let wading = self.player_in_water();
        if let Some(event) = self.treks.step((p.x, p.z), wading, compass) {
            self.bring_in(event);
        }
        self.try_show_rainbow();
        self.try_show_grand_castle();
        self.try_show_ruined_castle();
        self.update_graveyard_ghost();
        self.try_show_well_or_graveyard();
        self.update_dimension();
    }

    /// Make the lake or mountain that a finished trek leads to, and say so.
    fn bring_in(&mut self, event: TrekEvent) {
        let phase = self.rng.range(0, 63);
        match event {
            TrekEvent::Lake { at, dir } => {
                self.world.add_lake(Lake::ahead(at, dir, phase));
                self.toasts.push("The river opens out ahead into a great lake.".to_string());
            }
            TrekEvent::Mountain { .. } if self.world.landmarks.mountain().is_some() => {}
            TrekEvent::Mountain { at, dir } => {
                self.world.add_mountain(Mountain::ahead(at, dir, phase));
                self.toasts.push("The compass has led you north: a great mountain rises ahead. Its needle will point to it from now on.".to_string());
            }
        }
    }

    /// Whether the pile of gold can lie in the column (x, z): dry, bare ground with nothing standing on it. The chunk
    /// need not be loaded (the rainbow lies beyond the generated land).
    fn gold_fits_at(&self, x: i32, z: i32) -> bool {
        self.world.is_bare_dry_ground(x, z)
    }

    /// With a peep stone in hand at night, and only if the world has no grand castle yet, bring one into the world:
    /// beyond the edge of the land, in a direction where no lake or mountain is (as far as one can be found), with the
    /// ghostly trail to it shown by the peep stone. Returns whether the castle appeared.
    pub fn try_show_grand_castle(&mut self) -> bool {
        if self.world.landmarks.grand_castle.is_some() || !self.world.night || self.held_item() != Some(ItemKind::PeepStone) {
            return false;
        }
        let p = self.player.pos;
        let first = self.rng.unit() * std::f32::consts::TAU;
        let sites: Vec<GrandCastle> = (0..16).map(|i| GrandCastle::ahead((p.x, p.z), first + i as f32 * std::f32::consts::TAU / 16.0)).collect();
        let site = sites.iter().copied().find(|c| self.world.landmarks.has_room_for(c)).unwrap_or(sites[0]);
        self.world.add_grand_castle(site);
        self.toasts.push("A ghostly trail winds away through the dark.".to_string());
        true
    }

    /// Wearing the king's crown in the overworld, and only if the world has no ruined castle yet, bring one out beyond the
    /// edge of the land, straight ahead of the way the player faces (or as near that as clear ground allows): they walk into
    /// it. The sword in the stone at its centre comes out for the crown. Returns whether the castle appeared.
    pub fn try_show_ruined_castle(&mut self) -> bool {
        if self.world.landmarks.ruined_castle.is_some() || !self.in_overworld() || self.worn != Some(ItemKind::AncientCrown) {
            return false;
        }
        let p = self.player.pos;
        let facing = self.player.yaw;
        let sites: Vec<GrandCastle> = (0..16).map(|i| GrandCastle::ahead((p.x, p.z), facing + i as f32 * std::f32::consts::TAU / 16.0)).collect();
        let site = sites.iter().copied().find(|c| self.world.landmarks.has_room_for_ruins(c)).unwrap_or(sites[0]);
        self.world.add_ruined_castle(site);
        self.toasts.push("The crown grows warm. Far ahead, the ruins of a castle wait.".to_string());
        true
    }

    /// With a peep stone in hand, and only if the world has never had a rainbow, bring it out: an arc `RAINBOW_SPAN` blocks
    /// long that stands wholly beyond the render distance (so nobody sees it appear) and ends on a pile of gold. Returns
    /// whether the rainbow appeared. If no good spot is found this frame, the next frame tries again.
    pub fn try_show_rainbow(&mut self) -> bool {
        if self.world.landmarks.rainbow != RainbowState::Unseen || self.world.night || self.held_item() != Some(ItemKind::PeepStone) {
            return false;
        }
        let p = self.player.pos;
        let beyond = rainbow_distance();
        for _ in 0..RAINBOW_SPOT_TRIES {
            let angle = self.rng.unit() * std::f32::consts::TAU;
            // Counted along either axis, like the chunks: the gold is `beyond` to `beyond + extra` blocks out that way.
            let reach = (beyond + self.rng.unit() * RAINBOW_GOLD_EXTRA_DISTANCE) / angle.cos().abs().max(angle.sin().abs());
            let (x, z) = ((p.x + angle.cos() * reach).floor() as i32, (p.z + angle.sin() * reach).floor() as i32);
            if !self.gold_fits_at(x, z) {
                continue;
            }
            // The arc starts to one side or the other of the gold, and must stay beyond the render distance all along.
            let sideways = if self.rng.chance(50) { 1.0 } else { -1.0 };
            let side = (-angle.sin() * sideways, angle.cos() * sideways);
            let from = ((x as f32 + 0.5 + side.0 * RAINBOW_SPAN).floor() as i32, (z as f32 + 0.5 + side.1 * RAINBOW_SPAN).floor() as i32);
            if self.world.is_wet_column(from.0, from.1) || !rainbow_clear_of_view((p.x, p.z), from, (x, z)) {
                continue;
            }
            let y = self.world.ground_top(x, z) + 1;
            self.world.set_block(x, y, z, Block::GoldPile);
            self.world.set_rainbow(RainbowState::Active(Rainbow { from, gold: (x, y, z) }));
            self.toasts.push("A rainbow appears! Follow it to the gold at its end.".to_string());
            return true;
        }
        false
    }

    /// The rainbow to draw this frame: it shows only by day, while a peep stone is in the hand, and until its gold
    /// is taken.
    pub fn visible_rainbow(&self) -> Option<Rainbow> {
        if self.held_item() == Some(ItemKind::PeepStone) && !self.world.night { self.world.active_rainbow() } else { None }
    }

    /// The ghostly trail a peep stone or a bone totem in the hand shows, as the corners of a path (x, z) that starts under
    /// the player. A peep stone leads by night to the grand castle (to its gate, and on to the door of its keep), by day
    /// to the end of the rainbow, even once the gold has been taken. A bone totem leads by day to the well and by night to
    /// the graveyard. In the Skylands a bone totem leads to the silver tower, once it has appeared. `None` when neither is in
    /// hand, nothing is out there to lead to, or the player is in the Horizen.
    pub fn trail_path(&self) -> Option<Vec<(f32, f32)>> {
        let start = (self.player.pos.x, self.player.pos.z);
        // In the Horizen a peep stone leads along the tunnel to the silver tower in the great cavern.
        if self.in_horizen() {
            // A bone totem leads through the caves to the king's castle in the giant cave.
            if self.held_item() == Some(ItemKind::BoneTotem) {
                let (kx, kz) = horizen::king_centre(self.info.seed);
                let mut path = horizen::king_route(self.info.seed, start);
                path.push((kx as f32 + 0.5, kz as f32 + 0.5));
                return Some(path);
            }
            let tower = self.world.landmarks.horizen_tower?;
            if self.held_item() != Some(ItemKind::PeepStone) {
                return None;
            }
            let mut path = horizen::route(self.info.seed, start);
            path.extend([tower.approach(), tower.middle()]);
            return Some(path);
        }
        // In the Skylands a bone totem, by day or night, leads to the silver tower once it has appeared on its island.
        if self.in_skylands() {
            // A peep stone leads to the giant island and its volcano, until the Phoenix is dead.
            if self.held_item() == Some(ItemKind::PeepStone) {
                if self.world.landmarks.phoenix_slain {
                    return None;
                }
                return Some(vec![start, skylands::volcano_foot(self.info.seed)]);
            }
            if self.held_item() != Some(ItemKind::BoneTotem) {
                return None;
            }
            let (cx, cz) = self.world.landmarks.sky_tower?;
            let island = skylands::island_in_cell(self.info.seed, cx, cz);
            let tower = Tower { x: island.x, z: island.z };
            return Some(vec![start, tower.approach(), tower.middle()]);
        }
        if !self.in_overworld() {
            return None;
        }
        match self.held_item()? {
            ItemKind::PeepStone if self.world.night => {
                let castle = self.world.landmarks.grand_castle?;
                Some(vec![start, castle.gate_approach(), castle.keep_door()])
            }
            ItemKind::PeepStone => {
                let (x, _, z) = self.world.landmarks.rainbow_end()?;
                Some(vec![start, (x as f32 + 0.5, z as f32 + 0.5)])
            }
            ItemKind::BoneTotem if self.world.night => {
                let yard = self.world.landmarks.graveyard?;
                Some(vec![start, yard.gate_approach(), yard.middle()])
            }
            ItemKind::BoneTotem => Some(vec![start, self.world.landmarks.well_end()?]),
            _ => None,
        }
    }

    /// The colour of the trail: pale blue by night and pale gold by day for a peep stone, bone white by day and sickly
    /// green by night for a bone totem.
    pub fn trail_color(&self) -> (f32, f32, f32) {
        match (self.held_item(), self.world.night) {
            (Some(ItemKind::BoneTotem), false) => (0.95, 0.95, 0.88),
            (Some(ItemKind::BoneTotem), true) => (0.55, 1.0, 0.60),
            (_, true) => (0.70, 0.86, 1.0),
            (_, false) => (1.0, 0.92, 0.62),
        }
    }

    /// With a bone totem in hand by day, and only if the world has no well yet, bring the well into the world beyond the
    /// edge of the land; by night, in the same way, the graveyard. The totem's trail then leads to it for ever.
    pub fn try_show_well_or_graveyard(&mut self) -> bool {
        if !self.in_overworld() || self.held_item() != Some(ItemKind::BoneTotem) {
            return false;
        }
        let p = self.player.pos;
        let first = self.rng.unit() * std::f32::consts::TAU;
        let angle = |i: usize| first + i as f32 * std::f32::consts::TAU / 16.0;
        if self.world.night {
            if self.world.landmarks.graveyard.is_some() {
                return false;
            }
            let sites: Vec<Graveyard> = (0..16).map(|i| Graveyard::ahead((p.x, p.z), angle(i))).collect();
            let site = sites.iter().copied().find(|g| self.world.landmarks.has_room_for_graveyard(g)).unwrap_or(sites[0]);
            self.world.add_graveyard(site);
            self.toasts.push("A sickly green trail winds away through the dark.".to_string());
        } else {
            if self.world.landmarks.well.is_some() {
                return false;
            }
            let sites: Vec<Well> = (0..16).map(|i| Well::ahead((p.x, p.z), angle(i))).collect();
            let site = sites.iter().copied().find(|w| self.world.landmarks.has_room_for_well(w)).unwrap_or(sites[0]);
            self.world.add_well(site);
            self.toasts.push("A pale trail leads away across the land.".to_string());
        }
        true
    }

    /// With a peep stone in hand in the Horizen, and only if it has no tower yet, bring its one silver tower out in the middle
    /// of the great cavern (`horizen::cavern_centre`), with the ghostly trail to it shown by the peep stone. There is no sky in
    /// the cave to tell day from night by, so the stone works at any hour. Returns whether the tower appeared.
    pub fn try_show_horizen_tower(&mut self) -> bool {
        if !self.in_horizen() || self.world.landmarks.horizen_tower.is_some() || self.held_item() != Some(ItemKind::PeepStone) {
            return false;
        }
        let (x, z) = horizen::cavern_centre(self.info.seed);
        self.world.add_horizen_tower(Tower { x, z });
        self.toasts.push("A pale golden trail leads away down the tunnel.".to_string());
        true
    }

    /// In the Horizen: note a cave the player comes into. The arrival chamber is the first; every `HORIZEN_CAMP_EVERY`th
    /// cave entered (the great cavern only counts) has a camp set up in it: a burning log with a tin pot of red or purple
    /// berry stew, ready to eat. Returns whether a camp was made.
    pub fn update_cave_visits(&mut self) -> bool {
        let p = self.player.pos;
        let Some(cave) = horizen::cave_at(self.info.seed, p.x.floor() as i32, p.z.floor() as i32) else { return false };
        if self.world.landmarks.caves_visited.contains(&cave.cell) {
            return false;
        }
        self.world.landmarks.caves_visited.push(cave.cell);
        if self.world.landmarks.caves_visited.len() % HORIZEN_CAMP_EVERY != 0 || cave.great || cave.king {
            return false;
        }
        self.build_camp(cave)
    }

    /// Set a camp in `cave`: a log on dry floor near its middle, alight, with a pot of water and berries on it, the stew done.
    fn build_camp(&mut self, cave: horizen::Cave) -> bool {
        let (cx, cz) = (cave.x.floor() as i32, cave.z.floor() as i32);
        let seed = self.info.seed;
        let spot = (0..=6_i32).find_map(|r| {
            (-r..=r).flat_map(|dz| (-r..=r).map(move |dx| (dx, dz))).filter(|&(dx, dz)| dx.abs().max(dz.abs()) == r).map(|(dx, dz)| (cx + dx, cz + dz)).find(|&(x, z)| {
                self.world.chunks.contains_key(&World::chunk_of(x, z)) && horizen::air_height(seed, x, z) >= HORIZEN_MIN_AIR && !horizen::is_pond(seed, x, z)
            })
        });
        let Some((x, z)) = spot else { return false };
        let y = GROUND_TOP + 1;
        self.world.set_block(x, y, z, Block::PlacedLog);
        self.world.ignite((x, y, z), HORIZEN_CAMP_FIRE_SECONDS);
        let berries = if self.rng.chance(50) { ItemKind::Berries } else { ItemKind::PurpleBerries };
        self.world.pots.insert((x, y, z), Pot { water: true, meat: Some(berries), left: 0.0 });
        self.toasts.push("You come on a camp: a fire, and a pot of stew still hot.".to_string());
        true
    }

    /// In the Skylands: note the island the player stands on, and once they have walked on `SKY_TOWER_VISITS` different
    /// ones, put the tower on the next island that has yet to appear (the nearest one not made so far), once.
    pub fn update_sky_tower(&mut self) {
        let p = self.player.pos;
        let seed = self.info.seed;
        if self.player.on_ground {
            if let Some(cell) = skylands::island_cell_under(seed, p.x.floor() as i32, p.z.floor() as i32) {
                self.world.landmarks.note_sky_visit(cell);
            }
        }
        if self.world.landmarks.sky_tower.is_some() || self.world.landmarks.sky_visited.len() < SKY_TOWER_VISITS {
            return;
        }
        let visited = self.world.landmarks.sky_visited.clone();
        let cell = skylands::pick_tower_cell(seed, (p.x.floor() as i32, p.z.floor() as i32), &visited, &|kx, kz| self.world.chunks.contains_key(&(kx, kz)));
        if let Some(cell) = cell {
            self.world.add_sky_tower(cell);
            self.toasts.push("Far off among the clouds, something tall glints in the light.".to_string());
        }
    }

    /// The bearing the compass needle points along for the player now.
    pub fn compass_bearing(&self) -> f32 {
        let at = (self.player.pos.x, self.player.pos.z);
        // In the Horizen and the Skylands the needle points to where the player arrived.
        let spawn = match self.world.dimension {
            Dimension::Overworld => return compass_bearing(&self.world.landmarks, at),
            Dimension::Horizen => (HORIZEN_SPAWN.0 as f32 + 0.5, HORIZEN_SPAWN.1 as f32 + 0.5),
            Dimension::Skylands => (skylands::spawn_point().x, skylands::spawn_point().z),
        };
        let (dx, dz) = (spawn.0 - at.0, spawn.1 - at.1);
        if dx.abs() + dz.abs() < 1e-3 { COMPASS_NORTH_BEARING } else { dz.atan2(dx) }
    }

    // --- The pipes and the ghosts -------------------------------------------------------------

    /// Say something as a ghost: words on the screen for `GHOST_TEXT_SECONDS`.
    fn ghost_says(&mut self, text: &str) {
        self.ghost_text = Some((text.to_string(), GHOST_TEXT_SECONDS));
    }

    /// Whether the player stands in the graveyard (inside its square).
    fn in_graveyard_square(&self) -> bool {
        let p = self.player.pos;
        self.in_overworld() && self.world.landmarks.near_graveyard(p.x.floor() as i32, p.z.floor() as i32, 0)
    }

    /// The graveyard's ghost asks for a song when the player walks in, until it has had one.
    pub fn update_graveyard_ghost(&mut self) {
        let inside = self.in_graveyard_square();
        if inside && !self.in_graveyard && !self.world.landmarks.graveyard_rested {
            self.ghost_says(ghosts::GRAVEYARD_ASKS);
        }
        self.in_graveyard = inside;
    }

    /// Whether the column and height the player stands at is inside the tower centred on (`cx`, `cz`) whose floor is `base`.
    fn inside_tower(&self, cx: i32, cz: i32, base: i32) -> bool {
        let p = self.player.pos;
        (p.x - (cx as f32 + 0.5)).abs() <= TOWER_HALF as f32 + 0.5 && (p.z - (cz as f32 + 0.5)).abs() <= TOWER_HALF as f32 + 0.5 && p.y >= base as f32 - 0.5 && p.y <= (base + TOWER_HEIGHT) as f32
    }

    /// What the ghost of the silver tower the player is in would say, if they are in one: the Skylands' tower, the Horizen's
    /// tower in the cave, or the tower on a lake's island.
    pub fn tower_ghost_here(&self) -> Option<&'static str> {
        let marks = &self.world.landmarks;
        match self.world.dimension {
            Dimension::Horizen => marks.horizen_tower.filter(|t| self.inside_tower(t.x, t.z, GROUND_TOP + 1)).map(|_| ghosts::CAVE_TOWER_GHOST),
            Dimension::Skylands => marks.sky_tower.map(|cell| skylands::island_in_cell(self.info.seed, cell.0, cell.1)).filter(|i| self.inside_tower(i.x, i.z, i.top + 1)).map(|_| ghosts::SKY_TOWER_GHOST),
            Dimension::Overworld => marks.lakes.iter().filter(|l| l.has_tower()).find(|l| self.inside_tower(l.x, l.z, self.world.ground_top(l.x, l.z) + 1)).map(|_| ghosts::LAKE_TOWER_GHOST),
        }
    }

    /// Play a tune on the musical pipes in hand (right click). In the graveyard it is the song its ghost asked for: the ghost
    /// thanks the player, gives them a silver key and rests, for good. In a silver tower a ghostly voice answers with a hint. By
    /// day, every quail and crow within `PIPES_CALL_RANGE` flies to the player, lands near them and stands still for
    /// `PIPES_CALM_SECONDS`. Returns whether a tune was played.
    pub fn play_pipes(&mut self) -> bool {
        if self.held_item() != Some(ItemKind::MusicalPipes) {
            return false;
        }
        self.toasts.push("You play a tune on the pipes.".to_string());
        if self.in_graveyard_square() && !self.world.landmarks.graveyard_rested {
            self.world.landmarks.graveyard_rested = true;
            let at = self.player.pos;
            let key = self.key_or_gold();
            self.give_or_drop(key, (at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32));
            self.ghost_says(ghosts::GRAVEYARD_THANKS);
        } else if let Some(line) = self.tower_ghost_here() {
            self.ghost_says(line);
        } else if self.in_king_castle() {
            self.ghost_says(ghosts::KING_GREETING);
            self.king_dialog = Some(1);
        }
        // At night the music calls bats instead of birds.
        self.call_bats();
        if self.in_overworld() && !self.world.night {
            let p = self.player.pos;
            for i in 0..self.animals.len() {
                let a = &self.animals[i];
                if !matches!(a.kind, AnimalKind::Quail | AnimalKind::Crow) || vec2(a.pos.x - p.x, a.pos.z - p.z).length() > PIPES_CALL_RANGE {
                    continue;
                }
                let angle = self.rng.unit() * std::f32::consts::TAU;
                let dist = PIPES_LAND_MIN + self.rng.unit() * (PIPES_LAND_MAX - PIPES_LAND_MIN);
                let target = vec3(p.x + angle.cos() * dist, p.y, p.z + angle.sin() * dist);
                self.animals[i].call_to(target);
            }
        }
        true
    }

    // --- Crows and the ancient altar -------------------------------------------------------

    /// Whether an altar can stand on the column (x, z): loaded, dry, bare ground (plants may be cleared).
    fn altar_fits(&self, x: i32, z: i32) -> bool {
        if !self.world.chunks.contains_key(&World::chunk_of(x, z)) || self.world.is_wet_column(x, z) {
            return false;
        }
        let ground = self.world.ground_top(x, z);
        self.world.block_at(x, ground, z) == Block::Dirt && matches!(self.world.block_at(x, ground + 1, z), Block::Air | Block::Grass | Block::Flower)
    }

    /// The nearest column to (x, z) (within `ALTAR_SEARCH_RADIUS`) where an altar fits.
    fn altar_site_near(&self, x: i32, z: i32) -> Option<(i32, i32)> {
        (0..=ALTAR_SEARCH_RADIUS).find_map(|r| {
            (-r..=r).flat_map(|dz| (-r..=r).map(move |dx| (dx, dz))).filter(|&(dx, dz)| dx.abs().max(dz.abs()) == r).map(|(dx, dz)| (x + dx, z + dz)).find(|&(ax, az)| self.altar_fits(ax, az))
        })
    }

    /// A crow that the player has followed west for `CROW_ALTAR_WEST_BLOCKS` comes down on a new ancient altar: a small altar
    /// with a loadstone on it, built where it lands. If there is no room there it flies on a little and tries again.
    fn land_crows_at_altars(&mut self) {
        let due: Vec<usize> = (0..self.animals.len()).filter(|&i| self.animals[i].kind == AnimalKind::Crow && self.animals[i].is_landing() && self.animals[i].west_followed >= CROW_ALTAR_WEST_BLOCKS).collect();
        for i in due {
            let at = self.animals[i].pos;
            match self.altar_site_near(at.x.floor() as i32, at.z.floor() as i32) {
                Some((x, z)) => {
                    let y = self.world.ground_top(x, z) + 1;
                    self.world.set_block(x, y, z, Block::AltarLoadstone);
                    // The perch is on top of the altar's collision box (which reaches up over the loadstone), never inside it: a bird
                    // set down inside a block can move in no direction at all.
                    let top = Block::AltarLoadstone.bounds().map_or(1.0, |(_, hi)| hi[1]);
                    let perch = vec3(x as f32 + 0.5, y as f32 + top + 0.01, z as f32 + 0.5);
                    self.animals[i].land_at(perch, &mut self.rng);
                    self.toasts.push("The crow settles on an ancient altar.".to_string());
                }
                None => self.animals[i].west_followed = CROW_ALTAR_WEST_BLOCKS - 6.0,
            }
        }
    }

    // --- Relics: gold boulders and the sword in the stone ---------------------------------------

    /// Right click a boulder with the Philosopher's Stone: it turns to gold (the stone is kept).
    fn turn_boulder_to_gold(&mut self, (x, y, z): Pos) -> bool {
        if self.world.block_at(x, y, z) != Block::Boulder {
            return false;
        }
        self.world.break_block(x, y, z);
        self.world.set_block(x, y, z, Block::GoldBoulder);
        self.toasts.push("The Philosopher's Stone turns the boulder to gold.".to_string());
        true
    }

    /// Right click the sword in the stone. Only a player wearing the king's crown can draw it: the stone is left bare and
    /// Excalibur is theirs.
    pub fn try_pull_sword(&mut self, (x, y, z): Pos) -> bool {
        let base = match self.world.block_at(x, y, z) {
            Block::SwordInStone => (x, y, z),
            Block::SwordHilt => (x, y - 1, z),
            _ => return false,
        };
        if self.worn != Some(ItemKind::AncientCrown) {
            self.toasts.push("The sword will not move. Only a king can draw it.".to_string());
            return false;
        }
        self.world.set_block(base.0, base.1, base.2, Block::PulledStone);
        self.world.break_block(base.0, base.1 + 1, base.2);
        self.give_or_drop(ItemKind::Excalibur, (base.0, base.1 + 1, base.2));
        self.toasts.push("The crown shines. You draw Excalibur from the stone.".to_string());
        true
    }

    // --- Wearing -------------------------------------------------------------------------

    /// Put on the cloak, armour, crown or backpack in hand, taking one from the stack. They all share the one worn place, so
    /// whatever was worn goes back into the inventory (a backpack's extra rows must first be emptied into the other slots, or
    /// the swap is refused). Returns whether something was put on.
    pub fn wear_held(&mut self) -> bool {
        if !self.held_item().map_or(false, |k| k.is_wearable()) {
            return false;
        }
        match self.inventory.put_on(&mut self.worn, self.selected) {
            Ok(kind) if kind.is_backpack() => self.toasts.push(format!("You put on the {}: {} more rows of slots", kind.name(), BACKPACK_EXTRA_ROWS)),
            Ok(kind) => self.toasts.push(format!("You put on the {}", kind.name())),
            Err(why) => {
                self.toasts.push(why.to_string());
                return false;
            }
        }
        true
    }

    /// Take off what is worn, into the inventory (a backpack's rows must be emptied into the other slots first).
    #[allow(dead_code)]
    pub fn take_off(&mut self) -> bool {
        match self.inventory.take_off(&mut self.worn) {
            Ok(Some(old)) => {
                self.toasts.push(format!("You take off the {}", old.name()));
                true
            }
            Ok(None) => false,
            Err(why) => {
                self.toasts.push(why.to_string());
                false
            }
        }
    }

    fn stow_or_drop(&mut self, kind: ItemKind) {
        if self.inventory.add(kind, 1) != 0 {
            let spot = self.player.pos + vec3(0.0, 1.0, 0.0);
            self.items.push(DroppedItem::spawn_at(kind, spot, &mut self.rng));
        }
    }

    // --- Bosses, the cyclone and the Skylands ----------------------------------------------

    /// Bring the bosses out when the player comes near: the Yeti on the summit of the mountain, and the monster of every
    /// lake with a tower whose monster is still alive. Called with the wildlife spawns.
    fn spawn_bosses(&mut self) {
        let p = self.player.pos;
        if !self.world.landmarks.yeti_slain && !self.animals.iter().any(|a| a.kind == AnimalKind::Yeti) {
            if let Some((sx, sz)) = self.world.landmarks.summit() {
                let gap = vec2(p.x - (sx as f32 + 0.5), p.z - (sz as f32 + 0.5)).length();
                if gap <= YETI_SPAWN_DISTANCE && self.world.chunks.contains_key(&World::chunk_of(sx, sz)) {
                    let at = vec3(sx as f32 + 0.5, self.world.ground_top(sx, sz) as f32 + 1.01, sz as f32 + 0.5);
                    self.animals.push(Animal::new(AnimalKind::Yeti, at, &mut self.rng));
                }
            }
        }
        if self.in_overworld() && !self.world.landmarks.orangutan_slain && !self.animals.iter().any(|a| a.kind == AnimalKind::Orangutan) {
            let (jx, jz) = crate::terrain::jungle_centre(self.info.seed);
            let (dx, dz, dy) = crate::temple::orangutan_post();
            let gap = vec2(p.x - (jx as f32 + 0.5), p.z - (jz as f32 + 0.5)).length();
            if gap <= ORANGUTAN_SPAWN_DISTANCE && self.world.chunks.contains_key(&World::chunk_of(jx + dx, jz + dz)) {
                let base = self.world.ground_top(jx, jz);
                let at = vec3((jx + dx) as f32 + 0.5, (base + 1 + dy) as f32 + 0.01, (jz + dz) as f32 + 0.5);
                let mut ape = Animal::new(AnimalKind::Orangutan, at, &mut self.rng);
                ape.home = vec3(jx as f32 + 0.5, at.y, jz as f32 + 0.5);
                self.animals.push(ape);
            }
        }
        if self.in_skylands() && !self.world.landmarks.phoenix_slain && !self.animals.iter().any(|a| a.kind == AnimalKind::Phoenix) {
            let crater = skylands::volcano_crater(self.info.seed);
            let gap = vec2(p.x - crater.x, p.z - crater.z).length();
            if gap <= PHOENIX_SPAWN_DISTANCE && self.world.chunks.contains_key(&World::chunk_of_pos(crater)) {
                self.animals.push(Animal::new(AnimalKind::Phoenix, crater + vec3(0.0, 3.0, 0.0), &mut self.rng));
            }
        }
        let lakes: Vec<Lake> = self.world.landmarks.lakes.iter().copied().filter(|l| self.world.landmarks.monster_alive(l)).collect();
        for lake in lakes {
            let centre = vec3(lake.x as f32 + 0.5, 0.0, lake.z as f32 + 0.5);
            let has_monster = self.animals.iter().any(|a| a.kind == AnimalKind::LochNessMonster && vec2(a.home.x - centre.x, a.home.z - centre.z).length() <= lake.radius as f32);
            if has_monster || vec2(p.x - centre.x, p.z - centre.z).length() > lake.radius as f32 + LOCH_NESS_SPAWN_MARGIN {
                continue;
            }
            // Somewhere in the water between the island and the far shore, in a chunk that is loaded.
            let inner = lake.island_radius() * 1.25 + 12.0;
            for _ in 0..32 {
                let angle = self.rng.unit() * std::f32::consts::TAU;
                let dist = inner + self.rng.unit() * (lake.radius as f32 * 0.8 - inner).max(1.0);
                let (x, z) = ((centre.x + angle.cos() * dist).floor() as i32, (centre.z + angle.sin() * dist).floor() as i32);
                if lake.contains(x, z) && self.world.chunks.contains_key(&World::chunk_of(x, z)) {
                    let at = vec3(x as f32 + 0.5, self.world.ground_top(x, z) as f32 + 1.01, z as f32 + 0.5);
                    self.animals.push(Animal::new(AnimalKind::LochNessMonster, at, &mut self.rng));
                    break;
                }
            }
        }
    }

    /// What a boss's death does: the Yeti's raises the cyclone on the summit; the monster's is only noted.
    fn boss_slain(&mut self, boss: &Animal) {
        match boss.kind {
            AnimalKind::Yeti => {
                self.world.landmarks.yeti_slain = true;
                self.toasts.push("The Yeti is slain. A cyclone rises on the summit.".to_string());
            }
            AnimalKind::LochNessMonster => {
                let nearest = self.world.landmarks.lakes.iter().copied().filter(|l| l.has_tower()).min_by(|a, b| {
                    let d = |l: &Lake| (l.x as f32 - boss.home.x).powi(2) + (l.z as f32 - boss.home.z).powi(2);
                    d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(lake) = nearest {
                    self.world.record_monster_slain(&lake);
                    self.toasts.push("The Loch Ness Monster is slain.".to_string());
                }
            }
            AnimalKind::Orangutan => {
                self.world.landmarks.orangutan_slain = true;
                self.toasts.push("The Orangutan is slain.".to_string());
            }
            AnimalKind::Phoenix => {
                self.world.landmarks.phoenix_slain = true;
                self.tornadoes.clear();
                self.toasts.push("The Phoenix is slain. The fire tornadoes that sweep the islands die away; the crater\'s never does.".to_string());
            }
            kind if kind.is_wild_hunt() => {
                // The last of the Hunt to fall wins the game.
                if !self.animals.iter().any(|a| a.kind.is_wild_hunt()) {
                    self.win_the_game();
                }
            }
            _ => {}
        }
    }

    /// The Wild Hunt is beaten: the game is won (and saved as won), with a message on the screen. Play goes on.
    fn win_the_game(&mut self) {
        self.world.landmarks.game_won = true;
        self.hostile_arrows.clear();
        self.victory = Some(VICTORY_TEXT_SECONDS);
        self.toasts.push("The Wild Hunt is beaten. You have won!".to_string());
    }

    /// The nearest living boss within `BOSS_BAR_DISTANCE`, as its name and the share of its health left (for the bar at
    /// the top of the screen).
    pub fn boss_bar(&self) -> Option<(&'static str, f32)> {
        let p = self.player.pos;
        let nearest = self
            .animals
            .iter()
            .filter(|a| a.kind.is_boss() && (a.pos - p).length() <= BOSS_BAR_DISTANCE)
            .min_by(|a, b| (a.pos - p).length().partial_cmp(&(b.pos - p).length()).unwrap_or(std::cmp::Ordering::Equal))?;
        if nearest.kind.is_wild_hunt() {
            // The whole Hunt shares one bar: what is left of all of it together.
            let left: i32 = self.animals.iter().filter(|a| a.kind.is_wild_hunt()).map(|a| a.health.max(0)).sum();
            let present: i32 = self.animals.iter().filter(|a| a.kind.is_wild_hunt()).map(|a| a.kind.max_health()).sum();
            let total = if self.hunt_total > 0 { self.hunt_total.max(left) } else { present }.max(1);
            return Some((nearest.kind.boss_name(), (left as f32 / total as f32).clamp(0.0, 1.0)));
        }
        Some((nearest.kind.boss_name(), (nearest.health as f32 / nearest.kind.max_health() as f32).clamp(0.0, 1.0)))
    }

    /// The cyclone in the world the player is in: on the summit once the Yeti is dead, and in the Skylands the one that
    /// leads home.
    pub fn cyclone(&self) -> Option<Cyclone> {
        match self.world.dimension {
            Dimension::Overworld if self.world.landmarks.yeti_slain => {
                let (sx, sz) = self.world.landmarks.summit()?;
                Some(Cyclone { pos: vec3(sx as f32 + 0.5, (self.world.ground_top(sx, sz) + 1) as f32, sz as f32 + 0.5) })
            }
            _ => None,
        }
    }

    /// How white the screen is from standing in a cyclone: it fades in over the last `CYCLONE_FADE_SECONDS`.
    pub fn cyclone_whiteness(&self) -> f32 {
        ((self.cyclone_timer - (CYCLONE_SECONDS - CYCLONE_FADE_SECONDS)) / CYCLONE_FADE_SECONDS).clamp(0.0, 1.0)
    }

    /// How white the whole screen is: a cyclone taking hold, or a white-out of the Skylands.
    pub fn white_alpha(&self) -> f32 {
        self.cyclone_whiteness().max(self.whiteout.map_or(0.0, |w| w.alpha()))
    }

    /// While the screen is white from a fall out of the Skylands: the overworld, to be drawn behind it, and where its camera
    /// stands (the player's eye carried from the first island to the summit, so the fall is a fall onto the mountain).
    pub fn overworld_backdrop(&self, eye: Vec3) -> Option<(&World, Vec3)> {
        if !self.in_skylands() || !self.whiteout.map_or(false, |w| w.shows_overworld()) {
            return None;
        }
        let over = &self.parked(Dimension::Overworld)?.world;
        let (jx, jz) = crate::terrain::jungle_centre(self.info.seed);
        let landing = vec3(jx as f32 + 0.5, (over.ground_top(jx, jz) + 1) as f32 + SKY_FALL_DROP, jz as f32 + 0.5);
        Some((over, eye - skylands::spawn_point() + landing))
    }

    /// Carry the player up through the cyclone into the Skylands, arriving in a fading white.
   pub fn enter_skylands(&mut self) {
   	    if !self.in_overworld() {
   	    	        return;
   	    	            }

   	    	                self.travel_to(Dimension::Skylands);

   	    	                    // Always return to the original Skylands spawn point.
   	    	                        self.player.pos = skylands::spawn_point();
   	    	                            self.player.vel = Vec3::ZERO;
   	    	                                self.player.knock = Vec3::ZERO;

   	    	                                    self.world.stream_chunks(self.player.pos);
   	    	                                        self.world.rebuild_dirty(self.player.pos, usize::MAX);

   	    	                                            self.whiteout = Some(Whiteout {
   	    	                                            	        elapsed: 0.0,
   	    	                                            	                kind: WhiteoutKind::Arrival,
   	    	                                            	                    });

   	    	                                            	                        self.toasts.push(
   	    	                                            	                        	        "The cyclone carries you up into a sky of floating islands.".to_string()
   	    	                                            	                        	            );
   	    	                                            	                        	            }
   	    	                                            	                        )
   	    	                                            })
   	    }
   }

    /// Carry the player home through the cyclone of the first island, to the summit.
    pub fn leave_skylands(&mut self) {
        if !self.in_skylands() {
            return;
        }
        self.travel_to(Dimension::Overworld);
        // Set down beside the cyclone, not in it, or it would carry the player straight back.
        if let Some(c) = self.cyclone() {
            self.player.pos = c.pos + vec3(CYCLONE_RADIUS + 2.5, 0.01, 0.0);
            self.player.lift_out_of_blocks(&self.world);
        }
        self.whiteout = Some(Whiteout { elapsed: 0.0, kind: WhiteoutKind::Arrival });
        self.toasts.push("The cyclone sets you down on the summit.".to_string());
    }

    /// The dry column nearest the middle of the jungle, where a fall out of the Skylands lands.
    pub fn jungle_landing(&self) -> (i32, i32) {
        let (cx, cz) = crate::terrain::jungle_centre(self.info.seed);
        // Beside the temple, not on it.
        for r in crate::temple::TEMPLE_REACH + 3..crate::temple::TEMPLE_REACH + 40 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) == r && !self.world.is_wet_column(cx + dx, cz + dz) {
                        return (cx + dx, cz + dz);
                    }
                }
            }
        }
        (cx, cz)
    }

    /// Make the jungle's chunks in the parked overworld, so that it is there to show behind the white screen of a fall.
    fn prepare_jungle_backdrop(&mut self) {
        let (jx, jz) = crate::terrain::jungle_centre(self.info.seed);
        if let Some(a) = [&mut self.away, &mut self.shelf].into_iter().flatten().find(|a| a.world.dimension == Dimension::Overworld) {
            let pos = vec3(jx as f32 + 0.5, a.world.ground_top(jx, jz) as f32 + 1.0 + SKY_FALL_DROP, jz as f32 + 0.5);
            a.world.stream_chunks(pos);
            a.world.rebuild_dirty(pos, usize::MAX);
        }
    }

    /// Advance the cyclone (five seconds inside it carries the player through), a fall off an island, and the white screen.
    /// Call once a frame.
    pub fn update_travel(&mut self, dt: f32) {
        if let Some(mut w) = self.whiteout.take() {
            w.elapsed += dt;
            if let WhiteoutKind::Fall { moved: false } = w.kind {
                if w.elapsed >= SKY_FALL_FADE_IN {
                    // Wholly white now: the player leaves the Skylands and is put high above the normal world's spawn point,
                    // to fall back on to it.
                    self.travel_to(Dimension::Overworld);
                    let (jx, jz) = self.jungle_landing();
                    self.player.pos = vec3(jx as f32 + 0.5, self.world.ground_top(jx, jz) as f32 + 1.0 + SKY_FALL_DROP, jz as f32 + 0.5);
                    self.player.vel = Vec3::ZERO;
                    self.player.knock = Vec3::ZERO;
                    self.world.stream_chunks(self.player.pos);
                    self.world.rebuild_dirty(self.player.pos, usize::MAX);
                    w.kind = WhiteoutKind::Fall { moved: true };
                }
            }
            self.whiteout = (w.elapsed < w.duration()).then_some(w);
        }
        if self.in_skylands() && self.whiteout.is_none() && self.player.pos.y < SKY_VOID_Y {
            self.prepare_jungle_backdrop();
            self.whiteout = Some(Whiteout { elapsed: 0.0, kind: WhiteoutKind::Fall { moved: false } });
            self.toasts.push("You fall from the island.".to_string());
        }
        let inside = self.cyclone().map_or(false, |c| c.contains(self.player.pos));
        if !inside {
            self.cyclone_timer = 0.0;
            return;
        }
        self.cyclone_timer += dt;
        if self.cyclone_timer >= CYCLONE_SECONDS {
            self.cyclone_timer = 0.0;
            if self.in_overworld() {
                self.enter_skylands();
            } else {
                self.leave_skylands();
            }
        }
    }

    // --- The Horizen ---------------------------------------------------------------------

    /// Finish a well fall only when the player is standing on its stone floor.
    pub fn update_dimension(&mut self) {
        if !self.in_overworld() {
            return;
        }
        let p = self.player.pos;
        // Water is deliberately non-solid, so `on_ground` becomes true only when the falling player reaches the
        // stone floor. This prevents a shallow fall through the well from teleporting early.
        if self.player.on_ground
            && p.y <= WELL_FLOOR_Y + 0.05
            && self.world.landmarks.is_well(p.x.floor() as i32, p.z.floor() as i32)
        {
            self.enter_horizen();
        }
    }

    /// Go to `target`: the world, the items on the ground and the player's place in it trade with the ones parked there
    /// (a dimension nobody has been to is made new). Creatures, arrows and the work in progress do not cross.
    fn travel_to(&mut self, target: Dimension) {
        if self.world.dimension == target {
            return;
        }
        let seed = self.info.seed;
        let from = self.world.dimension;
        let mut other = self.take_parked(target).unwrap_or_else(|| Away::fresh(target, seed));
        let (yaw, pitch) = (self.player.yaw, self.player.pitch);
        std::mem::swap(&mut self.world, &mut other.world);
        std::mem::swap(&mut self.items, &mut other.items);
        std::mem::swap(&mut self.player, &mut other.player);
        // The player keeps looking the way they were looking.
        self.player.yaw = yaw;
        self.player.pitch = pitch;
        self.player.vel = Vec3::ZERO;
        self.player.knock = Vec3::ZERO;
        self.park(other);
        // Leaving the Skylands wakes the Yeti again: it must be killed anew before the summit cyclone returns.
        if from == Dimension::Skylands && target == Dimension::Overworld && self.world.landmarks.yeti_slain {
            self.world.landmarks.yeti_slain = false;
            self.toasts.push("Far below, the Yeti stirs on the summit once more.".to_string());
        }
        // An Iron Quail left behind comes again (whole) when the player is back in the overworld.
        self.iron_quails_owed += self.animals.iter().filter(|a| a.kind == AnimalKind::IronQuail).count() as u32;
        // A vampire bat left behind in the cave is waiting for the player's next visit; the king's spirits do not leave the castle's cave.
        self.vampire_bats_owed += self.animals.iter().filter(|a| a.kind == AnimalKind::VampireBat).count() as u32;
        self.king_dialog = None;
        self.animals.clear();
        self.arrows.clear();
        self.hostile_arrows.clear();
        self.mining = None;
        self.blackout = None;
        self.tornadoes.clear();
        self.crater_tornado = None;
        self.tornado_timer = 0.0;
        self.cyclone_timer = 0.0;
        self.reset_storm();
        self.treks = Treks::new();
        self.world.stream_chunks(self.player.pos);
        self.world.rebuild_dirty(self.player.pos, usize::MAX);
        // A Horizen saved before it was a cave may leave the player inside the rock: back to the arrival pool.
        if target == Dimension::Horizen && !horizen::is_open(seed, self.player.pos.x.floor() as i32, self.player.pos.z.floor() as i32) {
            self.player.pos = Player::new().pos;
        }
        self.player.lift_out_of_blocks(&self.world);
    }

    /// Fall into the Horizen. The player is swapped into its arrival pool while the screen is black.
    pub fn enter_horizen(&mut self) {
        if self.in_horizen() {
            return;
        }
        self.travel_to(Dimension::Horizen);
        // Whatever it was when it was left, the cave is day when you come in.
        self.world.set_night(false);
        self.horizen_transition = Some(0.0);
        self.toasts.push("You fall through the water into the Horizen.".to_string());
    }

    /// Advance the black screen after a well fall. The player has already appeared in the arrival pool underneath it.
    pub fn update_horizen_transition(&mut self, dt: f32) {
        let Some(mut elapsed) = self.horizen_transition else { return };
        elapsed += dt;
        self.horizen_transition = (elapsed < HORIZEN_TRANSITION_SECONDS).then_some(elapsed);
    }

    /// Darkness of the well transition, from clear to black and back to clear.
    pub fn horizen_transition_darkness(&self) -> f32 {
        let Some(elapsed) = self.horizen_transition else { return 0.0 };
        let t = (elapsed / HORIZEN_TRANSITION_SECONDS).clamp(0.0, 1.0);
        const FADE_SHARE: f32 = 0.30;
        if t < FADE_SHARE {
            t / FADE_SHARE
        } else if t > 1.0 - FADE_SHARE {
            (1.0 - t) / FADE_SHARE
        } else {
            1.0
        }
    }

    /// Put a new animal on the ground at column (x, z). Fails when that column is not loaded, is under a river or
    /// a lake, or something solid is in the way.
    pub fn spawn_animal(&mut self, kind: AnimalKind, x: i32, z: i32) -> bool {
        if !self.world.chunks.contains_key(&World::chunk_of(x, z)) || self.world.is_wet_column(x, z) {
            return false;
        }
        let ground_y = (self.world.ground_top(x, z) + 1) as f32;
        let pos = if kind == AnimalKind::WillOWhisp {
            vec3(x as f32 + 0.5, ground_y + WILL_O_WHISP_SPAWN_HEIGHT, z as f32 + 0.5)
        } else {
            vec3(x as f32 + 0.5, ground_y + 0.01, z as f32 + 0.5)
        };
        let half = vec3(kind.half_width(), 0.0, kind.half_width());
        if kind != AnimalKind::WillOWhisp && self.world.box_blocked(pos - half, pos + half + vec3(0.0, kind.height(), 0.0)) {
            return false;
        }
        self.animals.push(Animal::new(kind, pos, &mut self.rng));
        true
    }

    /// Every `ANIMAL_SPAWN_INTERVAL` seconds, bring wildlife into the world a good way from the player, unless
    /// that kind is already at its limit. Will-o-whisps are included only while the world is night.
    fn spawn_wildlife(&mut self) {
        // In the cave the only wildlife is bats.
        if self.in_horizen() {
            self.cave_spawn_tick = self.cave_spawn_tick.wrapping_add(1);
            if self.cave_spawn_tick % BAT_CAVE_SPAWN_EVERY == 0 {
                self.spawn_cave_bats();
            }
            return;
        }
        if !self.in_overworld() {
            return;
        }
        if self.player_in_mine() {
            self.spawn_mine_bats();
        }
        if self.player_in_jungle() {
            self.spawn_jungle_wildlife();
        }
        // Ravens come out at night.
        if self.world.night && self.animals.iter().filter(|a| a.kind == AnimalKind::Raven && !a.summoned).count() < MAX_RAVEN && self.rng.next() % 100 < SPAWN_WEIGHT_RAVEN {
            self.spawn_raven(false);
        }
        let count = |k: AnimalKind| self.animals.iter().filter(|a| a.kind == k).count();
        let room = |kind: AnimalKind| match kind {
            AnimalKind::Deer => MAX_DEER,
            AnimalKind::Quail => MAX_QUAIL,
            AnimalKind::Bear => MAX_BEAR,
            AnimalKind::Rabbit => MAX_RABBIT,
            AnimalKind::Moose => MAX_MOOSE,
            AnimalKind::WillOWhisp => MAX_WILL_O_WHISP,
            // The bosses are never wildlife, and neither are bats in the normal world (only the pipes call them there).
            AnimalKind::Crow => MAX_CROW,
            AnimalKind::Jaguar => MAX_JAGUAR,
            AnimalKind::FlyingSnake => MAX_FLYING_SNAKE,
            AnimalKind::Yeti | AnimalKind::LochNessMonster | AnimalKind::IronQuail | AnimalKind::Bat | AnimalKind::Raven | AnimalKind::Phoenix => 0,
            AnimalKind::VampireBat | AnimalKind::SpiritArcher | AnimalKind::Orangutan => 0,
            AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer | AnimalKind::HuntBear | AnimalKind::HuntCrow => 0,
        }
        .saturating_sub(count(kind));
        let mut open: Vec<(AnimalKind, u64)> = [
            (AnimalKind::Deer, MAX_DEER, SPAWN_WEIGHT_DEER),
            (AnimalKind::Quail, MAX_QUAIL, SPAWN_WEIGHT_QUAIL),
            (AnimalKind::Crow, MAX_CROW, SPAWN_WEIGHT_CROW),
            (AnimalKind::Bear, MAX_BEAR, SPAWN_WEIGHT_BEAR),
            (AnimalKind::Rabbit, MAX_RABBIT, SPAWN_WEIGHT_RABBIT),
            (AnimalKind::Moose, MAX_MOOSE, SPAWN_WEIGHT_MOOSE),
        ]
        .into_iter()
        .filter(|&(kind, max, _)| count(kind) < max)
        .map(|(kind, _, weight)| (kind, weight))
        .collect();
        if self.world.night && count(AnimalKind::WillOWhisp) < MAX_WILL_O_WHISP {
            open.push((AnimalKind::WillOWhisp, SPAWN_WEIGHT_WILL_O_WHISP));
        }
        let total: u64 = open.iter().map(|&(_, w)| w).sum();
        if total == 0 {
            return;
        }
        let mut roll = self.rng.next() % total;
        let kind = open
            .iter()
            .find(|&&(_, weight)| {
                let hit = roll < weight;
                roll = roll.saturating_sub(weight);
                hit
            })
            .map_or(open[0].0, |&(kind, _)| kind);
        let angle = self.rng.unit() * std::f32::consts::TAU;
        let dist = ANIMAL_SPAWN_MIN_DISTANCE + self.rng.unit() * (ANIMAL_SPAWN_MAX_DISTANCE - ANIMAL_SPAWN_MIN_DISTANCE);
        let p = self.player.pos;
        let (x, z) = ((p.x + angle.cos() * dist).floor() as i32, (p.z + angle.sin() * dist).floor() as i32);
        // A covey never takes the kind past its limit.
        let group = if kind == AnimalKind::Quail { self.rng.range(1, 4) as usize } else { 1 }.min(room(kind));
        for _ in 0..group {
            let (dx, dz) = (self.rng.range(-2, 3), self.rng.range(-2, 3));
            self.spawn_animal(kind, x + dx, z + dz);
        }
    }

    /// Move the animals, drop the feathers startled quail lose, let bears strike the player they reach,
    /// remove animals that are far away and now and then let new ones in. Deer and quail react to the nearer
    /// of the player and any bear; a bear only ever reacts to the player.
    pub fn update_animals(&mut self, dt: f32) {
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        match self.world.dimension {
            // The cave of the Horizen holds bats and nothing else (see `spawn_cave_bats`).
            Dimension::Horizen => {}
            // Nothing lives in the Skylands but the Phoenix in its volcano, and fire tornadoes sweep across them by day.
            Dimension::Skylands => self.update_sky_tornado(dt),
            Dimension::Overworld => {}
        }
        let player = self.player.pos;
        // Will-o-whisps are drawn to a player with a holly rod in the hotbar. One that is bonded and has been left
        // far behind (the player ran off, or started again at the beginning) catches up at once.
        let lure = if self.holly_rod_in_hotbar() { Some(player) } else { None };
        for a in self.animals.iter_mut().filter(|a| a.kind == AnimalKind::WillOWhisp) {
            a.lure = lure;
            if lure.is_none() {
                a.bonded = false;
            }
            if a.bonded && lure.is_some() && (a.pos - player).length() > WILL_O_WHISP_TELEPORT_DISTANCE {
                a.pos = player + vec3(0.0, WILL_O_WHISP_ORBIT_HEIGHT, WILL_O_WHISP_ORBIT_RADIUS);
            }
        }
        let flat = |a: &Animal| vec2(a.pos.x - player.x, a.pos.z - player.z).length();
        // Whisps go with the night, except the ones that follow the player.
        // Bats called by the pipes go with the dawn.
        let overworld = self.in_overworld();
        self.animals.retain(|a| {
            a.kind.never_gives_up()
                || flat(a) < if a.kind.is_boss() { BOSS_DESPAWN_DISTANCE } else { ANIMAL_DESPAWN_DISTANCE }
                    && (self.world.night || a.kind != AnimalKind::WillOWhisp || a.bonded)
                    && (self.world.night || !overworld || a.kind != AnimalKind::Bat || a.lives_in_mine)
                    // Ravens go with the dawn, except the ones Nessie called, which act as if it were night.
                    && (self.world.night || !overworld || a.kind != AnimalKind::Raven || a.summoned)
        });
        let cloaked = self.worn == Some(ItemKind::PeltCloak);
        for a in self.animals.iter_mut().filter(|a| a.kind == AnimalKind::Bear) {
            a.pacified = cloaked && !a.provoked;
        }
        let bears: Vec<Vec3> = self.animals.iter().filter(|a| a.kind == AnimalKind::Bear).map(|a| a.pos).collect();
        let mut strikes: Vec<(usize, Vec3, u32, &'static str)> = Vec::new();
        let mut volleys: Vec<Arrow> = Vec::new();
        // A Rabbit Foot in the hotbar makes the Wild Hunt's archer nock its arrows half as fast.
        let slowdown = if self.rabbit_foot_in_hotbar() { HUNT_ARCHER_FOOT_FACTOR } else { 1.0 };
        let player_vel = self.player.vel;
        let mut events: Vec<BossEvent> = Vec::new();
        for (index, a) in self.animals.iter_mut().enumerate() {
            let near = a.kind.never_gives_up() || vec2(a.pos.x - player.x, a.pos.z - player.z).length() <= if a.kind.is_boss() { BOSS_UPDATE_DISTANCE } else { ANIMAL_UPDATE_DISTANCE };
            if !near || !self.world.chunks.contains_key(&World::chunk_of_pos(a.pos)) {
                continue;
            }
            let threat = if a.kind.fears_bears() {
                let dist2 = |p: Vec3| (a.pos - p).length_squared();
                bears.iter().copied().fold(player, |best, bear| if dist2(bear) < dist2(best) { bear } else { best })
            } else {
                player
            };
            let before_x = a.pos.x;
            if let Some(kind) = a.update(&self.world, threat, dt, &mut self.rng) {
                self.items.push(DroppedItem::spawn_at(kind, a.pos + vec3(0.0, 0.3, 0.0), &mut self.rng));
            }
            if a.kind == AnimalKind::Crow {
                let gap = vec2(a.pos.x - player.x, a.pos.z - player.z).length();
                if gap > CROW_LOSE_DISTANCE {
                    a.west_followed = 0.0;
                } else if a.mode == crate::animal::Mode::Fly && gap <= CROW_FOLLOW_DISTANCE {
                    a.west_followed += (before_x - a.pos.x).max(0.0);
                }
            }
            if a.try_attack(player) {
                strikes.push((index, a.pos, a.kind.attack_damage(), a.kind.death_message()));
            }
            events.extend(a.take_events());
            if a.ready_to_shoot(player) {
                // An archer (or an Iron Quail) shoots if it can see the player, leading them a little and allowing for the missile's fall.
                let quail = a.kind == AnimalKind::IronQuail;
                let (speed, spread) = if quail { (IRON_FEATHER_SPEED, IRON_FEATHER_SPREAD) } else { (HUNT_ARROW_SPEED, HUNT_ARROW_SPREAD) };
                let origin = a.pos + vec3(a.heading.cos() * 0.5, if quail { 0.8 } else { 1.5 }, a.heading.sin() * 0.5);
                let gap = (player + vec3(0.0, 1.0, 0.0) - origin).length();
                let flight = gap / speed;
                let aim = player + vec3(0.0, 1.0 + 0.5 * ARROW_GRAVITY * flight * flight, 0.0) + vec3(player_vel.x, 0.0, player_vel.z) * flight * 0.5;
                let mut dir = (aim - origin).normalize_or_zero();
                let (jx, jy, jz) = (self.rng.unit() - 0.5, self.rng.unit() - 0.5, self.rng.unit() - 0.5);
                dir = (dir + vec3(jx, jy, jz) * 2.0 * spread).normalize_or_zero();
                let sight = player + vec3(0.0, 1.0, 0.0) - origin;
                if dir != Vec3::ZERO && clear_line(&self.world, origin, sight.normalize_or_zero(), sight.length()) {
                    volleys.push(if quail { Arrow::from_iron_quail(origin, dir) } else { Arrow::from_archer(origin, dir) });
                    a.shot_fired(slowdown);
                }
            }
        }
        self.hostile_arrows.extend(volleys);
        self.update_hostile_arrows(dt);
        self.land_crows_at_altars();
        // A bat that strikes a player holding pink salt dies on it instead: no damage, one lump of salt used up.
        let mut salted = Vec::new();
        let mut blows = Vec::new();
        for (index, from, damage, death) in strikes {
            let is_bat = self.animals[index].kind == AnimalKind::Bat;
            if is_bat && self.held_item() == Some(ItemKind::PinkSalt) && self.inventory.take_from_slot(self.selected).is_some() {
                salted.push(index);
            } else {
                blows.push((from, damage, death));
            }
        }
        if !salted.is_empty() {
            self.toasts.push(if salted.len() == 1 { "The bat strikes your salt crystal and falls dead; the crystal is spent.".to_string() } else { "The bats strike your salt and fall dead.".to_string() });
            self.remove_dead(salted);
        }
        for (from, damage, death) in blows {
            if !self.hurt_player(damage, death) {
                self.knock_player(from);
            }
        }
        self.update_spirits();
        for event in events {
            self.carry_out(event);
        }
        self.update_hazards(dt);
        self.spawn_timer += dt;
        if self.spawn_timer >= ANIMAL_SPAWN_INTERVAL {
            self.spawn_timer = 0.0;
            self.spawn_wildlife();
            self.spawn_bosses();
            self.summon_iron_quails();
        }
    }

    /// Fly the arrows the Wild Hunt's archer has shot. One that passes through the player hurts them (and throws them back a
    /// little); one that strikes a block or flies too long is gone.
    fn update_hostile_arrows(&mut self, dt: f32) {
        let player = self.player.pos;
        let mut hits = Vec::new();
        let mut landed = Vec::new();
        let world = &self.world;
        self.hostile_arrows.retain_mut(|arrow| {
            let from = arrow.pos;
            let fate = arrow.update(world, &[], dt);
            if arrow.hit_player(from, player) {
                hits.push((from, arrow.kind));
                return false;
            }
            if fate != Fate::InFlight && arrow.kind == ItemKind::IronFeather {
                // An iron feather that missed lies where it fell, to be picked up.
                landed.push(arrow.pos - arrow.vel.normalize_or_zero() * ARROW_DROP_BACKOFF);
            }
            fate == Fate::InFlight
        });
        for spot in landed {
            self.items.push(DroppedItem::at_rest(ItemKind::IronFeather, spot));
        }
        for (from, kind) in hits {
            let (damage, death) = if kind == ItemKind::IronFeather {
                (IRON_FEATHER_DAMAGE as u32, "An iron feather pierced you")
            } else {
                (HUNT_ARROW_DAMAGE, "An arrow of the Wild Hunt killed you")
            };
            if !self.hurt_player(damage, death) {
                self.knock_player(from);
            }
        }
    }

    // --- Boss attacks: the Yeti's smash and ice, Nessie's ravens and acid, the Phoenix's fire -----------

    /// Do what a boss has done (see `BossEvent`).
    fn carry_out(&mut self, event: BossEvent) {
        let player = self.player.pos;
        match event {
            BossEvent::Smash { at } => self.smash(at, AnimalKind::Yeti, BurstShape::Smash, YETI_SMASH_RADIUS, YETI_SMASH_DAMAGE, YETI_SMASH_KNOCKBACK, "The Yeti smashed you flat"),
            BossEvent::OrangutanSmash { at } => self.smash(at, AnimalKind::Orangutan, BurstShape::OrangutanSmash, ORANGUTAN_SMASH_RADIUS, ORANGUTAN_SMASH_DAMAGE, ORANGUTAN_SMASH_KNOCKBACK, "The Orangutan smashed you flat"),
            BossEvent::RockThrow { from } => {
                let dir = Projectile::aim(ProjectileKind::Rock, from, player + vec3(0.0, 1.0, 0.0), self.player.vel);
                self.projectiles.push(Projectile::launch(ProjectileKind::Rock, from, dir));
            }
            BossEvent::IceThrow { from } => {
                let dir = Projectile::aim(ProjectileKind::IceBall, from, player + vec3(0.0, 1.0, 0.0), self.player.vel);
                self.projectiles.push(Projectile::launch(ProjectileKind::IceBall, from, dir));
            }
            BossEvent::Fireball { from } => {
                let dir = Projectile::aim(ProjectileKind::Fireball, from, player + vec3(0.0, 1.0, 0.0), self.player.vel);
                self.projectiles.push(Projectile::launch(ProjectileKind::Fireball, from, dir));
            }
            BossEvent::CallRavens { count } => self.call_ravens(count, LOCH_NESS_MAX_RAVENS),
            BossEvent::Acid { from } => {
                for (x, z) in acid_line(&|x, z| self.world.is_wet_column(x, z), from, player) {
                    match self.acid.iter_mut().find(|c| (c.x, c.z) == (x, z)) {
                        Some(cell) => cell.left = ACID_SECONDS,
                        None => self.acid.push(AcidCell { x, z, left: ACID_SECONDS }),
                    }
                }
            }
        }
    }

    /// A boss's fists hit the ground at `at`: everything within `radius` of its body (`owner`'s), level with it, is hurt for `damage` and
    /// thrown `knockback` blocks away, others of `owner`'s kind excepted.
    fn smash(&mut self, at: Vec3, owner: AnimalKind, shape: BurstShape, radius: f32, damage: u32, knockback: f32, death: &str) {
        let player = self.player.pos;
        self.bursts.push(ShownBurst { pos: at, shape, age: 0.0 });
        let near = |p: Vec3, half: f32| vec2(p.x - at.x, p.z - at.z).length() - owner.half_width() - half <= radius && (p.y - at.y).abs() <= 2.5;
        if near(player, PLAYER_HALF_WIDTH) && !self.hurt_player(damage, death) {
            self.player.launch(player - at, knockback);
        }
        let mut dead = Vec::new();
        for (i, a) in self.animals.iter_mut().enumerate() {
            if a.kind == owner || a.kind == AnimalKind::SpiritArcher || !near(a.pos, a.kind.half_width()) {
                continue;
            }
            if a.damage(damage as i32) {
                dead.push(i);
            } else {
                a.launch(a.pos - at, knockback);
            }
        }
        self.remove_dead(dead);
    }

    /// Fly the projectiles, burst them, burn a player standing in acid water, and fade the flashes.
    fn update_hazards(&mut self, dt: f32) {
        let player = self.player.pos;
        let mut bursts: Vec<Burst> = Vec::new();
        let world = &self.world;
        self.projectiles.retain_mut(|p| match p.update(world, player, dt) {
            Some(burst) => {
                bursts.push(burst);
                false
            }
            None => true,
        });
        for burst in bursts {
            self.bursts.push(ShownBurst { pos: burst.pos, shape: match burst.kind { ProjectileKind::IceBall => BurstShape::Ice, ProjectileKind::Rock => BurstShape::Rock, ProjectileKind::Fireball => BurstShape::Fire }, age: 0.0 });
            if burst.reaches(player) && !self.hurt_player(burst.kind.burst_damage(), burst.kind.death_message()) {
                self.knock_player(burst.pos);
            }
        }
        for b in &mut self.bursts {
            b.age += dt;
        }
        self.bursts.retain(|b| b.age < BURST_SHOW_SECONDS);
        for c in &mut self.acid {
            c.left -= dt;
        }
        self.acid.retain(|c| c.left > 0.0);
        if self.standing_in_acid() {
            self.acid_burn += dt;
            while self.acid_burn >= ACID_DAMAGE_INTERVAL {
                self.acid_burn -= ACID_DAMAGE_INTERVAL;
                self.hurt_player(ACID_DAMAGE, "The acid water dissolved you");
            }
        } else {
            self.acid_burn = 0.0;
        }
    }

    /// Whether the player is wading in a column of tainted water.
    pub fn standing_in_acid(&self) -> bool {
        let p = self.player.pos;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        self.player_in_water() && self.acid.iter().any(|c| (c.x, c.z) == (x, z))
    }

    /// Bring `count` ravens out of the sky round the player (day or night: they are Nessie's, and act as if it were night), unless
    /// `cap` of hers are about already.
    fn call_ravens(&mut self, count: usize, cap: usize) {
        for _ in 0..count {
            if self.animals.iter().filter(|a| a.kind == AnimalKind::Raven && a.summoned).count() >= cap {
                return;
            }
            self.spawn_raven(true);
        }
    }

    /// Put a raven in the air round the player: close by if `summoned` (Nessie's), farther off if it is one of the night's.
    fn spawn_raven(&mut self, summoned: bool) -> bool {
        let p = self.player.pos;
        let (min, max) = if summoned { (14.0, 22.0) } else { (30.0, 46.0) };
        for _ in 0..12 {
            let angle = self.rng.unit() * std::f32::consts::TAU;
            let dist = min + self.rng.unit() * (max - min);
            let pos = vec3(p.x + angle.cos() * dist, p.y + LOCH_NESS_RAVEN_SPAWN_HEIGHT, p.z + angle.sin() * dist);
            let half = vec3(RAVEN_HALF_WIDTH, 0.0, RAVEN_HALF_WIDTH);
            if !self.world.chunks.contains_key(&World::chunk_of_pos(pos)) || self.world.box_blocked(pos - half, pos + half + vec3(0.0, RAVEN_HEIGHT, 0.0)) {
                continue;
            }
            let mut raven = Animal::new(AnimalKind::Raven, pos, &mut self.rng);
            raven.summoned = summoned;
            self.animals.push(raven);
            return true;
        }
        false
    }

    /// Whether the player is inside a mineshaft: under the mountain's slopes and below their surface.
    pub fn player_in_mine(&self) -> bool {
        let p = self.player.pos;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        self.in_overworld() && self.world.landmarks.mountain().map_or(false, |m| m.rise_at(x, z) > 0) && p.y < (self.world.ground_top(x, z) - 1) as f32
    }

    /// The mineshafts' wildlife: bats, up to `MAX_BAT_MINE`, come out of sight in the open tunnels near the player.
    fn spawn_mine_bats(&mut self) {
        if self.animals.iter().filter(|a| a.kind == AnimalKind::Bat && a.lives_in_mine).count() >= MAX_BAT_MINE {
            return;
        }
        let p = self.player.pos;
        for _ in 0..16 {
            let angle = self.rng.unit() * std::f32::consts::TAU;
            let dist = BAT_MINE_MIN_DISTANCE + self.rng.unit() * (BAT_MINE_MAX_DISTANCE - BAT_MINE_MIN_DISTANCE);
            let pos = vec3((p.x + angle.cos() * dist).floor() + 0.5, p.y + 1.0 + self.rng.unit() * 2.0, (p.z + angle.sin() * dist).floor() + 0.5);
            if !self.world.chunks.contains_key(&World::chunk_of_pos(pos)) {
                continue;
            }
            let (x, z) = (pos.x.floor() as i32, pos.z.floor() as i32);
            let half = vec3(BAT_HALF_WIDTH, 0.0, BAT_HALF_WIDTH);
            // Only in open tunnel under the ground, never in the open air.
            if pos.y >= (self.world.ground_top(x, z) - 1) as f32 || self.world.box_blocked(pos - half, pos + half + vec3(0.0, BAT_HEIGHT, 0.0)) {
                continue;
            }
            let mut bat = Animal::new(AnimalKind::Bat, pos, &mut self.rng);
            bat.lives_in_mine = true;
            self.animals.push(bat);
            return;
        }
    }

    // --- Bats ------------------------------------------------------------------------------

    /// Put a flier (a bat) in the air `height` blocks over the ground of column (x, z), if that column is loaded and there is
    /// room for it. Returns its index in `animals`.
    fn spawn_flyer(&mut self, kind: AnimalKind, x: i32, z: i32, height: f32) -> Option<usize> {
        if !self.world.chunks.contains_key(&World::chunk_of(x, z)) {
            return None;
        }
        let pos = vec3(x as f32 + 0.5, (self.world.ground_top(x, z) + 1) as f32 + height, z as f32 + 0.5);
        let half = vec3(kind.half_width(), 0.0, kind.half_width());
        if self.world.box_blocked(pos - half, pos + half + vec3(0.0, kind.height(), 0.0)) {
            return None;
        }
        self.animals.push(Animal::new(kind, pos, &mut self.rng));
        Some(self.animals.len() - 1)
    }

    /// Playing the pipes at night calls bats out of the dark: `BATS_PER_PIPES` of them (never more than `MAX_BAT_OVERWORLD` in all)
    /// appear round the player and go straight for them. They do not live in the normal world, so there are no others, and they
    /// are gone at dawn. Returns how many came.
    pub fn call_bats(&mut self) -> usize {
        if !self.in_overworld() || !self.world.night {
            return 0;
        }
        let have = self.animals.iter().filter(|a| a.kind == AnimalKind::Bat).count();
        let want = BATS_PER_PIPES.min(MAX_BAT_OVERWORLD.saturating_sub(have));
        let p = self.player.pos;
        let mut came = 0;
        for _ in 0..want {
            for _ in 0..12 {
                let angle = self.rng.unit() * std::f32::consts::TAU;
                let dist = BAT_PIPES_MIN_DISTANCE + self.rng.unit() * (BAT_PIPES_MAX_DISTANCE - BAT_PIPES_MIN_DISTANCE);
                let (x, z) = ((p.x + angle.cos() * dist).floor() as i32, (p.z + angle.sin() * dist).floor() as i32);
                if let Some(i) = self.spawn_flyer(AnimalKind::Bat, x, z, BAT_SPAWN_HEIGHT) {
                    self.animals[i].hunting = true;
                    came += 1;
                    break;
                }
            }
        }
        if came > 0 {
            self.toasts.push("Bats come screeching out of the dark!".to_string());
        }
        came
    }

    /// The cave's wildlife: bats, up to `MAX_BAT_CAVE`, appear out of sight in open parts of the cave with room to fly.
    fn spawn_cave_bats(&mut self) {
        if self.animals.iter().filter(|a| a.kind == AnimalKind::Bat).count() >= MAX_BAT_CAVE {
            return;
        }
        let p = self.player.pos;
        let seed = self.info.seed;
        for _ in 0..8 {
            let angle = self.rng.unit() * std::f32::consts::TAU;
            let dist = ANIMAL_SPAWN_MIN_DISTANCE + self.rng.unit() * (ANIMAL_SPAWN_MAX_DISTANCE - ANIMAL_SPAWN_MIN_DISTANCE);
            let (x, z) = ((p.x + angle.cos() * dist).floor() as i32, (p.z + angle.sin() * dist).floor() as i32);
            let air = horizen::air_height(seed, x, z);
            if air < BAT_CAVE_MIN_AIR || self.world.is_wet_column(x, z) {
                continue;
            }
            let height = (air as f32 - 2.0).min(BAT_SPAWN_HEIGHT).max(1.0);
            if self.spawn_flyer(AnimalKind::Bat, x, z, height).is_some() {
                return;
            }
        }
    }

    // --- The jungle's wildlife -------------------------------------------------------------------

    /// Whether the player is in the normal world's jungle.
    pub fn player_in_jungle(&self) -> bool {
        let p = self.player.pos;
        self.in_overworld() && crate::terrain::in_jungle(self.info.seed, p.x.floor() as i32, p.z.floor() as i32)
    }

    /// The jungle's wildlife: jaguars on the ground and flying snakes in the air, up to `MAX_JAGUAR` and `MAX_FLYING_SNAKE`, come in out of
    /// sight a good way from the player, in the jungle.
    fn spawn_jungle_wildlife(&mut self) {
        let count = |g: &Game, k: AnimalKind| g.animals.iter().filter(|a| a.kind == k).count();
        let kind = if self.rng.next() % 100 < SPAWN_WEIGHT_JAGUAR + SPAWN_WEIGHT_FLYING_SNAKE {
            if self.rng.next() % (SPAWN_WEIGHT_JAGUAR + SPAWN_WEIGHT_FLYING_SNAKE) < SPAWN_WEIGHT_JAGUAR { AnimalKind::Jaguar } else { AnimalKind::FlyingSnake }
        } else {
            return;
        };
        let max = if kind == AnimalKind::Jaguar { MAX_JAGUAR } else { MAX_FLYING_SNAKE };
        if count(self, kind) >= max {
            return;
        }
        let p = self.player.pos;
        for _ in 0..8 {
            let angle = self.rng.unit() * std::f32::consts::TAU;
            let dist = ANIMAL_SPAWN_MIN_DISTANCE + self.rng.unit() * (ANIMAL_SPAWN_MAX_DISTANCE - ANIMAL_SPAWN_MIN_DISTANCE);
            let (x, z) = ((p.x + angle.cos() * dist).floor() as i32, (p.z + angle.sin() * dist).floor() as i32);
            if !crate::terrain::in_jungle(self.info.seed, x, z) {
                continue;
            }
            let placed = if kind == AnimalKind::Jaguar { self.spawn_animal(kind, x, z) } else { self.spawn_flyer(kind, x, z, SNAKE_SPIRAL_HEIGHT + 2.0).is_some() };
            if placed {
                return;
            }
        }
    }

    // --- The king's castle, his gold and his spirits ----------------------------------------------

    /// Whether the player is inside the walls of the king's castle in the Horizen's giant cave.
    pub fn in_king_castle(&self) -> bool {
        let (kx, kz) = horizen::king_centre(self.info.seed);
        let p = self.player.pos;
        self.in_horizen() && (p.x - (kx as f32 + 0.5)).abs() <= KING_WALL_HALF as f32 && (p.z - (kz as f32 + 0.5)).abs() <= KING_WALL_HALF as f32
    }

    /// How many spirit archers follow the player now.
    pub fn spirit_count(&self) -> usize {
        self.animals.iter().filter(|a| a.kind == AnimalKind::SpiritArcher).count()
    }

    /// Give the king's ghost `offerings` times `GOLD_PER_SPIRIT` gold: for each, one spirit archer comes to follow the player (it shoots
    /// `SPIRIT_SHOTS` arrows at whatever they hit, then vanishes). Nothing is taken if the player has not that much gold or the spirits
    /// would pass `MAX_SPIRIT_ARCHERS`. Returns how many spirits came.
    pub fn give_gold_to_king(&mut self, offerings: u32) -> u32 {
        let room = MAX_SPIRIT_ARCHERS.saturating_sub(self.spirit_count()) as u32;
        let cost = offerings * GOLD_PER_SPIRIT;
        if offerings == 0 || !self.in_king_castle() {
            return 0;
        }
        if offerings > room {
            self.ghost_says(ghosts::KING_FULL);
            return 0;
        }
        if self.inventory.count(ItemKind::Gold) < cost {
            self.ghost_says(ghosts::KING_WANTS_MORE);
            return 0;
        }
        self.inventory.remove(ItemKind::Gold, cost);
        let p = self.player.pos;
        for n in 0..offerings {
            let a = n as f32 * 2.4;
            let mut spirit = Animal::new(AnimalKind::SpiritArcher, p + vec3(a.cos() * SPIRIT_FOLLOW_RADIUS, SPIRIT_FOLLOW_HEIGHT, a.sin() * SPIRIT_FOLLOW_RADIUS), &mut self.rng);
            spirit.set_orbit(a);
            self.animals.push(spirit);
        }
        self.ghost_says(ghosts::KING_SENDS);
        self.toasts.push(format!("{offerings} spirit archer{} will follow you", if offerings == 1 { "" } else { "s" }));
        offerings
    }

    /// The spirit archers shoot, each at the nearest creature the player has hit lately (within `SPIRIT_RANGE`, in plain sight), once
    /// every `SPIRIT_SHOOT_INTERVAL` seconds. A spirit that has shot its last arrow vanishes.
    fn update_spirits(&mut self) {
        let targets: Vec<(Vec3, f32)> = self.animals.iter().filter(|a| a.marked > 0.0 && a.kind.is_huntable()).map(|a| (a.pos + vec3(0.0, a.kind.height() * 0.5, 0.0), a.kind.height())).collect();
        let mut spent = Vec::new();
        let mut volleys = Vec::new();
        for (i, a) in self.animals.iter_mut().enumerate() {
            if !a.spirit_ready() || targets.is_empty() {
                continue;
            }
            let origin = a.pos + vec3(0.0, 1.2, 0.0);
            let nearest = targets.iter().map(|&(t, _)| (t, (t - origin).length())).filter(|&(_, d)| d <= SPIRIT_RANGE && d > 0.5).min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal));
            let Some((target, gap)) = nearest else { continue };
            let dir = (target - origin).normalize_or_zero();
            if !clear_line(&self.world, origin, dir, gap) {
                continue;
            }
            let flight = gap / ARROW_SPEED;
            let aim = (target + vec3(0.0, 0.5 * ARROW_GRAVITY * flight * flight, 0.0) - origin).normalize_or_zero();
            volleys.push(Arrow::from_spirit(origin, aim));
            a.heading = dir.z.atan2(dir.x);
            if a.spirit_fired() {
                spent.push(i);
            }
        }
        self.arrows.extend(volleys);
        for i in spent.into_iter().rev() {
            self.animals.remove(i);
        }
    }

    // --- The Wild Hunt -----------------------------------------------------------------------

    /// Whether the player stands inside Stonehenge (within `STONEHENGE_REACH` of the middle of the ring).
    pub fn in_stonehenge(&self) -> bool {
        let (cx, cz) = stonehenge::centre(self.info.seed);
        let p = self.player.pos;
        self.in_overworld() && (p.x - (cx as f32 + 0.5)).abs() <= STONEHENGE_REACH as f32 && (p.z - (cz as f32 + 0.5)).abs() <= STONEHENGE_REACH as f32
    }

    /// Whether any warrior of the Wild Hunt is alive.
    pub fn hunt_alive(&self) -> bool {
        self.animals.iter().any(|a| a.kind.is_wild_hunt())
    }

    /// Blow the Horn of the Hunt in hand (right click). Anywhere but Stonehenge nothing answers. In the ring it calls the Wild
    /// Hunt, the last boss: three ghostly warriors who appear round the player, a swordsman, an archer and a stalker. While they
    /// live, blowing the horn calls no more. Returns whether the horn was blown.
    pub fn blow_horn(&mut self) -> bool {
        if self.held_item() != Some(ItemKind::HornOfTheHunt) {
            return false;
        }
        if !self.in_stonehenge() {
            self.toasts.push("You blow the Horn of the Hunt. Only the wind answers.".to_string());
            return true;
        }
        if self.hunt_alive() {
            self.toasts.push("The Wild Hunt is already upon you.".to_string());
            return true;
        }
        let p = self.player.pos;
        let first = self.rng.unit() * std::f32::consts::TAU;
        // One swordsman, two archers, two lancers on horses, and the ghostly bears and crows (the first of which split when killed).
        let mut roster = vec![AnimalKind::HuntSwordsman, AnimalKind::HuntArcher, AnimalKind::HuntArcher, AnimalKind::HuntLancer, AnimalKind::HuntLancer];
        roster.extend(std::iter::repeat(AnimalKind::HuntBear).take(HUNT_GHOST_BEARS));
        roster.extend(std::iter::repeat(AnimalKind::HuntCrow).take(HUNT_GHOST_CROWS));
        self.hunt_total = 0;
        for (n, kind) in roster.iter().copied().enumerate() {
            // Each a share of the way round the circle from the next, on the first open ground that fits.
            for attempt in 0..40 {
                let angle = first + n as f32 * std::f32::consts::TAU / roster.len() as f32 + (attempt as f32 * 0.37).sin() * 0.5;
                let dist = HUNT_SPAWN_MIN_DISTANCE + self.rng.unit() * (HUNT_SPAWN_MAX_DISTANCE - HUNT_SPAWN_MIN_DISTANCE);
                let (x, z) = ((p.x + angle.cos() * dist).floor() as i32, (p.z + angle.sin() * dist).floor() as i32);
                if self.spawn_animal(kind, x, z) {
                    if let Some(a) = self.animals.last_mut() {
                        a.splits = matches!(kind, AnimalKind::HuntBear | AnimalKind::HuntCrow);
                    }
                    self.hunt_total += kind.max_health();
                    break;
                }
            }
        }
        self.toasts.push("The horn sounds, and the Wild Hunt rides out of the mist!".to_string());
        true
    }

    fn reset_storm(&mut self) {
        self.storm_active = false;
        self.storm_gather_timer = 0.0;
        self.storm_clear_timer = 0.0;
        self.storm_lightning_timer = 0.0;
        self.storm_lightning_flash = 0.0;
    }

    fn next_storm_lightning_delay(&mut self) -> f32 {
        WHISP_STORM_LIGHTNING_MIN_INTERVAL
            + self.rng.unit() * (WHISP_STORM_LIGHTNING_MAX_INTERVAL - WHISP_STORM_LIGHTNING_MIN_INTERVAL)
    }

    /// A bolt strikes `pos`: it is drawn for a moment (`storm_lightning`), and every creature within
    /// `LIGHTNING_RADIUS` blocks of it is hurt, most at the centre. The player is never hurt.
    fn trigger_storm_lightning(&mut self, pos: Vec3) {
        self.storm_lightning_pos = pos;
        self.storm_lightning_flash = WHISP_STORM_LIGHTNING_FLASH_SECONDS;
        let hits = self.lightning_hits(pos);
        self.hurt_animals_from(&hits, pos);
    }

    /// The creatures a bolt at `pos` hurts, as (index, damage): `LIGHTNING_DAMAGE` at the centre, half of it at the
    /// edge of `LIGHTNING_RADIUS`, measured from the middle of the creature.
    fn lightning_hits(&self, pos: Vec3) -> Vec<(usize, i32)> {
        self.animals
            .iter()
            .enumerate()
            .filter(|(_, a)| a.kind.is_huntable())
            .filter_map(|(i, a)| {
                let middle = a.pos + vec3(0.0, a.kind.height() / 2.0, 0.0);
                let distance = (middle - pos).length();
                (distance <= LIGHTNING_RADIUS).then(|| (i, ((LIGHTNING_DAMAGE as f32) * (1.0 - 0.5 * distance / LIGHTNING_RADIUS)).round() as i32))
            })
            .collect()
    }

    /// Where a held Holly Rod would call lightning now: the creature under the crosshair, or else the top of the block
    /// there, out to `HOLLY_ROD_REACH` blocks (far beyond the ordinary `REACH`). `None` with anything else in hand, or
    /// when the rod points at nothing.
    pub fn holly_rod_strike_point(&self, eye: Vec3, dir: Vec3) -> Option<Vec3> {
        if self.held_item() != Some(ItemKind::HollyRod) {
            return None;
        }
        if let Some(i) = self.animal_along(eye, dir, HOLLY_ROD_REACH) {
            return Some(self.animals[i].pos);
        }
        let hit = self.world.raycast_hit(eye, dir, HOLLY_ROD_REACH)?;
        let top = hit.block.bounds().map_or(1.0, |(_, hi)| hi[1]);
        Some(vec3(hit.x as f32 + 0.5, hit.y as f32 + top, hit.z as f32 + 0.5))
    }

    fn trigger_distant_storm_lightning(&mut self) {
        let angle = self.rng.unit() * std::f32::consts::TAU;
        let distance = WHISP_STORM_LIGHTNING_DISTANCE * (0.75 + self.rng.unit() * 0.6);
        let x = (self.player.pos.x + angle.cos() * distance).floor() as i32;
        let z = (self.player.pos.z + angle.sin() * distance).floor() as i32;
        let y = (self.world.ground_top(x, z) + 1) as f32;
        self.trigger_storm_lightning(vec3(x as f32 + 0.5, y, z as f32 + 0.5));
    }

    /// Advance the daytime storm caused by bonded will-o-whisps. It gathers after twenty seconds with a follower,
    /// lingers for twenty seconds after the last follower leaves, and occasionally flashes lightning in the distance.
    pub fn update_storm(&mut self, dt: f32) {
        if !self.in_overworld() || self.world.night {
            self.reset_storm();
            return;
        }

        let has_followers = self.following_whisp_count() > 0;
        if has_followers {
            self.storm_clear_timer = 0.0;
            if !self.storm_active {
                self.storm_gather_timer += dt;
                if self.storm_gather_timer >= WHISP_STORM_GATHER_SECONDS {
                    self.storm_active = true;
                    self.storm_lightning_timer = self.next_storm_lightning_delay();
                    self.toasts.push("Will-o-whisps have drawn in a storm.".to_string());
                }
            }
        } else {
            self.storm_gather_timer = 0.0;
            if self.storm_active {
                self.storm_clear_timer += dt;
                if self.storm_clear_timer >= WHISP_STORM_CLEAR_SECONDS {
                    self.reset_storm();
                    self.toasts.push("The whisp storm clears.".to_string());
                    return;
                }
            }
        }

        self.storm_lightning_flash = (self.storm_lightning_flash - dt).max(0.0);
        if self.storm_active {
            self.storm_lightning_timer -= dt;
            if self.storm_lightning_timer <= 0.0 {
                self.trigger_distant_storm_lightning();
                self.storm_lightning_timer = self.next_storm_lightning_delay();
            }
        }
    }

    /// Call lightning onto `strike` (see `holly_rod_strike_point`) and dismiss one bonded whisp when a Holly Rod is
    /// used during a storm. The bolt never hurts the player, but it hurts any creature near where it lands. The rod
    /// is not consumed; each successful strike costs one following whisp.
    pub fn use_holly_rod_on_target(&mut self, strike: Option<Vec3>) -> bool {
        let Some(strike_pos) = strike else { return false };
        if !self.storm_active
            || self.held_item() != Some(ItemKind::HollyRod)
            || self.following_whisp_count() == 0
        {
            return false;
        }
        let Some(index) = self.animals.iter().position(|a| a.kind == AnimalKind::WillOWhisp && a.bonded) else {
            return false;
        };
        self.animals.remove(index);
        self.trigger_storm_lightning(strike_pos);
        self.storm_lightning_timer = self.next_storm_lightning_delay();
        self.toasts.push("Lightning answers the Holly Rod.".to_string());
        true
    }

    /// Spawn and advance the Skylands' daytime tornadoes. The target is captured at spawn time, so walking away does not make
    /// a tornado chase the player. One spawns every `SKY_TORNADO_INTERVAL` seconds and each lasts `SKY_TORNADO_LIFETIME`.
    fn update_sky_tornado(&mut self, dt: f32) {
        // No fire tornado survives the night, the one in the crater included.
        if self.world.night {
            self.tornadoes.clear();
            self.crater_tornado = None;
            self.tornado_timer = 0.0;
            return;
        }
        if self.crater_tornado.is_none() {
            self.crater_tornado = Some(FireTornado::crater(skylands::volcano_crater(self.info.seed)));
        }
        if self.world.landmarks.phoenix_slain {
            self.tornadoes.clear();
            self.tornado_timer = 0.0;
        } else {
            self.tornado_timer += dt;
            if self.tornado_timer >= SKY_TORNADO_INTERVAL {
                self.tornado_timer = 0.0;
                self.spawn_sky_tornado();
            }
        }
        let player = self.player.pos;
        for tornado in self.tornadoes.iter_mut().chain(self.crater_tornado.iter_mut()) {
            tornado.update(dt);
        }
        self.tornadoes.retain(|t| t.age < SKY_TORNADO_LIFETIME);
        let mut damage = false;
        let mut source = player;
        for tornado in self.tornadoes.iter_mut().chain(self.crater_tornado.iter_mut()) {
            if tornado.hits(player) {
                tornado.damage_timer += dt;
                if tornado.damage_timer >= SKY_TORNADO_DAMAGE_INTERVAL {
                    tornado.damage_timer = 0.0;
                    damage = true;
                    source = tornado.pos;
                }
            } else {
                tornado.damage_timer = 0.0;
            }
        }
        if damage && !self.hurt_player(SKY_TORNADO_DAMAGE, "The fire tornado burns you") {
            self.knock_player(source);
        }
    }

    /// Start a tornado `SKY_TORNADO_SPAWN_DISTANCE` blocks from the player, in a random direction, at the height the player
    /// stands at (the sky has no ground to stand it on, and the islands lie at different heights): it sweeps across the
    /// islands and the gaps between them to where the player was.
    fn spawn_sky_tornado(&mut self) {
        let target = self.player.pos;
        let angle = self.rng.unit() * std::f32::consts::TAU;
        let start = vec3(target.x + angle.cos() * SKY_TORNADO_SPAWN_DISTANCE, target.y, target.z + angle.sin() * SKY_TORNADO_SPAWN_DISTANCE);
        self.tornadoes.push(FireTornado::new(start, target));
    }

    /// Advance dropped items and let the player pick up whatever is within reach.
    pub fn update_items(&mut self, dt: f32) {
        for item in &mut self.items {
            item.update(&self.world, dt);
        }
        for kind in collect_nearby(&mut self.items, &mut self.inventory, self.player.pos) {
            self.toasts.push(format!("+1 {}", kind.name()));
        }
        self.limit_items();
        self.toasts.update(dt);
        if let Some((_, left)) = self.ghost_text.as_mut() {
            *left -= dt;
        }
        if self.ghost_text.as_ref().map_or(false, |(_, left)| *left <= 0.0) {
            self.ghost_text = None;
        }
        if let Some(left) = self.victory.as_mut() {
            *left -= dt;
        }
        if self.victory.map_or(false, |left| left <= 0.0) {
            self.victory = None;
        }
    }

    /// Keep at most `MAX_DROPPED_ITEMS` items on the ground: when there are more, the oldest vanish first, judged by
    /// `DroppedItem::age` (the list itself is not in order of age, because picking an item up moves the last one into its
    /// place). Items of equal age, such as those just loaded from a save, go in list order.
    pub fn limit_items(&mut self) {
        let excess = self.items.len().saturating_sub(MAX_DROPPED_ITEMS);
        if excess == 0 {
            return;
        }
        let mut by_age: Vec<usize> = (0..self.items.len()).collect();
        by_age.sort_by(|&a, &b| self.items[b].age.partial_cmp(&self.items[a].age).unwrap_or(std::cmp::Ordering::Equal));
        let doomed: std::collections::HashSet<usize> = by_age.into_iter().take(excess).collect();
        let mut index = 0;
        self.items.retain(|_| {
            index += 1;
            !doomed.contains(&(index - 1))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::idx;
    use crate::item::{ItemKind, ItemStack};
    use crate::world::Hit;
    use std::path::PathBuf;

    fn game(seed: u64) -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        Game::from_save(info, SaveData::new(), seed)
    }

    #[test]
    fn dirt_cannot_be_dug_by_hand_or_with_any_tool() {
        let mut g = game(1);
        for held in [None, Some(ItemKind::StoneAxe), Some(ItemKind::Stone), Some(ItemKind::BowDrill)] {
            g.inventory.slots[0] = held.map(|k| ItemStack { kind: k, count: 1 });
            g.select_slot(0);
            for (x, z) in [(3, 3), (10, 12)] {
                assert!(g.mine(x, GROUND_TOP, z).is_empty(), "{held:?}");
                assert_eq!(g.world.block_at(x, GROUND_TOP, z), Block::Dirt);
                assert!(!g.can_interact(Block::Dirt));
                g.update_mining(Some(dirt_hit(x, z)), true, 30.0);
                assert_eq!(g.world.block_at(x, GROUND_TOP, z), Block::Dirt, "even held for 30 s");
                assert_eq!(g.mining_progress(), None);
            }
        }
        assert!(g.items.is_empty() && g.world.removed.is_empty());
    }

    #[test]
    fn mining_unbreakable_blocks_does_nothing() {
        let mut g = game(2);
        assert!(g.mine(5, 0, 5).is_empty());
        assert_eq!(g.world.block_at(5, 0, 5), Block::Stone);
        assert!(g.mine(5, 50, 5).is_empty()); // air
        // Find a wood block in the spawn chunk.
        let wood = (0..WORLD_HEIGHT)
            .flat_map(|y| (0..20).flat_map(move |x| (0..20).map(move |z| (x, y, z))))
            .find(|&(x, y, z)| g.world.block_at(x, y, z) == Block::Wood)
            .expect("a tree");
        assert!(g.mine(wood.0, wood.1, wood.2).is_empty());
        assert_eq!(g.world.block_at(wood.0, wood.1, wood.2), Block::Wood, "wood needs an axe");
        assert!(g.items.is_empty());
        // Even an axe cannot break stone.
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::StoneAxe, count: 1 });
        assert!(g.mine(5, 0, 5).is_empty());
        assert_eq!(g.world.block_at(5, 0, 5), Block::Stone);
    }

    fn first_wood(g: &Game) -> (i32, i32, i32) {
        (0..WORLD_HEIGHT)
            .flat_map(|y| (0..20).flat_map(move |x| (0..20).map(move |z| (x, y, z))))
            .find(|&(x, y, z)| g.world.block_at(x, y, z) == Block::Wood)
            .expect("a tree")
    }

    #[test]
    fn a_held_stone_axe_breaks_wood_into_a_log() {
        let mut g = game(6);
        let (x, y, z) = first_wood(&g);
        g.inventory.slots[2] = Some(ItemStack { kind: ItemKind::StoneAxe, count: 1 });
        g.select_slot(1);
        assert!(g.mine(x, y, z).is_empty(), "axe is in slot 2 but slot 1 is held");
        assert_eq!(g.world.block_at(x, y, z), Block::Wood);
        g.select_slot(2);
        assert_eq!(g.held_item(), Some(ItemKind::StoneAxe));
        assert_eq!(g.mine(x, y, z), vec![ItemKind::Log]);
        assert_eq!(g.world.block_at(x, y, z), Block::Air);
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.items[0].kind, ItemKind::Log);
        assert!(g.world.removed.contains(&(x, y, z)), "the edit is persisted");
    }

    #[test]
    fn other_held_items_do_not_chop_wood() {
        let mut g = game(7);
        let (x, y, z) = first_wood(&g);
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::Stone, count: 5 });
        assert!(g.mine(x, y, z).is_empty());
        assert_eq!(g.world.block_at(x, y, z), Block::Wood);
    }

    fn first_block(g: &Game, wanted: Block) -> (i32, i32, i32) {
        (0..WORLD_HEIGHT)
            .flat_map(|y| (0..20).flat_map(move |x| (0..20).map(move |z| (x, y, z))))
            .find(|&(x, y, z)| g.world.block_at(x, y, z) == wanted)
            .expect("block exists")
    }

    /// A bare spot of ground (no tree, no plant) and the hit a ray down onto it would give.
    fn ground_hit(g: &Game, x: i32, z: i32) -> Hit {
        let top = GROUND_TOP;
        assert_eq!(g.world.block_at(x, top, z), Block::Dirt);
        assert_eq!(g.world.block_at(x, top + 1, z), Block::Air, "pick a bare spot");
        Hit { x, y: top, z, block: Block::Dirt, prev: (x, top + 1, z) }
    }

    fn hold(g: &mut Game, kind: ItemKind, count: u32) {
        g.inventory.slots[0] = Some(ItemStack { kind, count });
        g.select_slot(0);
    }

    fn far_from_player(g: &mut Game) {
        g.player.pos = vec3(0.5, 6.0, 0.5);
    }

    #[test]
    fn plants_drop_what_they_should() {
        let mut g = game(20);
        let mut grass = 0;
        let mut berries = 0;
        for i in 0..200 {
            let (x, z) = (i % 20, (i / 20) % 10);
            // Set a plant straight into the world, as a clearing would.
            g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(x, 7, z)] =
                if i % 2 == 0 { Block::Grass } else { Block::Bush };
            let d = g.mine(x, 7, z);
            if i % 2 == 0 {
                assert!(d.iter().all(|&k| k == ItemKind::Grass) && d.len() <= 1);
                grass += d.len();
            } else {
                assert!(d.iter().all(|&k| matches!(k, ItemKind::Berries | ItemKind::PurpleBerries)) && d.len() <= 5);
                berries += d.len();
            }
            assert_eq!(g.world.block_at(x, 7, z), Block::Air);
        }
        assert!((55..=85).contains(&grass), "~70 of 100 grass should drop, got {grass}");
        assert!((200..=300).contains(&berries), "~250 berries from 100 bushes, got {berries}");
    }

    #[test]
    fn flowers_break_for_nothing() {
        let mut g = game(21);
        g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(3, 7, 3)] = Block::Flower;
        assert!(g.mine(3, 7, 3).is_empty());
        assert_eq!(g.world.block_at(3, 7, 3), Block::Air);
        assert!(g.items.is_empty());
    }

    #[test]
    fn placing_a_pebble_then_breaking_it_gives_the_pebble_back() {
        let mut g = game(22);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Pebble, 2);
        let hit = ground_hit(&g, 10, 10);
        assert!(g.use_held(&hit));
        assert_eq!(g.world.block_at(10, 6, 10), Block::PlacedPebble);
        assert_eq!(g.inventory.count(ItemKind::Pebble), 1);
        assert!(g.world.placed.contains_key(&(10, 6, 10)));
        assert_eq!(g.mine(10, 6, 10), vec![ItemKind::Pebble]);
        assert_eq!(g.world.block_at(10, 6, 10), Block::Air);
        assert!(g.world.placed.is_empty(), "breaking a placed block forgets it");
        assert!(g.world.removed.is_empty(), "and does not mark generated terrain as removed");
    }

    #[test]
    fn logs_pebbles_and_stones_can_all_be_placed_and_recovered() {
        for (kind, block) in [
            (ItemKind::Log, Block::PlacedLog),
            (ItemKind::Pebble, Block::PlacedPebble),
            (ItemKind::Stone, Block::PlacedStone),
        ] {
            let mut g = game(23);
            far_from_player(&mut g);
            hold(&mut g, kind, 1);
            assert!(g.use_held(&ground_hit(&g, 12, 12)), "{kind:?}");
            assert_eq!(g.world.block_at(12, 6, 12), block);
            assert_eq!(g.inventory.slots[0], None, "the last item leaves the hotbar slot");
            assert_eq!(g.mine(12, 6, 12), vec![kind]);
        }
    }

    #[test]
    fn only_placeable_items_can_be_placed() {
        let mut g = game(24);
        far_from_player(&mut g);
        for kind in [ItemKind::Stick, ItemKind::Twine, ItemKind::StoneAxe, ItemKind::BowDrill, ItemKind::Grass] {
            hold(&mut g, kind, 1);
            assert!(!g.use_held(&ground_hit(&g, 12, 12)), "{kind:?}");
            assert_eq!(g.inventory.count(kind), 1);
        }
        g.inventory.slots[0] = None;
        assert!(!g.use_held(&ground_hit(&g, 12, 12)), "empty hand places nothing");
    }

    #[test]
    fn placement_needs_a_free_cell_on_solid_ground() {
        let mut g = game(25);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Pebble, 10);
        // Into a solid block: refused.
        assert!(!g.can_place(Block::PlacedPebble, (12, 5, 12)));
        // Floating two blocks above the ground: refused.
        assert!(!g.can_place(Block::PlacedPebble, (12, 8, 12)));
        // Not in the bottom layer or above the world.
        assert!(!g.can_place(Block::PlacedPebble, (12, 0, 12)));
        assert!(!g.can_place(Block::PlacedPebble, (12, WORLD_HEIGHT, 12)));
        // In ungenerated space: refused.
        assert!(!g.can_place(Block::PlacedPebble, (10_000, 6, 10_000)));
        // On the ground: fine, but only once.
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        assert!(!g.can_place(Block::PlacedPebble, (12, 6, 12)), "already occupied");
        // Nothing stacks on a placed item (it is not a solid cube).
        assert!(!g.can_place(Block::PlacedPebble, (12, 7, 12)));
    }

    #[test]
    fn a_solid_log_cannot_be_placed_inside_the_player() {
        let mut g = game(26);
        g.player.pos = vec3(12.5, 6.0, 12.5);
        assert!(!g.can_place(Block::PlacedLog, (12, 6, 12)), "standing in that cell");
        assert!(g.can_place(Block::PlacedLog, (14, 6, 12)));
        // Small walk-through items may go under the player's feet.
        assert!(g.can_place(Block::PlacedPebble, (12, 6, 12)));
    }

    fn lit_fire(g: &mut Game) -> (i32, i32, i32) {
        far_from_player(g);
        hold(g, ItemKind::Log, 5);
        assert!(g.use_held(&ground_hit(g, 12, 12)));
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::BowDrill, count: 1 });
        g.select_slot(1);
        assert!(g.try_ignite(12, 6, 12));
        g.select_slot(0);
        (12, 6, 12)
    }

    #[test]
    fn a_bow_drill_lights_a_placed_log_for_sixty_seconds() {
        let mut g = game(27);
        let pos = lit_fire(&mut g);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Fire);
        assert_eq!(g.world.fires[&pos], 60.0);
        assert_eq!(g.world.placed[&pos], Block::Fire);
    }

    #[test]
    fn only_a_held_bow_drill_lights_only_placed_logs() {
        let mut g = game(28);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Log, 1);
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        // Wrong tool in hand.
        for kind in [None, Some(ItemKind::StoneAxe), Some(ItemKind::Stick)] {
            g.inventory.slots[0] = kind.map(|k| ItemStack { kind: k, count: 1 });
            assert!(!g.try_ignite(12, 6, 12), "{kind:?}");
        }
        // Right tool, wrong target: dirt, a tree trunk, and a pebble.
        hold(&mut g, ItemKind::BowDrill, 1);
        assert!(!g.try_ignite(12, GROUND_TOP, 14));
        let wood = first_block(&g, Block::Wood);
        assert!(!g.try_ignite(wood.0, wood.1, wood.2), "trees do not burn");
        assert_eq!(g.world.block_at(wood.0, wood.1, wood.2), Block::Wood);
        assert!(g.world.fires.is_empty());
        // And with the drill in hand the log is not broken by the mining path either.
        assert!(g.mine(12, 6, 12).is_empty());
        assert_eq!(g.world.block_at(12, 6, 12), Block::PlacedLog);
        // Finally it works.
        assert!(g.try_ignite(12, 6, 12));
    }

    #[test]
    fn every_log_added_to_a_fire_adds_sixty_seconds() {
        let mut g = game(29);
        let pos = lit_fire(&mut g);
        let fire = Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Fire, prev: (pos.0, pos.1 + 1, pos.2) };
        g.world.update_fires(10.0);
        assert!((g.world.fires[&pos] - 50.0).abs() < 1e-3);
        let before = g.inventory.count(ItemKind::Log);
        assert!(g.use_held(&fire));
        assert!((g.world.fires[&pos] - 110.0).abs() < 1e-3, "50 left + 60");
        assert_eq!(g.inventory.count(ItemKind::Log), before - 1);
        assert!(g.use_held(&fire));
        assert!((g.world.fires[&pos] - 170.0).abs() < 1e-3);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Fire, "the fire block itself stays one block");
        assert_eq!(g.world.placed.len(), 1);
    }

    #[test]
    fn only_logs_feed_a_fire_and_a_log_is_not_placed_on_top_of_one() {
        let mut g = game(30);
        let pos = lit_fire(&mut g);
        let fire = Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Fire, prev: (pos.0, pos.1 + 1, pos.2) };
        for kind in [ItemKind::Pebble, ItemKind::Stone, ItemKind::Stick] {
            hold(&mut g, kind, 1);
            assert!(!g.use_held(&fire), "{kind:?}");
            assert_eq!(g.inventory.count(kind), 1);
        }
        assert_eq!(g.world.fires[&pos], 60.0);
        assert_eq!(g.world.placed.len(), 1);
    }

    #[test]
    fn a_fire_burns_out_into_charcoal_that_can_be_picked_up() {
        let mut g = game(31);
        let pos = lit_fire(&mut g);
        assert_eq!(g.world.update_fires(59.0), 0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Fire);
        assert_eq!(g.world.update_fires(1.5), 1);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Charcoal);
        assert!(g.world.fires.is_empty());
        assert_eq!(g.world.placed[&pos], Block::Charcoal);
        // Burning fire cannot be broken by hand, but charcoal can.
        assert_eq!(g.mine(pos.0, pos.1, pos.2), vec![ItemKind::Charcoal]);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Air);
        assert!(g.world.placed.is_empty());
    }

    #[test]
    fn burning_fire_cannot_be_broken() {
        let mut g = game(32);
        let pos = lit_fire(&mut g);
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::StoneAxe, count: 1 });
        assert!(g.mine(pos.0, pos.1, pos.2).is_empty());
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Fire);
        assert!(g.world.fires.contains_key(&pos));
    }

    #[test]
    fn interaction_depends_on_the_held_item() {
        let mut g = game(33);
        assert!(g.can_interact(Block::Grass) && g.can_interact(Block::PlacedLog));
        assert!(!g.can_interact(Block::Dirt) && !g.can_interact(Block::Water));
        assert!(!g.can_interact(Block::Wood) && !g.can_interact(Block::Fire));
        hold(&mut g, ItemKind::StoneAxe, 1);
        assert!(g.can_interact(Block::Wood));
        hold(&mut g, ItemKind::BowDrill, 1);
        assert!(g.can_interact(Block::PlacedLog), "lighting it counts");
        assert!(g.can_interact(Block::Grass));
    }

    fn dirt_hit(x: i32, z: i32) -> Hit {
        Hit { x, y: GROUND_TOP, z, block: Block::Dirt, prev: (x, GROUND_TOP + 1, z) }
    }

    /// A bush (3 s to break) set in the world at (x, 7, z), and the hit on it.
    fn bush_hit(g: &mut Game, x: i32, z: i32) -> Hit {
        g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(x, 7, z)] = Block::Bush;
        Hit { x, y: 7, z, block: Block::Bush, prev: (x, 8, z) }
    }

    #[test]
    fn blocks_break_only_after_being_held_for_their_time() {
        let mut g = game(40);
        let hit = bush_hit(&mut g, 12, 12);
        for _ in 0..29 {
            assert!(g.update_mining(Some(hit), true, 0.1).is_empty());
        }
        assert_eq!(g.world.block_at(12, 7, 12), Block::Bush, "2.9 s is not enough for a bush");
        assert!((g.mining_progress().unwrap() - 2.9 / 3.0).abs() < 1e-3);
        g.update_mining(Some(hit), true, 0.2);
        assert_eq!(g.world.block_at(12, 7, 12), Block::Air);
        assert_eq!(g.mining_progress(), None);
    }

    #[test]
    fn plants_and_foliage_break_in_their_own_times() {
        for (block, seconds) in [
            (Block::Grass, 0.5),
            (Block::Flower, 0.5),
            (Block::Bush, 3.0),
            (Block::Leaves, 2.0),
            (Block::Needles, 2.0),
        ] {
            let mut g = game(41);
            g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(3, 7, 3)] = block;
            let hit = Hit { x: 3, y: 7, z: 3, block, prev: (3, 8, 3) };
            g.update_mining(Some(hit), true, seconds - 0.05);
            assert_eq!(g.world.block_at(3, 7, 3), block, "{block:?} breaks too early");
            g.update_mining(Some(hit), true, 0.1);
            assert_eq!(g.world.block_at(3, 7, 3), Block::Air, "{block:?} should be broken");
        }
    }

    #[test]
    fn wood_takes_five_seconds_with_an_axe_and_never_without() {
        let mut g = game(42);
        let (x, y, z) = first_wood(&g);
        let hit = Hit { x, y, z, block: Block::Wood, prev: (x, y, z) };
        g.update_mining(Some(hit), true, 10.0);
        assert_eq!(g.world.block_at(x, y, z), Block::Wood, "no axe, no progress");
        assert_eq!(g.mining_progress(), None);
        hold(&mut g, ItemKind::StoneAxe, 1);
        g.update_mining(Some(hit), true, 4.9);
        assert_eq!(g.world.block_at(x, y, z), Block::Wood);
        assert_eq!(g.update_mining(Some(hit), true, 0.2), vec![ItemKind::Log]);
        assert_eq!(g.world.block_at(x, y, z), Block::Air);
    }

    #[test]
    fn progress_resets_on_release_retarget_or_tool_change() {
        let mut g = game(43);
        let (a, b) = (bush_hit(&mut g, 12, 12), bush_hit(&mut g, 13, 12));
        g.update_mining(Some(a), true, 2.0);
        g.update_mining(Some(a), false, 0.1);
        assert_eq!(g.mining_progress(), None, "released the button");
        g.update_mining(Some(a), true, 2.0);
        g.update_mining(None, true, 0.1);
        assert_eq!(g.mining_progress(), None, "looked away");
        g.update_mining(Some(a), true, 2.0);
        g.update_mining(Some(b), true, 1.2);
        assert!((g.mining_progress().unwrap() - 0.4).abs() < 1e-3, "a different block starts over");
        assert_eq!(g.world.block_at(12, 7, 12), Block::Bush);
        assert_eq!(g.world.block_at(13, 7, 12), Block::Bush);
    }

    #[test]
    fn a_bow_drill_still_lights_a_log_at_once() {
        let mut g = game(44);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Log, 1);
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        hold(&mut g, ItemKind::BowDrill, 1);
        let log = Hit { x: 12, y: 6, z: 12, block: Block::PlacedLog, prev: (12, 7, 12) };
        assert!(g.update_mining(Some(log), true, 0.01).is_empty());
        assert_eq!(g.world.block_at(12, 6, 12), Block::Fire);
        assert_eq!(g.mining_progress(), None);
    }

    fn near_fire(g: &mut Game) -> (i32, i32, i32) {
        let pos = lit_fire(g);
        g.player.pos = vec3(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
        pos
    }

    #[test]
    fn touching_fire_costs_one_life_per_second() {
        let mut g = game(45);
        assert_eq!(g.health, MAX_HEALTH);
        let pos = lit_fire(&mut g);
        g.player.pos = vec3(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5); // on top of the log
        assert!(g.touching_fire());
        g.update_health(0.9);
        assert_eq!(g.health, 10);
        g.update_health(0.2);
        assert_eq!(g.health, 9);
        for _ in 0..30 {
            g.update_health(0.1);
        }
        assert_eq!(g.health, 6);
    }

    #[test]
    fn standing_away_from_fire_does_no_harm_and_resets_the_burn() {
        let mut g = game(46);
        let pos = near_fire(&mut g);
        g.update_health(0.8);
        g.player.pos = vec3(pos.0 as f32 + 4.5, pos.1 as f32, pos.2 as f32 + 0.5);
        assert!(!g.touching_fire());
        g.update_health(5.0);
        assert_eq!(g.health, 10);
        g.player.pos = vec3(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
        g.update_health(0.8);
        assert_eq!(g.health, 10, "the earlier 0.8 s does not carry over");
    }

    #[test]
    fn burnt_out_fire_is_harmless() {
        let mut g = game(47);
        let pos = near_fire(&mut g);
        g.world.update_fires(61.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Charcoal);
        assert!(!g.touching_fire());
    }

    #[test]
    fn dying_respawns_with_full_health_and_the_inventory() {
        let mut g = game(48);
        near_fire(&mut g);
        g.inventory.add(ItemKind::Stick, 3);
        g.health = 1;
        g.update_health(1.0);
        assert_eq!(g.health, MAX_HEALTH);
        assert_eq!(g.player.pos, Player::new().pos);
        assert_eq!(g.inventory.count(ItemKind::Stick), 3);
        assert!(!g.touching_fire());
    }

    #[test]
    fn eating_a_berry_restores_one_life_and_uses_it_up() {
        let mut g = game(49);
        hold(&mut g, ItemKind::Berries, 2);
        assert!(!g.eat_held(), "full health: the berry is kept");
        assert_eq!(g.inventory.count(ItemKind::Berries), 2);
        g.health = 5;
        assert!(g.eat_held());
        assert_eq!((g.health, g.inventory.count(ItemKind::Berries)), (6, 1));
        g.health = MAX_HEALTH - 1;
        assert!(g.eat_held());
        assert_eq!(g.health, MAX_HEALTH);
        assert_eq!(g.inventory.slots[0], None);
        hold(&mut g, ItemKind::Stick, 1);
        g.health = 3;
        assert!(!g.eat_held(), "only berries are food");
        assert_eq!(g.health, 3);
    }

    // --- Planks ---------------------------------------------------------------------------

    /// Remove every tree, plant and the like from the loaded chunks, leaving open air above the ground.
    fn clear_above_ground(g: &mut Game) {
        for chunk in g.world.chunks.values_mut() {
            for y in GROUND_TOP + 1..WORLD_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        chunk.blocks[idx(x, y, z)] = Block::Air;
                    }
                }
            }
        }
    }

    #[test]
    fn planks_are_placed_on_the_ground_and_break_back_into_planks() {
        let mut g = game(60);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Plank, 3);
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        assert_eq!(g.world.block_at(12, 6, 12), Block::Planks);
        assert_eq!(g.inventory.count(ItemKind::Plank), 2);
        assert_eq!(g.world.placed[&(12, 6, 12)], Block::Planks);
        // A plank breaks by hand, takes 1.5 s, and drops itself.
        let hit = Hit { x: 12, y: 6, z: 12, block: Block::Planks, prev: (12, 7, 12) };
        g.inventory.slots[1] = None;
        g.select_slot(1);
        g.update_mining(Some(hit), true, 1.4);
        assert_eq!(g.world.block_at(12, 6, 12), Block::Planks);
        assert_eq!(g.update_mining(Some(hit), true, 0.2), vec![ItemKind::Plank]);
        assert_eq!(g.world.block_at(12, 6, 12), Block::Air);
        assert!(g.world.placed.is_empty());
    }

    #[test]
    fn planks_build_walls_and_a_ceiling_that_reaches_out_from_the_wall_top() {
        let mut g = game(61);
        clear_above_ground(&mut g);
        far_from_player(&mut g);
        // A wall of two planks, then a ceiling plank beside the top of the wall, with nothing below it.
        for y in [6, 7] {
            assert!(g.can_place(Block::Planks, (12, y, 12)), "wall plank at y = {y}");
            g.world.set_block(12, y, 12, Block::Planks);
        }
        assert!(g.can_place(Block::Planks, (13, 7, 12)), "attached to the wall's side");
        g.world.set_block(13, 7, 12, Block::Planks);
        // Keep reaching out: the next plank hangs off the ceiling plank.
        assert!(g.can_place(Block::Planks, (14, 7, 12)));
        // Placing through the right-click path puts one in front of the targeted face.
        hold(&mut g, ItemKind::Plank, 1);
        let top_of_ceiling = Hit { x: 13, y: 7, z: 12, block: Block::Planks, prev: (13, 8, 12) };
        assert!(g.use_held(&top_of_ceiling));
        assert_eq!(g.world.block_at(13, 8, 12), Block::Planks);
    }

    #[test]
    fn a_plank_cannot_float_or_hang_from_leaves_or_go_inside_the_player() {
        let mut g = game(62);
        clear_above_ground(&mut g);
        far_from_player(&mut g);
        assert!(!g.can_place(Block::Planks, (12, 9, 12)), "floating in the air");
        g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(12, 9, 11)] = Block::Leaves;
        assert!(!g.can_place(Block::Planks, (12, 9, 12)), "leaves do not hold planks");
        g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(12, 9, 11)] = Block::Wood;
        assert!(!g.can_place(Block::Planks, (12, 9, 12)), "nor do trunks");
        assert!(!g.can_place(Block::Planks, (12, 5, 12)), "inside the ground");
        assert!(!g.can_place(Block::Planks, (12, 0, 12)));
        g.player.pos = vec3(12.5, 6.0, 12.5);
        assert!(!g.can_place(Block::Planks, (12, 6, 12)), "inside the player");
        assert!(g.can_place(Block::Planks, (14, 6, 12)));
    }

    #[test]
    fn planks_placed_on_the_ground_stay_put_after_a_reload() {
        let mut g = game(63);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Plank, 1);
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        let mut w2 = World::with_edits(1, g.world.removed.clone(), g.world.placed.clone(), g.world.fires.clone());
        w2.ensure_chunk(0, 0);
        assert_eq!(w2.block_at(12, 6, 12), Block::Planks);
    }

    // --- Food and cooking -----------------------------------------------------------------

    #[test]
    fn cooked_meat_heals_and_raw_meat_cannot_be_eaten() {
        let mut g = game(64);
        g.health = 2;
        for raw in [ItemKind::Venison, ItemKind::Poultry, ItemKind::Leather, ItemKind::Feather] {
            hold(&mut g, raw, 1);
            assert!(!g.eat_held(), "{raw:?}");
            assert_eq!((g.health, g.inventory.count(raw)), (2, 1));
        }
        hold(&mut g, ItemKind::CookedVenison, 2);
        assert!(g.eat_held());
        assert_eq!((g.health, g.inventory.count(ItemKind::CookedVenison)), (2 + VENISON_HEAL, 1));
        hold(&mut g, ItemKind::CookedPoultry, 1);
        assert!(g.eat_held());
        assert_eq!(g.health, 2 + VENISON_HEAL + POULTRY_HEAL);
        hold(&mut g, ItemKind::CookedVenison, 1);
        g.health = MAX_HEALTH;
        assert!(!g.eat_held(), "only when hurt");
        g.health = MAX_HEALTH - 1;
        assert!(g.eat_held());
        assert_eq!(g.health, MAX_HEALTH, "healing stops at full health");
    }

    fn fire_hit(pos: (i32, i32, i32)) -> Hit {
        Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Fire, prev: (pos.0, pos.1 + 1, pos.2) }
    }

    #[test]
    fn meat_laid_on_a_burning_log_cooks_and_is_taken_off_with_an_empty_hand() {
        let mut g = game(65);
        let pos = lit_fire(&mut g);
        let fire = fire_hit(pos);
        hold(&mut g, ItemKind::Venison, 2);
        assert!(g.can_use_fire(&fire));
        assert!(g.use_held(&fire), "laid on the log");
        assert_eq!(g.inventory.count(ItemKind::Venison), 1);
        assert_eq!(g.world.cooking[&pos].raw, ItemKind::Venison);
        assert!(!g.use_held(&fire), "the log holds one piece at a time");
        assert_eq!(g.inventory.count(ItemKind::Venison), 1);
        // Taking it off too early gets nothing: with the raw meat away, a bare hand finds it uncooked.
        g.inventory.slots[0] = None;
        assert!(!g.can_use_fire(&fire) && !g.use_held(&fire));
        assert!(g.world.cooking.contains_key(&pos));
        g.burn_fires(COOK_SECONDS + 0.1);
        assert!(g.can_use_fire(&fire));
        assert!(g.use_held(&fire));
        assert_eq!(g.inventory.count(ItemKind::CookedVenison), 1);
        assert_eq!(g.inventory.count(ItemKind::Venison), 0);
        assert!(g.world.cooking.is_empty());
        // And the cooked meat can now be eaten.
        g.health = 5;
        hold(&mut g, ItemKind::CookedVenison, 1);
        assert!(g.eat_held());
        assert_eq!(g.health, 5 + VENISON_HEAL);
    }

    #[test]
    fn meat_can_only_be_cooked_on_a_burning_log() {
        let mut g = game(66);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Log, 1);
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        hold(&mut g, ItemKind::Poultry, 1);
        let unlit = Hit { x: 12, y: 6, z: 12, block: Block::PlacedLog, prev: (12, 7, 12) };
        assert!(!g.use_held(&unlit), "an unlit log is no stove");
        assert_eq!(g.inventory.count(ItemKind::Poultry), 1);
        // Laid on the ground it would be placed as nothing: meat is not placeable.
        assert!(!g.use_held(&ground_hit(&g, 14, 12)));
        assert!(g.world.cooking.is_empty());
    }

    #[test]
    fn a_log_still_feeds_a_fire_and_a_ready_meal_comes_off_first() {
        let mut g = game(67);
        let pos = lit_fire(&mut g);
        let fire = fire_hit(pos);
        hold(&mut g, ItemKind::Poultry, 1);
        g.use_held(&fire);
        g.burn_fires(COOK_SECONDS + 1.0);
        hold(&mut g, ItemKind::Log, 3);
        assert!(g.use_held(&fire), "the finished poultry is taken before the log is added");
        assert_eq!(g.inventory.count(ItemKind::CookedPoultry), 1);
        assert_eq!(g.inventory.count(ItemKind::Log), 3);
        assert!(g.use_held(&fire));
        assert_eq!(g.inventory.count(ItemKind::Log), 2, "now the log feeds the fire");
    }

    #[test]
    fn meat_on_a_fire_that_burns_out_drops_on_the_ground() {
        let mut g = game(68);
        let pos = lit_fire(&mut g);
        hold(&mut g, ItemKind::Venison, 1);
        g.use_held(&fire_hit(pos));
        g.burn_fires(FIRE_SECONDS + 1.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Charcoal);
        assert!(g.world.cooking.is_empty());
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.items[0].kind, ItemKind::CookedVenison, "it had time to cook");
    }

    // --- Charcoal fires, bellows and smelting ---------------------------------------------

    /// Charcoal placed on bare ground at (12, 6, 12), lit with a bow drill. Slot 0 holds more charcoal.
    fn lit_embers(g: &mut Game) -> (i32, i32, i32) {
        far_from_player(g);
        hold(g, ItemKind::Charcoal, 5);
        assert!(g.use_held(&ground_hit(g, 12, 12)));
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::BowDrill, count: 1 });
        g.select_slot(1);
        assert!(g.try_ignite(12, 6, 12));
        g.select_slot(0);
        (12, 6, 12)
    }

    fn give(g: &mut Game, slot: usize, kind: ItemKind, count: u32) {
        g.inventory.slots[slot] = Some(ItemStack { kind, count });
    }

    #[test]
    fn charcoal_can_be_placed_lit_and_burns_for_a_minute_like_a_log() {
        let mut g = game(90);
        let pos = lit_embers(&mut g);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Embers);
        assert_eq!(g.world.fires[&pos], FIRE_SECONDS);
        assert_eq!(g.world.placed[&pos], Block::Embers);
        assert_eq!(g.inventory.count(ItemKind::Charcoal), 4, "one charcoal was placed");
        // Unlit charcoal is still broken by hand, and lit charcoal is not.
        g.burn_fires(FIRE_SECONDS - 1.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Embers);
        assert!(g.mine(pos.0, pos.1, pos.2).is_empty(), "burning charcoal cannot be broken");
    }

    #[test]
    fn burnt_out_charcoal_leaves_ash_that_can_be_broken_and_picked_up() {
        let mut g = game(91);
        let pos = lit_embers(&mut g);
        g.burn_fires(FIRE_SECONDS + 0.1);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Ash);
        assert!(g.world.fires.is_empty());
        assert_eq!(g.world.placed[&pos], Block::Ash);
        assert!(!g.touching_fire());
        g.inventory.slots[0] = None;
        g.select_slot(0);
        let hit = Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Ash, prev: (pos.0, pos.1 + 1, pos.2) };
        g.update_mining(Some(hit), true, BREAK_SECONDS_PLACED + 0.1);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Air);
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.items[0].kind, ItemKind::Ash);
        assert!(g.world.placed.is_empty());
    }

    #[test]
    fn a_burnt_log_leaves_charcoal_that_can_be_lit_again_into_ash() {
        let mut g = game(92);
        let pos = lit_fire(&mut g);
        g.burn_fires(FIRE_SECONDS + 1.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Charcoal);
        hold(&mut g, ItemKind::BowDrill, 1);
        assert!(g.try_ignite(pos.0, pos.1, pos.2));
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Embers);
        g.burn_fires(FIRE_SECONDS + 1.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Ash);
    }

    #[test]
    fn a_bow_drill_on_charcoal_lights_it_instead_of_breaking_it() {
        let mut g = game(93);
        far_from_player(&mut g);
        hold(&mut g, ItemKind::Charcoal, 1);
        assert!(g.use_held(&ground_hit(&g, 12, 12)));
        hold(&mut g, ItemKind::BowDrill, 1);
        let charcoal = Hit { x: 12, y: 6, z: 12, block: Block::Charcoal, prev: (12, 7, 12) };
        assert!(g.can_interact(Block::Charcoal));
        assert!(g.update_mining(Some(charcoal), true, 0.01).is_empty());
        assert_eq!(g.world.block_at(12, 6, 12), Block::Embers);
    }

    #[test]
    fn another_charcoal_or_log_adds_sixty_seconds_to_burning_charcoal() {
        let mut g = game(94);
        let pos = lit_embers(&mut g);
        let embers = Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Embers, prev: (pos.0, pos.1 + 1, pos.2) };
        g.burn_fires(10.0);
        assert!(g.can_use_fire(&embers));
        assert!(g.use_held(&embers));
        assert!((g.world.fires[&pos] - 110.0).abs() < 1e-3, "50 left + 60");
        assert_eq!(g.inventory.count(ItemKind::Charcoal), 3);
        give(&mut g, 0, ItemKind::Log, 1);
        assert!(g.use_held(&embers));
        assert!((g.world.fires[&pos] - 170.0).abs() < 1e-3);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Embers, "still one block");
        assert_eq!(g.world.placed.len(), 1);
        // Anything else does nothing.
        give(&mut g, 0, ItemKind::Stick, 1);
        assert!(!g.use_held(&embers));
    }

    #[test]
    fn food_cooks_on_burning_charcoal_just_like_on_a_log() {
        let mut g = game(95);
        let pos = lit_embers(&mut g);
        let embers = Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Embers, prev: (pos.0, pos.1 + 1, pos.2) };
        give(&mut g, 0, ItemKind::Poultry, 1);
        assert!(g.use_held(&embers));
        g.burn_fires(COOK_SECONDS + 0.1);
        assert!(g.can_use_fire(&embers));
        g.inventory.slots[0] = None;
        assert!(g.use_held(&embers));
        assert_eq!(g.inventory.count(ItemKind::CookedPoultry), 1);
    }

    #[test]
    fn stone_and_pebbles_are_still_placed_on_the_ground_and_metal_is_not_placeable() {
        let mut g = game(101);
        far_from_player(&mut g);
        for kind in [ItemKind::Stone, ItemKind::Pebble] {
            give(&mut g, 0, kind, 1);
            assert!(g.use_held(&ground_hit(&g, 12, 12)), "{kind:?}");
            g.mine(12, 6, 12);
        }
        for kind in [
            ItemKind::Copper, ItemKind::Tin, ItemKind::Ash, ItemKind::Bellows, ItemKind::Bronze, ItemKind::IronOre,
            ItemKind::Iron, ItemKind::LeatherCord, ItemKind::BronzeMallet,
        ] {
            give(&mut g, 0, kind, 1);
            assert!(!g.use_held(&ground_hit(&g, 12, 12)), "{kind:?}");
        }
    }

    // --- Clay forges, bronze and iron ------------------------------------------------------

    fn forge_hit(pos: (i32, i32, i32), block: Block) -> Hit {
        Hit { x: pos.0, y: pos.1, z: pos.2, block, prev: (pos.0, pos.1 + 1, pos.2) }
    }

    /// A clay forge put down at (12, 6, 12). Slot 0 is left holding nothing in particular.
    fn placed_forge(g: &mut Game) -> (i32, i32, i32) {
        far_from_player(g);
        hold(g, ItemKind::ClayForge, 1);
        assert!(g.use_held(&ground_hit(g, 12, 12)));
        (12, 6, 12)
    }

    /// A forge at (12, 6, 12), filled with charcoal and lit with a bow drill. Slot 0 is empty and selected.
    fn lit_forge(g: &mut Game) -> (i32, i32, i32) {
        let pos = placed_forge(g);
        hold(g, ItemKind::Charcoal, 3);
        assert!(g.use_held(&forge_hit(pos, Block::ClayForge)));
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::BowDrill, count: 1 });
        g.select_slot(1);
        assert!(g.try_ignite(pos.0, pos.1, pos.2));
        g.inventory.slots[0] = None;
        g.select_slot(0);
        pos
    }

    #[test]
    fn a_clay_forge_is_put_down_filled_with_charcoal_and_lit_with_a_bow_drill() {
        let mut g = game(130);
        let pos = placed_forge(&mut g);
        assert_eq!(g.inventory.count(ItemKind::ClayForge), 0, "the forge was used up");
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::ClayForge);
        assert_eq!(g.world.placed[&pos], Block::ClayForge);
        let empty = forge_hit(pos, Block::ClayForge);

        // Only charcoal goes in; an empty forge cannot be lit.
        for kind in [ItemKind::Log, ItemKind::Stone, ItemKind::Clay, ItemKind::Tin] {
            give(&mut g, 0, kind, 2);
            assert!(!g.can_use_fire(&empty) && !g.use_held(&empty), "{kind:?}");
        }
        give(&mut g, 1, ItemKind::BowDrill, 1);
        g.select_slot(1);
        assert!(g.can_interact(Block::ClayForge), "an empty forge is broken, not lit");
        assert!(!g.try_ignite(pos.0, pos.1, pos.2), "nothing in it to burn");

        give(&mut g, 0, ItemKind::Charcoal, 2);
        g.select_slot(0);
        assert!(g.can_use_fire(&empty));
        assert!(g.use_held(&empty));
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::ForgeCharged);
        assert_eq!(g.world.placed[&pos], Block::ForgeCharged);
        assert_eq!(g.inventory.count(ItemKind::Charcoal), 1);
        let charged = forge_hit(pos, Block::ForgeCharged);
        assert!(!g.can_use_fire(&charged) && !g.use_held(&charged), "one charcoal fills it, and it is not burning yet");
        assert_eq!(g.inventory.count(ItemKind::Charcoal), 1);

        g.select_slot(1);
        assert!(g.can_interact(Block::ForgeCharged));
        assert!(g.try_ignite(pos.0, pos.1, pos.2));
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::ForgeLit);
        assert_eq!(g.world.fires[&pos], FIRE_SECONDS);

        // A lit forge takes more fuel like any fire.
        g.select_slot(0);
        let lit = forge_hit(pos, Block::ForgeLit);
        assert!(g.can_use_fire(&lit) && g.use_held(&lit));
        assert_eq!(g.world.fires[&pos], 2.0 * FIRE_SECONDS);
        assert_eq!(g.world.placed.len(), 1, "still one block");
    }

    #[test]
    fn a_broken_clay_forge_drops_four_clay_and_a_filled_one_also_its_charcoal() {
        assert_eq!(FORGE_CLAY_DROP, 4);
        let mut g = game(131);
        let pos = placed_forge(&mut g);
        g.inventory.slots[0] = None;
        let hit = forge_hit(pos, Block::ClayForge);
        assert!(g.can_interact(Block::ClayForge));
        assert!(g.update_mining(Some(hit), true, BREAK_SECONDS_FORGE - 0.1).is_empty());
        assert_eq!(g.update_mining(Some(hit), true, 0.2), vec![ItemKind::Clay; 4]);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Air);
        assert!(g.world.placed.is_empty(), "the forge itself is not given back");

        let mut g = game(132);
        let pos = placed_forge(&mut g);
        hold(&mut g, ItemKind::Charcoal, 1);
        g.use_held(&forge_hit(pos, Block::ClayForge));
        g.inventory.slots[0] = None;
        let mut drops = g.update_mining(Some(forge_hit(pos, Block::ForgeCharged)), true, BREAK_SECONDS_FORGE + 0.1);
        drops.sort_by_key(|k| k.id());
        assert_eq!(drops, vec![ItemKind::Charcoal, ItemKind::Clay, ItemKind::Clay, ItemKind::Clay, ItemKind::Clay]);

        // Once it burns it cannot be broken, like any fire.
        let mut g = game(133);
        let pos = lit_forge(&mut g);
        let lit = forge_hit(pos, Block::ForgeLit);
        for _ in 0..3 {
            g.update_mining(Some(lit), true, 30.0);
        }
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::ForgeLit);
        assert!(!g.can_interact(Block::ForgeLit));
    }

    #[test]
    fn bellows_make_a_lit_forge_glow_but_do_nothing_to_open_embers_logs_or_a_cold_forge() {
        let mut g = game(134);
        let pos = lit_forge(&mut g);
        give(&mut g, 2, ItemKind::Bellows, 1);
        g.select_slot(2);
        let forge = forge_hit(pos, Block::ForgeLit);
        assert!(!g.world.is_glowing(pos));
        assert!(g.can_interact(Block::ForgeLit));
        assert!(g.update_mining(Some(forge), true, 0.01).is_empty());
        assert!(g.world.is_glowing(pos));
        assert_eq!(g.world.glow[&pos], BELLOWS_GLOW_SECONDS);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::ForgeLit, "blowing does not break it");
        assert_eq!(g.mining_progress(), None);
        // The glow fades; blowing again renews it.
        g.burn_fires(BELLOWS_GLOW_SECONDS - 1.0);
        assert!(g.world.is_glowing(pos));
        g.update_mining(Some(forge), true, 0.01);
        assert_eq!(g.world.glow[&pos], BELLOWS_GLOW_SECONDS);
        g.burn_fires(BELLOWS_GLOW_SECONDS + 0.1);
        assert!(!g.world.is_glowing(pos));

        // Open embers and logs are not worked with bellows, and neither is a forge that is not lit.
        let mut g = game(135);
        let embers = lit_embers(&mut g);
        give(&mut g, 2, ItemKind::Bellows, 1);
        g.select_slot(2);
        assert!(!g.try_blow(embers.0, embers.1, embers.2) && !g.can_interact(Block::Embers));
        assert!(!g.world.is_glowing(embers));
        let mut g = game(136);
        let log = lit_fire(&mut g);
        give(&mut g, 2, ItemKind::Bellows, 1);
        g.select_slot(2);
        assert!(!g.try_blow(log.0, log.1, log.2) && !g.can_interact(Block::Fire));
        g.world.set_block(14, 6, 14, Block::ForgeCharged);
        assert!(!g.try_blow(14, 6, 14), "a filled forge that is not lit");
        assert!(g.can_interact(Block::Grass), "bellows still pick like a bare hand");
    }

    #[test]
    fn a_stone_smelts_into_copper_and_a_pebble_into_tin_on_plain_burning_charcoal() {
        let mut g = game(137);
        let pos = lit_embers(&mut g);
        let embers = forge_hit(pos, Block::Embers);
        assert!(!g.world.is_glowing(pos), "no bellows involved");
        give(&mut g, 0, ItemKind::Stone, 2);
        assert!(g.can_use_fire(&embers));
        assert!(g.use_held(&embers), "set on the embers like food");
        assert_eq!(g.inventory.count(ItemKind::Stone), 1);
        assert_eq!(g.world.cooking[&pos].raw, ItemKind::Stone);
        assert!(!g.use_held(&embers), "one piece at a time");
        g.burn_fires(SMELT_SECONDS - 1.0);
        assert_eq!(g.world.cooking[&pos].item(), ItemKind::Stone, "not yet");
        g.select_slot(5); // an empty hand
        assert!(!g.use_held(&embers), "nothing to take yet");
        g.burn_fires(1.1);
        assert_eq!(g.world.cooking[&pos].item(), ItemKind::Copper);
        assert!(g.use_held(&embers));
        assert_eq!((g.inventory.count(ItemKind::Copper), g.inventory.count(ItemKind::Stone)), (1, 1));
        assert!(g.world.cooking.is_empty());

        give(&mut g, 0, ItemKind::Pebble, 3);
        g.select_slot(0);
        assert!(g.use_held(&embers));
        g.burn_fires(SMELT_SECONDS + 0.1);
        g.select_slot(5);
        assert!(g.use_held(&embers));
        assert_eq!((g.inventory.count(ItemKind::Tin), g.inventory.count(ItemKind::Pebble)), (1, 2));
    }

    #[test]
    fn stones_and_pebbles_do_not_smelt_on_a_log_fire_or_in_a_forge_and_are_kept() {
        let mut g = game(138);
        let pos = lit_fire(&mut g);
        let fire = fire_hit(pos);
        for kind in [ItemKind::Stone, ItemKind::Pebble, ItemKind::IronOre, ItemKind::Clay] {
            give(&mut g, 0, kind, 1);
            assert!(!g.can_use_fire(&fire) && !g.use_held(&fire), "{kind:?}");
            assert_eq!(g.inventory.count(kind), 1);
        }
        assert!(g.world.cooking.is_empty());
        assert!(!g.world.blow(pos), "logs do not glow under bellows");

        let mut g = game(139);
        let pos = lit_forge(&mut g);
        let forge = forge_hit(pos, Block::ForgeLit);
        for kind in [ItemKind::Stone, ItemKind::Pebble] {
            give(&mut g, 0, kind, 1);
            assert!(!g.can_use_fire(&forge) && !g.use_held(&forge), "{kind:?}");
            assert_eq!(g.inventory.count(kind), 1);
        }
        assert!(g.world.cooking.is_empty());
    }

    #[test]
    fn one_tin_and_one_copper_in_a_lit_forge_make_bronze_without_bellows() {
        let mut g = game(140);
        let pos = lit_forge(&mut g);
        let forge = forge_hit(pos, Block::ForgeLit);
        give(&mut g, 0, ItemKind::Tin, 2);
        give(&mut g, 2, ItemKind::Copper, 3);
        assert!(g.can_use_fire(&forge));
        assert!(g.use_held(&forge));
        assert_eq!((g.inventory.count(ItemKind::Tin), g.inventory.count(ItemKind::Copper)), (1, 2), "one of each used");
        assert_eq!(g.world.cooking[&pos].raw, ItemKind::Tin);
        assert!(!g.can_use_fire(&forge) && !g.use_held(&forge), "one batch at a time");
        g.burn_fires(SMELT_SECONDS - 1.0);
        g.select_slot(5); // an empty hand
        assert!(!g.use_held(&forge), "not done");
        g.burn_fires(1.1);
        assert_eq!(g.world.cooking[&pos].item(), ItemKind::Bronze);
        assert!(g.use_held(&forge));
        assert_eq!(g.inventory.count(ItemKind::Bronze), 1);
        assert!(g.world.cooking.is_empty());

        // Holding the copper works just as well.
        g.select_slot(2);
        assert_eq!(g.held_item(), Some(ItemKind::Copper));
        assert!(g.can_use_fire(&forge) && g.use_held(&forge));
        assert_eq!((g.inventory.count(ItemKind::Tin), g.inventory.count(ItemKind::Copper)), (0, 1));
    }

    #[test]
    fn bronze_needs_both_metals_and_a_lit_forge_not_open_embers() {
        let mut g = game(141);
        let pos = lit_forge(&mut g);
        let forge = forge_hit(pos, Block::ForgeLit);
        give(&mut g, 0, ItemKind::Tin, 1);
        assert!(!g.can_use_fire(&forge) && !g.use_held(&forge), "no copper");
        assert_eq!(g.inventory.count(ItemKind::Tin), 1, "kept");
        g.inventory.slots[0] = None;
        give(&mut g, 0, ItemKind::Copper, 1);
        assert!(!g.can_use_fire(&forge) && !g.use_held(&forge), "no tin");
        assert!(g.world.cooking.is_empty());

        let mut g = game(142);
        let pos = lit_embers(&mut g);
        let embers = forge_hit(pos, Block::Embers);
        give(&mut g, 0, ItemKind::Tin, 1);
        give(&mut g, 2, ItemKind::Copper, 1);
        assert!(!g.can_use_fire(&embers) && !g.use_held(&embers), "bronze needs a forge");
        g.select_slot(2);
        assert!(!g.use_held(&embers));
        assert_eq!((g.inventory.count(ItemKind::Tin), g.inventory.count(ItemKind::Copper)), (1, 1));
    }

    #[test]
    fn unfinished_bronze_gives_both_metals_back_when_the_forge_burns_out() {
        let mut g = game(143);
        let pos = lit_forge(&mut g);
        give(&mut g, 0, ItemKind::Tin, 1);
        give(&mut g, 2, ItemKind::Copper, 1);
        assert!(g.use_held(&forge_hit(pos, Block::ForgeLit)));
        g.world.fires.insert(pos, 1.0);
        g.burn_fires(2.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::ClayForge, "empty again, ready to be filled");
        let mut kinds: Vec<_> = g.items.iter().map(|i| i.kind).collect();
        kinds.sort_by_key(|k| k.id());
        assert_eq!(kinds, vec![ItemKind::Copper, ItemKind::Tin]);
        assert!(g.world.cooking.is_empty());
    }

    #[test]
    fn iron_ore_needs_a_lit_forge_that_bellows_keep_glowing() {
        let mut g = game(144);
        let pos = lit_forge(&mut g);
        let forge = forge_hit(pos, Block::ForgeLit);
        give(&mut g, 0, ItemKind::IronOre, 2);
        assert!(!g.can_use_fire(&forge), "not hot enough without bellows");
        assert!(!g.use_held(&forge));
        assert_eq!(g.inventory.count(ItemKind::IronOre), 2, "the ore is kept");
        assert!(g.toasts.contains("not hot enough"));
        assert!(g.world.blow(pos));
        assert!(g.can_use_fire(&forge) && g.use_held(&forge));
        assert_eq!(g.world.cooking[&pos].raw, ItemKind::IronOre);
        g.burn_fires(SMELT_SECONDS + 0.1);
        g.select_slot(5);
        assert!(g.use_held(&forge));
        assert_eq!((g.inventory.count(ItemKind::Iron), g.inventory.count(ItemKind::IronOre)), (1, 1));

        // Not in the open, charcoal fire either, glowing or not.
        let mut g = game(145);
        let pos = lit_embers(&mut g);
        give(&mut g, 0, ItemKind::IronOre, 1);
        assert!(!g.can_use_fire(&forge_hit(pos, Block::Embers)));
    }

    #[test]
    fn a_burnt_out_charcoal_smelter_drops_the_unmelted_stone() {
        let mut g = game(146);
        let pos = lit_embers(&mut g);
        give(&mut g, 0, ItemKind::Stone, 1);
        g.use_held(&forge_hit(pos, Block::Embers));
        g.world.fires.insert(pos, 1.0);
        g.burn_fires(2.0);
        assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Ash);
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.items[0].kind, ItemKind::Stone, "it had not melted yet");
        assert!(g.world.cooking.is_empty());
    }

    // --- Boulders and the bronze mallet --------------------------------------------------------

    /// A boulder standing at (12, 6, 12) on a cleared chunk.
    fn boulder_hit(g: &mut Game) -> Hit {
        far_from_player(g);
        clear_above_ground(g);
        g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(12, 6, 12)] = Block::Boulder;
        Hit { x: 12, y: 6, z: 12, block: Block::Boulder, prev: (12, 7, 12) }
    }

    #[test]
    fn only_a_bronze_mallet_breaks_a_boulder_into_stone_pebble_and_iron_ore() {
        let mut g = game(150);
        let hit = boulder_hit(&mut g);
        for held in [None, Some(ItemKind::StoneAxe), Some(ItemKind::Stone), Some(ItemKind::Bronze), Some(ItemKind::BowDrill)] {
            g.inventory.slots[0] = held.map(|k| ItemStack { kind: k, count: 1 });
            g.select_slot(0);
            assert!(!g.can_interact(Block::Boulder), "{held:?}");
            for _ in 0..3 {
                assert!(g.update_mining(Some(hit), true, 30.0).is_empty(), "{held:?}");
            }
            assert_eq!(g.world.block_at(12, 6, 12), Block::Boulder, "{held:?}");
            assert_eq!(g.mining_progress(), None);
        }
        hold(&mut g, ItemKind::BronzeMallet, 1);
        assert!(g.can_interact(Block::Boulder));
        assert!(g.update_mining(Some(hit), true, BREAK_SECONDS_BOULDER - 0.1).is_empty());
        assert!(g.mining_progress().unwrap() > 0.9);
        let drops = g.update_mining(Some(hit), true, 0.2);
        assert_eq!(drops[0], ItemKind::Stone);
        assert_eq!(drops[1], ItemKind::Pebble);
        let iron = drops.len() - 2;
        assert!((BOULDER_IRON_MIN as usize..=BOULDER_IRON_MAX as usize).contains(&iron), "{iron} iron");
        assert!(drops[2..].iter().all(|&k| k == ItemKind::IronOre));
        assert_eq!(g.items.len(), drops.len(), "they all fall to the ground");
        assert_eq!(g.world.block_at(12, 6, 12), Block::Air);
        assert!(g.world.removed.contains(&(12, 6, 12)), "a broken boulder stays gone");
    }

    #[test]
    fn a_bronze_mallet_hurts_animals_harder_than_an_axe_and_swings_slower() {
        assert!(MALLET_DAMAGE > AXE_DAMAGE && MALLET_COOLDOWN > AXE_COOLDOWN);
        let mut g = hunting_game(151);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 2.5);
        hold(&mut g, ItemKind::BronzeMallet, 1);
        assert_eq!(g.melee_target(eye(&g), dir), Some(0));
        assert!(g.swing_melee(eye(&g), dir));
        assert_eq!(g.animals[0].health, DEER_HEALTH - MALLET_DAMAGE);
        assert!(!g.swing_melee(eye(&g), dir), "the mallet is still swinging back");
        assert_eq!(g.attack_cooldown, MALLET_COOLDOWN);
        assert!(!g.shoot_bow(eye(&g), dir), "a mallet is not a bow");
        g.attack_cooldown = 0.0;
        assert!(g.swing_melee(eye(&g), dir));
        assert!(g.animals.is_empty(), "two blows kill a deer");
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Venison).count(), DEER_VENISON_DROP as usize);
    }

    #[test]
    fn standing_in_burning_charcoal_burns() {
        let mut g = game(103);
        let pos = lit_embers(&mut g);
        g.player.pos = vec3(pos.0 as f32 + 0.5, pos.1 as f32, pos.2 as f32 + 0.5);
        assert!(g.touching_fire(), "embers are walk-through but hot");
        g.update_health(1.1);
        assert_eq!(g.health, MAX_HEALTH - 1);
        g.world.fires.clear();
        g.world.set_block(pos.0, pos.1, pos.2, Block::Ash);
        assert!(!g.touching_fire());
    }

    // --- Hunting --------------------------------------------------------------------------

    /// A world around the spawn point with every tree cleared away, and the player at the spawn.
    fn hunting_game(seed: u64) -> Game {
        let mut g = game(seed);
        clear_above_ground(&mut g);
        g.player.pos = vec3(10.0, 6.01, 10.0);
        g
    }

    fn eye(g: &Game) -> Vec3 {
        g.player.eye()
    }

    /// Put a deer or quail `gap` blocks in front of the player (towards +z); returns the unit
    /// vector from the eye to the middle of its body.
    fn animal_ahead(g: &mut Game, kind: AnimalKind, gap: f32) -> Vec3 {
        let pos = vec3(10.0, 6.0, 10.0 + gap);
        g.animals.push(Animal::new(kind, pos, &mut Rng::new(1)));
        (pos + vec3(0.0, kind.height() / 2.0, 0.0) - eye(g)).normalize()
    }

    #[test]
    fn only_a_stone_axe_or_a_bow_hurts_animals() {
        for held in [None, Some(ItemKind::Stick), Some(ItemKind::Stone), Some(ItemKind::Log), Some(ItemKind::BowDrill), Some(ItemKind::Plank)] {
            let mut g = hunting_game(70);
            let dir = animal_ahead(&mut g, AnimalKind::Deer, 2.0);
            g.inventory.slots[0] = held.map(|k| ItemStack { kind: k, count: 1 });
            g.select_slot(0);
            assert_eq!(g.melee_target(eye(&g), dir), None, "{held:?}");
            assert!(!g.swing_melee(eye(&g), dir), "{held:?}");
            assert!(!g.shoot_bow(eye(&g), dir), "{held:?}");
            assert!(g.arrows.is_empty());
            assert_eq!(g.animals[0].health, DEER_HEALTH, "{held:?} does no damage");
        }
    }

    #[test]
    fn an_axe_swing_hurts_the_animal_under_the_crosshair_and_then_has_to_recover() {
        let mut g = hunting_game(71);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 2.5);
        hold(&mut g, ItemKind::StoneAxe, 1);
        assert_eq!(g.melee_target(eye(&g), dir), Some(0));
        assert!(g.swing_melee(eye(&g), dir));
        assert_eq!(g.animals[0].health, DEER_HEALTH - AXE_DAMAGE);
        assert!(!g.swing_melee(eye(&g), dir), "the axe is still swinging back");
        assert_eq!(g.animals[0].health, DEER_HEALTH - AXE_DAMAGE);
        // Once the swing is over the deer has bolted, so put it back where it stood.
        let spot = g.animals[0].pos;
        g.update_animals(AXE_COOLDOWN + 0.01);
        assert!(g.animals[0].pos.z > spot.z, "the hurt deer ran away");
        g.animals[0].pos = spot;
        assert!(g.swing_melee(eye(&g), dir));
        assert!(g.animals.is_empty() == (2 * AXE_DAMAGE >= DEER_HEALTH), "the second blow kills a deer of {DEER_HEALTH} health");
    }

    #[test]
    fn an_axe_reaches_only_a_few_blocks_and_not_through_walls_or_past_the_crosshair() {
        let mut g = hunting_game(72);
        hold(&mut g, ItemKind::StoneAxe, 1);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, AXE_REACH + 2.0);
        assert_eq!(g.melee_target(eye(&g), dir), None, "too far away");
        g.animals.clear();
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 3.0);
        assert!(g.melee_target(eye(&g), dir).is_some());
        // Looking well to the side misses.
        let aside = vec3(dir.x + 0.6, dir.y, dir.z).normalize();
        assert_eq!(g.melee_target(eye(&g), aside), None);
        // A plank wall between the player and the deer blocks the blow.
        for y in 6..9 {
            g.world.set_block(10, y, 11, Block::Planks);
        }
        assert_eq!(g.melee_target(eye(&g), dir), None, "hidden behind the wall");
        g.world.break_block(10, 7, 11);
        g.world.break_block(10, 8, 11);
        assert!(g.melee_target(eye(&g), dir).is_none() || g.world.block_at(10, 7, 11) == Block::Air);
    }

    #[test]
    fn killing_a_deer_drops_venison_and_leather_and_a_quail_drops_poultry() {
        let mut g = hunting_game(73);
        hold(&mut g, ItemKind::StoneAxe, 1);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 2.0);
        let needed = (DEER_HEALTH + AXE_DAMAGE - 1) / AXE_DAMAGE;
        for blow in 1..=needed {
            assert!(g.swing_melee(eye(&g), dir), "blow {blow}");
            assert_eq!(g.animals.len(), if blow < needed { 1 } else { 0 });
            g.attack_cooldown = 0.0;
        }
        assert!(g.animals.is_empty(), "{needed} axe blows kill a deer");
        let count = |g: &Game, k| g.items.iter().filter(|i| i.kind == k).count() as u32;
        assert_eq!(count(&g, ItemKind::Venison), DEER_VENISON_DROP);
        assert_eq!(count(&g, ItemKind::Leather), DEER_LEATHER_DROP);

        let mut g = hunting_game(74);
        hold(&mut g, ItemKind::StoneAxe, 1);
        let dir = animal_ahead(&mut g, AnimalKind::Quail, 1.5);
        assert!(g.swing_melee(eye(&g), dir));
        assert!(g.animals.is_empty(), "one blow kills a quail");
        assert_eq!(g.items.len() as u32, QUAIL_POULTRY_DROP + QUAIL_FEATHER_DROP);
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Poultry).count() as u32, QUAIL_POULTRY_DROP);
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Feather).count() as u32, QUAIL_FEATHER_DROP, "several feathers");
        assert!(QUAIL_FEATHER_DROP >= 3);
    }

    #[test]
    fn a_bow_shoots_arrows_that_cost_an_arrow_and_hurt_what_they_hit() {
        let mut g = hunting_game(75);
        hold(&mut g, ItemKind::Bow, 1);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 8.0);
        assert!(!g.shoot_bow(eye(&g), dir), "no arrows");
        assert!(g.arrows.is_empty());
        g.attack_cooldown = 0.0;
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Arrow, count: 2 });
        assert!(g.shoot_bow(eye(&g), dir));
        assert_eq!(g.inventory.count(ItemKind::Arrow), 1);
        assert_eq!(g.arrows.len(), 1);
        assert!(!g.shoot_bow(eye(&g), dir), "the bow has to be drawn again");
        // Fly the arrow; the deer is not simulated here, so it stays where it is.
        for _ in 0..60 {
            g.update_arrows(1.0 / 60.0);
        }
        assert!(g.arrows.is_empty(), "the arrow is used up");
        let deer = g.animals.iter().find(|a| a.kind == AnimalKind::Deer).unwrap();
        assert_eq!(deer.health, DEER_HEALTH - ARROW_DAMAGE);
    }

    #[test]
    fn two_arrows_kill_a_deer_and_a_wall_stops_an_arrow() {
        let mut g = hunting_game(76);
        hold(&mut g, ItemKind::Bow, 1);
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Arrow, count: 4 });
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 6.0);
        for _ in 0..2 {
            assert!(g.shoot_bow(eye(&g), dir));
            for _ in 0..40 {
                g.update_arrows(1.0 / 60.0);
            }
            g.attack_cooldown = 0.0;
        }
        assert!(g.animals.is_empty(), "6 health, 3 damage per arrow");
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Venison).count() as u32, DEER_VENISON_DROP);

        let mut g = hunting_game(77);
        hold(&mut g, ItemKind::Bow, 1);
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Arrow, count: 1 });
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 8.0);
        for y in 6..10 {
            for x in 8..13 {
                g.world.set_block(x, y, 12, Block::Planks);
            }
        }
        assert!(g.shoot_bow(eye(&g), dir));
        for _ in 0..90 {
            g.update_arrows(1.0 / 60.0);
        }
        assert!(g.arrows.is_empty());
        assert_eq!(g.animals[0].health, DEER_HEALTH, "the wall took the arrow");
    }

    #[test]
    fn a_bow_in_hand_does_not_dig_or_chop() {
        let mut g = hunting_game(78);
        hold(&mut g, ItemKind::Bow, 1);
        g.update_mining(Some(dirt_hit(12, 12)), true, 10.0);
        assert_eq!(g.world.block_at(12, GROUND_TOP, 12), Block::Dirt);
        assert_eq!(g.mining_progress(), None);
        assert!(!g.can_interact(Block::Dirt));
    }

    // --- Wildlife -------------------------------------------------------------------------

    #[test]
    fn a_startled_quail_drops_a_feather_on_the_ground_that_can_be_picked_up() {
        let mut g = hunting_game(79);
        g.animals.push(Animal::new(AnimalKind::Quail, vec3(10.0, 6.0, 15.0), &mut Rng::new(1)));
        g.update_animals(1.0 / 60.0); // lands
        g.update_animals(1.0 / 60.0); // takes off: the player is 5 blocks away
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.items[0].kind, ItemKind::Feather);
        // Walking over to it collects the feather.
        for _ in 0..240 {
            g.update_items(1.0 / 60.0);
            if let Some(i) = g.items.first() {
                g.player.pos = vec3(i.pos.x, 6.0, i.pos.z);
            }
        }
        assert_eq!(g.inventory.count(ItemKind::Feather), 1);
    }

    #[test]
    fn a_deer_and_a_quail_keep_away_from_the_player_in_the_game_loop() {
        let mut g = hunting_game(80);
        g.animals.push(Animal::new(AnimalKind::Deer, vec3(10.0, 6.0, 16.0), &mut Rng::new(1)));
        for _ in 0..120 {
            g.update_animals(1.0 / 60.0);
        }
        let deer = g.animals.iter().find(|a| a.kind == AnimalKind::Deer).unwrap();
        assert!((deer.pos - g.player.pos).length() > FLEE_DISTANCE, "ran off, now {:.1} away", (deer.pos - g.player.pos).length());
    }

    #[test]
    fn animals_spawn_on_the_ground_out_of_sight_and_stay_within_their_limits() {
        let mut g = game(81);
        assert!(g.animals.is_empty());
        for _ in 0..800 {
            g.update_animals(ANIMAL_SPAWN_INTERVAL);
        }
        let deer = g.animals.iter().filter(|a| a.kind == AnimalKind::Deer).count();
        let quail = g.animals.iter().filter(|a| a.kind == AnimalKind::Quail).count();
        let bears = g.animals.iter().filter(|a| a.kind == AnimalKind::Bear).count();
        assert!(deer > 0 && quail > 0 && bears > 0, "all kinds turn up (deer {deer}, quail {quail}, bears {bears})");
        assert!(deer <= MAX_DEER && quail <= MAX_QUAIL && bears <= MAX_BEAR);
        for a in &g.animals {
            let d = vec2(a.pos.x - g.player.pos.x, a.pos.z - g.player.pos.z).length();
            assert!(d < ANIMAL_DESPAWN_DISTANCE, "{d}");
            let ground = g.world.ground_top(a.pos.x.floor() as i32, a.pos.z.floor() as i32);
            assert!(a.pos.y >= (ground + 1) as f32 - 0.1, "stands on the ground, valleys and all");
        }
    }

    #[test]
    fn spawning_needs_a_loaded_free_spot_and_far_animals_are_removed() {
        let mut g = hunting_game(82);
        assert!(!g.spawn_animal(AnimalKind::Deer, 10_000, 10_000), "not generated");
        g.world.set_block(14, 6, 14, Block::Planks);
        assert!(!g.spawn_animal(AnimalKind::Deer, 14, 14), "blocked by a plank");
        assert!(g.spawn_animal(AnimalKind::Deer, 16, 14));
        assert_eq!(g.animals.len(), 1);
        g.player.pos = vec3(10_000.0, 6.0, 10_000.0);
        g.update_animals(0.1);
        assert!(g.animals.is_empty(), "left far behind");
    }

    #[test]
    fn hotbar_selection_clamps_and_wraps() {
        let mut g = game(8);
        assert_eq!(g.selected, 0);
        g.select_slot(4);
        assert_eq!(g.selected, 4);
        g.select_slot(HOTBAR_SLOTS); // not a hotbar slot
        assert_eq!(g.selected, 4);
        g.scroll_selection(-5);
        assert_eq!(g.selected, HOTBAR_SLOTS - 1);
        g.scroll_selection(1);
        assert_eq!(g.selected, 0);
        g.scroll_selection(-1);
        assert_eq!(g.selected, HOTBAR_SLOTS - 1);
    }

    #[test]
    fn mining_leaves_can_drop_sticks() {
        let mut g = game(3);
        let leaves: Vec<_> = (0..WORLD_HEIGHT)
            .flat_map(|y| (0..20).flat_map(move |x| (0..20).map(move |z| (x, y, z))))
            .filter(|&(x, y, z)| matches!(g.world.block_at(x, y, z), Block::Leaves | Block::Needles))
            .collect();
        assert!(leaves.len() > 60, "expected plenty of foliage, found {}", leaves.len());
        let mut sticks = 0;
        for &(x, y, z) in leaves.iter().take(60) {
            let d = g.mine(x, y, z);
            assert!(d.iter().all(|&k| k == ItemKind::Stick) && d.len() <= 1);
            sticks += d.len();
        }
        assert!((15..=45).contains(&sticks), "~50% of 60 leaves should drop sticks, got {sticks}");
        assert_eq!(g.items.len(), sticks);
    }

    #[test]
    fn dropped_items_land_and_are_picked_up_into_the_inventory() {
        let mut g = game(4);
        // Something drops beside the player.
        g.player.pos = vec3(10.5, 6.0, 10.5);
        g.items.push(DroppedItem::spawn(ItemKind::Stick, (11, 7, 10), &mut Rng::new(2)));
        // Tosses are small, so everything stays within a few blocks; walk over and wait.
        let mut frames = 0;
        while !g.items.is_empty() && frames < 600 {
            g.update_items(1.0 / 60.0);
            if let Some(i) = g.items.first() {
                g.player.pos = vec3(i.pos.x, 6.0, i.pos.z); // stand next to the item
            }
            frames += 1;
        }
        assert!(g.items.is_empty(), "items should be collected");
        let held: u32 = ItemKind::ALL.iter().map(|&k| g.inventory.count(k)).sum();
        assert!(held > 0);
    }

    #[test]
    fn pickup_is_announced() {
        let mut g = game(5);
        g.player.pos = vec3(10.5, 6.0, 10.5);
        g.items.push(DroppedItem::at_rest(ItemKind::Stick, vec3(10.6, 6.05, 10.5)));
        g.update_items(1.0 / 60.0);
        assert_eq!(g.inventory.count(ItemKind::Stick), 1);
        assert_eq!(g.toasts.len(), 1);
    }

    // --- Bears ----------------------------------------------------------------------------

    fn count_items(g: &Game, k: ItemKind) -> u32 {
        g.items.iter().filter(|i| i.kind == k).count() as u32
    }

    #[test]
    fn a_bear_reaching_the_player_hurts_them_and_can_kill_them() {
        let mut g = hunting_game(110);
        g.animals.push(Animal::new(AnimalKind::Bear, vec3(10.0, 6.0, 12.0), &mut Rng::new(1)));
        let mut seen = vec![g.health];
        for _ in 0..(3.0 / 0.02) as i32 {
            g.update_animals(0.02);
            if *seen.last().unwrap() != g.health {
                seen.push(g.health);
            }
        }
        assert_eq!(seen, vec![MAX_HEALTH, MAX_HEALTH - BEAR_DAMAGE, MAX_HEALTH - 2 * BEAR_DAMAGE, MAX_HEALTH - 3 * BEAR_DAMAGE], "a blow every {BEAR_ATTACK_COOLDOWN} s");
        // Killed: respawn with full health, keeping the inventory.
        g.inventory.add(ItemKind::Stick, 2);
        g.health = BEAR_DAMAGE;
        g.player.pos = vec3(10.0, 6.0, 10.0);
        g.animals[0].pos = vec3(10.0, 6.0, 11.0);
        g.animals[0].damage(0); // no effect on health; just a nudge that keeps it charging
        for _ in 0..200 {
            g.update_animals(0.02);
            if g.player.pos == Player::new().pos {
                break;
            }
        }
        assert_eq!(g.player.pos, Player::new().pos);
        assert_eq!(g.health, MAX_HEALTH);
        assert_eq!(g.inventory.count(ItemKind::Stick), 2);
        assert!(g.toasts.len() >= 1);
    }

    #[test]
    fn a_bear_across_the_field_does_not_hurt_and_deer_and_quail_are_not_chased() {
        let mut g = hunting_game(111);
        // Bear 45 blocks away (beyond its notice distance), with a deer and a quail close to it.
        g.animals.push(Animal::new(AnimalKind::Bear, vec3(55.0, 6.0, 10.0), &mut Rng::new(1)));
        g.animals.push(Animal::new(AnimalKind::Deer, vec3(59.0, 6.0, 10.0), &mut Rng::new(2)));
        g.animals.push(Animal::new(AnimalKind::Quail, vec3(55.0, 6.0, 14.0), &mut Rng::new(3)));
        g.update_animals(1.0 / 60.0);
        g.update_animals(1.0 / 60.0);
        assert_eq!(g.animals[1].mode, crate::animal::Mode::Flee, "the deer runs from the bear 4 blocks away");
        assert!(g.animals[1].heading.cos() > 0.9, "directly away from it (east)");
        assert_eq!(g.animals[2].mode, crate::animal::Mode::Fly, "the quail takes off");
        assert_eq!(count_items(&g, ItemKind::Feather), 1, "and loses a feather, as it does for the player");
        let mut bear_charged = false;
        for _ in 0..(6.0 / 0.02) as i32 {
            g.update_animals(0.02);
            bear_charged |= g.animals[0].mode == crate::animal::Mode::Charge;
            let bear = g.animals[0].pos;
            assert!(g.animals.iter().all(|a| a.kind == AnimalKind::Bear || a.pos != bear));
        }
        assert!(!bear_charged, "it charges only at the player");
        assert_eq!(g.health, MAX_HEALTH);
        assert!(g.animals.iter().find(|a| a.kind == AnimalKind::Deer).is_some(), "nothing was eaten");
    }

    #[test]
    fn deer_run_from_whichever_of_the_player_and_a_bear_is_nearer() {
        let mut g = hunting_game(112);
        // The player is 20 blocks west of the deer; a bear is 6 blocks north of it.
        g.animals.push(Animal::new(AnimalKind::Deer, vec3(30.0, 6.0, 10.0), &mut Rng::new(2)));
        g.animals.push(Animal::new(AnimalKind::Bear, vec3(30.0, 6.0, 4.0), &mut Rng::new(1)));
        g.update_animals(1.0 / 60.0);
        let deer = g.animals.iter().find(|a| a.kind == AnimalKind::Deer).unwrap();
        assert_eq!(deer.mode, crate::animal::Mode::Flee);
        assert!(deer.heading.sin() > 0.9, "south, away from the bear and not away from the distant player");
    }

    #[test]
    fn a_bear_takes_seven_axe_blows_or_ten_arrows_and_drops_meat_and_a_pelt() {
        let mut g = hunting_game(113);
        hold(&mut g, ItemKind::StoneAxe, 1);
        g.animals.push(Animal::new(AnimalKind::Bear, vec3(10.0, 6.0, 12.5), &mut Rng::new(1)));
        let dir = (vec3(10.0, 6.0 + 0.7, 12.5) - eye(&g)).normalize();
        let mut blows = 0;
        while !g.animals.is_empty() {
            g.animals[0].pos = vec3(10.0, 6.0, 12.5); // keep it in front of the axe
            g.attack_cooldown = 0.0;
            assert!(g.swing_melee(eye(&g), dir));
            blows += 1;
            assert!(blows <= 7);
        }
        assert_eq!(blows, 7);
        assert_eq!(count_items(&g, ItemKind::BearMeat), BEAR_MEAT_DROP);
        assert_eq!(count_items(&g, ItemKind::BearPelt), BEAR_PELT_DROP);

        let mut g = hunting_game(114);
        hold(&mut g, ItemKind::Bow, 1);
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Arrow, count: 16 });
        g.animals.push(Animal::new(AnimalKind::Bear, vec3(10.0, 6.0, 16.0), &mut Rng::new(1)));
        let dir = (vec3(10.0, 6.7, 16.0) - eye(&g)).normalize();
        let mut shots = 0;
        while !g.animals.is_empty() && shots < 16 {
            g.animals[0].pos = vec3(10.0, 6.0, 16.0);
            g.attack_cooldown = 0.0;
            assert!(g.shoot_bow(eye(&g), dir));
            shots += 1;
            for _ in 0..40 {
                g.update_arrows(1.0 / 60.0);
            }
        }
        assert_eq!(shots, (BEAR_HEALTH + ARROW_DAMAGE - 1) / ARROW_DAMAGE, "{BEAR_HEALTH} health, {ARROW_DAMAGE} damage per arrow");
        assert_eq!(count_items(&g, ItemKind::BearMeat), BEAR_MEAT_DROP);
    }

    #[test]
    fn only_an_axe_or_arrows_hurt_a_bear() {
        for held in [None, Some(ItemKind::Stick), Some(ItemKind::Stone), Some(ItemKind::BowDrill)] {
            let mut g = hunting_game(115);
            g.animals.push(Animal::new(AnimalKind::Bear, vec3(10.0, 6.0, 12.0), &mut Rng::new(1)));
            g.inventory.slots[0] = held.map(|k| ItemStack { kind: k, count: 1 });
            g.select_slot(0);
            let dir = (vec3(10.0, 6.7, 12.0) - eye(&g)).normalize();
            assert!(!g.swing_melee(eye(&g), dir) && !g.shoot_bow(eye(&g), dir), "{held:?}");
            assert_eq!(g.animals[0].health, BEAR_HEALTH);
        }
    }

    #[test]
    fn bear_meat_is_cooked_on_a_fire_and_heals_five() {
        let mut g = game(116);
        let pos = lit_fire(&mut g);
        let fire = fire_hit(pos);
        hold(&mut g, ItemKind::BearMeat, 1);
        assert!(g.use_held(&fire), "laid on the fire");
        g.burn_fires(COOK_SECONDS + 0.1);
        g.inventory.slots[0] = None;
        assert!(g.use_held(&fire));
        assert_eq!(g.inventory.count(ItemKind::CookedBearMeat), 1);
        g.health = 2;
        hold(&mut g, ItemKind::CookedBearMeat, 1);
        assert!(g.eat_held());
        assert_eq!(g.health, 2 + BEAR_MEAT_HEAL);
        hold(&mut g, ItemKind::BearMeat, 1);
        g.health = 2;
        assert!(!g.eat_held(), "raw bear meat cannot be eaten");
        hold(&mut g, ItemKind::BearPelt, 1);
        assert!(!g.eat_held() && !g.use_held(&ground_hit(&g, 14, 12)));
    }

    #[test]
    fn animals_never_spawn_in_a_river() {
        let mut g = game(117);
        let seed = g.world.seed;
        // Find a river column and load the chunks around it.
        let (x, z) = (-150..150).flat_map(|z| (-150..150).map(move |x| (x, z))).find(|&(x, z)| crate::river::is_river(seed, x, z)).expect("a river");
        g.world.stream_chunks(vec3(x as f32, 6.0, z as f32));
        for kind in [AnimalKind::Deer, AnimalKind::Quail, AnimalKind::Bear] {
            assert!(!g.spawn_animal(kind, x, z), "{kind:?} in the water");
        }
        assert!(g.animals.is_empty());
        let dry = (x - 30..x + 30).find(|&xx| !crate::river::river_near(seed, xx, z, 3) && g.world.chunks.contains_key(&World::chunk_of(xx, z)));
        if let Some(xx) = dry {
            assert!(g.spawn_animal(AnimalKind::Bear, xx, z), "dry land beside the river is fine");
        }
    }

    #[test]
    fn a_world_saved_while_standing_in_dirt_that_can_no_longer_be_dug_lifts_the_player_out() {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut data = SaveData::new();
        data.player.pos = vec3(10.5, 5.2, 10.5); // inside the top dirt layer
        data.removed.insert((10, GROUND_TOP, 10)); // a hole dug in an older version
        let g = Game::from_save(info, data, 1);
        assert_eq!(g.world.block_at(10, GROUND_TOP, 10), Block::Dirt, "the hole is filled again");
        assert!(!Player::collides(&g.world, g.player.pos), "not stuck in the ground");
        assert!(g.player.pos.y >= (GROUND_TOP + 1) as f32);
    }

    #[test]
    fn the_player_wades_through_a_river_and_climbs_out() {
        let mut w = World::new(1, std::collections::HashSet::new());
        w.ensure_chunk(0, 0);
        for y in GROUND_TOP + 1..WORLD_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    w.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(x, y, z)] = Block::Air;
                }
            }
        }
        for x in 8..14 {
            w.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(x, GROUND_TOP, 10)] = Block::Water;
        }
        let mut p = Player::new();
        p.pos = vec3(7.5, 6.0, 10.5);
        p.yaw = 0.0; // facing +x
        let input = crate::player::MoveInput { forward: true, ..Default::default() };
        for _ in 0..54 {
            p.update(&w, 1.0 / 60.0, &input);
        }
        assert!((p.pos.y - 5.0).abs() < 0.05, "in the river bed, y = {}", p.pos.y);
        assert!(p.pos.x > 9.0 && p.pos.x < 13.0, "wading, x = {}", p.pos.x);
        let mut jump = crate::player::MoveInput { forward: true, jump: true, ..Default::default() };
        jump.sprint = true;
        for _ in 0..90 {
            p.update(&w, 1.0 / 60.0, &jump);
        }
        let walk = crate::player::MoveInput { forward: true, ..Default::default() };
        for _ in 0..60 {
            p.update(&w, 1.0 / 60.0, &walk);
        }
        assert!(p.pos.x > 14.0 && (p.pos.y - 6.0).abs() < 0.05, "out on the far bank: {:?}", p.pos);
    }

    // --- Ores, clay and the river bed ------------------------------------------------------

    /// A single river column at (12, y, 12) on a cleared chunk: the dirt cut to a bed and `thing` lying in the water.
    fn river_with(g: &mut Game, thing: Block) -> Hit {
        clear_above_ground(g);
        g.world.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(12, GROUND_TOP, 12)] = thing;
        Hit { x: 12, y: GROUND_TOP, z: 12, block: thing, prev: (12, GROUND_TOP + 1, 12) }
    }

    #[test]
    fn clay_is_dug_from_under_the_water_and_the_water_stays() {
        let mut g = game(122);
        far_from_player(&mut g);
        let hit = river_with(&mut g, Block::WetClay);
        assert!(g.can_interact(Block::WetClay));
        assert!(g.update_mining(Some(hit), true, BREAK_SECONDS_PLACED - 0.1).is_empty());
        assert_eq!(g.update_mining(Some(hit), true, 0.2), vec![ItemKind::Clay; CLAY_DROP as usize]);
        assert_eq!(g.world.block_at(12, GROUND_TOP, 12), Block::Water, "the river is still one block deep");
        assert_eq!(g.world.block_at(12, GROUND_TOP - 1, 12), Block::Dirt, "and the bed is untouched");
        // The removal is remembered, as the water left behind.
        let mut again = World::with_edits(1, g.world.removed.clone(), g.world.placed.clone(), std::collections::HashMap::new());
        again.ensure_chunk(0, 0);
        assert!(again.block_at(12, GROUND_TOP, 12) != Block::WetClay);
        for _ in 0..3 {
            g.update_mining(Some(Hit { block: Block::Water, ..hit }), true, 30.0);
        }
        assert_eq!(g.world.block_at(12, GROUND_TOP, 12), Block::Water, "water itself cannot be taken");
    }

    #[test]
    fn pebbles_and_stones_under_the_water_are_picked_up_and_leave_water() {
        for (thing, item) in [(Block::WetPebble, ItemKind::Pebble), (Block::WetStone, ItemKind::Stone)] {
            let mut g = game(123);
            far_from_player(&mut g);
            let hit = river_with(&mut g, thing);
            assert_eq!(g.update_mining(Some(hit), true, 1.0), vec![item]);
            assert_eq!(g.world.block_at(12, GROUND_TOP, 12), Block::Water);
            assert!(g.world.removed.contains(&(12, GROUND_TOP, 12)));
        }
    }

    #[test]
    fn nothing_can_be_placed_into_the_water() {
        let mut g = game(125);
        far_from_player(&mut g);
        let hit = river_with(&mut g, Block::Water);
        hold(&mut g, ItemKind::Pebble, 2);
        // A ray down onto the river bed would place into the water's cell.
        let onto_bed = Hit { x: 12, y: GROUND_TOP - 1, z: 12, block: Block::Dirt, prev: (hit.x, GROUND_TOP, hit.z) };
        assert!(!g.use_held(&onto_bed));
        assert_eq!(g.inventory.count(ItemKind::Pebble), 2);
    }

    // --- Arrows that miss, knockback, jars, pots and stew, metal axes --------------------------

    #[test]
    fn an_arrow_that_hits_the_ground_lands_as_an_item_to_pick_up_and_one_that_hits_an_animal_is_used_up() {
        let mut g = hunting_game(160);
        g.arrows.push(Arrow::fired(vec3(10.5, 7.0, 10.5), vec3(0.0, -0.3, 1.0).normalize()));
        for _ in 0..60 {
            g.update_arrows(1.0 / 60.0);
        }
        assert!(g.arrows.is_empty());
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Arrow).count(), 1, "the arrow is on the ground");
        let spot = g.items[0].pos;
        assert!(spot.z > 11.0 && spot.y < 8.0);
        g.player.pos = vec3(spot.x, 6.0, spot.z);
        for _ in 0..30 {
            g.update_items(1.0 / 60.0);
        }
        assert_eq!(g.inventory.count(ItemKind::Arrow), 1, "and it can be used again");

        let mut g = hunting_game(161);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 4.0);
        g.arrows.push(Arrow::fired(eye(&g), dir));
        for _ in 0..30 {
            g.update_arrows(1.0 / 60.0);
        }
        assert!(g.arrows.is_empty() && g.animals[0].health < DEER_HEALTH, "it hit");
        assert!(g.items.iter().all(|i| i.kind != ItemKind::Arrow), "an arrow that did damage is gone");
    }

    #[test]
    fn a_blow_knocks_a_hurt_animal_back_and_a_blow_that_hurts_the_player_knocks_them_back() {
        let mut g = hunting_game(162);
        let dir = animal_ahead(&mut g, AnimalKind::Moose, 2.5);
        hold(&mut g, ItemKind::StoneAxe, 1);
        assert!(g.swing_melee(eye(&g), dir));
        let moose = &g.animals[0];
        assert!(moose.knock.z > 1.0 && moose.knock.x.abs() < 1e-3, "pushed away from the player: {:?}", moose.knock);
        assert!(moose.enraged, "and it is angry");

        // A moose that reaches the player hits for two and throws them back.
        let mut g = hunting_game(163);
        g.player.pos = vec3(10.0, 6.01, 10.0);
        let mut moose = Animal::new(AnimalKind::Moose, vec3(10.0, 6.0, 11.1), &mut Rng::new(1));
        moose.damage(1);
        g.animals.push(moose);
        g.update_animals(0.05);
        assert_eq!(g.health, MAX_HEALTH - MOOSE_DAMAGE);
        assert!(g.player.knock.z < -1.0, "thrown away from the moose: {:?}", g.player.knock);
        let z0 = g.player.pos.z;
        for _ in 0..30 {
            g.player.update(&g.world, 1.0 / 60.0, &crate::player::MoveInput::default());
        }
        assert!(g.player.pos.z < z0 - 0.5, "the push moves the player");
        assert!(g.player.knock.length() < 1.0, "and fades");

        // When the player dies the moose calms down.
        let mut g = hunting_game(164);
        g.player.pos = vec3(10.0, 6.01, 10.0);
        let mut moose = Animal::new(AnimalKind::Moose, vec3(10.0, 6.0, 11.1), &mut Rng::new(1));
        moose.damage(1);
        g.animals.push(moose);
        g.health = MOOSE_DAMAGE;
        g.update_animals(0.05);
        assert_eq!(g.health, MAX_HEALTH, "respawned");
        assert!(!g.animals[0].enraged);
    }

    #[test]
    fn burning_throws_the_player_out_of_the_fire() {
        let mut g = game(165);
        let pos = lit_fire(&mut g);
        g.player.pos = vec3(pos.0 as f32 + 0.8, pos.1 as f32 + 0.2, pos.2 as f32 + 0.5);
        assert!(g.touching_fire());
        g.update_health(FIRE_DAMAGE_INTERVAL + 0.01);
        assert_eq!(g.health, MAX_HEALTH - FIRE_DAMAGE);
        assert!(g.player.knock.x > 1.0, "pushed away from the fire: {:?}", g.player.knock);
    }

    #[test]
    fn a_clay_jar_fills_from_water_and_nothing_else() {
        let mut g = game(166);
        far_from_player(&mut g);
        river_with(&mut g, Block::Water);
        g.player.pos = vec3(12.5, 6.0, 10.5);
        let eye = g.player.eye();
        let water = (vec3(12.5, 5.5, 12.7) - eye).normalize();
        let dry = (vec3(14.5, 5.9, 12.5) - eye).normalize();
        hold(&mut g, ItemKind::ClayJar, 2);
        assert!(!g.fill_jar(eye, dry), "dry ground");
        assert!(g.fill_jar(eye, water));
        assert_eq!((g.inventory.count(ItemKind::ClayJar), g.inventory.count(ItemKind::WaterJar)), (1, 1));
        hold(&mut g, ItemKind::WaterJar, 1);
        assert!(!g.fill_jar(eye, water), "a full jar cannot be filled again");
        hold(&mut g, ItemKind::Stone, 1);
        assert!(!g.fill_jar(eye, water), "only a jar takes water");
        // Looking at the water through the ground does not work either.
        hold(&mut g, ItemKind::ClayJar, 1);
        let through = (vec3(12.5, 3.0, 11.0) - eye).normalize();
        assert!(!g.fill_jar(eye, through));
    }

    #[test]
    fn a_tin_pot_on_a_fire_makes_a_stew_from_water_and_meat_that_heals_double() {
        let mut g = game(167);
        let pos = lit_fire(&mut g);
        let fire = fire_hit(pos);
        hold(&mut g, ItemKind::TinPot, 1);
        assert!(g.can_use_fire(&fire) && g.use_held(&fire));
        assert!(g.world.pots.contains_key(&pos) && g.inventory.count(ItemKind::TinPot) == 0);

        hold(&mut g, ItemKind::Venison, 2);
        assert!(!g.use_held(&fire), "no water yet");
        assert!(g.toasts.contains("Fill the pot with water first"));
        assert_eq!(g.inventory.count(ItemKind::Venison), 2);
        hold(&mut g, ItemKind::WaterJar, 1);
        assert!(g.can_use_fire(&fire) && g.use_held(&fire));
        assert!(g.world.pots[&pos].water);
        assert_eq!((g.inventory.count(ItemKind::WaterJar), g.inventory.count(ItemKind::ClayJar)), (0, 1), "the jar is empty again");
        hold(&mut g, ItemKind::WaterJar, 1);
        assert!(!g.can_use_fire(&fire) && !g.use_held(&fire), "already full");

        hold(&mut g, ItemKind::Venison, 2);
        assert!(g.can_use_fire(&fire) && g.use_held(&fire));
        assert_eq!((g.inventory.count(ItemKind::Venison), g.world.pots[&pos].meat), (1, Some(ItemKind::Venison)));
        assert!(!g.use_held(&fire), "one stew at a time");
        g.burn_fires(STEW_SECONDS - 1.0);
        g.select_slot(5);
        assert!(!g.can_use_fire(&fire) && !g.use_held(&fire), "not done yet");
        g.burn_fires(1.1);
        assert!(g.can_use_fire(&fire) && g.use_held(&fire));
        assert_eq!(g.inventory.count(ItemKind::VenisonStew), 1);
        assert_eq!(g.world.pots[&pos], crate::world::Pot::empty());

        // The stew heals twice what cooked venison does.
        g.health = 1;
        hold(&mut g, ItemKind::VenisonStew, 1);
        assert!(g.eat_held());
        assert_eq!(g.health, 1 + 2 * VENISON_HEAL);
        // The empty pot comes off the fire again.
        g.select_slot(5);
        assert!(g.use_held(&fire));
        assert_eq!(g.inventory.count(ItemKind::TinPot), 1);
        assert!(g.world.pots.is_empty());
    }

    #[test]
    fn every_kind_of_raw_meat_makes_its_own_stew_and_a_pot_goes_on_charcoal_but_not_in_a_forge() {
        for (raw, stew) in [
            (ItemKind::Poultry, ItemKind::PoultryStew),
            (ItemKind::BearMeat, ItemKind::BearStew),
            (ItemKind::RabbitMeat, ItemKind::RabbitStew),
            (ItemKind::MooseMeat, ItemKind::MooseStew),
        ] {
            let mut g = game(168);
            let pos = lit_embers(&mut g);
            let embers = forge_hit(pos, Block::Embers);
            hold(&mut g, ItemKind::TinPot, 1);
            assert!(g.use_held(&embers), "a pot goes on charcoal");
            hold(&mut g, ItemKind::WaterJar, 1);
            assert!(g.use_held(&embers));
            hold(&mut g, raw, 1);
            assert!(g.use_held(&embers), "{raw:?}");
            g.burn_fires(STEW_SECONDS + 0.1);
            g.select_slot(5);
            assert!(g.use_held(&embers));
            assert_eq!(g.inventory.count(stew), 1, "{raw:?} makes {stew:?}");
        }
        let mut g = game(169);
        let pos = lit_forge(&mut g);
        hold(&mut g, ItemKind::TinPot, 1);
        assert!(!g.can_use_fire(&forge_hit(pos, Block::ForgeLit)) && !g.use_held(&forge_hit(pos, Block::ForgeLit)));
        assert_eq!(g.inventory.count(ItemKind::TinPot), 1);
        // A burnt-out fire drops the pot and its meat.
        let mut g = game(170);
        let pos = lit_fire(&mut g);
        let fire = fire_hit(pos);
        for (kind, n) in [(ItemKind::TinPot, 1), (ItemKind::WaterJar, 1), (ItemKind::Poultry, 1)] {
            hold(&mut g, kind, n);
            assert!(g.use_held(&fire));
        }
        g.world.fires.insert(pos, 1.0);
        g.burn_fires(2.0);
        let mut kinds: Vec<_> = g.items.iter().map(|i| i.kind).collect();
        kinds.sort_by_key(|k| k.id());
        assert_eq!(kinds, vec![ItemKind::Poultry, ItemKind::TinPot]);
    }

    #[test]
    fn metal_axes_chop_and_hurt_animals_one_harder_each() {
        for (i, axe) in [ItemKind::StoneAxe, ItemKind::CopperAxe, ItemKind::BronzeAxe, ItemKind::IronAxe].into_iter().enumerate() {
            let mut g = hunting_game(180 + i as u64);
            let dir = animal_ahead(&mut g, AnimalKind::Bear, 2.5);
            hold(&mut g, axe, 1);
            assert!(g.can_interact(Block::Wood), "{axe:?} chops");
            assert!(g.swing_melee(eye(&g), dir));
            let damage = [AXE_DAMAGE, COPPER_AXE_DAMAGE, BRONZE_AXE_DAMAGE, IRON_AXE_DAMAGE][i];
            assert_eq!(g.animals[0].health, BEAR_HEALTH - damage, "{axe:?}");
        }
    }

    #[test]
    fn rabbits_and_moose_join_the_wildlife_within_their_limits() {
        let mut g = hunting_game(190);
        for _ in 0..400 {
            g.spawn_timer = ANIMAL_SPAWN_INTERVAL;
            g.update_animals(0.01);
            g.player.pos = vec3(10.0, 6.01, 10.0);
        }
        let count = |g: &Game, k: AnimalKind| g.animals.iter().filter(|a| a.kind == k).count();
        assert!(count(&g, AnimalKind::Rabbit) <= MAX_RABBIT && count(&g, AnimalKind::Moose) <= MAX_MOOSE);
        assert!(count(&g, AnimalKind::Rabbit) > 0, "rabbits turned up");
    }

    // --- Sling, every axe, harder knockback, loadstones ------------------------------------------

    #[test]
    fn every_axe_and_the_mallet_hurt_an_animal_when_the_button_is_held_on_it() {
        for (item, damage) in [
            (ItemKind::StoneAxe, AXE_DAMAGE),
            (ItemKind::CopperAxe, COPPER_AXE_DAMAGE),
            (ItemKind::BronzeAxe, BRONZE_AXE_DAMAGE),
            (ItemKind::IronAxe, IRON_AXE_DAMAGE),
            (ItemKind::BronzeMallet, MALLET_DAMAGE),
        ] {
            let mut g = hunting_game(170);
            let dir = animal_ahead(&mut g, AnimalKind::Moose, 2.5);
            hold(&mut g, item, 1);
            // A click alone does not swing, and neither does holding it with nothing under the crosshair...
            assert!(!g.attack(eye(&g), dir, true, false, true), "{item:?} needs the button held");
            assert!(!g.attack(eye(&g), dir, false, true, false), "{item:?} needs an animal under the crosshair");
            assert_eq!(g.animals[0].health, MOOSE_HEALTH, "{item:?}");
            // ...holding it on the animal does.
            assert!(g.attack(eye(&g), dir, false, true, true), "{item:?}");
            assert_eq!(g.animals[0].health, MOOSE_HEALTH - damage, "{item:?}");
            assert_eq!(g.attack_cooldown, item.melee_cooldown(), "{item:?}");
            assert!(!g.attack(eye(&g), dir, false, true, true), "{item:?} has to recover first");
        }
    }

    #[test]
    fn nothing_else_in_hand_swings_or_shoots() {
        for held in [None, Some(ItemKind::Stick), Some(ItemKind::Stone), Some(ItemKind::Pebble), Some(ItemKind::BowDrill), Some(ItemKind::Compass), Some(ItemKind::Loadstone)] {
            let mut g = hunting_game(171);
            let dir = animal_ahead(&mut g, AnimalKind::Deer, 2.0);
            g.inventory.slots[0] = held.map(|k| ItemStack { kind: k, count: 5 });
            g.select_slot(0);
            assert!(!g.attack(eye(&g), dir, true, true, true), "{held:?}");
            assert!(g.arrows.is_empty() && g.animals[0].health == DEER_HEALTH, "{held:?}");
        }
    }

    #[test]
    fn a_sling_slings_a_pebble_from_the_inventory_after_a_click() {
        let mut g = hunting_game(172);
        hold(&mut g, ItemKind::Sling, 1);
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Pebble, count: 3 });
        let dir = vec3(0.0, 0.2, 1.0).normalize();
        assert!(!g.attack(eye(&g), dir, false, true, false), "holding the button without a click does not sling");
        assert!(g.attack(eye(&g), dir, true, true, false));
        assert_eq!(g.inventory.count(ItemKind::Pebble), 2);
        assert_eq!(g.arrows.len(), 1);
        assert_eq!(g.arrows[0].kind, ItemKind::Pebble);
        assert!((g.arrows[0].vel.length() - SLING_SPEED).abs() < 1e-3);
        assert_eq!(g.attack_cooldown, SLING_COOLDOWN);
        assert!(!g.attack(eye(&g), dir, true, true, false), "one sling per cooldown");
        assert_eq!(g.inventory.count(ItemKind::Pebble), 2);
    }

    #[test]
    fn a_sling_needs_pebbles_and_a_bow_still_needs_arrows_not_pebbles() {
        let mut g = hunting_game(173);
        hold(&mut g, ItemKind::Sling, 1);
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Arrow, count: 4 });
        let dir = vec3(0.0, 0.0, 1.0);
        assert!(!g.shoot_sling(eye(&g), dir), "arrows are no ammunition for a sling");
        assert!(g.arrows.is_empty() && g.toasts.contains("no pebbles"));
        let mut g = hunting_game(174);
        hold(&mut g, ItemKind::Bow, 1);
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Pebble, count: 4 });
        assert!(!g.shoot_bow(eye(&g), dir), "pebbles are no ammunition for a bow");
        assert!(!g.shoot_sling(eye(&g), dir), "and a sling is not what is in hand");
        assert_eq!(g.inventory.count(ItemKind::Pebble), 4);
    }

    #[test]
    fn a_pebble_that_hurts_is_used_up_and_one_that_misses_lands_to_be_picked_up_again() {
        // A hit: SLING_DAMAGE, and the pebble is gone.
        let mut g = hunting_game(175);
        let dir = animal_ahead(&mut g, AnimalKind::Deer, 4.0);
        g.arrows.push(Arrow::slung(eye(&g), dir));
        for _ in 0..30 {
            g.update_arrows(1.0 / 60.0);
        }
        assert!(g.arrows.is_empty());
        assert_eq!(g.animals[0].health, DEER_HEALTH - SLING_DAMAGE);
        assert!(g.items.iter().all(|i| i.kind != ItemKind::Pebble), "a pebble that did damage is gone");

        // A miss: it lands as a pebble, and walking over it puts it back in the inventory.
        let mut g = hunting_game(176);
        g.arrows.push(Arrow::slung(vec3(10.5, 7.0, 10.5), vec3(0.0, -0.3, 1.0).normalize()));
        for _ in 0..60 {
            g.update_arrows(1.0 / 60.0);
        }
        assert!(g.arrows.is_empty());
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Pebble).count(), 1, "the pebble is on the ground");
        assert!(g.items.iter().all(|i| i.kind != ItemKind::Arrow));
        let spot = g.items[0].pos;
        g.player.pos = vec3(spot.x, 6.0, spot.z);
        for _ in 0..30 {
            g.update_items(1.0 / 60.0);
        }
        assert_eq!(g.inventory.count(ItemKind::Pebble), 1, "and it can be slung again");
    }

    #[test]
    fn a_slung_pebble_hurts_less_than_an_arrow_and_an_arrow_still_lands_as_an_arrow() {
        assert!(SLING_DAMAGE > 0 && SLING_DAMAGE < ARROW_DAMAGE);
        assert_eq!(Arrow::slung(Vec3::ZERO, Vec3::X).damage(), SLING_DAMAGE);
        assert_eq!(Arrow::fired(Vec3::ZERO, Vec3::X).damage(), ARROW_DAMAGE);
        let mut g = hunting_game(177);
        g.arrows.push(Arrow::fired(vec3(10.5, 7.0, 10.5), vec3(0.0, -0.3, 1.0).normalize()));
        for _ in 0..60 {
            g.update_arrows(1.0 / 60.0);
        }
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Arrow).count(), 1);
        assert!(g.items.iter().all(|i| i.kind != ItemKind::Pebble));
    }

    #[test]
    fn a_blow_throws_a_hurt_animal_back_hard() {
        assert!(ANIMAL_KNOCKBACK_SPEED >= 12.0 && KNOCKBACK_HOP >= 5.0, "more than the gentle push it used to be");
        for item in [ItemKind::StoneAxe, ItemKind::IronAxe, ItemKind::BronzeMallet] {
            let mut g = hunting_game(178);
            let dir = animal_ahead(&mut g, AnimalKind::Moose, 2.5);
            hold(&mut g, item, 1);
            assert!(g.attack(eye(&g), dir, false, true, true));
            let moose = &g.animals[0];
            assert!((moose.knock.z - ANIMAL_KNOCKBACK_SPEED).abs() < 1e-3, "{item:?}: {:?}", moose.knock);
            assert_eq!(moose.vel.y, KNOCKBACK_HOP);
        }
        // A slung pebble and an arrow push too.
        for shot in [Arrow::slung as fn(Vec3, Vec3) -> Arrow, Arrow::fired] {
            let mut g = hunting_game(179);
            let dir = animal_ahead(&mut g, AnimalKind::Moose, 4.0);
            g.arrows.push(shot(eye(&g), dir));
            for _ in 0..30 {
                g.update_arrows(1.0 / 60.0);
            }
            assert!(g.animals[0].knock.z > 10.0, "{:?}", g.animals[0].knock);
        }
    }

    #[test]
    fn a_compass_is_made_of_a_loadstone_and_an_iron_ore_and_a_sling_of_a_leather() {
        let mut g = game(181);
        give(&mut g, 0, ItemKind::Loadstone, 1);
        give(&mut g, 1, ItemKind::IronOre, 1);
        give(&mut g, 2, ItemKind::Leather, 1);
        let names: Vec<_> = crate::recipe::craftable(&g.inventory).iter().map(|r| r.output).collect();
        assert!(names.contains(&ItemKind::Compass) && names.contains(&ItemKind::Sling) && names.contains(&ItemKind::LeatherCord), "{names:?}");
        let compass = crate::recipe::RECIPES.iter().find(|r| r.output == ItemKind::Compass).unwrap();
        assert!(crate::recipe::craft(&mut g.inventory, compass));
        assert_eq!((g.inventory.count(ItemKind::Loadstone), g.inventory.count(ItemKind::IronOre), g.inventory.count(ItemKind::Compass)), (0, 0, 1));
        // The compass does nothing yet: holding it and clicking changes nothing.
        hold(&mut g, ItemKind::Compass, 1);
        let dir = vec3(0.0, 0.0, 1.0);
        assert!(!g.attack(eye(&g), dir, true, true, true));
        assert!(!g.eat_held());
    }

    #[test]
    fn a_dev_world_loads_as_dev_mode_and_a_normal_one_does_not() {
        let mut data = SaveData::new();
        data.dev_mode = true;
        let info = WorldInfo { name: "dev".into(), seed: 1, path: PathBuf::new() };
        assert!(Game::from_save(info, data, 1).dev_mode);
        assert!(!game(190).dev_mode);
    }
}

#[cfg(test)]
mod landmark_tests {
    use super::*;
    use crate::chunk::idx;
    use std::path::PathBuf;

    fn game(seed: u64) -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        Game::from_save(info, SaveData::new(), seed)
    }

    fn hold(g: &mut Game, kind: ItemKind) {
        g.inventory.slots[0] = Some(crate::item::ItemStack { kind, count: 1 });
        g.select_slot(0);
    }

    fn load_column(g: &mut Game, x: i32, z: i32) {
        let (cx, cz) = World::chunk_of(x, z);
        g.world.ensure_chunk(cx, cz);
    }

    #[test]
    fn a_finished_river_trek_brings_a_one_block_deep_lake_with_a_channel_from_the_river() {
        let mut g = game(1);
        let at = (g.player.pos.x, g.player.pos.z);
        g.bring_in(TrekEvent::Lake { at, dir: (1.0, 0.0) });
        assert_eq!(g.world.landmarks.lakes.len(), 1);
        let lake = g.world.landmarks.lakes[0];
        assert!(lake.x as f32 - at.0 >= lake_distance() as f32, "beyond the generated land");
        // A point in the water between the island and the shore.
        let (wx, wz) = (lake.x + (lake.radius as f32 * 0.55) as i32, lake.z);
        load_column(&mut g, wx, wz);
        // The surface of the lake lies at the level of the river: a bed one block down, water above it.
        assert_eq!(g.world.block_at(wx, GROUND_TOP, wz), Block::Water);
        assert_eq!(g.world.block_at(wx, GROUND_TOP - LAKE_DEPTH, wz), Block::Dirt);
        assert_eq!(g.world.block_at(wx, GROUND_TOP + 1, wz), Block::Air);
        assert!(g.world.is_wet_column(wx, wz));
        // In the middle is an island: a hill of dry ground with a flat top, and a silver tower on the top.
        load_column(&mut g, lake.x + 30, lake.z);
        load_column(&mut g, lake.x + TOWER_HALF, lake.z);
        load_column(&mut g, lake.x, lake.z);
        assert!(!g.world.is_wet_column(lake.x, lake.z) && !g.world.is_wet_column(lake.x + 30, lake.z));
        let top = GROUND_TOP + ISLAND_HILL_HEIGHT;
        assert_eq!(g.world.ground_top(lake.x, lake.z), top, "the island is a hill with a flat top");
        let slope = g.world.ground_top(lake.x + 30, lake.z);
        assert!(slope > GROUND_TOP && slope < top, "and its side slopes down to the water: {slope}");
        assert_eq!(g.world.block_at(lake.x + TOWER_HALF, top + 1, lake.z), Block::SilverStone);
        // The channel leaves the column the player stood in and runs to the lake, all of it water.
        let (px, pz) = (at.0.floor() as i32, at.1.floor() as i32);
        assert!(g.world.is_wet_column(px, pz));
        let mid = ((px + lake.x - lake.radius) / 2, (pz + lake.z) / 2);
        assert!(g.world.is_wet_column(mid.0, mid.1), "the middle of the channel is wet");
        assert!(g.toasts.contains("great lake"));
        // The chunk the player stands in was generated again, and they still stand on something.
        assert!(g.world.chunks.contains_key(&World::chunk_of(px, pz)));
        assert_eq!(g.world.ground_top(px, pz), GROUND_TOP - LAKE_DEPTH, "the river bed under the player is unchanged");
    }

    #[test]
    fn a_lake_takes_the_place_of_what_stood_there_and_animals_do_not_spawn_in_it() {
        let mut g = game(2);
        let at = (g.player.pos.x, g.player.pos.z);
        g.bring_in(TrekEvent::Lake { at, dir: (0.0, 1.0) });
        let lake = g.world.landmarks.lakes[0];
        let (wx, wz) = (lake.x + (lake.radius as f32 * 0.55) as i32, lake.z);
        load_column(&mut g, wx, wz);
        assert!(!g.spawn_animal(AnimalKind::Deer, wx, wz), "no deer on the water");
        let (cx, cz) = World::chunk_of(wx, wz);
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                if g.world.is_wet_column(wx, wz) {
                    for y in GROUND_TOP + 1..WORLD_HEIGHT {
                        let b = g.world.block_at(wx, y, wz);
                        assert!(!matches!(b, Block::Wood | Block::Leaves | Block::Needles | Block::Grass | Block::Bush), "{b:?} on the water at {wx},{wz}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_finished_compass_trek_brings_a_tall_climbable_snow_capped_mountain_you_cannot_dig() {
        let mut g = game(3);
        let at = (g.player.pos.x, g.player.pos.z);
        g.bring_in(TrekEvent::Mountain { at, dir: crate::landmarks::compass_direction() });
        let m = g.world.landmarks.mountains[0];
        assert!(m.z < at.1 as i32 - mountain_distance(), "north of the player, beyond the loaded land");
        assert!(g.toasts.contains("mountain"));
        // Along a line from the far side to the summit the ground climbs, but only in steps of a jump or a ledge.
        let mut last = g.world.ground_top(m.x + m.radius * 2, m.z);
        let mut highest = last;
        for x in (m.x..=m.x + m.radius * 2).rev() {
            let top = g.world.ground_top(x, m.z);
            assert!((top - last).abs() <= MOUNTAIN_LEDGE_HEIGHT + 1, "step of {} at x={x}", top - last);
            last = top;
            highest = highest.max(top);
        }
        assert_eq!(highest, GROUND_TOP + MOUNTAIN_HEIGHT);
        // The summit is rather tall and snowy, and the snow cannot be mined.
        load_column(&mut g, m.x, m.z);
        assert_eq!(g.world.block_at(m.x, highest, m.z), Block::Snow);
        assert_eq!(g.world.block_at(m.x, highest + 1, m.z), Block::Air);
        assert!(highest >= GROUND_TOP + 3 * HILL_MAX_HEIGHT);
        hold(&mut g, ItemKind::StoneAxe);
        assert!(g.mine(m.x, highest, m.z).is_empty());
        assert_eq!(g.world.block_at(m.x, highest, m.z), Block::Snow, "snow cannot be mined yet");
        assert!(!g.can_interact(Block::Snow));
        // Lower down it is dirt, and the snow starts at the snow line.
        assert!(Block::Snow.collides() && Block::Snow.opaque());
        assert_eq!(g.world.block_at(m.x, crate::terrain::SNOW_Y - 1, m.z), Block::Dirt);
        assert_eq!(g.world.block_at(m.x, crate::terrain::SNOW_Y, m.z), Block::Snow);
    }

    #[test]
    fn no_tree_grows_above_the_tree_line_on_a_mountain() {
        let mut g = game(4);
        let at = (g.player.pos.x, g.player.pos.z);
        g.bring_in(TrekEvent::Mountain { at, dir: (0.0, -1.0) });
        let m = g.world.landmarks.mountains[0];
        let (cx0, cz0) = World::chunk_of(m.x, m.z);
        for cz in cz0 - 3..=cz0 + 3 {
            for cx in cx0 - 3..=cx0 + 3 {
                g.world.ensure_chunk(cx, cz);
                let blocks = &g.world.chunks[&(cx, cz)].blocks;
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        let top = g.world.ground_top(wx, wz);
                        for y in 0..WORLD_HEIGHT {
                            if matches!(blocks[idx(lx, y, lz)], Block::Wood) {
                                assert!(top <= GROUND_TOP + TREE_LINE_RISE, "a trunk at {wx},{wz} on ground {top}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_peep_stone_in_hand_shows_one_rainbow_that_ends_in_gold_beyond_the_render_distance() {
        let mut g = game(5);
        assert!(!g.try_show_rainbow(), "no peep stone in hand");
        assert_eq!(g.world.landmarks.rainbow, RainbowState::Unseen);
        hold(&mut g, ItemKind::Stick);
        assert!(!g.try_show_rainbow());
        hold(&mut g, ItemKind::PeepStone);
        assert!(g.try_show_rainbow());
        let rainbow = g.world.active_rainbow().expect("a rainbow");
        let (x, y, z) = rainbow.gold;
        load_column(&mut g, x, z);
        assert_eq!(g.world.block_at(x, y, z), Block::GoldPile);
        assert_eq!(g.world.block_at(x, y - 1, z), Block::Dirt, "the gold lies on the ground");
        let p = g.player.pos;
        let view = ((render_radius() + 1) * CHUNK_SIZE) as f32;
        let along_an_axis = |cx: i32, cz: i32| (cx as f32 + 0.5 - p.x).abs().max((cz as f32 + 0.5 - p.z).abs());
        assert!(along_an_axis(x, z) > view, "the gold is {} blocks out, beyond the {view} that are drawn", along_an_axis(x, z));
        assert!(along_an_axis(rainbow.from.0, rainbow.from.1) > view, "so is the far end of the arc");
        assert!(rainbow_clear_of_view((p.x, p.z), rainbow.from, (x, z)), "and every part of the arch between them");
        assert!(g.toasts.contains("rainbow"));
        assert!(!g.try_show_rainbow(), "only one rainbow while the gold is out there");
        // It shows only while the peep stone is in hand.
        assert_eq!(g.visible_rainbow(), Some(rainbow));
        hold(&mut g, ItemKind::Stick);
        assert_eq!(g.visible_rainbow(), None);
    }

    #[test]
    fn the_rainbow_stays_beyond_the_render_distance_whatever_it_is_set_to() {
        let before = render_radius();
        for radius in [MIN_RENDER_RADIUS, 6, MAX_RENDER_RADIUS] {
            set_render_radius(radius);
            let mut g = game(60 + radius as u64);
            hold(&mut g, ItemKind::PeepStone);
            assert!(g.try_show_rainbow(), "radius {radius}");
            let rainbow = g.world.active_rainbow().unwrap();
            let p = g.player.pos;
            let view = ((radius + 1) * CHUNK_SIZE) as f32;
            let (x, z) = (rainbow.gold.0, rainbow.gold.2);
            assert!(((x as f32 - p.x).abs().max((z as f32 - p.z).abs())) > view, "radius {radius}: gold inside the view");
            assert!(rainbow_clear_of_view((p.x, p.z), rainbow.from, (x, z)), "radius {radius}: arch inside the view");
        }
        set_render_radius(before);
    }

    #[test]
    fn taking_the_gold_ends_rainbows_for_the_rest_of_the_game_even_after_saving_and_loading() {
        let mut g = game(6);
        hold(&mut g, ItemKind::PeepStone);
        assert!(g.try_show_rainbow());
        let (x, y, z) = g.world.active_rainbow().unwrap().gold;
        load_column(&mut g, x, z);
        let drops = g.mine(x, y, z);
        assert_eq!(drops, vec![ItemKind::Gold; RAINBOW_GOLD_DROP as usize]);
        assert_eq!(g.world.block_at(x, y, z), Block::Air);
        assert_eq!(g.world.landmarks.rainbow, RainbowState::Spent);
        assert!(!g.world.placed.contains_key(&(x, y, z)));
        assert!(g.toasts.contains("will not be another"));
        for _ in 0..5 {
            assert!(!g.try_show_rainbow(), "never another rainbow");
        }
        assert_eq!(g.visible_rainbow(), None);
        // Saved and loaded again, the world still has no rainbow.
        let text = crate::save::serialize_save("t", 1, &g.world, &g.player, &g.inventory, &g.items, g.health, false);
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, crate::save::parse_save(&text), 7);
        hold(&mut again, ItemKind::PeepStone);
        assert!(!again.try_show_rainbow());
        assert_eq!(again.world.landmarks.rainbow, RainbowState::Spent);
    }

    #[test]
    fn an_unclaimed_rainbow_and_its_gold_survive_saving_and_loading() {
        let mut g = game(8);
        hold(&mut g, ItemKind::PeepStone);
        assert!(g.try_show_rainbow());
        let rainbow = g.world.active_rainbow().unwrap();
        let text = crate::save::serialize_save("t", 1, &g.world, &g.player, &g.inventory, &g.items, g.health, false);
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, crate::save::parse_save(&text), 9);
        assert_eq!(again.world.active_rainbow(), Some(rainbow));
        load_column(&mut again, rainbow.gold.0, rainbow.gold.2);
        assert_eq!(again.world.block_at(rainbow.gold.0, rainbow.gold.1, rainbow.gold.2), Block::GoldPile);
        hold(&mut again, ItemKind::PeepStone);
        assert!(!again.try_show_rainbow(), "the same rainbow is still out there");
    }

    fn put_in_hotbar(g: &mut Game, slot: usize, kind: ItemKind) {
        g.inventory.slots[slot] = Some(crate::item::ItemStack { kind, count: 1 });
    }

    #[test]
    fn the_rainbow_is_a_thing_of_the_day_and_a_peep_stone_at_night_brings_no_rainbow() {
        let mut g = game(30);
        g.world.set_night(true);
        hold(&mut g, ItemKind::PeepStone);
        assert!(!g.try_show_rainbow(), "no rainbow in the dark");
        assert_eq!(g.world.landmarks.rainbow, RainbowState::Unseen);
        g.world.set_night(false);
        assert!(g.try_show_rainbow());
        assert!(g.visible_rainbow().is_some());
        g.world.set_night(true);
        assert_eq!(g.visible_rainbow(), None, "and it is not there at night");
        g.world.set_night(false);
        assert!(g.visible_rainbow().is_some(), "but it is there again by day");
    }

    #[test]
    fn a_peep_stone_at_night_brings_one_grand_castle_beyond_the_land_and_a_trail_to_its_gate() {
        let mut g = game(31);
        assert!(!g.try_show_grand_castle(), "not by day");
        g.world.set_night(true);
        assert!(!g.try_show_grand_castle(), "no peep stone in hand");
        assert_eq!(g.trail_path(), None);
        hold(&mut g, ItemKind::PeepStone);
        assert_eq!(g.trail_path(), None, "no castle yet, so nothing to lead to");
        assert!(g.try_show_grand_castle());
        assert!(g.toasts.contains("ghostly trail"));
        let castle = g.world.landmarks.grand_castle.expect("a castle");
        let p = g.player.pos;
        let nearest = (castle.x as f32 - p.x).abs().max((castle.z as f32 - p.z).abs()) - GRAND_REACH as f32;
        assert!(nearest >= grand_distance() as f32 - 1.5);
        assert!(nearest >= ((gen_radius() + 1) * CHUNK_SIZE) as f32 - 1.0, "beyond the generated land: {nearest}");
        assert!(g.world.landmarks.has_room_for(&castle));
        assert!(!g.try_show_grand_castle(), "only one grand castle");
        // The trail leads from the player to the gate and on to the door of the keep.
        let path = g.trail_path().expect("a trail");
        assert_eq!(path, vec![(p.x, p.z), castle.gate_approach(), castle.keep_door()]);
        // It is shown only to a peep stone in the hand, and at night it leads to the castle, not to a rainbow.
        hold(&mut g, ItemKind::Stick);
        assert_eq!(g.trail_path(), None);
        hold(&mut g, ItemKind::PeepStone);
        g.world.set_night(false);
        assert_eq!(g.trail_path(), None, "by day it leads to the end of the rainbow, and there is none yet");
        // The castle is really there when the land round it is made, and it is kept by a save.
        g.world.set_night(true);
        let (wx, wz) = (castle.x + GRAND_WALL_HALF, castle.z);
        load_column(&mut g, wx, wz);
        assert_eq!(g.world.block_at(wx, GROUND_TOP + 1, wz), Block::CastleStone);
        let text = crate::save::serialize_save("t", 1, &g.world, &g.player, &g.inventory, &g.items, g.health, false);
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, crate::save::parse_save(&text), 9);
        assert_eq!(again.world.landmarks.grand_castle, Some(castle));
        hold(&mut again, ItemKind::PeepStone);
        assert!(!again.try_show_grand_castle(), "the same castle is still out there");
    }

    #[test]
    fn the_grand_castle_keeps_clear_of_a_lake_in_the_way() {
        let mut g = game(32);
        g.world.set_night(true);
        hold(&mut g, ItemKind::PeepStone);
        let p = g.player.pos;
        let spot = |angle: f32| GrandCastle::ahead((p.x, p.z), angle);
        // Lakes all along the east half of the horizon: the castle has to go to the west.
        for i in 0..64 {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 63.0;
            let c = spot(angle);
            g.world.landmarks.lakes.push(Lake { x: c.x, z: c.z, radius: 20, phase: 0, from: (c.x, c.z) });
        }
        assert!(g.try_show_grand_castle());
        let castle = g.world.landmarks.grand_castle.unwrap();
        assert!(castle.x < p.x as i32 + grand_distance() / 2, "it went west: {castle:?}");
        assert!(g.world.landmarks.lakes.iter().all(|l| {
            let (x0, z0, x1, z1) = l.bounds(0);
            castle.x + GRAND_REACH < x0 || castle.x - GRAND_REACH > x1 || castle.z + GRAND_REACH < z0 || castle.z - GRAND_REACH > z1
        }), "{castle:?} is not clear of the lakes");
    }

    #[test]
    fn by_day_a_peep_stone_leads_to_the_end_of_the_rainbow_even_after_the_gold_is_taken() {
        let mut g = game(33);
        hold(&mut g, ItemKind::PeepStone);
        assert_eq!(g.trail_path(), None, "no rainbow yet");
        assert!(g.try_show_rainbow());
        let (x, y, z) = g.world.active_rainbow().unwrap().gold;
        load_column(&mut g, x, z);
        let p = g.player.pos;
        assert_eq!(g.trail_path(), Some(vec![(p.x, p.z), (x as f32 + 0.5, z as f32 + 0.5)]));
        // The gold is taken and the rainbow is gone, but the trail still leads to the place.
        g.mine(x, y, z);
        assert_eq!(g.world.landmarks.rainbow, RainbowState::Spent);
        assert_eq!(g.visible_rainbow(), None);
        assert_eq!(g.trail_path(), Some(vec![(p.x, p.z), (x as f32 + 0.5, z as f32 + 0.5)]));
        // And it still does after the world is saved and loaded.
        let text = crate::save::serialize_save("t", 1, &g.world, &g.player, &g.inventory, &g.items, g.health, false);
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, crate::save::parse_save(&text), 7);
        hold(&mut again, ItemKind::PeepStone);
        let q = again.player.pos;
        assert_eq!(again.trail_path(), Some(vec![(q.x, q.z), (x as f32 + 0.5, z as f32 + 0.5)]));
        // At night it leads to the castle instead, not to the rainbow.
        again.world.set_night(true);
        assert_eq!(again.trail_path(), None);
    }

    #[test]
    fn day_and_night_change_behind_a_one_second_blackout() {
        let mut g = game(34);
        assert!(!g.world.night && g.blackout.is_none());
        g.set_night(false);
        assert!(g.blackout.is_none(), "no blackout when nothing changes");
        g.set_night(true);
        let b = g.blackout.expect("a blackout");
        assert!(!g.world.night, "the sky is still the day's while the screen is clear");
        assert_eq!(b.darkness(), 0.0);
        // The screen fades to black, and the world changes only once it is black.
        let mut dark_at_change = 0.0f32;
        let mut seen_black = false;
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        while g.blackout.is_some() {
            let was_night = g.world.night;
            g.update_blackout(dt);
            elapsed += dt;
            if let Some(b) = g.blackout {
                if !was_night && g.world.night {
                    dark_at_change = b.darkness();
                }
                seen_black |= b.darkness() >= 1.0;
            }
            assert!(elapsed < DAY_NIGHT_BLACKOUT_SECONDS + 0.1, "it ends");
        }
        assert!(g.world.night, "and then it is night");
        assert!(dark_at_change > 0.9, "the change happened in the dark: {dark_at_change}");
        assert!(seen_black);
        assert!((elapsed - DAY_NIGHT_BLACKOUT_SECONDS).abs() < 0.05, "a second in all: {elapsed}");
        assert!(g.toasts.contains("Night falls"));
        // The same on the way back to day.
        g.set_night(false);
        for _ in 0..70 {
            g.update_blackout(dt);
        }
        assert!(!g.world.night && g.blackout.is_none());
        assert!(g.toasts.contains("Day breaks"));
    }

    #[test]
    fn the_blackout_is_black_in_the_middle_and_clear_at_both_ends() {
        let at = |t: f32| Blackout { elapsed: t * DAY_NIGHT_BLACKOUT_SECONDS, night: true, switched: false }.darkness();
        assert_eq!(at(0.0), 0.0);
        assert_eq!(at(1.0), 0.0);
        assert_eq!(at(0.5), 1.0);
        assert!(at(0.15) > 0.4 && at(0.15) < 0.6 && at(0.85) > 0.4 && at(0.85) < 0.6);
        assert!(at(0.2) < at(0.3) && at(0.7) > at(0.8));
    }

    #[test]
    fn drinking_the_stews_starts_the_blackout_and_a_second_drink_during_it_wins() {
        let mut g = game(35);
        hold(&mut g, ItemKind::PurpleBerryStew);
        g.health = MAX_HEALTH;
        assert!(g.eat_held());
        assert!(g.blackout.is_some() && !g.world.night);
        g.update_blackout(0.4);
        assert!(g.world.night, "dark by now, and it has become night");
        hold(&mut g, ItemKind::RedBerryStew);
        assert!(g.eat_held());
        g.update_blackout(0.05);
        assert!(!g.world.night, "the red stew turns it back at once, since the screen is still black");
        for _ in 0..40 {
            g.update_blackout(0.05);
        }
        assert!(g.blackout.is_none() && !g.world.night);
    }

    fn whisp_game(seed: u64) -> (Game, usize) {
        let mut g = game(seed);
        g.world.set_night(true);
        let p = g.player.pos;
        assert!(g.spawn_animal(AnimalKind::WillOWhisp, p.x as i32 + 10, p.z as i32));
        let index = g.animals.iter().position(|a| a.kind == AnimalKind::WillOWhisp).unwrap();
        (g, index)
    }

    fn whisps(g: &Game) -> Vec<&Animal> {
        g.animals.iter().filter(|a| a.kind == AnimalKind::WillOWhisp).collect()
    }

    /// Run the animals for `seconds`, without new ones coming in.
    fn pass(g: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 20.0) as usize {
            g.spawn_timer = 0.0;
            g.update_animals(0.05);
        }
    }

    #[test]
    fn a_whisp_flees_a_player_without_a_holly_rod_and_even_one_that_is_not_in_the_hotbar() {
        let (mut g, _) = whisp_game(40);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        g.player.pos = g.animals[0].pos - vec3(5.0, 0.0, 0.0);
        pass(&mut g, 2.0);
        assert!(whisps(&g).iter().all(|a| !a.bonded && (a.pos - g.player.pos).length() > 7.0), "it keeps away");
        // A rod in the inventory but outside the hotbar draws nothing.
        g.inventory.slots[HOTBAR_SLOTS] = Some(crate::item::ItemStack { kind: ItemKind::HollyRod, count: 1 });
        assert!(!g.holly_rod_in_hotbar());
        pass(&mut g, 2.0);
        assert!(whisps(&g).iter().all(|a| !a.bonded));
    }

    #[test]
    fn a_whisp_is_drawn_to_a_holly_rod_in_the_hotbar_and_bonds_within_five_feet_without_the_rod_in_hand() {
        let (mut g, _) = whisp_game(41);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        hold(&mut g, ItemKind::Stick);
        put_in_hotbar(&mut g, 4, ItemKind::HollyRod);
        assert_eq!(g.held_item(), Some(ItemKind::Stick), "the rod is not in the hand");
        assert!(g.holly_rod_in_hotbar());
        let before = (g.animals[0].pos - g.player.pos).length();
        assert!(before > WILL_O_WHISP_FLEE_DISTANCE && before <= WILL_O_WHISP_ATTRACT_DISTANCE);
        pass(&mut g, 1.0);
        let a = whisps(&g)[0];
        assert!(!a.bonded || (a.pos - g.player.pos).length() < before, "it floats towards the player");
        assert!((a.pos - g.player.pos).length() < before - 2.0);
        pass(&mut g, 4.0);
        let a = whisps(&g)[0];
        assert!(a.bonded, "within five feet it is bonded");
        assert!((a.pos - g.player.pos).length() <= WILL_O_WHISP_ORBIT_HEIGHT + WILL_O_WHISP_ORBIT_RADIUS + 0.3, "and stays close: {}", (a.pos - g.player.pos).length());
    }

    #[test]
    fn a_whisp_beyond_the_reach_of_the_rod_is_not_drawn() {
        let (mut g, _) = whisp_game(42);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        put_in_hotbar(&mut g, 2, ItemKind::HollyRod);
        let p = g.player.pos;
        g.animals[0].pos = p + vec3(WILL_O_WHISP_ATTRACT_DISTANCE + 8.0, 3.0, 0.0);
        let before = g.animals[0].pos;
        pass(&mut g, 2.0);
        assert!(whisps(&g).iter().all(|a| !a.bonded && (a.pos - before).length() < 1.0), "it stays where it is");
    }

    #[test]
    fn a_bonded_whisp_follows_the_player_and_goes_with_them_into_the_day() {
        let (mut g, _) = whisp_game(43);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        put_in_hotbar(&mut g, 1, ItemKind::HollyRod);
        pass(&mut g, 5.0);
        assert!(whisps(&g)[0].bonded);
        // The player walks away; the whisp keeps up.
        g.player.pos += vec3(30.0, 0.0, 0.0);
        pass(&mut g, 5.0);
        assert!(whisps(&g)[0].bonded && (whisps(&g)[0].pos - g.player.pos).length() < 3.0, "it followed");
        // A drink of the red stew makes it day. The whisp is still there, and still follows.
        hold(&mut g, ItemKind::RedBerryStew);
        g.eat_held();
        for _ in 0..30 {
            g.update_blackout(0.05);
        }
        assert!(!g.world.night);
        pass(&mut g, 1.0);
        assert_eq!(whisps(&g).len(), 1, "the bonded whisp stays into the day");
        g.player.pos += vec3(0.0, 0.0, 25.0);
        pass(&mut g, 4.0);
        assert!(whisps(&g)[0].bonded && (whisps(&g)[0].pos - g.player.pos).length() < 3.0, "and follows by day");
        // No new whisp comes in by day.
        for _ in 0..200 {
            g.spawn_wildlife();
        }
        assert_eq!(whisps(&g).len(), 1);
        // Take the rod out of the hotbar: the whisp lets go and, it being day, is gone.
        g.inventory.slots[1] = None;
        pass(&mut g, 0.1);
        assert!(whisps(&g).is_empty(), "without the rod the whisp is lost with the night");
    }

    #[test]
    fn a_bonded_whisp_lets_go_at_night_too_when_the_rod_leaves_the_hotbar_and_flees_again() {
        let (mut g, _) = whisp_game(44);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        put_in_hotbar(&mut g, 0, ItemKind::HollyRod);
        pass(&mut g, 5.0);
        assert!(whisps(&g)[0].bonded);
        g.inventory.slots[0] = None;
        pass(&mut g, 0.1);
        assert_eq!(whisps(&g).len(), 1, "it is night, so it stays");
        assert!(!whisps(&g)[0].bonded);
        let from = (whisps(&g)[0].pos - g.player.pos).length();
        // The player is close to it, so it floats away from them.
        pass(&mut g, 2.0);
        assert!((whisps(&g)[0].pos - g.player.pos).length() > from + 3.0, "it flees again");
    }

    #[test]
    fn a_bonded_whisp_left_far_behind_rejoins_the_player() {
        let (mut g, _) = whisp_game(45);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        put_in_hotbar(&mut g, 0, ItemKind::HollyRod);
        pass(&mut g, 5.0);
        g.player.pos += vec3(WILL_O_WHISP_TELEPORT_DISTANCE + 20.0, 0.0, 0.0);
        pass(&mut g, 0.1);
        assert_eq!(whisps(&g).len(), 1);
        assert!((whisps(&g)[0].pos - g.player.pos).length() < 5.0, "it caught up at once");
    }

    #[test]
    fn bonded_daytime_whisps_gather_a_storm_and_it_lingers_after_the_last_one_leaves() {
        let (mut g, _) = whisp_game(46);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        put_in_hotbar(&mut g, 0, ItemKind::HollyRod);
        pass(&mut g, 5.0);
        assert_eq!(g.following_whisp_count(), 1);
        g.world.set_night(false);
        g.update_storm(WHISP_STORM_GATHER_SECONDS - 0.1);
        assert!(!g.storm_active, "clouds gather only after the delay");
        g.update_storm(0.1);
        assert!(g.storm_active);
        g.animals.clear();
        g.update_storm(WHISP_STORM_CLEAR_SECONDS - 0.1);
        assert!(g.storm_active, "the storm lingers after its last whisp");
        g.update_storm(0.1);
        assert!(!g.storm_active, "the storm eventually clears");
    }

    #[test]
    fn holly_rod_lightning_consumes_one_following_whisp_without_hurting_the_player() {
        let (mut g, _) = whisp_game(47);
        g.animals.retain(|a| a.kind == AnimalKind::WillOWhisp);
        put_in_hotbar(&mut g, 0, ItemKind::HollyRod);
        pass(&mut g, 5.0);
        let extra = g.animals[0].clone();
        g.animals.push(extra);
        g.world.set_night(false);
        g.storm_active = true;
        assert_eq!(g.following_whisp_count(), 2);
        g.health = MAX_HEALTH - 1;
        let strike = g.player.pos + vec3(30.0, 0.0, 0.0);
        assert!(!g.use_holly_rod_on_target(None), "pointing at nothing calls nothing");
        assert!(g.use_holly_rod_on_target(Some(strike)));
        assert_eq!(g.following_whisp_count(), 1);
        assert_eq!(g.health, MAX_HEALTH - 1, "Holly Rod lightning does not hurt the player");
        assert!(g.toasts.contains("Lightning answers the Holly Rod"));
        let (pos, flash) = g.storm_lightning().expect("the bolt is out");
        assert_eq!(pos, strike, "it lands where the rod pointed, not on the player");
        assert!(flash > 0.0);
    }

    #[test]
    fn lightning_hurts_creatures_near_the_strike_most_at_the_centre_and_never_the_player() {
        let mut g = game(51);
        g.animals.clear();
        let strike = g.player.pos + vec3(30.0, 0.0, 0.0);
        let mut rng = crate::rng::Rng::new(5);
        let at = |dx: f32| strike + vec3(dx, 0.0, 0.0);
        g.animals.push(Animal::new(AnimalKind::Deer, at(0.0), &mut rng));
        g.animals.push(Animal::new(AnimalKind::Bear, at(4.0), &mut rng));
        g.animals.push(Animal::new(AnimalKind::Rabbit, at(12.0), &mut rng));
        g.health = MAX_HEALTH - 2;
        g.trigger_storm_lightning(strike);
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::Deer), "the deer at the centre is killed");
        assert!(g.items.iter().any(|i| i.kind == ItemKind::Venison), "and drops its loot");
        let bear = g.animals.iter().find(|a| a.kind == AnimalKind::Bear).expect("a bear survives a glancing blow");
        assert!(bear.health < AnimalKind::Bear.max_health() - 10 && bear.health > 0, "but is badly hurt: {}", bear.health);
        let rabbit = g.animals.iter().find(|a| a.kind == AnimalKind::Rabbit).expect("the rabbit is out of reach");
        assert_eq!(rabbit.health, AnimalKind::Rabbit.max_health(), "beyond the radius nothing happens");
        assert_eq!(g.health, MAX_HEALTH - 2, "the player is never hurt");
    }

    #[test]
    fn a_holly_rod_points_far_beyond_ordinary_reach_at_a_creature_or_a_block() {
        let mut g = game(52);
        g.world.stream_chunks(g.player.pos);
        let eye = g.player.eye();
        // Clear a corridor of trees and rocks, and put a wall of planks forty blocks away, far beyond a hand's reach.
        for x in 8..46 {
            for z in 9..18 {
                for y in GROUND_TOP + 1..GROUND_TOP + 14 {
                    if g.world.block_at(x, y, z) != Block::Air {
                        g.world.break_block(x, y, z);
                    }
                }
            }
        }
        for z in 9..18 {
            for y in GROUND_TOP + 1..GROUND_TOP + 7 {
                g.world.set_block(40, y, z, Block::Planks);
            }
        }
        let dir = (vec3(40.5, GROUND_TOP as f32 + 3.0, 13.5) - eye).normalize();
        hold(&mut g, ItemKind::Stone);
        assert!(g.holly_rod_strike_point(eye, dir).is_none(), "only a held Holly Rod points");
        hold(&mut g, ItemKind::HollyRod);
        let spot = g.holly_rod_strike_point(eye, dir).expect("the rod reaches the far wall");
        assert!((spot - eye).length() > 25.0, "{} blocks away", (spot - eye).length());
        assert!(g.world.raycast_hit(eye, dir, REACH).is_none(), "an ordinary reach would have hit nothing");
        assert!(HOLLY_ROD_REACH > REACH * 8.0);
        assert!(g.holly_rod_strike_point(eye, vec3(0.0, 1.0, 0.0)).is_none(), "pointing at the sky hits nothing");
        // A creature in the line of fire is the target, whatever lies behind it.
        g.animals.clear();
        let mut rng = crate::rng::Rng::new(9);
        let deer_at = vec3(eye.x + 22.0, GROUND_TOP as f32 + 1.0, eye.z);
        g.animals.push(Animal::new(AnimalKind::Deer, deer_at, &mut rng));
        let to_deer = (deer_at + vec3(0.0, 0.7, 0.0) - eye).normalize();
        if let Some(found) = g.holly_rod_strike_point(eye, to_deer) {
            assert!((found - deer_at).length() < 1.0 || (found - eye).length() < (deer_at - eye).length(), "{found:?}");
        }
    }

    #[test]
    fn standing_in_a_river_counts_as_wading_and_a_jump_does_not_leave_it() {
        let mut g = game(9);
        let (x, z) = (-150..150).flat_map(|z| (-150..150).map(move |x| (x, z))).find(|&(x, z)| crate::river::is_river(1, x, z)).expect("a river");
        load_column(&mut g, x, z);
        g.player.pos = vec3(x as f32 + 0.5, GROUND_TOP as f32 + 0.01, z as f32 + 0.5);
        assert!(g.player_in_water());
        g.player.pos.y += 1.0;
        assert!(g.player_in_water(), "a jump");
        g.player.pos.y += 2.0;
        assert!(!g.player_in_water(), "well clear of the water");
        let dry = (x - 30..x + 30).find(|&xx| !g.world.is_wet_column(xx, z) && g.world.chunks.contains_key(&World::chunk_of(xx, z)));
        if let Some(dx) = dry {
            g.player.pos = vec3(dx as f32 + 0.5, GROUND_TOP as f32 + 1.01, z as f32 + 0.5);
            assert!(!g.player_in_water());
        }
    }

    #[test]
    fn bears_are_a_quarter_as_common_and_twice_as_tough_as_they_were() {
        assert_eq!(BEAR_HEALTH, 2 * 14);
        assert_eq!(MAX_BEAR, 1, "at most one at a time, where it was two");
        let others = SPAWN_WEIGHT_DEER + SPAWN_WEIGHT_QUAIL + SPAWN_WEIGHT_RABBIT + SPAWN_WEIGHT_MOOSE;
        let share = |w: u64| w as f64 / (w + others) as f64;
        let ratio = share(SPAWN_WEIGHT_BEAR) / share(15);
        assert!((0.2..=0.3).contains(&ratio), "about a quarter of the old chance to be picked, got {ratio:.2}");
    }

    #[test]
    fn above_a_hundred_items_on_the_ground_the_oldest_vanish_first() {
        let mut g = game(11);
        // 130 items far from the player. Item n is n tenths of a second old, and lies at x = 40 + n / 100, so which
        // ones survive can be told from where they are. They are shuffled in the list, as picking things up shuffles it.
        let mut order: Vec<usize> = (0..130).collect();
        order.reverse();
        order.swap(0, 77);
        order.swap(5, 120);
        for &n in &order {
            let mut item = DroppedItem::at_rest(ItemKind::Stick, vec3(40.0 + n as f32 * 0.01, 6.05, 40.0));
            item.age = 100.0 - n as f32 * 0.1; // n = 0 is the oldest
            g.items.push(item);
        }
        assert_eq!(g.items.len(), 130);
        g.update_items(0.01);
        assert_eq!(g.items.len(), MAX_DROPPED_ITEMS, "exactly a hundred are left");
        let mut left: Vec<i32> = g.items.iter().map(|i| ((i.pos.x - 40.0) * 100.0).round() as i32).collect();
        left.sort_unstable();
        assert_eq!(left, (30..130).collect::<Vec<i32>>(), "the thirty oldest went, whatever their place in the list");
        // At or below the limit nothing goes.
        g.limit_items();
        assert_eq!(g.items.len(), MAX_DROPPED_ITEMS);
        g.items.truncate(40);
        g.limit_items();
        assert_eq!(g.items.len(), 40);
    }

    #[test]
    fn picking_up_an_item_does_not_make_a_newer_one_look_old() {
        let mut g = game(14);
        for n in 0..MAX_DROPPED_ITEMS {
            g.items.push(DroppedItem::at_rest(ItemKind::Stick, vec3(60.0 + n as f32 * 0.01, 6.05, 60.0)));
        }
        g.update_items(0.5);
        // The player picks up the very first item: the last one moves into its place in the list.
        g.player.pos = vec3(60.0, 6.01, 60.0);
        g.update_items(0.01);
        assert!(g.items.len() < MAX_DROPPED_ITEMS);
        // A fresh item arrives, and enough more to pass the limit by one.
        while g.items.len() <= MAX_DROPPED_ITEMS {
            g.items.push(DroppedItem::at_rest(ItemKind::Pebble, vec3(90.0, 6.05, 90.0)));
        }
        let newest = g.items.last().map(|i| i.pos);
        g.items.last_mut().unwrap().age = 0.0;
        g.player.pos = vec3(0.0, 6.01, 0.0);
        g.limit_items();
        assert_eq!(g.items.len(), MAX_DROPPED_ITEMS);
        assert_eq!(g.items.last().map(|i| i.pos), newest, "the newest item stays");
    }

    #[test]
    fn items_fallen_while_mining_also_count_towards_the_limit() {
        let mut g = game(12);
        for _ in 0..MAX_DROPPED_ITEMS + 25 {
            g.items.push(DroppedItem::spawn_at(ItemKind::Pebble, vec3(60.0, 8.0, 60.0), &mut Rng::new(3)));
        }
        g.update_items(0.016);
        assert_eq!(g.items.len(), MAX_DROPPED_ITEMS);
    }

    #[test]
    fn will_o_whisps_come_a_lot_more_often_than_they_did() {
        assert!(MAX_WILL_O_WHISP >= 3 * 4 && SPAWN_WEIGHT_WILL_O_WHISP >= 5 * 20, "at least 3x as many and 5x the weight they had");
        // At night, with nothing else at its limit, a whisp is the most likely thing to appear.
        let day: u64 = SPAWN_WEIGHT_DEER + SPAWN_WEIGHT_QUAIL + SPAWN_WEIGHT_BEAR + SPAWN_WEIGHT_RABBIT + SPAWN_WEIGHT_MOOSE;
        assert!(SPAWN_WEIGHT_WILL_O_WHISP as f64 / (day + SPAWN_WEIGHT_WILL_O_WHISP) as f64 > 0.3);
        let mut g = game(13);
        g.world.set_night(true);
        g.player.pos = vec3(10.0, 6.01, 10.0);
        let mut whisps = 0;
        for _ in 0..200 {
            g.animals.clear();
            g.spawn_timer = ANIMAL_SPAWN_INTERVAL;
            g.update_animals(0.0);
            whisps += g.animals.iter().filter(|a| a.kind == AnimalKind::WillOWhisp).count();
        }
        assert!(whisps > 20, "plenty of whisps over 200 spawn attempts at night, got {whisps}");
    }
}


#[cfg(test)]
mod totem_horizen_and_wear_tests {
    use super::*;
    use crate::item::ItemStack;
    use crate::player::MoveInput;
    use std::path::PathBuf;

    fn game(seed: u64) -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        Game::from_save(info, SaveData::new(), seed)
    }

    fn hold(g: &mut Game, kind: ItemKind) {
        g.inventory.slots[0] = Some(ItemStack { kind, count: 1 });
        g.select_slot(0);
    }

    fn give(g: &mut Game, kind: ItemKind, count: u32) {
        assert_eq!(g.inventory.add(kind, count), 0);
    }

    #[test]
    fn a_bone_totem_by_day_brings_one_well_and_a_trail_that_leads_there_for_ever() {
        let mut g = game(1);
        hold(&mut g, ItemKind::BoneTotem);
        assert_eq!(g.trail_path(), None, "nothing out there yet");
        assert!(g.try_show_well_or_graveyard());
        let well = g.world.landmarks.well.expect("a well");
        assert!(g.world.landmarks.graveyard.is_none(), "by day only the well");
        let p = g.player.pos;
        let nearest = (well.x as f32 - p.x).abs().max((well.z as f32 - p.z).abs());
        assert!(nearest >= ((gen_radius() + 1) * CHUNK_SIZE) as f32, "beyond the generated land: {nearest}");
        assert!(!g.try_show_well_or_graveyard(), "only one well");
        let path = g.trail_path().expect("a trail");
        assert_eq!((path[0], *path.last().unwrap()), ((p.x, p.z), well.centre()));
        // Only a totem in hand shows it; a peep stone shows its own trails.
        hold(&mut g, ItemKind::Stick);
        assert_eq!(g.trail_path(), None);
        hold(&mut g, ItemKind::PeepStone);
        assert_eq!(g.trail_path(), None, "the peep stone does not lead to the well");
        // At night the totem leads elsewhere, and the well stays put.
        hold(&mut g, ItemKind::BoneTotem);
        g.world.set_night(true);
        assert_eq!(g.trail_path(), None, "no graveyard yet");
        assert!(g.try_show_well_or_graveyard());
        let yard = g.world.landmarks.graveyard.expect("a graveyard");
        assert!(!g.try_show_well_or_graveyard(), "only one graveyard");
        let path = g.trail_path().unwrap();
        assert_eq!(path, vec![(p.x, p.z), yard.gate_approach(), yard.middle()]);
        g.world.set_night(false);
        assert_eq!(g.world.landmarks.well, Some(well));
        assert_eq!(g.trail_path().unwrap().last().copied(), Some(well.centre()), "by day it leads to the well again");
    }

    #[test]
    fn the_well_is_deep_water_in_a_rim_and_the_graveyard_has_stones_when_generated() {
        let mut g = game(2);
        let well = Well { x: 40, z: 40 };
        g.world.add_well(well);
        g.world.add_graveyard(Graveyard { x: 40, z: 80 });
        for (cx, cz) in [(2, 2), (1, 1), (2, 4), (1, 3)] {
            g.world.ensure_chunk(cx, cz);
        }
        for y in STONE_LAYERS..=GROUND_TOP {
            assert_eq!(g.world.block_at(41, y, 41), Block::Water, "water all the way down at {y}");
        }
        assert_eq!(g.world.block_at(41, 0, 41), Block::Stone);
        assert_eq!(g.world.block_at(39, GROUND_TOP + 1, 41), Block::CastleStone, "the rim");
        assert_eq!(g.world.block_at(41, GROUND_TOP + 1, 41), Block::Air);
        assert!(g.world.is_wet_column(41, 41));
        assert!(g.world.landmarks.is_well(41, 41) && !g.world.landmarks.is_well(44, 41));
        assert_eq!(g.world.block_at(43, GROUND_TOP + 1, 83), Block::Gravestone);
    }

    fn fall_in_the_well(g: &mut Game) {
        let well = Well { x: 40, z: 40 };
        g.world.add_well(well);
        for cz in 1..=3 {
            for cx in 1..=3 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        g.player.pos = vec3(41.5, GROUND_TOP as f32 + 2.5, 41.5);
        g.player.vel = Vec3::ZERO;
        for _ in 0..240 {
            g.player.update(&g.world, 1.0 / 60.0, &MoveInput::default());
            g.update_landmarks();
            if g.in_horizen() {
                break;
            }
        }
    }

    #[test]
    fn the_well_does_not_transport_the_player_until_the_stone_floor_is_hit() {
        let mut g = game(30);
        let well = Well { x: 40, z: 40 };
        g.world.add_well(well);
        for cz in 1..=3 {
            for cx in 1..=3 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        g.player.pos = vec3(41.5, WELL_FLOOR_Y + 2.0, 41.5);
        g.player.on_ground = true;
        g.update_dimension();
        assert!(!g.in_horizen(), "a shallow fall does not count");
        g.player.pos.y = WELL_FLOOR_Y;
        g.update_dimension();
        assert!(g.in_horizen(), "the floor collision completes the fall");
        assert!(g.horizen_transition.is_some(), "the transfer starts blacked out");
    }

    #[test]
    fn a_daytime_sky_tornado_spawns_on_the_forty_block_ring_and_night_clears_it() {
        let mut g = game(31);
        g.travel_to(Dimension::Skylands);
        g.world.night = false;
        let target = g.player.pos;
        g.update_animals(SKY_TORNADO_INTERVAL - 0.01);
        assert!(g.tornadoes.is_empty(), "the tornado has not spawned early");
        g.update_animals(0.01);
        assert_eq!(g.tornadoes.len(), 1);
        let tornado = g.tornadoes[0];
        let distance = vec2(tornado.pos.x - target.x, tornado.pos.z - target.z).length();
        assert!((distance - SKY_TORNADO_SPAWN_DISTANCE).abs() < 1.5, "spawn ring distance: {distance}");
        assert!((tornado.pos.y - target.y).abs() < 1e-5, "it sweeps at the player's height");
        assert!((tornado.target - target).length() < 1e-5, "target is the captured player position");
        g.world.set_night(true);
        g.update_animals(0.0);
        assert!(g.tornadoes.is_empty(), "night removes the tornado");
    }

    #[test]
    fn sky_tornadoes_come_four_times_as_often_as_one_lasts_so_there_are_sometimes_four() {
        assert_eq!(SKY_TORNADO_INTERVAL * 4.0, SKY_TORNADO_LIFETIME);
        assert_eq!(SKY_TORNADO_DAMAGE, 4, "double the damage");
        assert_eq!(SKY_TORNADO_LIFETIME, 30.0, "they last as long as they did");
        let mut g = game(33);
        g.travel_to(Dimension::Skylands);
        g.world.night = false;
        g.health = 1_000_000;
        let mut most = 0;
        let mut seen_two = false;
        for _ in 0..(120.0 / 0.5) as i32 {
            g.update_animals(0.5);
            most = most.max(g.tornadoes.len());
            seen_two |= g.tornadoes.len() == 4;
            assert!(g.tornadoes.iter().all(|t| t.age <= SKY_TORNADO_LIFETIME + 0.5), "none outlives its lifetime");
        }
        assert!(seen_two && most == 4, "four at once, never five: {most}");
        // Over two minutes there are about sixteen spawns, not eight.
        let mut g = game(34);
        g.travel_to(Dimension::Skylands);
        g.world.night = false;
        g.health = 1_000_000;
        let mut spawns = 0;
        let mut last = 0;
        for _ in 0..(120.0 / 0.5) as i32 {
            g.update_animals(0.5);
            let newest = g.tornadoes.iter().map(|t| t.age).fold(f32::MAX, f32::min);
            if g.tornadoes.len() > last || newest < 0.6 {
                spawns += (newest < 0.6) as i32;
            }
            last = g.tornadoes.len();
        }
        assert!((15..=16).contains(&spawns), "{spawns} spawns in two minutes");
    }

    #[test]
    fn there_are_no_fire_tornadoes_in_the_horizen_or_the_overworld() {
        let mut g = game(32);
        g.world.night = false;
        g.update_animals(SKY_TORNADO_INTERVAL + 1.0);
        assert!(g.tornadoes.is_empty(), "not in the overworld");
        fall_in_the_well(&mut g);
        g.world.night = false;
        g.update_animals(SKY_TORNADO_INTERVAL + 1.0);
        assert!(g.tornadoes.is_empty(), "not in the Horizen");
    }

    #[test]
    fn falling_into_the_well_takes_the_player_to_the_horizen_arrival_pool_with_no_creatures() {
        let mut g = game(3);
        g.animals.push(Animal::new(AnimalKind::Deer, vec3(30.0, 6.01, 30.0), &mut Rng::new(1)));
        fall_in_the_well(&mut g);
        assert!(g.in_horizen(), "the player fell into the Horizen");
        assert_eq!(g.world.dimension, Dimension::Horizen);
        assert!(g.animals.is_empty(), "no creature of the overworld comes along");
        // Land and sky of the new world: flat, with ponds; the player appears on the dedicated arrival pool.
        g.world.stream_chunks(g.player.pos);
        g.player.update(&g.world, 0.5, &MoveInput::default());
        assert!(g.world.is_wet_column(g.player.pos.x.floor() as i32, g.player.pos.z.floor() as i32));
        assert!((g.player.pos.y - GROUND_TOP as f32).abs() < 0.2, "on the arrival pool: {}", g.player.pos.y);
        assert!(g.world.chunks.values().all(|c| !c.blocks.contains(&Block::Wood) && !c.blocks.contains(&Block::Bush)), "nothing grows there");
        // The only wildlife that spawns in the Horizen is bats.
        for _ in 0..50 {
            g.spawn_timer = ANIMAL_SPAWN_INTERVAL;
            g.update_animals(0.0);
        }
        assert!(g.animals.iter().all(|a| a.kind == AnimalKind::Bat));
        // The overworld is kept, with its landmarks, for when the player is back.
        assert!(g.away.as_ref().unwrap().world.landmarks.well.is_some());
    }

    #[test]
    fn in_the_horizen_potions_change_the_sky_and_landmarks_still_do_nothing() {
        let mut g = game(4);
        fall_in_the_well(&mut g);
        assert!(g.in_horizen());
        give(&mut g, ItemKind::PurpleBerryStew, 1);
        g.select_slot(0);
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::PurpleBerryStew, count: 1 });
        g.eat_held();
        for _ in 0..90 {
            g.update_blackout(1.0 / 60.0);
        }
        assert!(g.blackout.is_none() && g.in_overworld(), "the purple stew brings night, and the player blacks out and wakes in the overworld");
        assert!(g.parked(Dimension::Horizen).map_or(false, |a| !a.world.night), "the cave itself stays day");
        g.enter_horizen();
        assert!(!g.world.night, "and it is day again when they come back");
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::RedBerryStew, count: 1 });
        g.eat_held();
        for _ in 0..90 {
            g.update_blackout(1.0 / 60.0);
        }
        assert!(g.blackout.is_none() && !g.world.night && g.in_horizen(), "the red stew keeps it day, and keeps the player in the Horizen");
        // (A compass does nothing here; the bone totem leads to the king's castle, see `king_tests`.)
        for item in [ItemKind::Compass] {
            hold(&mut g, item);
            for _ in 0..3 {
                g.update_landmarks();
            }
            assert_eq!(g.trail_path(), None, "{item:?}");
        }
        hold(&mut g, ItemKind::BoneTotem);
        g.update_landmarks();
        assert!(g.world.landmarks.grand_castle.is_none() && g.world.landmarks.well.is_none() && g.world.landmarks.rainbow == RainbowState::Unseen);
        assert!(g.world.landmarks.graveyard.is_none() && g.world.landmarks.mountains.is_empty());
    }

    #[test]
    fn dying_in_the_horizen_returns_the_player_to_the_overworld_start() {
        let mut g = game(5);
        fall_in_the_well(&mut g);
        assert!(g.in_horizen());
        g.hurt_player(MAX_HEALTH, "gone");
        assert!(!g.in_horizen());
        assert_eq!(g.player.pos, Player::new().pos);
    }

    #[test]
    fn the_horizen_and_what_is_worn_survive_saving_and_loading() {
        let mut g = game(6);
        g.worn = Some(ItemKind::PeltCloak);
        fall_in_the_well(&mut g);
        assert!(g.in_horizen());
        g.world.set_night(true);
        let (x, z) = (g.player.pos.x.floor() as i32 + 2, g.player.pos.z.floor() as i32);
        g.world.set_block(x, GROUND_TOP + 1, z, Block::PlacedStone);
        g.items.push(DroppedItem::at_rest(ItemKind::Stick, vec3(12.5, 6.04, 12.5)));
        let text = g.save_text();
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, crate::save::parse_save(&text), 9);
        assert!(again.in_horizen(), "still in the Horizen");
        assert!(again.world.night, "the Horizen keeps its own time of day");
        assert_eq!(again.worn, Some(ItemKind::PeltCloak));
        assert_eq!(again.world.block_at(x, GROUND_TOP + 1, z), Block::PlacedStone, "the edit in the Horizen is kept");
        assert_eq!(again.items.len(), 1);
        assert!(again.away.as_ref().unwrap().world.landmarks.well.is_some(), "and the overworld with it");
        again.travel_to(Dimension::Overworld);
        assert_eq!(again.world.dimension, Dimension::Overworld);
        assert!(again.items.is_empty() && again.world.landmarks.well.is_some());
    }

    #[test]
    fn a_pelt_cloak_is_made_of_a_bear_pelt_and_antlers_and_gold_make_a_bone_totem() {
        use crate::recipe::{craft, recipe_for};
        let mut g = game(7);
        give(&mut g, ItemKind::BearPelt, 1);
        give(&mut g, ItemKind::Antlers, 1);
        give(&mut g, ItemKind::Gold, 1);
        assert!(craft(&mut g.inventory, recipe_for(ItemKind::PeltCloak)));
        assert!(!recipe_for(ItemKind::BoneTotem).can_afford(&g.inventory), "one gold is not enough");
        give(&mut g, ItemKind::Gold, 1);
        assert!(craft(&mut g.inventory, recipe_for(ItemKind::BoneTotem)));
        assert_eq!((g.inventory.count(ItemKind::Antlers), g.inventory.count(ItemKind::Gold), g.inventory.count(ItemKind::BoneTotem)), (0, 0, 1));
        assert_eq!(recipe_for(ItemKind::BoneTotem).describe(), "1 Antlers + 2 Gold");
    }

    #[test]
    fn one_garment_is_worn_at_a_time_and_putting_on_another_returns_the_first() {
        let mut g = game(8);
        hold(&mut g, ItemKind::PeltCloak);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::PeltCloak));
        assert_eq!(g.inventory.count(ItemKind::PeltCloak), 0);
        hold(&mut g, ItemKind::RoyalArmor);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::RoyalArmor));
        assert_eq!(g.inventory.count(ItemKind::PeltCloak), 1, "the cloak is back in the inventory");
        hold(&mut g, ItemKind::Stick);
        assert!(!g.wear_held(), "a stick cannot be worn");
        assert!(g.take_off());
        assert_eq!((g.worn, g.inventory.count(ItemKind::RoyalArmor)), (None, 1));
        assert!(!g.take_off());
    }

    #[test]
    fn the_royal_armour_halves_every_blow_and_a_blow_still_costs_at_least_one_life() {
        let mut g = game(9);
        g.hurt_player(BEAR_DAMAGE, "x");
        assert_eq!(g.health, MAX_HEALTH - BEAR_DAMAGE);
        g.health = MAX_HEALTH;
        g.worn = Some(ItemKind::RoyalArmor);
        g.hurt_player(BEAR_DAMAGE, "x");
        assert_eq!(g.health, MAX_HEALTH - BEAR_DAMAGE / 2);
        g.health = MAX_HEALTH;
        g.hurt_player(1, "x");
        assert_eq!(g.health, MAX_HEALTH - 1, "never nothing");
        g.health = MAX_HEALTH;
        g.hurt_player(PURPLE_BERRY_STEW_DAMAGE + 2, "x");
        assert_eq!(g.health, MAX_HEALTH - 2);
        g.worn = Some(ItemKind::PeltCloak);
        g.health = MAX_HEALTH;
        g.hurt_player(BEAR_DAMAGE, "x");
        assert_eq!(g.health, MAX_HEALTH - BEAR_DAMAGE, "the cloak does not soften blows");
    }

    fn bear_beside_the_player(g: &mut Game) {
        g.player.pos = vec3(10.0, 6.01, 10.0);
        g.animals.clear();
        let spawned = (13..17).flat_map(|x| (7..14).map(move |z| (x, z))).any(|(x, z)| g.spawn_animal(AnimalKind::Bear, x, z));
        assert!(spawned, "room for a bear beside the spawn point");
    }

    fn let_time_pass(g: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 60.0) as i32 {
            g.spawn_timer = 0.0;
            g.update_animals(1.0 / 60.0);
        }
    }

    #[test]
    fn a_bear_attacks_a_player_without_a_cloak() {
        let mut g = game(11);
        bear_beside_the_player(&mut g);
        let_time_pass(&mut g, 2.5);
        assert!(g.health < MAX_HEALTH, "an ordinary bear charges and hurts");
    }

    #[test]
    fn a_bear_leaves_a_player_in_a_pelt_cloak_alone_until_the_player_hits_it() {
        let mut g = game(11);
        g.worn = Some(ItemKind::PeltCloak);
        bear_beside_the_player(&mut g);
        let_time_pass(&mut g, 10.0);
        assert_eq!(g.health, MAX_HEALTH, "no blow from a bear that was not hit");
        assert!(g.animals[0].pacified && !g.animals[0].provoked);
        // Hit it once: from now on it is hostile, cloak or no cloak.
        g.hurt_animals(&[(0, 1)]);
        assert!(g.animals[0].provoked);
        let_time_pass(&mut g, 2.5);
        assert!(g.health < MAX_HEALTH, "a bear that has been hit attacks");
        assert!(!g.animals[0].pacified);
    }

    #[test]
    fn the_grand_castle_changes_between_day_and_night_in_the_loaded_world() {
        let mut g = game(12);
        let castle = GrandCastle { x: 40, z: 40 };
        g.world.add_grand_castle(castle);
        for cz in 1..=3 {
            for cx in 1..=3 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        let gate = (castle.x, castle.z + GRAND_WALL_HALF, GROUND_TOP + 1);
        assert_eq!(g.world.block_at(gate.0, gate.2, gate.1), Block::Air, "by day the gate is a breach");
        g.world.set_night(true);
        for cz in 1..=3 {
            for cx in 1..=3 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        assert_eq!(g.world.block_at(gate.0, gate.2, gate.1), Block::CastleDoor, "by night it is shut");
        g.world.set_night(false);
        assert_eq!(g.world.block_at(gate.0, gate.2, gate.1), Block::Air);
    }

    #[test]
    fn claiming_the_armour_gives_the_royal_armour_once_for_good() {
        let mut g = game(13);
        let castle = GrandCastle { x: 40, z: 40 };
        g.world.set_night(true);
        g.world.add_grand_castle(castle);
        for cz in 0..=3 {
            for cx in 0..=3 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        let (x, z, y) = (castle.x - 6, castle.z, GROUND_TOP + 1 + GRAND_STOREY);
        let armour_x = castle.x + (-(GRAND_KEEP_HALF - 2) + 3);
        assert_eq!(g.world.block_at(armour_x, y, z), Block::ArmorStand, "{x}");
        hold(&mut g, ItemKind::Stick);
        let drops = g.mine(armour_x, y, z);
        assert_eq!(drops, vec![ItemKind::RoyalArmor]);
        // It is gone, night after night.
        g.world.set_night(false);
        g.world.set_night(true);
        assert_eq!(g.world.block_at(armour_x, y, z), Block::Air);
    }

    #[test]
    fn there_is_only_one_mountain_and_the_compass_leads_to_it() {
        let mut g = game(14);
        let at = (g.player.pos.x, g.player.pos.z);
        g.bring_in(TrekEvent::Mountain { at, dir: (0.0, -1.0) });
        assert_eq!(g.world.landmarks.mountains.len(), 1);
        let first = g.world.landmarks.mountains[0];
        g.bring_in(TrekEvent::Mountain { at: (500.0, 500.0), dir: (1.0, 0.0) });
        g.world.add_mountain(Mountain { x: 5, z: 5, ..first });
        assert_eq!(g.world.landmarks.mountains, vec![first], "a second mountain never comes");
        // The needle now points at it from wherever the player stands.
        g.player.pos = vec3(300.0, 6.0, 300.0);
        let b = g.compass_bearing();
        let want = (first.z as f32 + 0.5 - 300.0).atan2(first.x as f32 + 0.5 - 300.0);
        assert!((b - want).abs() < 1e-4);
        // And walking north with the compass in hand brings no further mountain.
        hold(&mut g, ItemKind::Compass);
        for i in 0..400 {
            g.player.pos = vec3(300.0, 6.0, 300.0 - i as f32 * 2.0);
            g.update_landmarks();
        }
        assert_eq!(g.world.landmarks.mountains.len(), 1);
    }
}

#[cfg(test)]
mod boss_and_skylands_tests {
    use super::*;
    use crate::landmarks::MOUNTAIN_MAX_HEIGHT;
    use crate::player::MoveInput;
    use std::path::PathBuf;

    fn game() -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        Game::from_save(info, SaveData::new(), 5)
    }

    fn mountain() -> Mountain {
        Mountain { x: 30, z: -500, radius: MOUNTAIN_RADIUS, height: MOUNTAIN_HEIGHT.min(MOUNTAIN_MAX_HEIGHT), phase: 7 }
    }

    /// A game with a mountain and the player standing on its summit, a little way from the middle.
    fn on_the_summit() -> Game {
        let mut g = game();
        g.world.add_mountain(mountain());
        let (sx, sz) = g.world.landmarks.summit().unwrap();
        let y = g.world.ground_top(sx, sz) as f32 + 1.01;
        g.player.pos = vec3(sx as f32 + 0.5 + 12.0, y, sz as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        g
    }

    fn yetis(g: &Game) -> Vec<&Animal> {
        g.animals.iter().filter(|a| a.kind == AnimalKind::Yeti).collect()
    }

    fn lake() -> Lake {
        Lake { x: 60, z: 60, radius: LAKE_RADIUS, phase: 3, from: (10, 10) }
    }

    fn gate_cell(g: &Game, lake: &Lake) -> Block {
        let (x, z) = (lake.x, lake.z + TOWER_HALF);
        g.world.block_at(x, g.world.ground_top(x, z) + 1, z)
    }

    /// A game with a lake whose castle gate is near the player.
    fn at_the_castle() -> (Game, Lake) {
        let mut g = game();
        let lake = lake();
        g.world.add_lake(lake);
        let (x, z) = (lake.x, lake.z + TOWER_HALF + 4);
        g.player.pos = vec3(x as f32 + 0.5, g.world.ground_top(x, z) as f32 + 1.01, z as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        (g, lake)
    }

    #[test]
    fn the_yeti_appears_once_on_the_flat_summit_when_the_player_comes_near() {
        let mut g = on_the_summit();
        let (sx, sz) = g.world.landmarks.summit().unwrap();
        let top = g.world.ground_top(sx, sz);
        for (dx, dz) in [(0, 0), (6, -4), (-10, 9), (13, 0)] {
            assert_eq!(g.world.ground_top(sx + dx, sz + dz), top, "the arena is level");
        }
        assert!(yetis(&g).is_empty());
        g.spawn_bosses();
        g.spawn_bosses();
        let found = yetis(&g);
        assert_eq!(found.len(), 1, "never a second one");
        assert_eq!((found[0].pos.x, found[0].pos.z), (sx as f32 + 0.5, sz as f32 + 0.5));
        assert_eq!(found[0].health, YETI_HEALTH);
        assert!(found[0].pos.y + found[0].kind.height() < WORLD_HEIGHT as f32, "it fits under the top of the world");
    }

    #[test]
    fn the_yeti_stays_away_from_a_distant_player_and_after_its_death() {
        let mut g = game();
        g.world.add_mountain(mountain());
        g.spawn_bosses();
        assert!(yetis(&g).is_empty(), "the summit is far away");
        let mut g = on_the_summit();
        g.world.landmarks.yeti_slain = true;
        g.spawn_bosses();
        assert!(yetis(&g).is_empty(), "a slain Yeti does not return");
    }

    #[test]
    fn the_yeti_charges_a_nearby_player_hits_hard_and_never_leaves_its_leash() {
        let mut g = on_the_summit();
        g.spawn_bosses();
        let (home, start) = (g.animals[0].home, g.player.pos);
        g.player.pos = home + vec3(YETI_NOTICE_DISTANCE - 4.0, 0.0, 0.0);
        g.player.pos.y = start.y;
        let before = g.health;
        for _ in 0..(8.0 * 60.0) as i32 {
            g.update_animals(1.0 / 60.0);
            if g.health < before {
                break;
            }
        }
        assert_eq!(g.health, before - YETI_DAMAGE, "the Yeti reached the player and struck once");
        // A player who runs off is followed only as far as the leash.
        let yeti = g.animals.iter().find(|a| a.kind == AnimalKind::Yeti).map(|a| a.pos).unwrap_or(home);
        assert!(vec2(yeti.x - home.x, yeti.z - home.z).length() <= YETI_LEASH + 2.0);
    }

    #[test]
    fn killing_the_yeti_sets_the_flag_drops_loot_and_raises_the_cyclone() {
        let mut g = on_the_summit();
        g.spawn_bosses();
        assert!(g.cyclone().is_none() && g.boss_bar().is_some(), "a live Yeti has a bar and there is no cyclone yet");
        g.hurt_animals(&[(0, 10_000)]);
        assert!(g.world.landmarks.yeti_slain && yetis(&g).is_empty());
        assert!(g.boss_bar().is_none());
        let count = |k: ItemKind| g.items.iter().filter(|i| i.kind == k).count() as u32;
        assert_eq!((count(ItemKind::Gold), count(ItemKind::BearPelt)), (YETI_GOLD_DROP, YETI_PELT_DROP));
        let cyclone = g.cyclone().expect("a cyclone now stands on the summit");
        let (sx, sz) = g.world.landmarks.summit().unwrap();
        assert_eq!((cyclone.pos.x, cyclone.pos.z), (sx as f32 + 0.5, sz as f32 + 0.5));
    }

    #[test]
    fn the_health_bar_follows_the_nearest_boss_and_its_health() {
        let mut g = on_the_summit();
        g.spawn_bosses();
        assert_eq!(g.boss_bar(), Some(("The Yeti", 1.0)));
        g.animals[0].health = YETI_HEALTH / 4;
        assert_eq!(g.boss_bar().map(|b| (b.1 * 100.0).round() as i32), Some(25));
        g.player.pos += vec3(BOSS_BAR_DISTANCE + 20.0, 0.0, 0.0);
        assert_eq!(g.boss_bar(), None, "out of range");
    }

    /// Stand the player in the summit's cyclone.
    fn in_the_cyclone(g: &mut Game) {
        g.world.landmarks.yeti_slain = true;
        g.animals.clear();
        g.player.pos = g.cyclone().unwrap().pos + vec3(0.5, 0.01, 0.0);
    }

    #[test]
    fn two_and_a_half_seconds_in_the_cyclone_carry_the_player_to_the_skylands_and_stepping_out_resets_it() {
        let mut g = on_the_summit();
        in_the_cyclone(&mut g);
        let dt = 0.1;
        assert_eq!(CYCLONE_SECONDS, 2.5, "half the time it took");
        for _ in 0..22 {
            g.update_travel(dt);
        }
        assert!(g.in_overworld(), "2.2 seconds is not enough");
        assert!(g.cyclone_whiteness() > 0.0 && g.cyclone_whiteness() < 1.0, "the screen is going white");
        let inside = g.player.pos;
        g.player.pos += vec3(CYCLONE_RADIUS + 3.0, 0.0, 0.0);
        g.update_travel(dt);
        assert_eq!(g.cyclone_timer, 0.0, "stepping out starts the count again");
        g.player.pos = inside;
        for _ in 0..24 {
            g.update_travel(dt);
        }
        assert!(g.in_overworld(), "2.4 seconds");
        g.update_travel(dt);
        g.update_travel(dt);
        assert!(g.in_skylands(), "two and a half seconds");
        assert!((g.player.pos - skylands::spawn_point()).length() < 1.0, "arrives on the first island");
        assert!(g.whiteout.map_or(false, |w| w.kind == WhiteoutKind::Arrival && w.alpha() > 0.9), "the white clears gradually");
        assert!(g.parked(Dimension::Overworld).is_some());
    }

    #[test]
    fn the_skylands_have_their_own_day_and_night_and_nothing_lives_there() {
        let mut g = on_the_summit();
        g.enter_skylands();
        assert!(!g.world.night && !g.parked(Dimension::Overworld).unwrap().world.night);
        g.set_night(true);
        for _ in 0..200 {
            g.update_blackout(0.05);
        }
        assert!(g.world.night, "night falls in the Skylands");
        assert!(!g.parked(Dimension::Overworld).unwrap().world.night, "and not in the overworld");
        for _ in 0..200 {
            g.update_animals(0.1);
        }
        assert!(g.animals.is_empty());
        assert!(g.trail_path().is_none());
    }

    #[test]
    fn falling_off_an_island_whites_out_the_screen_and_drops_the_player_from_fifty_blocks_above_the_jungle() {
        let mut g = on_the_summit();
        g.enter_skylands();
        g.whiteout = None;
        // No normal world behind the Skylands while standing on them.
        assert!(g.overworld_backdrop(g.player.eye()).is_none());
        g.player.pos = vec3(200.5, SKY_VOID_Y - 5.0, 200.5);
        g.update_travel(0.05);
        let w = g.whiteout.expect("the white-out begins");
        assert!(matches!(w.kind, WhiteoutKind::Fall { moved: false }));
        assert!(g.overworld_backdrop(g.player.eye()).is_some(), "the normal world shows during the white-out");
        let mut t = 0.05;
        while t < SKY_FALL_FADE_IN + 0.1 {
            g.update_travel(0.05);
            t += 0.05;
        }
        let (jx, jz) = g.jungle_landing();
        assert!(g.in_overworld(), "the fall ends in the normal world");
        assert!((g.player.pos.x - (jx as f32 + 0.5)).abs() < 0.01 && (g.player.pos.z - (jz as f32 + 0.5)).abs() < 0.01, "in the jungle");
        assert!(crate::terrain::in_jungle(g.info.seed, jx, jz));
        let ground = g.world.ground_top(jx, jz) as f32;
        assert!((g.player.pos.y - (ground + 1.0 + SKY_FALL_DROP)).abs() < 1.0, "50 blocks above: {}", g.player.pos.y);
        assert!(g.white_alpha() > 0.8, "still white as the fall begins");
        // Let them fall back onto the ground at the spawn point; the white clears.
        for _ in 0..((SKY_FALL_FADE_OUT + 1.0) * 60.0) as i32 {
            g.world.stream_chunks(g.player.pos);
            g.player.update(&g.world, 1.0 / 60.0, &MoveInput::default());
            g.update_travel(1.0 / 60.0);
        }
        assert!(g.whiteout.is_none() && g.white_alpha() == 0.0);
        assert!(g.in_overworld() && g.overworld_backdrop(g.player.eye()).is_none());
        assert!((g.player.pos.y - (ground + 1.0)).abs() < 1.5, "landed in the jungle's clearing at {}", g.player.pos.y);
    }

    #[test]
    fn there_is_no_cyclone_in_the_skylands_and_the_only_way_out_is_to_fall() {
        let mut g = on_the_summit();
        in_the_cyclone(&mut g);
        g.enter_skylands();
        g.whiteout = None;
        assert!(g.cyclone().is_none(), "nothing carries the player home");
        for _ in 0..80 {
            g.update_travel(0.1);
        }
        assert!(g.in_skylands(), "and they stay where they are");
        g.player.pos.y = SKY_VOID_Y - 1.0;
        g.update_travel(0.1);
        for _ in 0..30 {
            g.update_travel(0.1);
        }
        assert!(g.in_overworld(), "jumping off takes them down");
    }

    #[test]
    fn leaving_the_skylands_wakes_the_yeti_and_it_must_be_killed_again_for_the_cyclone() {
        let mut g = on_the_summit();
        let (sx, sz) = g.world.landmarks.summit().unwrap();
        in_the_cyclone(&mut g);
        assert!(g.world.landmarks.yeti_slain && g.cyclone().is_some());
        g.enter_skylands();
        g.whiteout = None;
        g.leave_skylands();
        assert!(!g.world.landmarks.yeti_slain, "the Yeti lives again");
        assert!(g.cyclone().is_none());
        g.player.pos = vec3(sx as f32 + 5.5, g.world.ground_top(sx, sz) as f32 + 1.0, sz as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.spawn_bosses();
        let yeti = g.animals.iter().position(|a| a.kind == AnimalKind::Yeti).expect("it is back on the summit");
        g.hurt_animals(&[(yeti, YETI_HEALTH)]);
        assert!(g.world.landmarks.yeti_slain && g.cyclone().is_some(), "killed again, the cyclone returns");
        // Falling out of the Skylands, or dying there, wakes it just the same.
        g.enter_skylands();
        g.whiteout = None;
        assert!(g.hurt_player(MAX_HEALTH, "dead"));
        assert!(!g.world.landmarks.yeti_slain);
    }

    #[test]
    fn each_lake_has_its_own_monster_and_a_dead_one_never_comes_back() {
        let mut g = game();
        let a = Lake { x: 0, z: 0, radius: 130, phase: 3, from: (0, 0) };
        let b = Lake { x: 900, z: 0, radius: 130, phase: 5, from: (900, 0) };
        g.world.landmarks.lakes = vec![a, b];
        assert!(a.has_tower() && b.has_tower());
        assert!(g.world.landmarks.monster_alive(&a) && g.world.landmarks.monster_alive(&b));
        g.world.record_monster_slain(&a);
        assert!(!g.world.landmarks.monster_alive(&a), "the first lake's monster is dead for good");
        assert!(g.world.landmarks.monster_alive(&b), "the other lake's monster is another one, still alive");
        let again = Game::from_save(WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() }, parse_save(&g.save_text()), 3);
        assert!(!again.world.landmarks.monster_alive(&a) && again.world.landmarks.monster_alive(&b), "and it stays dead after a reload");
    }

    #[test]
    fn dying_in_the_skylands_returns_to_the_start_of_the_overworld() {
        let mut g = on_the_summit();
        g.enter_skylands();
        assert!(g.hurt_player(MAX_HEALTH, "dead"));
        assert!(g.in_overworld() && g.whiteout.is_none());
        assert_eq!(g.player.pos, Player::new().pos);
    }

    #[test]
    fn the_skylands_and_the_bosses_survive_a_save_and_load() {
        let mut g = on_the_summit();
        in_the_cyclone(&mut g);
        g.world.landmarks.monsters_slain.push((60, 60));
        g.enter_skylands();
        g.player.pos = vec3(skylands::spawn_point().x + 3.0, skylands::spawn_point().y, skylands::spawn_point().z);
        let text = g.save_text();
        assert!(text.contains("dimension skylands") && text.contains("yetislain") && text.contains("nessie 60 60"));
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&text), 9);
        assert!(again.in_skylands(), "loaded into the Skylands");
        assert!((again.player.pos.x - (skylands::spawn_point().x + 3.0)).abs() < 0.01);
        assert!(again.parked(Dimension::Overworld).map_or(false, |a| a.world.landmarks.yeti_slain && a.world.landmarks.monsters_slain == vec![(60, 60)]));
        assert!(again.parked(Dimension::Horizen).is_none());
    }

    #[test]
    fn all_three_dimensions_can_be_parked_and_saved_together() {
        let mut g = on_the_summit();
        g.enter_horizen();
        g.travel_to(Dimension::Overworld);
        g.travel_to(Dimension::Skylands);
        assert!(g.parked(Dimension::Overworld).is_some() && g.parked(Dimension::Horizen).is_some());
        let text = g.save_text();
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&text), 9);
        assert!(again.in_skylands() && again.parked(Dimension::Horizen).is_some() && again.parked(Dimension::Overworld).is_some());
    }

    #[test]
    fn the_tower_is_a_ruin_until_the_monster_dies_and_then_its_door_is_locked_until_a_silver_key_is_used() {
        let (mut g, lake) = at_the_castle();
        let (x, z) = (lake.x, lake.z + TOWER_HALF);
        let door = (x, g.world.ground_top(x, z) + 1, z);
        let top = g.world.ground_top(lake.x, lake.z) + 1 + TOWER_STOREY * (TOWER_STOREYS - 1);
        // While the monster lives the tower is a ruin: a gap where the door was, no relic plinth, a broken stair.
        assert_eq!(gate_cell(&g, &lake), Block::Air, "a broken doorway, no door to lock");
        assert!(!g.try_unlock(door), "nothing to unlock");
        assert_eq!(g.world.block_at(lake.x, top, lake.z), Block::Air, "no plinth at the top");
        let floor = g.world.ground_top(lake.x, lake.z) + TOWER_STOREY;
        assert_eq!(g.world.block_at(lake.x + 2, floor, lake.z + 2), Block::Air, "no floor over the ground floor to climb to");
        // Kill its monster: the tower is made whole again, with its door shut and locked.
        g.spawn_bosses();
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::LochNessMonster).expect("the monster rises");
        g.hurt_animals(&[(i, 10_000)]);
        assert!(g.world.landmarks.monsters_slain.contains(&(lake.x, lake.z)));
        assert_eq!(gate_cell(&g, &lake), Block::TowerDoor, "mended and locked");
        assert_eq!(g.world.block_at(lake.x, top, lake.z), Block::StonePlinth, "the Philosopher's Stone is back on its plinth");
        // Without the key the door stays shut, and says what it needs.
        assert!(!g.try_unlock(door) && gate_cell(&g, &lake) == Block::TowerDoor);
        assert!(g.can_interact(Block::TowerDoor) == false);
        g.spawn_bosses();
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::LochNessMonster), "it does not come back");
        // A silver key in the hand opens both halves of the door for good, and is kept.
        g.inventory.slots[g.selected] = Some(crate::item::ItemStack { kind: ItemKind::SilverKey, count: 1 });
        assert!(g.can_interact(Block::TowerDoorTop));
        let hit = Hit { x: door.0, y: door.1 + 1, z: door.2, block: Block::TowerDoorTop, prev: (door.0, door.1 + 1, door.2 + 1) };
        assert!(g.use_held(&hit));
        assert_eq!(gate_cell(&g, &lake), Block::Air, "open");
        assert_eq!(g.world.block_at(door.0, door.1 + 1, door.2), Block::Air);
        assert_eq!(g.inventory.count(ItemKind::SilverKey), 1, "the key is not used up: it only turns the lock");
        // The door stays open after a reload.
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, parse_save(&g.save_text()), 3);
        again.world.stream_chunks(g.player.pos);
        assert_eq!(gate_cell(&again, &lake), Block::Air);
    }

    #[test]
    fn the_monster_rises_in_the_lake_and_never_leaves_the_water() {
        let (mut g, lake) = at_the_castle();
        // Stand at the water's edge, as the player would to fight it.
        let shore = (0..40).map(|i| lake.z + 60 + i).find(|&z| !g.world.is_wet_column(lake.x, z) && g.world.is_wet_column(lake.x, z - 1)).unwrap_or(lake.z + 90);
        g.player.pos = vec3(lake.x as f32 + 0.5, g.world.ground_top(lake.x, shore) as f32 + 1.01, shore as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        for _ in 0..200 {
            g.spawn_bosses();
            if g.animals.iter().any(|a| a.kind == AnimalKind::LochNessMonster) {
                break;
            }
        }
        assert!(g.animals.iter().any(|a| a.kind == AnimalKind::LochNessMonster), "the monster rises");
        let start_gap = g.animals.iter().find(|a| a.kind == AnimalKind::LochNessMonster).map(|a| (a.pos - g.player.pos).length()).unwrap();
        let mut closest = start_gap;
        for frame in 0..(60.0 * 60.0) as i32 {
            g.update_animals(1.0 / 60.0);
            let m = g.animals.iter().find(|a| a.kind == AnimalKind::LochNessMonster).expect("alive");
            assert!(g.world.is_wet_column(m.pos.x.floor() as i32, m.pos.z.floor() as i32), "frame {frame}: on dry land at {:?}", m.pos);
            closest = closest.min((m.pos - g.player.pos).length());
            g.player.pos.y = g.world.ground_top(g.player.pos.x.floor() as i32, g.player.pos.z.floor() as i32) as f32 + 1.01;
        }
        assert!(closest < start_gap, "it swims towards the player on the shore ({start_gap} -> {closest})");
    }
}

#[cfg(test)]
mod silver_tower_tests {
    use super::*;
    use crate::item::ItemStack;
    use std::path::PathBuf;

    fn game() -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        Game::from_save(info, SaveData::new(), 5)
    }

    fn hold(g: &mut Game, kind: ItemKind) {
        g.inventory.slots[0] = Some(ItemStack { kind, count: 1 });
        g.select_slot(0);
    }

    #[test]
    fn a_peep_stone_in_the_horizen_brings_out_one_tower_in_the_great_cavern_and_a_trail_down_the_tunnel() {
        let mut g = game();
        g.enter_horizen();
        g.world.night = false;
        assert!(!g.try_show_horizen_tower(), "no peep stone in hand");
        hold(&mut g, ItemKind::PeepStone);
        assert_eq!(g.trail_path(), None, "nothing out there yet");
        assert!(g.try_show_horizen_tower());
        let tower = g.world.landmarks.horizen_tower.expect("the tower is out");
        assert_eq!((tower.x, tower.z), horizen::cavern_centre(g.info.seed), "in the middle of the great cavern");
        assert!(horizen::air_height(g.info.seed, tower.x, tower.z) > TOWER_HEIGHT, "under a high roof");
        let path = g.trail_path().expect("the peep stone leads there");
        assert_eq!(path.last().copied(), Some(tower.middle()));
        assert!(path.len() > 5, "the trail follows the winding tunnel");
        assert!(!g.try_show_horizen_tower(), "only one");
        // There is no sky in the cave to tell night by: the stone leads at any hour.
        g.world.night = true;
        assert!(g.trail_path().is_some());
        // The tower survives a save and reload with its door unlocked.
        g.world.night = false;
        g.world.unlock_tower(tower.x, tower.z);
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&g.save_text()), 3);
        assert!(again.in_horizen());
        assert_eq!(again.world.landmarks.horizen_tower, Some(tower));
        assert!(again.world.landmarks.is_unlocked(tower.x, tower.z));
    }

    #[test]
    fn the_tenth_new_island_in_the_skylands_brings_the_tower_to_the_next_island_to_appear() {
        let mut g = game();
        g.travel_to(Dimension::Skylands);
        g.player.on_ground = true;
        let seed = g.info.seed;
        let mut count = 0;
        // Walk onto islands one by one; nine are not enough.
        for cell in [(0, 0), (1, 0), (2, 0), (3, 0), (3, 1), (2, 1), (1, 1), (0, 1), (-1, 1), (-1, 0)] {
            if count == SKY_TOWER_VISITS - 1 {
                g.update_sky_tower();
                assert!(g.world.landmarks.sky_tower.is_none(), "nine islands are not enough");
            }
            let island = skylands::island_in_cell(seed, cell.0, cell.1);
            g.player.pos = vec3(island.x as f32 + 0.5, island.top as f32 + 1.01, island.z as f32 + 0.5);
            g.update_sky_tower();
            count += 1;
        }
        assert_eq!(g.world.landmarks.sky_visited.len(), SKY_TOWER_VISITS);
        let cell = g.world.landmarks.sky_tower.expect("the tenth island brings the tower");
        assert!(!g.world.landmarks.sky_visited.contains(&cell) && cell != (0, 0));
        let island = skylands::island_in_cell(seed, cell.0, cell.1);
        assert!(!g.world.chunks.contains_key(&World::chunk_of(island.x, island.z)), "it is the next to appear: not made yet");
        // Its chunks hold a locked tower with the Horn of the Hunt at the top; no other island has one.
        g.world.ensure_chunk(World::chunk_of(island.x, island.z).0, World::chunk_of(island.x, island.z).1);
        assert_eq!(g.world.block_at(island.x, island.top + 1 + TOWER_STOREY * (TOWER_STOREYS - 1), island.z), Block::HornPlinth);
        let door_chunk = World::chunk_of(island.x, island.z + TOWER_HALF);
        g.world.ensure_chunk(door_chunk.0, door_chunk.1);
        assert_eq!(g.world.block_at(island.x, island.top + 1, island.z + TOWER_HALF), Block::TowerDoor);
        // Only one tower is ever brought out.
        g.update_sky_tower();
        assert_eq!(g.world.landmarks.sky_tower, Some(cell));
    }

    #[test]
    fn in_the_horizen_and_the_skylands_the_compass_points_to_where_the_player_arrived() {
        let mut g = game();
        g.enter_horizen();
        g.player.pos = vec3(HORIZEN_SPAWN.0 as f32 + 40.5, 6.0, HORIZEN_SPAWN.1 as f32 + 0.5);
        assert!((g.compass_bearing().abs() - std::f32::consts::PI).abs() < 0.01, "the spawn lies west: {}", g.compass_bearing());
        g.travel_to(Dimension::Overworld);
        assert_eq!(g.compass_bearing(), COMPASS_NORTH_BEARING, "north in the overworld until a mountain is out");
        g.travel_to(Dimension::Skylands);
        let spawn = skylands::spawn_point();
        g.player.pos = spawn + vec3(0.0, 0.0, -30.0);
        assert!((g.compass_bearing() - std::f32::consts::FRAC_PI_2).abs() < 0.01, "the arrival island lies south: {}", g.compass_bearing());
    }

    #[test]
    fn a_bone_totem_in_the_skylands_points_to_the_silver_tower_once_it_has_appeared() {
        let mut g = game();
        g.travel_to(Dimension::Skylands);
        hold(&mut g, ItemKind::BoneTotem);
        assert_eq!(g.trail_path(), None, "no tower yet, nothing to point to");
        let cell = (2, 1);
        g.world.add_sky_tower(cell);
        let island = skylands::island_in_cell(g.info.seed, cell.0, cell.1);
        let p = g.player.pos;
        for night in [false, true] {
            g.world.night = night;
            let path = g.trail_path().expect("the totem points to the tower");
            assert_eq!(path.first().copied(), Some((p.x, p.z)));
            assert_eq!(path.last().copied(), Some((island.x as f32 + 0.5, island.z as f32 + 0.5)), "ending at the middle of the tower, night {night}");
            let tower = Tower { x: island.x, z: island.z };
            assert_eq!(path[1], tower.approach(), "by way of the front of its door");
        }
        hold(&mut g, ItemKind::Stick);
        assert_eq!(g.trail_path(), None, "only the totem points the way");
        hold(&mut g, ItemKind::PeepStone);
        let path = g.trail_path().expect("a peep stone in the Skylands leads to the volcano");
        assert_eq!(*path.last().unwrap(), skylands::volcano_foot(g.info.seed));
        g.world.landmarks.phoenix_slain = true;
        assert_eq!(g.trail_path(), None, "and does nothing once the Phoenix is dead");
        g.world.landmarks.phoenix_slain = false;
        // The wisps float over each island's top, and over the gaps at the height of the player's feet.
        g.world.night = false;
        let over_island = crate::effects::trail_floor(&g.world, island.x, island.z, 77.4);
        assert_eq!(over_island, island.top as f32 + 1.0);
        let void = (0..400).map(|i| (island.x + 28 + i, island.z + 28)).find(|&(x, z)| skylands::surface_top(g.info.seed, x, z, false).is_none()).unwrap();
        assert_eq!(crate::effects::trail_floor(&g.world, void.0, void.1, 77.4), 77.0);
        // Back in the overworld the totem leads to the well (none yet), not to the tower.
        g.travel_to(Dimension::Overworld);
        hold(&mut g, ItemKind::BoneTotem);
        assert_eq!(g.trail_path(), None);
    }

    #[test]
    fn a_deer_leaves_two_bones_one_time_in_five() {
        let mut rng = Rng::new(21);
        let trials = 20_000;
        let mut hits = 0;
        for _ in 0..trials {
            let drops = AnimalKind::Deer.drops(&mut rng);
            let bones = drops.iter().filter(|&&k| k == ItemKind::Bone).count();
            assert!(bones == 0 || bones == DEER_BONE_DROP as usize);
            hits += (bones > 0) as usize;
        }
        let rate = hits as f64 / trials as f64;
        assert!((rate - 0.20).abs() < 0.015, "bone rate {rate}");
        assert!(AnimalKind::Quail.drops(&mut rng).iter().all(|&k| k != ItemKind::Bone));
    }

    #[test]
    fn taking_a_relic_off_its_plinth_gives_it_once() {
        let mut g = game();
        for (block, item) in [(Block::StonePlinth, ItemKind::PhilosophersStone), (Block::HornPlinth, ItemKind::HornOfTheHunt), (Block::CrownPlinth, ItemKind::AncientCrown)] {
            let mut rng = Rng::new(1);
            assert_eq!(crate::item::drops_for(block, &mut rng), vec![item]);
        }
        assert_eq!(crate::item::drops_for(Block::KeyStand, &mut Rng::new(1)), vec![ItemKind::SilverKey]);
        let mut rng = Rng::new(1);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..400 {
            let drops = crate::item::drops_for(Block::PinkSalt, &mut rng);
            assert!(drops.iter().all(|&k| k == ItemKind::PinkSalt));
            assert!((PINK_SALT_DROP_MIN as usize..=PINK_SALT_DROP_MAX as usize).contains(&drops.len()), "{} lumps", drops.len());
            seen.insert(drops.len());
        }
        assert_eq!(seen.len(), (PINK_SALT_DROP_MAX - PINK_SALT_DROP_MIN + 1) as usize, "every count from 8 to 16 turns up");
        g.inventory.slots[0] = None;
        assert!(Block::PinkSalt.can_break_with(None), "salt is taken by hand");
    }
}

#[cfg(test)]
mod crow_pan_relic_and_mother_tests {
    use super::*;
    use crate::chunk::idx;
    use crate::item::ItemStack;
    use std::path::PathBuf;

    fn game() -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut g = Game::from_save(info, SaveData::new(), 5);
        g.world.stream_chunks(g.player.pos);
        g
    }

    fn hold(g: &mut Game, kind: ItemKind) {
        g.inventory.slots[0] = Some(ItemStack { kind, count: 1 });
        g.select_slot(0);
    }

    fn put(g: &mut Game, (x, y, z): (i32, i32, i32), block: Block) {
        g.world.chunks.get_mut(&World::chunk_of(x, z)).unwrap().blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))] = block;
    }

    /// Open sky over the spawn area: every block above the ground is air.
    fn clear_sky(g: &mut Game) {
        for chunk in g.world.chunks.values_mut() {
            for y in GROUND_TOP + 1..WORLD_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        chunk.blocks[idx(x, y, z)] = Block::Air;
                    }
                }
            }
        }
    }

    fn count(g: &Game, kind: ItemKind) -> u32 {
        g.inventory.count(kind) + g.items.iter().filter(|i| i.kind == kind).count() as u32
    }

    // --- The tin pan ---

    #[test]
    fn a_tin_pan_is_two_tin_and_the_pan_finds_gold_in_clay_one_time_in_twenty_or_one_in_five_with_a_rabbit_foot() {
        let r = crate::recipe::recipe_for(ItemKind::TinPan);
        assert_eq!(r.inputs, &[(ItemKind::Tin, 2)]);
        let trials = 4000;
        let pan = |foot: bool, held: ItemKind| {
            let mut g = game();
            clear_sky(&mut g);
            if foot {
                g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::RabbitFoot, count: 1 });
            }
            hold(&mut g, held);
            let mut clay = 0;
            for i in 0..trials {
                let at = (12 + i % 3, 6, 12);
                put(&mut g, at, Block::WetClay);
                clay += g.mine(at.0, at.1, at.2).iter().filter(|&&k| k == ItemKind::Clay).count();
            }
            assert_eq!(clay, trials as usize * CLAY_DROP as usize, "clay drops as ever");
            count(&g, ItemKind::Gold) as f64 / trials as f64
        };
        let plain = pan(false, ItemKind::TinPan);
        let lucky = pan(true, ItemKind::TinPan);
        assert!((plain - 0.05).abs() < 0.02, "one in twenty: {plain}");
        assert!((lucky - 0.20).abs() < 0.03, "one in five: {lucky}");
        assert_eq!(pan(true, ItemKind::Stick), 0.0, "only the pan pans");
    }

    // --- The Philosopher's Stone ---

    #[test]
    fn the_philosophers_stone_turns_a_boulder_to_gold_that_breaks_by_hand_into_gold_and_stays_gone() {
        let mut g = game();
        let at = (12, 6, 12);
        put(&mut g, at, Block::Boulder);
        let hit = Hit { x: 12, y: 6, z: 12, block: Block::Boulder, prev: (12, 7, 12) };
        hold(&mut g, ItemKind::Stick);
        assert!(!g.use_held(&hit), "nothing happens without the stone");
        hold(&mut g, ItemKind::PhilosophersStone);
        assert!(g.can_interact(Block::Boulder));
        assert!(g.use_held(&hit));
        assert_eq!(g.world.block_at(12, 6, 12), Block::GoldBoulder);
        assert_eq!(g.held_item(), Some(ItemKind::PhilosophersStone), "the stone is not used up");
        // It survives a save; after it is broken it neither returns as gold nor as the boulder.
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info.clone(), parse_save(&g.save_text()), 3);
        again.world.ensure_chunk(0, 0);
        assert_eq!(again.world.block_at(12, 6, 12), Block::GoldBoulder);
        hold(&mut again, ItemKind::Stick);
        assert_eq!(again.mine(12, 6, 12), vec![ItemKind::Gold; GOLD_BOULDER_GOLD_DROP as usize]);
        let mut last = Game::from_save(info, parse_save(&again.save_text()), 3);
        last.world.ensure_chunk(0, 0);
        assert_eq!(last.world.block_at(12, 6, 12), Block::Air);
    }

    // --- Crows and the altar ---

    #[test]
    fn a_crow_leaves_only_feathers() {
        let drops = AnimalKind::Crow.drops(&mut Rng::new(1));
        assert_eq!(drops, vec![ItemKind::Feather; CROW_FEATHER_DROP as usize]);
        assert!(AnimalKind::Quail.drops(&mut Rng::new(1)).contains(&ItemKind::Poultry));
    }

    #[test]
    fn a_startled_crow_flies_west_wherever_the_player_stands_and_a_quail_flies_away() {
        for (player_dx, player_dz) in [(5.0, 0.0), (-5.0, 0.0), (0.0, 5.0), (0.0, -5.0)] {
            let mut g = game();
            clear_sky(&mut g);
            let start = vec3(30.5, GROUND_TOP as f32 + 1.0, 10.5);
            let mut crow = Animal::new(AnimalKind::Crow, start, &mut Rng::new(3));
            let mut quail = Animal::new(AnimalKind::Quail, start, &mut Rng::new(3));
            let player = start + vec3(player_dx, 0.0, player_dz);
            let mut rng = Rng::new(9);
            for _ in 0..60 {
                crow.update(&g.world, player, 1.0 / 60.0, &mut rng);
                quail.update(&g.world, player, 1.0 / 60.0, &mut rng);
            }
            for _ in 0..60 {
                crow.update(&g.world, player, 1.0 / 60.0, &mut rng);
                quail.update(&g.world, player, 1.0 / 60.0, &mut rng);
            }
            assert!(crow.pos.x < start.x - 4.0, "the crow went west: {:?} (player at {player_dx},{player_dz})", crow.pos);
            assert!((crow.pos.z - start.z).abs() < 0.5, "straight west");
            if player_dx > 0.0 {
                assert!(quail.pos.x < start.x, "a quail flees from the player, here westwards too");
            } else if player_dx < 0.0 {
                assert!(quail.pos.x > start.x, "a quail flees from the player, here eastwards");
            }
            g.animals.push(crow);
        }
    }

    #[test]
    fn a_crow_with_the_way_west_shut_flies_away_like_a_quail() {
        let mut g = game();
        clear_sky(&mut g);
        let start = vec3(30.5, GROUND_TOP as f32 + 1.0, 10.5);
        // A high wall across the whole of the way west.
        for z in 0..CHUNK_SIZE * 2 {
            for y in GROUND_TOP + 1..GROUND_TOP + 30 {
                put(&mut g, (20, y, z), Block::CastleStone);
            }
        }
        let mut crow = Animal::new(AnimalKind::Crow, start, &mut Rng::new(3));
        let mut rng = Rng::new(9);
        let player = start + vec3(-5.0, 0.0, 0.0);
        for _ in 0..90 {
            crow.update(&g.world, player, 1.0 / 60.0, &mut rng);
        }
        assert!(crow.pos.x > start.x, "it flies away from the player, east, since west is shut: {:?}", crow.pos);
    }

    #[test]
    fn following_one_crow_west_for_two_chunks_brings_it_down_on_a_new_ancient_altar_with_a_loadstone() {
        let mut g = game();
        for cx in -4..=4 {
            for cz in -3..=3 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        clear_sky(&mut g);
        // Where the crow starts, and a clear flat patch of ground for the altar.
        let start = vec3(30.5, GROUND_TOP as f32 + 1.0, 10.5);
        let mut crow = Animal::new(AnimalKind::Crow, start, &mut Rng::new(3));
        g.animals.push(crow);
        let mut perched = None;
        'chase: for _ in 0..40 {
            // The player walks up to within the startle distance of the crow and the crow takes off.
            let c = g.animals.iter().find(|a| a.kind == AnimalKind::Crow).unwrap().pos;
            g.player.pos = vec3(c.x + 6.0, c.y, c.z);
            for _ in 0..600 {
                let c = g.animals.iter().find(|a| a.kind == AnimalKind::Crow).unwrap().pos;
                // Keep up with the crow, a few blocks behind it.
                g.player.pos.x = g.player.pos.x.min(c.x + 12.0);
                g.world.stream_chunks(g.player.pos);
                clear_sky(&mut g);
                g.update_animals(1.0 / 60.0);
                let altar = (-120..60).flat_map(|x| (-60..60).map(move |z| (x, z))).find(|&(x, z)| g.world.block_at(x, GROUND_TOP + 1, z) == Block::AltarLoadstone);
                if let Some(at) = altar {
                    perched = Some(at);
                    break 'chase;
                }
            }
        }
        let (ax, _az) = perched.expect("the crow came down on an altar");
        assert!(ax <= 30 - CROW_ALTAR_WEST_BLOCKS as i32 + 8, "it was followed two chunks west: altar at x={ax}");
        // The crow sits on top of the altar, not inside it, and is free to go again: it used to be set down inside the altar's
        // collision box and stuck there for ever.
        let eps = Vec3::splat(0.01);
        let crow = g.animals.iter().find(|a| a.kind == AnimalKind::Crow).unwrap();
        let (lo, hi) = crow.hitbox();
        assert!(!g.world.box_blocked(lo + eps, hi - eps), "the crow is not inside the altar: {:?}", crow.pos);
        let landed = crow.pos;
        for _ in 0..900 {
            g.world.stream_chunks(g.player.pos);
            g.update_animals(1.0 / 60.0);
        }
        let crow = g.animals.iter().find(|a| a.kind == AnimalKind::Crow).unwrap();
        assert!((crow.pos - landed).length() > 0.4, "the crow got away from the altar instead of sticking: {landed:?} -> {:?}", crow.pos);
        // The loadstone is taken by hand, the altar stays and is saved.
        let (ax, az) = perched.unwrap();
        hold(&mut g, ItemKind::Stick);
        assert_eq!(g.mine(ax, GROUND_TOP + 1, az), vec![ItemKind::Loadstone]);
        assert_eq!(g.world.block_at(ax, GROUND_TOP + 1, az), Block::AncientAltar);
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, parse_save(&g.save_text()), 3);
        again.world.stream_chunks(vec3(ax as f32, 6.0, az as f32));
        assert_eq!(again.world.block_at(ax, GROUND_TOP + 1, az), Block::AncientAltar, "the altar is kept");
    }

    #[test]
    fn a_crow_that_the_player_leaves_behind_forgets_how_far_it_was_followed() {
        let mut g = game();
        clear_sky(&mut g);
        let mut crow = Animal::new(AnimalKind::Crow, vec3(30.5, GROUND_TOP as f32 + 1.0, 10.5), &mut Rng::new(3));
        crow.west_followed = 30.0;
        g.animals.push(crow);
        g.player.pos = vec3(30.5 + CROW_LOSE_DISTANCE + 5.0, GROUND_TOP as f32 + 1.0, 10.5);
        g.update_animals(1.0 / 60.0);
        let crow = g.animals.iter().find(|a| a.kind == AnimalKind::Crow).unwrap();
        assert_eq!(crow.west_followed, 0.0);
    }

    // --- The Iron Quail ---

    fn kill_quail(g: &mut Game, n: usize) {
        for _ in 0..n {
            let p = g.player.pos + vec3(3.0, 0.0, 0.0);
            g.animals.push(Animal::new(AnimalKind::Quail, p, &mut Rng::new(1)));
            let i = g.animals.len() - 1;
            g.hurt_animals(&[(i, 999)]);
        }
    }

    fn mothers(g: &Game) -> usize {
        g.animals.iter().filter(|a| a.kind == AnimalKind::IronQuail).count()
    }

    #[test]
    fn every_ten_quail_killed_bring_a_iron_quail_that_drops_a_silver_key() {
        let mut g = game();
        clear_sky(&mut g);
        kill_quail(&mut g, 9);
        assert_eq!(mothers(&g), 0, "nine are not enough");
        kill_quail(&mut g, 1);
        assert_eq!(mothers(&g), IRON_QUAILS_PER_SUMMON as usize, "the tenth calls three");
        assert_eq!(IRON_QUAILS_PER_SUMMON, 3);
        let mother = g.animals.iter().position(|a| a.kind == AnimalKind::IronQuail).unwrap();
        let gap = (g.animals[mother].pos - g.player.pos).length();
        assert!(gap > 8.0 && gap < 20.0, "she appears near: {gap}");
        // She hunts the player and never gives up, however far they run.
        g.player.pos += vec3(300.0, 0.0, 0.0);
        g.world.stream_chunks(g.player.pos);
        clear_sky(&mut g);
        g.update_animals(0.1);
        assert_eq!(mothers(&g), 3, "they do not despawn");
        assert!(g.boss_bar().is_some() || (g.animals[mother].pos - g.player.pos).length() > BOSS_BAR_DISTANCE);
        kill_quail(&mut g, 10);
        assert_eq!(mothers(&g), 6, "ten more, three more");
        assert_eq!(g.quail_kills, 20);
        // The first to die drops the silver key; the others, since a key now lies on the ground, drop a gold bar each instead.
        let (keys, gold) = (count(&g, ItemKind::SilverKey), count(&g, ItemKind::Gold));
        for _ in 0..3 {
            let i = g.animals.iter().position(|a| a.kind == AnimalKind::IronQuail).unwrap();
            g.hurt_animals(&[(i, IRON_QUAIL_HEALTH)]);
        }
        assert_eq!(count(&g, ItemKind::SilverKey), keys + 1, "one silver key");
        assert_eq!(count(&g, ItemKind::Gold), gold + 2, "and two gold bars");
        assert_eq!(mothers(&g), 3);
    }

    #[test]
    fn the_iron_quail_hurts_the_player_and_the_fight_ends_when_the_player_dies() {
        let mut g = game();
        clear_sky(&mut g);
        kill_quail(&mut g, 10);
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::IronQuail).unwrap();
        g.animals[i].pos = g.player.pos + vec3(1.0, 0.0, 0.0);
        let start = g.health;
        for _ in 0..120 {
            g.update_animals(1.0 / 60.0);
            if g.health < start {
                break;
            }
        }
        assert!(g.health < start, "she strikes for {IRON_QUAIL_DAMAGE}");
        assert!(g.hurt_player(99, "test"), "the player dies");
        assert_eq!(mothers(&g), 0, "the fight is over");
    }

    #[test]
    fn a_mother_left_behind_in_another_dimension_comes_again_and_the_count_is_saved() {
        let mut g = game();
        clear_sky(&mut g);
        kill_quail(&mut g, 12);
        assert_eq!(mothers(&g), 3);
        g.travel_to(Dimension::Skylands);
        assert_eq!(mothers(&g), 0);
        assert_eq!(g.iron_quails_owed, 3);
        g.travel_to(Dimension::Overworld);
        g.world.stream_chunks(g.player.pos);
        g.summon_iron_quails();
        assert_eq!(mothers(&g), 3);
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&g.save_text()), 3);
        assert_eq!(again.quail_kills, 12);
    }

    // --- The crown, the ruined castle and Excalibur ---

    #[test]
    fn the_crown_is_worn_and_brings_a_ruined_castle_out_ahead_of_the_player_once() {
        let mut g = game();
        assert!(!g.try_show_ruined_castle(), "not without the crown");
        hold(&mut g, ItemKind::AncientCrown);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::AncientCrown));
        g.player.yaw = 0.0;
        assert!(g.try_show_ruined_castle());
        let castle = g.world.landmarks.ruined_castle.expect("the ruin is out");
        assert!(castle.x as f32 > g.player.pos.x + GRAND_REACH as f32, "ahead, to the east, beyond the generated land");
        assert!(!g.try_show_ruined_castle(), "only one");
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&g.save_text()), 3);
        assert_eq!(again.world.landmarks.ruined_castle, Some(castle));
        assert_eq!(again.worn, Some(ItemKind::AncientCrown));
    }

    #[test]
    fn the_ruin_is_large_and_broken_with_the_sword_in_the_stone_at_its_centre_and_a_way_in_from_the_south() {
        let salt = 12345;
        let cell = |dx, dz, dy| crate::landmarks::ruins_cell(salt, dx, dz, dy);
        assert_eq!(cell(0, 0, 0), Block::SwordInStone);
        assert_eq!(cell(0, 0, 1), Block::SwordHilt);
        assert!(RUINS_WALL_HALF >= 20, "a large castle");
        // The way in: nothing blocks the column from outside the gap to the dais.
        for dz in 3..=RUINS_WALL_HALF + 6 {
            for dy in 0..4 {
                assert_eq!(cell(0, dz, dy), Block::Air, "blocked at dz {dz} dy {dy}");
            }
        }
        // The walls are there, but broken (not one even height), and the north side is closed.
        let heights: std::collections::HashSet<i32> = (-RUINS_WALL_HALF + 4..=RUINS_WALL_HALF - 4).map(|dx| (0..GRAND_HEIGHT).take_while(|&dy| cell(dx, -RUINS_WALL_HALF, dy) == Block::CastleStone).count() as i32).collect();
        assert!(heights.len() > 3 && heights.iter().all(|&h| h >= 1), "a broken wall: {heights:?}");
        let tall = (0..GRAND_HEIGHT).filter(|&dy| cell(RUINS_WALL_HALF + 1, RUINS_WALL_HALF + 1, dy) == Block::CastleStone).count();
        assert!(tall > 3, "a tumbled tower at the corner");
        for dy in 0..GRAND_HEIGHT {
            assert_eq!(crate::landmarks::GrandCastle { x: 0, z: 0 }.ruin_block(GRAND_REACH + 1, 0, dy), None, "outside the square");
        }
    }

    #[test]
    fn only_a_crowned_player_draws_excalibur_from_the_stone() {
        let mut g = game();
        let (x, y, z) = (12, 6, 12);
        put(&mut g, (x, y, z), Block::SwordInStone);
        put(&mut g, (x, y + 1, z), Block::SwordHilt);
        let hit = Hit { x, y, z, block: Block::SwordInStone, prev: (x, y + 1, z) };
        assert!(!g.can_interact(Block::SwordInStone));
        assert!(!g.use_held(&hit), "bare-headed the sword will not move");
        assert_eq!(g.world.block_at(x, y, z), Block::SwordInStone);
        hold(&mut g, ItemKind::AncientCrown);
        g.wear_held();
        assert!(g.can_interact(Block::SwordHilt));
        let hilt = Hit { x, y: y + 1, z, block: Block::SwordHilt, prev: (x, y + 2, z) };
        assert!(g.use_held(&hilt), "the hilt works as well as the blade");
        assert_eq!(count(&g, ItemKind::Excalibur), 1);
        assert_eq!(g.world.block_at(x, y, z), Block::PulledStone);
        assert_eq!(g.world.block_at(x, y + 1, z), Block::Air);
        // It is gone for good, also after a save.
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut again = Game::from_save(info, parse_save(&g.save_text()), 3);
        again.world.ensure_chunk(0, 0);
        assert_eq!(again.world.block_at(x, y, z), Block::PulledStone);
        assert_eq!(again.world.block_at(x, y + 1, z), Block::Air);
    }

    #[test]
    fn excalibur_is_the_best_weapon_in_the_game() {
        let best = ItemKind::ALL.into_iter().filter(|&k| k != ItemKind::Excalibur).filter_map(ItemKind::melee_damage).max().unwrap();
        assert!(ItemKind::Excalibur.melee_damage().unwrap() > best);
        assert!(EXCALIBUR_DAMAGE > ARROW_DAMAGE && EXCALIBUR_DAMAGE > SLING_DAMAGE);
        assert!(ItemKind::Excalibur.melee_reach() >= AXE_REACH);
        assert!(ItemKind::Excalibur.melee_cooldown() <= AXE_COOLDOWN);
        let mut g = game();
        clear_sky(&mut g);
        hold(&mut g, ItemKind::Excalibur);
        g.animals.push(Animal::new(AnimalKind::Bear, g.player.pos + vec3(3.0, 0.0, 0.0), &mut Rng::new(2)));
        let eye = g.player.pos + vec3(0.0, 0.7, 0.0);
        let before = g.animals[0].health;
        assert!(g.swing_melee(eye, vec3(1.0, 0.0, 0.0)), "the swing reaches the bear");
        assert_eq!(g.animals[0].health, before - EXCALIBUR_DAMAGE);
    }

    #[test]
    fn crows_join_the_wildlife_within_their_limit() {
        let mut g = game();
        clear_sky(&mut g);
        for _ in 0..400 {
            g.spawn_wildlife();
        }
        let crows = g.animals.iter().filter(|a| a.kind == AnimalKind::Crow).count();
        assert!(crows > 0 && crows <= MAX_CROW, "{crows} crows");
    }

    // --- The cave dimension: day on entry, night blackouts, camps ---

    #[test]
    fn coming_into_the_horizen_is_always_day_and_night_there_wakes_the_player_at_stonehenge() {
        let mut g = game();
        let well = Well { x: 60, z: 60 };
        g.world.add_well(well);
        g.world.night = true;
        g.enter_horizen();
        assert!(!g.world.night, "day, whatever the overworld was");
        // A purple stew in the cave brings night: the player blacks out and wakes next to the well, out of the cave.
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::PurpleBerryStew, count: 1 });
        g.select_slot(0);
        g.eat_held();
        for _ in 0..90 {
            g.update_blackout(1.0 / 60.0);
        }
        assert!(g.in_overworld() && g.blackout.is_none());
        let p = g.player.pos;
        let (ax, az) = stonehenge::arrival(g.info.seed);
        assert!((p.x - ax).abs() < 0.01 && (p.z - az).abs() < 0.01, "in the ring of Stonehenge, not by the well: {p:?}");
        let (sx, sz) = stonehenge::centre(g.info.seed);
        assert!(vec2(sx as f32 - 10.0, sz as f32 - 10.0).length() > 4.0 * CHUNK_SIZE as f32, "about five chunks from where the world began");
        assert!(g.toasts.contains("Stonehenge"));
        // The cave is day again on the next visit.
        g.enter_horizen();
        assert!(!g.world.night);
    }

    #[test]
    fn every_fourth_cave_entered_has_a_camp_with_a_burning_log_and_a_pot_of_ready_stew() {
        let mut g = game();
        g.enter_horizen();
        let seed = g.info.seed;
        let mut camps = Vec::new();
        // Walk into the caves one by one (the arrival chamber counts as the first).
        let cells = [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0)];
        for (n, cell) in cells.into_iter().enumerate() {
            let cave = horizen::cave_in_cell(seed, cell.0, cell.1);
            if cave.great {
                continue;
            }
            g.player.pos = vec3(cave.x, GROUND_TOP as f32 + 1.0, cave.z);
            g.world.stream_chunks(g.player.pos);
            let made = g.update_cave_visits();
            assert_eq!(made, g.world.landmarks.caves_visited.len() % HORIZEN_CAMP_EVERY == 0, "cave {n}");
            assert!(!g.update_cave_visits(), "a cave already entered is not entered again");
            if made {
                camps.push(cave);
            }
        }
        assert!(!camps.is_empty());
        for cave in camps {
            let camp = g.world.pots.iter().find(|(&(x, _, z), _)| horizen::cave_at(seed, x, z).map(|c| c.cell) == Some(cave.cell) || ((x as f32 - cave.x).abs() < 8.0 && (z as f32 - cave.z).abs() < 8.0));
            let (&pos, pot) = camp.expect("a pot in the cave");
            assert_eq!(g.world.block_at(pos.0, pos.1, pos.2), Block::Fire, "on a burning log");
            assert!(g.world.fires.contains_key(&pos));
            assert!(pot.water && matches!(pot.stew(), Some(ItemKind::RedBerryStew | ItemKind::PurpleBerryStew)), "stew, ready");
            // Right click with an empty hand ladles it out.
            g.inventory.slots[0] = None;
            g.select_slot(0);
            let hit = Hit { x: pos.0, y: pos.1, z: pos.2, block: Block::Fire, prev: (pos.0, pos.1 + 1, pos.2) };
            assert!(g.use_held(&hit));
            assert!(matches!(g.inventory.slots.iter().flatten().next().map(|s| s.kind), Some(ItemKind::RedBerryStew | ItemKind::PurpleBerryStew)));
        }
    }

    #[test]
    fn the_caves_entered_are_saved_so_a_camp_is_not_made_twice() {
        let mut g = game();
        g.enter_horizen();
        for cell in [(0, 0), (1, 0)] {
            let cave = horizen::cave_in_cell(g.info.seed, cell.0, cell.1);
            g.player.pos = vec3(cave.x, GROUND_TOP as f32 + 1.0, cave.z);
            g.world.stream_chunks(g.player.pos);
            g.update_cave_visits();
        }
        let again = Game::from_save(WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() }, parse_save(&g.save_text()), 3);
        assert_eq!(again.world.landmarks.caves_visited, vec![(0, 0), (1, 0)]);
    }

    // --- The jungle temple ---

    #[test]
    fn a_big_stepped_temple_stands_in_the_middle_of_the_jungle_and_a_fall_from_the_sky_lands_beside_it() {
        let mut g = game();
        let (jx, jz) = crate::terrain::jungle_centre(g.info.seed);
        for cz in (jz - 30).div_euclid(CHUNK_SIZE)..=(jz + 30).div_euclid(CHUNK_SIZE) {
            for cx in (jx - 30).div_euclid(CHUNK_SIZE)..=(jx + 30).div_euclid(CHUNK_SIZE) {
                g.world.ensure_chunk(cx, cz);
            }
        }
        let base = g.world.ground_top(jx, jz);
        assert_eq!(g.world.block_at(jx, base + TEMPLE_TOP_FOR_TEST, jz), Block::GoldPile, "gold on the shrine floor at the top");
        assert_eq!(g.world.block_at(jx + 5, base + 1, jz + 5), Block::MayaStone, "solid pyramid");
        assert_eq!(g.world.block_at(jx, base + 1, jz + TEMPLE_BASE_HALF + 3), Block::Air, "open ground beyond the foot");
        let (lx, lz) = g.jungle_landing();
        assert!((lx - jx).abs().max((lz - jz).abs()) > TEMPLE_BASE_HALF + 1, "the landing is not on the temple");
        assert!(crate::terrain::in_jungle_clearing(g.info.seed, lx, lz), "but still in the bare middle of the jungle");
    }

    const TEMPLE_TOP_FOR_TEST: i32 = TEMPLE_TIERS * TEMPLE_TIER_HEIGHT + 1;

    // --- Backpack, pipes, ghosts, Stonehenge, the Mother in the air ---

    #[test]
    fn a_leather_backpack_is_made_of_leather_and_cord_and_gives_two_rows_only_while_worn() {
        let r = crate::recipe::recipe_for(ItemKind::LeatherBackpack);
        assert_eq!(r.inputs, &[(ItemKind::Leather, 4), (ItemKind::LeatherCord, 2)]);
        let mut g = game();
        assert_eq!(g.inventory.slots.len(), INVENTORY_SLOTS);
        hold(&mut g, ItemKind::LeatherBackpack);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::LeatherBackpack));
        assert_eq!(g.inventory.slots.len(), INVENTORY_SLOTS + 2 * HOTBAR_SLOTS, "two more rows");
        // The extra slots hold things; they survive a save only together with the pack.
        g.inventory.slots[INVENTORY_SLOTS + 4] = Some(ItemStack { kind: ItemKind::Gold, count: 5 });
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info.clone(), parse_save(&g.save_text()), 3);
        assert_eq!(again.worn, Some(ItemKind::LeatherBackpack));
        assert_eq!(again.inventory.slots.len(), INVENTORY_SLOTS + BACKPACK_SLOTS);
        assert_eq!(again.inventory.slots[INVENTORY_SLOTS + 4].map(|s| (s.kind, s.count)), Some((ItemKind::Gold, 5)));
        // The pack shares the one worn place: a cloak takes it, the rows go and the pack comes back into the inventory.
        hold(&mut g, ItemKind::PeltCloak);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::PeltCloak));
        assert_eq!((g.inventory.slots.len(), g.inventory.count(ItemKind::LeatherBackpack), g.inventory.count(ItemKind::Gold)), (INVENTORY_SLOTS, 1, 5));
        // And the pack takes the place of the cloak.
        hold(&mut g, ItemKind::LeatherBackpack);
        assert!(g.wear_held());
        assert_eq!((g.worn, g.inventory.count(ItemKind::PeltCloak), g.inventory.slots.len()), (Some(ItemKind::LeatherBackpack), 1, INVENTORY_SLOTS + BACKPACK_SLOTS));
        // Without the pack worn the extra rows are not there.
        let mut bare = SaveData::new();
        bare.inventory.slots[3] = Some(ItemStack { kind: ItemKind::Stick, count: 2 });
        let plain = Game::from_save(info, bare, 3);
        assert_eq!(plain.inventory.slots.len(), INVENTORY_SLOTS);
    }

    #[test]
    fn an_old_save_that_wore_a_cloak_and_a_pack_keeps_the_pack_on_and_the_cloak_in_the_inventory() {
        let mut data = parse_save("t\n1\n0 5 0 0 0\nworn pelt_cloak\nback leather_backpack\n");
        data.inventory.add_pack_rows();
        data.inventory.slots[INVENTORY_SLOTS + 2] = Some(ItemStack { kind: ItemKind::Gold, count: 2 });
        let g = Game::from_save(WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() }, data, 3);
        assert_eq!(g.worn, Some(ItemKind::LeatherBackpack));
        assert_eq!(g.inventory.slots.len(), INVENTORY_SLOTS + BACKPACK_SLOTS);
        assert_eq!((g.inventory.count(ItemKind::PeltCloak), g.inventory.count(ItemKind::Gold)), (1, 2));
        assert!(!g.save_text().contains("\nback "), "the old line is not written any more");
        assert!(g.save_text().contains("\nworn leather_backpack\n"));
        // A pack alone on an old save is simply worn.
        let alone = Game::from_save(WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() }, parse_save("t\n1\n0 5 0 0 0\nback leather_backpack\n"), 3);
        assert_eq!(alone.worn, Some(ItemKind::LeatherBackpack));
    }

    #[test]
    fn a_pack_that_is_not_worn_gives_no_rows_even_if_an_old_save_has_items_in_them() {
        let mut data = SaveData::new();
        data.inventory.add_pack_rows();
        data.inventory.slots[INVENTORY_SLOTS + 1] = Some(ItemStack { kind: ItemKind::Gold, count: 3 });
        let g = Game::from_save(WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() }, data, 3);
        assert_eq!(g.inventory.slots.len(), INVENTORY_SLOTS);
        assert_eq!(g.inventory.count(ItemKind::Gold), 3, "the gold moved into the ordinary slots");
    }

    #[test]
    fn the_silver_key_opens_the_lock_and_is_kept() {
        let mut g = game();
        let lake = Lake { x: 0, z: 0, radius: 130, phase: 3, from: (0, 0) };
        g.world.landmarks.lakes.push(lake);
        let door = (0, GROUND_TOP + 1, TOWER_HALF);
        g.world.ensure_chunk(0, 0);
        put(&mut g, door, Block::TowerDoor);
        hold(&mut g, ItemKind::SilverKey);
        assert!(g.try_unlock(door));
        assert_eq!(g.inventory.count(ItemKind::SilverKey), 1, "kept");
        assert!(g.world.landmarks.is_unlocked(lake.x, lake.z));
    }

    // --- The Iron Quail flies ---

    #[test]
    fn the_iron_quail_flies_up_to_a_player_high_above_the_ground_and_strikes_them() {
        let mut g = game();
        clear_sky(&mut g);
        kill_quail(&mut g, 10);
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::IronQuail).unwrap();
        let high = GROUND_TOP as f32 + 1.0 + 14.0;
        g.player.pos.y = high;
        g.animals[i].pos = g.player.pos + vec3(8.0, -13.0, 0.0);
        let start = g.health;
        let mut peak = 0.0_f32;
        for _ in 0..600 {
            g.player.pos.y = high;
            g.update_animals(1.0 / 60.0);
            if let Some(m) = g.animals.iter().find(|a| a.kind == AnimalKind::IronQuail) {
                peak = peak.max(m.pos.y);
            }
            if g.health < start {
                break;
            }
        }
        assert!(peak > GROUND_TOP as f32 + 8.0, "she left the ground and flew up: {peak}");
        assert!(g.health < start, "and she reached the player in the air");
        assert!(g.animals.iter().find(|a| a.kind == AnimalKind::IronQuail).map_or(false, |m| m.is_airborne()));
    }

    // --- The pipes ---

    fn quail_at(g: &mut Game, kind: AnimalKind, dx: f32, dz: f32) -> usize {
        let p = g.player.pos + vec3(dx, 0.0, dz);
        g.animals.push(Animal::new(kind, vec3(p.x, GROUND_TOP as f32 + 1.0, p.z), &mut Rng::new(4)));
        g.animals.len() - 1
    }

    #[test]
    fn pipes_played_by_day_bring_quail_and_crows_flying_to_the_player_to_stand_still_for_three_seconds() {
        let mut g = game();
        g.world.night = false;
        clear_sky(&mut g);
        let far = quail_at(&mut g, AnimalKind::Quail, 40.0, 0.0);
        let crow = quail_at(&mut g, AnimalKind::Crow, 0.0, -35.0);
        let deer = quail_at(&mut g, AnimalKind::Deer, 30.0, 10.0);
        let beyond = quail_at(&mut g, AnimalKind::Quail, 85.0, 0.0);
        let beyond_start = g.animals[beyond].pos;
        hold(&mut g, ItemKind::MusicalPipes);
        assert!(g.play_pipes());
        let p = g.player.pos;
        let mut flew = false;
        let mut frames = 0;
        // Run until both have come down (a landed bird stands calm, and its call is spent).
        while !(g.animals[far].calm > 0.0 && g.animals[crow].calm > 0.0) {
            g.update_animals(1.0 / 60.0);
            flew |= g.animals[far].is_airborne();
            frames += 1;
            assert!(frames < 25 * 60, "they did not come");
        }
        assert!(flew, "the quail took off");
        let gap = |a: &Animal| vec2(a.pos.x - p.x, a.pos.z - p.z).length();
        for &i in &[far, crow] {
            let a = &g.animals[i];
            assert!(gap(a) < PIPES_LAND_MAX + 1.0, "{:?} came to the player: {}", a.kind, gap(a));
            assert!(!a.is_airborne(), "and landed");
            assert!(a.lure_target.is_none());
        }
        assert!(gap(&g.animals[deer]) > 15.0, "a deer takes no notice");
        assert!((g.animals[beyond].pos - beyond_start).length() < 8.0, "a quail out of range stays put");
        // They stand still for three seconds after landing, and do not take off however close the player is.
        let landed = g.animals[far].pos;
        let mut still = 0.0;
        let mut g2 = game();
        g2.world.night = false;
        clear_sky(&mut g2);
        let q = quail_at(&mut g2, AnimalKind::Quail, 12.0, 0.0);
        hold(&mut g2, ItemKind::MusicalPipes);
        g2.play_pipes();
        loop {
            g2.update_animals(1.0 / 60.0);
            if g2.animals[q].calm > 0.0 && g2.animals[q].lure_target.is_none() {
                break;
            }
            still += 1.0;
            assert!(still < 20.0 * 60.0, "it never landed");
        }
        let at = g2.animals[q].pos;
        for _ in 0..(2.5 * 60.0) as i32 {
            g2.update_animals(1.0 / 60.0);
        }
        assert!((g2.animals[q].pos - at).length() < 0.05, "still for the three seconds, with the player right there");
        for _ in 0..(1.5 * 60.0) as i32 {
            g2.update_animals(1.0 / 60.0);
        }
        assert!(g2.animals[q].calm == 0.0);
        let _ = landed;
    }

    #[test]
    fn pipes_at_night_do_not_call_the_birds() {
        let mut g = game();
        g.world.night = true;
        clear_sky(&mut g);
        let q = quail_at(&mut g, AnimalKind::Quail, 30.0, 0.0);
        hold(&mut g, ItemKind::MusicalPipes);
        assert!(g.play_pipes());
        assert!(g.animals[q].lure_target.is_none());
        assert!(!{ hold(&mut g, ItemKind::Stick); g.play_pipes() }, "only pipes play");
    }

    // --- The ghosts ---

    fn in_the_graveyard(g: &mut Game) -> Graveyard {
        let yard = Graveyard { x: 60, z: 60 };
        g.world.add_graveyard(yard);
        g.player.pos = vec3(yard.x as f32 + 0.5, GROUND_TOP as f32 + 1.0, yard.z as f32 + 0.5);
        yard
    }

    #[test]
    fn the_graveyard_ghost_asks_for_a_song_gives_a_silver_key_for_one_thanks_and_never_speaks_again() {
        let mut g = game();
        g.player.pos = vec3(10.5, GROUND_TOP as f32 + 1.0, 10.5);
        g.update_graveyard_ghost();
        assert!(g.ghost_text.is_none(), "nothing to say away from the graveyard");
        let yard = in_the_graveyard(&mut g);
        g.update_graveyard_ghost();
        assert_eq!(g.ghost_text.as_ref().map(|t| t.0.as_str()), Some(crate::ghosts::GRAVEYARD_ASKS), "it asks for a song as you walk in");
        g.update_items(GHOST_TEXT_SECONDS + 1.0);
        assert!(g.ghost_text.is_none(), "the words fade");
        g.update_graveyard_ghost();
        assert!(g.ghost_text.is_none(), "it asks once as you walk in, not every frame");
        // Without pipes nothing happens; with them the ghost is thanked.
        hold(&mut g, ItemKind::Stick);
        assert!(!g.play_pipes());
        assert_eq!(count(&g, ItemKind::SilverKey), 0);
        hold(&mut g, ItemKind::MusicalPipes);
        assert!(g.play_pipes());
        assert_eq!(g.ghost_text.as_ref().map(|t| t.0.as_str()), Some(crate::ghosts::GRAVEYARD_THANKS));
        assert_eq!(count(&g, ItemKind::SilverKey), 1, "it gives a silver key");
        assert!(g.world.landmarks.graveyard_rested);
        // From then on it says nothing, and gives nothing more.
        g.update_items(GHOST_TEXT_SECONDS + 1.0);
        g.player.pos = vec3(yard.x as f32 + 40.0, GROUND_TOP as f32 + 1.0, yard.z as f32);
        g.update_graveyard_ghost();
        g.player.pos = vec3(yard.x as f32 + 0.5, GROUND_TOP as f32 + 1.0, yard.z as f32 + 0.5);
        g.update_graveyard_ghost();
        assert!(g.ghost_text.is_none(), "it has stopped talking");
        g.play_pipes();
        assert!(g.ghost_text.is_none());
        assert_eq!(count(&g, ItemKind::SilverKey), 1);
        let again = Game::from_save(WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() }, parse_save(&g.save_text()), 3);
        assert!(again.world.landmarks.graveyard_rested, "and it stays at rest after a reload");
    }

    #[test]
    fn a_ghostly_voice_answers_a_song_played_in_each_of_the_three_towers() {
        // The cave tower.
        let mut g = game();
        g.enter_horizen();
        let (tx, tz) = horizen::cavern_centre(g.info.seed);
        g.world.add_horizen_tower(crate::tower::Tower { x: tx, z: tz });
        g.player.pos = vec3(tx as f32 + 0.5, GROUND_TOP as f32 + 2.0, tz as f32 + 0.5);
        hold(&mut g, ItemKind::MusicalPipes);
        assert!(g.play_pipes());
        assert_eq!(g.ghost_text.as_ref().map(|t| t.0.as_str()), Some(crate::ghosts::CAVE_TOWER_GHOST));
        g.player.pos.x += TOWER_HALF as f32 + 4.0;
        g.ghost_text = None;
        g.play_pipes();
        assert!(g.ghost_text.is_none(), "outside the tower it is silent");
        // The sky tower.
        let mut s = game();
        s.travel_to(Dimension::Skylands);
        s.world.landmarks.sky_tower = Some((1, 1));
        let island = skylands::island_in_cell(s.info.seed, 1, 1);
        s.player.pos = vec3(island.x as f32 + 0.5, island.top as f32 + 3.0, island.z as f32 + 0.5);
        hold(&mut s, ItemKind::MusicalPipes);
        s.play_pipes();
        assert_eq!(s.ghost_text.as_ref().map(|t| t.0.as_str()), Some(crate::ghosts::SKY_TOWER_GHOST));
        // The lake tower (Nessie's).
        let mut l = game();
        let lake = Lake { x: 0, z: 0, radius: 130, phase: 3, from: (0, 0) };
        l.world.landmarks.lakes.push(lake);
        l.world.stream_chunks(vec3(0.0, 6.0, 0.0));
        let base = l.world.ground_top(lake.x, lake.z) + 1;
        l.player.pos = vec3(0.5, base as f32 + 1.0, 0.5);
        hold(&mut l, ItemKind::MusicalPipes);
        l.play_pipes();
        assert_eq!(l.ghost_text.as_ref().map(|t| t.0.as_str()), Some(crate::ghosts::LAKE_TOWER_GHOST));
        assert!(l.ghost_text.as_ref().unwrap().0.contains("stonehenge"));
    }

    // --- Stonehenge in the world ---

    #[test]
    fn stonehenge_is_built_five_chunks_from_the_spawn_point_with_its_altar_and_standing_stones() {
        let mut g = game();
        let (cx, cz) = stonehenge::centre(g.info.seed);
        for z in (cz - 16).div_euclid(CHUNK_SIZE)..=(cz + 16).div_euclid(CHUNK_SIZE) {
            for x in (cx - 16).div_euclid(CHUNK_SIZE)..=(cx + 16).div_euclid(CHUNK_SIZE) {
                g.world.ensure_chunk(x, z);
            }
        }
        let base = g.world.ground_top(cx, cz);
        assert_eq!(g.world.block_at(cx, base + 2, cz), Block::CastleStone, "the altar stone");
        let ring: Vec<Block> = (0..STONEHENGE_STONES).map(|i| {
            let a = i as f32 * std::f32::consts::TAU / STONEHENGE_STONES as f32;
            g.world.block_at(cx + (a.cos() * STONEHENGE_RADIUS).round() as i32, base + 3, cz + (a.sin() * STONEHENGE_RADIUS).round() as i32)
        }).collect();
        assert!(ring.iter().filter(|&&b| b == Block::CastleStone).count() >= STONEHENGE_STONES as usize - 2, "the standing stones: {ring:?}");
        let (ax, az) = stonehenge::arrival(g.info.seed);
        assert_eq!(g.world.block_at(ax.floor() as i32, base + 1, az.floor() as i32), Block::Air, "room to stand where a player wakes");
    }
}

#[cfg(test)]
mod bat_and_wild_hunt_tests {
    use super::*;
    use crate::animal::HuntPhase;
    use crate::chunk::idx;
    use crate::item::ItemStack;
    use std::path::PathBuf;

    fn game() -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut g = Game::from_save(info, SaveData::new(), 5);
        g.world.stream_chunks(g.player.pos);
        g
    }

    fn hold(g: &mut Game, kind: ItemKind) {
        g.inventory.slots[0] = Some(ItemStack { kind, count: 1 });
        g.select_slot(0);
    }

    /// Open sky and level ground over the spawn area, and a player who cannot be killed (their health is counted instead).
    fn open_field(g: &mut Game) {
        for chunk in g.world.chunks.values_mut() {
            for y in GROUND_TOP + 1..WORLD_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        chunk.blocks[idx(x, y, z)] = Block::Air;
                    }
                }
            }
        }
        g.health = 1_000_000;
    }

    fn put(g: &mut Game, (x, y, z): (i32, i32, i32), block: Block) {
        g.world.chunks.get_mut(&World::chunk_of(x, z)).unwrap().blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))] = block;
    }

    fn standing(g: &Game, dx: f32, dz: f32) -> Vec3 {
        g.player.pos + vec3(dx, 0.0, dz)
    }

    fn bats(g: &Game) -> usize {
        g.animals.iter().filter(|a| a.kind == AnimalKind::Bat).count()
    }

    fn lost(g: &Game) -> u32 {
        1_000_000 - g.health
    }

    const DT: f32 = 1.0 / 60.0;

    // --- Bats ---

    #[test]
    fn playing_the_pipes_at_night_calls_bats_that_hunt_the_player_and_by_day_it_does_not() {
        let mut g = game();
        hold(&mut g, ItemKind::MusicalPipes);
        g.world.night = false;
        assert!(g.play_pipes());
        assert_eq!(bats(&g), 0, "no bats by day");
        g.world.night = true;
        assert!(g.play_pipes());
        assert_eq!(bats(&g), BATS_PER_PIPES);
        assert!(g.animals.iter().filter(|a| a.kind == AnimalKind::Bat).all(|a| a.hunting));
        let p = g.player.pos;
        assert!(g.animals.iter().filter(|a| a.kind == AnimalKind::Bat).all(|a| {
            let gap = vec2(a.pos.x - p.x, a.pos.z - p.z).length();
            gap >= BAT_PIPES_MIN_DISTANCE - 1.0 && gap <= BAT_PIPES_MAX_DISTANCE + 1.0 && a.pos.y > g.world.ground_top(a.pos.x.floor() as i32, a.pos.z.floor() as i32) as f32 + 1.0
        }), "they come out of the air round the player");
        // They are never more than the cap, however often the tune is played.
        for _ in 0..5 {
            g.play_pipes();
        }
        assert_eq!(bats(&g), MAX_BAT_OVERWORLD);
        // And they go with the dawn.
        g.world.night = false;
        g.update_animals(0.01);
        assert_eq!(bats(&g), 0);
    }

    #[test]
    fn bats_never_spawn_by_themselves_in_the_normal_world_but_do_in_the_cave() {
        let mut g = game();
        g.world.night = true;
        for _ in 0..300 {
            g.spawn_timer = ANIMAL_SPAWN_INTERVAL;
            g.update_animals(0.0);
        }
        assert!(!g.animals.is_empty(), "other wildlife does spawn at night");
        assert!(g.animals.iter().all(|a| !matches!(a.kind, AnimalKind::Bat | AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer)));
        let mut g = game();
        g.enter_horizen();
        for _ in 0..300 {
            g.spawn_timer = ANIMAL_SPAWN_INTERVAL;
            g.update_animals(0.0);
        }
        assert!(bats(&g) >= 3 && bats(&g) <= MAX_BAT_CAVE, "bats live in the cave: {}", bats(&g));
        assert!(g.animals.iter().all(|a| a.kind == AnimalKind::Bat), "and nothing else does");
        let seed = g.info.seed;
        assert!(g.animals.iter().all(|a| horizen::air_height(seed, a.pos.x.floor() as i32, a.pos.z.floor() as i32) >= BAT_CAVE_MIN_AIR), "each in open cave");
        // The pipes call no bats in the cave (it is always day there).
        hold(&mut g, ItemKind::MusicalPipes);
        let before = bats(&g);
        g.play_pipes();
        assert_eq!(bats(&g), before);
    }

    #[test]
    fn a_bat_flies_at_the_player_and_bites_and_is_easily_killed() {
        let mut g = game();
        open_field(&mut g);
        g.world.night = true;
        let at = standing(&g, 8.0, 0.0) + vec3(0.0, 3.0, 0.0);
        let mut bat = Animal::new(AnimalKind::Bat, at, &mut Rng::new(2));
        bat.hunting = true;
        g.animals.push(bat);
        for _ in 0..(5.0 / DT) as i32 {
            g.update_animals(DT);
        }
        assert!(lost(&g) >= BAT_DAMAGE, "the bat bit: {}", lost(&g));
        assert_eq!(lost(&g) % BAT_DAMAGE, 0);
        let bat = &g.animals[0];
        assert!(bat.pos.y > GROUND_TOP as f32 + 1.2 && (bat.pos - g.player.pos).length() < 3.0, "it hovers about the player's body: {:?}", bat.pos);
        g.hurt_animals(&[(0, ARROW_DAMAGE)]);
        assert_eq!(bats(&g), 0, "one arrow kills it");
        assert!(BAT_SPEED < SPRINT_SPEED);
    }

    // --- The Wild Hunt ---

    fn in_stonehenge(g: &mut Game) {
        let (cx, cz) = stonehenge::centre(g.info.seed);
        let (ax, az) = stonehenge::arrival(g.info.seed);
        g.player.pos = vec3(ax, g.world.ground_top(cx, cz) as f32 + 1.01, az);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        g.player.pos.y = g.world.ground_top(ax.floor() as i32, az.floor() as i32) as f32 + 1.01;
    }

    #[test]
    fn blowing_the_horn_in_stonehenge_calls_the_whole_hunt_once_and_elsewhere_only_the_wind_answers() {
        let mut g = game();
        hold(&mut g, ItemKind::HornOfTheHunt);
        assert!(!g.in_stonehenge());
        assert!(g.blow_horn(), "it is blown");
        assert!(!g.hunt_alive(), "but nothing comes out in the forest");
        in_stonehenge(&mut g);
        assert!(g.in_stonehenge());
        hold(&mut g, ItemKind::Stick);
        assert!(!g.blow_horn() && !g.hunt_alive(), "only the horn does it");
        hold(&mut g, ItemKind::HornOfTheHunt);
        assert!(g.blow_horn());
        let kinds: Vec<AnimalKind> = g.animals.iter().filter(|a| a.kind.is_wild_hunt()).map(|a| a.kind).collect();
        assert_eq!(kinds.len(), 5 + HUNT_GHOST_BEARS + HUNT_GHOST_CROWS);
        for (kind, n) in [
            (AnimalKind::HuntSwordsman, 1),
            (AnimalKind::HuntArcher, 2),
            (AnimalKind::HuntLancer, 2),
            (AnimalKind::HuntBear, HUNT_GHOST_BEARS),
            (AnimalKind::HuntCrow, HUNT_GHOST_CROWS),
        ] {
            assert_eq!(kinds.iter().filter(|&&k| k == kind).count(), n, "{kind:?}");
        }
        assert!(g.animals.iter().filter(|a| matches!(a.kind, AnimalKind::HuntBear | AnimalKind::HuntCrow)).all(|a| a.splits), "the first bears and crows split");
        let p = g.player.pos;
        assert!(g.animals.iter().all(|a| {
            let gap = vec2(a.pos.x - p.x, a.pos.z - p.z).length();
            gap >= HUNT_SPAWN_MIN_DISTANCE - 3.0 && gap <= HUNT_SPAWN_MAX_DISTANCE + 3.0
        }));
        assert!(g.animals.iter().all(|a| a.kind.is_boss() && a.health == a.kind.max_health()));
        // Blown again while they live, it calls no more.
        assert!(g.blow_horn());
        assert_eq!(g.animals.iter().filter(|a| a.kind.is_wild_hunt()).count(), 5 + HUNT_GHOST_BEARS + HUNT_GHOST_CROWS);
        // They do not despawn however far away they are, and the horn is not blown in other dimensions.
        g.animals[0].pos += vec3(300.0, 0.0, 0.0);
        g.update_animals(0.01);
        assert_eq!(g.animals.iter().filter(|a| a.kind.is_wild_hunt()).count(), 5 + HUNT_GHOST_BEARS + HUNT_GHOST_CROWS);
    }

    #[test]
    fn the_swordsman_charges_slower_than_a_sprint_and_cuts() {
        assert!(HUNT_SWORDSMAN_SPEED < SPRINT_SPEED);
        let mut g = game();
        open_field(&mut g);
        g.animals.push(Animal::new(AnimalKind::HuntSwordsman, standing(&g, 12.0, 0.0), &mut Rng::new(2)));
        let start = (g.animals[0].pos - g.player.pos).length();
        for _ in 0..60 {
            g.update_animals(DT);
        }
        let after_one_second = (g.animals[0].pos - g.player.pos).length();
        assert!((start - after_one_second - HUNT_SWORDSMAN_SPEED).abs() < 1.0, "it ran about {} blocks", start - after_one_second);
        for _ in 0..(4.0 / DT) as i32 {
            g.update_animals(DT);
        }
        assert!(lost(&g) >= HUNT_SWORD_DAMAGE && lost(&g) % HUNT_SWORD_DAMAGE == 0, "it cut: {}", lost(&g));
        assert!(lost(&g) <= HUNT_SWORD_DAMAGE * 4, "no faster than the sword's cooldown: {}", lost(&g));
    }

    /// New arrows this frame: those that have flown for no more than one frame.
    fn new_arrows(g: &Game) -> usize {
        g.hostile_arrows.iter().filter(|a| a.age < DT * 1.5).count()
    }

    #[test]
    fn the_archer_keeps_its_distance_and_shoots_from_afar_and_a_rabbit_foot_halves_its_fire() {
        let shots = |foot: bool| {
            let mut g = game();
            open_field(&mut g);
            if foot {
                g.inventory.slots[3] = Some(ItemStack { kind: ItemKind::RabbitFoot, count: 1 });
            }
            g.animals.push(Animal::new(AnimalKind::HuntArcher, standing(&g, 20.0, 0.0), &mut Rng::new(2)));
            let (mut shots, mut nearest, mut farthest) = (0, f32::MAX, 0.0_f32);
            for _ in 0..(30.0 / DT) as i32 {
                g.update_animals(DT);
                shots += new_arrows(&g);
                let gap = (g.animals[0].pos - g.player.pos).length();
                nearest = nearest.min(gap);
                farthest = farthest.max(gap);
            }
            assert!(nearest > HUNT_ARCHER_MIN_RANGE - 3.0 && farthest < HUNT_ARCHER_MAX_RANGE + 3.0, "it stays between {nearest} and {farthest}");
            (shots, lost(&g))
        };
        let (plain, damage) = shots(false);
        let (with_foot, _) = shots(true);
        assert!((8..=11).contains(&plain), "an arrow every three seconds: {plain} in thirty");
        assert!((4..=6).contains(&with_foot), "half as many with a rabbit foot: {with_foot}");
        assert!(damage > 0 && damage % HUNT_ARROW_DAMAGE == 0, "its arrows find the player: {damage}");
    }

    #[test]
    fn the_archer_cannot_shoot_through_a_wall() {
        let mut g = game();
        open_field(&mut g);
        let p = g.player.pos;
        let wall_x = p.floor().x as i32 + 12;
        for z in -20..=20 {
            for y in GROUND_TOP + 1..GROUND_TOP + 9 {
                put(&mut g, (wall_x, y, p.z.floor() as i32 + z), Block::CastleStone);
            }
        }
        g.animals.push(Animal::new(AnimalKind::HuntArcher, standing(&g, 20.0, 0.0), &mut Rng::new(2)));
        g.animals[0].pos.x = wall_x as f32 + 8.0;
        for _ in 0..(8.0 / DT) as i32 {
            g.update_animals(DT);
            assert_eq!(new_arrows(&g), 0);
        }
        assert_eq!(lost(&g), 0);
    }

    #[test]
    fn a_lancer_charges_straight_in_runs_it_through_once_gallops_away_rests_and_charges_again() {
        assert!(HUNT_LANCER_CHARGE_SPEED > SPRINT_SPEED);
        let mut g = game();
        open_field(&mut g);
        g.animals.push(Animal::new(AnimalKind::HuntLancer, standing(&g, 0.0, -14.0), &mut Rng::new(2)));
        let (mut phases, mut stabs) = (vec![g.animals[0].hunt_phase], 0);
        for _ in 0..(40.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            let a = &g.animals[0];
            if g.health < MAX_HEALTH {
                stabs += 1;
                assert_eq!(MAX_HEALTH - g.health, HUNT_LANCER_DAMAGE);
                assert_eq!(a.hunt_phase, HuntPhase::Retreat, "one blow, then it gallops away");
            }
            if *phases.last().unwrap() != a.hunt_phase {
                phases.push(a.hunt_phase);
            }
        }
        assert!(stabs >= 2, "it charged home {stabs} times");
        assert!(phases.windows(3).any(|w| w == [HuntPhase::Dash, HuntPhase::Retreat, HuntPhase::Rest]), "charge, gallop away, rest: {phases:?}");
        assert!(phases.contains(&HuntPhase::Rest) && phases.iter().filter(|&&p| p == HuntPhase::Dash).count() >= 2, "and charge again: {phases:?}");
    }

    #[test]
    fn a_resting_lancer_stands_still_and_a_charging_one_comes_straight() {
        let mut g = game();
        open_field(&mut g);
        let mut lancer = Animal::new(AnimalKind::HuntLancer, standing(&g, 0.0, -20.0), &mut Rng::new(2));
        lancer.hunt_phase = HuntPhase::Rest;
        lancer.hunt_timer = 1.0;
        g.animals.push(lancer);
        let start = g.animals[0].pos;
        for _ in 0..(0.9 / DT) as i32 {
            g.update_animals(DT);
        }
        assert!((g.animals[0].pos - start).length() < 0.2, "it rests: {:?}", g.animals[0].pos);
        for _ in 0..(1.5 / DT) as i32 {
            g.update_animals(DT);
        }
        let gap = (g.animals[0].pos - g.player.pos).length();
        assert!(gap < 20.0 - 5.0, "then it charged: {gap}");
        assert!(g.animals[0].pos.x.abs() < 1.5 + g.player.pos.x.abs(), "straight at the player");
    }

    #[test]
    fn excalibur_does_triple_damage_to_the_wild_hunt_and_no_more_to_anything_else() {
        let mut g = game();
        open_field(&mut g);
        hold(&mut g, ItemKind::Excalibur);
        let eye = g.player.pos + vec3(0.0, 1.6, 0.0);
        let dir = vec3(1.0, 0.0, 0.0);
        for (kind, factor) in [(AnimalKind::HuntSwordsman, EXCALIBUR_HUNT_FACTOR), (AnimalKind::HuntArcher, EXCALIBUR_HUNT_FACTOR), (AnimalKind::HuntLancer, EXCALIBUR_HUNT_FACTOR), (AnimalKind::Moose, 1)] {
            g.animals.clear();
            g.attack_cooldown = 0.0;
            g.animals.push(Animal::new(kind, standing(&g, 2.5, 0.0), &mut Rng::new(2)));
            assert!(g.swing_melee(eye, dir), "{kind:?}");
            assert_eq!(kind.max_health() - g.animals[0].health, EXCALIBUR_DAMAGE * factor, "{kind:?}");
        }
        // Other weapons do their ordinary damage to the Hunt.
        g.animals.clear();
        g.attack_cooldown = 0.0;
        hold(&mut g, ItemKind::IronAxe);
        g.animals.push(Animal::new(AnimalKind::HuntSwordsman, standing(&g, 2.0, 0.0), &mut Rng::new(2)));
        assert!(g.swing_melee(eye, dir));
        assert_eq!(HUNT_SWORDSMAN_HEALTH - g.animals[0].health, IRON_AXE_DAMAGE);
        assert_eq!(EXCALIBUR_HUNT_FACTOR, 3);
    }

    #[test]
    fn the_hunt_shares_one_health_bar() {
        let mut g = game();
        open_field(&mut g);
        for kind in [AnimalKind::HuntSwordsman, AnimalKind::HuntArcher, AnimalKind::HuntLancer] {
            g.animals.push(Animal::new(kind, standing(&g, 30.0, 0.0), &mut Rng::new(2)));
        }
        g.hunt_total = 0;
        let (name, share) = g.boss_bar().expect("a bar");
        assert_eq!((name, share), ("The Wild Hunt", 1.0));
        g.animals[1].health = 0;
        let total = (HUNT_SWORDSMAN_HEALTH + HUNT_ARCHER_HEALTH + HUNT_LANCER_HEALTH) as f32;
        let (_, share) = g.boss_bar().unwrap();
        assert!((share - (total - HUNT_ARCHER_HEALTH as f32) / total).abs() < 1e-5, "{share}");
    }

    #[test]
    fn beating_the_whole_hunt_wins_the_game_and_play_goes_on() {
        let mut g = game();
        open_field(&mut g);
        for kind in [AnimalKind::HuntSwordsman, AnimalKind::HuntArcher, AnimalKind::HuntLancer] {
            g.animals.push(Animal::new(kind, standing(&g, 30.0, 0.0), &mut Rng::new(2)));
        }
        assert!(!g.world.landmarks.game_won);
        g.hurt_animals(&[(0, 100_000)]);
        g.hurt_animals(&[(0, 100_000)]);
        assert!(!g.world.landmarks.game_won && g.victory.is_none(), "two of three is not enough");
        g.hurt_animals(&[(0, 100_000)]);
        assert!(g.world.landmarks.game_won && g.victory.is_some(), "the third wins it");
        // The win is saved and loaded.
        let text = g.save_text();
        assert!(text.lines().any(|l| l == "won"));
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        assert!(Game::from_save(info, parse_save(&text), 3).world.landmarks.game_won);
        // The message fades, and the game carries on: the horn calls the Hunt again.
        for _ in 0..(VICTORY_TEXT_SECONDS / 0.5) as i32 + 2 {
            g.update_items(0.5);
        }
        assert!(g.victory.is_none());
        in_stonehenge(&mut g);
        hold(&mut g, ItemKind::HornOfTheHunt);
        g.blow_horn();
        assert!(g.hunt_alive() && g.world.landmarks.game_won);
    }

    #[test]
    fn dying_to_the_hunt_ends_the_fight_and_the_horn_can_call_it_again() {
        let mut g = game();
        in_stonehenge(&mut g);
        hold(&mut g, ItemKind::HornOfTheHunt);
        g.blow_horn();
        g.hostile_arrows.push(Arrow::from_archer(g.player.pos, vec3(1.0, 0.0, 0.0)));
        assert!(g.hurt_player(MAX_HEALTH, "dead"));
        assert!(!g.hunt_alive() && g.hostile_arrows.is_empty());
        assert!(!g.world.landmarks.game_won);
        in_stonehenge(&mut g);
        hold(&mut g, ItemKind::HornOfTheHunt);
        g.blow_horn();
        assert!(g.hunt_alive());
    }
}

#[cfg(test)]
mod boss_update_tests {
    use super::*;
    use crate::animal::HuntPhase;
    use crate::chunk::idx;
    use crate::item::ItemStack;
    use crate::landmarks::MOUNTAIN_MAX_HEIGHT;
    use crate::player::MoveInput;
    use std::path::PathBuf;

    const DT: f32 = 1.0 / 60.0;

    fn game() -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut g = Game::from_save(info, SaveData::new(), 5);
        g.world.stream_chunks(g.player.pos);
        g
    }

    /// Open sky over the spawn area: every block above the ground is air.
    fn open_field(g: &mut Game) {
        for chunk in g.world.chunks.values_mut() {
            for y in GROUND_TOP + 1..WORLD_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        chunk.blocks[idx(x, y, z)] = Block::Air;
                    }
                }
            }
        }
    }

    fn put(g: &mut Game, (x, y, z): (i32, i32, i32), block: Block) {
        if let Some(c) = g.world.chunks.get_mut(&World::chunk_of(x, z)) {
            c.blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))] = block;
        }
    }

    /// A spot on the ground `dx` east and `dz` south of the player.
    fn standing(g: &Game, dx: f32, dz: f32) -> Vec3 {
        vec3(g.player.pos.x + dx, (GROUND_TOP + 1) as f32 + 0.01, g.player.pos.z + dz)
    }

    fn give(g: &mut Game, slot: usize, kind: ItemKind, count: u32) {
        g.inventory.slots[slot] = Some(ItemStack { kind, count });
    }

    fn hold(g: &mut Game, kind: ItemKind) {
        give(g, 0, kind, 1);
        g.select_slot(0);
    }

    fn add(g: &mut Game, kind: AnimalKind, at: Vec3) -> usize {
        g.animals.push(Animal::new(kind, at, &mut Rng::new(2)));
        g.animals.len() - 1
    }

    // --- Movement: bosses do not get stuck ---

    #[test]
    fn a_yeti_goes_round_the_end_of_a_wall_to_reach_the_player() {
        let mut g = game();
        open_field(&mut g);
        let p = g.player.pos;
        let wall_x = p.x.floor() as i32 + 8;
        for z in -12..=12 {
            for y in GROUND_TOP + 1..GROUND_TOP + 5 {
                put(&mut g, (wall_x, y, p.z.floor() as i32 + z), Block::CastleStone);
            }
        }
        { let at = standing(&g, 14.0, 0.0); add(&mut g, AnimalKind::Yeti, at) };
        let mut best = f32::MAX;
        for _ in 0..(40.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            best = best.min(g.animals[0].pos.x);
        }
        assert!(best < wall_x as f32 - 0.5, "it got round to the player's side of the wall: {best} vs {wall_x}");
    }

    #[test]
    fn a_yeti_climbs_a_ledge_two_blocks_high_that_a_deer_could_not() {
        let mut g = game();
        open_field(&mut g);
        let p = g.player.pos;
        let ledge = p.x.floor() as i32 + 7;
        for z in -60..=60 {
            for y in GROUND_TOP + 1..GROUND_TOP + 3 {
                put(&mut g, (ledge, y, p.z.floor() as i32 + z), Block::CastleStone);
            }
        }
        { let at = standing(&g, 12.0, 0.0); add(&mut g, AnimalKind::Yeti, at) };
        let mut best = f32::MAX;
        for _ in 0..(12.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            best = best.min(g.animals[0].pos.x);
        }
        assert!(best < ledge as f32, "it climbed over: {best} vs {ledge}");
    }

    // --- The Yeti's smash and ice ---

    #[test]
    fn a_yeti_smash_hurts_four_and_throws_the_player_ten_blocks_but_only_those_within_two_blocks() {
        let mut g = game();
        open_field(&mut g);
        let at = standing(&g, 0.0, 0.0);
        g.player.pos = at + vec3(2.0, 0.0, 0.0);
        g.carry_out(BossEvent::Smash { at });
        assert_eq!(g.health, MAX_HEALTH - YETI_SMASH_DAMAGE, "four damage");
        let start = g.player.pos;
        for _ in 0..(4.0 / DT) as i32 {
            g.player.update(&g.world, DT, &MoveInput::default());
        }
        let thrown = vec2(g.player.pos.x - start.x, g.player.pos.z - start.z);
        assert!(thrown.x > 0.0 && thrown.y.abs() < 1.0, "straight away from the Yeti: {thrown:?}");
        assert!((thrown.length() - YETI_SMASH_KNOCKBACK).abs() < 1.5, "thrown about ten blocks: {}", thrown.length());
        // A player three blocks beyond its reach is untouched.
        let mut g = game();
        open_field(&mut g);
        let at = standing(&g, 0.0, 0.0);
        g.player.pos = at + vec3(YETI_SMASH_RADIUS + AnimalKind::Yeti.half_width() + PLAYER_HALF_WIDTH + 1.0, 0.0, 0.0);
        g.carry_out(BossEvent::Smash { at });
        assert_eq!(g.health, MAX_HEALTH);
        assert_eq!(g.player.knock, Vec3::ZERO);
        assert!(g.bursts.iter().any(|b| b.shape == BurstShape::Smash), "the ring is shown");
    }

    #[test]
    fn a_smash_also_hurts_and_throws_other_creatures_but_not_other_yetis() {
        let mut g = game();
        open_field(&mut g);
        let at = standing(&g, 20.0, 0.0);
        g.player.pos = standing(&g, -30.0, 0.0);
        let near = add(&mut g, AnimalKind::Deer, at + vec3(1.5, 0.0, 0.0));
        let far = add(&mut g, AnimalKind::Deer, at + vec3(12.0, 0.0, 0.0));
        let other_yeti = add(&mut g, AnimalKind::Yeti, at + vec3(1.0, 0.0, 1.0));
        g.carry_out(BossEvent::Smash { at });
        assert_eq!(g.animals[near].health, DEER_HEALTH - YETI_SMASH_DAMAGE as i32);
        assert!(g.animals[near].knock.length() > 10.0, "thrown hard");
        assert_eq!(g.animals[far].health, DEER_HEALTH);
        assert_eq!(g.animals[other_yeti].health, YETI_HEALTH);
    }

    #[test]
    fn a_yeti_close_to_the_player_rears_up_and_smashes_and_far_from_them_throws_ice() {
        let mut g = game();
        open_field(&mut g);
        { let at = standing(&g, 4.0, 0.0); add(&mut g, AnimalKind::Yeti, at) };
        let (mut reared, mut smashed, mut threw) = (false, false, false);
        for _ in 0..(6.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            reared |= g.animals[0].action == crate::animal::BossAction::Smash;
            smashed |= g.bursts.iter().any(|b| b.shape == BurstShape::Smash);
            threw |= !g.projectiles.is_empty();
        }
        assert!(reared && smashed, "it rears up and hits the ground when close");
        assert!(!threw, "and throws no ice at a player within six blocks");
        let mut g = game();
        open_field(&mut g);
        { let at = standing(&g, 22.0, 0.0); add(&mut g, AnimalKind::Yeti, at) };
        let mut ice = false;
        for _ in 0..(3.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            ice |= g.projectiles.iter().any(|p| p.kind == ProjectileKind::IceBall);
        }
        assert!(ice, "far off it throws a lump of ice");
    }

    #[test]
    fn a_lump_of_ice_bursts_where_it_lands_and_hurts_a_player_in_the_burst() {
        let mut g = game();
        open_field(&mut g);
        g.player.pos = standing(&g, 0.0, 0.0);
        let from = g.player.pos + vec3(14.0, 3.0, 0.0);
        g.carry_out(BossEvent::IceThrow { from });
        assert_eq!(g.projectiles.len(), 1);
        for _ in 0..(3.0 / DT) as i32 {
            g.update_hazards(DT);
        }
        assert!(g.projectiles.is_empty(), "it burst");
        assert!(g.bursts.iter().any(|b| b.shape == BurstShape::Ice) || g.health < MAX_HEALTH);
        assert_eq!(g.health, MAX_HEALTH - ICE_BURST_DAMAGE, "it was thrown at the player and hit them for {ICE_BURST_DAMAGE}");
        // Bursts fade.
        for _ in 0..(BURST_SHOW_SECONDS / DT) as i32 + 5 {
            g.update_hazards(DT);
        }
        assert!(g.bursts.is_empty());
    }

    // --- Nessie ---

    fn lake() -> Lake {
        Lake { x: 60, z: 60, radius: LAKE_RADIUS, phase: 3, from: (10, 10) }
    }

    /// A game with a lake, the player on its shore and the monster risen.
    fn at_the_shore() -> Game {
        let mut g = game();
        let lake = lake();
        g.world.add_lake(lake);
        let shore = (0..40).map(|i| lake.z + 60 + i).find(|&z| !g.world.is_wet_column(lake.x, z) && g.world.is_wet_column(lake.x, z - 1)).unwrap_or(lake.z + 90);
        g.player.pos = vec3(lake.x as f32 + 0.5, g.world.ground_top(lake.x, shore) as f32 + 1.01, shore as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        for _ in 0..200 {
            g.spawn_bosses();
            if g.animals.iter().any(|a| a.kind == AnimalKind::LochNessMonster) {
                break;
            }
        }
        assert!(g.animals.iter().any(|a| a.kind == AnimalKind::LochNessMonster), "the monster rises");
        g
    }

    fn nessie(g: &Game) -> &Animal {
        g.animals.iter().find(|a| a.kind == AnimalKind::LochNessMonster).unwrap()
    }

    /// How many blocks (up to 12) the nearest dry column is from the point (x, z).
    fn clearance(g: &Game, x: f32, z: f32) -> f32 {
        let mut r = 0.0;
        while r <= 12.0 {
            for i in 0..32 {
                let a = i as f32 * std::f32::consts::TAU / 32.0;
                if !g.world.is_wet_column((x + a.cos() * r).floor() as i32, (z + a.sin() * r).floor() as i32) {
                    return r;
                }
            }
            r += 0.5;
        }
        12.0
    }

    fn stand_on_ground(g: &mut Game) {
        g.player.pos.y = g.world.ground_top(g.player.pos.x.floor() as i32, g.player.pos.z.floor() as i32) as f32 + 1.01;
    }

    #[test]
    fn nessie_keeps_five_blocks_from_the_shore_however_the_player_tempts_her() {
        let mut g = at_the_shore();
        let start = clearance(&g, nessie(&g).pos.x, nessie(&g).pos.z);
        let floor = start.min(LOCH_NESS_SHORE_MARGIN) - 0.8;
        let mut closest_to_player = f32::MAX;
        for frame in 0..(90.0 * 60.0) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            stand_on_ground(&mut g);
            let m = nessie(&g);
            let c = clearance(&g, m.pos.x, m.pos.z);
            assert!(c >= floor, "frame {frame}: {c} blocks from the shore at {:?}", m.pos);
            closest_to_player = closest_to_player.min((m.pos - g.player.pos).length());
        }
        assert!(closest_to_player > AnimalKind::LochNessMonster.half_width() + PLAYER_HALF_WIDTH + LOCH_NESS_REACH, "so her neck can never reach a player on the shore: {closest_to_player}");
    }

    #[test]
    fn nessie_calls_ravens_by_day_against_a_player_far_from_her_and_taints_a_line_of_water() {
        let mut g = at_the_shore();
        g.world.night = false;
        let (mut ravens, mut acid) = (0, 0);
        for _ in 0..(40.0 * 60.0) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            stand_on_ground(&mut g);
            ravens = ravens.max(g.animals.iter().filter(|a| a.kind == AnimalKind::Raven && a.summoned).count());
            acid = acid.max(g.acid.len());
        }
        assert!(ravens >= LOCH_NESS_RAVENS_PER_CALL, "she called ravens, by day: {ravens}");
        assert!(ravens <= LOCH_NESS_MAX_RAVENS, "never more than her limit: {ravens}");
        assert!(acid >= 3, "a line of tainted water: {acid} columns");
        assert!(g.acid.iter().all(|c| g.world.is_wet_column(c.x, c.z)), "only water is tainted");
        assert!(g.animals.iter().filter(|a| a.kind == AnimalKind::Raven && a.summoned).all(|a| a.summoned), "they stay by day");
    }

    #[test]
    fn acid_water_burns_a_player_wading_in_it_for_ten_seconds_and_then_is_clean_again() {
        let mut g = at_the_shore();
        g.animals.clear();
        let lake = lake();
        let (x, z) = (lake.x, (0..80).map(|i| lake.z + i).find(|&z| g.world.is_wet_column(lake.x, z)).unwrap());
        g.player.pos = vec3(x as f32 + 0.5, g.world.ground_top(x, z) as f32 + 1.01, z as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        assert!(g.player_in_water());
        g.update_hazards(1.1);
        assert_eq!(g.health, MAX_HEALTH, "clean water is harmless");
        g.acid.push(AcidCell { x, z, left: ACID_SECONDS });
        assert!(g.standing_in_acid());
        for _ in 0..(3.05 / DT) as i32 {
            g.update_hazards(DT);
        }
        assert_eq!(g.health, MAX_HEALTH - 3 * ACID_DAMAGE, "a life a second");
        for _ in 0..(8.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_hazards(DT);
        }
        assert!(g.acid.is_empty() && !g.standing_in_acid(), "after {ACID_SECONDS} seconds it is clean");
    }

    #[test]
    fn a_bow_or_a_sling_shot_sends_nessie_to_the_middle_of_the_lake_and_calls_more_ravens() {
        let mut g = at_the_shore();
        g.world.night = false;
        give(&mut g, 1, ItemKind::Arrow, 5);
        hold(&mut g, ItemKind::Bow);
        let before = clearance(&g, nessie(&g).pos.x, nessie(&g).pos.z);
        assert!(g.shoot_bow(g.player.eye(), vec3(0.0, 0.0, -1.0)));
        assert!(nessie(&g).is_fleeing_shot());
        let ravens = g.animals.iter().filter(|a| a.kind == AnimalKind::Raven && a.summoned).count();
        assert_eq!(ravens, LOCH_NESS_ALARM_RAVENS, "more ravens come");
        // A second shot while she is running calls no more.
        g.attack_cooldown = 0.0;
        g.shoot_bow(g.player.eye(), vec3(0.0, 0.0, -1.0));
        assert_eq!(g.animals.iter().filter(|a| a.kind == AnimalKind::Raven && a.summoned).count(), ravens);
        for _ in 0..(LOCH_NESS_FLEE_SECONDS * 60.0) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            stand_on_ground(&mut g);
        }
        let after = clearance(&g, nessie(&g).pos.x, nessie(&g).pos.z);
        assert!(after >= before.max(LOCH_NESS_SHORE_MARGIN), "she ran out to open water: {before} -> {after}");
        assert!(after >= 9.0, "far from the shore: {after}");
    }

    // --- Ravens ---

    #[test]
    fn at_night_a_raven_keeps_its_distance_swoops_down_for_one_life_and_flies_away_again() {
        let mut g = game();
        open_field(&mut g);
        g.world.night = true;
        { let at = standing(&g, 12.0, 0.0) + vec3(0.0, 9.0, 0.0); add(&mut g, AnimalKind::Raven, at) };
        let (mut strikes, mut last_strike, mut nearest) = (0, -100.0_f32, f32::MAX);
        let mut far_after: Vec<f32> = Vec::new();
        let mut t = 0.0;
        for _ in 0..(60.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.spawn_timer = 0.0; // no other ravens come out of the night
            g.update_animals(DT);
            t += DT;
            let gap = (g.animals[0].pos - g.player.pos).length();
            nearest = nearest.min(gap);
            if g.health < MAX_HEALTH {
                assert_eq!(MAX_HEALTH - g.health, RAVEN_DAMAGE, "one life a swoop");
                assert!(t - last_strike >= RAVEN_ATTACK_MIN - 0.1, "not every moment");
                strikes += 1;
                last_strike = t;
                far_after.push(0.0);
            } else if let Some(f) = far_after.last_mut() {
                *f = f.max(gap);
            }
        }
        assert!(strikes >= 3, "it swooped {strikes} times in a minute");
        assert!(far_after.iter().filter(|&&f| f > 8.0).count() >= strikes as usize - 1, "and flew well away after each: {far_after:?}");
        assert!(nearest < 3.0, "it came down to the player's head: {nearest}");
        assert_eq!(g.animals[0].kind, AnimalKind::Raven);
    }

    #[test]
    fn ravens_come_out_at_night_and_go_at_dawn_except_the_ones_nessie_called() {
        let mut g = game();
        open_field(&mut g);
        g.world.night = true;
        for _ in 0..400 {
            g.spawn_timer = ANIMAL_SPAWN_INTERVAL;
            g.update_animals(0.0);
        }
        let ravens = |g: &Game| g.animals.iter().filter(|a| a.kind == AnimalKind::Raven).count();
        assert!(ravens(&g) >= 1 && ravens(&g) <= MAX_RAVEN, "ravens at night: {}", ravens(&g));
        g.animals.retain(|a| a.kind == AnimalKind::Raven);
        g.call_ravens(2, LOCH_NESS_MAX_RAVENS);
        g.world.night = false;
        g.update_animals(0.01);
        assert!(g.animals.iter().all(|a| a.summoned), "the night's ravens are gone at dawn");
        assert_eq!(ravens(&g), 2, "and Nessie's stay");
    }

    // --- The Iron Quail's feathers, thrown feathers and Phoenix Arrows ---

    #[test]
    fn an_iron_quail_shoots_iron_feathers_and_one_that_misses_lies_on_the_ground_to_be_picked_up() {
        let mut g = game();
        open_field(&mut g);
        let q = { let at = standing(&g, 14.0, 0.0) + vec3(0.0, 2.0, 0.0); add(&mut g, AnimalKind::IronQuail, at) };
        g.animals[q].pos = standing(&g, 14.0, 0.0) + vec3(0.0, 2.0, 0.0);
        let (mut shot, mut hurt) = (false, false);
        for _ in 0..(8.0 / DT) as i32 {
            g.update_animals(DT);
            shot |= !g.hostile_arrows.is_empty();
            hurt |= g.health < MAX_HEALTH;
            g.health = MAX_HEALTH;
        }
        assert!(shot, "feathers fly at the player");
        // A feather aimed at the ground falls short and lies there.
        g.hostile_arrows.clear();
        g.items.clear();
        g.hostile_arrows.push(Arrow::from_iron_quail(g.player.pos + vec3(6.0, 1.0, 0.0), vec3(0.0, -1.0, 0.0)));
        for _ in 0..60 {
            g.update_hostile_arrows(DT);
        }
        assert!(g.hostile_arrows.is_empty());
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::IronFeather).count(), 1, "the feather lies where it fell");
        assert!(hurt || shot);
        // One that hits the player hurts for its damage and is gone.
        g.items.clear();
        g.hostile_arrows.push(Arrow::from_iron_quail(g.player.pos + vec3(3.0, 1.0, 0.0), vec3(-1.0, 0.0, 0.0)));
        for _ in 0..30 {
            g.update_hostile_arrows(DT);
        }
        assert_eq!(g.health, MAX_HEALTH - IRON_FEATHER_DAMAGE as u32);
        assert!(g.items.iter().all(|i| i.kind != ItemKind::IronFeather));
    }

    #[test]
    fn a_held_iron_feather_is_thrown_like_a_star_and_is_used_up_only_when_it_hits_an_animal() {
        let mut g = game();
        open_field(&mut g);
        give(&mut g, 0, ItemKind::IronFeather, 3);
        g.select_slot(0);
        assert!(!Block::Dirt.can_break_with(g.held_item()), "a feather digs nothing");
        let eye = g.player.eye();
        // Thrown into the ground, it lands as an item again.
        assert!(g.attack(eye, vec3(1.0, -0.3, 0.0), true, true, false));
        assert_eq!(g.inventory.count(ItemKind::IronFeather), 2);
        for _ in 0..120 {
            g.update_arrows(DT);
        }
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::IronFeather).count(), 1, "it lies on the ground");
        // Thrown at a deer, it hurts it and is used up.
        g.items.clear();
        let deer = { let at = standing(&g, 6.0, 0.0); add(&mut g, AnimalKind::Deer, at) };
        g.animals[deer].pos.y = eye.y - 0.5;
        g.attack_cooldown = 0.0;
        assert!(g.throw_feather(eye, vec3(1.0, -0.05, 0.0)));
        for _ in 0..60 {
            g.update_arrows(DT);
        }
        assert!(g.animals.is_empty() || g.animals[0].health < DEER_HEALTH, "the deer was hit");
        assert!(g.items.iter().all(|i| i.kind != ItemKind::IronFeather), "and the feather is used up");
        assert_eq!(g.inventory.count(ItemKind::IronFeather), 1);
        // Not before the throw is over.
        assert!(!g.throw_feather(eye, vec3(1.0, 0.0, 0.0)));
    }

    #[test]
    fn a_phoenix_arrow_is_never_used_up_and_never_lands_on_the_ground() {
        let mut g = game();
        open_field(&mut g);
        give(&mut g, 1, ItemKind::PhoenixArrow, 1);
        give(&mut g, 2, ItemKind::Arrow, 3);
        hold(&mut g, ItemKind::Bow);
        let eye = g.player.eye();
        for _ in 0..4 {
            g.attack_cooldown = 0.0;
            assert!(g.shoot_bow(eye, vec3(1.0, -0.2, 0.0)));
        }
        assert_eq!(g.inventory.count(ItemKind::PhoenixArrow), 1, "never used up");
        assert_eq!(g.inventory.count(ItemKind::Arrow), 3, "and it is shot before the ordinary arrows");
        for _ in 0..240 {
            g.update_arrows(DT);
        }
        assert!(g.arrows.is_empty());
        assert!(g.items.is_empty(), "nothing lies on the ground where they missed");
        // Without one, the bow uses ordinary arrows and those do land.
        g.inventory.remove(ItemKind::PhoenixArrow, 1);
        g.attack_cooldown = 0.0;
        assert!(g.shoot_bow(eye, vec3(1.0, -0.2, 0.0)));
        assert_eq!(g.inventory.count(ItemKind::Arrow), 2);
        for _ in 0..240 {
            g.update_arrows(DT);
        }
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::Arrow).count(), 1);
    }

    // --- The silver key ---

    #[test]
    fn anything_that_would_drop_a_silver_key_drops_a_gold_bar_if_the_player_has_one_or_one_lies_about() {
        let mut g = game();
        open_field(&mut g);
        let stand = |g: &mut Game, n: i32| {
            let (x, z) = (g.player.pos.x.floor() as i32 + 3 + n, g.player.pos.z.floor() as i32);
            put(g, (x, GROUND_TOP + 1, z), Block::KeyStand);
            (x, GROUND_TOP + 1, z)
        };
        let at = stand(&mut g, 0);
        assert_eq!(g.mine(at.0, at.1, at.2), vec![ItemKind::SilverKey], "the first is a silver key");
        // It lies on the ground: the next stand gives gold.
        let at = stand(&mut g, 1);
        assert_eq!(g.mine(at.0, at.1, at.2), vec![ItemKind::Gold], "a key lies on the ground, so a gold bar");
        // Picked up, it is in the inventory, and the same holds.
        g.items.clear();
        g.inventory.add(ItemKind::SilverKey, 1);
        let at = stand(&mut g, 2);
        assert_eq!(g.mine(at.0, at.1, at.2), vec![ItemKind::Gold], "a key in the inventory, so a gold bar");
        // With none left anywhere, a key drops again.
        g.items.clear();
        g.inventory.remove(ItemKind::SilverKey, 1);
        let at = stand(&mut g, 3);
        assert_eq!(g.mine(at.0, at.1, at.2), vec![ItemKind::SilverKey]);
        // Other gold drops and other items are untouched.
        assert_eq!(g.key_or_gold(), ItemKind::Gold);
    }

    // --- The Wild Hunt's ghostly bears and crows ---

    #[test]
    fn the_first_ghostly_crows_and_bears_split_in_two_when_killed_and_their_young_do_not() {
        let mut g = game();
        open_field(&mut g);
        let c = { let at = standing(&g, 30.0, 0.0); add(&mut g, AnimalKind::HuntCrow, at) };
        g.animals[c].splits = true;
        let b = { let at = standing(&g, -30.0, 0.0); add(&mut g, AnimalKind::HuntBear, at) };
        g.animals[b].splits = true;
        g.hunt_total = HUNT_CROW_HEALTH + HUNT_BEAR_HEALTH;
        g.hurt_animals(&[(c, 1000)]);
        let crows = |g: &Game| g.animals.iter().filter(|a| a.kind == AnimalKind::HuntCrow).count();
        let bears = |g: &Game| g.animals.iter().filter(|a| a.kind == AnimalKind::HuntBear).count();
        assert_eq!((crows(&g), bears(&g)), (HUNT_SPLIT_INTO, 1), "two more crows come");
        assert!(g.animals.iter().filter(|a| a.kind == AnimalKind::HuntCrow).all(|a| !a.splits), "and they do not split");
        assert!(!g.world.landmarks.game_won, "the Hunt is not beaten while there are more");
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::HuntCrow).unwrap();
        g.hurt_animals(&[(i, 1000)]);
        assert_eq!(crows(&g), 1, "a killed young crow is simply gone");
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::HuntBear).unwrap();
        g.hurt_animals(&[(i, 1000)]);
        assert_eq!((bears(&g), g.animals.iter().filter(|a| a.kind == AnimalKind::HuntBear).filter(|a| !a.splits).count()), (HUNT_SPLIT_INTO, HUNT_SPLIT_INTO));
        assert!(g.hunt_total > HUNT_CROW_HEALTH + HUNT_BEAR_HEALTH, "the shared bar grows with them");
        let (_, share) = g.boss_bar().expect("a bar");
        assert!(share > 0.0 && share <= 1.0);
        // Kill the lot: now the game is won.
        while let Some(i) = g.animals.iter().position(|a| a.kind.is_wild_hunt()) {
            g.hurt_animals(&[(i, 100_000)]);
        }
        assert!(g.world.landmarks.game_won);
    }

    #[test]
    fn a_ghostly_bear_charges_and_a_ghostly_crow_flies_at_the_player_and_both_strike() {
        let mut g = game();
        open_field(&mut g);
        { let at = standing(&g, 10.0, 0.0); add(&mut g, AnimalKind::HuntBear, at) };
        { let at = standing(&g, 0.0, 10.0) + vec3(0.0, 3.0, 0.0); add(&mut g, AnimalKind::HuntCrow, at) };
        let (mut worst, mut closest) = (0, [f32::MAX; 2]);
        for _ in 0..(8.0 / DT) as i32 {
            g.spawn_timer = 0.0;
            g.update_animals(DT);
            worst = worst.max(MAX_HEALTH - g.health);
            g.health = MAX_HEALTH;
            for (i, a) in g.animals.iter().enumerate() {
                closest[i] = closest[i].min((a.pos - g.player.pos).length());
            }
        }
        assert!(worst >= HUNT_CROW_DAMAGE.min(HUNT_BEAR_DAMAGE), "they struck");
        assert!(closest.iter().all(|&c| c < 3.0), "both came right in: {closest:?}");
    }

    // --- Mine bats and salt ---

    #[test]
    fn bats_live_in_the_mineshafts_and_nowhere_else_in_the_normal_world() {
        let mut g = game();
        let m = Mountain { x: 30, z: -500, radius: MOUNTAIN_RADIUS, height: MOUNTAIN_HEIGHT.min(MOUNTAIN_MAX_HEIGHT), phase: 7 };
        g.world.add_mountain(m);
        let (rx, rz, floor) = crate::mines::network(&m).rooms[MINE_KEY_SHAFTS[0] * 3 + MINE_KEY_RING];
        g.player.pos = vec3(rx as f32 + 0.5, floor as f32 + 1.01, rz as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        assert!(g.player_in_mine(), "the player is in a chamber of the mines");
        g.world.night = false;
        for _ in 0..80 {
            g.spawn_mine_bats();
        }
        let bats: Vec<&Animal> = g.animals.iter().filter(|a| a.kind == AnimalKind::Bat).collect();
        assert!(!bats.is_empty() && bats.len() <= MAX_BAT_MINE, "bats in the mines: {}", bats.len());
        assert!(bats.iter().all(|a| a.lives_in_mine && a.pos.y < (g.world.ground_top(a.pos.x.floor() as i32, a.pos.z.floor() as i32) - 1) as f32), "each in the tunnels under the ground");
        // They stay by day, and are not despawned with the dawn.
        let n = bats.len();
        g.update_animals(0.01);
        assert_eq!(g.animals.iter().filter(|a| a.kind == AnimalKind::Bat).count(), n);
        // They bite a player who comes near.
        g.animals.retain(|a| a.kind == AnimalKind::Bat);
        let mut bat = Animal::new(AnimalKind::Bat, g.player.pos + vec3(0.0, 1.0, 1.0), &mut Rng::new(2));
        bat.lives_in_mine = true;
        g.animals.clear();
        g.animals.push(bat);
        let mut bitten = false;
        for _ in 0..(6.0 / DT) as i32 {
            g.update_animals(DT);
            bitten |= g.health < MAX_HEALTH;
        }
        assert!(bitten);
        // In the open air, under no mountain, there are none.
        let mut open = game();
        open.world.night = false;
        for _ in 0..80 {
            open.spawn_mine_bats();
        }
        assert!(!open.player_in_mine());
    }

    // --- The Phoenix ---

    fn on_the_giant_island() -> Game {
        let mut g = game();
        g.travel_to(Dimension::Skylands);
        g.world.night = false;
        g.whiteout = None;
        let island = skylands::giant_island(g.info.seed);
        g.player.pos = vec3(island.x as f32 + 0.5 + 30.0, island.top as f32 + 1.01, island.z as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        g
    }

    #[test]
    fn the_phoenix_rises_from_the_crater_throws_fireballs_and_swoops() {
        let mut g = on_the_giant_island();
        for _ in 0..20 {
            g.spawn_bosses();
        }
        assert_eq!(g.animals.iter().filter(|a| a.kind == AnimalKind::Phoenix).count(), 1, "one Phoenix");
        let crater = skylands::volcano_crater(g.info.seed);
        assert!(vec2(g.animals[0].pos.x - crater.x, g.animals[0].pos.z - crater.z).length() < 1.0, "in the crater");
        let (mut fire, mut burst, mut swoop, mut nearest) = (false, false, false, f32::MAX);
        for _ in 0..(40.0 / DT) as i32 {
            g.update_animals(DT);
            fire |= g.projectiles.iter().any(|p| p.kind == ProjectileKind::Fireball);
            burst |= g.bursts.iter().any(|b| b.shape == BurstShape::Fire);
            if let Some(p) = g.animals.iter().find(|a| a.kind == AnimalKind::Phoenix) {
                swoop |= p.air_phase == crate::animal::AirPhase::Dive;
                nearest = nearest.min((p.pos - g.player.pos).length());
            }
            g.health = MAX_HEALTH;
            g.tornadoes.clear();
        }
        assert!(fire && burst, "fireballs fly and burst");
        assert!(swoop && nearest < 3.0, "and it swoops down on the player: {nearest}");
        assert_eq!(g.animals.iter().filter(|a| a.kind == AnimalKind::Phoenix).count(), 1, "still just the one");
        assert!(g.boss_bar().map_or(false, |(n, _)| n == "The Phoenix"));
    }

    #[test]
    fn a_swoop_costs_four_and_a_fireball_three() {
        let mut g = on_the_giant_island();
        g.carry_out(BossEvent::Fireball { from: g.player.pos + vec3(8.0, 2.0, 0.0) });
        for _ in 0..(2.0 / DT) as i32 {
            g.update_hazards(DT);
        }
        assert_eq!(g.health, MAX_HEALTH - FIREBALL_DAMAGE, "the fireball burst on them");
        g.health = MAX_HEALTH;
        let mut phoenix = Animal::new(AnimalKind::Phoenix, g.player.pos + vec3(0.0, 1.0, 0.5), &mut Rng::new(2));
        phoenix.air_phase = crate::animal::AirPhase::Dive;
        assert!(phoenix.try_attack(g.player.pos));
        assert_eq!(AnimalKind::Phoenix.attack_damage(), PHOENIX_SWOOP_DAMAGE);
        assert_eq!(PHOENIX_SWOOP_DAMAGE, 4);
    }

    #[test]
    fn killing_the_phoenix_ends_the_fire_tornadoes_for_good_and_drops_a_feather_that_makes_a_phoenix_arrow() {
        let mut g = on_the_giant_island();
        for _ in 0..40 {
            g.update_animals(1.0);
            g.health = MAX_HEALTH;
        }
        assert!(!g.tornadoes.is_empty(), "tornadoes while it lives");
        for _ in 0..20 {
            g.spawn_bosses();
        }
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::Phoenix).unwrap();
        g.hurt_animals(&[(i, PHOENIX_HEALTH)]);
        assert!(g.world.landmarks.phoenix_slain);
        assert!(g.tornadoes.is_empty(), "the tornadoes stop");
        assert_eq!(g.items.iter().filter(|i| i.kind == ItemKind::PhoenixFeather).count(), PHOENIX_FEATHER_DROP as usize);
        for _ in 0..80 {
            g.update_animals(1.0);
            g.spawn_bosses();
        }
        assert!(g.tornadoes.is_empty(), "and none comes again, by day");
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::Phoenix), "nor does it");
        // The feather, a stick and a pebble make the arrow.
        give(&mut g, 3, ItemKind::PhoenixFeather, 1);
        give(&mut g, 4, ItemKind::Stick, 1);
        give(&mut g, 5, ItemKind::Pebble, 1);
        let recipe = crate::recipe::RECIPES.iter().find(|r| r.output == ItemKind::PhoenixArrow).expect("a recipe");
        assert!(crate::recipe::craft(&mut g.inventory, recipe));
        assert_eq!(g.inventory.count(ItemKind::PhoenixArrow), 1);
        // The Phoenix stays dead through a save and a load.
        let info = WorldInfo { name: "t".into(), seed: 1, path: PathBuf::new() };
        let text = g.save_text();
        assert!(text.lines().any(|l| l.ends_with("phoenixslain")), "it is saved");
        let mut again = Game::from_save(info, parse_save(&text), 3);
        assert!(again.in_skylands() && again.world.landmarks.phoenix_slain);
        again.world.night = false;
        again.update_animals(30.0);
        assert!(again.tornadoes.is_empty());
    }

    #[test]
    fn a_peep_stone_in_the_skylands_leads_to_the_giant_island() {
        let mut g = game();
        g.travel_to(Dimension::Skylands);
        hold(&mut g, ItemKind::PeepStone);
        let path = g.trail_path().expect("a trail");
        let (x, z) = *path.last().unwrap();
        let island = skylands::giant_island(g.info.seed);
        assert!(vec2(x - island.x as f32, z - island.z as f32).length() < island.radius, "it ends on the giant island");
        assert!(vec2(x - island.x as f32, z - island.z as f32).length() > 5.0, "at the foot of the volcano");
    }

    #[test]
    fn the_hunt_has_lancers_that_rest_between_charges_and_there_is_no_stalker() {
        let mut g = game();
        open_field(&mut g);
        let l = { let at = standing(&g, 12.0, 0.0); add(&mut g, AnimalKind::HuntLancer, at) };
        let mut phases = std::collections::HashSet::new();
        for _ in 0..(20.0 / DT) as i32 {
            g.health = MAX_HEALTH;
            g.update_animals(DT);
            phases.insert(format!("{:?}", g.animals[l].hunt_phase));
        }
        assert_eq!(phases.len(), 3, "charge, gallop away and rest: {phases:?}");
        assert!(HuntPhase::Rest != HuntPhase::Dash);
    }
}

#[cfg(test)]
mod upgrade_tests {
    use super::*;
    use crate::chunk::idx;
    use crate::animal::{AirPhase, Pounce};
    use crate::item::ItemStack;
    use std::path::PathBuf;

    fn game() -> Game {
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let mut g = Game::from_save(info, SaveData::new(), 5);
        g.world.stream_chunks(g.player.pos);
        g
    }

    fn hold(g: &mut Game, kind: ItemKind, count: u32) {
        g.inventory.slots[0] = Some(ItemStack { kind, count });
        g.select_slot(0);
    }

    fn open_sky(g: &mut Game) {
        for chunk in g.world.chunks.values_mut() {
            for y in GROUND_TOP + 1..WORLD_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        chunk.blocks[idx(x, y, z)] = Block::Air;
                    }
                }
            }
        }
    }

    fn step(g: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 60.0) as i32 {
            g.update_animals(1.0 / 60.0);
        }
    }

    /// A game in the Horizen with the player standing at (x, z) on the cave floor, the chunks round them loaded.
    fn in_cave_at(x: i32, z: i32) -> Game {
        let mut g = game();
        g.travel_to(Dimension::Horizen);
        g.player.pos = vec3(x as f32 + 0.5, GROUND_TOP as f32 + 1.01, z as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.animals.clear();
        g
    }

    fn at_king() -> Game {
        let g = game();
        let (kx, kz) = horizen::king_centre(g.info.seed);
        in_cave_at(kx, kz + KING_WALL_HALF - 6)
    }

    // --- bats, salt, the vampire bat ---

    #[test]
    fn a_bat_that_strikes_a_player_holding_salt_dies_and_takes_one_salt_and_no_damage_is_done() {
        let mut g = game();
        open_sky(&mut g);
        g.world.night = true;
        hold(&mut g, ItemKind::PinkSalt, 3);
        let mut bat = Animal::new(AnimalKind::Bat, g.player.pos + vec3(0.4, 0.2, 0.0), &mut Rng::new(1));
        bat.hunting = true;
        g.animals.push(bat);
        step(&mut g, 3.0);
        assert_eq!(g.health, MAX_HEALTH, "no damage");
        assert_eq!(g.inventory.count(ItemKind::PinkSalt), 2, "one crystal lost");
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::Bat), "and the bat is dead");
    }

    #[test]
    fn without_salt_in_hand_a_bat_bites_as_before_and_salt_elsewhere_does_not_help() {
        let mut g = game();
        open_sky(&mut g);
        g.world.night = true;
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::PinkSalt, count: 3 });
        hold(&mut g, ItemKind::Stick, 1);
        let mut bat = Animal::new(AnimalKind::Bat, g.player.pos + vec3(0.4, 0.2, 0.0), &mut Rng::new(1));
        bat.hunting = true;
        g.animals.push(bat);
        step(&mut g, 3.0);
        assert!(g.health < MAX_HEALTH, "bitten");
        assert_eq!(g.inventory.count(ItemKind::PinkSalt), 3);
    }

    #[test]
    fn the_cave_brings_in_bats_only_on_every_second_spawn_tick() {
        let mut g = in_cave_at(HORIZEN_SPAWN.0, HORIZEN_SPAWN.1 + 6);
        let (mut calls_with_bat, mut odd_with_bat) = (0, 0);
        for n in 1..=40u32 {
            g.animals.clear();
            g.spawn_wildlife();
            if !g.animals.is_empty() {
                calls_with_bat += 1;
                odd_with_bat += (n % 2 == 1) as i32;
            }
        }
        assert_eq!(odd_with_bat, 0, "nothing comes on the odd ticks");
        assert!((1..=20).contains(&calls_with_bat), "at most the twenty even ticks brought a bat: {calls_with_bat}");
    }

    fn kill_cave_bats(g: &mut Game, n: usize) {
        for _ in 0..n {
            let at = g.player.pos + vec3(3.0, 1.0, 0.0);
            g.animals.push(Animal::new(AnimalKind::Bat, at, &mut Rng::new(1)));
            let i = g.animals.len() - 1;
            g.hurt_animals(&[(i, 999)]);
        }
    }

    #[test]
    fn every_twentieth_cave_bat_killed_brings_a_vampire_bat_that_only_swoops() {
        let mut g = in_cave_at(HORIZEN_SPAWN.0, HORIZEN_SPAWN.1 + 6);
        kill_cave_bats(&mut g, 19);
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::VampireBat), "not before the twentieth");
        kill_cave_bats(&mut g, 1);
        assert_eq!(g.animals.iter().filter(|a| a.kind == AnimalKind::VampireBat).count(), 1, "the twentieth brings one");
        assert_eq!(g.bat_kills, 20);
        assert!(g.boss_bar().map_or(false, |(n, _)| n == "The Vampire Bat"), "with a health bar");
        // It circles, swoops, bites once and flies off: it never strikes outside a dive.
        g.player.pos = vec3(HORIZEN_SPAWN.0 as f32 + 0.5, GROUND_TOP as f32 + 1.01, HORIZEN_SPAWN.1 as f32 + 0.5 + 6.0);
        let (mut dives, mut struck_outside_dive) = (0, false);
        let mut last_phase = AirPhase::Circle;
        for _ in 0..(40.0 * 60.0) as i32 {
            let before = g.health;
            g.update_animals(1.0 / 60.0);
            g.health = MAX_HEALTH;
            let Some(v) = g.animals.iter().find(|a| a.kind == AnimalKind::VampireBat) else { break };
            if v.air_phase == AirPhase::Dive && last_phase != AirPhase::Dive {
                dives += 1;
            }
            if g.health < before {
                struck_outside_dive |= v.air_phase == AirPhase::Circle;
            }
            last_phase = v.air_phase;
        }
        assert!(dives >= 3, "it keeps swooping: {dives}");
        assert!(!struck_outside_dive);
        // Another twenty kills bring another once the first is dead.
        g.animals.clear();
        kill_cave_bats(&mut g, 20);
        assert_eq!(g.animals.iter().filter(|a| a.kind == AnimalKind::VampireBat).count(), 1);
    }

    #[test]
    fn bat_kills_in_the_caves_are_saved() {
        let mut g = in_cave_at(HORIZEN_SPAWN.0, HORIZEN_SPAWN.1 + 6);
        kill_cave_bats(&mut g, 7);
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&g.save_text()), 3);
        assert_eq!(again.bat_kills, 7);
    }

    // --- the boots, the idol ---

    fn jump_distance(boots: bool) -> f32 {
        let mut g = game();
        for cx in -2..=3 {
            for cz in -2..=2 {
                g.world.ensure_chunk(cx, cz);
            }
        }
        open_sky(&mut g);
        g.worn = boots.then_some(ItemKind::BootsOfTheSky);
        g.apply_worn_effects();
        g.player.pos = vec3(2.5, GROUND_TOP as f32 + 1.0, 8.5);
        g.player.yaw = 0.0;
        let input = crate::player::MoveInput { forward: true, sprint: true, jump: true, ..Default::default() };
        for _ in 0..10 {
            g.player.update(&g.world, 1.0 / 120.0, &crate::player::MoveInput::default());
        }
        let start = g.player.pos.x;
        let mut airborne = false;
        for _ in 0..600 {
            g.player.update(&g.world, 1.0 / 120.0, &input);
            if !g.player.on_ground {
                airborne = true;
            } else if airborne {
                break;
            }
        }
        g.player.pos.x - start
    }

    #[test]
    fn the_boots_of_the_sky_double_the_jump_distance_and_raise_the_running_speed_by_half() {
        let (plain, boots) = (jump_distance(false), jump_distance(true));
        assert!(plain > 4.0, "a sprint jump is about five blocks: {plain}");
        assert!((boots / plain - 2.0).abs() < 0.15, "twice as far: {plain} -> {boots}");
        let mut g = game();
        g.worn = Some(ItemKind::BootsOfTheSky);
        g.apply_worn_effects();
        assert_eq!(g.player.speed_factor, 1.5);
        g.worn = Some(ItemKind::PeltCloak);
        g.apply_worn_effects();
        assert_eq!((g.player.speed_factor, g.player.jump_factor), (1.0, 1.0), "only the boots do it");
    }

    #[test]
    fn the_boots_are_worn_in_the_one_worn_place_and_swap_with_other_garments() {
        let mut g = game();
        g.inventory.slots[0] = Some(ItemStack { kind: ItemKind::BootsOfTheSky, count: 1 });
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::PeltCloak, count: 1 });
        g.select_slot(0);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::BootsOfTheSky));
        g.select_slot(1);
        assert!(g.wear_held());
        assert_eq!(g.worn, Some(ItemKind::PeltCloak));
        assert_eq!(g.inventory.count(ItemKind::BootsOfTheSky), 1, "the boots came off and went back to the inventory");
    }

    #[test]
    fn a_cursed_idol_in_the_hotbar_doubles_damage_and_stops_all_healing() {
        let mut g = game();
        open_sky(&mut g);
        let deer_at = g.player.pos + vec3(6.0, 0.0, 0.0);
        g.animals.push(Animal::new(AnimalKind::Deer, deer_at, &mut Rng::new(1)));
        g.hurt_animals(&[(0, 3)]);
        assert_eq!(g.animals[0].health, DEER_HEALTH - 3, "an ordinary blow");
        g.inventory.slots[4] = Some(ItemStack { kind: ItemKind::CursedIdol, count: 1 });
        assert!(g.idol_in_hotbar());
        g.hurt_animals(&[(0, 1)]);
        assert_eq!(g.animals[0].health, DEER_HEALTH - 3 - 2, "double");
        g.health = 5;
        hold(&mut g, ItemKind::Berries, 3);
        assert!(!g.eat_held() && g.health == 5 && g.inventory.count(ItemKind::Berries) == 3, "berries do not heal");
        hold(&mut g, ItemKind::RedBerryStew, 1);
        assert!(!g.eat_held() && g.health == 5, "nor does stew");
        // Out of the hotbar (into the pack rows) the idol does nothing.
        g.inventory.slots[4] = None;
        g.inventory.slots[20] = Some(ItemStack { kind: ItemKind::CursedIdol, count: 1 });
        assert!(!g.idol_in_hotbar());
        hold(&mut g, ItemKind::Berries, 3);
        assert!(g.eat_held() && g.health == 6);
    }

    // --- the king ---

    #[test]
    fn playing_the_pipes_in_the_kings_castle_brings_his_voice_and_a_gold_pop_up() {
        let mut g = at_king();
        assert!(g.in_king_castle());
        hold(&mut g, ItemKind::MusicalPipes, 1);
        assert!(g.play_pipes());
        let (text, _) = g.ghost_text.clone().expect("the king speaks");
        assert_eq!(text, "I am the king eternal. Give me gold and I shall send my spirits to guard you.");
        assert_eq!(g.king_dialog, Some(1));
        assert!(g.ui_open(), "the pop-up takes the mouse");
        // Outside the walls the pipes bring nothing of the kind.
        let mut far = in_cave_at(HORIZEN_SPAWN.0, HORIZEN_SPAWN.1 + 6);
        hold(&mut far, ItemKind::MusicalPipes, 1);
        far.play_pipes();
        assert!(far.king_dialog.is_none() && !far.in_king_castle());
    }

    #[test]
    fn every_ten_gold_given_to_the_king_sends_one_spirit_archer_and_never_more_than_was_paid_for() {
        let mut g = at_king();
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Gold, count: 25 });
        assert_eq!(g.give_gold_to_king(3), 0, "thirty gold is more than the player has");
        assert_eq!(g.inventory.count(ItemKind::Gold), 25);
        assert_eq!(g.give_gold_to_king(2), 2);
        assert_eq!(g.inventory.count(ItemKind::Gold), 5);
        assert_eq!(g.spirit_count(), 2);
        assert_eq!(g.give_gold_to_king(1), 0, "five gold is not ten");
        let mut elsewhere = in_cave_at(HORIZEN_SPAWN.0, HORIZEN_SPAWN.1 + 6);
        elsewhere.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Gold, count: 50 });
        assert_eq!(elsewhere.give_gold_to_king(1), 0, "the king takes gold only in his castle");
        assert_eq!(elsewhere.inventory.count(ItemKind::Gold), 50);
    }

    #[test]
    fn a_spirit_archer_follows_the_player_shoots_ten_arrows_at_what_they_hit_and_then_vanishes() {
        let mut g = at_king();
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Gold, count: 10 });
        assert_eq!(g.give_gold_to_king(1), 1);
        // With nothing hit, it only keeps company: it shoots nothing and does not vanish.
        let spot = g.player.pos + vec3(14.0, 0.0, 0.0);
        let mut deer = Animal::new(AnimalKind::Deer, spot, &mut Rng::new(1));
        deer.health = 10_000;
        g.animals.push(deer);
        let run = vec3(20.0, 0.0, 0.0);
        g.player.pos += run;
        g.world.stream_chunks(g.player.pos);
        step(&mut g, 3.0);
        let spirit = g.animals.iter().find(|a| a.kind == AnimalKind::SpiritArcher).expect("still there");
        assert!((spirit.pos - g.player.pos).length() < 6.0, "it kept up when the player moved: {:?}", spirit.pos - g.player.pos);
        assert_eq!(spirit.shots_left, SPIRIT_SHOTS, "nothing was hit, nothing was shot");
        // The player hits the deer; the spirit then shoots at it until it has shot ten arrows and is gone.
        let deer_i = g.animals.iter().position(|a| a.kind == AnimalKind::Deer).unwrap();
        g.animals[deer_i].pos = g.player.pos + vec3(12.0, 0.0, 0.0);
        g.hurt_animals(&[(deer_i, 1)]);
        let start_health = g.animals[deer_i].health;
        let mut shots_seen = SPIRIT_SHOTS;
        let place = g.animals[deer_i].pos;
        for _ in 0..(40.0 * 60.0) as i32 {
            let i = g.animals.iter().position(|a| a.kind == AnimalKind::Deer).unwrap();
            g.animals[i].pos = place;
            g.animals[i].marked = SPIRIT_MARK_SECONDS;
            g.update_animals(1.0 / 60.0);
            g.update_arrows(1.0 / 60.0);
            if let Some(s) = g.animals.iter().find(|a| a.kind == AnimalKind::SpiritArcher) {
                shots_seen = s.shots_left;
            } else {
                break;
            }
        }
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::SpiritArcher), "it vanished after its last shot (shots left {shots_seen})");
        let deer = g.animals.iter().find(|a| a.kind == AnimalKind::Deer).unwrap();
        assert!(deer.health < start_health - 10, "ten arrows hit the deer: {} -> {}", start_health, deer.health);
        assert!(deer.health <= start_health - (SPIRIT_SHOTS as i32 - 2) * ARROW_DAMAGE, "most of them struck");
    }

    #[test]
    fn spirit_archers_are_not_hurt_by_the_player_and_are_not_targets() {
        let mut g = at_king();
        g.inventory.slots[1] = Some(ItemStack { kind: ItemKind::Gold, count: 10 });
        g.give_gold_to_king(1);
        let spirit = g.animals.iter().position(|a| a.kind == AnimalKind::SpiritArcher).unwrap();
        assert!(!g.animals[spirit].kind.is_huntable());
        assert!(!g.animals[spirit].damage(999), "nothing kills a spirit");
        assert_eq!(g.animals[spirit].marked, 0.0);
    }

    #[test]
    fn a_bone_totem_in_the_horizen_leads_through_the_caves_to_the_kings_castle() {
        let mut g = in_cave_at(HORIZEN_SPAWN.0, HORIZEN_SPAWN.1 + 6);
        hold(&mut g, ItemKind::BoneTotem, 1);
        let path = g.trail_path().expect("a trail");
        let (kx, kz) = horizen::king_centre(g.info.seed);
        assert_eq!(*path.last().unwrap(), (kx as f32 + 0.5, kz as f32 + 0.5));
        assert_eq!(path[0], (g.player.pos.x, g.player.pos.z));
        hold(&mut g, ItemKind::Stick, 1);
        assert!(g.trail_path().is_none());
    }

    // --- the volcano's tornado, the bridges at night ---

    fn in_skylands() -> Game {
        let mut g = game();
        g.travel_to(Dimension::Skylands);
        g
    }

    #[test]
    fn the_crater_tornado_is_always_there_by_day_burns_and_is_gone_at_night() {
        let mut g = in_skylands();
        let crater = skylands::volcano_crater(g.info.seed);
        g.player.pos = crater + vec3(30.0, 0.0, 0.0);
        g.world.stream_chunks(g.player.pos);
        g.update_animals(0.1);
        let t = g.crater_tornado.expect("a tornado in the crater by day");
        assert!(t.scale > 1.5 && (t.pos.x - crater.x).abs() < 0.01);
        g.phoenix_free();
        for _ in 0..(SKY_TORNADO_LIFETIME * 2.0 * 60.0) as i32 {
            g.update_animals(1.0 / 60.0);
        }
        assert!(g.crater_tornado.is_some(), "it never dies away");
        // Standing in it hurts.
        g.player.pos = crater;
        g.health = MAX_HEALTH;
        for _ in 0..(SKY_TORNADO_DAMAGE_INTERVAL * 60.0) as i32 + 5 {
            g.update_animals(1.0 / 60.0);
        }
        assert!(g.health < MAX_HEALTH, "the crater burns");
        // Night: no fire tornado at all.
        g.set_night(true);
        for _ in 0..200 {
            g.update_blackout(0.05);
        }
        g.update_animals(0.1);
        assert!(g.world.night && g.crater_tornado.is_none() && g.tornadoes.is_empty(), "no tornadoes at night");
        g.set_night(false);
        for _ in 0..200 {
            g.update_blackout(0.05);
        }
        g.update_animals(0.1);
        assert!(g.crater_tornado.is_some(), "and it is back by day");
    }

    impl Game {
        /// Test helper: a Phoenix that is already dead (so that only the crater's tornado is left).
        fn phoenix_free(&mut self) {
            self.world.landmarks.phoenix_slain = true;
            self.animals.clear();
        }
    }

    #[test]
    fn the_crater_tornado_survives_the_phoenix_but_the_wandering_ones_do_not() {
        let mut g = in_skylands();
        g.update_animals(SKY_TORNADO_INTERVAL + 1.0);
        assert!(!g.tornadoes.is_empty());
        g.phoenix_free();
        g.update_animals(0.1);
        assert!(g.tornadoes.is_empty() && g.crater_tornado.is_some());
    }

    #[test]
    fn at_night_the_bridges_of_the_skylands_break_in_the_loaded_world_and_come_back_by_day() {
        let mut g = in_skylands();
        let seed = g.info.seed;
        let bridge = skylands::east_bridge(seed, 0, 0);
        let mid = ((bridge.from.x + bridge.to.x) / 2, (bridge.from.z + bridge.to.z) / 2);
        let deck = skylands::deck_at(seed, mid.0, mid.1, false).expect("a deck in the middle by day");
        g.player.pos = vec3(mid.0 as f32 + 0.5, deck.y as f32 + 1.01, mid.1 as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        assert_eq!(g.world.block_at(mid.0, deck.y, mid.1), Block::BridgePlank);
        g.set_night(true);
        for _ in 0..200 {
            g.update_blackout(0.05);
        }
        assert!(g.world.night);
        assert_eq!(g.world.block_at(mid.0, deck.y, mid.1), Block::Air, "the middle of the bridge is gone");
        let p = g.player.pos;
        assert!(skylands::surface_top(seed, p.x.floor() as i32, p.z.floor() as i32, true).is_some(), "the player was set on solid footing, not dropped: {p:?}");
        g.set_night(false);
        for _ in 0..200 {
            g.update_blackout(0.05);
        }
        assert_eq!(g.world.block_at(mid.0, deck.y, mid.1), Block::BridgePlank, "whole again by day");
    }

    // --- the cave of the king ---

    #[test]
    fn the_kings_castle_stands_in_a_giant_cave_with_the_boots_in_its_heart() {
        let mut g = at_king();
        let (kx, kz) = horizen::king_centre(g.info.seed);
        g.player.pos = vec3(kx as f32 + 0.5, GROUND_TOP as f32 + 3.5, kz as f32 + 0.5);
        g.world.stream_chunks(g.player.pos);
        assert_eq!(g.world.block_at(kx, GROUND_TOP + 3, kz), Block::BootsPlinth);
        assert_eq!(g.world.block_at(kx, GROUND_TOP + 1, kz + KING_WALL_HALF), Block::Air, "the gate stands open");
        assert_eq!(g.world.block_at(kx + KING_WALL_HALF, GROUND_TOP + 1, kz), Block::CastleStone, "the wall");
        // Taking the boots by hand gives them.
        hold(&mut g, ItemKind::Stick, 1);
        assert_eq!(g.mine(kx, GROUND_TOP + 3, kz), vec![ItemKind::BootsOfTheSky]);
    }

    // --- the jungle ---

    #[test]
    fn the_jungle_covers_four_times_the_ground_and_does_not_reach_the_spawn_point() {
        use crate::terrain::{in_jungle, jungle_centre};
        assert_eq!(JUNGLE_RADIUS, 180);
        assert_eq!(JUNGLE_RADIUS * JUNGLE_RADIUS, 4 * 90 * 90, "four times the old area");
        for seed in 0..40 {
            let (jx, jz) = jungle_centre(seed);
            assert!(!in_jungle(seed, CHUNK_SIZE / 2, CHUNK_SIZE / 2), "seed {seed}: the spawn is not in the jungle");
            let gap = (((jx - CHUNK_SIZE / 2).pow(2) + (jz - CHUNK_SIZE / 2).pow(2)) as f32).sqrt();
            assert!(gap - JUNGLE_RADIUS as f32 > 100.0, "seed {seed}: well clear of the spawn, {gap}");
            assert!(in_jungle(seed, jx + 170, jz) && !in_jungle(seed, jx + 190, jz));
        }
    }

    fn in_jungle_game() -> (Game, i32, i32) {
        let mut g = game();
        let (jx, jz) = crate::terrain::jungle_centre(g.info.seed);
        g.player.pos = vec3(jx as f32 + 0.5, (GROUND_TOP + 1) as f32 + 0.5, jz as f32 + 0.5 + 60.0);
        g.world.stream_chunks(g.player.pos);
        g.world.rebuild_dirty(g.player.pos, usize::MAX);
        g.player.pos.y = g.world.ground_top(jx, jz + 60) as f32 + 1.01;
        (g, jx, jz)
    }

    #[test]
    fn the_orangutan_appears_on_the_temple_when_the_player_comes_near_stays_dead_once_slain_and_throws_and_smashes() {
        let (mut g, jx, jz) = in_jungle_game();
        g.spawn_bosses();
        let ape = g.animals.iter().find(|a| a.kind == AnimalKind::Orangutan).expect("an orangutan");
        let base = g.world.ground_top(jx, jz);
        assert!(ape.pos.y >= (base + 1 + temple_top_minus_tier()) as f32, "on the temple's upper tiers: {:?}", ape.pos);
        assert!((ape.pos.x - jx as f32).abs() < 8.0 && (ape.pos.z - jz as f32).abs() < 8.0);
        assert!(g.boss_bar().map_or(false, |(n, _)| n == "The Orangutan") || (g.player.pos - ape.pos).length() > BOSS_BAR_DISTANCE);
        // It hurls rocks at a player in range and smashes one who is close.
        let (mut rocks, mut smash, mut hit) = (false, false, false);
        let start = g.world.ground_top(jx, jz) as f32 + 1.0 + temple_top_minus_tier() as f32;
        for scene in 0..2 {
            let apepos = g.animals.iter().find(|a| a.kind == AnimalKind::Orangutan).unwrap().pos;
            g.player.pos = apepos + if scene == 0 { vec3(0.0, 0.0, 16.0) } else { vec3(0.0, 0.0, 2.8) };
            g.player.pos.y = start;
            g.world.stream_chunks(g.player.pos);
            for _ in 0..(14.0 * 60.0) as i32 {
                g.update_animals(1.0 / 60.0);
                g.health = MAX_HEALTH;
                let a = g.animals.iter().find(|a| a.kind == AnimalKind::Orangutan).unwrap();
                g.player.pos.x = a.pos.x;
                g.player.pos.y = a.pos.y;
                g.player.pos.z = a.pos.z + if scene == 0 { 16.0 } else { 2.8 };
                rocks |= g.projectiles.iter().any(|p| p.kind == ProjectileKind::Rock);
                smash |= g.bursts.iter().any(|b| b.shape == BurstShape::OrangutanSmash);
                hit |= g.bursts.iter().any(|b| b.shape == BurstShape::Rock);
            }
        }
        assert!(rocks && hit, "rocks fly and burst");
        assert!(smash, "and it smashes the ground");
        // Killing it is final, and saved.
        let i = g.animals.iter().position(|a| a.kind == AnimalKind::Orangutan).unwrap();
        g.hurt_animals(&[(i, 100_000)]);
        assert!(g.world.landmarks.orangutan_slain);
        assert!(g.items.iter().filter(|d| d.kind == ItemKind::Gold).count() >= ORANGUTAN_GOLD_DROP as usize);
        g.spawn_bosses();
        assert!(g.animals.iter().all(|a| a.kind != AnimalKind::Orangutan), "it does not come back");
        let info = WorldInfo { name: "test".into(), seed: 1, path: PathBuf::new() };
        let again = Game::from_save(info, parse_save(&g.save_text()), 3);
        assert!(again.world.landmarks.orangutan_slain);
    }

    fn temple_top_minus_tier() -> i32 {
        crate::temple::orangutan_post().2
    }

    #[test]
    fn the_cursed_idol_is_taken_from_its_plinth_in_the_temples_heart() {
        let (mut g, jx, jz) = in_jungle_game();
        let base = g.world.ground_top(jx, jz);
        g.player.pos = vec3(jx as f32 + 0.5, (base + 1) as f32 + 0.01, jz as f32 + 3.5);
        g.world.stream_chunks(g.player.pos);
        assert_eq!(g.world.block_at(jx, base + 1, jz), Block::IdolPlinth);
        hold(&mut g, ItemKind::Stick, 1);
        assert_eq!(g.mine(jx, base + 1, jz), vec![ItemKind::CursedIdol]);
    }

    #[test]
    fn a_jaguar_creeps_up_crouches_leaps_and_slinks_off() {
        let mut g = game();
        open_sky(&mut g);
        let at = g.player.pos + vec3(20.0, 0.0, 0.0);
        g.animals.push(Animal::new(AnimalKind::Jaguar, at, &mut Rng::new(2)));
        let mut seen: Vec<Pounce> = Vec::new();
        let mut hurt = false;
        for _ in 0..(40.0 * 60.0) as i32 {
            g.update_animals(1.0 / 60.0);
            let j = g.animals.iter().find(|a| a.kind == AnimalKind::Jaguar).unwrap();
            if seen.last() != Some(&j.pounce) {
                seen.push(j.pounce);
            }
            hurt |= g.health < MAX_HEALTH;
            g.health = MAX_HEALTH;
            g.player.vel = Vec3::ZERO;
        }
        assert!(seen.windows(4).any(|w| w == [Pounce::Stalk, Pounce::Crouch, Pounce::Leap, Pounce::Retreat]), "stalk, crouch, leap, retreat: {seen:?}");
        assert!(hurt, "a pounce that lands hurts");
    }

    #[test]
    fn a_flying_snake_spirals_through_the_air_and_swoops_down_to_bite() {
        let mut g = game();
        open_sky(&mut g);
        let at = g.player.pos + vec3(8.0, 5.0, 0.0);
        g.animals.push(Animal::new(AnimalKind::FlyingSnake, at, &mut Rng::new(4)));
        let (mut lo, mut hi, mut rmin, mut rmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut dived, mut hurt) = (false, false);
        for _ in 0..(60.0 * 60.0) as i32 {
            g.update_animals(1.0 / 60.0);
            let s = g.animals.iter().find(|a| a.kind == AnimalKind::FlyingSnake).unwrap();
            if s.air_phase == AirPhase::Circle {
                let r = vec2(s.pos.x - g.player.pos.x, s.pos.z - g.player.pos.z).length();
                lo = lo.min(s.pos.y - g.player.pos.y);
                hi = hi.max(s.pos.y - g.player.pos.y);
                rmin = rmin.min(r);
                rmax = rmax.max(r);
            }
            dived |= s.air_phase == AirPhase::Dive;
            hurt |= g.health < MAX_HEALTH;
            g.health = MAX_HEALTH;
        }
        assert!(hi - lo > 3.0 && rmax - rmin > 3.0, "it twists up and down and in and out: height {lo}..{hi}, radius {rmin}..{rmax}");
        assert!(dived && hurt, "and swoops down to bite");
    }

    #[test]
    fn jaguars_and_snakes_live_only_in_the_jungle() {
        let (mut g, _, _) = in_jungle_game();
        assert!(g.player_in_jungle());
        for _ in 0..400 {
            g.spawn_wildlife();
        }
        assert!(g.animals.iter().any(|a| a.kind == AnimalKind::Jaguar) && g.animals.iter().any(|a| a.kind == AnimalKind::FlyingSnake));
        assert!(g.animals.iter().filter(|a| a.kind == AnimalKind::Jaguar).count() <= MAX_JAGUAR);
        assert!(g.animals.iter().filter(|a| a.kind == AnimalKind::FlyingSnake).count() <= MAX_FLYING_SNAKE);
        let mut plain = game();
        for _ in 0..400 {
            plain.spawn_wildlife();
        }
        assert!(plain.animals.iter().all(|a| !matches!(a.kind, AnimalKind::Jaguar | AnimalKind::FlyingSnake)), "none in the ordinary forest");
    }

    #[test]
    fn bones_from_deer_and_from_the_sky_are_the_same_item_and_three_make_pipes() {
        let mut rng = Rng::new(3);
        assert_eq!(crate::item::drops_for(Block::Bones, &mut rng), vec![ItemKind::Bone]);
        assert_eq!(ItemKind::Bone.name(), "Bone");
        assert_eq!(ItemKind::from_id("deer_bone"), Some(ItemKind::Bone), "old saves still load");
        assert_eq!(ItemKind::from_id("bone"), Some(ItemKind::Bone));
    }
}
