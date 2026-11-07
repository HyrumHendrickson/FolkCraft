//! The Skylands: a third dimension, reached by standing in the cyclone that appears on the summit once the Yeti is dead.
//!
//! It is a sky of floating islands and nothing else: no ground, no water, no creatures and no landmarks. The land is a
//! pure function of the world seed, as everywhere else. The islands lie on a grid of `SKY_CELL` blocks, one to a cell,
//! each a lobed disc of grass over a tapering underside of rock; the first, where the player arrives, is at
//! `SKY_SPAWN_CENTRE`. Every island is joined to its east neighbour by a wooden bridge, and to its south neighbour by
//! one (always in column 0, and otherwise `SKY_SOUTH_BRIDGE_PERCENT` of the time), so every island can be reached
//! from the first. A bridge is a deck of `Block::BridgePlank`, `SKY_BRIDGE_WIDTH` wide, climbing from the top of one
//! island to the top of the other, with a post now and then along its edges.
//!
//! Islands are of four kinds (`IslandKind`): wooded, with ancient ruins, with a hill, or with an ancient altar and some bones. One in
//! `SKY_LARGE_ONE_IN` is the large form of its kind. Far away, in place of a block of nine cells, lies the giant island of the Phoenix
//! (`giant_island`): about four times as wide, with a volcano on it (`volcano_crater`).
//!
//! The Skylands have their own day and night. A player who falls below `SKY_VOID_Y` has fallen off (see
//! `Game::update_skylands`).

use macroquad::prelude::*;

use crate::block::Block;
use crate::chunk::idx;
use crate::config::*;
use crate::landmarks::Landmarks;
use crate::rng::{hash3, Rng};
use crate::terrain::place_oak;
use crate::tower::{self, Relic};

/// One floating island: a disc round (`x`, `z`) with a wavy shore, flat on top at `top` and tapering to a point below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Island {
    pub x: i32,
    pub z: i32,
    /// The nominal radius; the shore wanders about 19 percent either way (`Island::reach`).
    pub radius: f32,
    /// y of the highest block (grass on dirt).
    pub top: i32,
    /// How many blocks of rock hang below the top at the middle.
    pub depth: i32,
    /// What is on it (see `IslandKind`), and whether it is the large form.
    pub kind: IslandKind,
    pub large: bool,
    /// Decides the shape of the shore.
    phase: u64,
}

/// What an island has on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IslandKind {
    /// Grass and a few oaks.
    Wooded,
    /// A broken ring of wall, standing pillars and rubble.
    Ruins,
    /// A smooth hill in the middle.
    Hill,
    /// An ancient altar on a dais with nothing on it, and bones lying about.
    Altar,
    /// The giant island of the Phoenix, with its volcano.
    Volcano,
}

impl Island {
    /// How far from the middle the shore lies in the direction `angle` (radians).
    pub fn reach(&self, angle: f32) -> f32 {
        let (p1, p2) = ((self.phase & 0xFF) as f32 / 255.0 * std::f32::consts::TAU, ((self.phase >> 8) & 0xFF) as f32 / 255.0 * std::f32::consts::TAU);
        self.radius * (1.0 + 0.12 * (3.0 * angle + p1).sin() + 0.07 * (5.0 * angle + p2).sin())
    }

    /// Blocks from the middle of the column (wx, wz) to the middle of the island, and as a share of the shore's reach
    /// in that direction (below 1 on the island).
    fn share(&self, wx: i32, wz: i32) -> f32 {
        let (dx, dz) = (wx as f32 + 0.5 - (self.x as f32 + 0.5), wz as f32 + 0.5 - (self.z as f32 + 0.5));
        let dist = (dx * dx + dz * dz).sqrt();
        dist / self.reach(dz.atan2(dx)).max(1.0)
    }

    /// The lowest block of the island in the column (wx, wz), or `None` off the island: the rock tapers from `depth`
    /// blocks thick at the middle to nothing at the shore.
    pub fn bottom_at(&self, wx: i32, wz: i32) -> Option<i32> {
        let t = self.share(wx, wz);
        (t < 1.0).then(|| self.top - (self.depth as f32 * (1.0 - t * t)) as i32)
    }

    /// Whether the column (wx, wz) is on the island.
    pub fn contains(&self, wx: i32, wz: i32) -> bool {
        self.share(wx, wz) < 1.0
    }

    /// How many blocks above `top` the ground of the column (wx, wz) rises: a hill's slope, a volcano's cone and crater, else nothing.
    pub fn rise_at(&self, wx: i32, wz: i32) -> i32 {
        match self.kind {
            IslandKind::Hill => {
                let t = (1.0 - self.share(wx, wz) / SKY_HILL_SHARE).clamp(0.0, 1.0);
                (SKY_HILL_HEIGHT as f32 * t * t * (3.0 - 2.0 * t)).round() as i32
            }
            IslandKind::Volcano => {
                let d = vec2(wx as f32 + 0.5 - (self.x as f32 + 0.5), wz as f32 + 0.5 - (self.z as f32 + 0.5)).length();
                volcano_rise(d)
            }
            _ => 0,
        }
    }

    /// y of the highest block of the island in the column (wx, wz).
    pub fn surface_y(&self, wx: i32, wz: i32) -> i32 {
        self.top + self.rise_at(wx, wz)
    }
}

/// The height of the volcano above the island's top at `d` blocks from its middle: a cone `VOLCANO_RADIUS` wide at its foot and
/// `VOLCANO_HEIGHT` high, with a bowl-shaped crater `VOLCANO_CRATER_RADIUS` wide in the top whose floor lies `VOLCANO_FLOOR` up.
pub fn volcano_rise(d: f32) -> i32 {
    if d >= VOLCANO_RADIUS {
        return 0;
    }
    let rim = (VOLCANO_HEIGHT as f32 * (1.0 - VOLCANO_CRATER_RADIUS / VOLCANO_RADIUS)).round();
    if d >= VOLCANO_CRATER_RADIUS {
        return (VOLCANO_HEIGHT as f32 * (1.0 - d / VOLCANO_RADIUS)).round() as i32;
    }
    let floor = VOLCANO_FLOOR as f32;
    let t = d / VOLCANO_CRATER_RADIUS;
    (floor + (rim - floor) * t * t).round() as i32
}

