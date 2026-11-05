//! The voxel world: chunk streaming, block access, edits, fire, collision and raycasting.

use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::block::Block;
use crate::chunk::{idx, Chunk};
use crate::config::*;
use crate::item::{Hearth, ItemKind};
use crate::terrain::generate_chunk_blocks;

pub type Pos = (i32, i32, i32);

/// A piece of raw meat, a stone, a pebble, iron ore, or a tin (with its copper) lying on a burning block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cooking {
    pub raw: ItemKind,
    /// Seconds of heat still needed; zero once it is cooked or smelted.
    pub left: f32,
}

impl Cooking {
    pub fn done(&self) -> bool {
        self.left <= 0.0
    }

    /// What lies there right now: the item put on, or what it became once done.
    pub fn item(&self) -> ItemKind {
        if self.done() { self.raw.processed().unwrap_or(self.raw) } else { self.raw }
    }

    /// How far along it is, from 0 to 1.
    pub fn progress(&self) -> f32 {
        (1.0 - self.left / self.raw.heat_seconds()).clamp(0.0, 1.0)
    }

    /// What falls off when the fire burns out under it: the finished piece, or everything that went on
    /// unfinished (a tin in a forge went on with a copper, so both come back).
    pub fn released(&self) -> Vec<ItemKind> {
        if self.raw == ItemKind::Tin && !self.done() {
            vec![ItemKind::Tin, ItemKind::Copper]
        } else {
            vec![self.item()]
        }
    }
}

/// A tin pot sitting on a burning log or charcoal: water in it, raw meat in the water, and the heat the stew
/// still needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pot {
    pub water: bool,
    /// The raw meat in the pot (not yet a stew).
    pub meat: Option<ItemKind>,
    /// Seconds of heat the stew still needs; counts down only while the pot holds water and meat.
    pub left: f32,
}

impl Pot {
    pub fn empty() -> Pot {
        Pot { water: false, meat: None, left: STEW_SECONDS }
    }

    /// The stew, once it is done.
    pub fn stew(&self) -> Option<ItemKind> {
        if self.left <= 0.0 { self.meat.and_then(ItemKind::stewed) } else { None }
    }

    /// How far along the stew is, from 0 to 1.
    pub fn progress(&self) -> f32 {
        (1.0 - self.left / STEW_SECONDS).clamp(0.0, 1.0)
    }
}

pub struct World {
    pub seed: i64,
    pub chunks: HashMap<(i32, i32), Chunk>,
    /// World coordinates of generated blocks the player has removed (persisted to disk).
    pub removed: HashSet<Pos>,
    /// Blocks the player has put in the world (placed items, fire, charcoal), by position
    /// (persisted to disk). They are applied after `removed` when a chunk is generated.
    pub placed: HashMap<Pos, Block>,
    /// Seconds of burning left for each `Block::Fire` (persisted to disk).
    pub fires: HashMap<Pos, f32>,
    /// Meat and metal lying on a burning block, at most one piece per fire (persisted to disk).
    pub cooking: HashMap<Pos, Cooking>,
    /// Seconds of extra glow left for each lit clay forge worked with bellows (persisted).
    pub glow: HashMap<Pos, f32>,
    /// Tin pots standing on burning logs and charcoal, at most one per fire (persisted).
    pub pots: HashMap<Pos, Pot>,
    /// Items left behind by fires that burnt out, for the game to drop.
    pub released: Vec<(Pos, ItemKind)>,
}

/// What a ray hit: the block's cell, the block, and the empty cell the ray came from
/// (where a new block would go).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block: Block,
    pub prev: Pos,
}

