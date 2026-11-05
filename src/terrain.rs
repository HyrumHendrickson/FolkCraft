//! Procedural terrain: stone/dirt ground with occasional broad, gentle hills and valleys, winding
//! rivers, a forest of oak and pine trees, and occasional clearings of grass, flowers and bushes.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::block::Block;
use crate::chunk::idx;
use crate::config::*;
use crate::river::{is_river, river_near};
use crate::rng::{hash3, Rng};

/// A round tree-free patch of the forest floor, planted with grass, flowers and bushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clearing {
    /// Centre, in world coordinates.
    pub x: i32,
    pub z: i32,
    pub radius: i32,
}

impl Clearing {
    /// Whether the column (wx, wz) is part of the clearing.
    pub fn contains(&self, wx: i32, wz: i32) -> bool {
        let (dx, dz) = (wx - self.x, wz - self.z);
        dx * dx + dz * dz <= self.radius * self.radius
    }

    /// Whether a tree trunk may not stand at (wx, wz): the clearing plus a buffer so that no
    /// canopy overhangs it.
    pub fn blocks_trees_at(&self, wx: i32, wz: i32) -> bool {
        let reach = self.radius + CLEARING_TREE_BUFFER;
        let (dx, dz) = (wx - self.x, wz - self.z);
        dx * dx + dz * dz <= reach * reach
    }
}

/// The clearing in region (rx, rz), if it has one. Regions are `CLEARING_REGION_CHUNKS`
/// chunks square. A clearing, with its tree buffer, always lies wholly inside its region,
/// so every chunk only needs to ask about its own region.
pub fn clearing_in_region(seed: i64, rx: i32, rz: i32) -> Option<Clearing> {
    let mut rng = Rng::new(hash3(seed, rx, rz, 0xC1EA));
    if !rng.chance(CLEARING_CHANCE) {
        return None;
    }
    let size = CLEARING_REGION_CHUNKS * CHUNK_SIZE;
    // A clearing and its tree buffer have to fit inside the region with room to move its centre, so a
    // `CLEARING_MAX_RADIUS` too big for the region is cut down to the biggest that fits (and never divides by zero).
    let largest = ((size - 1) / 2 - CLEARING_TREE_BUFFER).max(CLEARING_MIN_RADIUS);
    let radius = rng.range(CLEARING_MIN_RADIUS, CLEARING_MAX_RADIUS + 1).min(largest);
    let margin = radius + CLEARING_TREE_BUFFER;
    Some(Clearing {
        x: rx * size + rng.range(margin, size - margin),
        z: rz * size + rng.range(margin, size - margin),
        radius,
    })
}

/// A broad, gentle change in the height of the ground: a hill (a rise) or a valley (a dip, the same
/// shape upside down). Trees grow on it and clearings can lie on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Relief {
    /// Centre, in world coordinates.
    pub x: i32,
    pub z: i32,
    pub radius: i32,
    /// Blocks the ground is raised at the centre; negative for a valley, which lowers it.
    pub height: i32,
}

impl Relief {
    /// Whether this is a valley rather than a hill.
    #[cfg(test)]
    pub fn is_valley(&self) -> bool {
        self.height < 0
    }

    /// How many blocks the ground is raised at the column (wx, wz) (lowered, if negative): zero outside
    /// the feature, the full `height` at the centre, and a smooth ramp in between.
    pub fn rise_at(&self, wx: i32, wz: i32) -> i32 {
        let (dx, dz) = ((wx - self.x) as f32, (wz - self.z) as f32);
        let dist = (dx * dx + dz * dz).sqrt();
        let radius = self.radius as f32;
        if dist >= radius {
            return 0;
        }
        let t = 1.0 - dist / radius;
        (self.height as f32 * t * t * (3.0 - 2.0 * t)).round() as i32
    }
}

/// Work out the hill or valley of region (rx, rz). One roll decides: the first `HILL_CHANCE` percent make a
/// hill, the next `VALLEY_CHANCE` percent a valley, the rest nothing. A feature lies wholly inside its region
/// and keeps `HILL_SPAWN_CLEARANCE` blocks from the spawn point and `RIVER_RELIEF_MARGIN` blocks from any
/// river: if a river is in the way it is tried at up to `RELIEF_PLACEMENT_TRIES` places in the region, and
/// dropped when none is clear. (The first place is the one it always had, so land without a river is unchanged.)
fn generate_relief(seed: i64, rx: i32, rz: i32) -> Option<Relief> {
    let mut rng = Rng::new(hash3(seed, rx, rz, 0x4177));
    let roll = rng.next() % 100;
    let valley = if roll < HILL_CHANCE {
        false
    } else if roll < HILL_CHANCE + VALLEY_CHANCE {
        true
    } else {
        return None;
    };
    let (min_radius, max_radius) = if valley { (VALLEY_MIN_RADIUS, VALLEY_MAX_RADIUS) } else { (HILL_MIN_RADIUS, HILL_MAX_RADIUS) };
    let radius = rng.range(min_radius, max_radius + 1);
    let height = if valley {
        -rng.range(VALLEY_MIN_DEPTH, VALLEY_MAX_DEPTH + 1)
    } else {
        rng.range(HILL_MIN_HEIGHT, HILL_MAX_HEIGHT + 1)
    };
    let size = HILL_REGION_CHUNKS * CHUNK_SIZE;
    let spawn = CHUNK_SIZE / 2;
    for attempt in 0..RELIEF_PLACEMENT_TRIES {
        let relief = Relief {
            x: rx * size + rng.range(radius, size - radius + 1),
            z: rz * size + rng.range(radius, size - radius + 1),
            radius,
            height,
        };
        let (dx, dz) = ((relief.x - spawn) as f32, (relief.z - spawn) as f32);
        let keep_away = (radius + HILL_SPAWN_CLEARANCE) as f32;
        if dx * dx + dz * dz < keep_away * keep_away {
            // Too near the spawn point. A feature whose first place was there has always been left out.
            if attempt == 0 {
                return None;
            }
            continue;
        }
        // A river in the way: try somewhere else in the region.
        if !river_near(seed, relief.x, relief.z, radius + RIVER_RELIEF_MARGIN) {
            return Some(relief);
        }
    }
    None
}

