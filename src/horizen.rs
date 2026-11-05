//! The Horizen: a second dimension, reached by falling into the well.
//!
//! A dimension is a world of its own. The Horizen shares nothing with the overworld but the player and what they
//! carry: its land, its edits (blocks put down, fires lit) and its items on the ground are separate, and none of the
//! overworld's creatures live there. For now there is nothing in it but empty flat land, at the height of the
//! overworld's plain, broken by many small ponds. Its sky is split in two: the sun and the clouds fill one half and the
//! moon and the stars the other (`sky.rs`), it never changes between day and night, and the compass, the peep stone and
//! the bone totem show nothing there.
//!
//! Like the overworld, the land is a pure function of the world seed, here of the pond of each square of
//! `HORIZEN_POND_CELL` blocks.

use crate::block::Block;
use crate::chunk::idx;
use crate::config::*;
use crate::rng::hash3;

/// Which of the two worlds a `World` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dimension {
    /// The ordinary world: forest, hills, rivers and the landmarks the player brings into it.
    #[default]
    Overworld,
    /// The empty flat land with ponds, behind the well.
    Horizen,
}

/// A pond: an oval of water, `rx` blocks across east to west and `rz` north to south (radii), round a centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pond {
    pub x: f32,
    pub z: f32,
    pub rx: f32,
    pub rz: f32,
}

impl Pond {
    /// Whether the middle of the column (wx, wz) lies in the water.
    pub fn contains(&self, wx: i32, wz: i32) -> bool {
        let (dx, dz) = ((wx as f32 + 0.5 - self.x) / self.rx, (wz as f32 + 0.5 - self.z) / self.rz);
        dx * dx + dz * dz <= 1.0
    }
}

/// The pond (if any) in the square of `HORIZEN_POND_CELL` blocks numbered (cx, cz). Its middle lies well inside the
/// square, so it never reaches into the next one; the ponds near the arrival point are left out.
pub fn pond_in_cell(seed: i64, cx: i32, cz: i32) -> Option<Pond> {
    let h = hash3(seed, cx, cz, 0x4021);
    if h % 100 >= HORIZEN_POND_CHANCE {
        return None;
    }
    let unit = |shift: u32| ((h >> shift) & 0xFFFF) as f32 / 65535.0;
    let max = HORIZEN_POND_MAX_RADIUS;
    let slack = HORIZEN_POND_CELL as f32 - 2.0 * max - 1.0;
    let pond = Pond {
        x: (cx * HORIZEN_POND_CELL) as f32 + max + 0.5 + unit(8) * slack,
        z: (cz * HORIZEN_POND_CELL) as f32 + max + 0.5 + unit(24) * slack,
        rx: HORIZEN_POND_MIN_RADIUS + unit(40) * (max - HORIZEN_POND_MIN_RADIUS),
        rz: HORIZEN_POND_MIN_RADIUS + unit(52) * (max - HORIZEN_POND_MIN_RADIUS),
    };
    let (sx, sz) = (HORIZEN_SPAWN.0 as f32 + 0.5, HORIZEN_SPAWN.1 as f32 + 0.5);
    let gap = ((pond.x - sx).powi(2) + (pond.z - sz).powi(2)).sqrt() - pond.rx.max(pond.rz);
    (gap >= HORIZEN_SPAWN_CLEARANCE).then_some(pond)
}

/// Whether a pond covers the column (wx, wz).
pub fn is_pond(seed: i64, wx: i32, wz: i32) -> bool {
    pond_in_cell(seed, wx.div_euclid(HORIZEN_POND_CELL), wz.div_euclid(HORIZEN_POND_CELL)).map_or(false, |p| p.contains(wx, wz))
}

/// y of the highest block of the ground in the column (wx, wz): flat, and one block lower under a pond.
pub fn ground_top(seed: i64, wx: i32, wz: i32) -> i32 {
    GROUND_TOP - if is_pond(seed, wx, wz) { RIVER_DEPTH } else { 0 }
}

