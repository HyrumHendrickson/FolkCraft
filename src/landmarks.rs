//! Landmarks: lakes, mountains and the rainbow, which the player brings into the world by exploring.
//!
//! The land of a world is a pure function of its seed (`terrain.rs`). Landmarks are the exception: they are
//! added while the game runs and saved with the world, and the terrain generator consults them.
//!
//! - A **lake** appears when the player has walked ten chunks through a river without leaving the water. It is a
//!   large wavy basin, one block deep like a river, joined to the river by a straight channel. In the middle is a
//!   large island of dry land, and on the island stands a small castle.
//! - A **mountain** appears when the player has followed a held compass north for ten chunks. It is tall, can be
//!   climbed one block at a time, and is capped with snow.
//! - A **rainbow** appears once, when the player holds a peep stone. It ends in a pile of gold; once the gold
//!   has been taken there is never another rainbow.
//!
//! This file holds the data and the rules (where the water and the slopes are, when a trek is complete). The
//! world, the save file and the game only call into it, and none of it needs a window to be tested.

use crate::block::Block;
use crate::config::*;
use crate::world::Pos;

/// Tallest a mountain can be: its summit and the cell above it must lie inside the world.
pub const MOUNTAIN_MAX_HEIGHT: i32 = WORLD_HEIGHT - 2 - GROUND_TOP;
/// Largest radius a saved landmark may have, so a damaged save cannot ask for an absurd amount of work.
pub const MAX_LANDMARK_RADIUS: i32 = 600;

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much longer or shorter than its nominal radius a round landmark is in the direction `angle`: a wavy,
/// lopsided outline (between 0.75 and 1.25), different for each `phase`.
fn lobes(phase: i32, angle: f32) -> f32 {
    let p = phase as f32 * 0.1;
    1.0 + 0.16 * (2.0 * angle + p).sin() + 0.09 * (3.0 * angle + p * 1.7).sin()
}

/// How far a landmark of `radius` can reach at most, in any direction.
fn farthest_reach(radius: i32) -> f32 {
    radius as f32 * 1.25
}

