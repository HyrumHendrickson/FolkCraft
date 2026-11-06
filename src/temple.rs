//! The jungle temple: a big Mayan-style stepped pyramid of pale stone in the bare middle of the jungle, with a broad stair
//! up its south face and a shrine on its top, where a pile of gold lies.
//!
//! Pure functions of the position relative to the temple's centre: `temple_cell(dx, dz, dy)` is what stands there, `dy` blocks
//! above the ground at the centre of the jungle (`terrain::jungle_centre`). The terrain generator builds it (and fills the
//! ground under it) in `terrain::generate_chunk_for`.

use crate::block::Block;
use crate::config::*;

/// The farthest the temple reaches from its centre, along either axis.
pub const TEMPLE_REACH: i32 = TEMPLE_BASE_HALF;

/// The height of the pyramid's top platform above the ground.
pub const TEMPLE_TOP: i32 = TEMPLE_TIERS * TEMPLE_TIER_HEIGHT;

/// What the temple has in the cell (`dx`, `dz`) from its centre and `dy` blocks above the ground there: `Block::Air` where
/// nothing stands (also outside the temple).
///
/// - `TEMPLE_TIERS` square tiers, each `TEMPLE_TIER_HEIGHT` high and two blocks narrower on every side than the one below,
/// - a stair `2 * TEMPLE_STAIR_HALF + 1` wide up the south side (towards +z), a block a step, from the ground to the top,
/// - on the top platform a shrine: walls, a doorway in the south side, a flat roof and a tall roof comb along it, with a pile
///   of gold on the floor in the middle.
pub fn temple_cell(dx: i32, dz: i32, dy: i32) -> Block {
    let (ax, az) = (dx.abs(), dz.abs());
    let edge = ax.max(az);
    if edge > TEMPLE_BASE_HALF || dy < 0 {
        return Block::Air;
    }
    let tier = ((TEMPLE_BASE_HALF - edge) / 2).min(TEMPLE_TIERS - 1);
    let mut height = TEMPLE_TIER_HEIGHT * (tier + 1);
    if dz > 0 && ax <= TEMPLE_STAIR_HALF {
        height = height.min(TEMPLE_BASE_HALF + 1 - dz);
    }
    if dy < height {
        return Block::MayaStone;
    }
    // The shrine on the top platform.
    let rel = dy - TEMPLE_TOP;
    if rel < 0 || ax > 3 || az > 3 {
        return Block::Air;
    }
    let wall = ax == 3 || az == 3;
    match rel {
        0 if (ax, az) == (0, 0) => Block::GoldPile,
        0..=3 if wall => {
            let doorway = dz == 3 && ax <= 1 && rel < 3;
            if doorway { Block::Air } else { Block::MayaStone }
        }
        4 => Block::MayaStone,
        5..=8 if az == 0 => Block::MayaStone,
        _ => Block::Air,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    fn surface(dx: i32, dz: i32) -> i32 {
        (0..TEMPLE_HEIGHT).rev().find(|&dy| temple_cell(dx, dz, dy) != Block::Air && temple_cell(dx, dz, dy) != Block::GoldPile).map_or(0, |dy| dy + 1)
    }

    #[test]
    fn the_temple_is_big_and_steps_up_in_tiers_to_a_platform_with_a_shrine() {
        assert!(TEMPLE_BASE_HALF >= 20 && TEMPLE_TIERS >= 8, "a big pyramid");
        let side = (0..TEMPLE_HEIGHT).filter(|&dy| temple_cell(-TEMPLE_BASE_HALF, 0, dy) == Block::MayaStone).count() as i32;
        assert_eq!(side, TEMPLE_TIER_HEIGHT, "the bottom tier");
        let mut last = 0;
        for edge in (0..=TEMPLE_BASE_HALF).rev() {
            let h = surface(-edge, 0);
            assert!(h >= last, "never steps down on the way in: {edge}");
            last = h;
        }
        assert_eq!(surface(-4, 0), TEMPLE_TOP, "the platform is {} high", TEMPLE_TOP);
        assert_eq!(temple_cell(0, 0, TEMPLE_TOP), Block::GoldPile, "gold on the shrine floor");
        assert!(TEMPLE_HEIGHT > TEMPLE_TOP + 9, "room for the roof comb");
        for dy in 0..TEMPLE_HEIGHT {
            assert_eq!(temple_cell(TEMPLE_BASE_HALF + 1, 0, dy), Block::Air, "nothing outside");
        }
    }

    #[test]
    fn the_stair_climbs_a_block_a_step_from_the_ground_to_the_shrine_door() {
        // Walk from outside the south face as a player does: feet in free air with solid stone (or the ground) under them,
        // and a step up or down of one block at most.
        let start = (0, TEMPLE_BASE_HALF + 3);
        let free = |x: i32, z: i32, y: i32| (0..2).all(|k| matches!(temple_cell(x, z, y + k), Block::Air | Block::GoldPile));
        let supported = |x: i32, z: i32, y: i32| y == 0 || temple_cell(x, z, y - 1) == Block::MayaStone;
        let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
        let mut queue = VecDeque::from([(start.0, start.1, 0)]);
        seen.insert((start.0, start.1, 0));
        while let Some((x, z, y)) = queue.pop_front() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (x + dx, z + dz);
                if nx.abs() > TEMPLE_BASE_HALF + 4 || nz.abs() > TEMPLE_BASE_HALF + 4 {
                    continue;
                }
                for ny in [y - 1, y, y + 1] {
                    if ny >= 0 && free(nx, nz, ny) && supported(nx, nz, ny) && seen.insert((nx, nz, ny)) {
                        queue.push_back((nx, nz, ny));
                    }
                }
            }
        }
        assert!(seen.contains(&(0, 4, TEMPLE_TOP - 1)), "the last step but one, in front of the doorway");
        assert!(seen.contains(&(0, 3, TEMPLE_TOP)), "the doorway");
        assert!(seen.contains(&(0, 1, TEMPLE_TOP)), "inside the shrine, on its floor, is reachable on foot");
        assert!(!seen.iter().any(|&(x, z, y)| y == TEMPLE_TOP && z < -4 && x.abs() < TEMPLE_BASE_HALF), "the platform is only the top tier, with nothing level with it behind");
    }

    #[test]
    fn the_shrine_has_walls_a_south_doorway_a_roof_and_a_roof_comb() {
        for rel in 0..3 {
            assert_eq!(temple_cell(0, 3, TEMPLE_TOP + rel), Block::Air, "doorway at {rel}");
            assert_eq!(temple_cell(0, -3, TEMPLE_TOP + rel), Block::MayaStone, "north wall at {rel}");
        }
        assert_eq!(temple_cell(0, 3, TEMPLE_TOP + 3), Block::MayaStone, "lintel");
        assert_eq!(temple_cell(1, 1, TEMPLE_TOP + 4), Block::MayaStone, "roof");
        assert_eq!(temple_cell(2, 0, TEMPLE_TOP + 7), Block::MayaStone, "roof comb");
        assert_eq!(temple_cell(2, 2, TEMPLE_TOP + 7), Block::Air);
    }
}