/// The blocks of chunk (cx, cz) of the Horizen: the stone layer, dirt up to the ground (nothing can be dug), water
/// filling the ponds up to the plain, and nothing else: no tree, plant, stone or animal.
pub fn generate_chunk(seed: i64, cx: i32, cz: i32) -> Vec<Block> {
    let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
            let top = ground_top(seed, wx, wz);
            for y in 0..STONE_LAYERS {
                blocks[idx(lx, y, lz)] = Block::Stone;
            }
            for y in STONE_LAYERS..=top {
                blocks[idx(lx, y, lz)] = Block::Dirt;
            }
            for y in top + 1..=GROUND_TOP {
                blocks[idx(lx, y, lz)] = Block::Water;
            }
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_land_is_flat_and_a_pure_function_of_the_seed() {
        for (x, z) in [(0, 0), (-77, 31), (400, -250)] {
            assert_eq!(is_pond(5, x, z), is_pond(5, x, z));
            assert!(ground_top(5, x, z) == GROUND_TOP || ground_top(5, x, z) == GROUND_TOP - RIVER_DEPTH);
        }
        assert_ne!((-200..200).map(|x| is_pond(1, x, 7)).collect::<Vec<_>>(), (-200..200).map(|x| is_pond(2, x, 7)).collect::<Vec<_>>(), "another seed, other ponds");
    }

    #[test]
    fn there_are_many_small_ponds() {
        let (mut wet, mut total) = (0, 0);
        for z in -150..150 {
            for x in -150..150 {
                total += 1;
                wet += is_pond(9, x, z) as i32;
            }
        }
        let share = wet as f32 / total as f32;
        assert!(share > 0.03 && share < 0.35, "pond share {share}");
        let cells = (-10..10).flat_map(|cz| (-10..10).map(move |cx| pond_in_cell(9, cx, cz))).flatten().count();
        assert!(cells > 100, "{cells} ponds in 400 squares");
        for p in (-10..10).flat_map(|cz| (-10..10).map(move |cx| pond_in_cell(9, cx, cz))).flatten() {
            assert!(p.rx <= HORIZEN_POND_MAX_RADIUS && p.rz <= HORIZEN_POND_MAX_RADIUS, "small: {p:?}");
        }
    }

    #[test]
    fn a_pond_stays_inside_its_square() {
        for cz in -6..6 {
            for cx in -6..6 {
                let Some(p) = pond_in_cell(3, cx, cz) else { continue };
                let (x0, z0) = ((cx * HORIZEN_POND_CELL) as f32, (cz * HORIZEN_POND_CELL) as f32);
                assert!(p.x - p.rx >= x0 && p.x + p.rx <= x0 + HORIZEN_POND_CELL as f32, "{p:?}");
                assert!(p.z - p.rz >= z0 && p.z + p.rz <= z0 + HORIZEN_POND_CELL as f32, "{p:?}");
            }
        }
    }

    #[test]
    fn the_arrival_point_is_dry_with_room_round_it() {
        for seed in 0..40 {
            for dz in -10..=10 {
                for dx in -10..=10 {
                    assert!(!is_pond(seed, HORIZEN_SPAWN.0 + dx, HORIZEN_SPAWN.1 + dz), "seed {seed}: water at {dx},{dz}");
                }
            }
        }
    }

    #[test]
    fn a_chunk_is_stone_dirt_and_ponds_and_nothing_stands_on_it() {
        let mut ponds = 0;
        for cz in -3..3 {
            for cx in -3..3 {
                let blocks = generate_chunk(11, cx, cz);
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        assert_eq!(blocks[idx(lx, 0, lz)], Block::Stone);
                        let wet = is_pond(11, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        ponds += wet as i32;
                        assert_eq!(blocks[idx(lx, GROUND_TOP, lz)], if wet { Block::Water } else { Block::Dirt });
                        assert_eq!(blocks[idx(lx, GROUND_TOP - 1, lz)], Block::Dirt);
                        for y in GROUND_TOP + 1..WORLD_HEIGHT {
                            assert_eq!(blocks[idx(lx, y, lz)], Block::Air, "nothing above the plain at {lx},{y},{lz}");
                        }
                    }
                }
            }
        }
        assert!(ponds > 0);
    }
}