/// Distance from the point (px, pz) to the line segment from `a` to `b`.
fn distance_to_segment(px: f32, pz: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dz * dz;
    let t = if len2 > 0.0 { (((px - a.0) * dx + (pz - a.1) * dz) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((px - (a.0 + t * dx)).powi(2) + (pz - (a.1 + t * dz)).powi(2)).sqrt()
}

/// A lake: a large wavy basin of shallow water with a large island in the middle (and a castle on the island) and,
/// running out of it, the channel that joins it to the river the player walked up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lake {
    /// Centre of the basin and of the island, in world coordinates.
    pub x: i32,
    pub z: i32,
    pub radius: i32,
    /// Decides the shape of the shore.
    pub phase: i32,
    /// Where the channel leaves the river (a column of the river itself).
    pub from: (i32, i32),
}

impl Lake {
    /// A lake in the direction `dir` (a unit vector) from the player's column `at`, its near shore at least
    /// `LAKE_DISTANCE` blocks away, joined to `at` by a channel.
    pub fn ahead(at: (f32, f32), dir: (f32, f32), phase: i32) -> Lake {
        let gap = LAKE_DISTANCE as f32 + farthest_reach(LAKE_RADIUS);
        Lake {
            x: (at.0 + dir.0 * gap).round() as i32,
            z: (at.1 + dir.1 * gap).round() as i32,
            radius: LAKE_RADIUS,
            phase,
            from: (at.0.floor() as i32, at.1.floor() as i32),
        }
    }

    /// The nominal radius of the island.
    pub fn island_radius(&self) -> f32 {
        self.radius as f32 * LAKE_ISLAND_SHARE
    }

    /// Whether the island is big enough, even at its narrowest, to hold the castle with room to spare. Small
    /// lakes (there are none in play, but a saved or hand-made one can be small) have an island and no castle.
    pub fn has_castle(&self) -> bool {
        self.island_radius() * 0.75 >= CASTLE_REACH as f32 * std::f32::consts::SQRT_2 + 2.0
    }

    /// Blocks from the column (wx, wz) to the outer shore of the lake, island included: negative inside it
    /// (so it is flat all the way across, and the hills fade out round it), and also negative inside the channel.
    pub fn outer_distance(&self, wx: i32, wz: i32) -> f32 {
        let (px, pz) = (wx as f32 + 0.5, wz as f32 + 0.5);
        let (cx, cz) = (self.x as f32 + 0.5, self.z as f32 + 0.5);
        let (dx, dz) = (px - cx, pz - cz);
        let dist = (dx * dx + dz * dz).sqrt();
        let channel = distance_to_segment(px, pz, (self.from.0 as f32 + 0.5, self.from.1 as f32 + 0.5), (cx, cz)) - LAKE_CHANNEL_HALF_WIDTH;
        // Far beyond the shore (further than the hills fade over) the exact shape does not matter, and the angle is
        // not worked out. The cut-off is past the fade, so the fade itself is exact and continuous.
        let basin = if dist > farthest_reach(self.radius) + LANDMARK_BLEND as f32 { dist - farthest_reach(self.radius) } else { dist - self.radius as f32 * lobes(self.phase, dz.atan2(dx)) };
        basin.min(channel)
    }

    /// Blocks from the column (wx, wz) to the shore of the island: negative on the island.
    fn island_distance(&self, wx: i32, wz: i32) -> f32 {
        let (dx, dz) = ((wx - self.x) as f32, (wz - self.z) as f32);
        let reach = self.island_radius();
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > reach * 1.25 {
            return dist - reach * 1.25;
        }
        dist - reach * lobes(self.phase + 31, dz.atan2(dx))
    }

    /// Whether the column (wx, wz) is under the lake: inside its shore and outside its island.
    pub fn contains(&self, wx: i32, wz: i32) -> bool {
        self.outer_distance(wx, wz) <= 0.0 && self.island_distance(wx, wz) > 0.0
    }

    /// The columns the lake can touch, widened by `margin`: (min x, min z, max x, max z).
    pub fn bounds(&self, margin: i32) -> (i32, i32, i32, i32) {
        let reach = farthest_reach(self.radius).ceil() as i32 + margin;
        let (cx, cz) = (self.x, self.z);
        let (fx, fz) = self.from;
        let channel = LAKE_CHANNEL_HALF_WIDTH.ceil() as i32 + margin;
        (
            (cx - reach).min(fx - channel),
            (cz - reach).min(fz - channel),
            (cx + reach).max(fx + channel),
            (cz + reach).max(fz + channel),
        )
    }

    /// What the castle is made of in the cell `dy` blocks above the island's ground at the column (wx, wz):
    /// `Some(Block::CastleStone)` for a wall, `Some(Block::Air)` for a cell the castle keeps empty, `None` outside the
    /// castle's square or on a lake with no castle.
    pub fn castle_block(&self, wx: i32, wz: i32, dy: i32) -> Option<Block> {
        if !self.has_castle() || !(0..CASTLE_HEIGHT).contains(&dy) {
            return None;
        }
        let (dx, dz) = (wx - self.x, wz - self.z);
        if dx.abs() > CASTLE_REACH || dz.abs() > CASTLE_REACH {
            return None;
        }
        Some(if castle_is_solid(dx, dz, dy) { Block::CastleStone } else { Block::Air })
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the castle's square.
    pub fn near_castle(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.has_castle() && (wx - self.x).abs() <= CASTLE_REACH + margin && (wz - self.z).abs() <= CASTLE_REACH + margin
    }
}

/// The shape of the castle, in blocks from its centre (`dx`, `dz`) and above the ground (`dy`, 0 is the first layer of
/// air). Seen from above: an outer wall `CASTLE_WALL_HALF` blocks from the centre on each side, with a gate in the
/// south side (towards +z) three blocks wide; a solid five-block tower at each corner; and in the middle a keep, a
/// seven-block square building with a door in its south side and a roof. All of it ends in battlements: every other
/// block along the top is left out.
fn castle_is_solid(dx: i32, dz: i32, dy: i32) -> bool {
    let (ax, az) = (dx.abs(), dz.abs());
    let merlon = (dx + dz).rem_euclid(2) == 0;
    let wall = CASTLE_WALL_HALF;
    // The corner towers.
    if (ax - wall).abs() <= 2 && (az - wall).abs() <= 2 {
        return dy <= 9 || (dy == 10 && merlon);
    }
    // The outer wall, with its gate.
    if ax.max(az) == wall {
        if dz == wall && ax <= 1 && dy <= 3 {
            return false;
        }
        return dy <= 5 || (dy == 6 && merlon);
    }
    // The keep: walls, a door, a roof, battlements.
    if ax <= 3 && az <= 3 {
        let shell = ax == 3 || az == 3;
        if dy == 8 {
            return true;
        }
        if dy == 9 {
            return shell && merlon;
        }
        if dy <= 7 {
            if dz == 3 && dx == 0 && dy <= 2 {
                return false;
            }
            return shell;
        }
    }
    false
}

/// A mountain: ground raised in a smooth, lopsided heap, steep enough to be tall and gentle enough to climb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mountain {
    /// Centre (the summit), in world coordinates.
    pub x: i32,
    pub z: i32,
    pub radius: i32,
    /// Blocks the ground is raised at the summit, at most `MOUNTAIN_MAX_HEIGHT`.
    pub height: i32,
    /// Decides the shape of the foot.
    pub phase: i32,
}

impl Mountain {
    /// A mountain in the direction `dir` (a unit vector) from the player's column `at`, its foot at least
    /// `MOUNTAIN_DISTANCE` blocks away.
    pub fn ahead(at: (f32, f32), dir: (f32, f32), phase: i32) -> Mountain {
        let gap = MOUNTAIN_DISTANCE as f32 + farthest_reach(MOUNTAIN_RADIUS);
        Mountain {
            x: (at.0 + dir.0 * gap).round() as i32,
            z: (at.1 + dir.1 * gap).round() as i32,
            radius: MOUNTAIN_RADIUS,
            height: MOUNTAIN_HEIGHT.min(MOUNTAIN_MAX_HEIGHT),
            phase,
        }
    }

    /// Blocks from the column (wx, wz) to the foot of the mountain: negative on it.
    fn edge_distance(&self, wx: i32, wz: i32) -> f32 {
        let (dx, dz) = ((wx - self.x) as f32, (wz - self.z) as f32);
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > farthest_reach(self.radius) + LANDMARK_BLEND as f32 {
            return dist - farthest_reach(self.radius);
        }
        dist - self.radius as f32 * lobes(self.phase, dz.atan2(dx))
    }

    /// How many blocks the ground is raised at the column (wx, wz): zero off the mountain, the full `height` at
    /// the summit and a smooth slope between.
    pub fn rise_at(&self, wx: i32, wz: i32) -> i32 {
        let (dx, dz) = ((wx - self.x) as f32, (wz - self.z) as f32);
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= farthest_reach(self.radius) {
            return 0;
        }
        let foot = self.radius as f32 * lobes(self.phase, dz.atan2(dx));
        if dist >= foot {
            return 0;
        }
        (self.height as f32 * smoothstep(1.0 - dist / foot)).round() as i32
    }

    /// The columns the mountain can touch, widened by `margin`: (min x, min z, max x, max z).
    pub fn bounds(&self, margin: i32) -> (i32, i32, i32, i32) {
        let reach = farthest_reach(self.radius).ceil() as i32 + margin;
        (self.x - reach, self.z - reach, self.x + reach, self.z + reach)
    }
}

/// The rainbow: an arc that rises beside the player and comes down on a pile of gold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rainbow {
    /// The column where the arc starts, near where the player stood.
    pub from: (i32, i32),
    /// The cell of the pile of gold at the other end.
    pub gold: Pos,
}