thread_local! {
    /// `generate_relief` looks at every column of a feature to keep it off the rivers, which is too much
    /// work to repeat for each of the thousands of height lookups a chunk makes. Regions are few, so the
    /// answers are remembered.
    static RELIEF_CACHE: RefCell<HashMap<(i64, i32, i32), Option<Relief>>> = RefCell::new(HashMap::new());
}

/// The hill or valley in region (rx, rz), if it has one. Regions are `HILL_REGION_CHUNKS` chunks square
/// and a feature lies wholly inside its region, so every chunk only needs to ask about its own region.
pub fn relief_in_region(seed: i64, rx: i32, rz: i32) -> Option<Relief> {
    RELIEF_CACHE.with(|cache| *cache.borrow_mut().entry((seed, rx, rz)).or_insert_with(|| generate_relief(seed, rx, rz)))
}

/// The hill or valley (if any) that can touch the column (wx, wz).
pub fn relief_for_column(seed: i64, wx: i32, wz: i32) -> Option<Relief> {
    let region = |w: i32| w.div_euclid(CHUNK_SIZE).div_euclid(HILL_REGION_CHUNKS);
    relief_in_region(seed, region(wx), region(wz))
}

/// y of the highest natural dirt block in the column (wx, wz): the flat ground raised by a hill, lowered
/// by a valley or by a river bed.
pub fn ground_top(seed: i64, wx: i32, wz: i32) -> i32 {
    surface(seed, relief_for_column(seed, wx, wz), wx, wz).0
}

/// Whether the bush at a position starts with purple berries during the day.
pub fn berry_bush_is_purple(seed: i64, wx: i32, wz: i32) -> bool {
    hash3(seed, wx, wz, 0xB3A7) % PURPLE_BUSH_ONE_IN == 0
}

/// The highest dirt block of the column (wx, wz) and whether a river runs over it, given the relief
/// that can touch it. Rivers keep off hills and valleys, so the two never add up.
fn surface(seed: i64, relief: Option<Relief>, wx: i32, wz: i32) -> (i32, bool) {
    let river = is_river(seed, wx, wz);
    let rise = relief.map_or(0, |r| r.rise_at(wx, wz));
    (GROUND_TOP + rise - if river { RIVER_DEPTH } else { 0 }, river)
}

/// The clearing (if any) that can touch chunk (cx, cz).
pub fn clearing_for_chunk(seed: i64, cx: i32, cz: i32) -> Option<Clearing> {
    clearing_in_region(seed, cx.div_euclid(CLEARING_REGION_CHUNKS), cz.div_euclid(CLEARING_REGION_CHUNKS))
}

/// Round, broad-canopied tree standing on the dirt whose top block is at `ground`.
/// `lx`/`lz` must be at least 3 blocks from the chunk edge.
pub fn place_oak(blocks: &mut [Block], rng: &mut Rng, lx: i32, lz: i32, ground: i32) {
    let base = ground + 1;
    let height = rng.range(5, 8); // trunk blocks
    let centre = height - 1; // canopy is centred on the top of the trunk
    let r2 = 8.5 + rng.range(0, 4) as f32; // squared horizontal radius, ~2.9..3.4

    for dy in (height - 3)..=(height + 1) {
        let vy = (dy - centre) as f32;
        for dz in -3..=3i32 {
            for dx in -3..=3i32 {
                let dist = (dx * dx + dz * dz) as f32 + vy * vy * 1.4;
                if dist > r2 {
                    continue;
                }
                // Ragged outer shell so the canopy is not a perfect ball.
                if dist > r2 - 2.5 && rng.chance(28) {
                    continue;
                }
                let i = idx(lx + dx, base + dy, lz + dz);
                if blocks[i] == Block::Air {
                    blocks[i] = Block::Leaves;
                }
            }
        }
    }
    for dy in 0..height {
        blocks[idx(lx, base + dy, lz)] = Block::Wood;
    }
}

/// Tall conical evergreen standing on the dirt whose top block is at `ground`.
/// `lx`/`lz` must be at least 3 blocks from the chunk edge.
pub fn place_pine(blocks: &mut [Block], rng: &mut Rng, lx: i32, lz: i32, ground: i32) {
    let base = ground + 1;
    let height = rng.range(7, 10);

    for dy in 2..=height {
        let k = height - dy; // 0 at the tip
        let radius: i32 = ((k + 1) / 2).min(3);
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let corner = dx.abs() == radius && dz.abs() == radius;
                if radius >= 2 && corner {
                    continue;
                }
                if radius >= 1 && (dx.abs() == radius || dz.abs() == radius) && rng.chance(15) {
                    continue;
                }
                let i = idx(lx + dx, base + dy, lz + dz);
                if blocks[i] == Block::Air {
                    blocks[i] = Block::Needles;
                }
            }
        }
    }
    for dy in 0..height {
        blocks[idx(lx, base + dy, lz)] = Block::Wood;
    }
}

