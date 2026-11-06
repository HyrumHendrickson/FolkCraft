//! Landmarks: lakes, mountains and the rainbow, which the player brings into the world by exploring.
//!
//! The land of a world is a pure function of its seed (`terrain.rs`). Landmarks are the exception: they are
//! added while the game runs and saved with the world, and the terrain generator consults them.
//!
//! - A **lake** appears when the player has walked ten chunks through a river without leaving the water. It is a
//!   large wavy basin, one block deep like a river, joined to the river by a straight channel. In the middle is a
//!   large island: a hill with a flat top, and on the flat top stands a silver tower (`tower.rs`).
//! - The world's one **mountain** appears when the player has followed a held compass north for ten chunks. It is
//!   tall, can be climbed one block at a time, and is capped with snow. Rings of cliff (ledges) break its slope, each
//!   with one gap, the gaps on alternate sides, so it has to be climbed in a zigzag. Once it is there the compass
//!   always points at it (`compass_bearing`), and there is never a second one.
//! - A **rainbow** appears once, by day, when the player holds a peep stone. It ends in a pile of gold; once the gold
//!   has been taken there is never another rainbow. The peep stone shows a ghostly trail to the end of the rainbow
//!   for ever after.
//! - A **grand castle** appears once, at night, when the player holds a peep stone: a large castle of five floors,
//!   beyond the edge of the land, which the peep stone's ghostly trail leads to. It is whole and shut by night and a
//!   ruin by day (`grand_castle_cell`): the gate, the stair to the second floor and the door of the throne room are
//!   open or closed the opposite way round, and the royal armour lies in the throne room only at night.
//! - Two more silver towers stand in the other dimensions: one on an island of the Skylands (`Landmarks::sky_tower`, brought
//!   out by walking on ten islands) and one on the plain of the Horizen (`Landmarks::horizen_tower`, which a peep stone leads
//!   to by day). Each has a door locked until a silver key from the mines has been used on it (`towers_unlocked`).
//! - A **well** (by day) and a **graveyard** (by night) each appear once, when the player holds a bone totem at that
//!   time of day; the totem's ghostly trail leads to whichever of them is out there. The well is a deep square of
//!   water: whoever falls in is taken to the Horizen (`horizen.rs`).
//!
//! This file holds the data and the rules (where the water and the slopes are, when a trek is complete). The
//! world, the save file and the game only call into it, and none of it needs a window to be tested.

use crate::block::Block;
use crate::config::*;
use crate::rng::hash3;
use crate::tower::{self, Relic, Tower};
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