/// Where the world's one rainbow stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RainbowState {
    /// No peep stone has been held yet.
    #[default]
    Unseen,
    /// The rainbow is out there, and its gold has not been taken.
    Active(Rainbow),
    /// The gold has been taken. There will never be another rainbow.
    Spent,
}

/// Everything the player has added to the world's land and sky.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Landmarks {
    pub lakes: Vec<Lake>,
    pub mountains: Vec<Mountain>,
    pub rainbow: RainbowState,
}

impl Landmarks {
    /// Whether the column (wx, wz) lies under a lake.
    pub fn is_lake(&self, wx: i32, wz: i32) -> bool {
        self.lakes.iter().any(|lake| lake.contains(wx, wz))
    }

    /// How many blocks a mountain raises the ground at the column (wx, wz) (the highest, where two overlap).
    pub fn mountain_rise(&self, wx: i32, wz: i32) -> i32 {
        self.mountains.iter().map(|m| m.rise_at(wx, wz)).max().unwrap_or(0)
    }

    /// How much of the natural hills and valleys show at the column (wx, wz): 0 on a lake or a mountain, rising
    /// smoothly to 1 `LANDMARK_BLEND` blocks away from one, so no cliff is left at the edge. 1 where there is
    /// nothing near.
    pub fn relief_scale(&self, wx: i32, wz: i32) -> f32 {
        let blend = LANDMARK_BLEND as f32;
        let lakes = self.lakes.iter().map(|l| l.outer_distance(wx, wz));
        let mountains = self.mountains.iter().map(|m| m.edge_distance(wx, wz));
        lakes.chain(mountains).map(|d| smoothstep(d / blend)).fold(1.0, f32::min)
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the square a castle stands in.
    pub fn near_castle(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.lakes.iter().any(|l| l.near_castle(wx, wz, margin))
    }

    /// Whether any lake or mountain (with the margin the hills fade over) can touch the chunk (cx, cz).
    pub fn touches_chunk(&self, cx: i32, cz: i32) -> bool {
        let (x0, z0) = (cx * CHUNK_SIZE, cz * CHUNK_SIZE);
        let (x1, z1) = (x0 + CHUNK_SIZE - 1, z0 + CHUNK_SIZE - 1);
        let overlaps = |(bx0, bz0, bx1, bz1): (i32, i32, i32, i32)| bx0 <= x1 && bx1 >= x0 && bz0 <= z1 && bz1 >= z0;
        self.lakes.iter().any(|l| overlaps(l.bounds(LANDMARK_BLEND))) || self.mountains.iter().any(|m| overlaps(m.bounds(LANDMARK_BLEND)))
    }
}

/// A trek the player has finished, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrekEvent {
    /// Ten chunks through a river: a lake lies ahead. `dir` is the way the player went (a unit vector).
    Lake { at: (f32, f32), dir: (f32, f32) },
    /// Ten chunks along the compass: a mountain lies ahead.
    Mountain { at: (f32, f32), dir: (f32, f32) },
}

