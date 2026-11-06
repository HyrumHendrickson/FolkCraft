//! The silver tower: one design, built three times.
//!
//! One stands on the island of a lake (where the small castle used to be), one on an island of the Skylands that appears
//! after the player has walked on ten islands, and one on the plain of the Horizen, which a peep stone leads to by day.
//! They are identical but for the relic on the plinth at the top: the Philosopher's Stone, the Horn of the Hunt or the
//! Ancient Crown.
//!
//! Seen from above the tower is a square `2 * TOWER_HALF + 1` blocks across. Its walls are `Block::SilverStone`, with
//! a slit window in the middle of each side on every floor, and a door in the south side (towards +z) that is locked
//! (`Block::TowerDoor`) until a silver key has been used on it. Inside, five floors stand `TOWER_STOREY` blocks apart,
//! each a slab with a stair that climbs against a wall, a quarter turn further round on every floor (the stairs are
//! `Block::SilverStone` too, one more block high at every step). The roof has battlements. On the plinth in the middle of
//! the top floor lies the relic.
//!
//! Everything here is a pure function of a cell's place in the tower (`tower_cell`), so each world only has to ask
//! for the cells of the chunk it is making.

use crate::block::Block;
use crate::config::*;
use crate::rng::hash3;

/// What lies on the plinth at the top of a tower.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relic {
    /// The tower on the lake's island.
    PhilosophersStone,
    /// The tower in the Skylands.
    HornOfTheHunt,
    /// The tower in the Horizen.
    AncientCrown,
}

impl Relic {
    /// The plinth block that holds this relic.
    pub fn block(self) -> Block {
        match self {
            Relic::PhilosophersStone => Block::StonePlinth,
            Relic::HornOfTheHunt => Block::HornPlinth,
            Relic::AncientCrown => Block::CrownPlinth,
        }
    }
}

/// Turn a column of the tower's floor plan a quarter turn, `turns` times.
fn turn(mut x: i32, mut z: i32, turns: i32) -> (i32, i32) {
    for _ in 0..turns.rem_euclid(4) {
        (x, z) = (-z, x);
    }
    (x, z)
}

/// Where the stair of floor `storey` (0 is the ground floor) has a column at (dx, dz), as how many steps up it stands there
/// (1 to 4): the stair runs along the north wall on the ground floor and turns a quarter turn on each floor above. `None`
/// where it has no step.
fn stair_step(dx: i32, dz: i32, storey: i32) -> Option<i32> {
    // Turn the column back into the ground floor's plan, where the steps run east along the north wall.
    let (bx, bz) = turn(dx, dz, 4 - storey.rem_euclid(4));
    (bz == -(TOWER_HALF - 1) && (-2..=1).contains(&bx)).then_some(bx + 3)
}

/// Whether the slab over floor `storey` has a hole at (dx, dz) for the head of whoever climbs the stair: over its last
/// three steps, so there is room to hop up from one to the next.
fn stair_hole(dx: i32, dz: i32, storey: i32) -> bool {
    matches!(stair_step(dx, dz, storey), Some(2) | Some(3) | Some(4))
}

/// What the tower has in the cell (`dx`, `dz`) blocks from its centre and `dy` blocks above the ground (0 is the first
/// layer of air): `Some(block)` inside the tower's square and height, `None` outside it. `locked` says whether the door
/// is still shut.
pub fn tower_cell(dx: i32, dz: i32, dy: i32, locked: bool, relic: Relic) -> Option<Block> {
    let half = TOWER_HALF;
    if dx.abs() > half || dz.abs() > half || !(0..TOWER_HEIGHT).contains(&dy) {
        return None;
    }
    let roof = TOWER_STOREY * TOWER_STOREYS - 1;
    let merlon = (dx + dz).rem_euclid(2) == 0;
    // The roof, and its battlements.
    if dy == roof {
        return Some(Block::SilverStone);
    }
    if dy == roof + 1 {
        return Some(if dx.abs().max(dz.abs()) == half && merlon { Block::SilverStone } else { Block::Air });
    }
    let (storey, rise) = (dy / TOWER_STOREY, dy % TOWER_STOREY);
    // The walls: a door in the south side, a slit window in the middle of each side on every floor.
    if dx.abs().max(dz.abs()) == half {
        let door = dx == 0 && dz == half;
        if door && dy <= 1 {
            return Some(match (locked, dy) {
                (false, _) => Block::Air,
                (true, 0) => Block::TowerDoor,
                (true, _) => Block::TowerDoorTop,
            });
        }
        let window = (dx == 0 || dz == 0) && rise == 2;
        return Some(if window { Block::Air } else { Block::SilverStone });
    }
    // Inside: the slab at the top of each floor (with a hole over the stair), the steps, and the plinth.
    if rise == TOWER_STOREY - 1 {
        return Some(if stair_hole(dx, dz, storey) { Block::Air } else { Block::SilverStone });
    }
    if storey < TOWER_STOREYS - 1 {
        if let Some(step) = stair_step(dx, dz, storey) {
            if rise < step {
                return Some(Block::SilverStone);
            }
        }
    } else if (dx, dz, rise) == (0, 0, 0) {
        return Some(relic.block());
    }
    Some(Block::Air)
}

