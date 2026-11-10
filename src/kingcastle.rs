//! The king's castle: the largest castle in the game, standing on the floor of the Horizen's giant cave, and a dungeon.
//!
//! Seen from outside it is a square `2 * KING_WALL_HALF + 1` blocks across: a thick outer wall with battlements and a gate in the south
//! side (towards +z) and a tower at each corner. Inside the wall it is solid stone carved into rooms, on two storeys. Eight rooms are
//! joined in a line, each by a door of its own (`DOORS`): seven rooms that spirit guards hold (`ROOMS[..KING_THRONE_ROOM]`), and the throne
//! room last, where there is no guard and the Boots of the Sky wait on a dais. The doors are shut (`Block::CastleDoor`, unbreakable) until
//! the guards of the room before are dead (`cleared` rooms are done, so door `i` is open once `cleared > i`).
//!
//! The way through, by room: the gatehouse, a hall of pillars, the armoury, the barracks and the crypt on the ground floor; a stair up
//! (behind the crypt's door); the chapel and the library on the upper floor; then the throne room.
//!
//! Pure function of a cell's place relative to the castle's centre (`castle_cell`); `dy` counts up from the cave floor's top.

use crate::animal::AnimalKind;
use crate::block::Block;
use crate::config::*;

/// A room: the free columns (inclusive) on one storey (0 is the ground floor, 1 the upper floor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    pub name: &'static str,
    pub storey: i32,
    pub x0: i32,
    pub x1: i32,
    pub z0: i32,
    pub z1: i32,
}

impl Room {
    /// Height (above the cave floor) of the first free cell of the room: the floor of its storey is under it.
    pub fn base(&self) -> i32 {
        self.storey * (KING_SLAB_DY + 1)
    }

    /// Whether the column (dx, dz) is in the room's floor plan.
    pub fn has(&self, dx: i32, dz: i32) -> bool {
        (self.x0..=self.x1).contains(&dx) && (self.z0..=self.z1).contains(&dz)
    }
}

/// The index of the throne room, the last room: the rooms before it are held by guards.
pub const KING_THRONE_ROOM: usize = 7;

/// The rooms, in the order they are cleared and opened.
pub const ROOMS: [Room; 8] = [
    Room { name: "gatehouse", storey: 0, x0: -9, x1: 9, z0: 33, z1: 42 },
    Room { name: "hall of pillars", storey: 0, x0: -21, x1: 21, z0: 15, z1: 31 },
    Room { name: "armoury", storey: 0, x0: -41, x1: -23, z0: 15, z1: 31 },
    Room { name: "barracks", storey: 0, x0: -41, x1: -23, z0: -3, z1: 13 },
    Room { name: "crypt", storey: 0, x0: -21, x1: -5, z0: -3, z1: 13 },
    Room { name: "chapel", storey: 1, x0: 6, x1: 22, z0: -3, z1: 13 },
    Room { name: "library", storey: 1, x0: 6, x1: 40, z0: -22, z1: -5 },
    Room { name: "throne room", storey: 1, x0: -34, x1: 4, z0: -30, z1: -8 },
];

/// What holds each room of the dungeon, as (kind, how many) pairs: the creatures that come out when the player steps into the room (its turn
/// having come), and whose death opens the door out of it. The gatehouse has two spirit guards, the hall of pillars two vampire bats, the
/// armoury two Iron Quails, the barracks three Bronze Parrots and the crypt eight spirit guards; each room above has `KING_GUARDS_PER_ROOM`
/// spirit guards, and the throne room has nothing.
pub fn garrison(room: usize) -> &'static [(AnimalKind, usize)] {
    match room {
        0 => &[(AnimalKind::SpiritGuard, 2)],
        1 => &[(AnimalKind::VampireBat, 2)],
        2 => &[(AnimalKind::IronQuail, 2)],
        3 => &[(AnimalKind::BronzeParrot, 3)],
        4 => &[(AnimalKind::SpiritGuard, 8)],
        r if r < KING_THRONE_ROOM => &[(AnimalKind::SpiritGuard, KING_GUARDS_PER_ROOM)],
        _ => &[],
    }
}

/// Whether a creature of `kind` at the cell (dx, dz, dy) (from the middle of the castle, `dy` above the cave floor's top) is one that holds
/// `room`: of a kind its garrison has, and inside the room.
pub fn holds(room: usize, kind: AnimalKind, dx: i32, dz: i32, dy: i32) -> bool {
    garrison(room).iter().any(|&(k, _)| k == kind) && room_at(dx, dz, dy) == Some(room)
}