/// A step longer than this many blocks in one update is a respawn, not walking: treks start over.
const TREK_JUMP_LIMIT: f32 = 12.0;

/// The unit vector the compass needle points along, in (x, z).
pub fn compass_direction() -> (f32, f32) {
    (COMPASS_NORTH_BEARING.cos(), COMPASS_NORTH_BEARING.sin())
}

/// Where the compass needle points on the screen, for a player looking along `yaw`: (right, down), so a needle
/// pointing straight up the screen is (0, -1). North dead ahead gives (0, -1); north to the player's right
/// gives a vector pointing right.
pub fn compass_needle(yaw: f32) -> (f32, f32) {
    let relative = COMPASS_NORTH_BEARING - yaw;
    (relative.sin(), -relative.cos())
}

/// Progress of the two treks. Not saved: a trek starts over when the world is loaded.
#[derive(Clone, Debug, Default)]
pub struct Treks {
    /// Where the player was at the last update.
    last: Option<(f32, f32)>,
    /// Where the player stepped into the water, while they have stayed in it since.
    river_start: Option<(f32, f32)>,
    /// Set once a river trek has made its lake, until the player next stands on dry land.
    river_done: bool,
    /// Blocks walked northwards with the compass in hand.
    compass_blocks: f32,
    /// Whether the compass was in hand at the last update.
    compass_held: bool,
}