/// The cell of the grid at the middle of the block of nine that the giant island takes the place of: a few cells from the first island,
/// in a direction the seed decides.
fn giant_cell(seed: i64) -> (i32, i32) {
    let angle = (hash3(seed, 0, 0, 0x9F0E) % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
    let cells = (PHOENIX_ISLAND_DISTANCE / SKY_CELL as f32).round();
    ((angle.cos() * cells).round() as i32, (angle.sin() * cells).round() as i32)
}

/// The giant island of the Phoenix, in the middle of its block of nine cells (every cell of the block is this island).
pub fn giant_island(seed: i64) -> Island {
    let (cx, cz) = giant_cell(seed);
    Island {
        x: cx * SKY_CELL,
        z: cz * SKY_CELL,
        radius: PHOENIX_ISLAND_RADIUS,
        top: SKY_BASE_Y,
        depth: PHOENIX_ISLAND_DEPTH,
        kind: IslandKind::Volcano,
        large: true,
        phase: hash3(seed, 0, 0, 0x9F0F) >> 20,
    }
}

/// Whether the grid cell (cx, cz) belongs to the giant island's block of nine.
fn in_giant_block(seed: i64, cx: i32, cz: i32) -> bool {
    let (gx, gz) = giant_cell(seed);
    (cx - gx).abs() <= 1 && (cz - gz).abs() <= 1
}

/// Where the Phoenix lives: the floor of the volcano's crater, standing height above it.
pub fn volcano_crater(seed: i64) -> Vec3 {
    let g = giant_island(seed);
    vec3(g.x as f32 + 0.5, (g.top + VOLCANO_FLOOR) as f32 + 1.01, g.z as f32 + 0.5)
}

/// The point over the giant island a peep stone leads to: the foot of the volcano on its side towards the first island, on the
/// ground (the trail itself is drawn by `Game::trail_path`).
pub fn volcano_foot(seed: i64) -> (f32, f32) {
    let g = giant_island(seed);
    let (dx, dz) = (SKY_SPAWN_CENTRE.0 - g.x, SKY_SPAWN_CENTRE.1 - g.z);
    let len = ((dx * dx + dz * dz) as f32).sqrt().max(1.0);
    (g.x as f32 + 0.5 + dx as f32 / len * VOLCANO_RADIUS * 0.8, g.z as f32 + 0.5 + dz as f32 / len * VOLCANO_RADIUS * 0.8)
}

/// The number of the grid cell that holds the world coordinate `w` (cell 0 is centred on 0).
fn cell_of(w: i32) -> i32 {
    (w + SKY_CELL / 2).div_euclid(SKY_CELL)
}

/// The islands whose grid cell lies within `radius` blocks (a square) of the column (x, z), each once (the giant island fills nine cells).
pub fn islands_near(seed: i64, x: i32, z: i32, radius: i32) -> Vec<Island> {
    let mut out: Vec<Island> = Vec::new();
    for cz in cell_of(z - radius)..=cell_of(z + radius) {
        for cx in cell_of(x - radius)..=cell_of(x + radius) {
            let island = island_in_cell(seed, cx, cz);
            if !out.iter().any(|i| (i.x, i.z) == (island.x, island.z)) {
                out.push(island);
            }
        }
    }
    out
}

/// The island of grid cell (cx, cz). Cell (0, 0) holds the first island, at a fixed place and height.
pub fn island_in_cell(seed: i64, cx: i32, cz: i32) -> Island {
    let h = hash3(seed, cx, cz, 0x5C1A);
    if (cx, cz) == (0, 0) {
        return Island {
            x: SKY_SPAWN_CENTRE.0,
            z: SKY_SPAWN_CENTRE.1,
            radius: SKY_SPAWN_RADIUS,
            top: SKY_BASE_Y,
            depth: (SKY_ISLAND_MAX_DEPTH + SKY_ISLAND_MIN_DEPTH) / 2,
            kind: IslandKind::Wooded,
            large: false,
            phase: h >> 20,
        };
    }
    if in_giant_block(seed, cx, cz) {
        return giant_island(seed);
    }
    let unit = |shift: u32| ((h >> shift) & 0xFFFF) as f32 / 65535.0;
    let jitter = |u: f32| ((u * 2.0 - 1.0) * SKY_ISLAND_JITTER as f32).round() as i32;
    let large = hash3(seed, cx, cz, 0x5C1F) % SKY_LARGE_ONE_IN == 0;
    let kind = match hash3(seed, cx, cz, 0x5C20) % 4 {
        0 => IslandKind::Wooded,
        1 => IslandKind::Ruins,
        2 => IslandKind::Hill,
        _ => IslandKind::Altar,
    };
    Island {
        x: cx * SKY_CELL + jitter(unit(0)),
        z: cz * SKY_CELL + jitter(unit(16)),
        radius: (SKY_ISLAND_MIN_RADIUS + unit(32) * (SKY_ISLAND_MAX_RADIUS - SKY_ISLAND_MIN_RADIUS)) * if large { SKY_LARGE_FACTOR } else { 1.0 },
        kind,
        large,
        top: SKY_BASE_Y + ((unit(48) * 2.0 - 1.0) * SKY_HEIGHT_JITTER as f32).round() as i32,
        depth: SKY_ISLAND_MIN_DEPTH + (hash3(seed, cx, cz, 0x5C1B) % (SKY_ISLAND_MAX_DEPTH - SKY_ISLAND_MIN_DEPTH + 1) as u64) as i32,
        phase: h >> 20,
    }
}

/// The island, if any, under the column (wx, wz).
pub fn island_at(seed: i64, wx: i32, wz: i32) -> Option<Island> {
    let (cx, cz) = (cell_of(wx), cell_of(wz));
    (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (cx + dx, cz + dz))).map(|(x, z)| island_in_cell(seed, x, z)).find(|i| i.contains(wx, wz))
}