pub fn generate_chunk_blocks(seed: i64, cx: i32, cz: i32) -> Vec<Block> {
    let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];

    // The surface of every column once: the height of its top dirt block, and whether a river covers it.
    let relief = relief_in_region(seed, cx.div_euclid(HILL_REGION_CHUNKS), cz.div_euclid(HILL_REGION_CHUNKS));
    let column = |lx: i32, lz: i32| (lz * CHUNK_SIZE + lx) as usize;
    let surfaces: Vec<(i32, bool)> = (0..CHUNK_SIZE)
        .flat_map(|lz| (0..CHUNK_SIZE).map(move |lx| (lx, lz)))
        .map(|(lx, lz)| surface(seed, relief, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz))
        .collect();
    let top_at = |lx: i32, lz: i32| surfaces[column(lx, lz)].0;
    let wet_at = |lx: i32, lz: i32| surfaces[column(lx, lz)].1;

    // Ground: a stone layer followed by dirt, as high as the hill (or as low as the valley) makes it there.
    // A river column has its dirt cut down to the bed and the gap up to the surrounding ground filled with
    // water. Nothing below the stone.
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            for y in 0..STONE_LAYERS {
                blocks[idx(lx, y, lz)] = Block::Stone;
            }
            for y in STONE_LAYERS..=top_at(lx, lz) {
                blocks[idx(lx, y, lz)] = Block::Dirt;
            }
            if wet_at(lx, lz) {
                for y in top_at(lx, lz) + 1..=GROUND_TOP {
                    blocks[idx(lx, y, lz)] = Block::Water;
                }
            }
        }
    }

    // Things lying on the bed under the water: clay, pebbles and stones (each takes the water's place in its cell).
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            if !wet_at(lx, lz) {
                continue;
            }
            let roll = hash3(seed, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz, 0xB3D) % 1000;
            let thing = if roll < RIVER_CLAY_PER_MILLE {
                Block::WetClay
            } else if roll < RIVER_CLAY_PER_MILLE + RIVER_PEBBLE_PER_MILLE {
                Block::WetPebble
            } else if roll < RIVER_CLAY_PER_MILLE + RIVER_PEBBLE_PER_MILLE + RIVER_STONE_PER_MILLE {
                Block::WetStone
            } else {
                continue;
            };
            blocks[idx(lx, top_at(lx, lz) + 1, lz)] = thing;
        }
    }

    // Trees. Trunks stay 3+ blocks from the chunk edge so the whole canopy
    // (radius <= 3) always lies inside its own chunk. None grow in or beside a clearing, or in a river.
    let clearing = clearing_for_chunk(seed, cx, cz);
    let mut rng = Rng::new(hash3(seed, cx, cz, 0x7EEE));
    let target = rng.range(6, 11);
    let mut placed: Vec<(i32, i32)> = Vec::new();
    for _ in 0..120 {
        if placed.len() as i32 >= target {
            break;
        }
        let lx = rng.range(3, CHUNK_SIZE - 3);
        let lz = rng.range(3, CHUNK_SIZE - 3);

        // Keep the spawn point (centre of chunk 0,0) clear.
        if cx == 0 && cz == 0 && (6..=13).contains(&lx) && (6..=13).contains(&lz) {
            continue;
        }
        if clearing.map_or(false, |c| c.blocks_trees_at(cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz)) {
            continue;
        }
        if wet_at(lx, lz) {
            continue;
        }
        if placed
            .iter()
            .any(|&(px, pz)| (px - lx).abs() < 4 && (pz - lz).abs() < 4)
        {
            continue;
        }
        placed.push((lx, lz));
        let ground = top_at(lx, lz);
        if rng.chance(65) {
            place_oak(&mut blocks, &mut rng, lx, lz, ground);
        } else {
            place_pine(&mut blocks, &mut rng, lx, lz, ground);
        }
    }

    // Plants on the floor of the clearing.
    if let Some(c) = clearing {
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                if !c.contains(wx, wz) || wet_at(lx, lz) {
                    continue;
                }
                let roll = hash3(seed, wx, wz, 0x91A7) % 100;
                let plant = if roll < GRASS_PERCENT {
                    Block::Grass
                } else if roll < GRASS_PERCENT + FLOWER_PERCENT {
                    Block::Flower
                } else if roll < GRASS_PERCENT + FLOWER_PERCENT + BUSH_PERCENT {
                    Block::Bush
                } else {
                    continue;
                };
                let i = idx(lx, top_at(lx, lz) + 1, lz);
                if blocks[i] == Block::Air {
                    blocks[i] = plant;
                }
            }
        }
    }

    // Stones and pebbles lying about on the ground (not in a river), on hills twice as often, and among them
    // the occasional boulder. Pebbles and stones are the blocks a player could place themselves, so they
    // can be picked up (and put back) the same way.
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            if wet_at(lx, lz) {
                continue;
            }
            let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
            let top = top_at(lx, lz);
            let factor = if top > GROUND_TOP { HILL_ROCK_FACTOR } else { 1 };
            let roll = hash3(seed, wx, wz, 0x50C4) % 1000;
            // The bands follow one another, so adding the boulders changed nothing for the others.
            let mut edge = 0;
            let mut band = |per_mille: u64| {
                edge += per_mille * factor;
                roll < edge
            };
            let rock = if band(GROUND_PEBBLE_PER_MILLE) {
                Block::PlacedPebble
            } else if band(GROUND_STONE_PER_MILLE) {
                Block::PlacedStone
            } else if band(GROUND_BOULDER_PER_MILLE) {
                Block::Boulder
            } else {
                continue;
            };
            let i = idx(lx, top + 1, lz);
            if blocks[i] == Block::Air {
                blocks[i] = rock;
            }
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_structure() {
        let b = generate_chunk_blocks(42, 0, 0);
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                assert_eq!(b[idx(x, 0, z)], Block::Stone);
                for y in 1..=5 {
                    assert_eq!(b[idx(x, y, z)], Block::Dirt);
                }
            }
        }
    }

    #[test]
    fn trees_exist_and_are_deterministic() {
        let a = generate_chunk_blocks(7, 3, -2);
        let b = generate_chunk_blocks(7, 3, -2);
        assert_eq!(a, b);
        assert!(a.contains(&Block::Wood));
        assert!(a.contains(&Block::Leaves) || a.contains(&Block::Needles));
        // Different seed gives a different forest.
        assert_ne!(a, generate_chunk_blocks(8, 3, -2));
    }

    #[test]
    fn trees_stay_inside_their_chunk_and_above_ground() {
        for seed in 0..40 {
            let b = generate_chunk_blocks(seed, 1, 1);
            for y in 0..WORLD_HEIGHT {
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let blk = b[idx(lx, y, lz)];
                        if matches!(blk, Block::Wood | Block::Leaves | Block::Needles) {
                            assert!(y > ground_top(seed, 20 + lx, 20 + lz), "trees stand above the ground, valleys included");
                        }
                    }
                }
            }
            // Trunks keep a 3-block margin, and every leaf is within 3 blocks of a trunk
            // (a leaf wrapped across a chunk edge would be far from any trunk).
            let mut trunks = Vec::new();
            for y in 0..WORLD_HEIGHT {
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        if b[idx(lx, y, lz)] == Block::Wood {
                            assert!((3..CHUNK_SIZE - 3).contains(&lx) && (3..CHUNK_SIZE - 3).contains(&lz));
                            trunks.push((lx, lz));
                        }
                    }
                }
            }
            for y in 0..WORLD_HEIGHT {
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        if matches!(b[idx(lx, y, lz)], Block::Leaves | Block::Needles) {
                            assert!(trunks.iter().any(|&(tx, tz)| (tx - lx).abs() <= 3 && (tz - lz).abs() <= 3));
                        }
                    }
                }
            }
        }
    }

    /// Find a chunk whose region holds a clearing, for tests.
    fn chunk_with_clearing(seed: i64) -> Option<(i32, i32, Clearing)> {
        for rz in -3..3 {
            for rx in -3..3 {
                if let Some(c) = clearing_in_region(seed, rx, rz) {
                    let size = CLEARING_REGION_CHUNKS;
                    return Some((rx * size + size / 2, rz * size + size / 2, c));
                }
            }
        }
        None
    }

    #[test]
    fn clearings_are_deterministic_and_appear_now_and_then() {
        let mut found = 0;
        let regions = 40 * 40;
        for rz in 0..40 {
            for rx in 0..40 {
                let c = clearing_in_region(11, rx, rz);
                assert_eq!(c, clearing_in_region(11, rx, rz));
                found += c.is_some() as i32;
            }
        }
        let share = found as f64 / regions as f64;
        let expected = CLEARING_CHANCE as f64 / 100.0;
        assert!((share - expected).abs() < 0.07, "clearing share {share}");
        // A different seed lays the clearings out differently.
        let layout = |seed| -> Vec<Option<Clearing>> { (0..8).map(|r| clearing_in_region(seed, r, 0)).collect() };
        assert_ne!(layout(11), layout(12));
    }

    #[test]
    fn a_clearing_and_its_tree_buffer_stay_inside_their_region() {
        let size = CLEARING_REGION_CHUNKS * CHUNK_SIZE;
        for seed in 0..20 {
            for rz in -4..4 {
                for rx in -4..4 {
                    if let Some(c) = clearing_in_region(seed, rx, rz) {
                        assert!((CLEARING_MIN_RADIUS..=CLEARING_MAX_RADIUS).contains(&c.radius));
                        let reach = c.radius + CLEARING_TREE_BUFFER;
                        assert!(c.x - reach >= rx * size && c.x + reach < (rx + 1) * size, "{c:?} x in region {rx}");
                        assert!(c.z - reach >= rz * size && c.z + reach < (rz + 1) * size, "{c:?} z in region {rz}");
                    }
                }
            }
        }
    }

    #[test]
    fn every_chunk_asks_about_its_own_region() {
        for seed in 0..10 {
            for cz in -7_i32..7 {
                for cx in -7_i32..7 {
                    let expect = clearing_in_region(
                        seed,
                        cx.div_euclid(CLEARING_REGION_CHUNKS),
                        cz.div_euclid(CLEARING_REGION_CHUNKS),
                    );
                    assert_eq!(clearing_for_chunk(seed, cx, cz), expect);
                }
            }
        }
    }

    #[test]
    fn clearings_hold_grass_flowers_and_bushes_only_on_the_ground_inside_the_disc() {
        let mut kinds = std::collections::HashSet::new();
        for seed in 0..6 {
            let size = CLEARING_REGION_CHUNKS;
            for rz in -2..2 {
                for rx in -2..2 {
                    let Some(c) = clearing_in_region(seed, rx, rz) else { continue };
                    for cz in rz * size..(rz + 1) * size {
                        for cx in rx * size..(rx + 1) * size {
                            let b = generate_chunk_blocks(seed, cx, cz);
                            for y in 0..WORLD_HEIGHT {
                                for lz in 0..CHUNK_SIZE {
                                    for lx in 0..CHUNK_SIZE {
                                        let blk = b[idx(lx, y, lz)];
                                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                                        if matches!(blk, Block::Grass | Block::Flower | Block::Bush) {
                                            assert_eq!(y, ground_top(seed, wx, wz) + 1, "plants sit on the ground");
                                            assert!(c.contains(wx, wz), "plant outside the clearing at {wx},{wz}");
                                            kinds.insert(blk);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(kinds.len(), 3, "grass, flowers and bushes all appear: {kinds:?}");
    }

    #[test]
    fn no_trunk_stands_in_or_beside_a_clearing() {
        let (cx, cz, c) = chunk_with_clearing(3).expect("a clearing");
        for dz in -1..=1 {
            for dx in -1..=1 {
                let b = generate_chunk_blocks(3, cx + dx, cz + dz);
                for y in 0..WORLD_HEIGHT {
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            if b[idx(lx, y, lz)] == Block::Wood {
                                let (wx, wz) = ((cx + dx) * CHUNK_SIZE + lx, (cz + dz) * CHUNK_SIZE + lz);
                                assert!(!c.blocks_trees_at(wx, wz), "trunk at {wx},{wz} inside {c:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn plants_are_roughly_as_dense_as_configured() {
        let (mut cells, mut grass, mut flowers, mut bushes) = (0, 0, 0, 0);
        for seed in 0..30 {
            for rz in -2..2 {
                for rx in -2..2 {
                    let Some(c) = clearing_in_region(seed, rx, rz) else { continue };
                    for wz in c.z - c.radius..=c.z + c.radius {
                        for wx in c.x - c.radius..=c.x + c.radius {
                            if !c.contains(wx, wz) {
                                continue;
                            }
                            cells += 1;
                            let b = generate_chunk_blocks(seed, wx.div_euclid(CHUNK_SIZE), wz.div_euclid(CHUNK_SIZE));
                            let i = idx(wx.rem_euclid(CHUNK_SIZE), ground_top(seed, wx, wz) + 1, wz.rem_euclid(CHUNK_SIZE));
                            match b[i] {
                                Block::Grass => grass += 1,
                                Block::Flower => flowers += 1,
                                Block::Bush => bushes += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        let share = |n: i32| n as f64 / cells as f64 * 100.0;
        assert!((share(grass) - GRASS_PERCENT as f64).abs() < 3.0, "grass {}", share(grass));
        assert!((share(flowers) - FLOWER_PERCENT as f64).abs() < 2.0, "flowers {}", share(flowers));
        assert!((share(bushes) - BUSH_PERCENT as f64).abs() < 2.0, "bushes {}", share(bushes));
    }

    #[test]
    fn hills_and_valleys_are_deterministic_and_appear_now_and_then() {
        let (mut hills, mut valleys) = (0, 0);
        for rz in 0..40 {
            for rx in 0..40 {
                let r = relief_in_region(11, rx, rz);
                assert_eq!(r, generate_relief(11, rx, rz), "the cache gives what generation gives");
                match r {
                    Some(r) if r.is_valley() => valleys += 1,
                    Some(_) => hills += 1,
                    None => {}
                }
            }
        }
        // Rivers take up much of the land and turn some away, so well under the configured share is found.
        let (hills, valleys) = (hills as f64 / 1600.0, valleys as f64 / 1600.0);
        let (want_hills, want_valleys) = (HILL_CHANCE as f64 / 100.0, VALLEY_CHANCE as f64 / 100.0);
        assert!((0.2..want_hills + 0.05).contains(&hills), "hill share {hills}");
        assert!((0.08..want_valleys + 0.05).contains(&valleys), "valley share {valleys}");
        let layout = |seed| -> Vec<Option<Relief>> { (0..8).map(|r| relief_in_region(seed, r, 0)).collect() };
        assert_ne!(layout(11), layout(12));
    }

    #[test]
    fn a_hill_or_valley_is_large_and_lies_inside_its_region() {
        let size = HILL_REGION_CHUNKS * CHUNK_SIZE;
        for seed in 0..20 {
            for rz in -4..4 {
                for rx in -4..4 {
                    if let Some(h) = relief_in_region(seed, rx, rz) {
                        if h.is_valley() {
                            assert!((VALLEY_MIN_RADIUS..=VALLEY_MAX_RADIUS).contains(&h.radius));
                            assert!((VALLEY_MIN_DEPTH..=VALLEY_MAX_DEPTH).contains(&-h.height));
                        } else {
                            assert!((HILL_MIN_RADIUS..=HILL_MAX_RADIUS).contains(&h.radius));
                            assert!((HILL_MIN_HEIGHT..=HILL_MAX_HEIGHT).contains(&h.height));
                        }
                        assert!(h.x - h.radius >= rx * size && h.x + h.radius <= (rx + 1) * size, "{h:?} x in region {rx}");
                        assert!(h.z - h.radius >= rz * size && h.z + h.radius <= (rz + 1) * size, "{h:?} z in region {rz}");
                    }
                }
            }
        }
    }

    #[test]
    fn hills_and_valleys_are_gentle_changing_one_block_at_a_time_to_their_full_size() {
        for seed in 0..30 {
            for rz in -3..3 {
                for rx in -3..3 {
                    let Some(h) = relief_in_region(seed, rx, rz) else { continue };
                    assert_eq!(h.rise_at(h.x, h.z), h.height, "the centre is the top (or the bottom)");
                    assert_eq!(h.rise_at(h.x + h.radius, h.z), 0);
                    assert_eq!(h.rise_at(h.x, h.z - h.radius - 5), 0);
                    let (lowest, highest_allowed) = (h.height.min(0), h.height.max(0));
                    let mut extreme: i32 = 0;
                    for wz in h.z - h.radius..=h.z + h.radius {
                        for wx in h.x - h.radius..=h.x + h.radius {
                            let rise = h.rise_at(wx, wz);
                            assert!((lowest..=highest_allowed).contains(&rise));
                            extreme = if rise.abs() > extreme.abs() { rise } else { extreme };
                            for (nx, nz) in [(wx + 1, wz), (wx, wz + 1)] {
                                assert!((h.rise_at(nx, nz) - rise).abs() <= 1, "no cliffs: {h:?} at {wx},{wz}");
                            }
                        }
                    }
                    assert_eq!(extreme, h.height);
                }
            }
        }
    }

    #[test]
    fn the_spawn_chunk_is_always_flat() {
        for seed in 0..300 {
            for lz in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    assert_eq!(ground_top(seed, lx, lz), GROUND_TOP, "seed {seed}");
                }
            }
        }
    }

    /// A chunk whose region holds a hill (`valley` false) or a valley, with its centre column.
    fn relief_chunk(seed: i64, valley: bool) -> Option<(i32, i32, Relief)> {
        for rz in -3..3 {
            for rx in -3..3 {
                if let Some(h) = relief_in_region(seed, rx, rz).filter(|h| h.is_valley() == valley) {
                    return Some((h.x.div_euclid(CHUNK_SIZE), h.z.div_euclid(CHUNK_SIZE), h));
                }
            }
        }
        None
    }

    fn hill_chunk(seed: i64) -> Option<(i32, i32, Relief)> {
        relief_chunk(seed, false)
    }

    #[test]
    fn a_hill_raises_the_dirt_and_leaves_air_above_it() {
        let (cx, cz, h) = hill_chunk(5).expect("a hill");
        let b = generate_chunk_blocks(5, cx, cz);
        let (lx, lz) = (h.x.rem_euclid(CHUNK_SIZE), h.z.rem_euclid(CHUNK_SIZE));
        let top = GROUND_TOP + h.height;
        assert_eq!(ground_top(5, h.x, h.z), top);
        assert_eq!(b[idx(lx, 0, lz)], Block::Stone);
        for y in 1..=top {
            assert_eq!(b[idx(lx, y, lz)], Block::Dirt, "y = {y}");
        }
        assert!(b[idx(lx, top + 1, lz)] != Block::Dirt && b[idx(lx, top + 1, lz)] != Block::Stone);
        assert!(top + 1 < WORLD_HEIGHT);
    }

    #[test]
    fn trees_on_a_hill_stand_on_its_surface_and_fit_under_the_sky() {
        let mut trees_on_rise = 0;
        for seed in 0..12 {
            let Some((cx, cz, _)) = hill_chunk(seed) else { continue };
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let b = generate_chunk_blocks(seed, cx + dx, cz + dz);
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            let (wx, wz) = ((cx + dx) * CHUNK_SIZE + lx, (cz + dz) * CHUNK_SIZE + lz);
                            let ground = ground_top(seed, wx, wz);
                            let lowest = (0..WORLD_HEIGHT).find(|&y| b[idx(lx, y, lz)] == Block::Wood);
                            if let Some(y) = lowest {
                                assert_eq!(y, ground + 1, "the trunk starts on the surface at {wx},{wz}");
                                assert_eq!(b[idx(lx, y - 1, lz)], Block::Dirt);
                                trees_on_rise += (ground > GROUND_TOP) as i32;
                            }
                            // Nothing hangs at the world's ceiling, so no tree is cut off.
                            assert_eq!(b[idx(lx, WORLD_HEIGHT - 1, lz)], Block::Air);
                        }
                    }
                }
            }
        }
        assert!(trees_on_rise > 10, "hills carry trees ({trees_on_rise})");
    }

    /// Count the pebbles and stones lying on the ground of the chunks in `cx0..cx1` x `cz0..cz1`,
    /// checking each rests on the dirt. Returns (pebbles, stones, columns).
    fn count_rocks(seed: i64, cx0: i32, cx1: i32, cz0: i32, cz1: i32, only_hills: Option<bool>) -> (u32, u32, u32) {
        let (mut pebbles, mut stones, mut columns) = (0, 0, 0);
        for cz in cz0..cz1 {
            for cx in cx0..cx1 {
                let b = generate_chunk_blocks(seed, cx, cz);
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        let top = ground_top(seed, wx, wz);
                        if only_hills.map_or(false, |h| h != (top > GROUND_TOP)) {
                            continue;
                        }
                        columns += 1;
                        match b[idx(lx, top + 1, lz)] {
                            Block::PlacedPebble => pebbles += 1,
                            Block::PlacedStone => stones += 1,
                            _ => continue,
                        }
                        assert_eq!(b[idx(lx, top, lz)], Block::Dirt, "rocks lie on the ground");
                    }
                }
            }
        }
        (pebbles, stones, columns)
    }

    #[test]
    fn stones_and_pebbles_lie_about_on_the_ground_at_the_configured_rate() {
        let (pebbles, stones, columns) = count_rocks(3, -4, 4, -4, 4, Some(false));
        let per_mille = |n: u32| n as f64 / columns as f64 * 1000.0;
        // Plants share the column in clearings, so the rate can only fall a little short.
        assert!((per_mille(pebbles) - GROUND_PEBBLE_PER_MILLE as f64).abs() < 6.0, "pebbles {}", per_mille(pebbles));
        assert!((per_mille(stones) - GROUND_STONE_PER_MILLE as f64).abs() < 4.0, "stones {}", per_mille(stones));
    }

    #[test]
    fn hills_are_rockier_than_the_flat_forest() {
        let (mut rocks_hill, mut cols_hill, mut rocks_flat, mut cols_flat) = (0, 0, 0, 0);
        for seed in 0..4 {
            let (p, s, c) = count_rocks(seed, -6, 6, -6, 6, Some(true));
            rocks_hill += p + s;
            cols_hill += c;
            let (p, s, c) = count_rocks(seed, -6, 6, -6, 6, Some(false));
            rocks_flat += p + s;
            cols_flat += c;
        }
        assert!(cols_hill > 5000, "enough hill to measure ({cols_hill})");
        let (hill, flat) = (rocks_hill as f64 / cols_hill as f64, rocks_flat as f64 / cols_flat as f64);
        assert!(hill > flat * 1.5, "hill {hill}, flat {flat}");
    }

    #[test]
    fn rocks_are_found_in_clearings_and_under_the_trees_too() {
        let mut in_clearing = 0;
        let mut in_forest = 0;
        for seed in 0..6 {
            for rz in -2..2 {
                for rx in -2..2 {
                    let Some(c) = clearing_in_region(seed, rx, rz) else { continue };
                    let size = CLEARING_REGION_CHUNKS;
                    for cz in rz * size..(rz + 1) * size {
                        for cx in rx * size..(rx + 1) * size {
                            let b = generate_chunk_blocks(seed, cx, cz);
                            for lz in 0..CHUNK_SIZE {
                                for lx in 0..CHUNK_SIZE {
                                    let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                                    if matches!(b[idx(lx, ground_top(seed, wx, wz) + 1, lz)], Block::PlacedPebble | Block::PlacedStone) {
                                        if c.contains(wx, wz) { in_clearing += 1 } else { in_forest += 1 }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(in_clearing > 0 && in_forest > 0, "clearing {in_clearing}, forest {in_forest}");
    }

    #[test]
    fn natural_rocks_can_be_picked_up_and_stay_gone() {
        use crate::world::World;
        let mut w = World::new(3, std::collections::HashSet::new());
        w.ensure_chunk(2, 2);
        let found = (0..CHUNK_SIZE)
            .flat_map(|lz| (0..CHUNK_SIZE).map(move |lx| (2 * CHUNK_SIZE + lx, 2 * CHUNK_SIZE + lz)))
            .map(|(x, z)| (x, ground_top(3, x, z) + 1, z))
            .find(|&(x, y, z)| w.block_at(x, y, z) == Block::PlacedPebble)
            .expect("a pebble in the chunk");
        w.break_block(found.0, found.1, found.2);
        assert!(w.removed.contains(&found) && w.placed.is_empty(), "recorded as a removal of generated terrain");
        let mut again = World::new(3, w.removed.clone());
        again.ensure_chunk(2, 2);
        assert_eq!(again.block_at(found.0, found.1, found.2), Block::Air);
    }

    #[test]
    fn clearings_can_lie_on_hills() {
        let mut on_hill = 0;
        for seed in 0..40 {
            for rz in -3..3 {
                for rx in -3..3 {
                    let Some(c) = clearing_in_region(seed, rx, rz) else { continue };
                    if ground_top(seed, c.x, c.z) > GROUND_TOP {
                        on_hill += 1;
                        let b = generate_chunk_blocks(seed, c.x.div_euclid(CHUNK_SIZE), c.z.div_euclid(CHUNK_SIZE));
                        let (lx, lz) = (c.x.rem_euclid(CHUNK_SIZE), c.z.rem_euclid(CHUNK_SIZE));
                        let ground = ground_top(seed, c.x, c.z);
                        assert!(!(0..WORLD_HEIGHT).any(|y| b[idx(lx, y, lz)] == Block::Wood), "no trunk in the clearing centre");
                        assert_eq!(b[idx(lx, ground, lz)], Block::Dirt);
                    }
                }
            }
        }
        assert!(on_hill > 0, "some clearing overlaps a hill");
    }

    // --- Rivers and valleys ----------------------------------------------------------------

    /// A chunk with a river column in it, for tests.
    fn chunk_with_river(seed: i64) -> Option<(i32, i32)> {
        (-12..12)
            .flat_map(|cz| (-12..12).map(move |cx| (cx, cz)))
            .find(|&(cx, cz)| (0..CHUNK_SIZE).any(|lz| (0..CHUNK_SIZE).any(|lx| is_river(seed, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz))))
    }

    #[test]
    fn a_river_is_water_filling_a_bed_one_block_below_the_ground() {
        let mut wet_columns = 0;
        for seed in 0..4 {
            let (cx, cz) = chunk_with_river(seed).expect("a river chunk");
            for (cx, cz) in [(cx, cz), (cx + 1, cz), (cx, cz + 1)] {
                let b = generate_chunk_blocks(seed, cx, cz);
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        let water: Vec<i32> = (0..WORLD_HEIGHT).filter(|&y| b[idx(lx, y, lz)].is_wet()).collect();
                        if is_river(seed, wx, wz) {
                            wet_columns += 1;
                            let bed = GROUND_TOP - RIVER_DEPTH;
                            assert_eq!(ground_top(seed, wx, wz), bed);
                            assert_eq!(b[idx(lx, bed, lz)], Block::Dirt, "the bed is dirt");
                            assert_eq!(water, (bed + 1..=GROUND_TOP).collect::<Vec<_>>(), "water fills the bed up to the ground beside it");
                            assert_eq!(b[idx(lx, GROUND_TOP + 1, lz)], Block::Air, "nothing grows or lies in the water");
                        } else {
                            assert!(water.is_empty(), "no water outside the rivers at {wx},{wz}");
                        }
                    }
                }
            }
        }
        assert!(wet_columns > 200, "{wet_columns} river columns checked");
    }

    #[test]
    fn rivers_have_clay_pebbles_and_stones_on_the_bed_in_their_configured_shares() {
        let (mut wet, mut clay, mut pebbles, mut stones) = (0, 0, 0, 0);
        for seed in 0..6 {
            let (cx, cz) = chunk_with_river(seed).expect("a river chunk");
            for dz in -3..=3 {
                for dx in -3..=3 {
                    let b = generate_chunk_blocks(seed, cx + dx, cz + dz);
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            if !is_river(seed, (cx + dx) * CHUNK_SIZE + lx, (cz + dz) * CHUNK_SIZE + lz) {
                                continue;
                            }
                            wet += 1;
                            match b[idx(lx, GROUND_TOP, lz)] {
                                Block::WetClay => clay += 1,
                                Block::WetPebble => pebbles += 1,
                                Block::WetStone => stones += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        let per_mille = |n: i32| n as f64 / wet as f64 * 1000.0;
        assert!(wet > 3000, "{wet} river columns");
        assert!((per_mille(clay) - RIVER_CLAY_PER_MILLE as f64).abs() < 20.0, "clay {}", per_mille(clay));
        assert!((per_mille(pebbles) - RIVER_PEBBLE_PER_MILLE as f64).abs() < 20.0, "pebbles {}", per_mille(pebbles));
        assert!((per_mille(stones) - RIVER_STONE_PER_MILLE as f64).abs() < 14.0, "stones {}", per_mille(stones));
        assert!(clay > 0 && pebbles > 0 && stones > 0);
    }

    #[test]
    fn boulders_lie_on_dry_ground_at_the_configured_rate_and_never_in_water() {
        let (mut columns, mut boulders) = (0, 0);
        for seed in 0..3 {
            for cz in -6..6 {
                for cx in -6..6 {
                    let b = generate_chunk_blocks(seed, cx, cz);
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                            let top = ground_top(seed, wx, wz);
                            if top > GROUND_TOP || is_river(seed, wx, wz) {
                                continue;
                            }
                            columns += 1;
                            if b[idx(lx, top + 1, lz)] == Block::Boulder {
                                boulders += 1;
                                assert_eq!(b[idx(lx, top, lz)], Block::Dirt, "they lie on the ground");
                            }
                        }
                    }
                }
            }
        }
        let per_mille = boulders as f64 / columns as f64 * 1000.0;
        assert!((per_mille - GROUND_BOULDER_PER_MILLE as f64).abs() < 1.0, "boulders {per_mille}");
        assert!(boulders > 0, "boulders are to be found");
    }

    #[test]
    fn adding_boulders_did_not_move_the_plain_pebbles_and_stones() {
        // A bare dry flat column that rolls inside the first band is a pebble, and inside the second a stone,
        // as before the boulders were added.
        let (mut pebbles, mut stones) = (0, 0);
        for cz in -3..3 {
            for cx in -3..3 {
                let b = generate_chunk_blocks(3, cx, cz);
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        if is_river(3, wx, wz) || ground_top(3, wx, wz) != GROUND_TOP {
                            continue;
                        }
                        let roll = hash3(3, wx, wz, 0x50C4) % 1000;
                        let above = b[idx(lx, GROUND_TOP + 1, lz)];
                        if above == Block::Air || matches!(above, Block::PlacedPebble | Block::PlacedStone) {
                            if roll < GROUND_PEBBLE_PER_MILLE {
                                assert_eq!(above, Block::PlacedPebble);
                                pebbles += 1;
                            } else if roll < GROUND_PEBBLE_PER_MILLE + GROUND_STONE_PER_MILLE {
                                assert_eq!(above, Block::PlacedStone);
                                stones += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(pebbles > 20 && stones > 8, "{pebbles} pebbles, {stones} stones checked");
    }

    #[test]
    fn rivers_run_over_level_ground_and_no_hill_or_valley_comes_near_one() {
        let mut features = 0;
        for seed in 0..12 {
            for rz in -4..4 {
                for rx in -4..4 {
                    let Some(h) = relief_in_region(seed, rx, rz) else { continue };
                    features += 1;
                    let reach = h.radius + RIVER_RELIEF_MARGIN;
                    for wz in h.z - reach..=h.z + reach {
                        for wx in h.x - reach..=h.x + reach {
                            if (wx - h.x).pow(2) + (wz - h.z).pow(2) <= reach * reach {
                                assert!(!is_river(seed, wx, wz), "a river at {wx},{wz} within reach of {h:?}");
                            }
                        }
                    }
                }
            }
        }
        assert!(features > 50, "{features} hills and valleys checked");
        // So a river column is always the level ground cut down by exactly the river's depth.
        for seed in 0..4 {
            for wz in -150..150 {
                for wx in -150..150 {
                    if is_river(seed, wx, wz) {
                        assert_eq!(ground_top(seed, wx, wz), GROUND_TOP - RIVER_DEPTH);
                    }
                }
            }
        }
    }

    #[test]
    fn rivers_never_cross_the_spawn_chunk_so_it_stays_dry_and_flat() {
        for seed in 0..60 {
            let b = generate_chunk_blocks(seed, 0, 0);
            assert!(!b.contains(&Block::Water), "seed {seed}");
        }
    }

    #[test]
    fn a_valley_lowers_the_ground_and_keeps_a_layer_of_dirt_above_the_stone() {
        let (cx, cz, v) = relief_chunk(5, true).expect("a valley");
        assert!(v.is_valley() && v.height < 0);
        let b = generate_chunk_blocks(5, cx, cz);
        let (lx, lz) = (v.x.rem_euclid(CHUNK_SIZE), v.z.rem_euclid(CHUNK_SIZE));
        let top = GROUND_TOP + v.height;
        assert_eq!(ground_top(5, v.x, v.z), top);
        assert!(top >= STONE_LAYERS, "at least one layer of dirt is left");
        assert_eq!(b[idx(lx, 0, lz)], Block::Stone);
        for y in 1..=top {
            assert_eq!(b[idx(lx, y, lz)], Block::Dirt, "y = {y}");
        }
        assert!(b[idx(lx, top + 1, lz)] != Block::Dirt, "nothing solid above the valley floor");
        assert!(VALLEY_MAX_DEPTH < DIRT_LAYERS);
    }

    #[test]
    fn valleys_carry_trees_like_the_flat_forest_and_clearings_can_lie_in_them() {
        let (mut trees_in_valleys, mut clearings_in_valleys) = (0, 0);
        for seed in 0..64 {
            let Some((cx, cz, v)) = relief_chunk(seed, true) else { continue };
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let b = generate_chunk_blocks(seed, cx + dx, cz + dz);
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            let (wx, wz) = ((cx + dx) * CHUNK_SIZE + lx, (cz + dz) * CHUNK_SIZE + lz);
                            let ground = ground_top(seed, wx, wz);
                            let lowest = (0..WORLD_HEIGHT).find(|&y| b[idx(lx, y, lz)] == Block::Wood);
                            if let Some(y) = lowest {
                                assert_eq!(y, ground + 1, "the trunk starts on the valley floor at {wx},{wz}");
                                trees_in_valleys += (ground < GROUND_TOP) as i32;
                            }
                        }
                    }
                }
            }
            // A clearing whose centre lies in the valley.
            for rz in -3..3 {
                for rx in -3..3 {
                    if let Some(c) = clearing_in_region(seed, rx, rz) {
                        clearings_in_valleys += (ground_top(seed, c.x, c.z) < GROUND_TOP && v.rise_at(c.x, c.z) < 0) as i32;
                    }
                }
            }
        }
        assert!(trees_in_valleys > 10, "valleys carry trees ({trees_in_valleys})");
        assert!(clearings_in_valleys > 0, "some clearing lies in a valley ({clearings_in_valleys})");
    }

    #[test]
    fn no_tree_plant_or_rock_stands_in_a_river() {
        for seed in 0..4 {
            let (cx, cz) = chunk_with_river(seed).expect("a river chunk");
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let (cx, cz) = (cx + dx, cz + dz);
                    let b = generate_chunk_blocks(seed, cx, cz);
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            if !is_river(seed, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz) {
                                continue;
                            }
                            for y in GROUND_TOP + 1..GROUND_TOP + 3 {
                                let blk = b[idx(lx, y, lz)];
                                assert!(!matches!(blk, Block::Wood | Block::Grass | Block::Flower | Block::Bush | Block::PlacedPebble | Block::PlacedStone), "{blk:?} in the river");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_clearing_fits_its_region_whatever_the_configured_radius_and_none_crashes_generation() {
        let size = CLEARING_REGION_CHUNKS * CHUNK_SIZE;
        let mut seen = 0;
        for seed in 0..40 {
            for rz in -6..6 {
                for rx in -6..6 {
                    let Some(c) = clearing_in_region(seed, rx, rz) else { continue };
                    seen += 1;
                    let margin = c.radius + CLEARING_TREE_BUFFER;
                    assert!(c.radius >= CLEARING_MIN_RADIUS && c.radius <= CLEARING_MAX_RADIUS);
                    assert!(c.x - margin >= rx * size && c.x + margin < (rx + 1) * size, "x of {c:?} in region {rx}");
                    assert!(c.z - margin >= rz * size && c.z + margin < (rz + 1) * size, "z of {c:?} in region {rz}");
                }
            }
        }
        assert!(seen > 500, "{seen} clearings checked");
    }

}