/// What a **ruined** tower has in the cell (`dx`, `dz`, `dy`): the tower of a lake's island while its Loch Ness Monster lives.
/// The walls have tumbled to ragged stumps of `1..=RUINED_WALL_MAX` blocks (the corners stand taller, but no one can climb
/// them), the doorway is a gap with no door, the floors and the roof are gone with the relic's plinth, rubble lies about the
/// ground floor, and the stair is broken off after its second step, so there is no way up. `Some(Air)` anywhere else inside the
/// tower's square (so the ground there is cleared as for a whole tower), `None` outside it.
pub fn ruined_tower_cell(dx: i32, dz: i32, dy: i32) -> Option<Block> {
    let half = TOWER_HALF;
    if dx.abs() > half || dz.abs() > half || !(0..TOWER_HEIGHT).contains(&dy) {
        return None;
    }
    let roll = |salt: i32| hash3(RUINED_TOWER_SEED, dx, dz, salt) as i32;
    let solid = if dx.abs().max(dz.abs()) == half {
        let doorway = dx == 0 && dz == half;
        let height = if dx.abs() == half && dz.abs() == half { RUINED_CORNER_HEIGHT } else { 1 + roll(1).rem_euclid(RUINED_WALL_MAX) };
        !doorway && dy < height
    } else if let Some(step) = stair_step(dx, dz, 0) {
        dy < step.min(RUINED_STAIR_STEPS)
    } else {
        dy == 0 && roll(2).rem_euclid(100) < RUINED_RUBBLE_PERCENT
    };
    Some(if solid { Block::SilverStone } else { Block::Air })
}

/// Whether the column (dx, dz) from a tower's centre is within `margin` blocks of its square.
pub fn near(dx: i32, dz: i32, margin: i32) -> bool {
    dx.abs() <= TOWER_REACH + margin && dz.abs() <= TOWER_REACH + margin
}

/// A free-standing tower (the one in the Horizen): where its centre column is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tower {
    pub x: i32,
    pub z: i32,
}

impl Tower {
    /// The spot a few blocks in front of the door (on the south side) where a ghostly trail ends, and the door itself.
    pub fn approach(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, (self.z + TOWER_HALF + 5) as f32 + 0.5)
    }

    /// The middle of the tower's square.
    pub fn middle(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, self.z as f32 + 0.5)
    }

    /// A tower whose nearest edge is `tower_distance()` blocks from `at` (counting along either axis, as the chunks do),
    /// in the direction `angle`, pushed `extra` blocks further out.
    #[cfg(test)]
    pub fn ahead(at: (f32, f32), angle: f32, extra: f32) -> Tower {
        let (c, s) = (angle.cos(), angle.sin());
        let gap = (tower_distance() as f32 + extra + TOWER_REACH as f32) / c.abs().max(s.abs());
        Tower { x: (at.0 + c * gap).round() as i32, z: (at.1 + s * gap).round() as i32 }
    }
}

/// Blocks round an island's shore that its tower's chunks can reach (used to tell whether an island is yet made).
pub const TOWER_FOOT_MARGIN: i32 = 2;