/// The grid cell of the island under the column (wx, wz), if there is one.
pub fn island_cell_under(seed: i64, wx: i32, wz: i32) -> Option<(i32, i32)> {
    island_at(seed, wx, wz).map(|i| (cell_of(i.x), cell_of(i.z)))
}

/// The island that carries the tower: the nearest to the column `at` (in a grid cell within a few cells of it) that the player
/// has not walked on, is not the first island, and none of whose chunks have been made yet (`made` says whether a chunk has),
/// so that it is the next one to appear. `None` if there is no such island.
pub fn pick_tower_cell(seed: i64, at: (i32, i32), visited: &[(i32, i32)], made: &dyn Fn(i32, i32) -> bool) -> Option<(i32, i32)> {
    let (cx, cz) = (cell_of(at.0), cell_of(at.1));
    let mut best: Option<((i32, i32), i64)> = None;
    for dz in -8..=8 {
        for dx in -8..=8 {
            let cell = (cx + dx, cz + dz);
            if cell == (0, 0) || visited.contains(&cell) || in_giant_block(seed, cell.0, cell.1) {
                continue;
            }
            let island = island_in_cell(seed, cell.0, cell.1);
            // The tower needs level ground: a wooded island (which is flat) and nothing else.
            if island.kind != IslandKind::Wooded {
                continue;
            }
            let reach = (tower::TOWER_FOOT_MARGIN + island.radius.ceil() as i32) as i32;
            let chunks_made = (island.z - reach).div_euclid(CHUNK_SIZE)..=(island.z + reach).div_euclid(CHUNK_SIZE);
            if chunks_made.into_iter().any(|kz| ((island.x - reach).div_euclid(CHUNK_SIZE)..=(island.x + reach).div_euclid(CHUNK_SIZE)).any(|kx| made(kx, kz))) {
                continue;
            }
            let d = ((island.x - at.0) as i64).pow(2) + ((island.z - at.1) as i64).pow(2);
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((cell, d));
            }
        }
    }
    best.map(|(c, _)| c)
}

/// A bridge between two islands: the cells at its ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bridge {
    pub from: Island,
    pub to: Island,
}

/// The bridge from the island of cell (cx, cz) to its east neighbour (there always is one).
pub fn east_bridge(seed: i64, cx: i32, cz: i32) -> Bridge {
    Bridge { from: island_in_cell(seed, cx, cz), to: island_in_cell(seed, cx + 1, cz) }
}

/// The bridge from the island of cell (cx, cz) to its south neighbour, if it has one.
pub fn south_bridge(seed: i64, cx: i32, cz: i32) -> Option<Bridge> {
    let wanted = cx == 0 || hash3(seed, cx, cz, 0x5C1C) % 100 < SKY_SOUTH_BRIDGE_PERCENT;
    wanted.then(|| Bridge { from: island_in_cell(seed, cx, cz), to: island_in_cell(seed, cx, cz + 1) })
}

/// What a bridge puts in one column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deck {
    /// y of the deck block.
    pub y: i32,
    /// Whether a post stands on the deck here (on an edge column, every fourth block along the bridge).
    pub post: bool,
}

impl Bridge {
    /// The deck of the bridge in the column (wx, wz) by day, `None` if the bridge does not cross it or the column is on one of
    /// its two islands (which are drawn instead). The deck runs level with the top of the island at each end and climbs
    /// in between.
    #[cfg(test)]
    pub fn deck_at(&self, wx: i32, wz: i32) -> Option<Deck> {
        self.deck_at_time(wx, wz, false)
    }

    /// `deck_at`, by day or by night. By night the bridge is broken: only a short ragged stub of `SKY_BROKEN_STUB` blocks is left
    /// at each end, and the middle is gone. A bridge shorter than `SKY_BROKEN_MIN_DECK` has no stubs at all, so that what is left
    /// can never be jumped across.
    pub fn deck_at_time(&self, wx: i32, wz: i32, night: bool) -> Option<Deck> {
        if self.from == self.to {
            // Both ends are the giant island: there is no bridge between a cell of its block and another.
            return None;
        }
        let (a, b) = ((self.from.x as f32 + 0.5, self.from.z as f32 + 0.5), (self.to.x as f32 + 0.5, self.to.z as f32 + 0.5));
        let (px, pz) = (wx as f32 + 0.5, wz as f32 + 0.5);
        let (ax, az) = (b.0 - a.0, b.1 - a.1);
        let len = (ax * ax + az * az).sqrt();
        let (ux, uz) = (ax / len, az / len);
        let along = (px - a.0) * ux + (pz - a.1) * uz;
        let across = ((px - a.0) * -uz + (pz - a.1) * ux).abs();
        if across > SKY_BRIDGE_WIDTH / 2.0 || along < 0.0 || along > len || self.from.contains(wx, wz) || self.to.contains(wx, wz) {
            return None;
        }
        // The deck leaves the shore of the first island and arrives at the shore of the second.
        let start = self.from.reach(az.atan2(ax));
        let end = len - self.to.reach((-az).atan2(-ax));
        if night {
            let stub = if end - start >= SKY_BROKEN_MIN_DECK { SKY_BROKEN_STUB } else { 0.0 };
            let from_end = (along - start).min(end - along);
            let ragged = from_end > stub - 1.0 && (wx.wrapping_mul(7).wrapping_add(wz.wrapping_mul(13))).rem_euclid(2) == 0;
            if from_end > stub || ragged {
                return None;
            }
        }
        let t = ((along - start) / (end - start).max(1.0)).clamp(0.0, 1.0);
        let y = self.from.top + ((self.to.top - self.from.top) as f32 * t).round() as i32;
        Some(Deck { y, post: across >= SKY_BRIDGE_WIDTH / 2.0 - 0.6 && (along.floor() as i32).rem_euclid(4) == 0 })
    }
}

