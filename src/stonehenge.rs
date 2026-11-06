//! Stonehenge: a ring of tall standing stones with lintels over some of them, round an altar stone. It stands
//! `STONEHENGE_DISTANCE_CHUNKS` chunks from the spawn point, in a direction the seed decides (a pure function of the seed).
//! A player who blacks out in the Horizen's cave wakes inside the ring (`arrival`). There is, as yet, nothing to do here
//! (the Nessie ghost tells of a horn that is to be blown here).

use crate::block::Block;
use crate::config::*;
use crate::rng::hash3;

/// The column at the middle of the ring.
pub fn centre(seed: i64) -> (i32, i32) {
    let angle = (hash3(seed, 31, 7, 0x5748) % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
    let dist = (STONEHENGE_DISTANCE_CHUNKS * CHUNK_SIZE) as f32;
    let spawn = (CHUNK_SIZE / 2) as f32;
    ((spawn + angle.cos() * dist).round() as i32, (spawn + angle.sin() * dist).round() as i32)
}

/// Where a player who comes to here stands: in the ring, south of the altar stone (middle of a column).
pub fn arrival(seed: i64) -> (f32, f32) {
    let (x, z) = centre(seed);
    (x as f32 + 0.5, z as f32 + 3.5)
}

/// Whether the column (wx, wz) is within `margin` blocks of the stones' square.
pub fn near(seed: i64, wx: i32, wz: i32, margin: i32) -> bool {
    let (x, z) = centre(seed);
    (wx - x).abs() <= STONEHENGE_REACH + margin && (wz - z).abs() <= STONEHENGE_REACH + margin
}

/// The south-west column of standing stone `i`, which is two by two blocks.
fn stone_corner(i: i32) -> (i32, i32) {
    let a = i as f32 * std::f32::consts::TAU / STONEHENGE_STONES as f32;
    ((a.cos() * STONEHENGE_RADIUS - 1.0).round() as i32, (a.sin() * STONEHENGE_RADIUS - 1.0).round() as i32)
}

/// What stands in the cell (`dx`, `dz`) from the middle of the ring, `dy` blocks above the ground there: the standing stones, a
/// lintel across the top of every pair of them (stones 0 and 1, 2 and 3, and so on, which leaves a gap between the pairs), and
/// the altar stone in the middle (three long, two high). `Block::Air` elsewhere.
pub fn stone_cell(dx: i32, dz: i32, dy: i32) -> Block {
    if dy < 0 {
        return Block::Air;
    }
    if (-1..=1).contains(&dx) && dz == 0 && dy < 2 {
        return Block::CastleStone;
    }
    for i in 0..STONEHENGE_STONES {
        let (sx, sz) = stone_corner(i);
        if (sx..=sx + 1).contains(&dx) && (sz..=sz + 1).contains(&dz) && dy < STONEHENGE_STONE_HEIGHT {
            return Block::CastleStone;
        }
    }
    if dy == STONEHENGE_STONE_HEIGHT {
        for i in (0..STONEHENGE_STONES).step_by(2) {
            let (a, b) = (stone_corner(i), stone_corner(i + 1));
            let steps = 40;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let (px, pz) = (a.0 as f32 + 0.5 + (b.0 - a.0) as f32 * t, a.1 as f32 + 0.5 + (b.1 - a.1) as f32 * t);
                if (dx as f32 + 0.5 - px).abs() <= 1.0 && (dz as f32 + 0.5 - pz).abs() <= 1.0 {
                    return Block::CastleStone;
                }
            }
        }
    }
    Block::Air
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn it_stands_about_five_chunks_from_the_spawn_point_in_a_direction_the_seed_decides() {
        let spawn = (CHUNK_SIZE / 2) as f32;
        for seed in 0..20 {
            let (x, z) = centre(seed);
            let gap = ((x as f32 - spawn).powi(2) + (z as f32 - spawn).powi(2)).sqrt();
            assert!((gap - (STONEHENGE_DISTANCE_CHUNKS * CHUNK_SIZE) as f32).abs() < 2.0, "seed {seed}: {gap}");
        }
        assert_ne!(centre(1), centre(2));
    }

    #[test]
    fn a_ring_of_tall_stones_with_lintels_in_pairs_and_an_altar_stone_in_the_middle() {
        let top = STONEHENGE_STONE_HEIGHT;
        let stones = (0..STONEHENGE_STONES).filter(|&i| {
            let (sx, sz) = stone_corner(i);
            (0..top).all(|dy| stone_cell(sx, sz, dy) == Block::CastleStone) && stone_cell(sx, sz, top + 1) == Block::Air
        });
        assert_eq!(stones.count() as i32, STONEHENGE_STONES, "every stone stands the full height");
        assert_eq!(stone_cell(0, 0, 1), Block::CastleStone, "the altar stone");
        assert_eq!(stone_cell(0, 0, 2), Block::Air);
        let (a, b) = (stone_corner(0), stone_corner(1));
        let mid = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        assert_eq!(stone_cell(mid.0, mid.1, top), Block::CastleStone, "a lintel between the first pair");
        let (c, d) = (stone_corner(1), stone_corner(2));
        let gap = ((c.0 + d.0) / 2, (c.1 + d.1) / 2);
        assert_eq!(stone_cell(gap.0, gap.1, top), Block::Air, "and none between one pair and the next: an entrance");
    }

    #[test]
    fn the_ring_can_be_walked_into_and_the_arrival_spot_is_inside_it_and_free() {
        let free = |x: i32, z: i32| (0..2).all(|dy| stone_cell(x, z, dy) == Block::Air);
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([(STONEHENGE_REACH, STONEHENGE_REACH)]);
        seen.insert((STONEHENGE_REACH, STONEHENGE_REACH));
        while let Some((x, z)) = queue.pop_front() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (x + dx, z + dz);
                if n.0.abs() <= STONEHENGE_REACH && n.1.abs() <= STONEHENGE_REACH && free(n.0, n.1) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        assert!(seen.contains(&(0, 3)), "from outside the ring one can walk in to the altar");
        assert!(seen.contains(&(0, 1)) && !seen.contains(&(0, 0)), "up to the altar stone and no further");
        let seed = 6;
        let (cx, cz) = centre(seed);
        let (ax, az) = arrival(seed);
        let (dx, dz) = (ax.floor() as i32 - cx, az.floor() as i32 - cz);
        assert!(free(dx, dz) && seen.contains(&(dx, dz)) && dx * dx + dz * dz < (STONEHENGE_RADIUS as i32).pow(2));
        assert!(near(seed, cx + STONEHENGE_REACH, cz, 0) && !near(seed, cx + STONEHENGE_REACH + 1, cz, 0));
    }
}
