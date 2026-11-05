//! Landmarks: lakes, mountains and the rainbow, which the player brings into the world by exploring.
//!
//! The land of a world is a pure function of its seed (`terrain.rs`). Landmarks are the exception: they are
//! added while the game runs and saved with the world, and the terrain generator consults them.
//!
//! - A **lake** appears when the player has walked ten chunks through a river without leaving the water. It is a
//!   large wavy basin, one block deep like a river, joined to the river by a straight channel. In the middle is a
//!   large island: a hill with a flat top, and on the flat top stands a small castle.
//! - A **mountain** appears when the player has followed a held compass north for ten chunks. It is tall, can be
//!   climbed one block at a time, and is capped with snow. Rings of cliff (ledges) break its slope, each with one
//!   gap, the gaps on alternate sides, so it has to be climbed in a zigzag.
//! - A **rainbow** appears once, by day, when the player holds a peep stone. It ends in a pile of gold; once the gold
//!   has been taken there is never another rainbow. The peep stone shows a ghostly trail to the end of the rainbow
//!   for ever after.
//! - A **grand castle** appears once, at night, when the player holds a peep stone: a large castle of five floors,
//!   beyond the edge of the land, which the peep stone's ghostly trail leads to.
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

    /// How many blocks the island hill raises the ground at the column (wx, wz): nothing off the island or on a
    /// lake too small for a castle, the full `ISLAND_HILL_HEIGHT` on the flat top (a disc of `ISLAND_PLATEAU_RADIUS`
    /// round the centre, which the castle stands on) and a slope between, steady enough to be climbed one block
    /// at a time.
    pub fn hill_rise(&self, wx: i32, wz: i32) -> i32 {
        if !self.has_castle() {
            return 0;
        }
        let (dx, dz) = ((wx - self.x) as f32, (wz - self.z) as f32);
        let reach = self.island_radius();
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > reach * 1.25 {
            return 0;
        }
        let foot = reach * lobes(self.phase + 31, dz.atan2(dx));
        if dist >= foot {
            return 0;
        }
        let run = (foot - ISLAND_PLATEAU_RADIUS).max(1.0);
        let t = ((foot - dist) / run).clamp(0.0, 1.0);
        // Mostly an even slope, with soft shoulders at the foot and the top.
        let shape = 0.6 * t + 0.4 * smoothstep(t);
        (ISLAND_HILL_HEIGHT as f32 * shape).round() as i32
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

    /// How many ledges (rings of cliff) break the mountain's slope.
    pub fn ledge_count(&self) -> i32 {
        (self.height / MOUNTAIN_LEDGE_SPACING_HEIGHT).clamp(0, MOUNTAIN_MAX_LEDGES)
    }

    /// The angle (radians) at which ledge `k` has its gap: to one side of the mountain's axis for every other ledge
    /// and to the other side for the rest, so that the way up zigzags.
    fn gap_angle(&self, k: i32) -> f32 {
        let axis = self.phase as f32 * 0.1;
        axis + if k % 2 == 0 { MOUNTAIN_GAP_OFFSET_ANGLE } else { -MOUNTAIN_GAP_OFFSET_ANGLE }
    }

    /// How many blocks the ground is raised at the column (wx, wz): zero off the mountain, the full `height` at
    /// the summit and a slope between, broken by ledges. A ledge is a ring of cliff `MOUNTAIN_LEDGE_HEIGHT` blocks
    /// high at a fixed share of the way up, with one gap: there the same rise is spread out into a ramp. The gap
    /// of each ledge lies on the opposite side from the one before it.
    pub fn rise_at(&self, wx: i32, wz: i32) -> i32 {
        let (dx, dz) = ((wx - self.x) as f32, (wz - self.z) as f32);
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= farthest_reach(self.radius) {
            return 0;
        }
        let angle = dz.atan2(dx);
        let foot = self.radius as f32 * lobes(self.phase, angle);
        if dist >= foot {
            return 0;
        }
        let ledges = self.ledge_count();
        let mut rise = (self.height - ledges * MOUNTAIN_LEDGE_HEIGHT) as f32 * smoothstep(1.0 - dist / foot);
        let ramp = (foot / (ledges + 1) as f32 * 0.45).clamp(2.0, MOUNTAIN_RAMP_WIDTH);
        for k in 0..ledges {
            let inward = foot * (k + 1) as f32 / (ledges + 1) as f32 - dist;
            if inward <= 0.0 {
                continue;
            }
            let off_axis = (angle - self.gap_angle(k) + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            let in_gap = 1.0 - smoothstep((off_axis.abs() - MOUNTAIN_GAP_HALF_ANGLE) / MOUNTAIN_GAP_BLEND_ANGLE);
            let width = MOUNTAIN_CLIFF_WIDTH + (ramp - MOUNTAIN_CLIFF_WIDTH) * in_gap;
            rise += MOUNTAIN_LEDGE_HEIGHT as f32 * smoothstep(inward / width);
        }
        rise.round() as i32
    }

    /// The columns the mountain can touch, widened by `margin`: (min x, min z, max x, max z).
    pub fn bounds(&self, margin: i32) -> (i32, i32, i32, i32) {
        let reach = farthest_reach(self.radius).ceil() as i32 + margin;
        (self.x - reach, self.z - reach, self.x + reach, self.z + reach)
    }
}

/// The grand castle: a large castle of grey stone beyond the edge of the land. Centre in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrandCastle {
    pub x: i32,
    pub z: i32,
}