/// The deck, if any, that the bridges lay over the column (wx, wz), whole by day and broken by night.
pub fn deck_at(seed: i64, wx: i32, wz: i32, night: bool) -> Option<Deck> {
    let (cx, cz) = (cell_of(wx), cell_of(wz));
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (x, z) = (cx + dx, cz + dz);
            if let Some(deck) = east_bridge(seed, x, z).deck_at_time(wx, wz, night) {
                return Some(deck);
            }
            if let Some(deck) = south_bridge(seed, x, z).and_then(|b| b.deck_at_time(wx, wz, night)) {
                return Some(deck);
            }
        }
    }
    None
}

/// y of the highest block of the land in the column (wx, wz) (the top of an island, or a bridge's deck), `None` over the
/// void.
pub fn surface_top(seed: i64, wx: i32, wz: i32, night: bool) -> Option<i32> {
    island_at(seed, wx, wz).map(|i| i.surface_y(wx, wz)).or_else(|| deck_at(seed, wx, wz, night).map(|d| d.y))
}

/// Where a player who stands over the column (wx, wz) can be put when the bridge under them has just broken (nightfall): the
/// nearest column within `reach` blocks that still has footing, standing on it. `None` if there is none.
pub fn nearest_footing(seed: i64, wx: i32, wz: i32, night: bool, reach: i32) -> Option<Vec3> {
    (1..=reach).find_map(|r| {
        (-r..=r)
            .flat_map(|dz| (-r..=r).map(move |dx| (dx, dz)))
            .filter(|&(dx, dz)| dx.abs().max(dz.abs()) == r)
            .filter_map(|(dx, dz)| surface_top(seed, wx + dx, wz + dz, night).map(|top| (dx * dx + dz * dz, wx + dx, wz + dz, top)))
            .min_by_key(|c| c.0)
            .map(|(_, x, z, top)| vec3(x as f32 + 0.5, top as f32 + 1.01, z as f32 + 0.5))
    })
}

/// Where the player arrives: the middle of the first island, standing on its grass.
pub fn spawn_point() -> Vec3 {
    vec3(SKY_SPAWN_CENTRE.0 as f32 + 0.5, SKY_BASE_Y as f32 + 1.01, SKY_SPAWN_CENTRE.1 as f32 + 0.5)
}

/// Whether trees grow near (wx, wz): only on wooded islands, never on the stretch of the first island where the player arrives,
/// nor near any island's shore.
fn tree_site(seed: i64, wx: i32, wz: i32, island: &Island, landmarks: &Landmarks) -> bool {
    if landmarks.sky_tower == Some((cell_of(island.x), cell_of(island.z))) {
        return false;
    }
    let near = |p: Vec3| (p.x - wx as f32).abs() < 5.0 && (p.z - wz as f32).abs() < 5.0;
    island.kind == IslandKind::Wooded
        && island.share(wx, wz) < 0.6
        && !(*island == island_in_cell(seed, 0, 0) && near(spawn_point()))
        && hash3(seed, wx, wz, 0x5C1D) % 100 < SKY_TREE_PERCENT
}

/// What an island of a given kind has above its ground in the column (wx, wz), as (blocks above the ground, block) pairs: the
/// ruins' broken wall, pillars and rubble, the altar's dais and bones.
fn decoration(seed: i64, island: &Island, wx: i32, wz: i32) -> Vec<(i32, Block)> {
    let share = island.share(wx, wz);
    let h = hash3(seed, wx, wz, 0x5C30);
    let mut out = Vec::new();
    match island.kind {
        IslandKind::Ruins => {
            if (SKY_RUIN_RING_SHARE - 0.05..SKY_RUIN_RING_SHARE + 0.05).contains(&share) {
                // The wall is broken: some columns are gone, the rest are of different heights.
                let height = if h % 5 == 0 { 0 } else { 1 + ((h >> 8) % SKY_RUIN_WALL_HEIGHT as u64) as i32 };
                out.extend((1..=height).map(|dy| (dy, Block::CastleStone)));
            }
            for k in 0..SKY_RUIN_PILLARS {
                let angle = (island.phase & 0xFF) as f32 / 255.0 * std::f32::consts::TAU + k as f32 * std::f32::consts::TAU / SKY_RUIN_PILLARS as f32;
                let r = SKY_RUIN_RING_SHARE * island.reach(angle);
                let (px, pz) = ((island.x as f32 + 0.5 + angle.cos() * r).floor() as i32, (island.z as f32 + 0.5 + angle.sin() * r).floor() as i32);
                if (px, pz) == (wx, wz) {
                    let height = 3 + (hash3(seed, px, pz, 0x5C31) % 3) as i32;
                    out.extend((1..=height).map(|dy| (dy, Block::CastleStone)));
                }
            }
            if share < SKY_RUIN_RING_SHARE - 0.12 && h % 40 == 0 {
                out.push((1, Block::PlacedStone));
            }
        }
        IslandKind::Altar => {
            let (dx, dz) = (wx - island.x, wz - island.z);
            if dx.abs() <= 1 && dz.abs() <= 1 {
                // A low dais with the altar on it: nothing lies on the altar.
                out.push((1, Block::CastleStone));
                if (dx, dz) == (0, 0) {
                    out.push((2, Block::AncientAltar));
                }
            } else if share < 0.7 && h % 1000 < SKY_BONES_PER_MILLE {
                out.push((1, Block::Bones));
            }
        }
        IslandKind::Wooded | IslandKind::Hill | IslandKind::Volcano => {}
    }
    out
}