/// A door: a block of cells `KING_DOOR_HEIGHT` high from `base`, shut until its room's guards are dead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
    pub x0: i32,
    pub x1: i32,
    pub z0: i32,
    pub z1: i32,
    pub base: i32,
}

/// Door `i` leads from room `i` to room `i + 1` (door 4, below the stair, leads from the crypt to the stair up to the chapel).
pub const DOORS: [Door; 7] = [
    Door { x0: -1, x1: 1, z0: 32, z1: 32, base: 0 },
    Door { x0: -22, x1: -22, z0: 22, z1: 24, base: 0 },
    Door { x0: -33, x1: -31, z0: 14, z1: 14, base: 0 },
    Door { x0: -22, x1: -22, z0: 5, z1: 7, base: 0 },
    Door { x0: -4, x1: -4, z0: 5, z1: 7, base: 0 },
    Door { x0: 13, x1: 15, z0: -4, z1: -4, base: KING_SLAB_DY + 1 },
    Door { x0: 5, x1: 5, z0: -16, z1: -14, base: KING_SLAB_DY + 1 },
];

/// The stair from the crypt's door up to the chapel: nine steps along +x, three wide. Step `i` is the column `STAIR_X0 + i`, stone from the
/// floor up to height `i`.
pub const STAIR_X0: i32 = -3;
pub const STAIR_STEPS: i32 = 9;
pub const STAIR_Z0: i32 = 5;
pub const STAIR_Z1: i32 = 7;

/// Whether the column (dx, dz) is anywhere near the castle (inside the square its farthest tower reaches).
pub fn near(dx: i32, dz: i32, margin: i32) -> bool {
    dx.abs() <= KING_REACH + margin && dz.abs() <= KING_REACH + margin
}

/// A battlement: every other block of a parapet is raised.
fn merlon(dx: i32, dz: i32) -> bool {
    (dx + dz).rem_euclid(2) == 0
}

/// The room whose free space holds the cell (dx, dz, dy), if any: `dy` is the height of the cell above the cave floor's top (a player's
/// feet cell on the floor of a ground-floor room is 0, of an upper room `KING_SLAB_DY + 1`).
pub fn room_at(dx: i32, dz: i32, dy: i32) -> Option<usize> {
    ROOMS.iter().position(|r| r.has(dx, dz) && (r.base()..r.base() + KING_STOREY_AIR).contains(&dy))
}

/// The door that fills the cell (dx, dz, dy), if one does.
fn door_at(dx: i32, dz: i32, dy: i32) -> Option<usize> {
    DOORS.iter().position(|d| (d.x0..=d.x1).contains(&dx) && (d.z0..=d.z1).contains(&dz) && (d.base..d.base + KING_DOOR_HEIGHT).contains(&dy))
}

/// What a room's furnishing puts in the cell (dx, dz) at height `fy` above the room's floor, if anything.
fn furniture(room: usize, dx: i32, dz: i32, fy: i32) -> Option<Block> {
    let stone = Some(Block::CastleStone);
    match room {
        // The gatehouse: four pillars either side of the way in.
        0 if matches!((dx.abs(), dz), (5, 36) | (5, 40)) => stone,
        // The hall of pillars: two rows down the hall, the middle left clear.
        1 if matches!(dx.abs(), 6 | 12 | 18) && matches!(dz, 19 | 27) => stone,
        // The armoury: plain swords on stands against the west wall and plain bows on stands against the north wall (the royal armour is
        // not here; it is in the grand castle).
        2 if fy == 0 && dx == -40 && dz.rem_euclid(4) == 1 => Some(Block::SwordStand),
        2 if fy == 0 && dz == 16 && matches!(dx, -38 | -34 | -28 | -25) => Some(Block::BowStand),
        // The barracks: rows of low stone bunks along the north and south walls.
        3 if fy == 0 && dx.rem_euclid(4) < 2 && (dz == -2 || (dz == 12 && !(-34..=-30).contains(&dx))) => stone,
        // The crypt: gravestones in rows either side of the aisle.
        4 if fy == 0 && matches!(dx, -18 | -14 | -10) && matches!(dz, -1 | 1 | 9 | 11) => Some(Block::Gravestone),
        // The chapel: pews either side of an aisle, and an altar at the south end.
        5 if (fy == 0 && ((7..=11).contains(&dx) || (17..=21).contains(&dx)) && matches!(dz, 0 | 2 | 10)) || (fy == 0 && (13..=15).contains(&dx) && (12..=13).contains(&dz)) || (fy == 1 && dx == 14 && dz == 13) => stone,
        // The library: shelves three high, in rows with gaps to walk through.
        6 if fy < 3 && matches!(dz, -9 | -19) && (10..=38).contains(&dx) && !(11..=17).contains(&dx) && (dx - 10).rem_euclid(8) < 5 => stone,
        // The throne room: pillars down both sides, a throne of three steps in the north, and the dais with the boots in the middle.
        7 => {
            let (px, pz) = (dx + 15, dz + 19);
            if matches!(dz, -26 | -12) && matches!(dx, -30 | -24 | -18 | -12 | -6 | 0) {
                return stone;
            }
            if dz <= -28 && (-17..=-13).contains(&dx) && fy < -27 - dz {
                return stone;
            }
            let edge = px.abs().max(pz.abs());
            if edge <= 3 && fy == 0 || edge <= 2 && fy == 1 {
                return stone;
            }
            ((px, pz) == (0, 0) && fy == 2).then_some(Block::BootsPlinth)
        }
        _ => None,
    }
}