impl GrandCastle {
    /// The columns the castle can touch, widened by `margin`: (min x, min z, max x, max z).
    pub fn bounds(&self, margin: i32) -> (i32, i32, i32, i32) {
        let reach = GRAND_REACH + margin;
        (self.x - reach, self.z - reach, self.x + reach, self.z + reach)
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the castle's square.
    pub fn near(&self, wx: i32, wz: i32, margin: i32) -> bool {
        (wx - self.x).abs() <= GRAND_REACH + margin && (wz - self.z).abs() <= GRAND_REACH + margin
    }

    /// What the castle is made of in the cell `dy` blocks above the ground at the column (wx, wz): see
    /// `Lake::castle_block`.
    pub fn castle_block(&self, wx: i32, wz: i32, dy: i32) -> Option<Block> {
        if !(0..GRAND_HEIGHT).contains(&dy) || !self.near(wx, wz, 0) {
            return None;
        }
        Some(if grand_castle_is_solid(wx - self.x, wz - self.z, dy) { Block::CastleStone } else { Block::Air })
    }

    /// The spot just outside the gate, on the south side (towards +z), and the door of the keep: where the ghostly
    /// trail goes, as (x, z) in world coordinates, at the middle of a column.
    pub fn gate_approach(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, (self.z + GRAND_WALL_HALF + 10) as f32 + 0.5)
    }

    pub fn keep_door(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, (self.z + GRAND_KEEP_HALF + 2) as f32 + 0.5)
    }

    /// A castle whose nearest edge is `GRAND_DISTANCE` blocks from `at` (counting along either axis, as the chunks
    /// do), in the direction `angle`: beyond the land that is already made.
    pub fn ahead(at: (f32, f32), angle: f32) -> GrandCastle {
        let (c, s) = (angle.cos(), angle.sin());
        let gap = (GRAND_DISTANCE + GRAND_REACH) as f32 / c.abs().max(s.abs());
        GrandCastle { x: (at.0 + c * gap).round() as i32, z: (at.1 + s * gap).round() as i32 }
    }
}

/// The cell of step `i` (1 to 5) of the stair that climbs from floor `storey` to the next one inside the keep. The
/// stairs follow one another along the east wall (going north) and then along the north wall (going west).
fn keep_stair_cell(storey: i32, i: i32) -> (i32, i32) {
    match storey {
        0 => (9, 9 - i),
        1 => (9, 4 - i),
        2 => (9, -1 - i),
        3 => (9 - i, -9),
        _ => (4 - i, -9),
    }
}