/// The block of an island's ground at height `y` in the column (wx, wz), whose surface is at `surface`: dirt over rock; on the volcano,
/// bare grey rock (no glowing blocks: the fire is the crater's tornado).
fn ground_block(island: &Island, wx: i32, wz: i32, y: i32, surface: i32) -> Block {
    if y <= surface - SKY_DIRT_LAYERS {
        return Block::Stone;
    }
    if island.kind != IslandKind::Volcano {
        return Block::Dirt;
    }
    let d = vec2(wx as f32 + 0.5 - (island.x as f32 + 0.5), wz as f32 + 0.5 - (island.z as f32 + 0.5)).length();
    if d >= VOLCANO_RADIUS { Block::Dirt } else { Block::Stone }
}

/// The blocks of chunk (cx, cz) of the Skylands: the islands (rock, dirt, a few oaks), and the bridges' decks and posts.
pub fn generate_chunk(seed: i64, cx: i32, cz: i32, landmarks: &Landmarks, night: bool) -> Vec<Block> {
    let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
    let mut trees = Vec::new();
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
            if let Some(island) = island_at(seed, wx, wz) {
                let bottom = island.bottom_at(wx, wz).unwrap_or(island.top);
                let surface = island.surface_y(wx, wz);
                for y in bottom..=surface {
                    blocks[idx(lx, y, lz)] = ground_block(&island, wx, wz, y, surface);
                }
                let tower_island = landmarks.sky_tower == Some((cell_of(island.x), cell_of(island.z)));
                if !tower_island {
                    for (dy, block) in decoration(seed, &island, wx, wz) {
                        blocks[idx(lx, surface + dy, lz)] = block;
                    }
                }
                if tower_island {
                    let (dx, dz) = (wx - island.x, wz - island.z);
                    let locked = !landmarks.is_unlocked(island.x, island.z);
                    for dy in 0..TOWER_HEIGHT {
                        if let Some(block) = tower::tower_cell(dx, dz, dy, locked, Relic::HornOfTheHunt) {
                            blocks[idx(lx, island.top + 1 + dy, lz)] = block;
                        }
                    }
                }
                if (3..CHUNK_SIZE - 3).contains(&lx) && (3..CHUNK_SIZE - 3).contains(&lz) && tree_site(seed, wx, wz, &island, landmarks) {
                    trees.push((lx, lz, island.top));
                }
            } else if let Some(deck) = deck_at(seed, wx, wz, night) {
                blocks[idx(lx, deck.y, lz)] = Block::BridgePlank;
                if deck.post {
                    blocks[idx(lx, deck.y + 1, lz)] = Block::BridgePost;
                }
            }
        }
    }
    let mut rng = Rng::new(hash3(seed, cx, cz, 0x5C1E));
    let mut planted: Vec<(i32, i32)> = Vec::new();
    for (lx, lz, top) in trees {
        if planted.iter().any(|&(px, pz)| (px - lx).abs() < 5 && (pz - lz).abs() < 5) {
            continue;
        }
        planted.push((lx, lz));
        place_oak(&mut blocks, &mut rng, lx, lz, top);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn the_first_island_is_where_the_player_arrives_and_is_flat_and_big() {
        let first = island_in_cell(7, 0, 0);
        assert_eq!((first.x, first.z, first.top), (SKY_SPAWN_CENTRE.0, SKY_SPAWN_CENTRE.1, SKY_BASE_Y));
        let p = spawn_point();
        assert_eq!(surface_top(7, p.x.floor() as i32, p.z.floor() as i32, false), Some(SKY_BASE_Y));
    }

    #[test]
    fn the_land_is_a_pure_function_of_the_seed_and_other_seeds_differ() {
        let sample = |seed: i64| (-200..200).map(|x| surface_top(seed, x, 11, false)).collect::<Vec<_>>();
        assert_eq!(sample(3), sample(3));
        assert_ne!(sample(3), sample(4));
    }

    #[test]
    fn islands_stay_in_their_cells_and_clear_of_one_another() {
        for seed in 0..6 {
            for cz in -4..4 {
                for cx in -4..4 {
                    if in_giant_block(seed, cx, cz) || in_giant_block(seed, cx + 1, cz) {
                        continue;
                    }
                    let i = island_in_cell(seed, cx, cz);
                    let reach = i.radius * 1.19;
                    let (x0, z0) = ((cx * SKY_CELL - SKY_CELL / 2) as f32, (cz * SKY_CELL - SKY_CELL / 2) as f32);
                    assert!(i.x as f32 - reach >= x0 && i.x as f32 + reach <= x0 + SKY_CELL as f32, "{i:?} leaves its cell");
                    assert!(i.z as f32 - reach >= z0 && i.z as f32 + reach <= z0 + SKY_CELL as f32, "{i:?} leaves its cell");
                    assert!(i.top - i.depth > SKY_VOID_Y as i32 + 10, "{i:?} hangs too low");
                    let east = island_in_cell(seed, cx + 1, cz);
                    let gap = ((east.x - i.x) as f32).hypot((east.z - i.z) as f32) - reach - east.radius * 1.19;
                    assert!(gap >= 6.0, "{gap} blocks between neighbours");
                }
            }
        }
    }

    #[test]
    fn an_island_is_grass_over_rock_that_tapers_to_the_shore() {
        let blocks = generate_chunk(5, 0, 0, &Landmarks::default(), false);
        let first = island_in_cell(5, 0, 0);
        let (lx, lz) = (first.x, first.z);
        assert_eq!(blocks[idx(lx, first.top, lz)], Block::Dirt);
        assert_eq!(blocks[idx(lx, first.top - SKY_DIRT_LAYERS, lz)], Block::Stone);
        assert_eq!(blocks[idx(lx, first.top + 1, lz)], Block::Air);
        let deepest = first.top - first.depth;
        assert_eq!(blocks[idx(lx, deepest, lz)], Block::Stone, "the rock hangs {} blocks below the top", first.depth);
        assert_eq!(blocks[idx(lx, deepest - 1, lz)], Block::Air);
        // Nothing at all lies beneath the underside of the island.
        for y in 0..deepest {
            assert_eq!(blocks[idx(lx, y, lz)], Block::Air);
        }
    }

    /// Whether a player could walk from the column `a` to the column `b` of the Skylands' surfaces, a step at a time.
    fn walkable(seed: i64, from: (i32, i32), to: (i32, i32)) -> bool {
        let mut seen: HashSet<(i32, i32)> = HashSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some((x, z)) = queue.pop_front() {
            if (x, z) == to {
                return true;
            }
            let here = surface_top(seed, x, z, false).unwrap();
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (x + dx, z + dz);
                if next.0.abs() > 180 || next.1.abs() > 180 || seen.contains(&next) {
                    continue;
                }
                if let Some(top) = surface_top(seed, next.0, next.1, false) {
                    // A player hops up one block and can drop down a block or two.
                    if top - here <= 1 && here - top <= 2 {
                        seen.insert(next);
                        queue.push_back(next);
                    }
                }
            }
        }
        false
    }

    #[test]
    fn bridges_join_the_first_island_to_every_neighbour_one_step_at_a_time() {
        for seed in [1, 5, 99, 4242] {
            let start = (SKY_SPAWN_CENTRE.0, SKY_SPAWN_CENTRE.1);
            for (cx, cz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (2, 0), (0, 2)] {
                let goal = island_in_cell(seed, cx, cz);
                assert!(walkable(seed, start, (goal.x, goal.z)), "seed {seed}: no way from the first island to cell ({cx}, {cz})");
            }
        }
    }

    #[test]
    fn a_bridge_deck_is_three_blocks_wide_and_has_posts_on_its_edges() {
        let seed = 8;
        let bridge = east_bridge(seed, 0, 0);
        let mut widths = Vec::new();
        let mid = ((bridge.from.x + bridge.to.x) / 2, (bridge.from.z + bridge.to.z) / 2);
        for dz in -4..=4 {
            if bridge.deck_at(mid.0, mid.1 + dz).is_some() {
                widths.push(dz);
            }
        }
        assert!(widths.len() >= 3 && widths.len() <= 4, "deck columns across the middle: {widths:?}");
        // Posts exist somewhere along the bridge, and only at the edge columns.
        let mut found = 0;
        for x in bridge.from.x..=bridge.to.x {
            for z in (bridge.from.z.min(bridge.to.z) - 6)..=(bridge.from.z.max(bridge.to.z) + 6) {
                found += bridge.deck_at(x, z).map_or(0, |d| d.post as i32);
            }
        }
        assert!(found >= 4, "{found} posts");
    }

    #[test]
    fn a_chunk_holds_decks_posts_and_trees_and_the_void_is_empty() {
        let (mut planks, mut posts, mut wood) = (0, 0, 0);
        for cz in -3..3 {
            for cx in -3..3 {
                for &b in generate_chunk(5, cx, cz, &Landmarks::default(), false).iter() {
                    planks += (b == Block::BridgePlank) as i32;
                    posts += (b == Block::BridgePost) as i32;
                    wood += (b == Block::Wood) as i32;
                }
            }
        }
        assert!(planks > 100 && posts > 4 && wood > 0, "planks {planks}, posts {posts}, wood {wood}");
        // Far from every island and bridge there is nothing.
        let void_found = (-50..50).any(|x| (-50..50).any(|z| surface_top(5, x * 3, z * 3, false).is_none()));
        assert!(void_found, "gaps between the islands are open sky");
    }

    #[test]
    fn trees_keep_off_the_arrival_spot() {
        for seed in 0..30 {
            let blocks = generate_chunk(seed, 0, 0, &Landmarks::default(), false);
            for p in [spawn_point()] {
                for dz in -2..=2 {
                    for dx in -2..=2 {
                        let (lx, lz) = (p.x.floor() as i32 + dx, p.z.floor() as i32 + dz);
                        if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&lz) {
                            for y in SKY_BASE_Y + 1..SKY_BASE_Y + 4 {
                                assert_eq!(blocks[idx(lx, y, lz)], Block::Air, "seed {seed}: something stands near {p:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn islands_are_of_four_kinds_and_one_in_three_is_large() {
        let mut kinds = HashSet::new();
        let (mut large, mut total) = (0, 0);
        for seed in 0..4 {
            for cz in -8..8 {
                for cx in -8..8 {
                    if (cx, cz) == (0, 0) || in_giant_block(seed, cx, cz) {
                        continue;
                    }
                    let i = island_in_cell(seed, cx, cz);
                    kinds.insert(format!("{:?}", i.kind));
                    total += 1;
                    large += i.large as i32;
                    let (lo, hi) = (SKY_ISLAND_MIN_RADIUS, SKY_ISLAND_MAX_RADIUS);
                    if i.large {
                        assert!(i.radius >= lo * SKY_LARGE_FACTOR - 0.01 && i.radius <= hi * SKY_LARGE_FACTOR + 0.01, "{i:?}");
                    } else {
                        assert!(i.radius >= lo - 0.01 && i.radius <= hi + 0.01, "{i:?}");
                    }
                }
            }
        }
        assert_eq!(kinds.len(), 4, "wooded, ruins, hill and altar islands: {kinds:?}");
        let share = large as f32 / total as f32;
        assert!((share - 1.0 / 3.0).abs() < 0.06, "about one in three is large: {share}");
        let first = island_in_cell(3, 0, 0);
        assert!(first.kind == IslandKind::Wooded && !first.large, "the first island is the ordinary wooded one");
    }

    /// Every block the chunks around an island hold, as (x, y, z, block).
    fn blocks_of(seed: i64, island: &Island) -> Vec<(i32, i32, i32, Block)> {
        let reach = island.radius.ceil() as i32 + 3;
        let mut out = Vec::new();
        for cz in (island.z - reach).div_euclid(CHUNK_SIZE)..=(island.z + reach).div_euclid(CHUNK_SIZE) {
            for cx in (island.x - reach).div_euclid(CHUNK_SIZE)..=(island.x + reach).div_euclid(CHUNK_SIZE) {
                let blocks = generate_chunk(seed, cx, cz, &Landmarks::default(), false);
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        for y in island.top + 1..island.top + 30 {
                            let b = blocks[idx(lx, y, lz)];
                            if b != Block::Air {
                                out.push((cx * CHUNK_SIZE + lx, y, cz * CHUNK_SIZE + lz, b));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    fn first_of_kind(seed: i64, kind: IslandKind) -> Island {
        (1..12).flat_map(|cx| (1..12).map(move |cz| (cx, cz))).map(|(cx, cz)| island_in_cell(seed, cx, cz)).find(|i| i.kind == kind && !in_giant_block(seed, (i.x as f32 / SKY_CELL as f32).round() as i32, (i.z as f32 / SKY_CELL as f32).round() as i32)).unwrap()
    }

    #[test]
    fn a_ruins_island_has_a_broken_wall_and_pillars_a_hill_island_a_hill_and_an_altar_island_an_empty_altar_and_bones() {
        let seed = 11;
        let ruins = blocks_of(seed, &first_of_kind(seed, IslandKind::Ruins));
        assert!(ruins.iter().filter(|b| b.3 == Block::CastleStone).count() > 15, "wall and pillars of old stone");
        let island = first_of_kind(seed, IslandKind::Ruins);
        let tallest = ruins.iter().filter(|b| b.3 == Block::CastleStone).map(|b| b.1 - island.top).max().unwrap();
        assert!(tallest >= 3, "pillars stand taller than the wall: {tallest}");
        let hill = first_of_kind(seed, IslandKind::Hill);
        let middle = hill.surface_y(hill.x, hill.z);
        assert!(middle >= hill.top + SKY_HILL_HEIGHT - 1, "a hill rises in the middle: {middle} over {}", hill.top);
        assert_eq!(hill.surface_y(hill.x + hill.radius as i32 + 2, hill.z), hill.top, "and the shore is level");
        let altar = first_of_kind(seed, IslandKind::Altar);
        let blocks = blocks_of(seed, &altar);
        let on_altar = blocks.iter().find(|b| b.3 == Block::AncientAltar).expect("an altar");
        assert_eq!((on_altar.0, on_altar.2), (altar.x, altar.z));
        assert!(!blocks.iter().any(|b| b.0 == altar.x && b.2 == altar.z && b.1 > on_altar.1), "nothing lies on the altar");
        assert!(blocks.iter().filter(|b| b.3 == Block::Bones).count() >= 3, "bones lie on the ground");
        let wooded = blocks_of(seed, &first_of_kind(seed, IslandKind::Wooded));
        assert!(wooded.iter().any(|b| b.3 == Block::Wood), "a wooded island has trees");
        assert!(!ruins.iter().any(|b| b.3 == Block::Wood), "and the others have none");
    }

    #[test]
    fn every_hill_slope_can_be_walked_up_a_block_at_a_time() {
        for seed in 0..6 {
            for cx in 1..10 {
                let i = island_in_cell(seed, cx, 2);
                if i.kind != IslandKind::Hill || in_giant_block(seed, cx, 2) {
                    continue;
                }
                for dz in -20..20 {
                    for dx in -20..20 {
                        let (x, z) = (i.x + dx, i.z + dz);
                        if i.contains(x, z) && i.contains(x + 1, z) {
                            assert!((i.surface_y(x, z) - i.surface_y(x + 1, z)).abs() <= 1, "seed {seed}: a step of more than one block at ({x}, {z})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_giant_island_is_about_four_times_as_wide_with_a_volcano_and_a_crater_and_is_reached_by_bridges() {
        for seed in [1, 7, 42] {
            let g = giant_island(seed);
            assert_eq!(g.kind, IslandKind::Volcano);
            assert!(g.radius >= 3.0 * SKY_ISLAND_MAX_RADIUS && g.radius <= 5.0 * SKY_ISLAND_MAX_RADIUS, "about four times a normal island's width: {}", g.radius);
            let (cx, cz) = giant_cell(seed);
            assert!(cx.abs() >= 3 || cz.abs() >= 3, "far from the first island");
            assert_eq!(island_in_cell(seed, cx + 1, cz - 1), g, "it stands in place of the nine cells round it");
            let crater = volcano_crater(seed);
            assert_eq!(surface_top(seed, crater.x.floor() as i32, crater.z.floor() as i32, false), Some(g.top + VOLCANO_FLOOR), "the Phoenix's floor");
            let rim = (0..40).map(|d| g.surface_y(g.x + d, g.z)).max().unwrap();
            assert!(rim > g.top + VOLCANO_FLOOR + 4 && rim <= g.top + VOLCANO_HEIGHT, "the cone rises round the crater: {rim}");
            // The cone climbs a block at a time (steps of at most one) from the foot to the rim.
            for d in 0..(VOLCANO_RADIUS as i32 + 3) {
                assert!((g.surface_y(g.x + d, g.z) - g.surface_y(g.x + d + 1, g.z)).abs() <= 2, "seed {seed}: the slope at {d}");
            }
            // The first island is bridged on towards it: there is a deck leaving the giant island on every side that has a neighbour.
            let outside = island_in_cell(seed, cx + 2, cz);
            assert_ne!(outside, g);
            assert!(east_bridge(seed, cx + 1, cz).from == g && east_bridge(seed, cx + 1, cz).to == outside);
        }
    }

    #[test]
    fn the_volcano_is_plain_grey_rock_with_no_red_blocks() {
        let seed = 5;
        let g = giant_island(seed);
        let (cx, cz) = (g.x.div_euclid(CHUNK_SIZE), g.z.div_euclid(CHUNK_SIZE));
        let blocks = generate_chunk(seed, cx, cz, &Landmarks::default(), false);
        let (lx, lz) = (g.x.rem_euclid(CHUNK_SIZE), g.z.rem_euclid(CHUNK_SIZE));
        assert_eq!(blocks[idx(lx, g.top + VOLCANO_FLOOR, lz)], Block::Stone, "the floor of the crater is rock");
        assert_eq!(blocks[idx(lx, g.top + VOLCANO_FLOOR + 1, lz)], Block::Air, "open above it");
        let slope_x = (g.x + (VOLCANO_CRATER_RADIUS as i32 + 3)).rem_euclid(CHUNK_SIZE);
        let slope_chunk = (g.x + VOLCANO_CRATER_RADIUS as i32 + 3).div_euclid(CHUNK_SIZE);
        let blocks = generate_chunk(seed, slope_chunk, cz, &Landmarks::default(), false);
        let top = surface_top(seed, g.x + VOLCANO_CRATER_RADIUS as i32 + 3, g.z, false).unwrap();
        assert!(matches!(blocks[idx(slope_x, top, lz)], Block::Stone), "bare rock on the slopes");
    }

    #[test]
    fn the_tower_island_is_always_a_flat_wooded_one() {
        for seed in 0..5 {
            if let Some(cell) = pick_tower_cell(seed, (0, 0), &[], &|_, _| false) {
                assert_eq!(island_in_cell(seed, cell.0, cell.1).kind, IslandKind::Wooded);
            }
        }
    }

    /// The decks of every bridge that leaves the island of cell (cx, cz) to the east or south, by day or night.
    fn bridges_near(seed: i64) -> Vec<Bridge> {
        let mut out = Vec::new();
        for cz in -3..4 {
            for cx in -3..4 {
                out.push(east_bridge(seed, cx, cz));
                out.extend(south_bridge(seed, cx, cz));
            }
        }
        out.into_iter().filter(|b| b.from != b.to).collect()
    }

    #[test]
    fn by_night_every_bridge_is_broken_and_cannot_be_crossed() {
        for seed in 0..4 {
            for bridge in bridges_near(seed) {
                let (a, b) = ((bridge.from.x, bridge.from.z), (bridge.to.x, bridge.to.z));
                let (x0, x1, z0, z1) = (a.0.min(b.0) - 20, a.0.max(b.0) + 20, a.1.min(b.1) - 20, a.1.max(b.1) + 20);
                // Walk from the first island over deck columns (as a player does, steps of one block) and see whether the second is reached.
                let mut seen = HashSet::new();
                let mut queue = VecDeque::new();
                for z in z0..=z1 {
                    for x in x0..=x1 {
                        if bridge.from.contains(x, z) {
                            seen.insert((x, z));
                            queue.push_back((x, z));
                        }
                    }
                }
                let mut crossed = false;
                while let Some((x, z)) = queue.pop_front() {
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let n = (x + dx, z + dz);
                        if n.0 < x0 || n.0 > x1 || n.1 < z0 || n.1 > z1 || seen.contains(&n) {
                            continue;
                        }
                        if bridge.to.contains(n.0, n.1) {
                            crossed = true;
                        } else if bridge.deck_at_time(n.0, n.1, true).is_some() {
                            seen.insert(n);
                            queue.push_back(n);
                        }
                    }
                }
                assert!(!crossed, "seed {seed}: a bridge can still be crossed by night");
            }
        }
    }

    #[test]
    fn a_player_over_a_broken_deck_is_set_on_the_nearest_footing() {
        let seed = 3;
        let bridge = bridges_near(seed).into_iter().find(|b| (((b.to.x - b.from.x).pow(2) + (b.to.z - b.from.z).pow(2)) as f32).sqrt() > 40.0).unwrap();
        let mid = ((bridge.from.x + bridge.to.x) / 2, (bridge.from.z + bridge.to.z) / 2);
        let (x, z) = (mid.0, mid.1);
        assert!(deck_at(seed, x, z, false).is_some() && deck_at(seed, x, z, true).is_none(), "the middle of the bridge is gone by night");
        let spot = nearest_footing(seed, x, z, true, 40).expect("some footing");
        assert!(surface_top(seed, spot.x.floor() as i32, spot.z.floor() as i32, true).is_some());
    }

    #[test]
    fn by_night_a_bridge_keeps_only_stubs_at_its_ends_and_by_day_it_is_whole() {
        let bridge = bridges_near(3).into_iter().find(|b| {
            let len = (((b.to.x - b.from.x).pow(2) + (b.to.z - b.from.z).pow(2)) as f32).sqrt();
            len > 40.0
        }).expect("a long bridge");
        let (mut day, mut night) = (0, 0);
        for z in -400..400 {
            for x in -400..400 {
                day += bridge.deck_at_time(x, z, false).is_some() as i32;
                night += bridge.deck_at_time(x, z, true).is_some() as i32;
                if bridge.deck_at_time(x, z, true).is_some() {
                    assert!(bridge.deck_at_time(x, z, false).is_some(), "a broken bridge only loses planks");
                }
            }
        }
        assert!(day > 0 && night > 0 && night * 3 < day, "stubs remain, the rest is gone: day {day}, night {night}");
    }

    #[test]
    fn night_chunks_have_no_whole_bridge_and_the_islands_are_unchanged() {
        let seed = 5;
        let bridge = bridges_near(seed).into_iter().next().unwrap();
        let (cx, cz) = ((bridge.from.x + bridge.to.x) / 2 / CHUNK_SIZE, (bridge.from.z + bridge.to.z) / 2 / CHUNK_SIZE);
        let planks = |night: bool| generate_chunk(seed, cx, cz, &Landmarks::default(), night).iter().filter(|b| **b == Block::BridgePlank).count();
        assert!(planks(true) <= planks(false));
        let first = generate_chunk(seed, 0, 0, &Landmarks::default(), false);
        let first_night = generate_chunk(seed, 0, 0, &Landmarks::default(), true);
        let ground = |b: &Vec<Block>| b.iter().filter(|x| matches!(x, Block::Dirt | Block::Stone)).count();
        assert_eq!(ground(&first), ground(&first_night), "islands do not change with the time of day");
    }
}
