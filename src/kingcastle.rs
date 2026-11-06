//! The king's castle: the largest castle in the game, standing on the floor of the Horizen's giant cave.
//!
//! Seen from above it is a square `2 * KING_WALL_HALF + 1` blocks across (the grand castle's is `2 * GRAND_WALL_HALF + 1`): a thick outer
//! wall with battlements and a gate in the south side (towards +z), a tower at each corner, a courtyard, and in the middle a keep, a great
//! hall with pillars and slit windows, a door in its south side, and a dais in the middle with the boots of the sky on a plinth. Nothing is
//! locked: the way from the gate across the courtyard, through the keep's door and up the dais to the boots is open.
//!
//! Pure function of a cell's place relative to the castle's centre (`castle_cell`); `dy` counts up from the cave floor's top.

use crate::block::Block;
use crate::config::*;

/// Whether the column (dx, dz) is anywhere near the castle (inside the square its farthest tower reaches).
pub fn near(dx: i32, dz: i32, margin: i32) -> bool {
    dx.abs() <= KING_REACH + margin && dz.abs() <= KING_REACH + margin
}

/// A battlement: every other block of a parapet is raised.
fn merlon(dx: i32, dz: i32) -> bool {
    (dx + dz).rem_euclid(2) == 0
}

/// What stands in the cell (dx, dz) from the middle of the castle, `dy` blocks above the floor: `None` for air (and outside the castle).
pub fn castle_cell(dx: i32, dz: i32, dy: i32) -> Option<Block> {
    if dy < 0 || !near(dx, dz, 0) {
        return None;
    }
    let stone = Some(Block::CastleStone);
    let (ax, az) = (dx.abs(), dz.abs());
    let edge = ax.max(az);
    // The towers at the four corners: a hollow shell `2 * KING_TOWER_HALF + 1` wide with a floor at the top, battlements, and a doorway
    // on the side facing the courtyard.
    let (tx, tz) = (dx.signum() * KING_WALL_HALF, dz.signum() * KING_WALL_HALF);
    if dx != 0 && dz != 0 && (dx - tx).abs() <= KING_TOWER_HALF && (dz - tz).abs() <= KING_TOWER_HALF {
        let (rx, rz) = ((dx - tx).abs(), (dz - tz).abs());
        let shell = rx == KING_TOWER_HALF || rz == KING_TOWER_HALF;
        let doorway = dy < 3 && ((rx == KING_TOWER_HALF && (dx - tx).signum() == -dx.signum() && rz <= 1) || (rz == KING_TOWER_HALF && (dz - tz).signum() == -dz.signum() && rx <= 1));
        return match dy {
            d if d < KING_TOWER_HEIGHT && shell && !doorway => stone,
            d if d == KING_TOWER_HEIGHT => stone,
            d if d == KING_TOWER_HEIGHT + 1 && shell && merlon(dx, dz) => stone,
            _ => None,
        };
    }
    // The outer wall, two blocks thick, with the gate (an arch `2 * KING_GATE_HALF + 1` wide) in the south side.
    if edge >= KING_WALL_HALF - 1 && edge <= KING_WALL_HALF {
        let gate = dz > 0 && az >= KING_WALL_HALF - 1 && ax <= KING_GATE_HALF && dy < KING_WALL_HEIGHT - 3;
        return match dy {
            d if d < KING_WALL_HEIGHT && !gate => stone,
            d if d == KING_WALL_HEIGHT && !gate && merlon(dx, dz) => stone,
            _ => None,
        };
    }
    // The keep: walls two blocks thick with slit windows, pillars in the hall, a flat roof with battlements, a door in the south side.
    if edge <= KING_KEEP_HALF {
        let wall = edge >= KING_KEEP_HALF - 1;
        let door = dz > 0 && az >= KING_KEEP_HALF - 1 && ax <= KING_GATE_HALF && dy < 8;
        let window = dy >= 9 && dy <= 12 && (ax.rem_euclid(5) == 2 && az >= KING_KEEP_HALF - 1 || az.rem_euclid(5) == 2 && ax >= KING_KEEP_HALF - 1);
        let pillar = ax % 8 == 0 && az % 8 == 0 && ax >= 8 && az >= 8 && edge < KING_KEEP_HALF - 1 || (ax == 8 && az == 0) || (ax == 0 && az == 8 && dz < 0);
        if dy < KING_KEEP_HEIGHT {
            if wall && !door && !window {
                return stone;
            }
            if !wall && pillar {
                return stone;
            }
            // The dais in the middle of the hall: two steps, and the plinth on top.
            if !wall {
                if edge <= 3 && dy == 0 {
                    return stone;
                }
                if edge <= 2 && dy == 1 {
                    return stone;
                }
                if (ax, az) == (0, 0) && dy == 2 {
                    return Some(Block::BootsPlinth);
                }
            }
            return None;
        }
        return match dy {
            d if d == KING_KEEP_HEIGHT => stone,
            d if d == KING_KEEP_HEIGHT + 1 && edge == KING_KEEP_HALF && merlon(dx, dz) => stone,
            _ => None,
        };
    }
    None
}