/// The shape of the grand castle, in blocks from its centre (`dx`, `dz`) and above the ground (`dy`, 0 is the first
/// layer of air). Seen from above: an outer wall `GRAND_WALL_HALF` from the centre with a gate in the south side
/// (towards +z) five wide; a solid tower at each corner, two flanking the gate and one in the middle of each of the
/// other three sides; and in the middle a keep of `GRAND_KEEP_HALF`, five floors of `GRAND_STOREY` blocks each, with a
/// door in its south side, windows, a stair from each floor up to the next, and a roof with battlements to walk on.
/// Everything ends in battlements: every other block along the top is left out.
fn grand_castle_is_solid(dx: i32, dz: i32, dy: i32) -> bool {
    let (ax, az) = (dx.abs(), dz.abs());
    let merlon = (dx + dz).rem_euclid(2) == 0;
    let (wall, keep) = (GRAND_WALL_HALF, GRAND_KEEP_HALF);
    // Up to and including `top`, with a battlement above it.
    let crowned = |top: i32| dy <= top || (dy == top + 1 && merlon);

    // The corner towers.
    if (ax - wall).abs() <= 4 && (az - wall).abs() <= 4 {
        return crowned(25);
    }
    // The towers beside the gate, and in the middle of the north, east and west walls.
    let gate_tower = (ax - 7).abs() <= 2 && dz > 0 && (az - wall).abs() <= 3;
    let side_tower = (az - wall).abs() <= 3 && ax <= 3 && dz < 0 || (ax - wall).abs() <= 3 && az <= 3;
    if gate_tower || side_tower {
        return crowned(17);
    }
    // The outer wall, two blocks thick, with the gate.
    if ax.max(az) >= wall - 1 && ax.max(az) <= wall {
        if dz > 0 && az >= wall - 1 && ax <= 2 && dy <= 5 {
            return false;
        }
        return crowned(11);
    }
    // The keep.
    if ax <= keep && az <= keep {
        if ax >= keep - 1 || az >= keep - 1 {
            // Its walls: a door, and windows on every floor.
            if dz > 0 && az >= keep - 1 && ax <= 1 && dy <= 3 {
                return false;
            }
            let along = if az >= keep - 1 { dx } else { dz };
            let storey_row = dy.rem_euclid(GRAND_STOREY);
            if dy < GRAND_STOREY * GRAND_STOREYS - 1 && (1..=2).contains(&storey_row) && along.rem_euclid(4) == 0 && along.abs() <= 8 && (ax < keep - 1 || az < keep - 1) {
                return false;
            }
            return crowned(GRAND_STOREY * GRAND_STOREYS - 1);
        }
        // Inside: the stairs...
        for storey in 0..GRAND_STOREYS {
            for i in 1..=GRAND_STOREY {
                if (dx, dz) == keep_stair_cell(storey, i) && dy >= storey * GRAND_STOREY && dy < storey * GRAND_STOREY + i {
                    return true;
                }
            }
        }
        // ...and the floors (and the roof), each cut open above the first four steps of the stair below it.
        if dy >= GRAND_STOREY - 1 && (dy + 1) % GRAND_STOREY == 0 {
            let floor = (dy + 1) / GRAND_STOREY;
            let hole = (1..GRAND_STOREY).any(|i| (dx, dz) == keep_stair_cell(floor - 1, i));
            return !hole;
        }
    }
    false
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
    /// Where the rainbow's gold lay, once it has been taken (so the peep stone still leads there).
    pub spent_gold: Option<Pos>,
    /// The world's one grand castle, once a peep stone has been held at night.
    pub grand_castle: Option<GrandCastle>,
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

    /// How many blocks the island hills of the lakes raise the ground at the column (wx, wz).
    pub fn island_rise(&self, wx: i32, wz: i32) -> i32 {
        self.lakes.iter().map(|l| l.hill_rise(wx, wz)).max().unwrap_or(0)
    }

    /// Whether a grand castle at `castle` would stand clear of every lake and mountain (and of the ground they
    /// blend into).
    pub fn has_room_for(&self, castle: &GrandCastle) -> bool {
        let margin = LANDMARK_BLEND + 4;
        let (cx0, cz0, cx1, cz1) = castle.bounds(margin);
        let overlaps = |(x0, z0, x1, z1): (i32, i32, i32, i32)| x0 <= cx1 && x1 >= cx0 && z0 <= cz1 && z1 >= cz0;
        !self.lakes.iter().any(|l| overlaps(l.bounds(margin))) && !self.mountains.iter().any(|m| overlaps(m.bounds(margin)))
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the grand castle's square.
    pub fn near_grand_castle(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.grand_castle.map_or(false, |c| c.near(wx, wz, margin))
    }

    /// The end of the rainbow: the pile of gold, or the place where it lay once it has been taken.
    pub fn rainbow_end(&self) -> Option<Pos> {
        match self.rainbow {
            RainbowState::Active(r) => Some(r.gold),
            RainbowState::Spent => self.spent_gold,
            RainbowState::Unseen => None,
        }
    }

    /// How much of the natural hills and valleys show at the column (wx, wz): 0 on a lake or a mountain, rising
    /// smoothly to 1 `LANDMARK_BLEND` blocks away from one, so no cliff is left at the edge. 1 where there is
    /// nothing near.
    pub fn relief_scale(&self, wx: i32, wz: i32) -> f32 {
        let blend = LANDMARK_BLEND as f32;
        let lakes = self.lakes.iter().map(|l| l.outer_distance(wx, wz));
        let mountains = self.mountains.iter().map(|m| m.edge_distance(wx, wz));
        // The grand castle stands on level ground: the hills die away round it.
        let castle = self.grand_castle.map(|c| (wx - c.x).abs().max((wz - c.z).abs()) as f32 - (GRAND_REACH + 2) as f32);
        lakes.chain(mountains).chain(castle).map(|d| smoothstep(d / blend)).fold(1.0, f32::min)
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
        self.lakes.iter().any(|l| overlaps(l.bounds(LANDMARK_BLEND)))
            || self.mountains.iter().any(|m| overlaps(m.bounds(LANDMARK_BLEND)))
            || self.grand_castle.map_or(false, |c| overlaps(c.bounds(LANDMARK_BLEND)))
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

    /// The rise of every column of a mountain centred on (0, 0), as a grid `(2 * reach + 1)` columns across.
    fn heights(m: &Mountain, reach: i32) -> Vec<i16> {
        let side = 2 * reach + 1;
        (0..side * side).map(|i| m.rise_at(i % side - reach, i / side - reach) as i16).collect()
    }

    #[test]
    fn a_mountain_is_tall_and_every_ledge_has_a_gap_so_it_can_still_be_climbed() {
        assert_eq!(MOUNTAIN_HEIGHT, 10 * 17, "ten times the old height");
        assert!(MOUNTAIN_HEIGHT > 3 * HILL_MAX_HEIGHT, "much taller than any hill");
        for phase in [0, 17, 41, 62] {
            let m = mountain(phase);
            assert_eq!(m.rise_at(0, 0), MOUNTAIN_HEIGHT, "the summit");
            assert_eq!(m.rise_at(MOUNTAIN_RADIUS * 2, 0), 0);
            assert_eq!(m.ledge_count(), MOUNTAIN_MAX_LEDGES);
            let reach = farthest_reach(MOUNTAIN_RADIUS) as i32 + 2;
            let side = 2 * reach + 1;
            let grid = heights(&m, reach);
            assert_eq!(*grid.iter().max().unwrap() as i32, MOUNTAIN_HEIGHT);
            // A jump clears a little over one block. Walk from the foot, a block at a time, up steps of one at most
            // (and down any): the summit must be reachable.
            let at = |x: i32, z: i32| ((z + reach) * side + x + reach) as usize;
            let start = at(reach, 0);
            assert_eq!(grid[start], 0);
            let mut seen = vec![false; grid.len()];
            let mut queue = std::collections::VecDeque::from([start]);
            seen[start] = true;
            let mut summit = false;
            while let Some(i) = queue.pop_front() {
                let (x, z) = ((i as i32 % side) - reach, (i as i32 / side) - reach);
                summit |= grid[i] as i32 == MOUNTAIN_HEIGHT;
                for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
                    if nx.abs() > reach || nz.abs() > reach {
                        continue;
                    }
                    let j = at(nx, nz);
                    if !seen[j] && grid[j] - grid[i] <= 1 {
                        seen[j] = true;
                        queue.push_back(j);
                    }
                }
            }
            assert!(summit, "phase {phase}: no way up that a jump can climb");
        }
    }

    #[test]
    fn a_mountain_cannot_be_run_straight_up_from_any_side_but_has_to_be_zigzagged() {
        for phase in [0, 9, 33, 58] {
            let m = mountain(phase);
            let reach = farthest_reach(MOUNTAIN_RADIUS) as i32;
            for ray in 0..72 {
                let angle = ray as f32 * std::f32::consts::TAU / 72.0;
                // Walk the line inwards, column by column, and find the tallest step up.
                let mut last = (i32::MAX, i32::MAX);
                let mut previous = 0;
                let mut tallest = 0;
                for r in (0..=reach * 2).map(|i| reach as f32 - i as f32 * 0.5) {
                    let cell = ((angle.cos() * r).round() as i32, (angle.sin() * r).round() as i32);
                    if cell == last {
                        continue;
                    }
                    last = cell;
                    let here = m.rise_at(cell.0, cell.1);
                    tallest = tallest.max(here - previous);
                    previous = here;
                }
                assert!(tallest >= MOUNTAIN_LEDGE_HEIGHT, "phase {phase}: the way straight up at {ray} is only steps of {tallest}");
            }
            // The cliffs are as tall as the ledges and no more than a ledge and a slope.
            let grid = heights(&m, reach);
            let side = 2 * reach + 1;
            let steepest = (0..grid.len() as i32)
                .filter(|i| i % side + 1 < side)
                .map(|i| (grid[i as usize + 1] - grid[i as usize]).abs())
                .max()
                .unwrap();
            assert!(steepest as i32 <= MOUNTAIN_LEDGE_HEIGHT + 2, "steepest step {steepest}");
        }
    }

    #[test]
    fn the_gaps_in_the_ledges_lie_on_alternate_sides_of_the_mountain() {
        let m = mountain(20);
        let sides: Vec<f32> = (0..m.ledge_count()).map(|k| m.gap_angle(k)).collect();
        for pair in sides.windows(2) {
            assert!((pair[0] - pair[1]).abs() > 2.0 * (MOUNTAIN_GAP_HALF_ANGLE + MOUNTAIN_GAP_BLEND_ANGLE), "neighbouring gaps overlap: {pair:?}");
        }
        // A small mountain (as the tests use) has fewer ledges, or none, and is smooth.
        assert_eq!(Mountain { x: 0, z: 0, radius: 40, height: 10, phase: 0 }.ledge_count(), 0);
    }

    #[test]
    fn the_island_hill_is_flat_on_top_and_only_on_a_lake_big_enough_for_a_castle() {
        let l = Lake { x: 0, z: 0, radius: LAKE_RADIUS, phase: 5, from: (0, 0) };
        assert_eq!(l.hill_rise(0, 0), ISLAND_HILL_HEIGHT);
        assert_eq!(l.hill_rise(ISLAND_PLATEAU_RADIUS as i32, 0), ISLAND_HILL_HEIGHT, "the flat top reaches the plateau's edge");
        assert!(l.hill_rise(ISLAND_PLATEAU_RADIUS as i32 + 10, 0) < ISLAND_HILL_HEIGHT);
        assert_eq!(l.hill_rise(l.radius * 2, 0), 0);
        assert!(l.island_radius() * 0.75 > ISLAND_PLATEAU_RADIUS * 2.0, "a hill much larger than its flat top");
        let small = Lake { x: 0, z: 0, radius: 20, phase: 0, from: (0, 0) };
        assert_eq!(small.hill_rise(0, 0), 0, "a small island is flat");
        // The castle's whole square is on the flat top.
        let corner = (CASTLE_REACH as f32 * std::f32::consts::SQRT_2).ceil();
        assert!(corner <= ISLAND_PLATEAU_RADIUS);
    }

    /// Whether the cell is stone in the grand castle (and the ground, which is the level below it).
    fn grand_solid(dx: i32, dz: i32, dy: i32) -> bool {
        dy < 0 || GrandCastle { x: 0, z: 0 }.castle_block(dx, dz, dy) == Some(Block::CastleStone)
    }

    /// Where a player can stand in the grand castle, found by walking: (x, z, y) is a cell whose feet are in air
    /// with stone beneath. A step up of one block is a jump; falling is allowed. Returns every spot reached from
    /// `start`.
    fn walk_the_grand_castle(start: (i32, i32, i32)) -> std::collections::HashSet<(i32, i32, i32)> {
        let reach = GRAND_REACH + 8;
        let free = |x: i32, z: i32, y: i32| !grand_solid(x, z, y);
        let mut seen = std::collections::HashSet::from([start]);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some((x, z, y)) = queue.pop_front() {
            for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
                if nx.abs() > reach || nz.abs() > reach {
                    continue;
                }
                // Straight ahead, or up one block (which needs room to jump), or down a few.
                let mut options = Vec::new();
                if free(nx, nz, y) && free(nx, nz, y + 1) {
                    let mut ny = y;
                    while ny > y - 5 && free(nx, nz, ny - 1) {
                        ny -= 1;
                    }
                    if grand_solid(nx, nz, ny - 1) {
                        options.push(ny);
                    }
                }
                if free(x, z, y + 2) && free(nx, nz, y + 1) && free(nx, nz, y + 2) && grand_solid(nx, nz, y) {
                    options.push(y + 1);
                }
                for ny in options {
                    if seen.insert((nx, nz, ny)) {
                        queue.push_back((nx, nz, ny));
                    }
                }
            }
        }
        seen
    }

    #[test]
    fn the_grand_castle_can_be_entered_by_its_gate_and_climbed_floor_by_floor_to_the_roof() {
        let reached = walk_the_grand_castle((0, GRAND_WALL_HALF + 6, 0));
        let inside_keep = |&&(x, z, _): &&(i32, i32, i32)| x.abs() <= GRAND_KEEP_HALF - 2 && z.abs() <= GRAND_KEEP_HALF - 2;
        for floor in 0..=GRAND_STOREYS {
            let y = floor * GRAND_STOREY;
            assert!(reached.iter().filter(inside_keep).any(|&(_, _, h)| h == y), "no way to stand on floor {floor} (height {y}) inside the keep");
        }
        let courtyard = (0, GRAND_WALL_HALF - 8);
        assert!(reached.contains(&(courtyard.0, courtyard.1, 0)), "the courtyard is reached through the gate");
        let highest = reached.iter().map(|&(_, _, y)| y).max().unwrap();
        assert!(highest >= GRAND_STOREY * GRAND_STOREYS, "the roof of the keep (and a tower, if they can) is reachable: {highest}");
    }

    #[test]
    fn the_grand_castle_is_large_walled_and_towered_with_a_gate_and_a_keep_door() {
        let c = GrandCastle { x: 100, z: -40 };
        let at = |dx: i32, dz: i32, dy: i32| c.castle_block(c.x + dx, c.z + dz, dy);
        let wall = Some(Block::CastleStone);
        assert_eq!(at(GRAND_WALL_HALF, 0, 3), wall, "the outer wall");
        assert_eq!(at(0, GRAND_WALL_HALF, 3), Some(Block::Air), "the gate, in the south wall");
        assert_eq!(at(2, GRAND_WALL_HALF, 5), Some(Block::Air));
        assert_eq!(at(3, GRAND_WALL_HALF, 0), wall);
        assert_eq!(at(0, GRAND_WALL_HALF, 6), wall, "and a lintel over it");
        assert_eq!(at(GRAND_WALL_HALF, GRAND_WALL_HALF, 22), wall, "a corner tower is taller than the wall");
        assert_eq!(at(GRAND_WALL_HALF - 5, 0, 15), Some(Block::Air), "the wall is lower than the tower");
        assert_eq!(at(0, GRAND_KEEP_HALF, 1), Some(Block::Air), "the keep's door");
        assert_eq!(at(0, GRAND_KEEP_HALF, 4), wall);
        assert_eq!(at(1, -GRAND_KEEP_HALF, 1), wall, "and no door in the north");
        assert_eq!(at(4, -GRAND_KEEP_HALF, 1), Some(Block::Air), "a window");
        assert_eq!(at(0, 0, 1), Some(Block::Air), "empty inside");
        assert_eq!(at(0, 0, GRAND_STOREY - 1), wall, "the first floor");
        assert_eq!(at(0, 0, GRAND_STOREY * GRAND_STOREYS - 1), wall, "the roof");
        assert_eq!(at(0, 0, GRAND_STOREY * 2), Some(Block::Air), "the room above the second floor");
        assert_eq!(at(0, 0, GRAND_HEIGHT), None, "nothing above the castle");
        assert_eq!(c.castle_block(c.x + GRAND_REACH + 1, c.z, 0), None);
        assert!(GRAND_REACH >= GRAND_WALL_HALF + 4 && GRAND_HEIGHT > GRAND_STOREY * GRAND_STOREYS, "the keep's roof fits under the tallest tower");
        // Far larger than the small castle, and five floors tall.
        assert!(GRAND_REACH > 2 * CASTLE_REACH && GRAND_STOREYS >= 3);
        // The trail leads to the gate and then to the keep's door, both on the south side.
        let (gx, gz) = c.gate_approach();
        let (kx, kz) = c.keep_door();
        assert!(gz > (c.z + GRAND_WALL_HALF) as f32 && kz < gz && kx == gx);
        assert!(!grand_solid_at(&c, kx as i32, kz as i32), "the door's step is open air");
    }

    fn grand_solid_at(c: &GrandCastle, wx: i32, wz: i32) -> bool {
        c.castle_block(wx, wz, 0) == Some(Block::CastleStone)
    }

    #[test]
    fn the_grand_castle_flattens_the_hills_round_it_and_counts_as_touching_its_chunks() {
        let mut marks = Landmarks::default();
        marks.grand_castle = Some(GrandCastle { x: 0, z: 0 });
        assert_eq!(marks.relief_scale(0, 0), 0.0);
        assert_eq!(marks.relief_scale(GRAND_REACH, GRAND_REACH), 0.0);
        assert_eq!(marks.relief_scale(GRAND_REACH + 2 + LANDMARK_BLEND, 0), 1.0);
        assert!(marks.touches_chunk(0, 0) && marks.touches_chunk(-1, -1) && !marks.touches_chunk(20, 20));
        assert!(marks.near_grand_castle(GRAND_REACH + 3, 0, 5) && !marks.near_grand_castle(GRAND_REACH + 8, 0, 5));
    }

    #[test]
    fn the_end_of_the_rainbow_is_remembered_after_its_gold_is_taken() {
        let mut marks = Landmarks::default();
        assert_eq!(marks.rainbow_end(), None);
        marks.rainbow = RainbowState::Active(Rainbow { from: (0, 0), gold: (30, 6, 12) });
        assert_eq!(marks.rainbow_end(), Some((30, 6, 12)));
        marks.rainbow = RainbowState::Spent;
        assert_eq!(marks.rainbow_end(), None, "an old save that never kept the place");
        marks.spent_gold = Some((30, 6, 12));
        assert_eq!(marks.rainbow_end(), Some((30, 6, 12)));
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