/// A lake: a large wavy basin of shallow water with a large island in the middle (and a silver tower on the island) and,
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
    /// `lake_distance()` blocks away, joined to `at` by a channel.
    pub fn ahead(at: (f32, f32), dir: (f32, f32), phase: i32) -> Lake {
        let gap = lake_distance() as f32 + farthest_reach(LAKE_RADIUS);
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

    /// Whether the island is big enough, even at its narrowest, to hold the tower with room to spare. Small
    /// lakes (there are none in play, but a saved or hand-made one can be small) have an island and no tower.
    pub fn has_tower(&self) -> bool {
        self.island_radius() * 0.75 >= ISLAND_PLATEAU_RADIUS * 0.9
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
    /// lake too small for a tower, the full `ISLAND_HILL_HEIGHT` on the flat top (a disc of `ISLAND_PLATEAU_RADIUS`
    /// round the centre, which the tower stands on) and a slope between, steady enough to be climbed one block
    /// at a time.
    pub fn hill_rise(&self, wx: i32, wz: i32) -> i32 {
        if !self.has_tower() {
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

    /// What the tower is made of in the cell `dy` blocks above the island's ground at the column (wx, wz) (see
    /// `tower::tower_cell`): `None` outside the tower's square or on a lake with no tower. With `locked` the doorway is
    /// shut by `Block::TowerDoor`, as it is until a silver key has been used on it (see `Landmarks::tower_locked`). With
    /// `ruined` the tower is `tower::ruined_tower_cell` instead (as it is while the lake's monster lives, see
    /// `Landmarks::tower_ruined`), and `locked` does not matter: the ruin has no door to lock.
    pub fn tower_cell(&self, wx: i32, wz: i32, dy: i32, locked: bool, ruined: bool) -> Option<Block> {
        if !self.has_tower() {
            return None;
        }
        if ruined {
            return tower::ruined_tower_cell(wx - self.x, wz - self.z, dy);
        }
        tower::tower_cell(wx - self.x, wz - self.z, dy, locked, Relic::PhilosophersStone)
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the tower's square.
    pub fn near_tower(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.has_tower() && tower::near(wx - self.x, wz - self.z, margin)
    }
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
    /// `mountain_distance()` blocks away.
    pub fn ahead(at: (f32, f32), dir: (f32, f32), phase: i32) -> Mountain {
        let gap = mountain_distance() as f32 + farthest_reach(MOUNTAIN_RADIUS);
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

    /// How far from the centre the foot of the mountain lies in the direction `angle` (radians).
    pub fn foot_at(&self, angle: f32) -> f32 {
        self.radius as f32 * lobes(self.phase, angle)
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
        // The summit is a flat arena for the Yeti (the slope there is already within a block of the top).
        if dist <= SUMMIT_PLATEAU_RADIUS {
            return self.height;
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

    /// What the castle is made of in the cell `dy` blocks above the ground at the column (wx, wz), by night or by day:
    /// see `grand_castle_cell` and `Lake::tower_cell`. `None` outside the castle's square or above its tallest tower.
    pub fn castle_block(&self, wx: i32, wz: i32, dy: i32, night: bool) -> Option<Block> {
        if !(0..GRAND_HEIGHT).contains(&dy) || !self.near(wx, wz, 0) {
            return None;
        }
        Some(grand_castle_cell(self.salt(), wx - self.x, wz - self.z, dy, night))
    }

    /// What the ruined castle is made of in the cell `dy` blocks above the ground at the column (wx, wz) (see `ruins_cell`).
    pub fn ruin_block(&self, wx: i32, wz: i32, dy: i32) -> Option<Block> {
        if !(0..GRAND_HEIGHT).contains(&dy) || !self.near(wx, wz, 0) {
            return None;
        }
        Some(ruins_cell(self.salt(), wx - self.x, wz - self.z, dy))
    }

    /// A number that is the same for every cell of this castle and differs from castle to castle, so that the ruins
    /// of one world crumble differently from another's.
    fn salt(&self) -> i64 {
        self.x as i64 * 7919 + self.z as i64
    }

    /// The spot just outside the gate, on the south side (towards +z), and the door of the keep: where the ghostly
    /// trail goes, as (x, z) in world coordinates, at the middle of a column.
    pub fn gate_approach(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, (self.z + GRAND_WALL_HALF + 10) as f32 + 0.5)
    }

    pub fn keep_door(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, (self.z + GRAND_KEEP_HALF + 2) as f32 + 0.5)
    }

    /// A castle whose nearest edge is `grand_distance()` blocks from `at` (counting along either axis, as the chunks
    /// do), in the direction `angle`: beyond the land that is already made.
    pub fn ahead(at: (f32, f32), angle: f32) -> GrandCastle {
        let (c, s) = (angle.cos(), angle.sin());
        let gap = (grand_distance() + GRAND_REACH) as f32 / c.abs().max(s.abs());
        GrandCastle { x: (at.0 + c * gap).round() as i32, z: (at.1 + s * gap).round() as i32 }
    }
}

/// What the ruined castle has in the cell (`dx`, `dz`, `dy`) from its centre and above the ground: a broken outer wall with a
/// gap in the south side and a tumbled tower at each corner, the stumps of a keep, some rubble, and in the middle of the
/// courtyard, on a low dais, the stone with the sword in it (the blade at `dy` 0, the hilt at `dy` 1).
pub fn ruins_cell(salt: i64, dx: i32, dz: i32, dy: i32) -> Block {
    let (ax, az) = (dx.abs(), dz.abs());
    let w = RUINS_WALL_HALF;
    let h = (hash3(salt, dx, dz, 0x5255) % 1000) as i32;
    // The stone and its dais.
    if (dx, dz) == (0, 0) {
        return match dy {
            0 => Block::SwordInStone,
            1 => Block::SwordHilt,
            _ => Block::Air,
        };
    }
    if ax <= 2 && az <= 2 && dy == 0 && !(ax <= 0 && dz > 0) {
        return Block::CastleStone;
    }
    let edge = ax.max(az);
    // Corner towers: a hollow square, tumbled to different heights.
    let in_tower = ax >= w - RUINS_TOWER_HALF + 1 && az >= w - RUINS_TOWER_HALF + 1 && ax <= w + 1 && az <= w + 1;
    if in_tower {
        let rim = ax == w + 1 || az == w + 1 || ax == w - RUINS_TOWER_HALF + 1 || az == w - RUINS_TOWER_HALF + 1;
        let top = 5 + h % (RUINS_WALL_HEIGHT + 4);
        return if rim && dy < top { Block::CastleStone } else { Block::Air };
    }
    // The outer wall, two thick, broken down to a stump here and there; the gap in the south side is open.
    if edge == w || edge == w - 1 {
        if dz > 0 && az >= w - 1 && ax <= RUINS_GAP_HALF {
            return Block::Air;
        }
        let top = 1 + h % RUINS_WALL_HEIGHT;
        return if dy < top { Block::CastleStone } else { Block::Air };
    }
    // The keep: broken walls round the dais, with a gap on the south side to walk in by.
    if edge == 9 && !(dz > 0 && ax <= 2) && h % 100 < 55 {
        return if dy < 1 + (h / 100) % 6 { Block::CastleStone } else { Block::Air };
    }
    // Rubble in the courtyard, never on the way in from the gap.
    if dy == 0 && edge > 2 && edge < w - 1 && !(dz > 0 && ax <= 2) && h % 100 < 3 {
        return Block::CastleStone;
    }
    Block::Air
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

/// Which part of the grand castle's plan the column (`dx`, `dz`) belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    CornerTower,
    /// A tower beside the gate, or in the middle of the north, east or west wall.
    SideTower,
    Wall,
    Keep,
}

fn grand_part(dx: i32, dz: i32) -> Option<Part> {
    let (ax, az) = (dx.abs(), dz.abs());
    let (wall, keep) = (GRAND_WALL_HALF, GRAND_KEEP_HALF);
    if (ax - wall).abs() <= 4 && (az - wall).abs() <= 4 {
        return Some(Part::CornerTower);
    }
    let gate_tower = (ax - 7).abs() <= 2 && dz > 0 && (az - wall).abs() <= 3;
    let side_tower = (az - wall).abs() <= 3 && ax <= 3 && dz < 0 || (ax - wall).abs() <= 3 && az <= 3;
    if gate_tower || side_tower {
        return Some(Part::SideTower);
    }
    if ax.max(az) >= wall - 1 && ax.max(az) <= wall {
        return Some(Part::Wall);
    }
    if ax <= keep && az <= keep {
        return Some(Part::Keep);
    }
    None
}

/// Whether the cell is in the opening of the gate: a doorway five wide through the two blocks of the south wall, up to
/// the lintel.
fn is_gate_cell(dx: i32, dz: i32, dy: i32) -> bool {
    dz > 0 && dz.abs() >= GRAND_WALL_HALF - 1 && dz.abs() <= GRAND_WALL_HALF && dx.abs() <= 2 && dy <= 5
}

/// The shape of the grand castle as it was built, in blocks from its centre (`dx`, `dz`) and above the ground (`dy`, 0
/// is the first layer of air). Seen from above: an outer wall `GRAND_WALL_HALF` from the centre with a gate in the south
/// side (towards +z) five wide; a solid tower at each corner, two flanking the gate and one in the middle of each of the
/// other three sides; and in the middle a keep of `GRAND_KEEP_HALF`, five floors of `GRAND_STOREY` blocks each, with a
/// door in its south side, windows, a stair from each floor up to the next, and a roof with battlements to walk on.
/// Everything ends in battlements: every other block along the top is left out. The gate is an empty doorway here;
/// `grand_castle_cell` shuts it at night.
fn grand_castle_is_solid(dx: i32, dz: i32, dy: i32) -> bool {
    let (ax, az) = (dx.abs(), dz.abs());
    let merlon = (dx + dz).rem_euclid(2) == 0;
    let keep = GRAND_KEEP_HALF;
    // Up to and including `top`, with a battlement above it.
    let crowned = |top: i32| dy <= top || (dy == top + 1 && merlon);

    match grand_part(dx, dz) {
        Some(Part::CornerTower) => crowned(25),
        Some(Part::SideTower) => crowned(17),
        // The outer wall, two blocks thick, with the gate.
        Some(Part::Wall) => !is_gate_cell(dx, dz, dy) && crowned(11),
        Some(Part::Keep) => {
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
            false
        }
        None => false,
    }
}

/// Whether the cell is on the first step or two of the stair from the ground floor to the second floor: the part of it
/// that is still standing by day.
fn is_stump_of_first_stair(dx: i32, dz: i32) -> bool {
    (1..=2).any(|i| (dx, dz) == keep_stair_cell(0, i))
}

/// Whether the cell is on the stair from the ground floor to the second floor.
fn is_first_stair(dx: i32, dz: i32, dy: i32) -> bool {
    (1..=GRAND_STOREY).any(|i| (dx, dz) == keep_stair_cell(0, i) && dy < i)
}

/// The height (in blocks above the ground) to which the column (`dx`, `dz`) of a ruined part of the castle still
/// stands by day. Neighbouring columns fall together in groups of two by two.
fn ruined_height(salt: i64, dx: i32, dz: i32, part: Part) -> i32 {
    let roll = (hash3(salt, dx.div_euclid(2), dz.div_euclid(2), 0xD41) % 100) as i32;
    match part {
        // The wall is low rubble, with a breach here and there.
        Part::Wall => if roll % 6 == 0 { 0 } else { 1 + roll % RUIN_WALL_MAX },
        Part::SideTower => 3 + roll % 6,
        Part::CornerTower => 6 + roll % 9,
        // The shell of the keep above its second floor is a broken crown.
        Part::Keep => 10 + roll % 8,
    }
}

/// The wall that shuts the throne room off from the rest of the second floor, the door in it, and the throne room
/// itself. The room is on the second floor (the cells 5 to 8 above the ground), west of the wall, which runs
/// north to south across the keep `THRONE_WALL_X` blocks from its centre.
///
/// By night the door is shut (`Block::CastleDoor`), the throne is whole, and the royal armour stands on a pedestal. By
/// day the door is ruined (open), the top of the wall has crumbled in places, the throne is only its seat, there is
/// rubble on the floor and nothing else. `None` for a cell that is not part of it.
fn throne_room_cell(salt: i64, dx: i32, dz: i32, dy: i32, night: bool) -> Option<Block> {
    let rel = dy - GRAND_STOREY;
    if !(0..GRAND_STOREY - 1).contains(&rel) || dz.abs() > GRAND_KEEP_HALF - 2 || !(-(GRAND_KEEP_HALF - 2)..=THRONE_WALL_X).contains(&dx) {
        return None;
    }
    let roll = hash3(salt, dx, dz, 0x7480) % 100;
    if dx == THRONE_WALL_X {
        if dz.abs() <= THRONE_DOOR_HALF && rel <= 2 {
            return Some(if night { Block::CastleDoor } else { Block::Air });
        }
        let crumbled = !night && rel >= 2 && dz.abs() > THRONE_DOOR_HALF && roll % 3 == 0;
        return Some(if crumbled { Block::Air } else { Block::CastleStone });
    }
    let throne_x = -(GRAND_KEEP_HALF - 2);
    let armor_x = throne_x + 3;
    let seat = dx == throne_x && dz == 0 && rel == 0;
    let back = dx == throne_x && dz == 0 && (1..=2).contains(&rel);
    let arms = dx == throne_x && dz.abs() == 1 && rel == 0;
    Some(if night {
        if seat || back || arms {
            Block::CastleStone
        } else if (dx, dz) == (armor_x, 0) && rel == 0 {
            Block::ArmorStand
        } else {
            Block::Air
        }
    } else if seat || (rel == 0 && dz.abs() > 2 && roll % 17 == 0) {
        Block::CastleStone
    } else {
        Block::Air
    })
}

/// What the grand castle has in the cell (`dx`, `dz`, `dy`) at night (`night`) or by day: `Block::CastleStone` for a wall,
/// `Block::CastleDoor` for a shut gate or door, `Block::ArmorStand` for the royal armour, and `Block::Air`.
///
/// - **By night** it is whole: the gate is shut (nobody outside can get in), the stair to the second floor stands, the
///   door of the throne room is shut, and the armour is in the throne room.
/// - **By day** it is a ruin: the gate has collapsed into a wide breach (anyone can get in), the outer wall and the
///   towers are crumbled, the stair to the second floor is gone but for its first two steps, the keep stands to the roof
///   of its second floor and is broken above that, the door of the throne room has fallen, and there is no armour.
///
/// Both the stair and the door can be used only when the other is not, so to reach the armour a player has to be in the
/// keep when day or night changes.
pub fn grand_castle_cell(salt: i64, dx: i32, dz: i32, dy: i32, night: bool) -> Block {
    if let Some(block) = throne_room_cell(salt, dx, dz, dy, night) {
        return block;
    }
    let solid = |dy: i32| grand_castle_is_solid(dx, dz, dy);
    if night {
        if grand_part(dx, dz) == Some(Part::Wall) && is_gate_cell(dx, dz, dy) {
            return Block::CastleDoor;
        }
        return if solid(dy) { Block::CastleStone } else { Block::Air };
    }
    let Some(part) = grand_part(dx, dz) else { return Block::Air };
    let ruin = |block_here: bool| if block_here { Block::CastleStone } else { Block::Air };
    match part {
        Part::Wall => {
            // The gate has fallen in: a wide breach with a little rubble on the ground.
            if dz > 0 && dx.abs() <= 4 && dy <= 7 {
                let rubble = dy == 0 && hash3(salt, dx, dz, 0xB4E) % 3 == 0;
                return ruin(rubble);
            }
            ruin(dy < ruined_height(salt, dx, dz, part) && solid(dy))
        }
        Part::SideTower | Part::CornerTower => ruin(dy < ruined_height(salt, dx, dz, part) && solid(dy)),
        Part::Keep => {
            let shell = dx.abs() >= GRAND_KEEP_HALF - 1 || dz.abs() >= GRAND_KEEP_HALF - 1;
            let roof_of_second_floor = GRAND_STOREY * 2 - 1;
            if dy > roof_of_second_floor {
                // Above the second floor only a broken crown of wall is left: no floors, no stairs.
                return ruin(shell && dy < ruined_height(salt, dx, dz, part) && solid(dy));
            }
            // The stair to the second floor is ruined but for its first two steps.
            if is_first_stair(dx, dz, dy) && !is_stump_of_first_stair(dx, dz) {
                return Block::Air;
            }
            ruin(solid(dy))
        }
    }
}

/// The well: a deep square of water `WELL_SIZE` blocks across, sunk to the stone floor of the world and ringed by a rim
/// one block high. `x` and `z` are the corner of the water with the smallest coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Well {
    pub x: i32,
    pub z: i32,
}

impl Well {
    /// How many blocks the column (wx, wz) is from the square of water, counting along either axis (as a king moves):
    /// 0 over the water itself, 1 on the rim.
    pub fn distance(&self, wx: i32, wz: i32) -> i32 {
        let (dx, dz) = ((self.x - wx).max(wx - (self.x + WELL_SIZE - 1)).max(0), (self.z - wz).max(wz - (self.z + WELL_SIZE - 1)).max(0));
        dx.max(dz)
    }

    /// Whether the column (wx, wz) is over the water of the well.
    pub fn contains(&self, wx: i32, wz: i32) -> bool {
        (self.x..self.x + WELL_SIZE).contains(&wx) && (self.z..self.z + WELL_SIZE).contains(&wz)
    }

    /// Whether the column is within `margin` blocks of the rim.
    pub fn near(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.distance(wx, wz) <= WELL_RIM_WIDTH + margin
    }

    /// What stands in the cell `dy` blocks above the ground at the column (wx, wz): the stone rim, one block high (so a
    /// player can climb on and jump in); a post at each corner of the rim carrying a roof of planks with a ridge; and a rope
    /// hanging from the roof over the water. `None` away from the well or above it.
    pub fn block(&self, wx: i32, wz: i32, dy: i32) -> Option<Block> {
        if !self.near(wx, wz, 0) || !(0..WELL_HEIGHT).contains(&dy) {
            return None;
        }
        // Position in the square of the rim, 0 to WELL_SIZE + 1 on each side.
        let (rx, rz) = (wx - (self.x - 1), wz - (self.z - 1));
        let last = WELL_SIZE + 1;
        let corner = (rx == 0 || rx == last) && (rz == 0 || rz == last);
        let rope = (wx, wz) == (self.x + 1, self.z + 1);
        Some(match dy {
            0 if self.contains(wx, wz) => Block::Air,
            0 => Block::CastleStone,
            1..=4 if corner => Block::BridgePost,
            3..=4 if rope => Block::BridgePost,
            5 => Block::BridgePlank,
            6 if (1..=WELL_SIZE).contains(&rx) && (2..=3).contains(&rz) => Block::BridgePlank,
            _ => Block::Air,
        })
    }

    /// The columns the well can touch, widened by `margin`: (min x, min z, max x, max z).
    pub fn bounds(&self, margin: i32) -> (i32, i32, i32, i32) {
        let reach = WELL_RIM_WIDTH + margin;
        (self.x - reach, self.z - reach, self.x + WELL_SIZE - 1 + reach, self.z + WELL_SIZE - 1 + reach)
    }

    /// The middle of the water, in world coordinates: where the ghostly trail ends.
    pub fn centre(&self) -> (f32, f32) {
        (self.x as f32 + WELL_SIZE as f32 / 2.0, self.z as f32 + WELL_SIZE as f32 / 2.0)
    }

    /// A well whose nearest edge is `well_distance()` blocks from `at` (counting along either axis, as the chunks do),
    /// in the direction `angle`: beyond the land that is already made.
    pub fn ahead(at: (f32, f32), angle: f32) -> Well {
        let (c, s) = (angle.cos(), angle.sin());
        let gap = (well_distance() + WELL_SIZE) as f32 / c.abs().max(s.abs());
        Well { x: (at.0 + c * gap).round() as i32 - WELL_SIZE / 2, z: (at.1 + s * gap).round() as i32 - WELL_SIZE / 2 }
    }
}

/// The graveyard: a walled square of gravestones. Centre in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graveyard {
    pub x: i32,
    pub z: i32,
}

impl Graveyard {
    /// The columns the graveyard can touch, widened by `margin`: (min x, min z, max x, max z).
    pub fn bounds(&self, margin: i32) -> (i32, i32, i32, i32) {
        let reach = GRAVEYARD_REACH + margin;
        (self.x - reach, self.z - reach, self.x + reach, self.z + reach)
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the graveyard's square.
    pub fn near(&self, wx: i32, wz: i32, margin: i32) -> bool {
        (wx - self.x).abs() <= GRAVEYARD_REACH + margin && (wz - self.z).abs() <= GRAVEYARD_REACH + margin
    }

    /// What stands in the cell `dy` blocks above the ground at the column (wx, wz): a low crenellated wall all round
    /// with a gate three wide in the south side (towards +z), a path up the middle and, in rows, gravestones. Every
    /// other cell of the square is left empty. `None` outside the square or above the wall.
    pub fn block(&self, wx: i32, wz: i32, dy: i32) -> Option<Block> {
        if !self.near(wx, wz, 0) || !(0..2).contains(&dy) {
            return None;
        }
        let (dx, dz) = (wx - self.x, wz - self.z);
        let (ax, az) = (dx.abs(), dz.abs());
        let half = GRAVEYARD_HALF;
        if ax.max(az) == half {
            let gate = dz == half && ax <= 1;
            let merlon = (dx + dz).rem_euclid(2) == 0;
            return Some(if !gate && (dy == 0 || merlon) { Block::CastleStone } else { Block::Air });
        }
        let in_rows = ax <= half - 3 && az <= half - 3 && ax > 1 && dx.rem_euclid(GRAVEYARD_SPACING) == 0 && dz.rem_euclid(GRAVEYARD_SPACING) == 0;
        Some(if dy == 0 && in_rows { Block::Gravestone } else { Block::Air })
    }

    /// The spot just outside the gate, on the south side, and the middle of the graveyard: where the ghostly trail goes.
    pub fn gate_approach(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, (self.z + GRAVEYARD_HALF + 8) as f32 + 0.5)
    }

    pub fn middle(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, self.z as f32 + 0.5)
    }

    /// A graveyard whose nearest edge is `graveyard_distance()` blocks from `at` (counting along either axis), in the
    /// direction `angle`.
    pub fn ahead(at: (f32, f32), angle: f32) -> Graveyard {
        let (c, s) = (angle.cos(), angle.sin());
        let gap = (graveyard_distance() + GRAVEYARD_REACH) as f32 / c.abs().max(s.abs());
        Graveyard { x: (at.0 + c * gap).round() as i32, z: (at.1 + s * gap).round() as i32 }
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
    /// Where the rainbow's gold lay, once it has been taken (so the peep stone still leads there).
    pub spent_gold: Option<Pos>,
    /// The world's one grand castle, once a peep stone has been held at night.
    pub grand_castle: Option<GrandCastle>,
    /// The world's one ruined castle (same footprint as the grand castle), with the sword in the stone at its centre, once the
    /// king's crown has been worn in the overworld.
    pub ruined_castle: Option<GrandCastle>,
    /// The world's one well, once a bone totem has been held by day.
    pub well: Option<Well>,
    /// The world's one graveyard, once a bone totem has been held at night.
    pub graveyard: Option<Graveyard>,
    /// Whether the Yeti on the mountain's summit has been killed (a cyclone then stands there).
    pub yeti_slain: bool,
    /// The lakes (by their centre) whose Loch Ness Monster has been killed.
    pub monsters_slain: Vec<(i32, i32)>,
    /// The silver towers whose door has been unlocked, by the column of their centre (a lake's centre for the tower on its
    /// island). Each world keeps its own: the Skylands and the Horizen have theirs.
    pub towers_unlocked: Vec<(i32, i32)>,
    /// The Horizen's one silver tower, once a peep stone has been held by day there. Only the Horizen has one.
    pub horizen_tower: Option<Tower>,
    /// The Skylands' one silver tower: the grid cell of the island it stands on, once it has been brought out. Only the
    /// Skylands have one.
    pub sky_tower: Option<(i32, i32)>,
    /// The grid cells of the Skylands' islands the player has walked on.
    pub sky_visited: Vec<(i32, i32)>,
    /// Whether the graveyard's ghost has been played its song and has rested (it says nothing more).
    pub graveyard_rested: bool,
    /// Whether the player has beaten the Wild Hunt, and so won the game (they may play on).
    pub game_won: bool,
    /// Whether the Phoenix of the Skylands' volcano is dead: it does not return, and the fire tornadoes have stopped for good.
    pub phoenix_slain: bool,
    /// The squares of the Horizen's caves the player has entered, in the order they entered them.
    pub caves_visited: Vec<(i32, i32)>,
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

    /// Whether a structure whose columns lie in `bounds` (as `GrandCastle::bounds` gives them, widened by the margin the
    /// ground blends over) would stand clear of every lake, mountain and other site of the world.
    fn has_room(&self, bounds: (i32, i32, i32, i32), ignoring_castle: bool) -> bool {
        let margin = LANDMARK_BLEND + 4;
        let (cx0, cz0, cx1, cz1) = bounds;
        let overlaps = |(x0, z0, x1, z1): (i32, i32, i32, i32)| x0 <= cx1 && x1 >= cx0 && z0 <= cz1 && z1 >= cz0;
        !self.lakes.iter().any(|l| overlaps(l.bounds(margin)))
            && !self.mountains.iter().any(|m| overlaps(m.bounds(margin)))
            && (ignoring_castle || !self.grand_castle.map_or(false, |c| overlaps(c.bounds(margin))))
            && !self.ruined_castle.map_or(false, |c| overlaps(c.bounds(margin)))
            && !self.well.map_or(false, |w| overlaps(w.bounds(margin)))
            && !self.graveyard.map_or(false, |g| overlaps(g.bounds(margin)))
    }

    /// Whether a grand castle at `castle` would stand clear of every lake and mountain (and of the ground they
    /// blend into), and of the well and the graveyard.
    pub fn has_room_for(&self, castle: &GrandCastle) -> bool {
        self.has_room(castle.bounds(LANDMARK_BLEND + 4), true)
    }

    /// The same for the ruined castle: clear of the grand castle too.
    pub fn has_room_for_ruins(&self, castle: &GrandCastle) -> bool {
        self.has_room(castle.bounds(LANDMARK_BLEND + 4), false)
    }

    /// The same for a well.
    pub fn has_room_for_well(&self, well: &Well) -> bool {
        self.has_room(well.bounds(LANDMARK_BLEND + 4), false)
    }

    /// The same for a graveyard.
    pub fn has_room_for_graveyard(&self, graveyard: &Graveyard) -> bool {
        self.has_room(graveyard.bounds(LANDMARK_BLEND + 4), false)
    }

    /// The column at the middle of the mountain's summit, where the Yeti lives and, once it is dead, the cyclone stands.
    pub fn summit(&self) -> Option<(i32, i32)> {
        self.mountain().map(|m| (m.x, m.z))
    }

    /// Whether the door of the tower on the lake's island is still locked: it is, until a silver key has been used on it.
    pub fn tower_locked(&self, lake: &Lake) -> bool {
        lake.has_tower() && !self.towers_unlocked.contains(&(lake.x, lake.z))
    }

    /// Whether the tower on the lake's island is a ruin: it is, with its stair broken, for as long as the lake's Loch Ness
    /// Monster lives. Killing the monster makes the tower whole again, with its door locked until a silver key opens it.
    pub fn tower_ruined(&self, lake: &Lake) -> bool {
        self.monster_alive(lake)
    }

    /// Whether the lake has a Loch Ness Monster still alive (every lake with a tower has one until it is killed).
    pub fn monster_alive(&self, lake: &Lake) -> bool {
        lake.has_tower() && !self.monsters_slain.contains(&(lake.x, lake.z))
    }

    /// Whether the tower centred on the column (x, z) has been unlocked.
    pub fn is_unlocked(&self, x: i32, z: i32) -> bool {
        self.towers_unlocked.contains(&(x, z))
    }

    /// Note that the player has walked on the Skylands island of grid cell `cell`. Returns whether it is one they had not
    /// walked on before.
    pub fn note_sky_visit(&mut self, cell: (i32, i32)) -> bool {
        if self.sky_visited.contains(&cell) {
            return false;
        }
        self.sky_visited.push(cell);
        true
    }

    /// The world's one mountain, if it has come out yet. (A save made when mountains could be many holds more than
    /// one; the first is the one the compass points at.)
    pub fn mountain(&self) -> Option<&Mountain> {
        self.mountains.first()
    }

    /// Whether the column (wx, wz) is over the water of the well.
    pub fn is_well(&self, wx: i32, wz: i32) -> bool {
        self.well.map_or(false, |w| w.contains(wx, wz))
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the rim of the well.
    pub fn near_well(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.well.map_or(false, |w| w.near(wx, wz, margin))
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the graveyard's square.
    pub fn near_graveyard(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.graveyard.map_or(false, |g| g.near(wx, wz, margin))
    }

    /// Whether no river may run over the column (wx, wz): the ground round the grand castle, the well and the
    /// graveyard is kept dry and level.
    pub fn keeps_rivers_out(&self, wx: i32, wz: i32) -> bool {
        self.near_grand_castle(wx, wz, 2) || self.near_well(wx, wz, 3) || self.near_graveyard(wx, wz, 2)
    }

    /// Whether no tree may grow at the column (wx, wz): the towers and castles stand in clearings, and so do the well and the
    /// graveyard.
    pub fn keeps_trees_out(&self, wx: i32, wz: i32) -> bool {
        self.near_tower(wx, wz, TOWER_TREE_MARGIN)
            || self.near_grand_castle(wx, wz, GRAND_TREE_MARGIN)
            || self.near_well(wx, wz, WELL_TREE_MARGIN)
            || self.near_graveyard(wx, wz, GRAVEYARD_TREE_MARGIN)
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the grand castle's square.
    pub fn near_grand_castle(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.castles().any(|c| c.near(wx, wz, margin))
    }

    /// The grand castle and the ruined castle, whichever of them are out there (they share one footprint and one set of rules
    /// for the ground round them).
    pub fn castles(&self) -> impl Iterator<Item = GrandCastle> {
        [self.grand_castle, self.ruined_castle].into_iter().flatten()
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
        let castle = self.castles().map(|c| (wx - c.x).abs().max((wz - c.z).abs()) as f32 - (GRAND_REACH + 2) as f32).reduce(f32::min);
        // The well and the graveyard stand on level ground too.
        let well = self.well.map(|w| w.distance(wx, wz) as f32 - (WELL_RIM_WIDTH + 2) as f32);
        let graveyard = self.graveyard.map(|g| (wx - g.x).abs().max((wz - g.z).abs()) as f32 - (GRAVEYARD_REACH + 2) as f32);
        lakes.chain(mountains).chain(castle).chain(well).chain(graveyard).map(|d| smoothstep(d / blend)).fold(1.0, f32::min)
    }

    /// Whether the column (wx, wz) is within `margin` blocks of the square a lake's tower stands in.
    pub fn near_tower(&self, wx: i32, wz: i32, margin: i32) -> bool {
        self.lakes.iter().any(|l| l.near_tower(wx, wz, margin))
    }

    /// Whether any lake or mountain (with the margin the hills fade over) can touch the chunk (cx, cz).
    pub fn touches_chunk(&self, cx: i32, cz: i32) -> bool {
        let (x0, z0) = (cx * CHUNK_SIZE, cz * CHUNK_SIZE);
        let (x1, z1) = (x0 + CHUNK_SIZE - 1, z0 + CHUNK_SIZE - 1);
        let overlaps = |(bx0, bz0, bx1, bz1): (i32, i32, i32, i32)| bx0 <= x1 && bx1 >= x0 && bz0 <= z1 && bz1 >= z0;
        self.lakes.iter().any(|l| overlaps(l.bounds(LANDMARK_BLEND)))
            || self.mountains.iter().any(|m| overlaps(m.bounds(LANDMARK_BLEND)))
            || self.castles().any(|c| overlaps(c.bounds(LANDMARK_BLEND)))
            || self.well.map_or(false, |w| overlaps(w.bounds(LANDMARK_BLEND)))
            || self.graveyard.map_or(false, |g| overlaps(g.bounds(LANDMARK_BLEND)))
    }

    /// Whether the grand castle can touch the chunk (cx, cz). Only it changes between day and night.
    pub fn castle_touches_chunk(&self, cx: i32, cz: i32) -> bool {
        let (x0, z0) = (cx * CHUNK_SIZE, cz * CHUNK_SIZE);
        let (x1, z1) = (x0 + CHUNK_SIZE - 1, z0 + CHUNK_SIZE - 1);
        self.grand_castle.map_or(false, |c| {
            let (bx0, bz0, bx1, bz1) = c.bounds(0);
            bx0 <= x1 && bx1 >= x0 && bz0 <= z1 && bz1 >= z0
        })
    }

    /// The end of the well's trail: the middle of its water, once the well is out there.
    pub fn well_end(&self) -> Option<(f32, f32)> {
        self.well.map(|w| w.centre())
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

/// The bearing (radians, like a yaw) the compass needle points along for a player standing at `at` (x, z): towards the
/// world's one mountain once it has come out, and north until then.
pub fn compass_bearing(marks: &Landmarks, at: (f32, f32)) -> f32 {
    match marks.mountain() {
        Some(m) => {
            let (dx, dz) = (m.x as f32 + 0.5 - at.0, m.z as f32 + 0.5 - at.1);
            if dx.abs() + dz.abs() < 1e-3 { COMPASS_NORTH_BEARING } else { dz.atan2(dx) }
        }
        None => COMPASS_NORTH_BEARING,
    }
}

/// Where the compass needle points on the screen, for a player looking along `yaw` with the needle along `bearing`
/// (see `compass_bearing`): (right, down), so a needle pointing straight up the screen is (0, -1). A target dead ahead
/// gives (0, -1); one to the player's right gives a vector pointing right.
pub fn compass_needle(yaw: f32, bearing: f32) -> (f32, f32) {
    let relative = bearing - yaw;
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
    /// Set by the game once the world's one mountain is out: the compass then leads to it and no further trek brings a
    /// second.
    pub mountain_found: bool,
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

        if compass && !self.mountain_found {
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
        // The near shore, off the channel, is at least lake_distance() blocks from the player.
        let reach = farthest_reach(LAKE_RADIUS) as i32;
        let near = (-reach..=reach).step_by(2).flat_map(|dz| (-reach..=reach).step_by(2).map(move |dx| (l.x + dx, l.z + dz))).filter(|&(x, z)| l.contains(x, z) && (z - 10).abs() > 4);
        for (x, z) in near {
            assert!((((x - 10) as f32).hypot((z - 10) as f32)) >= lake_distance() as f32 - 1.0, "water at {x},{z} is too close");
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
    fn the_tower_stands_in_the_middle_of_the_island_with_a_locked_door_in_its_south_side() {
        let l = Lake { x: 100, z: -50, radius: LAKE_RADIUS, phase: 5, from: (0, 0) };
        assert!(l.has_tower());
        // Every column of the tower's square is dry island, with room to spare.
        for dz in -TOWER_REACH - 3..=TOWER_REACH + 3 {
            for dx in -TOWER_REACH - 3..=TOWER_REACH + 3 {
                assert!(!l.contains(l.x + dx, l.z + dz), "water at the tower's foot: {dx},{dz}");
            }
        }
        let at = |dx: i32, dz: i32, dy: i32, locked: bool| l.tower_cell(l.x + dx, l.z + dz, dy, locked, false);
        assert_eq!(at(TOWER_HALF, 0, 0, true), Some(Block::SilverStone), "the east wall");
        assert_eq!(at(0, TOWER_HALF, 0, true), Some(Block::TowerDoor), "the lock, in the south wall");
        assert_eq!(at(0, TOWER_HALF, 1, true), Some(Block::TowerDoorTop));
        assert_eq!(at(0, TOWER_HALF, 0, false), Some(Block::Air), "open once unlocked");
        assert_eq!(at(0, 0, TOWER_STOREY * (TOWER_STOREYS - 1), true), Some(Block::StonePlinth), "the Philosopher's Stone at the top");
        assert_eq!(at(0, 0, TOWER_HEIGHT, true), None, "nothing above the battlements");
        assert_eq!(l.tower_cell(l.x + TOWER_REACH + 1, l.z, 0, true, false), None, "nothing outside its square");
        assert!(l.near_tower(l.x + TOWER_REACH + TOWER_TREE_MARGIN, l.z, TOWER_TREE_MARGIN));
    }

    #[test]
    fn a_small_lake_has_an_island_but_no_tower() {
        let l = Lake { x: 0, z: 0, radius: 20, phase: 0, from: (0, 0) };
        assert!(!l.has_tower() && l.tower_cell(0, 0, 0, true, false).is_none());
        assert!(!l.near_tower(0, 0, 100));
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
    fn the_island_hill_is_flat_on_top_and_only_on_a_lake_big_enough_for_a_tower() {
        let l = Lake { x: 0, z: 0, radius: LAKE_RADIUS, phase: 5, from: (0, 0) };
        assert_eq!(l.hill_rise(0, 0), ISLAND_HILL_HEIGHT);
        assert_eq!(l.hill_rise(ISLAND_PLATEAU_RADIUS as i32, 0), ISLAND_HILL_HEIGHT, "the flat top reaches the plateau's edge");
        assert!(l.hill_rise(ISLAND_PLATEAU_RADIUS as i32 + 10, 0) < ISLAND_HILL_HEIGHT);
        assert_eq!(l.hill_rise(l.radius * 2, 0), 0);
        assert!(l.island_radius() * 0.75 > ISLAND_PLATEAU_RADIUS * 2.0, "a hill much larger than its flat top");
        let small = Lake { x: 0, z: 0, radius: 20, phase: 0, from: (0, 0) };
        assert_eq!(small.hill_rise(0, 0), 0, "a small island is flat");
        // The tower's whole square is on the flat top.
        let corner = (TOWER_REACH as f32 * std::f32::consts::SQRT_2).ceil();
        assert!(corner <= ISLAND_PLATEAU_RADIUS);
    }

    const SALT: i64 = 0;

    fn cell(dx: i32, dz: i32, dy: i32, night: bool) -> Block {
        if dy < 0 { Block::Dirt } else { grand_castle_cell(SALT, dx, dz, dy, night) }
    }

    /// Whether the cell is solid in the grand castle (and the ground, which is the level below it).
    fn grand_solid(dx: i32, dz: i32, dy: i32, night: bool) -> bool {
        cell(dx, dz, dy, night).collides()
    }

    /// Where a player can stand in the grand castle, found by walking: (x, z, y) is a cell whose feet are in air
    /// with stone beneath. A step up of one block is a jump; falling is allowed. Returns every spot reached from
    /// `start`.
    fn walk_the_grand_castle(start: (i32, i32, i32), night: bool) -> std::collections::HashSet<(i32, i32, i32)> {
        let reach = GRAND_REACH + 8;
        let free = |x: i32, z: i32, y: i32| !grand_solid(x, z, y, night);
        let solid = |x: i32, z: i32, y: i32| grand_solid(x, z, y, night);
        let mut seen = std::collections::HashSet::from([start]);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some((x, z, y)) = queue.pop_front() {
            for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
                if nx.abs() > reach || nz.abs() > reach {
                    continue;
                }
                let mut options = Vec::new();
                if free(nx, nz, y) && free(nx, nz, y + 1) {
                    let mut ny = y;
                    while ny > y - 5 && free(nx, nz, ny - 1) {
                        ny -= 1;
                    }
                    if solid(nx, nz, ny - 1) {
                        options.push(ny);
                    }
                }
                if free(x, z, y + 2) && free(nx, nz, y + 1) && free(nx, nz, y + 2) && solid(nx, nz, y) {
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

    fn on_second_floor(&(x, z, y): &(i32, i32, i32)) -> bool {
        y == GRAND_STOREY && x.abs() <= GRAND_KEEP_HALF - 2 && z.abs() <= GRAND_KEEP_HALF - 2
    }

    fn in_throne_room(&(x, z, y): &(i32, i32, i32)) -> bool {
        y == GRAND_STOREY && x < THRONE_WALL_X && z.abs() <= GRAND_KEEP_HALF - 2
    }

    #[test]
    fn by_night_the_gate_is_shut_and_nobody_outside_can_get_in() {
        let reached = walk_the_grand_castle((0, GRAND_WALL_HALF + 6, 0), true);
        assert!(!reached.contains(&(0, GRAND_WALL_HALF - 8, 0)), "the courtyard is out of reach");
        assert!(!reached.iter().any(|&(x, z, y)| y == 0 && x.abs() < GRAND_WALL_HALF - 1 && z.abs() < GRAND_WALL_HALF - 1), "nothing inside the wall at ground level");
        assert_eq!(cell(0, GRAND_WALL_HALF, 0, true), Block::CastleDoor);
        assert_eq!(cell(2, GRAND_WALL_HALF - 1, 5, true), Block::CastleDoor);
        assert_eq!(cell(3, GRAND_WALL_HALF, 0, true), Block::CastleStone);
    }

    #[test]
    fn by_day_the_gate_has_fallen_and_the_courtyard_and_keep_door_can_be_reached() {
        let reached = walk_the_grand_castle((0, GRAND_WALL_HALF + 6, 0), false);
        assert!(reached.contains(&(0, GRAND_WALL_HALF - 8, 0)), "through the breach to the courtyard");
        assert!(reached.contains(&(0, GRAND_KEEP_HALF - 3, 0)), "and in at the door of the keep");
        assert_eq!(cell(0, GRAND_WALL_HALF, 1, false), Block::Air);
    }

    #[test]
    fn by_day_the_stair_to_the_second_floor_is_ruined_and_by_night_it_works() {
        // Inside, from the door of the keep.
        let day = walk_the_grand_castle((0, GRAND_KEEP_HALF - 3, 0), false);
        assert!(day.iter().any(|&(x, z, y)| y == 0 && x.abs() < GRAND_KEEP_HALF - 1 && z.abs() < GRAND_KEEP_HALF - 1), "the ground floor");
        assert!(!day.iter().any(on_second_floor), "by day no way up to the second floor");
        let night = walk_the_grand_castle((0, GRAND_KEEP_HALF - 3, 0), true);
        assert!(night.iter().any(on_second_floor), "by night the stair climbs to the second floor");
        assert!(night.iter().any(|&(_, _, y)| y >= GRAND_STOREY * GRAND_STOREYS), "and on, floor by floor, to the roof");
    }

    #[test]
    fn the_throne_room_door_is_shut_by_night_and_open_by_day_and_the_armour_is_there_only_at_night() {
        // The second floor, east of the throne room wall (where the stair arrives).
        let start = (GRAND_KEEP_HALF - 2, 4, GRAND_STOREY);
        let night = walk_the_grand_castle(start, true);
        assert!(night.iter().any(on_second_floor));
        assert!(!night.iter().any(in_throne_room), "shut by night");
        let day = walk_the_grand_castle(start, false);
        assert!(day.iter().any(on_second_floor));
        assert!(day.iter().any(in_throne_room), "ruined by day: it can be entered");
        for dz in -THRONE_DOOR_HALF..=THRONE_DOOR_HALF {
            assert_eq!(cell(THRONE_WALL_X, dz, GRAND_STOREY, true), Block::CastleDoor);
            assert_eq!(cell(THRONE_WALL_X, dz, GRAND_STOREY, false), Block::Air);
        }
        let armour = |night: bool| {
            (-GRAND_KEEP_HALF..=GRAND_KEEP_HALF)
                .flat_map(|x| (-GRAND_KEEP_HALF..=GRAND_KEEP_HALF).flat_map(move |z| (0..GRAND_HEIGHT).map(move |y| (x, z, y))))
                .filter(|&(x, z, y)| cell(x, z, y, night) == Block::ArmorStand)
                .collect::<Vec<_>>()
        };
        assert_eq!(armour(true).len(), 1, "one set of armour by night");
        let (x, z, y) = armour(true)[0];
        assert!(in_throne_room(&(x, z, y)));
        assert!(armour(false).is_empty(), "none by day");
        // Little else in the room by day.
        let stones = |night: bool| (-9..THRONE_WALL_X).flat_map(|x| (-9..=9).map(move |z| (x, z))).filter(|&(x, z)| cell(x, z, GRAND_STOREY, night).collides()).count();
        assert!(stones(false) < stones(true) + 20, "by day the room holds only a seat and a little rubble");
    }

    #[test]
    fn crossing_the_castle_at_the_turn_of_day_and_night_reaches_the_armour() {
        // In the keep by day, one cannot climb; at night one can climb, and then by day the door is open.
        let up_at_night = walk_the_grand_castle((0, GRAND_KEEP_HALF - 3, 0), true);
        let &(x, z, y) = up_at_night.iter().find(|p| on_second_floor(p)).expect("second floor by night");
        let then_day = walk_the_grand_castle((x, z, y), false);
        assert!(then_day.iter().any(in_throne_room), "from the second floor, by day, the throne room is open");
    }

    #[test]
    fn the_ruined_castle_is_lower_than_the_whole_one() {
        let height = |night: bool| {
            (-GRAND_REACH..=GRAND_REACH).flat_map(|x| (-GRAND_REACH..=GRAND_REACH).map(move |z| (x, z))).map(|(x, z)| (0..GRAND_HEIGHT).filter(|&y| cell(x, z, y, night).collides()).max().unwrap_or(-1)).collect::<Vec<_>>()
        };
        let (day, night) = (height(false), height(true));
        assert!(day.iter().zip(&night).all(|(d, n)| d <= n), "ruins never rise above the whole castle");
        assert!(day.iter().sum::<i32>() < night.iter().sum::<i32>() * 2 / 3, "a ruin is much lower");
        assert!(day.iter().any(|&h| h > 4), "though some of it still stands tall");
    }

    #[test]
    fn the_grand_castle_is_large_walled_and_towered_with_a_gate_and_a_keep_door() {
        let c = GrandCastle { x: 100, z: -40 };
        let at = |dx: i32, dz: i32, dy: i32| c.castle_block(c.x + dx, c.z + dz, dy, true);
        let wall = Some(Block::CastleStone);
        assert_eq!(at(GRAND_WALL_HALF, 0, 3), wall, "the outer wall");
        assert_eq!(at(0, GRAND_WALL_HALF, 3), Some(Block::CastleDoor), "the gate, in the south wall, shut");
        assert_eq!(at(2, GRAND_WALL_HALF, 5), Some(Block::CastleDoor));
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
        assert_eq!(c.castle_block(c.x + GRAND_REACH + 1, c.z, 0, true), None);
        assert!(GRAND_REACH >= GRAND_WALL_HALF + 4 && GRAND_HEIGHT > GRAND_STOREY * GRAND_STOREYS, "the keep's roof fits under the tallest tower");
        assert!(GRAND_REACH > 2 * TOWER_REACH && GRAND_STOREYS >= 3);
        // The trail leads to the gate and then to the keep's door, both on the south side.
        let (gx, gz) = c.gate_approach();
        let (kx, kz) = c.keep_door();
        assert!(gz > (c.z + GRAND_WALL_HALF) as f32 && kz < gz && kx == gx);
        assert!(!grand_solid_at(&c, kx as i32, kz as i32), "the door's step is open air");
    }

    fn grand_solid_at(c: &GrandCastle, wx: i32, wz: i32) -> bool {
        c.castle_block(wx, wz, 0, true) == Some(Block::CastleStone)
    }

    #[test]
    fn the_well_is_a_four_by_four_square_of_water_with_a_one_block_rim() {
        let w = Well { x: 10, z: -20 };
        let water: Vec<_> = (0..40).flat_map(|z| (0..40).map(move |x| (x, z - 30))).filter(|&(x, z)| w.contains(x, z)).collect();
        assert_eq!(water.len(), (WELL_SIZE * WELL_SIZE) as usize);
        assert_eq!(w.block(9, -21, 0), Some(Block::CastleStone), "the rim, a corner");
        assert_eq!(w.block(10, -20, 0), Some(Block::Air), "no rim over the water");
        assert_eq!(w.block(8, -20, 0), None, "nothing beyond the rim");
        assert_eq!(w.block(9, -20, 1), Some(Block::Air), "the rim is one block high, so a player can climb on it");
        assert_eq!(w.block(9, -21, 2), Some(Block::BridgePost), "a post at each corner");
        assert_eq!(w.block(14, -16, 4), Some(Block::BridgePost));
        assert_eq!(w.block(11, -19, 4), Some(Block::BridgePost), "the rope hangs over the water");
        assert_eq!(w.block(11, -19, 1), Some(Block::Air), "but not down into it");
        assert_eq!(w.block(11, -18, 5), Some(Block::BridgePlank), "a roof over the whole well");
        assert_eq!(w.block(9, -21, 5), Some(Block::BridgePlank));
        assert_eq!(w.block(11, -18, 6), Some(Block::BridgePlank), "with a ridge");
        assert_eq!(w.block(9, -20, WELL_HEIGHT), None, "nothing above it");
        let (cx, cz) = w.centre();
        assert!(w.contains(cx as i32, cz as i32));
    }

    #[test]
    fn the_graveyard_has_a_wall_a_gate_a_path_and_rows_of_gravestones() {
        let g = Graveyard { x: 50, z: 50 };
        let at = |dx: i32, dz: i32, dy: i32| g.block(g.x + dx, g.z + dz, dy);
        assert_eq!(at(GRAVEYARD_HALF, 0, 0), Some(Block::CastleStone));
        assert_eq!(at(0, GRAVEYARD_HALF, 0), Some(Block::Air), "the gate, in the south wall");
        assert_eq!(at(1, GRAVEYARD_HALF, 1), Some(Block::Air));
        assert_eq!(at(0, 3, 0), Some(Block::Air), "a path up the middle");
        assert_eq!(at(3, 3, 0), Some(Block::Gravestone));
        assert_eq!(at(-6, -9, 0), Some(Block::Gravestone));
        assert_eq!(at(3, 3, 1), Some(Block::Air));
        assert_eq!(at(GRAVEYARD_REACH + 1, 0, 0), None);
        let stones = (-GRAVEYARD_REACH..=GRAVEYARD_REACH).flat_map(|z| (-GRAVEYARD_REACH..=GRAVEYARD_REACH).map(move |x| (x, z))).filter(|&(x, z)| at(x, z, 0) == Some(Block::Gravestone)).count();
        assert!(stones >= 30, "{stones} gravestones");
        // The way in from the gate to the middle is clear.
        for dz in 0..=GRAVEYARD_HALF {
            assert_ne!(at(0, dz, 0), Some(Block::CastleStone));
            assert_ne!(at(0, dz, 0), Some(Block::Gravestone));
        }
    }

    #[test]
    fn the_compass_points_north_until_the_mountain_is_out_and_then_at_it() {
        let mut marks = Landmarks::default();
        assert_eq!(compass_bearing(&marks, (5.0, 5.0)), COMPASS_NORTH_BEARING);
        marks.mountains.push(Mountain { x: 1000, z: 0, radius: 40, height: 10, phase: 0 });
        let east = compass_bearing(&marks, (0.0, 0.0));
        assert!(east.abs() < 0.01, "the mountain lies east: {east}");
        let south = compass_bearing(&marks, (1000.0, -500.0));
        assert!((south - std::f32::consts::FRAC_PI_2).abs() < 0.01, "from the north of it the needle points south (+z): {south}");
        // The needle on the screen points at it however the player turns.
        let (x, y) = compass_needle(0.0, east);
        assert!(x.abs() < 1e-2 && (y + 1.0).abs() < 1e-2, "facing east, the mountain is dead ahead");
    }

    #[test]
    fn once_the_mountain_is_found_no_trek_brings_a_second() {
        let reach = (COMPASS_TREK_CHUNKS * CHUNK_SIZE) as f32;
        let mut treks = Treks::new();
        treks.mountain_found = true;
        assert!(walk(&mut treks, (0.0, 0.0), (0.0, -reach * 3.0), false, true).is_empty());
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
        assert!((m.x - 100).abs() <= 1 && m.z < 500 - mountain_distance(), "north is smaller z: {m:?}");
        assert_eq!(m.height, MOUNTAIN_HEIGHT);
        let nearest = (-reach_of(&m)..=reach_of(&m)).flat_map(|dz| (-reach_of(&m)..=reach_of(&m)).map(move |dx| (m.x + dx, m.z + dz))).filter(|&(x, z)| m.rise_at(x, z) > 0).map(|(x, z)| ((x - 100) as f32).hypot((z - 500) as f32)).fold(f32::MAX, f32::min);
        assert!(nearest >= mountain_distance() as f32 - 1.0, "the foot is {nearest} away");
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
        let (x, y) = compass_needle(COMPASS_NORTH_BEARING, COMPASS_NORTH_BEARING);
        assert!(x.abs() < 1e-5 && (y + 1.0).abs() < 1e-5);
        // Facing east (yaw 0, forward is +x): north (-z) is on the left, so the needle points left.
        let (x, y) = compass_needle(0.0, COMPASS_NORTH_BEARING);
        assert!(x < -0.99 && y.abs() < 1e-5, "({x}, {y})");
        // Facing south: north is behind, so the needle points down.
        let (_, y) = compass_needle(std::f32::consts::FRAC_PI_2, COMPASS_NORTH_BEARING);
        assert!(y > 0.99);
    }

    #[test]
    fn the_rainbow_starts_unseen() {
        assert_eq!(Landmarks::default().rainbow, RainbowState::Unseen);
    }
}

#[cfg(test)]
mod boss_landmark_tests {
    use super::*;

    fn lake() -> Lake {
        Lake { x: 0, z: 0, radius: LAKE_RADIUS, phase: 3, from: (5, 5) }
    }

    #[test]
    fn the_door_is_locked_exactly_where_it_would_be_open() {
        let l = lake();
        let mut shut = 0;
        for dx in -TOWER_REACH..=TOWER_REACH {
            for dz in -TOWER_REACH..=TOWER_REACH {
                for dy in 0..TOWER_HEIGHT {
                    let (open, closed) = (l.tower_cell(dx, dz, dy, false, false), l.tower_cell(dx, dz, dy, true, false));
                    if open == closed {
                        continue;
                    }
                    shut += 1;
                    assert_eq!(open, Some(Block::Air));
                    assert!(matches!(closed, Some(Block::TowerDoor) | Some(Block::TowerDoorTop)));
                    assert!(dz == TOWER_HALF && dx == 0 && dy <= 1, "only the door: {dx},{dz},{dy}");
                }
            }
        }
        assert_eq!(shut, 2, "one block wide and two high");
    }

    #[test]
    fn a_lakes_tower_stays_locked_until_its_own_key_has_been_used_and_its_monster_is_a_separate_matter() {
        let mut marks = Landmarks::default();
        let other = Lake { x: 500, ..lake() };
        assert!(marks.tower_locked(&lake()) && marks.tower_locked(&other));
        marks.towers_unlocked.push((0, 0));
        assert!(!marks.tower_locked(&lake()) && marks.tower_locked(&other));
        assert!(marks.is_unlocked(0, 0) && !marks.is_unlocked(500, 0));
        let small = Lake { radius: 20, ..lake() };
        assert!(!marks.tower_locked(&small), "a lake with no tower has no door");
        // Killing the monster does not open the door, and the key does not kill the monster.
        assert!(marks.monster_alive(&lake()) && marks.monster_alive(&other));
        marks.monsters_slain.push((500, 0));
        assert!(marks.monster_alive(&lake()) && !marks.monster_alive(&other) && marks.tower_locked(&other));
        assert!(!marks.monster_alive(&small));
    }

    #[test]
    fn the_skylands_remember_the_islands_walked_on_and_which_one_has_the_tower() {
        let mut marks = Landmarks::default();
        assert!(marks.note_sky_visit((1, 0)) && marks.note_sky_visit((0, 1)));
        assert!(!marks.note_sky_visit((1, 0)), "an island counts once");
        assert_eq!(marks.sky_visited.len(), 2);
        assert_eq!(marks.sky_tower, None);
    }

    #[test]
    fn the_summit_is_a_flat_arena_that_can_still_be_climbed_to() {
        let m = Mountain { x: 0, z: 0, radius: MOUNTAIN_RADIUS, height: MOUNTAIN_HEIGHT.min(MOUNTAIN_MAX_HEIGHT), phase: 5 };
        let r = SUMMIT_PLATEAU_RADIUS as i32;
        for (dx, dz) in [(0, 0), (r, 0), (0, -r), (-9, 9)] {
            assert_eq!(m.rise_at(dx, dz), m.height);
        }
        for angle in 0..72 {
            let a = angle as f32 * std::f32::consts::TAU / 72.0;
            let at = |d: f32| m.rise_at((a.cos() * d).round() as i32, (a.sin() * d).round() as i32);
            assert!((at(SUMMIT_PLATEAU_RADIUS + 1.0) - m.height).abs() <= 1, "the edge of the arena is within a block of its floor");
        }
        assert_eq!(Landmarks { mountains: vec![m], ..Default::default() }.summit(), Some((0, 0)));
    }
}