impl Treks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Blocks walked north so far with the compass in hand.
    pub fn compass_progress(&self) -> f32 {
        self.compass_blocks
    }

    /// Advance both treks to the player's new column `at` (x, z). `in_water` is whether they stand in water,
    /// `compass` whether a compass is in their hand. Returns the trek they have just finished, if any. Call it
    /// every frame: a step of more than `TREK_JUMP_LIMIT` blocks (a respawn) starts both treks over.
    ///
    /// - River: the first update in the water marks the start. Leaving the water starts over. Once the player is
    ///   `LAKE_TREK_CHUNKS` chunks from the start (counting along either axis), that is a lake, and no further
    ///   lake comes until they have stood on dry land.
    /// - Compass: each step adds the distance walked northwards (a step south takes it away again, down to zero).
    ///   Putting the compass away starts over. `COMPASS_TREK_CHUNKS` chunks north is a mountain.
    pub fn step(&mut self, at: (f32, f32), in_water: bool, compass: bool) -> Option<TrekEvent> {
        let previous = self.last.replace(at);
        if previous.map_or(false, |p| ((at.0 - p.0).powi(2) + (at.1 - p.1).powi(2)).sqrt() > TREK_JUMP_LIMIT) {
            *self = Treks { last: Some(at), ..Treks::default() };
        }
        let mut event = None;

        if in_water {
            if !self.river_done {
                let start = *self.river_start.get_or_insert(at);
                let reach = (LAKE_TREK_CHUNKS * CHUNK_SIZE) as f32;
                if (at.0 - start.0).abs() >= reach || (at.1 - start.1).abs() >= reach {
                    let (dx, dz) = (at.0 - start.0, at.1 - start.1);
                    let len = (dx * dx + dz * dz).sqrt();
                    self.river_start = None;
                    self.river_done = true;
                    event = Some(TrekEvent::Lake { at, dir: (dx / len, dz / len) });
                }
            }
        } else {
            self.river_start = None;
            self.river_done = false;
        }

        if compass {
            if let (true, Some(last)) = (self.compass_held, previous) {
                let north = compass_direction();
                self.compass_blocks = (self.compass_blocks + (at.0 - last.0) * north.0 + (at.1 - last.1) * north.1).max(0.0);
            }
            self.compass_held = true;
            if self.compass_blocks >= (COMPASS_TREK_CHUNKS * CHUNK_SIZE) as f32 {
                self.compass_blocks = 0.0;
                event = event.or(Some(TrekEvent::Mountain { at, dir: compass_direction() }));
            }
        } else {
            self.compass_held = false;
            self.compass_blocks = 0.0;
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lake() -> Lake {
        Lake::ahead((10.0, 10.0), (1.0, 0.0), 7)
    }

    fn mountain(phase: i32) -> Mountain {
        Mountain { x: 0, z: 0, radius: MOUNTAIN_RADIUS, height: MOUNTAIN_HEIGHT, phase }
    }

    /// A point in the water of a lake, between the island and the shore: (x, z) at `share` of the radius east of the centre.
    fn ring_point(l: &Lake, share: f32) -> (i32, i32) {
        (l.x + (l.radius as f32 * share) as i32, l.z)
    }

    #[test]
    fn a_lake_lies_beyond_the_generated_land_and_is_joined_to_the_river() {
        let l = lake();
        let (rx, rz) = ring_point(&l, 0.55);
        assert!(l.contains(rx, rz), "water between the island and the shore");
        assert!(l.contains(10, 10), "the channel starts in the river the player stood in");
        assert!(l.contains((10 + l.x - l.radius) / 2, 10), "and runs straight to the lake");
        assert!(!l.contains(l.x, l.z + LAKE_RADIUS * 2) && !l.contains(l.x, l.z - LAKE_RADIUS * 2));
        // The near shore, off the channel, is at least LAKE_DISTANCE blocks from the player.
        let reach = farthest_reach(LAKE_RADIUS) as i32;
        let near = (-reach..=reach).step_by(2).flat_map(|dz| (-reach..=reach).step_by(2).map(move |dx| (l.x + dx, l.z + dz))).filter(|&(x, z)| l.contains(x, z) && (z - 10).abs() > 4);
        for (x, z) in near {
            assert!((((x - 10) as f32).hypot((z - 10) as f32)) >= LAKE_DISTANCE as f32 - 1.0, "water at {x},{z} is too close");
        }
    }

    #[test]
    fn a_lake_is_large_with_a_wavy_shore_and_a_large_island_in_the_middle() {
        let l = Lake { x: 0, z: 0, radius: LAKE_RADIUS, phase: 11, from: (0, 0) };
        let reach = farthest_reach(LAKE_RADIUS) as i32 + 2;
        let (mut water, mut island) = (0, 0);
        for z in -reach..=reach {
            for x in -reach..=reach {
                if l.contains(x, z) {
                    water += 1;
                } else if l.outer_distance(x, z) <= 0.0 {
                    island += 1;
                }
            }
        }
        let circle = |r: f32| std::f32::consts::PI * r * r;
        assert!(water as f32 > circle(LAKE_RADIUS as f32) * 0.6, "water {water}");
        assert!(water > 30_000, "a large lake, bigger than 30,000 columns of water: {water}");
        assert!(island as f32 > circle(l.island_radius()) * 0.6 && island > 2500, "a large island: {island} columns");
        assert!(l.island_radius() >= 35.0);
        assert!(!l.contains(0, 0), "the centre is the island");
        // The shore is not a perfect circle.
        let reach_at = |angle: f32| (0..(LAKE_RADIUS * 2)).rev().find(|&r| l.contains((r as f32 * angle.cos()) as i32, (r as f32 * angle.sin()) as i32)).unwrap();
        let radii: Vec<i32> = (0..16).map(|i| reach_at(i as f32 * 0.4)).collect();
        assert!(radii.iter().max().unwrap() - radii.iter().min().unwrap() >= 12, "not a perfect circle: {radii:?}");
        // Wading from the shore to the island, the water is unbroken: there is water all the way out to the island.
        let to_island = (0..60).map(|i| (i as f32 / 60.0 * LAKE_RADIUS as f32 * 0.7) as i32).filter(|&r| l.contains(r, 0)).count();
        assert!(to_island > 10);
    }

    #[test]
    fn the_castle_stands_in_the_middle_of_the_island_with_a_gate_and_a_door() {
        let l = Lake { x: 100, z: -50, radius: LAKE_RADIUS, phase: 5, from: (0, 0) };
        assert!(l.has_castle());
        // Every column of the castle's square is dry island, with room to spare.
        for dz in -CASTLE_REACH - 3..=CASTLE_REACH + 3 {
            for dx in -CASTLE_REACH - 3..=CASTLE_REACH + 3 {
                assert!(!l.contains(l.x + dx, l.z + dz), "water at the castle's foot: {dx},{dz}");
            }
        }
        let at = |dx: i32, dz: i32, dy: i32| l.castle_block(l.x + dx, l.z + dz, dy);
        let wall = Some(Block::CastleStone);
        assert_eq!(at(8, 0, 0), wall, "the east wall");
        assert_eq!(at(-8, 3, 5), wall);
        assert_eq!(at(0, -8, 0), wall, "the north wall");
        assert_eq!(at(0, 8, 0), Some(Block::Air), "the gate in the south wall, three wide");
        assert_eq!(at(1, 8, 3), Some(Block::Air));
        assert_eq!(at(-1, 8, 0), Some(Block::Air));
        assert_eq!(at(2, 8, 0), wall);
        assert_eq!(at(0, 8, 4), wall, "and a lintel over it");
        assert_eq!(at(8, 8, 9), wall, "the corner tower is higher than the wall");
        assert_eq!(at(7, 0, 8), Some(Block::Air), "the wall is lower than the tower");
        assert_eq!(at(0, 3, 0), Some(Block::Air), "the door of the keep, on its south side");
        assert_eq!(at(0, 3, 3), wall);
        assert_eq!(at(3, 0, 0), wall, "the keep's wall");
        assert_eq!(at(0, 0, 0), Some(Block::Air), "empty inside the keep");
        assert_eq!(at(0, 0, 8), wall, "with a roof");
        assert_eq!(at(0, 5, 0), Some(Block::Air), "the courtyard is open");
        assert_eq!(at(0, 0, 11), None, "nothing above the castle");
        assert_eq!(l.castle_block(l.x + CASTLE_REACH + 1, l.z, 0), None, "nothing outside its square");
        // Walls and floor leave a way in: from the gate to the keep door the cells are all open, a man's height.
        for dz in 3..=8 {
            for dy in 0..2 {
                assert_eq!(at(0, dz, dy), Some(Block::Air), "the way in at {dz},{dy}");
            }
        }
    }

    #[test]
    fn a_small_lake_has_an_island_but_no_castle() {
        let l = Lake { x: 0, z: 0, radius: 20, phase: 0, from: (0, 0) };
        assert!(!l.has_castle() && l.castle_block(0, 0, 0).is_none());
        assert!(!l.near_castle(0, 0, 100));
    }

    #[test]
    fn hills_fade_out_beside_a_lake_or_a_mountain() {
        let mut marks = Landmarks::default();
        assert_eq!(marks.relief_scale(5, 5), 1.0);
        marks.lakes.push(Lake { x: 0, z: 0, radius: 20, phase: 0, from: (0, 0) });
        assert_eq!(marks.relief_scale(0, 0), 0.0, "no hill on the water or the island");
        assert_eq!(marks.relief_scale(500, 500), 1.0, "none near: untouched");
        let ramp: Vec<f32> = (0..60).map(|x| marks.relief_scale(x, 0)).collect();
        assert!(ramp.windows(2).all(|w| w[1] >= w[0] - 1e-6), "the hills come back gradually");
        assert!(ramp[59] > 0.99);
        marks.mountains.push(Mountain { x: 5000, z: 0, ..mountain(3) });
        assert_eq!(marks.relief_scale(100, 100), 1.0);
        assert_eq!(marks.relief_scale(5000, 5), 0.0, "no hill on a mountain");
        assert_eq!(marks.relief_scale(5000 - 700, 0), 1.0, "and none at its foot");
    }

    #[test]
    fn a_mountain_is_ten_times_taller_than_it_was_and_can_be_climbed_one_block_at_a_time() {
        assert_eq!(MOUNTAIN_HEIGHT, 10 * 17, "ten times the old height");
        assert!(MOUNTAIN_HEIGHT > 3 * HILL_MAX_HEIGHT, "much taller than any hill");
        for phase in [0, 9, 17, 25, 33, 41, 49, 57, 62] {
            let m = mountain(phase);
            assert_eq!(m.rise_at(0, 0), MOUNTAIN_HEIGHT, "the summit");
            assert_eq!(m.rise_at(MOUNTAIN_RADIUS * 2, 0), 0);
            let reach = farthest_reach(MOUNTAIN_RADIUS) as i32 + 2;
            let mut highest = 0;
            for z in -reach..=reach {
                for x in -reach..=reach {
                    let here = m.rise_at(x, z);
                    highest = highest.max(here);
                    // A jump clears about one block, so no step between neighbouring columns may be bigger.
                    assert!((here - m.rise_at(x + 1, z)).abs() <= 1, "phase {phase}: step at {x},{z}");
                    assert!((here - m.rise_at(x, z + 1)).abs() <= 1, "phase {phase}: step at {x},{z}");
                }
            }
            assert_eq!(highest, MOUNTAIN_HEIGHT);
        }
    }

    #[test]
    fn a_mountain_fits_in_the_world_with_room_for_snow() {
        assert!(MOUNTAIN_HEIGHT <= MOUNTAIN_MAX_HEIGHT);
        assert!(GROUND_TOP + MOUNTAIN_HEIGHT + 1 < WORLD_HEIGHT, "the summit and the cell above it are inside the world");
        assert!(SNOW_LINE_RISE < MOUNTAIN_HEIGHT, "the top is snowy");
        assert!(SNOW_LINE_RISE > TREE_LINE_RISE && TREE_LINE_RISE >= HILL_MAX_HEIGHT, "trees grow on every hill, but not up in the snow");
        assert!(GROUND_TOP + TREE_LINE_RISE + 1 + 10 < WORLD_HEIGHT, "a pine at the tree line still fits");
    }

    #[test]
    fn a_new_mountain_lies_north_of_the_player_beyond_the_generated_land() {
        let m = Mountain::ahead((100.0, 500.0), compass_direction(), 4);
        assert!((m.x - 100).abs() <= 1 && m.z < 500 - MOUNTAIN_DISTANCE, "north is smaller z: {m:?}");
        assert_eq!(m.height, MOUNTAIN_HEIGHT);
        let nearest = (-reach_of(&m)..=reach_of(&m)).flat_map(|dz| (-reach_of(&m)..=reach_of(&m)).map(move |dx| (m.x + dx, m.z + dz))).filter(|&(x, z)| m.rise_at(x, z) > 0).map(|(x, z)| ((x - 100) as f32).hypot((z - 500) as f32)).fold(f32::MAX, f32::min);
        assert!(nearest >= MOUNTAIN_DISTANCE as f32 - 1.0, "the foot is {nearest} away");
    }

    fn reach_of(m: &Mountain) -> i32 {
        farthest_reach(m.radius) as i32 + 1
    }

    #[test]
    fn a_chunk_is_touched_only_when_a_landmark_can_reach_it() {
        let mut marks = Landmarks::default();
        assert!(!marks.touches_chunk(0, 0));
        marks.mountains.push(Mountain { x: 1000, z: 1000, radius: 40, height: 10, phase: 0 });
        assert!(marks.touches_chunk(50, 50) && !marks.touches_chunk(0, 0) && !marks.touches_chunk(50, 0));
        marks.lakes.push(Lake { x: 0, z: 0, radius: 20, phase: 0, from: (0, 0) });
        assert!(marks.touches_chunk(0, 0) && marks.touches_chunk(-1, -1));
    }

    /// Walk from `from` to `to` in one-block steps, returning every trek finished on the way.
    fn walk(treks: &mut Treks, from: (f32, f32), to: (f32, f32), in_water: bool, compass: bool) -> Vec<TrekEvent> {
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let steps = dx.abs().max(dz.abs()).ceil().max(1.0) as i32;
        (0..=steps)
            .filter_map(|i| {
                let t = i as f32 / steps as f32;
                treks.step((from.0 + dx * t, from.1 + dz * t), in_water, compass)
            })
            .collect()
    }

    #[test]
    fn ten_chunks_in_a_river_make_a_lake_once() {
        let reach = (LAKE_TREK_CHUNKS * CHUNK_SIZE) as f32;
        let mut treks = Treks::new();
        assert!(walk(&mut treks, (10.0, 10.0), (10.0 + reach - 1.0, 10.0), true, false).is_empty(), "a block short");
        let events = walk(&mut treks, (10.0 + reach - 1.0, 10.0), (10.0 + reach + 40.0, 12.0), true, false);
        assert_eq!(events.len(), 1, "one lake, however much further they wade: {events:?}");
        match events[0] {
            TrekEvent::Lake { at, dir } => {
                assert!((at.0 - (10.0 + reach)).abs() <= 1.0);
                assert!(dir.0 > 0.99 && (dir.0.hypot(dir.1) - 1.0).abs() < 1e-4, "{dir:?}");
            }
            other => panic!("expected a lake, got {other:?}"),
        }
        assert!(walk(&mut treks, (10.0 + reach + 40.0, 12.0), (10.0 + 3.0 * reach, 12.0), true, false).is_empty(), "still no second lake");
        // After standing on dry land a new trek can begin.
        assert!(walk(&mut treks, (0.0, 20.0), (0.0, 21.0), false, false).is_empty());
        let events = walk(&mut treks, (0.0, 21.0), (0.0, 21.0 + reach + 5.0), true, false);
        assert!(matches!(events[..], [TrekEvent::Lake { .. }]), "{events:?}");
    }

    #[test]
    fn leaving_the_water_starts_the_river_trek_over() {
        let reach = (LAKE_TREK_CHUNKS * CHUNK_SIZE) as f32;
        let mut treks = Treks::new();
        assert!(walk(&mut treks, (0.0, 0.0), (reach - 5.0, 0.0), true, false).is_empty());
        treks.step((reach - 4.0, 0.0), false, false); // one step on the bank
        assert!(walk(&mut treks, (reach - 4.0, 0.0), (reach + 50.0, 0.0), true, false).is_empty(), "the trek began again at the bank");
        assert!(matches!(walk(&mut treks, (reach + 50.0, 0.0), (reach + 50.0, reach), true, false)[..], [TrekEvent::Lake { .. }]), "ten chunks along z counts too");
    }

    #[test]
    fn walking_back_and_forth_in_a_river_goes_nowhere() {
        let mut treks = Treks::new();
        for _ in 0..200 {
            assert!(walk(&mut treks, (0.0, 0.0), (30.0, 0.0), true, false).is_empty());
            assert!(walk(&mut treks, (30.0, 0.0), (0.0, 0.0), true, false).is_empty());
        }
    }

    #[test]
    fn ten_chunks_north_with_a_compass_make_a_mountain() {
        let reach = (COMPASS_TREK_CHUNKS * CHUNK_SIZE) as f32;
        let mut treks = Treks::new();
        assert!(walk(&mut treks, (5.0, 0.0), (5.0, -reach + 2.0), false, true).is_empty(), "not yet");
        let events = walk(&mut treks, (5.0, -reach + 2.0), (5.0, -reach - 30.0), false, true);
        assert_eq!(events.len(), 1, "{events:?}");
        match events[0] {
            TrekEvent::Mountain { at, dir } => {
                assert!(at.1 <= -reach + 1.0 && at.1 >= -reach - 3.0, "{at:?}");
                assert!(dir.0.abs() < 1e-4 && dir.1 < -0.99, "north is -z: {dir:?}");
            }
            other => panic!("expected a mountain, got {other:?}"),
        }
        assert!(treks.compass_progress() < 40.0, "counting towards the next one");
    }

    #[test]
    fn only_progress_to_the_north_counts_and_the_compass_must_stay_in_hand() {
        let mut treks = Treks::new();
        walk(&mut treks, (0.0, 0.0), (8.0, 0.0), false, true); // east: nothing
        assert_eq!(treks.compass_progress(), 0.0);
        walk(&mut treks, (8.0, 0.0), (8.0, -6.0), false, true);
        assert!((treks.compass_progress() - 6.0).abs() < 1e-3);
        walk(&mut treks, (8.0, -6.0), (8.0, 0.0), false, true); // back south
        assert_eq!(treks.compass_progress(), 0.0, "never below nothing");
        walk(&mut treks, (8.0, 0.0), (8.0, -6.0), false, true);
        treks.step((8.0, -7.0), false, false); // compass put away
        assert_eq!(treks.compass_progress(), 0.0);
        treks.step((8.0, -8.0), false, true); // and taken out again: a new start
        assert_eq!(treks.compass_progress(), 0.0);
        treks.step((8.0, -9.0), false, true);
        assert!((treks.compass_progress() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn a_respawn_does_not_count_as_walking_and_both_treks_are_independent() {
        let mut treks = Treks::new();
        walk(&mut treks, (0.0, 0.0), (0.0, -5.0), false, true);
        assert!(treks.compass_progress() > 4.0);
        treks.step((0.0, 500.0), false, true); // moved a long way in one update
        assert_eq!(treks.compass_progress(), 0.0);
        // A river walk and a compass walk can finish on the same step; the lake is reported.
        let reach = (LAKE_TREK_CHUNKS * CHUNK_SIZE) as f32;
        let mut both = Treks::new();
        let events = walk(&mut both, (0.0, 0.0), (0.0, -reach), true, true);
        assert!(matches!(events[..], [TrekEvent::Lake { .. }]), "{events:?}");
    }

    #[test]
    fn the_compass_needle_points_north_on_the_screen() {
        // Facing north (yaw of the north bearing): needle straight up.
        let (x, y) = compass_needle(COMPASS_NORTH_BEARING);
        assert!(x.abs() < 1e-5 && (y + 1.0).abs() < 1e-5);
        // Facing east (yaw 0, forward is +x): north (-z) is on the left, so the needle points left.
        let (x, y) = compass_needle(0.0);
        assert!(x < -0.99 && y.abs() < 1e-5, "({x}, {y})");
        // Facing south: north is behind, so the needle points down.
        let (_, y) = compass_needle(std::f32::consts::FRAC_PI_2);
        assert!(y > 0.99);
    }

    #[test]
    fn the_rainbow_starts_unseen() {
        assert_eq!(Landmarks::default().rainbow, RainbowState::Unseen);
    }
}
