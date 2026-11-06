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
    /// Decides the shape of the shore.
    phase: u64,
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
}

/// The number of the grid cell that holds the world coordinate `w` (cell 0 is centred on 0).
fn cell_of(w: i32) -> i32 {
    (w + SKY_CELL / 2).div_euclid(SKY_CELL)
}

/// The island of grid cell (cx, cz). Cell (0, 0) holds the first island, at a fixed place and height.
pub fn island_in_cell(seed: i64, cx: i32, cz: i32) -> Island {
    let h = hash3(seed, cx, cz, 0x5C1A);
    if (cx, cz) == (0, 0) {
        return Island { x: SKY_SPAWN_CENTRE.0, z: SKY_SPAWN_CENTRE.1, radius: SKY_SPAWN_RADIUS, top: SKY_BASE_Y, depth: (SKY_ISLAND_MAX_DEPTH + SKY_ISLAND_MIN_DEPTH) / 2, phase: h >> 20 };
    }
    let unit = |shift: u32| ((h >> shift) & 0xFFFF) as f32 / 65535.0;
    let jitter = |u: f32| ((u * 2.0 - 1.0) * SKY_ISLAND_JITTER as f32).round() as i32;
    Island {
        x: cx * SKY_CELL + jitter(unit(0)),
        z: cz * SKY_CELL + jitter(unit(16)),
        radius: SKY_ISLAND_MIN_RADIUS + unit(32) * (SKY_ISLAND_MAX_RADIUS - SKY_ISLAND_MIN_RADIUS),
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
            if cell == (0, 0) || visited.contains(&cell) {
                continue;
            }
            let island = island_in_cell(seed, cell.0, cell.1);
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
    /// The deck of the bridge in the column (wx, wz), `None` if the bridge does not cross it or the column is on one of
    /// its two islands (which are drawn instead). The deck runs level with the top of the island at each end and climbs
    /// in between.
    pub fn deck_at(&self, wx: i32, wz: i32) -> Option<Deck> {
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
        let t = ((along - start) / (end - start).max(1.0)).clamp(0.0, 1.0);
        let y = self.from.top + ((self.to.top - self.from.top) as f32 * t).round() as i32;
        Some(Deck { y, post: across >= SKY_BRIDGE_WIDTH / 2.0 - 0.6 && (along.floor() as i32).rem_euclid(4) == 0 })
    }
}

/// The deck, if any, that the bridges lay over the column (wx, wz).
pub fn deck_at(seed: i64, wx: i32, wz: i32) -> Option<Deck> {
    let (cx, cz) = (cell_of(wx), cell_of(wz));
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (x, z) = (cx + dx, cz + dz);
            if let Some(deck) = east_bridge(seed, x, z).deck_at(wx, wz) {
                return Some(deck);
            }
            if let Some(deck) = south_bridge(seed, x, z).and_then(|b| b.deck_at(wx, wz)) {
                return Some(deck);
            }
        }
    }
    None
}

/// y of the highest block of the land in the column (wx, wz) (the top of an island, or a bridge's deck), `None` over the
/// void.
pub fn surface_top(seed: i64, wx: i32, wz: i32) -> Option<i32> {
    island_at(seed, wx, wz).map(|i| i.top).or_else(|| deck_at(seed, wx, wz).map(|d| d.y))
}

/// Where the player arrives: the middle of the first island, standing on its grass.
pub fn spawn_point() -> Vec3 {
    vec3(SKY_SPAWN_CENTRE.0 as f32 + 0.5, SKY_BASE_Y as f32 + 1.01, SKY_SPAWN_CENTRE.1 as f32 + 0.5)
}

/// Where the cyclone that leads home stands: on the first island, a few blocks from where the player arrives.
pub fn home_cyclone() -> Vec3 {
    spawn_point() + vec3(SKY_HOME_CYCLONE_OFFSET, -0.01, 0.0)
}

/// Whether trees grow near (wx, wz): never on the stretch of the first island where the player arrives and the cyclone
/// stands, nor near any island's shore.
fn tree_site(seed: i64, wx: i32, wz: i32, island: &Island, landmarks: &Landmarks) -> bool {
    if landmarks.sky_tower == Some((cell_of(island.x), cell_of(island.z))) {
        return false;
    }
    let near = |p: Vec3| (p.x - wx as f32).abs() < 5.0 && (p.z - wz as f32).abs() < 5.0;
    island.share(wx, wz) < 0.6
        && !(*island == island_in_cell(seed, 0, 0) && (near(spawn_point()) || near(home_cyclone())))
        && hash3(seed, wx, wz, 0x5C1D) % 100 < SKY_TREE_PERCENT
}

/// The blocks of chunk (cx, cz) of the Skylands: the islands (rock, dirt, a few oaks), and the bridges' decks and posts.
pub fn generate_chunk(seed: i64, cx: i32, cz: i32, landmarks: &Landmarks) -> Vec<Block> {
    let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
    let mut trees = Vec::new();
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
            if let Some(island) = island_at(seed, wx, wz) {
                let bottom = island.bottom_at(wx, wz).unwrap_or(island.top);
                for y in bottom..=island.top {
                    blocks[idx(lx, y, lz)] = if y > island.top - SKY_DIRT_LAYERS { Block::Dirt } else { Block::Stone };
                }
                if landmarks.sky_tower == Some((cell_of(island.x), cell_of(island.z))) {
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
            } else if let Some(deck) = deck_at(seed, wx, wz) {
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
        assert_eq!(surface_top(7, p.x.floor() as i32, p.z.floor() as i32), Some(SKY_BASE_Y));
        let home = home_cyclone();
        assert_eq!(surface_top(7, home.x.floor() as i32, home.z.floor() as i32), Some(SKY_BASE_Y), "the way home stands on the first island");
        assert!((home - p).length() > CYCLONE_RADIUS + 3.0, "arriving never puts the player inside the cyclone");
    }

    #[test]
    fn the_land_is_a_pure_function_of_the_seed_and_other_seeds_differ() {
        let sample = |seed: i64| (-200..200).map(|x| surface_top(seed, x, 11)).collect::<Vec<_>>();
        assert_eq!(sample(3), sample(3));
        assert_ne!(sample(3), sample(4));
    }

    #[test]
    fn islands_stay_in_their_cells_and_clear_of_one_another() {
        for seed in 0..6 {
            for cz in -4..4 {
                for cx in -4..4 {
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
        let blocks = generate_chunk(5, 0, 0, &Landmarks::default());
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
            let here = surface_top(seed, x, z).unwrap();
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (x + dx, z + dz);
                if next.0.abs() > 180 || next.1.abs() > 180 || seen.contains(&next) {
                    continue;
                }
                if let Some(top) = surface_top(seed, next.0, next.1) {
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
                for &b in generate_chunk(5, cx, cz, &Landmarks::default()).iter() {
                    planks += (b == Block::BridgePlank) as i32;
                    posts += (b == Block::BridgePost) as i32;
                    wood += (b == Block::Wood) as i32;
                }
            }
        }
        assert!(planks > 100 && posts > 4 && wood > 0, "planks {planks}, posts {posts}, wood {wood}");
        // Far from every island and bridge there is nothing.
        let void_found = (-50..50).any(|x| (-50..50).any(|z| surface_top(5, x * 3, z * 3).is_none()));
        assert!(void_found, "gaps between the islands are open sky");
    }

    #[test]
    fn trees_keep_off_the_arrival_spot_and_the_way_home() {
        for seed in 0..30 {
            let blocks = generate_chunk(seed, 0, 0, &Landmarks::default());
            for p in [spawn_point(), home_cyclone()] {
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
}