/// How far (blocks) from the player a new tower's nearest edge lies: past the generated land, as for every structure.
#[cfg(test)]
pub fn tower_distance() -> i32 {
    structure_distance()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    /// The tower on flat ground: ground below dy 0 everywhere, the tower's cells above, open air elsewhere.
    fn block(dx: i32, dz: i32, dy: i32, locked: bool) -> Block {
        if dy < 0 {
            return Block::Dirt;
        }
        tower_cell(dx, dz, dy, locked, Relic::PhilosophersStone).unwrap_or(Block::Air)
    }

    #[test]
    fn the_tower_is_a_square_of_five_storeys_with_a_floor_over_each() {
        assert_eq!(TOWER_HEIGHT, 26);
        assert_eq!(tower_cell(TOWER_HALF + 1, 0, 3, true, Relic::AncientCrown), None, "outside the square");
        assert_eq!(tower_cell(0, 0, TOWER_HEIGHT, true, Relic::AncientCrown), None, "above the battlements");
        assert_eq!(tower_cell(0, 0, -1, true, Relic::AncientCrown), None, "below the ground");
        // A floor slab at the top of each storey, and the roof over the last.
        for storey in 0..TOWER_STOREYS {
            let slab = storey * TOWER_STOREY + TOWER_STOREY - 1;
            assert_eq!(block(3, 3, slab, true), Block::SilverStone, "floor over storey {storey}");
            for rise in 0..TOWER_STOREY - 1 {
                assert_eq!(block(3, 3, storey * TOWER_STOREY + rise, true), Block::Air, "room on storey {storey}");
            }
        }
        // Battlements: every other block along the top of the wall.
        let tops: Vec<Block> = (-TOWER_HALF..=TOWER_HALF).map(|x| block(x, TOWER_HALF, TOWER_HEIGHT - 1, true)).collect();
        assert!(tops.contains(&Block::SilverStone) && tops.contains(&Block::Air));
        assert_eq!(block(0, 0, TOWER_HEIGHT - 1, true), Block::Air, "nothing inside the battlements");
    }

    #[test]
    fn the_door_is_in_the_south_side_and_locked_until_it_is_opened() {
        assert_eq!(block(0, TOWER_HALF, 0, true), Block::TowerDoor);
        assert_eq!(block(0, TOWER_HALF, 1, true), Block::TowerDoorTop);
        assert_eq!(block(0, TOWER_HALF, 0, false), Block::Air);
        assert_eq!(block(0, TOWER_HALF, 1, false), Block::Air);
        assert_eq!(block(1, TOWER_HALF, 0, false), Block::SilverStone, "the doorway is one block wide");
        assert_eq!(block(0, -TOWER_HALF, 0, true), Block::SilverStone, "no door on the north side");
        // A window in the middle of each side, on every floor, and none to climb through at the corners.
        assert_eq!(block(TOWER_HALF, 0, 2, true), Block::Air);
        assert_eq!(block(-TOWER_HALF, 0, TOWER_STOREY + 2, true), Block::Air);
        assert_eq!(block(TOWER_HALF, TOWER_HALF, 2, true), Block::SilverStone);
    }

    #[test]
    fn the_relic_lies_on_the_plinth_in_the_middle_of_the_top_floor_and_nowhere_else() {
        let top = (TOWER_STOREYS - 1) * TOWER_STOREY;
        for relic in [Relic::PhilosophersStone, Relic::HornOfTheHunt, Relic::AncientCrown] {
            assert_eq!(tower_cell(0, 0, top, true, relic), Some(relic.block()));
            let mut plinths = 0;
            for dz in -TOWER_HALF..=TOWER_HALF {
                for dx in -TOWER_HALF..=TOWER_HALF {
                    for dy in 0..TOWER_HEIGHT {
                        plinths += (tower_cell(dx, dz, dy, true, relic) == Some(relic.block())) as i32;
                    }
                }
            }
            assert_eq!(plinths, 1, "{relic:?}");
        }
    }

    /// The blocks of the tower on flat ground, as a function of (dx, dz, dy): whole (`ruined` false, door `locked` or not), or the ruin.
    type Cells<'a> = &'a dyn Fn(i32, i32, i32) -> Block;

    /// Whether a player could stand with their feet in the cell (x, y, z): room for the body, a floor under it.
    fn can_stand(cell: Cells, x: i32, y: i32, z: i32) -> bool {
        !cell(x, z, y).collides() && !cell(x, z, y + 1).collides() && cell(x, z, y - 1).collides()
    }

    fn climbable(locked: bool) -> HashSet<(i32, i32, i32)> {
        reachable(&|x, z, y| block(x, z, y, locked))
    }

    /// Every place a player can stand that can be reached from outside the door by walking, hopping up one block at a time
    /// (and dropping up to three), and so, for a whole tower with an open door, the top floor with the relic.
    fn reachable(cell: Cells) -> HashSet<(i32, i32, i32)> {
        let start = (0, TOWER_HALF + 2, 0);
        assert!(can_stand(cell, start.0, start.2, start.1));
        let mut seen: HashSet<(i32, i32, i32)> = HashSet::from([(start.0, start.2, start.1)]);
        let mut queue = VecDeque::from([(start.0, start.2, start.1)]);
        while let Some((x, y, z)) = queue.pop_front() {
            for (ox, oz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (x + ox, z + oz);
                if nx.abs() > TOWER_HALF + 8 || nz.abs() > TOWER_HALF + 8 {
                    continue;
                }
                // Level, a hop up of one block (with room over the head for it), or a drop of up to three.
                for ny in [y, y + 1, y - 1, y - 2, y - 3] {
                    let hop_room = ny <= y || !cell(x, z, y + 2).collides();
                    if hop_room && can_stand(cell, nx, ny, nz) && seen.insert((nx, ny, nz)) {
                        queue.push_back((nx, ny, nz));
                        break;
                    }
                }
            }
        }
        seen
    }

    #[test]
    fn a_locked_door_keeps_everyone_out_and_an_open_one_lets_them_climb_to_the_relic() {
        let locked = climbable(true);
        assert!(locked.iter().all(|&(x, _, z)| x.abs().max(z.abs()) > TOWER_HALF), "no way in past a locked door");
        let open = climbable(false);
        let top = (TOWER_STOREYS - 1) * TOWER_STOREY;
        for storey in 0..TOWER_STOREYS {
            let floor = storey * TOWER_STOREY;
            assert!(open.iter().any(|&(x, y, z)| y == floor && x.abs().max(z.abs()) < TOWER_HALF), "storey {storey} cannot be reached");
        }
        // Standing beside the plinth on the top floor.
        assert!(open.iter().any(|&(x, y, z)| y == top && (x, z) != (0, 0) && x.abs() <= 1 && z.abs() <= 1));
        // Nobody climbs out on to the roof, and every standing place is inside or in front of the tower.
        assert!(open.iter().all(|&(_, y, _)| y <= top + 1), "only the plinth is a step higher than the top floor");
    }

    /// The ruin on flat ground, like `block`.
    fn ruin(dx: i32, dz: i32, dy: i32) -> Block {
        if dy < 0 {
            return Block::Dirt;
        }
        ruined_tower_cell(dx, dz, dy).unwrap_or(Block::Air)
    }

    #[test]
    fn a_ruined_tower_is_low_ragged_stumps_with_a_gap_for_a_door_and_a_broken_stair() {
        assert_eq!(ruined_tower_cell(TOWER_HALF + 1, 0, 0), None, "outside the square");
        assert_eq!(ruined_tower_cell(0, 0, TOWER_HEIGHT), None, "above where the battlements were");
        assert_eq!(ruined_tower_cell(0, 0, 10), Some(Block::Air), "cleared inside its square, so the island's plants are gone");
        // Nothing is left of the floors, the roof or the relic's plinth, and there is no door to lock.
        let (mut stones, mut highest) = (0, 0);
        for dz in -TOWER_HALF..=TOWER_HALF {
            for dx in -TOWER_HALF..=TOWER_HALF {
                for dy in 0..TOWER_HEIGHT {
                    let cell = ruin(dx, dz, dy);
                    assert!(!matches!(cell, Block::TowerDoor | Block::TowerDoorTop | Block::StonePlinth | Block::HornPlinth | Block::CrownPlinth));
                    if cell == Block::SilverStone {
                        stones += 1;
                        highest = highest.max(dy + 1);
                    }
                }
            }
        }
        assert!(stones > 20, "there is something left to see: {stones}");
        assert_eq!(highest, RUINED_CORNER_HEIGHT, "only the corners stand above the stumps");
        for dy in 0..TOWER_HEIGHT {
            assert_eq!(ruin(0, TOWER_HALF, dy), Block::Air, "the doorway is an empty gap");
        }
        let stump = |dx: i32, dz: i32| (0..TOWER_HEIGHT).take_while(|&dy| ruin(dx, dz, dy) == Block::SilverStone).count() as i32;
        for dz in -TOWER_HALF..=TOWER_HALF {
            for dx in -TOWER_HALF..=TOWER_HALF {
                if dx.abs().max(dz.abs()) == TOWER_HALF && !(dx == 0 && dz == TOWER_HALF) && !(dx.abs() == TOWER_HALF && dz.abs() == TOWER_HALF) {
                    assert!((1..=RUINED_WALL_MAX).contains(&stump(dx, dz)), "a stump at {dx},{dz}");
                }
            }
        }
        // The stair is broken off after its second step; the whole one has four.
        let stair: Vec<(i32, i32)> = (-TOWER_HALF + 1..TOWER_HALF).flat_map(|dz| (-TOWER_HALF + 1..TOWER_HALF).map(move |dx| (dx, dz))).filter(|&(dx, dz)| stair_step(dx, dz, 0).is_some()).collect();
        assert_eq!(stair.len(), 4);
        assert_eq!(stair.iter().map(|&(dx, dz)| stump(dx, dz)).max(), Some(RUINED_STAIR_STEPS), "no step higher than the second");
        // It is the same ruin every time.
        assert_eq!(ruin(2, 4, 1), ruin(2, 4, 1));
    }

    #[test]
    fn nobody_can_climb_a_ruined_tower_but_the_ground_floor_can_be_walked() {
        let seen = reachable(&ruin);
        assert!(seen.iter().any(|&(x, _, z)| x.abs() < TOWER_HALF && z.abs() < TOWER_HALF), "the doorway lets them in");
        let top = seen.iter().map(|&(_, y, _)| y).max().unwrap();
        assert!(top <= RUINED_WALL_MAX, "the highest place anyone can stand is {top}");
        assert!(top < TOWER_STOREY, "never as high as the first floor was");
        // The whole tower, by contrast, climbs to the top once its door is open.
        assert!(reachable(&|x, z, y| block(x, z, y, false)).iter().any(|&(_, y, _)| y >= TOWER_STOREY * (TOWER_STOREYS - 1)));
    }

    #[test]
    fn each_stair_has_four_steps_that_rise_one_block_at_a_time_and_turn_a_quarter_each_floor() {
        for storey in 0..TOWER_STOREYS - 1 {
            let mut steps: Vec<(i32, i32, i32)> = Vec::new();
            for dz in -TOWER_HALF + 1..TOWER_HALF {
                for dx in -TOWER_HALF + 1..TOWER_HALF {
                    if let Some(step) = stair_step(dx, dz, storey) {
                        steps.push((step, dx, dz));
                    }
                }
            }
            steps.sort();
            assert_eq!(steps.iter().map(|s| s.0).collect::<Vec<_>>(), vec![1, 2, 3, 4], "storey {storey}");
            for pair in steps.windows(2) {
                assert_eq!((pair[0].1 - pair[1].1).abs() + (pair[0].2 - pair[1].2).abs(), 1, "steps are neighbours");
            }
        }
        assert_ne!(stair_step(-2, -3, 0), stair_step(-2, -3, 1), "the stair turns on the next floor");
    }

    #[test]
    fn a_tower_ahead_stands_beyond_the_land_and_has_a_door_approach_in_front_of_it() {
        let at = (10.0, 20.0);
        for i in 0..16 {
            let t = Tower::ahead(at, i as f32 * std::f32::consts::TAU / 16.0, 0.0);
            let gap = (t.x as f32 - at.0).abs().max((t.z as f32 - at.1).abs()) - TOWER_REACH as f32;
            assert!(gap >= tower_distance() as f32 - 1.5, "{gap}");
        }
        let t = Tower { x: 100, z: -40 };
        let (ax, az) = t.approach();
        assert!(az > (t.z + TOWER_HALF) as f32 && (ax - 100.5).abs() < 1e-3);
    }
}