/// What stands in the cell (dx, dz) from the middle of the castle, `dy` blocks above the floor: `None` for air (and outside the castle).
/// `cleared` is how many rooms' guards have been killed: door `i` is open (air) once `cleared > i`.
pub fn castle_cell(dx: i32, dz: i32, dy: i32, cleared: usize) -> Option<Block> {
    if dy < 0 || !near(dx, dz, 0) {
        return None;
    }
    let stone = Some(Block::CastleStone);
    let (ax, az) = (dx.abs(), dz.abs());
    let edge = ax.max(az);
    // The towers at the four corners: a hollow shell `2 * KING_TOWER_HALF + 1` wide with a floor at the top and battlements. (No way in.)
    let (tx, tz) = (dx.signum() * KING_WALL_HALF, dz.signum() * KING_WALL_HALF);
    if dx != 0 && dz != 0 && (dx - tx).abs() <= KING_TOWER_HALF && (dz - tz).abs() <= KING_TOWER_HALF {
        let shell = (dx - tx).abs() == KING_TOWER_HALF || (dz - tz).abs() == KING_TOWER_HALF;
        return match dy {
            d if d < KING_TOWER_HEIGHT && shell => stone,
            d if d == KING_TOWER_HEIGHT => stone,
            d if d == KING_TOWER_HEIGHT + 1 && shell && merlon(dx, dz) => stone,
            _ => None,
        };
    }
    // The outer wall, two blocks thick, with the gate (an arch `2 * KING_GATE_HALF + 1` wide) in the south side.
    if edge >= KING_WALL_HALF - 1 && edge <= KING_WALL_HALF {
        let gate = dz > 0 && az >= KING_WALL_HALF - 1 && ax <= KING_GATE_HALF && dy < KING_GATE_HEIGHT;
        return match dy {
            d if d < KING_WALL_HEIGHT && !gate => stone,
            d if d == KING_WALL_HEIGHT && !gate && merlon(dx, dz) => stone,
            _ => None,
        };
    }
    if edge > KING_WALL_HALF {
        return None;
    }
    // Inside the wall: solid stone to the roof, carved into rooms.
    if dy > KING_ROOF_DY {
        return None;
    }
    if dy == KING_ROOF_DY {
        return stone;
    }
    if let Some(i) = door_at(dx, dz, dy) {
        return if cleared > i { None } else { Some(Block::CastleDoor) };
    }
    // The stair up from the crypt's door: each step is stone up to its own height; over it (through the slab) the way is open.
    if (STAIR_Z0..=STAIR_Z1).contains(&dz) && (STAIR_X0..STAIR_X0 + STAIR_STEPS).contains(&dx) {
        let step = dx - STAIR_X0;
        return if dy <= step { stone } else { None };
    }
    if let Some(i) = room_at(dx, dz, dy) {
        return furniture(i, dx, dz, dy - ROOMS[i].base());
    }
    stone
}

/// The height, above the floor, of the highest block the castle has anywhere.
pub const CASTLE_HEIGHT: i32 = KING_TOWER_HEIGHT + 2;