/// Entry distance of a ray into an axis-aligned box, if it hits it (0 when starting inside).
pub fn ray_box(origin: Vec3, dir: Vec3, lo: [f32; 3], hi: [f32; 3]) -> Option<f32> {
    let (o, d) = (origin.to_array(), dir.to_array());
    let (mut t_min, mut t_max) = (0.0_f32, f32::INFINITY);
    for a in 0..3 {
        if d[a].abs() < 1e-8 {
            if o[a] < lo[a] || o[a] > hi[a] {
                return None;
            }
        } else {
            let (mut t1, mut t2) = ((lo[a] - o[a]) / d[a], (hi[a] - o[a]) / d[a]);
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }
    Some(t_min)
}


/// True when chunk `key` is within the render radius of chunk `centre`
/// (a square radius: Chebyshev distance in chunks).
pub fn in_render_range(centre: (i32, i32), key: (i32, i32)) -> bool {
    (key.0 - centre.0).abs() <= RENDER_RADIUS && (key.1 - centre.1).abs() <= RENDER_RADIUS
}

impl World {
    pub fn new(seed: i64, removed: HashSet<Pos>) -> Self {
        Self::with_edits(seed, removed, HashMap::new(), HashMap::new())
    }

    /// A world with the player's saved changes applied as its chunks are generated.
    pub fn with_edits(seed: i64, removed: HashSet<Pos>, placed: HashMap<Pos, Block>, fires: HashMap<Pos, f32>) -> Self {
        World { seed, chunks: HashMap::new(), removed, placed, fires, cooking: HashMap::new(), glow: HashMap::new(), pots: HashMap::new(), released: Vec::new() }
    }

    /// Put saved meat and metal back on its fires. Anything without a burning fire under it is dropped.
    pub fn with_cooking(mut self, cooking: HashMap<Pos, Cooking>) -> Self {
        self.cooking = cooking.into_iter().filter(|(pos, _)| self.fires.contains_key(pos)).collect();
        self
    }

    /// Put saved pots back on their fires. A pot without a burning fire under it is discarded.
    pub fn with_pots(mut self, pots: HashMap<Pos, Pot>) -> Self {
        self.pots = pots.into_iter().filter(|(pos, _)| self.fires.contains_key(pos)).collect();
        self
    }

    /// Restore the glow of saved forges. Glow without a fire burning there is dropped.
    pub fn with_glow(mut self, glow: HashMap<Pos, f32>) -> Self {
        self.glow = glow.into_iter().filter(|(pos, secs)| self.fires.contains_key(pos) && *secs > 0.0).collect();
        self
    }

    pub fn chunk_of(x: i32, z: i32) -> (i32, i32) {
        (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
    }

    /// Block at a world position. Missing chunks read as air; below the world reads as stone
    /// (so the underside of the stone layer is never meshed).
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        if y < 0 {
            return Block::Stone;
        }
        if y >= WORLD_HEIGHT {
            return Block::Air;
        }
        match self.chunks.get(&Self::chunk_of(x, z)) {
            Some(c) => c.blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))],
            None => Block::Air,
        }
    }

    /// Collision test for the box `lo..hi`. Ungenerated space and everything below the world
    /// count as solid walls. Blocks are tested by their actual shape, so a thin trunk or a
    /// low log only blocks where it is.
    pub fn box_blocked(&self, lo: Vec3, hi: Vec3) -> bool {
        // A boulder bulges `BOULDER_OVERHANG` past its cell, so look that far into the cells around the box too.
        let m = BOULDER_OVERHANG;
        for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
            for z in (lo.z - m).floor() as i32..=(hi.z + m).floor() as i32 {
                for x in (lo.x - m).floor() as i32..=(hi.x + m).floor() as i32 {
                    if self.cell_blocks_box(x, y, z, lo, hi) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn cell_blocks_box(&self, x: i32, y: i32, z: i32, lo: Vec3, hi: Vec3) -> bool {
        if y < 0 {
            return true;
        }
        if y >= WORLD_HEIGHT {
            return false;
        }
        let block = match self.chunks.get(&Self::chunk_of(x, z)) {
            Some(c) => c.blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))],
            None => return true,
        };
        if !block.collides() {
            return false;
        }
        let Some((bl, bh)) = block.bounds() else { return false };
        let (cx, cy, cz) = (x as f32, y as f32, z as f32);
        lo.x < cx + bh[0] && hi.x > cx + bl[0] && lo.y < cy + bh[1] && hi.y > cy + bl[1] && lo.z < cz + bh[2] && hi.z > cz + bl[2]
    }

    pub fn ensure_chunk(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            return;
        }
        let mut blocks = generate_chunk_blocks(self.seed, cx, cz);
        for &(x, y, z) in &self.removed {
            if Self::chunk_of(x, z) == (cx, cz) {
                // Only breakable blocks can have been removed. This also protects
                // stone if terrain changed since a world was last saved.
                let i = idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));
                if blocks[i].breakable() {
                    blocks[i] = blocks[i].remainder();
                }
            }
        }
        for (&(x, y, z), &block) in &self.placed {
            if Self::chunk_of(x, z) == (cx, cz) && (0..WORLD_HEIGHT).contains(&y) {
                blocks[idx(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))] = block;
            }
        }
        self.chunks.insert((cx, cz), Chunk { blocks, meshes: Vec::new(), water: Vec::new(), dirty: true });
        // Neighbours need their shared border re-meshed.
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if let Some(n) = self.chunks.get_mut(&(cx + dx, cz + dz)) {
                n.dirty = true;
            }
        }
    }

    /// Chunk containing a world-space position.
    pub fn chunk_of_pos(pos: Vec3) -> (i32, i32) {
        Self::chunk_of(pos.x.floor() as i32, pos.z.floor() as i32)
    }

    /// Keep every chunk within `GEN_RADIUS` of the player generated, and release the
    /// meshes of chunks that have left render range (they are rebuilt if the player returns).
    pub fn stream_chunks(&mut self, pos: Vec3) {
        let centre = Self::chunk_of_pos(pos);
        for dz in -GEN_RADIUS..=GEN_RADIUS {
            for dx in -GEN_RADIUS..=GEN_RADIUS {
                self.ensure_chunk(centre.0 + dx, centre.1 + dz);
            }
        }
        for (&key, chunk) in self.chunks.iter_mut() {
            if !in_render_range(centre, key) && !(chunk.meshes.is_empty() && chunk.water.is_empty()) {
                chunk.meshes.clear();
                chunk.water.clear();
                chunk.dirty = true;
            }
        }
    }

    /// Mark the chunk next to `key` dirty when (lx, lz) lies on that edge, because
    /// an edge block changes the faces visible in the neighbouring chunk.
    fn dirty_neighbours(&mut self, key: (i32, i32), lx: i32, lz: i32) {
        let mut touch = |dx: i32, dz: i32| {
            if let Some(n) = self.chunks.get_mut(&(key.0 + dx, key.1 + dz)) {
                n.dirty = true;
            }
        };
        if lx == 0 { touch(-1, 0); }
        if lx == CHUNK_SIZE - 1 { touch(1, 0); }
        if lz == 0 { touch(0, -1); }
        if lz == CHUNK_SIZE - 1 { touch(0, 1); }
    }

    /// Remove a block (caller has verified it is breakable), leaving air, or water if it lay under water
    /// (`Block::remainder`). A block the player had placed is simply forgotten; a generated block is
    /// added to the removed set.
    pub fn break_block(&mut self, x: i32, y: i32, z: i32) {
        let key = Self::chunk_of(x, z);
        let (lx, lz) = (x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));
        if let Some(c) = self.chunks.get_mut(&key) {
            c.blocks[idx(lx, y, lz)] = c.blocks[idx(lx, y, lz)].remainder();
            c.dirty = true;
        }
        if self.placed.remove(&(x, y, z)).is_none() {
            self.removed.insert((x, y, z));
        }
        self.fires.remove(&(x, y, z));
        self.dirty_neighbours(key, lx, lz);
    }

    /// Put a player-made block (placed item, fire, charcoal) at (x, y, z) and remember it.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: Block) {
        debug_assert!(block.id().is_some(), "{block:?} cannot be saved");
        let key = Self::chunk_of(x, z);
        let (lx, lz) = (x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));
        self.placed.insert((x, y, z), block);
        if let Some(c) = self.chunks.get_mut(&key) {
            c.blocks[idx(lx, y, lz)] = block;
            c.dirty = true;
        }
        self.dirty_neighbours(key, lx, lz);
    }

    /// Light the placed log, the charcoal or the filled forge at `pos`: a log becomes a fire, charcoal becomes
    /// embers and a filled forge a lit one, which burn for `seconds`. Returns false if there is nothing there to light.
    pub fn ignite(&mut self, pos: Pos, seconds: f32) -> bool {
        let lit = match self.block_at(pos.0, pos.1, pos.2) {
            Block::PlacedLog => Block::Fire,
            Block::Charcoal => Block::Embers,
            Block::ForgeCharged => Block::ForgeLit,
            _ => return false,
        };
        self.set_block(pos.0, pos.1, pos.2, lit);
        self.fires.insert(pos, seconds);
        true
    }

    /// Work the lit forge at `pos` with bellows: it glows brighter for `BELLOWS_GLOW_SECONDS`.
    /// Returns false unless a forge is burning there (open embers and logs cannot be worked this way).
    pub fn blow(&mut self, pos: Pos) -> bool {
        if self.block_at(pos.0, pos.1, pos.2) != Block::ForgeLit || !self.fires.contains_key(&pos) {
            return false;
        }
        self.glow.insert(pos, BELLOWS_GLOW_SECONDS);
        true
    }

    pub fn is_glowing(&self, pos: Pos) -> bool {
        self.glow.contains_key(&pos)
    }

    /// Add `seconds` to the fire at `pos`. Returns false if nothing is burning there.
    pub fn feed_fire(&mut self, pos: Pos, seconds: f32) -> bool {
        match self.fires.get_mut(&pos) {
            Some(t) => {
                *t += seconds;
                true
            }
            None => false,
        }
    }

    /// Whether `item` could be laid on the burning block at `pos` right now: raw meat goes on any fire, a stone or
    /// pebble on burning charcoal in the open, tin (the caller supplies the copper) in a lit forge, iron ore in a
    /// lit forge that is glowing. The block must be burning and hold no piece yet.
    pub fn can_lay(&self, pos: Pos, item: ItemKind) -> bool {
        let Some(hearth) = item.hearth() else { return false };
        if !self.fires.contains_key(&pos) || self.cooking.contains_key(&pos) || self.pots.contains_key(&pos) {
            return false;
        }
        let block = self.block_at(pos.0, pos.1, pos.2);
        match hearth {
            Hearth::AnyFire => true,
            Hearth::Embers => block == Block::Embers,
            Hearth::Forge => block == Block::ForgeLit,
            Hearth::GlowingForge => block == Block::ForgeLit && self.is_glowing(pos),
        }
    }

    /// Lay `item` on the burning block at `pos` (see `can_lay`). Returns false if it cannot go there.
    pub fn lay_on_fire(&mut self, pos: Pos, item: ItemKind) -> bool {
        if !self.can_lay(pos, item) {
            return false;
        }
        self.cooking.insert(pos, Cooking { raw: item, left: item.heat_seconds() });
        true
    }

    /// Whether a tin pot can be set on the block at `pos`: a burning log or burning charcoal in the open, with
    /// no pot and no piece of meat or metal on it yet.
    pub fn can_place_pot(&self, pos: Pos) -> bool {
        matches!(self.block_at(pos.0, pos.1, pos.2), Block::Fire | Block::Embers)
            && self.fires.contains_key(&pos)
            && !self.cooking.contains_key(&pos)
            && !self.pots.contains_key(&pos)
    }

    /// Set an empty pot on the fire at `pos` (see `can_place_pot`).
    pub fn place_pot(&mut self, pos: Pos) -> bool {
        if !self.can_place_pot(pos) {
            return false;
        }
        self.pots.insert(pos, Pot::empty());
        true
    }

    /// Fill the pot at `pos` with water. False if there is no pot or it already holds water.
    pub fn fill_pot(&mut self, pos: Pos) -> bool {
        match self.pots.get_mut(&pos) {
            Some(pot) if !pot.water => {
                pot.water = true;
                true
            }
            _ => false,
        }
    }

    /// Put raw `meat` into the watered pot at `pos`. False if there is no pot, no water, the pot already holds meat,
    /// or the item is not a raw meat.
    pub fn add_to_pot(&mut self, pos: Pos, meat: ItemKind) -> bool {
        match self.pots.get_mut(&pos) {
            Some(pot) if pot.water && pot.meat.is_none() && meat.stewed().is_some() => {
                pot.meat = Some(meat);
                pot.left = STEW_SECONDS;
                true
            }
            _ => false,
        }
    }

    /// Ladle the finished stew out of the pot at `pos`; the pot stays on the fire, empty.
    pub fn take_stew(&mut self, pos: Pos) -> Option<ItemKind> {
        let stew = self.pots.get(&pos)?.stew()?;
        self.pots.insert(pos, Pot::empty());
        Some(stew)
    }

    /// Take the pot off the fire at `pos`, spilling any water. False if there is none or it still holds meat.
    pub fn take_pot(&mut self, pos: Pos) -> bool {
        if self.pots.get(&pos).map_or(true, |p| p.meat.is_some()) {
            return false;
        }
        self.pots.remove(&pos);
        true
    }

    /// Whether the ray from `origin` along `dir` reaches water within `reach` blocks, before anything solid.
    pub fn water_in_reach(&self, origin: Vec3, dir: Vec3, reach: f32) -> bool {
        let eps = Vec3::splat(0.001);
        let mut t = 0.0;
        while t <= reach {
            let p = origin + dir * t;
            let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            if self.block_at(x, y, z).is_wet() && p.y - y as f32 <= 1.0 - WATER_SURFACE_DROP {
                return true;
            }
            if self.box_blocked(p - eps, p + eps) {
                return false;
            }
            t += 0.05;
        }
        false
    }

    /// Take the meat or metal off the burning block at `pos` if it is done, returning the finished item.
    pub fn take_product(&mut self, pos: Pos) -> Option<ItemKind> {
        if !self.cooking.get(&pos)?.done() {
            return None;
        }
        self.cooking.remove(&pos).map(|c| c.item())
    }

    /// Hand over (and forget) what fell off burnt-out fires since the last call.
    pub fn drain_released(&mut self) -> Vec<(Pos, ItemKind)> {
        std::mem::take(&mut self.released)
    }

    /// Burn every fire down by `dt` seconds, cooking meat and smelting metal lying on burning
    /// blocks (iron only while the forge glows). A burnt-out log leaves charcoal, burnt-out
    /// charcoal leaves ash, a burnt-out forge is empty again, and whatever lay on the fire (done or not) is released.
    /// Returns how many burned out. Fires keep burning in chunks that are not loaded.
    pub fn update_fires(&mut self, dt: f32) -> usize {
        for (pos, meat) in self.cooking.iter_mut() {
            let hot_enough = !meat.raw.needs_glow() || self.glow.contains_key(pos);
            if self.fires.contains_key(pos) && hot_enough {
                meat.left = (meat.left - dt).max(0.0);
            }
        }
        for (pos, pot) in self.pots.iter_mut() {
            if self.fires.contains_key(pos) && pot.water && pot.meat.is_some() {
                pot.left = (pot.left - dt).max(0.0);
            }
        }
        self.glow.retain(|_, secs| {
            *secs -= dt;
            *secs > 0.0
        });
        let mut out = Vec::new();
        for (&pos, t) in self.fires.iter_mut() {
            *t -= dt;
            if *t <= 0.0 {
                out.push(pos);
            }
        }
        for pos in &out {
            self.fires.remove(pos);
            self.glow.remove(pos);
            if let Some(pot) = self.pots.remove(pos) {
                self.released.push((*pos, ItemKind::TinPot));
                if let Some(meat) = pot.meat {
                    self.released.push((*pos, pot.stew().unwrap_or(meat)));
                }
            }
            if let Some(piece) = self.cooking.remove(pos) {
                self.released.extend(piece.released().into_iter().map(|item| (*pos, item)));
            }
            match self.placed.get(pos) {
                Some(Block::Fire) => self.set_block(pos.0, pos.1, pos.2, Block::Charcoal),
                Some(Block::Embers) => self.set_block(pos.0, pos.1, pos.2, Block::Ash),
                Some(Block::ForgeLit) => self.set_block(pos.0, pos.1, pos.2, Block::ClayForge),
                _ => {}
            }
        }
        out.len()
    }

    /// Voxel raycast (Amanatides & Woo). Returns the first block hit and the empty cell in
    /// front of it. The ray is tested against each block's real shape: a ray passing beside
    /// a thin trunk, a tuft of grass or a pebble carries on to whatever is behind it. Water
    /// cannot be aimed at: the ray looks through it to the river bed.
    pub fn raycast_hit(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<Hit> {
        let mut cell = (origin.x.floor() as i32, origin.y.floor() as i32, origin.z.floor() as i32);
        let step = (dir.x.signum() as i32, dir.y.signum() as i32, dir.z.signum() as i32);
        let next_boundary = |c: i32, s: i32| if s > 0 { (c + 1) as f32 } else { c as f32 };
        let t_max_axis = |o: f32, d: f32, c: i32, s: i32| {
            if d.abs() < 1e-8 { f32::INFINITY } else { (next_boundary(c, s) - o) / d }
        };
        let t_delta_axis = |d: f32| if d.abs() < 1e-8 { f32::INFINITY } else { (1.0 / d).abs() };
        let mut t_max = (
            t_max_axis(origin.x, dir.x, cell.0, step.0),
            t_max_axis(origin.y, dir.y, cell.1, step.1),
            t_max_axis(origin.z, dir.z, cell.2, step.2),
        );
        let t_delta = (t_delta_axis(dir.x), t_delta_axis(dir.y), t_delta_axis(dir.z));

        let mut prev = cell;
        let mut t = 0.0;
        while t <= max_dist {
            let b = self.block_at(cell.0, cell.1, cell.2);
            if b != Block::Air && !b.is_liquid() && cell.1 >= 0 {
                let hit = Hit { x: cell.0, y: cell.1, z: cell.2, block: b, prev };
                if b.is_full_cube() {
                    return Some(hit);
                }
                if let Some((lo, hi)) = b.bounds() {
                    let base = [cell.0 as f32, cell.1 as f32, cell.2 as f32];
                    let wlo = [base[0] + lo[0], base[1] + lo[1], base[2] + lo[2]];
                    let whi = [base[0] + hi[0], base[1] + hi[1], base[2] + hi[2]];
                    if ray_box(origin, dir, wlo, whi).map_or(false, |d| d <= max_dist) {
                        return Some(hit);
                    }
                }
            }
            // A boulder bulges past its cell, so a ray can clip it from a cell beside it.
            let t_exit = t_max.0.min(t_max.1).min(t_max.2);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let (nx, nz) = (cell.0 + dx, cell.2 + dz);
                    if (dx, dz) == (0, 0) || self.block_at(nx, cell.1, nz) != Block::Boulder {
                        continue;
                    }
                    if let Some((lo, hi)) = Block::Boulder.bounds() {
                        let base = [nx as f32, cell.1 as f32, nz as f32];
                        let wlo = [base[0] + lo[0], base[1] + lo[1], base[2] + lo[2]];
                        let whi = [base[0] + hi[0], base[1] + hi[1], base[2] + hi[2]];
                        if ray_box(origin, dir, wlo, whi).map_or(false, |d| d <= max_dist && d <= t_exit) {
                            return Some(Hit { x: nx, y: cell.1, z: nz, block: Block::Boulder, prev });
                        }
                    }
                }
            }
            prev = cell;
            if t_max.0 < t_max.1 && t_max.0 < t_max.2 {
                cell.0 += step.0;
                t = t_max.0;
                t_max.0 += t_delta.0;
            } else if t_max.1 < t_max.2 {
                cell.1 += step.1;
                t = t_max.1;
                t_max.1 += t_delta.1;
            } else {
                cell.2 += step.2;
                t = t_max.2;
                t_max.2 += t_delta.2;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{MoveInput, Player};

    #[test]
    fn break_and_persist() {
        let mut w = World::new(1, HashSet::new());
        w.stream_chunks(vec3(10.0, 6.0, 10.0));
        let hit = w.raycast_hit(vec3(10.5, 8.0, 10.5), vec3(0.0, -1.0, 0.0), 5.0).unwrap();
        assert_eq!((hit.y, hit.block), (5, Block::Dirt));
        assert_eq!(hit.prev, (hit.x, 6, hit.z), "the cell above the dirt is where a block would go");
        // A tree trunk is generated terrain that can be removed, and the removal survives a reload.
        let (x, y, z) = (0..WORLD_HEIGHT)
            .flat_map(|y| (0..CHUNK_SIZE).flat_map(move |x| (0..CHUNK_SIZE).map(move |z| (x, y, z))))
            .find(|&(x, y, z)| w.block_at(x, y, z) == Block::Wood)
            .expect("a tree");
        w.break_block(x, y, z);
        assert_eq!(w.block_at(x, y, z), Block::Air);
        let mut w2 = World::new(1, w.removed.clone());
        w2.ensure_chunk(0, 0);
        assert_eq!(w2.block_at(x, y, z), Block::Air);
    }

    #[test]
    fn saved_removals_of_dirt_are_ignored_because_dirt_can_no_longer_be_dug() {
        let mut w = World::new(1, HashSet::new());
        w.ensure_chunk(0, 0);
        w.break_block(3, GROUND_TOP, 3);
        assert_eq!(w.block_at(3, GROUND_TOP, 3), Block::Air, "break_block itself does not check");
        let mut again = World::new(1, w.removed.clone());
        again.ensure_chunk(0, 0);
        assert_eq!(again.block_at(3, GROUND_TOP, 3), Block::Dirt, "a world dug in an older version heals");
    }

    #[test]
    fn rays_look_through_water_to_the_river_bed() {
        let mut w = empty_sky_world();
        // One river column: the dirt is cut down to the bed at y = 4 and water fills the cell above it.
        put(&mut w, 8, GROUND_TOP, 8, Block::Water);
        let hit = w.raycast_hit(vec3(8.5, 9.0, 8.5), vec3(0.0, -1.0, 0.0), 10.0).unwrap();
        assert_eq!((hit.y, hit.block), (GROUND_TOP - RIVER_DEPTH, Block::Dirt), "the bed, not the water");
        assert_eq!(hit.prev, (8, GROUND_TOP, 8), "the cell in front of the bed is the water's");
    }

    #[test]
    fn water_does_not_stop_the_player_or_hold_them_up() {
        let mut w = empty_sky_world();
        put(&mut w, 8, GROUND_TOP, 8, Block::Water);
        put(&mut w, 8, GROUND_TOP - 1, 8, Block::Dirt);
        assert!(!w.box_blocked(vec3(8.2, GROUND_TOP as f32, 8.2), vec3(8.8, GROUND_TOP as f32 + 1.8, 8.8)));
    }

    #[test]
    fn stale_saves_cannot_remove_stone_but_removed_wood_stays_removed() {
        let b = generate_chunk_blocks(5, 0, 0);
        let mut removed = HashSet::new();
        removed.insert((4, 0, 4)); // stone
        let mut wood = None;
        for y in 0..WORLD_HEIGHT {
            for lz in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    if b[idx(lx, y, lz)] == Block::Wood && wood.is_none() {
                        wood = Some((lx, y, lz));
                    }
                }
            }
        }
        let (wx, wy, wz) = wood.expect("expected a tree");
        removed.insert((wx, wy, wz));
        let mut w = World::new(5, removed);
        w.ensure_chunk(0, 0);
        assert_eq!(w.block_at(4, 0, 4), Block::Stone);
        // Wood can be chopped with an axe, so a saved removal is honoured.
        assert_eq!(w.block_at(wx, wy, wz), Block::Air);
    }

    #[test]
    fn generates_radius_and_meshes_only_render_radius() {
        let mut w = World::new(1, HashSet::new());
        let pos = vec3(10.0, 6.0, 10.0);
        w.stream_chunks(pos);
        let gen_side = (2 * GEN_RADIUS + 1) as usize;
        assert_eq!(w.chunks.len(), gen_side * gen_side);

        w.rebuild_dirty(pos, usize::MAX);
        let meshed = w.chunks.values().filter(|c| !c.meshes.is_empty()).count();
        let render_side = (2 * RENDER_RADIUS + 1) as usize;
        assert_eq!(meshed, render_side * render_side);
        assert!(!w.chunks[&(RENDER_RADIUS, RENDER_RADIUS)].meshes.is_empty());
        assert!(w.chunks[&(GEN_RADIUS, 0)].meshes.is_empty());
        assert!(w.chunks[&(0, -GEN_RADIUS)].meshes.is_empty());
    }

    #[test]
    fn moving_away_releases_old_meshes_and_meshes_new_area() {
        let mut w = World::new(1, HashSet::new());
        let start = vec3(10.0, 6.0, 10.0);
        w.stream_chunks(start);
        w.rebuild_dirty(start, usize::MAX);
        assert!(!w.chunks[&(0, 0)].meshes.is_empty());

        let far = vec3(10.0 + 20.0 * 10.0, 6.0, 10.0);
        w.stream_chunks(far);
        assert!(w.chunks[&(0, 0)].meshes.is_empty(), "old chunk should be unloaded");
        w.rebuild_dirty(far, usize::MAX);
        assert!(!w.chunks[&(10, 0)].meshes.is_empty());
        assert!(!w.chunks[&(10 + RENDER_RADIUS, 0)].meshes.is_empty());
        assert!(w.chunks[&(10 + GEN_RADIUS, 0)].meshes.is_empty());
    }

    #[test]
    fn render_range_is_a_three_chunk_square() {
        assert!(in_render_range((0, 0), (3, -3)));
        assert!(in_render_range((5, 5), (2, 8)));
        assert!(!in_render_range((0, 0), (4, 0)));
        assert!(!in_render_range((0, 0), (0, -4)));
    }

    #[test]
    fn player_lands_on_ground_and_cannot_pass_stone() {
        let mut w = World::new(1, HashSet::new());
        w.stream_chunks(vec3(10.0, 6.0, 10.0));
        let mut p = Player::new();
        p.pos.y = 9.0;
        for _ in 0..120 {
            p.update(&w, 1.0 / 60.0, &MoveInput::default());
        }
        assert!((p.pos.y - 6.0).abs() < 0.01, "y = {}", p.pos.y);
        // A shaft down to the stone (made directly: dirt cannot be dug): must stop on stone (y = 1.0).
        for y in 1..=5 {
            w.break_block(10, y, 10);
        }
        p.pos = vec3(10.5, 8.0, 10.5);
        for _ in 0..240 {
            p.update(&w, 1.0 / 60.0, &MoveInput::default());
        }
        assert!((p.pos.y - 1.0).abs() < 0.01, "y = {}", p.pos.y);
    }

    /// A world with chunk (0, 0) cleared above the ground, so tests can place exactly what they need.
    fn empty_sky_world() -> World {
        let mut w = World::new(1, HashSet::new());
        w.ensure_chunk(0, 0);
        let c = w.chunks.get_mut(&(0, 0)).unwrap();
        for y in GROUND_TOP + 1..WORLD_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    c.blocks[idx(x, y, z)] = Block::Air;
                }
            }
        }
        w
    }

    fn put(w: &mut World, x: i32, y: i32, z: i32, b: Block) {
        w.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(x, y, z)] = b;
    }

    #[test]
    fn rays_pass_beside_a_thin_trunk_but_hit_it_head_on() {
        let mut w = empty_sky_world();
        put(&mut w, 5, 6, 5, Block::Wood);
        put(&mut w, 5, 6, 9, Block::Dirt);
        let down_z = vec3(0.0, 0.0, 1.0);
        // Trunk spans x in 5.15..5.85. A ray at x = 5.05 slips past it and hits the dirt behind.
        let hit = w.raycast_hit(vec3(5.05, 6.5, 1.5), down_z, 10.0).unwrap();
        assert_eq!((hit.x, hit.z, hit.block), (5, 9, Block::Dirt));
        // A ray through the middle hits the trunk, coming from the empty cell in front of it.
        let hit = w.raycast_hit(vec3(5.5, 6.5, 1.5), down_z, 10.0).unwrap();
        assert_eq!((hit.x, hit.z, hit.block, hit.prev), (5, 5, Block::Wood, (5, 6, 4)));
    }

    #[test]
    fn rays_ignore_tall_grass_cells_they_pass_over_but_hit_small_items_exactly() {
        let mut w = empty_sky_world();
        put(&mut w, 5, 6, 5, Block::PlacedPebble);
        // Pebble occupies roughly 0.3..0.7 and is under 0.15 high.
        let straight_down = vec3(0.0, -1.0, 0.0);
        let hit = w.raycast_hit(vec3(5.5, 9.0, 5.5), straight_down, 10.0).unwrap();
        assert_eq!((hit.x, hit.y, hit.z, hit.block), (5, 6, 5, Block::PlacedPebble));
        // Beside the pebble but in the same cell: carries on down to the dirt.
        let hit = w.raycast_hit(vec3(5.05, 9.0, 5.05), straight_down, 10.0).unwrap();
        assert_eq!((hit.y, hit.block), (GROUND_TOP, Block::Dirt));
        assert_eq!(hit.prev, (5, GROUND_TOP + 1, 5));
        // Too far away: no hit.
        assert!(w.raycast_hit(vec3(5.5, 9.0, 5.5), straight_down, 2.0).is_none());
    }

    #[test]
    fn a_ray_reports_the_cell_it_came_from() {
        let mut w = empty_sky_world();
        put(&mut w, 8, 6, 8, Block::Dirt);
        let hit = w.raycast_hit(vec3(3.5, 6.5, 8.5), vec3(1.0, 0.0, 0.0), 10.0).unwrap();
        assert_eq!((hit.x, hit.block, hit.prev), (8, Block::Dirt, (7, 6, 8)));
        let hit = w.raycast_hit(vec3(8.5, 9.5, 8.5), vec3(0.0, -1.0, 0.0), 10.0).unwrap();
        assert_eq!(hit.prev, (8, 7, 8));
    }

    #[test]
    fn collision_uses_the_shape_of_the_block() {
        let mut w = empty_sky_world();
        put(&mut w, 5, 6, 5, Block::Wood);
        put(&mut w, 9, 6, 9, Block::PlacedLog);
        put(&mut w, 12, 6, 12, Block::PlacedPebble);
        let boxx = |cx: f32, cy: f32, cz: f32, hw: f32, h: f32| w.box_blocked(vec3(cx - hw, cy, cz - hw), vec3(cx + hw, cy + h, cz + hw));
        // The margin beside a trunk is open; the trunk itself blocks.
        assert!(!boxx(5.05, 6.0, 5.5, 0.05, 1.8));
        assert!(boxx(5.5, 6.0, 5.5, 0.05, 1.8));
        assert!(boxx(5.2, 6.0, 5.5, 0.05, 1.8));
        // A log is 0.5 high: you can stand on it but pass over it at 0.6.
        assert!(boxx(9.5, 6.0, 9.5, 0.2, 1.8));
        assert!(!boxx(9.5, 6.5, 9.5, 0.2, 1.8));
        // Pebbles and plants do not block at all.
        assert!(!boxx(12.5, 6.0, 12.5, 0.2, 1.8));
        // Below the world and ungenerated chunks are walls.
        assert!(w.box_blocked(vec3(1.0, -0.5, 1.0), vec3(1.2, 0.5, 1.2)));
        assert!(w.box_blocked(vec3(500.0, 6.0, 500.0), vec3(500.2, 7.0, 500.2)));
        assert!(!w.box_blocked(vec3(1.0, 30.0, 1.0), vec3(1.2, 31.0, 1.2)), "above the world is open");
    }

    #[test]
    fn the_player_can_stand_on_a_log_and_walk_through_a_pebble() {
        let mut w = empty_sky_world();
        put(&mut w, 10, 6, 10, Block::PlacedLog);
        put(&mut w, 12, 6, 10, Block::PlacedPebble);
        let mut p = Player::new();
        p.pos = vec3(10.5, 9.0, 10.5);
        for _ in 0..120 {
            p.update(&w, 1.0 / 60.0, &MoveInput::default());
        }
        assert!((p.pos.y - 6.5).abs() < 0.01, "stands on the 0.5-high log, y = {}", p.pos.y);
        p.pos = vec3(12.5, 9.0, 10.5);
        for _ in 0..120 {
            p.update(&w, 1.0 / 60.0, &MoveInput::default());
        }
        assert!((p.pos.y - 6.0).abs() < 0.01, "pebbles do not hold you up, y = {}", p.pos.y);
    }

    #[test]
    fn placed_blocks_persist_and_breaking_them_leaves_terrain_alone() {
        let mut w = empty_sky_world();
        w.set_block(7, 6, 7, Block::PlacedStone);
        assert_eq!(w.block_at(7, 6, 7), Block::PlacedStone);
        assert!(w.chunks[&(0, 0)].dirty);
        // A reloaded world regenerates the same block from the saved edit.
        let mut w2 = World::with_edits(1, w.removed.clone(), w.placed.clone(), w.fires.clone());
        w2.ensure_chunk(0, 0);
        assert_eq!(w2.block_at(7, 6, 7), Block::PlacedStone);
        // Breaking it forgets it without touching the removed set.
        w.break_block(7, 6, 7);
        assert!(w.placed.is_empty() && w.removed.is_empty());
        assert_eq!(w.block_at(7, 6, 7), Block::Air);
    }

    #[test]
    fn a_block_placed_where_terrain_was_removed_survives_a_reload() {
        // A pebble lying on the ground is generated terrain that can be picked up; a stone is then put in its place.
        let mut w = World::new(1, HashSet::new());
        w.ensure_chunk(0, 0);
        let (x, y, z) = (0..CHUNK_SIZE)
            .flat_map(|z| (0..CHUNK_SIZE).map(move |x| (x, GROUND_TOP + 1, z)))
            .find(|&(x, y, z)| w.block_at(x, y, z) == Block::PlacedPebble)
            .expect("a pebble lies about");
        w.break_block(x, y, z);
        w.set_block(x, y, z, Block::PlacedStone);
        let mut w2 = World::with_edits(1, w.removed.clone(), w.placed.clone(), HashMap::new());
        w2.ensure_chunk(0, 0);
        assert_eq!(w2.block_at(x, y, z), Block::PlacedStone);
        w2.break_block(x, y, z);
        let mut w3 = World::with_edits(1, w2.removed.clone(), w2.placed.clone(), HashMap::new());
        w3.ensure_chunk(0, 0);
        assert_eq!(w3.block_at(x, y, z), Block::Air, "the pebble stays removed");
    }

    #[test]
    fn edits_on_a_chunk_edge_dirty_the_neighbour() {
        let mut w = World::new(1, HashSet::new());
        w.ensure_chunk(0, 0);
        w.ensure_chunk(1, 0);
        w.chunks.get_mut(&(1, 0)).unwrap().dirty = false;
        w.set_block(CHUNK_SIZE - 1, GROUND_TOP + 1, 4, Block::PlacedPebble);
        assert!(w.chunks[&(1, 0)].dirty);
    }

    #[test]
    fn fires_only_light_placed_logs_and_burn_down_to_charcoal() {
        let mut w = empty_sky_world();
        assert!(!w.ignite((4, 6, 4), 60.0), "nothing to light");
        w.set_block(4, 6, 4, Block::PlacedStone);
        assert!(!w.ignite((4, 6, 4), 60.0), "stones do not burn");
        w.set_block(5, 6, 4, Block::PlacedLog);
        assert!(w.ignite((5, 6, 4), 60.0));
        assert_eq!(w.block_at(5, 6, 4), Block::Fire);
        assert!(!w.ignite((5, 6, 4), 60.0), "already burning");
        assert!(w.feed_fire((5, 6, 4), 60.0));
        assert!(!w.feed_fire((4, 6, 4), 60.0));
        assert_eq!(w.update_fires(119.0), 0);
        assert_eq!(w.update_fires(2.0), 1);
        assert_eq!(w.block_at(5, 6, 4), Block::Charcoal);
        assert_eq!(w.update_fires(100.0), 0);
        assert_eq!(w.block_at(5, 6, 4), Block::Charcoal, "charcoal is permanent");
    }

    fn burning_world() -> (World, Pos) {
        let mut w = empty_sky_world();
        w.set_block(5, 6, 4, Block::PlacedLog);
        assert!(w.ignite((5, 6, 4), 60.0));
        (w, (5, 6, 4))
    }

    #[test]
    fn meat_cooks_on_a_burning_log_and_can_be_taken_once_done() {
        let (mut w, fire) = burning_world();
        assert!(!w.lay_on_fire(fire, ItemKind::Leather), "only raw meat can be laid on the fire");
        assert!(!w.lay_on_fire((6, 6, 4), ItemKind::Venison), "nothing burns there");
        assert!(w.lay_on_fire(fire, ItemKind::Venison));
        assert!(!w.lay_on_fire(fire, ItemKind::Poultry), "one piece per log");
        assert_eq!(w.take_product(fire), None, "still raw");
        w.update_fires(COOK_SECONDS - 1.0);
        assert!(!w.cooking[&fire].done());
        assert_eq!(w.cooking[&fire].item(), ItemKind::Venison);
        assert_eq!(w.take_product(fire), None);
        w.update_fires(1.0);
        assert_eq!(w.cooking[&fire].item(), ItemKind::CookedVenison);
        assert_eq!(w.take_product(fire), Some(ItemKind::CookedVenison));
        assert!(w.cooking.is_empty());
        assert_eq!(w.take_product(fire), None);
        assert!(w.lay_on_fire(fire, ItemKind::Poultry), "the log is free again");
    }

    #[test]
    fn a_fire_that_burns_out_drops_the_meat_it_held_cooked_or_raw() {
        let (mut w, fire) = burning_world();
        w.lay_on_fire(fire, ItemKind::Poultry);
        w.update_fires(COOK_SECONDS + 1.0);
        w.update_fires(60.0);
        assert_eq!(w.block_at(fire.0, fire.1, fire.2), Block::Charcoal);
        assert_eq!(w.drain_released(), vec![(fire, ItemKind::CookedPoultry)]);
        assert!(w.drain_released().is_empty() && w.cooking.is_empty());

        let (mut w, fire) = burning_world();
        w.update_fires(55.0);
        w.lay_on_fire(fire, ItemKind::Venison);
        w.update_fires(6.0);
        assert_eq!(w.drain_released(), vec![(fire, ItemKind::Venison)], "raw meat comes back raw");
    }

    #[test]
    fn charcoal_is_lit_into_embers_and_burns_down_to_ash() {
        let mut w = empty_sky_world();
        w.set_block(4, 6, 4, Block::Charcoal);
        assert!(w.ignite((4, 6, 4), 60.0));
        assert_eq!(w.block_at(4, 6, 4), Block::Embers);
        assert!(!w.ignite((4, 6, 4), 60.0), "already burning");
        assert!(!w.blow((4, 6, 4)), "bellows only work a forge");
        assert_eq!(w.update_fires(59.0), 0);
        assert_eq!(w.update_fires(2.0), 1);
        assert_eq!(w.block_at(4, 6, 4), Block::Ash);
        assert!(w.fires.is_empty());
        assert!(!w.ignite((4, 6, 4), 60.0), "ash does not burn");
    }

    fn embers_world() -> (World, Pos) {
        let mut w = empty_sky_world();
        w.set_block(4, 6, 4, Block::Charcoal);
        assert!(w.ignite((4, 6, 4), 100.0));
        (w, (4, 6, 4))
    }

    fn forge_world() -> (World, Pos) {
        let mut w = empty_sky_world();
        w.set_block(4, 6, 4, Block::ForgeCharged);
        assert!(w.ignite((4, 6, 4), 100.0));
        assert_eq!(w.block_at(4, 6, 4), Block::ForgeLit);
        (w, (4, 6, 4))
    }

    #[test]
    fn a_stone_and_a_pebble_smelt_on_plain_burning_charcoal_without_bellows() {
        let (mut w, pos) = embers_world();
        assert!(!w.lay_on_fire(pos, ItemKind::Leather) && !w.lay_on_fire(pos, ItemKind::Copper));
        assert!(w.lay_on_fire(pos, ItemKind::Stone));
        assert!(!w.lay_on_fire(pos, ItemKind::Pebble), "occupied");
        w.update_fires(SMELT_SECONDS - 1.0);
        assert_eq!(w.take_product(pos), None, "not yet");
        w.update_fires(1.0);
        assert_eq!(w.take_product(pos), Some(ItemKind::Copper));
        assert!(w.lay_on_fire(pos, ItemKind::Pebble));
        assert!((w.cooking[&pos].progress()).abs() < 1e-6);
        w.update_fires(SMELT_SECONDS / 2.0);
        assert!((w.cooking[&pos].progress() - 0.5).abs() < 1e-3);
        w.update_fires(SMELT_SECONDS);
        assert_eq!(w.take_product(pos), Some(ItemKind::Tin));
        assert!(w.lay_on_fire(pos, ItemKind::Poultry), "meat still cooks there");
    }

    #[test]
    fn stones_and_pebbles_do_not_smelt_on_a_log_fire_or_in_a_forge_and_bronze_needs_the_forge() {
        let (mut w, log) = burning_world();
        for item in [ItemKind::Stone, ItemKind::Pebble, ItemKind::Tin, ItemKind::IronOre] {
            assert!(!w.can_lay(log, item) && !w.lay_on_fire(log, item), "{item:?} on a log fire");
        }
        let (mut w, forge) = forge_world();
        for item in [ItemKind::Stone, ItemKind::Pebble] {
            assert!(!w.lay_on_fire(forge, item), "{item:?} in a forge");
        }
        let (mut w2, embers) = embers_world();
        assert!(!w2.lay_on_fire(embers, ItemKind::Tin), "bronze in the open");
        assert!(!w2.lay_on_fire(embers, ItemKind::IronOre), "iron in the open");
        assert!(w.lay_on_fire(forge, ItemKind::Tin));
    }

    #[test]
    fn bronze_smelts_in_a_lit_forge_without_bellows_and_unfinished_bronze_gives_back_both_metals() {
        let (mut w, forge) = forge_world();
        assert!(w.lay_on_fire(forge, ItemKind::Tin));
        w.update_fires(SMELT_SECONDS);
        assert_eq!(w.take_product(forge), Some(ItemKind::Bronze));

        let (mut w, forge) = forge_world();
        assert!(w.lay_on_fire(forge, ItemKind::Tin));
        w.update_fires(SMELT_SECONDS - 1.0);
        assert_eq!(w.cooking[&forge].released(), vec![ItemKind::Tin, ItemKind::Copper]);
        w.fires.insert(forge, 0.4);
        w.update_fires(0.5);
        assert_eq!(w.drain_released(), vec![(forge, ItemKind::Tin), (forge, ItemKind::Copper)]);
        assert_eq!(w.block_at(forge.0, forge.1, forge.2), Block::ClayForge, "a burnt-out forge is empty again");
        assert!(w.fires.is_empty() && w.cooking.is_empty());
    }

    #[test]
    fn iron_ore_smelts_only_in_a_lit_forge_while_bellows_keep_it_glowing() {
        let (mut w, forge) = forge_world();
        assert!(!w.can_lay(forge, ItemKind::IronOre), "not glowing");
        assert!(w.blow(forge) && w.is_glowing(forge));
        assert!(w.lay_on_fire(forge, ItemKind::IronOre));
        w.update_fires(SMELT_SECONDS - 2.0);
        assert!(!w.cooking[&forge].done());
        // The glow fades: the ore stops cooking until the forge is worked again.
        w.glow.insert(forge, 1.0);
        for _ in 0..100 {
            w.update_fires(0.05);
        }
        let left = w.cooking[&forge].left;
        assert!((left - 1.0).abs() < 0.1, "only the glowing second counted: {left}");
        assert!(!w.is_glowing(forge));
        w.update_fires(5.0);
        assert!((w.cooking[&forge].left - left).abs() < 1e-6, "cold forge, no progress");
        w.blow(forge);
        w.update_fires(2.0);
        assert_eq!(w.take_product(forge), Some(ItemKind::Iron));
        assert!(!w.blow((9, 6, 9)));
    }

    #[test]
    fn a_filled_forge_is_lit_and_a_lit_forge_can_be_fed() {
        let mut w = empty_sky_world();
        w.set_block(4, 6, 4, Block::ClayForge);
        assert!(!w.ignite((4, 6, 4), 60.0), "an empty forge does not burn");
        w.set_block(4, 6, 4, Block::ForgeCharged);
        assert!(w.ignite((4, 6, 4), 60.0));
        assert!(!w.ignite((4, 6, 4), 60.0), "already burning");
        assert!(w.feed_fire((4, 6, 4), 60.0));
        assert_eq!(w.update_fires(119.0), 0);
        assert_eq!(w.update_fires(2.0), 1);
        assert_eq!(w.block_at(4, 6, 4), Block::ClayForge);
    }

    #[test]
    fn saved_meat_without_a_fire_is_discarded() {
        let fires = HashMap::from([((1, 6, 1), 30.0)]);
        let meat = Cooking { raw: ItemKind::Venison, left: 3.0 };
        let w = World::with_edits(1, HashSet::new(), HashMap::from([((1, 6, 1), Block::Fire)]), fires)
            .with_cooking(HashMap::from([((1, 6, 1), meat), ((9, 6, 9), meat)]));
        assert_eq!(w.cooking.len(), 1);
        assert!(w.cooking.contains_key(&(1, 6, 1)));
    }

    #[test]
    fn fires_burn_down_even_in_unloaded_chunks() {
        let mut w = World::with_edits(
            1,
            HashSet::new(),
            HashMap::from([((500, 6, 500), Block::Fire)]),
            HashMap::from([((500, 6, 500), 5.0)]),
        );
        assert_eq!(w.update_fires(6.0), 1);
        assert_eq!(w.placed[&(500, 6, 500)], Block::Charcoal);
        w.ensure_chunk(500_i32.div_euclid(CHUNK_SIZE), 500_i32.div_euclid(CHUNK_SIZE));
        assert_eq!(w.block_at(500, 6, 500), Block::Charcoal);
    }

    #[test]
    fn a_pot_on_a_fire_stews_raw_meat_in_water_and_only_while_it_burns() {
        let (mut w, fire) = burning_world();
        assert!(w.can_place_pot(fire) && w.place_pot(fire) && !w.place_pot(fire));
        assert!(!w.lay_on_fire(fire, ItemKind::Venison), "the pot has the fire's one place");
        assert!(!w.add_to_pot(fire, ItemKind::Venison), "no water yet");
        assert!(w.fill_pot(fire) && !w.fill_pot(fire));
        assert!(!w.add_to_pot(fire, ItemKind::Leather) && !w.add_to_pot(fire, ItemKind::CookedVenison), "only raw meat");
        assert!(w.add_to_pot(fire, ItemKind::Venison) && !w.add_to_pot(fire, ItemKind::Poultry));
        assert!(!w.take_pot(fire), "not while it holds meat");
        w.update_fires(STEW_SECONDS - 1.0);
        assert_eq!(w.take_stew(fire), None);
        w.update_fires(1.0);
        assert_eq!(w.take_stew(fire), Some(ItemKind::VenisonStew));
        assert_eq!(w.pots[&fire], Pot::empty(), "the pot stays, empty");
        assert!(w.take_pot(fire) && w.pots.is_empty() && !w.take_pot(fire));
        // Without water nothing happens, however long the fire burns.
        assert!(w.place_pot(fire));
        w.update_fires(30.0);
        assert!(w.pots[&fire].water == false && w.pots[&fire].meat.is_none());
    }

    #[test]
    fn a_pot_goes_on_burning_logs_and_open_charcoal_only_and_never_over_a_piece_already_cooking() {
        let (mut w, log) = burning_world();
        assert!(w.lay_on_fire(log, ItemKind::Venison));
        assert!(!w.can_place_pot(log), "something is already cooking there");
        let (w2, forge) = forge_world();
        assert!(!w2.can_place_pot(forge), "not in a forge");
        let (mut w3, embers) = embers_world();
        assert!(w3.place_pot(embers));
        assert!(!w3.place_pot((9, 6, 9)), "nothing burns there");
    }

    #[test]
    fn a_fire_that_burns_out_drops_the_pot_and_what_was_in_it() {
        let (mut w, fire) = burning_world();
        w.place_pot(fire);
        w.fill_pot(fire);
        w.add_to_pot(fire, ItemKind::Poultry);
        w.update_fires(10.0);
        w.fires.insert(fire, 0.5);
        w.update_fires(1.0);
        let mut got = w.drain_released();
        got.sort_by_key(|&(_, k)| k.id());
        assert_eq!(got, vec![(fire, ItemKind::Poultry), (fire, ItemKind::TinPot)], "unfinished meat comes back raw");
        assert!(w.pots.is_empty());
        let (mut w, fire) = burning_world();
        w.place_pot(fire);
        w.fill_pot(fire);
        w.add_to_pot(fire, ItemKind::MooseMeat);
        w.update_fires(STEW_SECONDS);
        w.update_fires(60.0);
        let mut got = w.drain_released();
        got.sort_by_key(|&(_, k)| k.id());
        assert_eq!(got, vec![(fire, ItemKind::MooseStew), (fire, ItemKind::TinPot)], "a finished stew comes back as stew");
    }

    #[test]
    fn saved_pots_without_a_fire_are_discarded() {
        let fires = HashMap::from([((1, 6, 1), 30.0)]);
        let pot = Pot { water: true, meat: Some(ItemKind::Venison), left: 3.0 };
        let w = World::with_edits(1, HashSet::new(), HashMap::from([((1, 6, 1), Block::Fire)]), fires)
            .with_pots(HashMap::from([((1, 6, 1), pot), ((9, 6, 9), pot)]));
        assert_eq!(w.pots.len(), 1);
        assert!(w.pots.contains_key(&(1, 6, 1)));
    }

    #[test]
    fn a_boulders_collider_is_one_square_box_a_little_bigger_than_its_cell() {
        let mut w = empty_sky_world();
        put(&mut w, 10, 6, 10, Block::Boulder);
        let boxx = |x0: f32, x1: f32, z0: f32, z1: f32| w.box_blocked(vec3(x0, 6.1, z0), vec3(x1, 6.9, z1));
        // Past the cell's edge, within the overhang: blocked, on every side.
        assert!(boxx(9.70, 9.90, 10.3, 10.7), "west of the cell");
        assert!(boxx(11.10, 11.30, 10.3, 10.7), "east of the cell");
        assert!(boxx(10.3, 10.7, 9.70, 9.90), "north of the cell");
        assert!(boxx(10.3, 10.7, 11.10, 11.30), "south of the cell");
        // The collider is square: even a corner that the rounded shape leaves empty is solid.
        assert!(boxx(9.86, 9.9, 9.86, 9.9), "the cut corner still blocks");
        assert!(boxx(11.10, 11.14, 11.10, 11.14));
        // Beyond the overhang it is open, and the top is a standing place at height 1.
        assert!(!boxx(9.60, 9.80, 10.3, 10.7));
        assert!(!boxx(11.20, 11.40, 10.3, 10.7));
        assert!(!w.box_blocked(vec3(10.2, 7.0, 10.2), vec3(10.8, 8.8, 10.8)), "the player can stand on it");
        assert!(w.box_blocked(vec3(10.2, 6.95, 10.2), vec3(10.8, 8.75, 10.8)));
    }

    #[test]
    fn a_player_walking_into_a_boulder_stops_at_its_overhang_not_at_its_cell() {
        let mut w = empty_sky_world();
        put(&mut w, 10, 6, 10, Block::Boulder);
        let mut p = Player::new();
        // Yaw 0 faces +x, straight at the boulder's west face.
        p.pos = vec3(8.0, 6.0, 10.5);
        p.yaw = 0.0;
        let input = MoveInput { forward: true, ..MoveInput::default() };
        for _ in 0..180 {
            p.update(&w, 1.0 / 60.0, &input);
        }
        let face = 10.0 - BOULDER_OVERHANG;
        let front = p.pos.x + PLAYER_HALF_WIDTH;
        assert!(front <= face + 1e-3 && front > face - 0.2, "stopped with its front at {front}, the face is at {face}");
    }

    #[test]
    fn a_ray_can_hit_the_overhang_of_a_boulder_from_the_neighbouring_cell() {
        let mut w = empty_sky_world();
        put(&mut w, 10, 6, 10, Block::Boulder);
        // Along x at the boulder's height: the face lies at 9.85, in cell 9, so a short ray reaches it.
        let hit = w.raycast_hit(vec3(9.0, 6.5, 10.5), vec3(1.0, 0.0, 0.0), 0.9).expect("the overhang is hit");
        assert_eq!((hit.x, hit.y, hit.z, hit.block), (10, 6, 10, Block::Boulder));
        assert!(w.raycast_hit(vec3(9.0, 6.5, 10.5), vec3(1.0, 0.0, 0.0), 0.8).is_none(), "too short to reach it");
        // A ray that stays in cell 9 all the way (x = 9.9) still clips it at the z overhang.
        let hit = w.raycast_hit(vec3(9.9, 6.5, 8.0), vec3(0.0, 0.0, 1.0), 5.0).expect("clipped from the diagonal cell");
        assert_eq!((hit.x, hit.z, hit.block), (10, 10, Block::Boulder));
        // Just outside the overhang it passes.
        assert!(w.raycast_hit(vec3(9.8, 6.5, 8.0), vec3(0.0, 0.0, 1.0), 5.0).is_none());
        // And over the top it passes too.
        assert!(w.raycast_hit(vec3(9.0, 7.2, 10.5), vec3(1.0, 0.0, 0.0), 5.0).is_none());
    }

    #[test]
    fn a_closer_block_is_still_hit_before_a_boulders_overhang_behind_it() {
        let mut w = empty_sky_world();
        put(&mut w, 10, 6, 10, Block::Boulder);
        put(&mut w, 8, 6, 10, Block::Dirt);
        let hit = w.raycast_hit(vec3(5.5, 6.5, 10.5), vec3(1.0, 0.0, 0.0), 10.0).unwrap();
        assert_eq!((hit.x, hit.block), (8, Block::Dirt));
    }
}
