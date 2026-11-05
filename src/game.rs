//! A running game session: the loaded world, the player, their inventory and dropped items.

use macroquad::prelude::*;
use std::fs;

use crate::animal::{Animal, AnimalKind};
use crate::arrow::{Arrow, Fate};
use crate::block::Block;
use crate::config::*;
use crate::dropped::{collect_nearby, DroppedItem};
use crate::hud::{InventoryUi, Toasts};
use crate::inventory::Inventory;
use crate::item::{drops_for_with_bonuses, drops_for_with_berries, Hearth, ItemKind};
use crate::landmarks::{GrandCastle, Lake, Mountain, Rainbow, RainbowState, TrekEvent, Treks};
use crate::player::Player;
use crate::rng::{clock_seed, Rng};
use crate::save::{parse_save, save_world, SaveData, WorldInfo};
use crate::world::{Pot, ray_box, Hit, Pos, World};

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
        let mut player = data.player;
        player.lift_out_of_blocks(&world);
        Game {
            info,
            dev_mode,
            world,
            player,
            inventory: data.inventory,
            items: data.items,
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
        }
    }

    pub fn save(&self) {
        save_world(&self.info, &self.world, &self.player, &self.inventory, &self.items, self.health, self.dev_mode);
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
        self.world.break_block(x, y, z);
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
        if block == Block::GoldPile && self.world.active_rainbow().map_or(false, |r| r.gold == (x, y, z)) {
            // The gold is gone, and so is the rainbow, for the rest of the game.
            self.world.set_rainbow(RainbowState::Spent);
            self.toasts.push("The rainbow fades. There will not be another.".to_string());
        }
        for &kind in &drops {
            self.items.push(DroppedItem::spawn(kind, (x, y, z), &mut self.rng));
        }
        drops
    }

    /// Whether a holly rod is anywhere in the hotbar (it need not be in the hand).
    pub fn holly_rod_in_hotbar(&self) -> bool {
        self.inventory.slots[..HOTBAR_SLOTS].iter().flatten().any(|stack| stack.kind == ItemKind::HollyRod)
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
        self.health = self.health.saturating_sub(amount);
        if self.health == 0 {
            self.player = Player::new();
            self.health = MAX_HEALTH;
            self.burn_timer = 0.0;
            self.toasts.push(death.to_string());
            for a in self.animals.iter_mut().filter(|a| a.kind == AnimalKind::Moose) {
                a.calm();
            }
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
        let changed = self.world.night != night;
        self.world.set_night(night);
        if changed {
            self.toasts.push(if night { "Night falls".to_string() } else { "Day breaks".to_string() });
        }
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
        block.can_break_with(self.held_item()) || self.can_ignite(block) || self.can_blow(block)
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
        let eps = Vec3::splat(0.01);
        let steps = (dist / 0.1).ceil() as i32;
        (0..=steps).all(|i| {
            let p = eye + dir * (i as f32 * 0.1).min(dist);
            !self.world.box_blocked(p - eps, p + eps)
        })
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
        self.animal_along(eye, dir, AXE_REACH)
    }

    /// Hurt animals (index, damage). The ones that die are removed and drop their loot; the others are knocked back
    /// away from the player.
    fn hurt_animals(&mut self, hits: &[(usize, i32)]) {
        let from = self.player.pos;
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
        dead.sort_unstable_by(|a, b| b.cmp(a));
        for i in dead {
            let animal = self.animals.remove(i);
            let drops = animal.kind.drops(&mut self.rng);
            for kind in drops {
                let spot = animal.pos + vec3(0.0, 0.5, 0.0);
                self.items.push(DroppedItem::spawn_at(kind, spot, &mut self.rng));
            }
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

    /// Fire the held launcher (`launcher`, which must be what is in hand), using up one of its ammunition.
    fn shoot(&mut self, launcher: ItemKind, eye: Vec3, dir: Vec3) -> bool {
        let Some(ammo) = launcher.ammo() else { return false };
        if self.held_item() != Some(launcher) || self.attack_cooldown > 0.0 {
            return false;
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
                landed.push((arrow.kind, arrow.pos - arrow.vel.normalize_or_zero() * ARROW_DROP_BACKOFF));
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
        let p = self.player.pos;
        let compass = self.held_item() == Some(ItemKind::Compass);
        let wading = self.player_in_water();
        if let Some(event) = self.treks.step((p.x, p.z), wading, compass) {
            self.bring_in(event);
        }
        self.try_show_rainbow();
        self.try_show_grand_castle();
    }

    /// Make the lake or mountain that a finished trek leads to, and say so.
    fn bring_in(&mut self, event: TrekEvent) {
        let phase = self.rng.range(0, 63);
        match event {
            TrekEvent::Lake { at, dir } => {
                self.world.add_lake(Lake::ahead(at, dir, phase));
                self.toasts.push("The river opens out ahead into a great lake.".to_string());
            }
            TrekEvent::Mountain { at, dir } => {
                self.world.add_mountain(Mountain::ahead(at, dir, phase));
                self.toasts.push("The compass has led you north: a great mountain rises ahead.".to_string());
            }
        }
    }

    /// Whether the pile of gold can lie in the column (x, z): dry, loaded, bare ground with nothing standing on it.
    fn gold_fits_at(&self, x: i32, z: i32) -> bool {
        if !self.world.chunks.contains_key(&World::chunk_of(x, z)) || self.world.is_wet_column(x, z) {
            return false;
        }
        let ground = self.world.ground_top(x, z);
        self.world.block_at(x, ground, z) == Block::Dirt && self.world.block_at(x, ground + 1, z) == Block::Air
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

    /// With a peep stone in hand, and only if the world has never had a rainbow, bring it out: an arc that begins
    /// beside the player and ends `RAINBOW_GOLD_MIN_DISTANCE` to `RAINBOW_GOLD_MAX_DISTANCE` blocks away on a pile of
    /// gold. Returns whether the rainbow appeared. If no good spot is found this frame, the next frame tries again.
    pub fn try_show_rainbow(&mut self) -> bool {
        if self.world.landmarks.rainbow != RainbowState::Unseen || self.world.night || self.held_item() != Some(ItemKind::PeepStone) {
            return false;
        }
        let p = self.player.pos;
        for _ in 0..RAINBOW_SPOT_TRIES {
            let angle = self.rng.unit() * std::f32::consts::TAU;
            let dist = RAINBOW_GOLD_MIN_DISTANCE + self.rng.unit() * (RAINBOW_GOLD_MAX_DISTANCE - RAINBOW_GOLD_MIN_DISTANCE);
            let (x, z) = ((p.x + angle.cos() * dist).floor() as i32, (p.z + angle.sin() * dist).floor() as i32);
            if !self.gold_fits_at(x, z) {
                continue;
            }
            let y = self.world.ground_top(x, z) + 1;
            self.world.set_block(x, y, z, Block::GoldPile);
            // The arc starts a few blocks to one side of the player, so it rises beside them.
            let side = (-angle.sin(), angle.cos());
            let from = ((p.x + side.0 * RAINBOW_START_OFFSET).floor() as i32, (p.z + side.1 * RAINBOW_START_OFFSET).floor() as i32);
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

    /// The ghostly trail a peep stone in the hand shows, as the corners of a path (x, z) that starts under the player:
    /// by night it leads to the grand castle (to its gate, and on to the door of its keep), by day to the end of the
    /// rainbow, even once the gold has been taken. `None` when there is no peep stone in hand or nothing to lead to.
    pub fn trail_path(&self) -> Option<Vec<(f32, f32)>> {
        if self.held_item() != Some(ItemKind::PeepStone) {
            return None;
        }
        let start = (self.player.pos.x, self.player.pos.z);
        if self.world.night {
            let castle = self.world.landmarks.grand_castle?;
            Some(vec![start, castle.gate_approach(), castle.keep_door()])
        } else {
            let (x, _, z) = self.world.landmarks.rainbow_end()?;
            Some(vec![start, (x as f32 + 0.5, z as f32 + 0.5)])
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
        let count = |k: AnimalKind| self.animals.iter().filter(|a| a.kind == k).count();
        let room = |kind: AnimalKind| match kind {
            AnimalKind::Deer => MAX_DEER,
            AnimalKind::Quail => MAX_QUAIL,
            AnimalKind::Bear => MAX_BEAR,
            AnimalKind::Rabbit => MAX_RABBIT,
            AnimalKind::Moose => MAX_MOOSE,
            AnimalKind::WillOWhisp => MAX_WILL_O_WHISP,
        }
        .saturating_sub(count(kind));
        let mut open: Vec<(AnimalKind, u64)> = [
            (AnimalKind::Deer, MAX_DEER, SPAWN_WEIGHT_DEER),
            (AnimalKind::Quail, MAX_QUAIL, SPAWN_WEIGHT_QUAIL),
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
        let player = self.player.pos;
        // Will-o-whisps are drawn to a player with a holly rod in the hotbar. One that is bonded and has been left
        // far behind (the player ran off, or started again at the beginning) catches up at once.
        let lure = if self.holly_rod_in_hotbar() { Some(player) } else { None };
        for a in self.animals.iter_mut().filter(|a| a.kind == AnimalKind::WillOWhisp) {
            a.lure = lure;
            if a.bonded && lure.is_some() && (a.pos - player).length() > WILL_O_WHISP_TELEPORT_DISTANCE {
                a.pos = player + vec3(0.0, WILL_O_WHISP_ORBIT_HEIGHT, WILL_O_WHISP_ORBIT_RADIUS);
            }
        }
        let flat = |a: &Animal| vec2(a.pos.x - player.x, a.pos.z - player.z).length();
        // Whisps go with the night, except the ones that follow the player.
        self.animals.retain(|a| {
            flat(a) < ANIMAL_DESPAWN_DISTANCE && (self.world.night || a.kind != AnimalKind::WillOWhisp || a.bonded)
        });
        let bears: Vec<Vec3> = self.animals.iter().filter(|a| a.kind == AnimalKind::Bear).map(|a| a.pos).collect();
        let mut strikes: Vec<(Vec3, u32, &'static str)> = Vec::new();
        for a in &mut self.animals {
            let near = vec2(a.pos.x - player.x, a.pos.z - player.z).length() <= ANIMAL_UPDATE_DISTANCE;
            if !near || !self.world.chunks.contains_key(&World::chunk_of_pos(a.pos)) {
                continue;
            }
            let threat = if a.kind.fears_bears() {
                let dist2 = |p: Vec3| (a.pos - p).length_squared();
                bears.iter().copied().fold(player, |best, bear| if dist2(bear) < dist2(best) { bear } else { best })
            } else {
                player
            };
            if let Some(kind) = a.update(&self.world, threat, dt, &mut self.rng) {
                self.items.push(DroppedItem::spawn_at(kind, a.pos + vec3(0.0, 0.3, 0.0), &mut self.rng));
            }
            if a.try_attack(player) {
                strikes.push((a.pos, a.kind.attack_damage(), a.kind.death_message()));
            }
        }
        for (from, damage, death) in strikes {
            if !self.hurt_player(damage, death) {
                self.knock_player(from);
            }
        }
        self.spawn_timer += dt;
        if self.spawn_timer >= ANIMAL_SPAWN_INTERVAL {
            self.spawn_timer = 0.0;
            self.spawn_wildlife();
        }
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
            assert_eq!(g.animals[0].health, BEAR_HEALTH - AXE_DAMAGE - i as i32, "{axe:?}");
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
            (ItemKind::CopperAxe, AXE_DAMAGE + 1),
            (ItemKind::BronzeAxe, AXE_DAMAGE + 2),
            (ItemKind::IronAxe, AXE_DAMAGE + 3),
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
        assert!(lake.x as f32 - at.0 >= LAKE_DISTANCE as f32, "beyond the generated land");
        // A point in the water between the island and the shore.
        let (wx, wz) = (lake.x + (lake.radius as f32 * 0.55) as i32, lake.z);
        load_column(&mut g, wx, wz);
        // The surface of the lake lies at the level of the river: a bed one block down, water above it.
        assert_eq!(g.world.block_at(wx, GROUND_TOP, wz), Block::Water);
        assert_eq!(g.world.block_at(wx, GROUND_TOP - LAKE_DEPTH, wz), Block::Dirt);
        assert_eq!(g.world.block_at(wx, GROUND_TOP + 1, wz), Block::Air);
        assert!(g.world.is_wet_column(wx, wz));
        // In the middle is an island: a hill of dry ground with a flat top, and a castle on the top.
        load_column(&mut g, lake.x + 30, lake.z);
        load_column(&mut g, lake.x + CASTLE_WALL_HALF, lake.z);
        load_column(&mut g, lake.x, lake.z);
        assert!(!g.world.is_wet_column(lake.x, lake.z) && !g.world.is_wet_column(lake.x + 30, lake.z));
        let top = GROUND_TOP + ISLAND_HILL_HEIGHT;
        assert_eq!(g.world.ground_top(lake.x, lake.z), top, "the island is a hill with a flat top");
        let slope = g.world.ground_top(lake.x + 30, lake.z);
        assert!(slope > GROUND_TOP && slope < top, "and its side slopes down to the water: {slope}");
        assert_eq!(g.world.block_at(lake.x + CASTLE_WALL_HALF, top + 1, lake.z), Block::CastleStone);
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
        assert!(m.z < at.1 as i32 - MOUNTAIN_DISTANCE, "north of the player, beyond the loaded land");
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
    fn a_peep_stone_in_hand_shows_one_rainbow_that_ends_in_gold_within_walking_distance() {
        let mut g = game(5);
        assert!(!g.try_show_rainbow(), "no peep stone in hand");
        assert_eq!(g.world.landmarks.rainbow, RainbowState::Unseen);
        hold(&mut g, ItemKind::Stick);
        assert!(!g.try_show_rainbow());
        hold(&mut g, ItemKind::PeepStone);
        assert!(g.try_show_rainbow());
        let rainbow = g.world.active_rainbow().expect("a rainbow");
        let (x, y, z) = rainbow.gold;
        assert_eq!(g.world.block_at(x, y, z), Block::GoldPile);
        assert_eq!(g.world.block_at(x, y - 1, z), Block::Dirt, "the gold lies on the ground");
        let p = g.player.pos;
        let away = ((x as f32 + 0.5 - p.x).powi(2) + (z as f32 + 0.5 - p.z).powi(2)).sqrt();
        assert!(away >= RAINBOW_GOLD_MIN_DISTANCE - 1.5 && away <= RAINBOW_GOLD_MAX_DISTANCE + 1.5, "{away} blocks away");
        let start = (((rainbow.from.0 as f32 + 0.5) - p.x).powi(2) + ((rainbow.from.1 as f32 + 0.5) - p.z).powi(2)).sqrt();
        assert!(start < 8.0, "the arc begins beside the player ({start} blocks)");
        assert!(g.toasts.contains("rainbow"));
        assert!(!g.try_show_rainbow(), "only one rainbow while the gold is out there");
        // It shows only while the peep stone is in hand.
        assert_eq!(g.visible_rainbow(), Some(rainbow));
        hold(&mut g, ItemKind::Stick);
        assert_eq!(g.visible_rainbow(), None);
    }

    #[test]
    fn taking_the_gold_ends_rainbows_for_the_rest_of_the_game_even_after_saving_and_loading() {
        let mut g = game(6);
        hold(&mut g, ItemKind::PeepStone);
        assert!(g.try_show_rainbow());
        let (x, y, z) = g.world.active_rainbow().unwrap().gold;
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
        assert!(nearest >= GRAND_DISTANCE as f32 - 1.5);
        assert!(nearest >= ((GEN_RADIUS + 1) * CHUNK_SIZE) as f32 - 1.0, "beyond the generated land: {nearest}");
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
        assert!(castle.x < p.x as i32 + GRAND_DISTANCE / 2, "it went west: {castle:?}");
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