/// The cell, relative to the castle's middle, of the Boots of the Sky's plinth.
#[cfg(test)]
pub const BOOTS_CELL: (i32, i32, i32) = (-15, -19, KING_SLAB_DY + 1 + 2);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn it_is_the_largest_castle_in_the_game_and_two_storeys_stand_under_its_roof() {
        assert!(KING_WALL_HALF > GRAND_WALL_HALF + 10 && KING_REACH > GRAND_REACH + 10);
        assert!(KING_TOWER_HEIGHT > GRAND_HEIGHT - 4, "taller towers than the grand castle keep");
        assert!(CASTLE_HEIGHT + 2 <= KING_CAVE_HEIGHT - 20, "the cave's roof is well above it");
        assert!(KING_ROOF_DY == KING_SLAB_DY + 1 + KING_STOREY_AIR, "two storeys, then the roof");
        assert!(ROOMS.iter().any(|r| r.storey == 0) && ROOMS.iter().any(|r| r.storey == 1), "rooms on both floors");
        assert_eq!(ROOMS.len(), KING_THRONE_ROOM + 1);
        assert_eq!(DOORS.len(), KING_THRONE_ROOM, "one door between each pair of rooms");
    }

    #[test]
    fn nothing_stands_outside_the_castle_and_the_boots_are_in_the_throne_room() {
        for dy in 0..CASTLE_HEIGHT + 4 {
            assert_eq!(castle_cell(KING_REACH + 1, 0, dy, 0), None);
            assert_eq!(castle_cell(0, -KING_REACH - 1, dy, 0), None);
        }
        let (bx, bz, by) = BOOTS_CELL;
        assert_eq!(castle_cell(bx, bz, by, 0), Some(Block::BootsPlinth));
        assert_eq!(castle_cell(bx, bz, by + 1, 0), None);
        assert_eq!(room_at(bx, bz, by), Some(KING_THRONE_ROOM));
        assert_eq!(castle_cell(KING_WALL_HALF, -20, 5, 0), Some(Block::CastleStone), "the outer wall");
        assert_eq!(castle_cell(10, -20, KING_ROOF_DY, 0), Some(Block::CastleStone), "the roof");
    }

    #[test]
    fn the_armoury_has_stands_with_plain_swords_and_bows_and_no_royal_armour() {
        let armoury = ROOMS.iter().find(|r| r.name == "armoury").expect("an armoury");
        let (mut swords, mut bows) = (0, 0);
        for x in armoury.x0..=armoury.x1 {
            for z in armoury.z0..=armoury.z1 {
                for dy in armoury.base()..armoury.base() + 3 {
                    match castle_cell(x, z, dy, 0) {
                        Some(Block::SwordStand) => swords += 1,
                        Some(Block::BowStand) => bows += 1,
                        other => assert_ne!(other, Some(Block::ArmorStand), "no royal armour at {x},{z},{dy}"),
                    }
                }
            }
        }
        assert!(swords >= 1 && bows >= 1, "swords {swords}, bows {bows}");
        assert_eq!(crate::item::drops_for(Block::SwordStand, &mut crate::rng::Rng::new(1)), vec![crate::item::ItemKind::Sword]);
        assert_eq!(crate::item::drops_for(Block::BowStand, &mut crate::rng::Rng::new(1)), vec![crate::item::ItemKind::Bow]);
    }

    #[test]
    fn each_room_is_held_by_its_own_creatures_and_the_throne_room_by_none() {
        use AnimalKind::*;
        let held = |room: usize| garrison(room).to_vec();
        assert_eq!(held(0), vec![(SpiritGuard, 2)], "gatehouse");
        assert_eq!(held(1), vec![(VampireBat, 2)], "hall of pillars");
        assert_eq!(held(2), vec![(IronQuail, 2)], "armoury");
        assert_eq!(held(3), vec![(BronzeParrot, 3)], "barracks");
        assert_eq!(held(4), vec![(SpiritGuard, 8)], "crypt");
        for room in 5..KING_THRONE_ROOM {
            assert_eq!(held(room), vec![(SpiritGuard, KING_GUARDS_PER_ROOM)], "{}", ROOMS[room].name);
        }
        assert!(held(KING_THRONE_ROOM).is_empty());
        let r = ROOMS[1];
        assert!(holds(1, VampireBat, (r.x0 + r.x1) / 2, (r.z0 + r.z1) / 2, r.base() + 2));
        assert!(!holds(1, SpiritGuard, (r.x0 + r.x1) / 2, (r.z0 + r.z1) / 2, 0), "not its kind");
        assert!(!holds(2, VampireBat, (r.x0 + r.x1) / 2, (r.z0 + r.z1) / 2, 0), "not its room");
    }

    #[test]
    fn the_rooms_do_not_touch_and_each_door_stands_between_the_two_rooms_it_joins() {
        for (i, a) in ROOMS.iter().enumerate() {
            assert!(a.x0 >= -(KING_WALL_HALF - 2) && a.x1 <= KING_WALL_HALF - 2 && a.z0 >= -(KING_WALL_HALF - 2) && a.z1 <= KING_WALL_HALF - 2, "{} is inside the wall", a.name);
            for b in &ROOMS[i + 1..] {
                if a.storey == b.storey {
                    let apart = a.x1 + 2 <= b.x0 || b.x1 + 2 <= a.x0 || a.z1 + 2 <= b.z0 || b.z1 + 2 <= a.z0;
                    assert!(apart, "{} and {} need a wall between them", a.name, b.name);
                }
            }
        }
        // A door's cells, seen from just before and just after it, are in room i (or the way to it) and room i + 1 (or the stair).
        for (i, d) in DOORS.iter().enumerate() {
            let (mx, mz) = ((d.x0 + d.x1) / 2, (d.z0 + d.z1) / 2);
            let near_room = |room: usize| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(ex, ez)| room_at(mx + ex, mz + ez, d.base) == Some(room));
            assert!(near_room(i), "door {i} opens on room {i}");
            if i != 4 {
                assert!(near_room(i + 1), "door {i} opens on room {}", i + 1);
            }
        }
    }

    /// Whether a player could stand with their feet at height `y` in the column (x, z), the doors being as `cleared` rooms leave them.
    fn standable(x: i32, z: i32, y: i32, cleared: usize) -> bool {
        let free = |dy: i32| castle_cell(x, z, dy, cleared).is_none();
        free(y) && free(y + 1) && (y == 0 || matches!(castle_cell(x, z, y - 1, cleared), Some(Block::CastleStone)))
    }

    /// Every standing place reachable on foot from outside the gate, a block up or down at a time.
    fn reachable(cleared: usize) -> HashSet<(i32, i32, i32)> {
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
                    if ny >= 0 && standable(nx, nz, ny, cleared) && seen.insert((nx, nz, ny)) {
                        queue.push_back((nx, nz, ny));
                    }
                }
            }
        }
        seen
    }

    #[test]
    fn each_room_opens_only_when_the_one_before_is_cleared_and_the_boots_come_last() {
        for cleared in 0..=KING_THRONE_ROOM {
            let seen = reachable(cleared);
            let in_room = |i: usize| seen.iter().any(|&(x, z, y)| room_at(x, z, y) == Some(i));
            for i in 0..=KING_THRONE_ROOM {
                let expected = i <= cleared;
                assert_eq!(in_room(i), expected, "with {cleared} rooms cleared, the {} is {}", ROOMS[i].name, if expected { "open" } else { "shut" });
            }
            let (bx, bz, by) = BOOTS_CELL;
            let beside_boots = seen.iter().any(|&(x, z, y)| (x - bx).abs() <= 1 && (z - bz).abs() <= 1 && y == by);
            assert_eq!(beside_boots, cleared == KING_THRONE_ROOM, "the boots can be reached only once the last guarded room is cleared");
            // The stair is reached with the crypt's door open (door 4 is the fifth).
            let on_stair = seen.iter().any(|&(x, z, _)| (STAIR_X0..STAIR_X0 + STAIR_STEPS).contains(&x) && (STAIR_Z0..=STAIR_Z1).contains(&z));
            assert_eq!(on_stair, cleared >= 5, "the stair lies behind the crypt's door");
        }
    }

    #[test]
    fn nobody_can_walk_on_the_roof_or_into_the_shut_off_towers() {
        let seen = reachable(KING_THRONE_ROOM);
        assert!(!seen.iter().any(|&(_, _, y)| y > KING_ROOF_DY - 1), "nothing above the upper floor's ceiling");
        assert!(seen.iter().all(|&(x, z, y)| y == 0 || x.abs().max(z.abs()) <= KING_WALL_HALF - 2), "outside the inner rooms nobody stands above the floor: no climbing the wall or the towers");
        assert!(seen.iter().any(|&(x, z, y)| (x, y) == (0, 0) && z > KING_WALL_HALF - 2), "the gate itself can be walked through");
    }

    #[test]
    fn every_guarded_room_has_room_for_its_guards_and_the_throne_room_is_the_largest_hall() {
        for i in 0..KING_THRONE_ROOM {
            let r = ROOMS[i];
            let free = (r.x0..=r.x1).flat_map(|x| (r.z0..=r.z1).map(move |z| (x, z))).filter(|&(x, z)| standable(x, z, r.base(), KING_THRONE_ROOM)).count();
            assert!(free >= 40, "{} has {free} places to stand", r.name);
        }
        let area = |r: &Room| (r.x1 - r.x0 + 1) * (r.z1 - r.z0 + 1);
        assert!(area(&ROOMS[KING_THRONE_ROOM]) >= 700);
    }
}