/// The height, above the floor, of the highest block the castle has anywhere.
pub const CASTLE_HEIGHT: i32 = KING_TOWER_HEIGHT + 2;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn it_is_the_largest_castle_in_the_game() {
        assert!(KING_WALL_HALF > GRAND_WALL_HALF + 10 && KING_REACH > GRAND_REACH + 10 && KING_KEEP_HALF > GRAND_KEEP_HALF + 4);
        assert!(KING_TOWER_HEIGHT > GRAND_HEIGHT - 4, "taller towers than the grand castle keep");
        assert!(CASTLE_HEIGHT as i32 + 2 <= KING_CAVE_HEIGHT - 20, "the cave's roof is well above it");
    }

    #[test]
    fn nothing_stands_outside_the_castle_and_the_boots_are_in_the_middle() {
        for dy in 0..CASTLE_HEIGHT + 4 {
            assert_eq!(castle_cell(KING_REACH + 1, 0, dy), None);
            assert_eq!(castle_cell(0, -KING_REACH - 1, dy), None);
        }
        assert_eq!(castle_cell(0, 0, 2), Some(Block::BootsPlinth), "on the dais in the middle of the keep");
        assert_eq!(castle_cell(0, 0, 3), None);
        assert_eq!(castle_cell(KING_WALL_HALF, -20, 5), Some(Block::CastleStone), "the outer wall");
        assert_eq!(castle_cell(KING_WALL_HALF, KING_WALL_HALF, KING_TOWER_HEIGHT), Some(Block::CastleStone), "a tower's floor at the top");
        assert!((0..CASTLE_HEIGHT).any(|dy| castle_cell(KING_WALL_HALF + 2, KING_WALL_HALF + 2, dy).is_some()), "the corner towers");
        assert!(CASTLE_HEIGHT > KING_TOWER_HEIGHT);
    }

    /// Whether a player could stand with their feet at height `y` in the column (x, z): two free cells, something to stand on.
    fn standable(x: i32, z: i32, y: i32) -> bool {
        let free = |dy: i32| castle_cell(x, z, dy).is_none();
        free(y) && free(y + 1) && (y == 0 || matches!(castle_cell(x, z, y - 1), Some(Block::CastleStone)))
    }

    #[test]
    fn the_way_from_the_gate_through_the_keep_to_the_boots_can_be_walked_a_block_at_a_time() {
        let start = (0, KING_REACH + 2, 0);
        let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
        let mut queue = VecDeque::from([start]);
        seen.insert(start);
        while let Some((x, z, y)) = queue.pop_front() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (x + dx, z + dz);
                if nx.abs() > KING_REACH + 3 || nz.abs() > KING_REACH + 3 {
                    continue;
                }
                for ny in [y - 1, y, y + 1] {
                    if ny >= 0 && standable(nx, nz, ny) && seen.insert((nx, nz, ny)) {
                        queue.push_back((nx, nz, ny));
                    }
                }
            }
        }
        assert!(seen.contains(&(0, KING_WALL_HALF, 0)), "through the gate");
        assert!(seen.contains(&(0, KING_KEEP_HALF + 4, 0)), "across the courtyard");
        assert!(seen.contains(&(0, KING_KEEP_HALF, 0)), "through the keep's door");
        assert!(seen.contains(&(0, 1, 2)), "up the dais, right beside the plinth");
        assert!(!seen.iter().any(|&(x, z, y)| x.abs() >= KING_WALL_HALF - 3 && z.abs() >= KING_WALL_HALF - 3 && x.abs() <= KING_WALL_HALF + 3 && z.abs() <= KING_WALL_HALF + 3 && y == 0 && x.abs() == KING_WALL_HALF + KING_TOWER_HALF), "no gap in a tower's outer shell");
    }
}
