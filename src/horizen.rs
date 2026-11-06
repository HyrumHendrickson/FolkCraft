//! The Horizen: a second dimension, reached by falling into the well. It is a cave.
//!
//! A dimension is a world of its own. The Horizen shares nothing with the overworld but the player and what they
//! carry: its land, its edits (blocks put down, fires lit) and its items on the ground are separate, and none of the
//! overworld's creatures live there. It is a solid mass of stone with a flat floor at the height of the overworld's
//! plain, hollowed in three parts that are one connected way:
//!
//! - the **arrival chamber**, a dome round the arrival pool (`HORIZEN_SPAWN`),
//! - a winding **tunnel** from it to
//! - the one **great cavern**, a very large, ragged-edged hall far away, with the silver tower at its middle once a peep
//!   stone has brought the tower out (`Game::try_show_horizen_tower`).
//!
//! Small ponds lie on the floor wherever there is floor. Nothing can be dug (stone and dirt are unbreakable), so the cave
//! is exactly as generated. The Horizen has its own ordinary day/night state, but there is no sky to see it by; the
//! compass still points back to the arrival pool, and a peep stone leads along the tunnel to the tower.
//!
//! Like the overworld, the cave is a pure function of the world seed.

use crate::block::Block;
use crate::chunk::idx;
use crate::config::*;
use crate::landmarks::Landmarks;
use crate::rng::hash3;
use crate::tower::{self, Relic, Tower};

/// Which of the three worlds a `World` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dimension {
    /// The ordinary world: forest, hills, rivers and the landmarks the player brings into it.
    #[default]
    Overworld,
    /// The cave behind the well: a winding tunnel to one very large cavern with a silver tower in it.
    Horizen,
    /// The floating islands and their wooden bridges, reached by the cyclone on the mountain's summit (`skylands.rs`).
    Skylands,
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

/// y of the highest block of the ground in the column (wx, wz): the floor of the cave is flat, and one block lower
/// under a pond.
pub fn ground_top(seed: i64, wx: i32, wz: i32) -> i32 {
    GROUND_TOP - if is_pond(seed, wx, wz) { RIVER_DEPTH } else { 0 }
}

// --- The shape of the cave -------------------------------------------------------------------

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The middle of the arrival point's column, in world coordinates.
pub fn spawn_centre() -> (f32, f32) {
    (HORIZEN_SPAWN.0 as f32 + 0.5, HORIZEN_SPAWN.1 as f32 + 0.5)
}

/// The unit direction in which the tunnel runs from the arrival point to the great cavern.
fn axis(seed: i64) -> (f32, f32) {
    let angle = (hash3(seed, 0, 0, 0x4C41) % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
    (angle.cos(), angle.sin())
}

/// Where along the straight line (and how far to either side of it) the tunnel's winding starts.
fn wave_phase(seed: i64) -> f32 {
    (hash3(seed, 1, 0, 0x4C41) % 3600) as f32 / 3600.0 * std::f32::consts::TAU
}

/// The column at the middle of the great cavern, where the silver tower stands.
pub fn cavern_centre(seed: i64) -> (i32, i32) {
    let (sx, sz) = spawn_centre();
    let (ux, uz) = axis(seed);
    ((sx + ux * HORIZEN_CAVERN_DISTANCE).floor() as i32, (sz + uz * HORIZEN_CAVERN_DISTANCE).floor() as i32)
}

/// How far the middle of the tunnel lies to the left of the straight line at distance `t` along it: nothing at the
/// arrival chamber, then a smooth winding.
fn tunnel_offset(seed: i64, t: f32) -> f32 {
    let wave = (t / HORIZEN_TUNNEL_WAVELENGTH * std::f32::consts::TAU + wave_phase(seed)).sin();
    HORIZEN_TUNNEL_WIGGLE * wave * smooth(t / 40.0) * smooth((HORIZEN_CAVERN_DISTANCE - t) / 40.0)
}

/// The middle of the tunnel at distance `t` along it from the arrival point (0 to `HORIZEN_CAVERN_DISTANCE`), as (x, z).
pub fn tunnel_point(seed: i64, t: f32) -> (f32, f32) {
    let (sx, sz) = spawn_centre();
    let (ux, uz) = axis(seed);
    let off = tunnel_offset(seed, t);
    (sx + ux * t - uz * off, sz + uz * t + ux * off)
}

/// The air above the floor of a dome of the given radius and middle height, `dist` blocks from its middle.
fn dome_air(dist: f32, radius: f32, height: i32) -> i32 {
    if dist >= radius {
        return 0;
    }
    (height as f32 * (1.0 - (dist / radius).powi(2))).floor() as i32
}

fn chamber_air(wx: i32, wz: i32) -> i32 {
    let (sx, sz) = spawn_centre();
    dome_air(((wx as f32 + 0.5 - sx).powi(2) + (wz as f32 + 0.5 - sz).powi(2)).sqrt(), HORIZEN_CHAMBER_RADIUS, HORIZEN_CHAMBER_HEIGHT)
}

fn tunnel_air(seed: i64, wx: i32, wz: i32) -> i32 {
    let (sx, sz) = spawn_centre();
    let (ux, uz) = axis(seed);
    let (vx, vz) = (wx as f32 + 0.5 - sx, wz as f32 + 0.5 - sz);
    let t = vx * ux + vz * uz;
    if !(0.0..=HORIZEN_CAVERN_DISTANCE).contains(&t) {
        return 0;
    }
    let side = -vx * uz + vz * ux;
    let off = (side - tunnel_offset(seed, t)).abs();
    if off >= HORIZEN_TUNNEL_RADIUS {
        return 0;
    }
    HORIZEN_MIN_AIR + ((HORIZEN_TUNNEL_HEIGHT - HORIZEN_MIN_AIR) as f32 * (1.0 - (off / HORIZEN_TUNNEL_RADIUS).powi(2))).floor() as i32
}

/// The radius of the great cavern in the direction `angle` from its middle: ragged, not round.
fn cavern_radius(seed: i64, angle: f32) -> f32 {
    let (p1, p2) = ((hash3(seed, 2, 0, 0x4C41) % 628) as f32 / 100.0, (hash3(seed, 3, 0, 0x4C41) % 628) as f32 / 100.0);
    HORIZEN_CAVERN_RADIUS * (1.0 + HORIZEN_CAVERN_RAGGEDNESS * (0.6 * (3.0 * angle + p1).sin() + 0.4 * (5.0 * angle + p2).sin()))
}

fn cavern_air(seed: i64, wx: i32, wz: i32) -> i32 {
    let (cx, cz) = cavern_centre(seed);
    let (dx, dz) = (wx as f32 + 0.5 - (cx as f32 + 0.5), wz as f32 + 0.5 - (cz as f32 + 0.5));
    dome_air((dx * dx + dz * dz).sqrt(), cavern_radius(seed, dz.atan2(dx)), HORIZEN_CAVERN_HEIGHT)
}

/// One of the Horizen's caves: its square of the grid, its middle, its radius and the air above its middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cave {
    pub cell: (i32, i32),
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub height: i32,
    /// The great cavern with the tower (its shape is `cavern_air`, not a plain dome).
    pub great: bool,
}

fn cavern_cell(seed: i64) -> (i32, i32) {
    let (x, z) = cavern_centre(seed);
    (x.div_euclid(HORIZEN_CAVE_CELL), z.div_euclid(HORIZEN_CAVE_CELL))
}

/// The cave of the square (ci, cj) of the grid: the arrival chamber for the square of the arrival point, the great cavern
/// for the square it lies in, and for every other a dome somewhere in the square.
pub fn cave_in_cell(seed: i64, ci: i32, cj: i32) -> Cave {
    let (sx, sz) = spawn_centre();
    if (ci, cj) == (HORIZEN_SPAWN.0.div_euclid(HORIZEN_CAVE_CELL), HORIZEN_SPAWN.1.div_euclid(HORIZEN_CAVE_CELL)) {
        return Cave { cell: (ci, cj), x: sx, z: sz, radius: HORIZEN_CHAMBER_RADIUS, height: HORIZEN_CHAMBER_HEIGHT, great: false };
    }
    if (ci, cj) == cavern_cell(seed) {
        let (x, z) = cavern_centre(seed);
        return Cave { cell: (ci, cj), x: x as f32 + 0.5, z: z as f32 + 0.5, radius: HORIZEN_CAVERN_RADIUS, height: HORIZEN_CAVERN_HEIGHT, great: true };
    }
    let h = hash3(seed, ci, cj, 0x4341);
    let unit = |shift: u32| ((h >> shift) & 0xFFFF) as f32 / 65535.0;
    let margin = HORIZEN_CAVE_MAX_RADIUS + 4.0;
    let span = HORIZEN_CAVE_CELL as f32 - 2.0 * margin;
    Cave {
        cell: (ci, cj),
        x: (ci * HORIZEN_CAVE_CELL) as f32 + margin + unit(0) * span,
        z: (cj * HORIZEN_CAVE_CELL) as f32 + margin + unit(16) * span,
        radius: HORIZEN_CAVE_MIN_RADIUS + unit(32) * (HORIZEN_CAVE_MAX_RADIUS - HORIZEN_CAVE_MIN_RADIUS),
        height: HORIZEN_CAVE_MIN_HEIGHT + (unit(48) * (HORIZEN_CAVE_MAX_HEIGHT - HORIZEN_CAVE_MIN_HEIGHT + 1) as f32) as i32,
        great: false,
    }
}

/// The air over the column (wx, wz) from the dome of `cave` and the tunnels that join it to its east and south neighbours (and
/// from the neighbours that join it), i.e. everything of the cave network that can reach this square.
fn network_air(seed: i64, wx: i32, wz: i32) -> i32 {
    let (ci, cj) = (wx.div_euclid(HORIZEN_CAVE_CELL), wz.div_euclid(HORIZEN_CAVE_CELL));
    let p = (wx as f32 + 0.5, wz as f32 + 0.5);
    let own = cave_in_cell(seed, ci, cj);
    let mut air = if own.great { 0 } else { dome_air(((p.0 - own.x).powi(2) + (p.1 - own.z).powi(2)).sqrt(), own.radius, own.height) };
    for (a, b) in [((ci, cj), (ci + 1, cj)), ((ci, cj), (ci, cj + 1)), ((ci - 1, cj), (ci, cj)), ((ci, cj - 1), (ci, cj))] {
        let (ca, cb) = (cave_in_cell(seed, a.0, a.1), cave_in_cell(seed, b.0, b.1));
        let (ab, ap) = ((cb.x - ca.x, cb.z - ca.z), (p.0 - ca.x, p.1 - ca.z));
        let t = ((ap.0 * ab.0 + ap.1 * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1)).clamp(0.0, 1.0);
        let d = ((ap.0 - ab.0 * t).powi(2) + (ap.1 - ab.1 * t).powi(2)).sqrt();
        if d < HORIZEN_LINK_RADIUS {
            air = air.max(HORIZEN_MIN_AIR + ((HORIZEN_LINK_HEIGHT - HORIZEN_MIN_AIR) as f32 * (1.0 - (d / HORIZEN_LINK_RADIUS).powi(2))).floor() as i32);
        }
    }
    air
}

/// The cave the column (wx, wz) is in the heart of (within `HORIZEN_CAVE_ENTER_SHARE` of its radius, half that for the great
/// cavern), if any: what counts as having entered it.
pub fn cave_at(seed: i64, wx: i32, wz: i32) -> Option<Cave> {
    let cave = cave_in_cell(seed, wx.div_euclid(HORIZEN_CAVE_CELL), wz.div_euclid(HORIZEN_CAVE_CELL));
    let share = if cave.great { HORIZEN_CAVE_ENTER_SHARE / 2.0 } else { HORIZEN_CAVE_ENTER_SHARE };
    let dist = ((wx as f32 + 0.5 - cave.x).powi(2) + (wz as f32 + 0.5 - cave.z).powi(2)).sqrt();
    (dist <= cave.radius * share).then_some(cave)
}

/// How many blocks of air lie above the floor of the cave in the column (wx, wz): 0 where the rock is solid, otherwise at
/// least `HORIZEN_MIN_AIR`. The ceiling is rough, so it hangs down a little here and there.
pub fn air_height(seed: i64, wx: i32, wz: i32) -> i32 {
    let air = chamber_air(wx, wz).max(tunnel_air(seed, wx, wz)).max(cavern_air(seed, wx, wz)).max(network_air(seed, wx, wz));
    if air < HORIZEN_MIN_AIR {
        return 0;
    }
    let jag = (hash3(seed, wx, wz, 0x4A41) % (HORIZEN_CEILING_JAG as u64 + 1)) as i32;
    if air - jag >= HORIZEN_MIN_AIR { air - jag } else { air }
}

/// Whether the column (wx, wz) is open cave (floor with air above it) rather than solid rock.
pub fn is_open(seed: i64, wx: i32, wz: i32) -> bool {
    air_height(seed, wx, wz) > 0
}

/// Whether a pond covers the column (wx, wz): the arrival pool, or a pond on open floor (none within reach of the tower).
pub fn is_pond(seed: i64, wx: i32, wz: i32) -> bool {
    let (sx, sz) = spawn_centre();
    let dx = wx as f32 + 0.5 - sx;
    let dz = wz as f32 + 0.5 - sz;
    if dx * dx + dz * dz <= HORIZEN_ARRIVAL_POOL_RADIUS.powi(2) {
        return true;
    }
    let (tx, tz) = cavern_centre(seed);
    let reach = TOWER_REACH + TOWER_POND_MARGIN;
    if (wx - tx).abs() <= reach && (wz - tz).abs() <= reach {
        return false;
    }
    is_open(seed, wx, wz)
        && pond_in_cell(seed, wx.div_euclid(HORIZEN_POND_CELL), wz.div_euclid(HORIZEN_POND_CELL)).map_or(false, |p| p.contains(wx, wz))
}

/// The corners (x, z) of a ghostly trail through the tunnel for a player standing at `from`: the bends of the tunnel
/// ahead of them, which end in the great cavern. A player already in the cavern gets no bends.
pub fn route(seed: i64, from: (f32, f32)) -> Vec<(f32, f32)> {
    const STEP: f32 = 10.0;
    let (sx, sz) = spawn_centre();
    let (ux, uz) = axis(seed);
    let here = ((from.0 - sx) * ux + (from.1 - sz) * uz).clamp(0.0, HORIZEN_CAVERN_DISTANCE);
    let mut path = vec![from];
    let mut t = (here / STEP).floor() * STEP + STEP;
    while t < HORIZEN_CAVERN_DISTANCE - HORIZEN_CAVERN_RADIUS * 0.5 {
        path.push(tunnel_point(seed, t));
        t += STEP;
    }
    path
}

/// The blocks of chunk (cx, cz) of the Horizen: solid stone with the cave hollowed out of it (a dirt floor, water in
/// the ponds, air above) and the silver tower (with the Ancient Crown at the top) once a peep stone has brought it out.
/// No tree, plant, stone or animal.
pub fn generate_chunk(seed: i64, cx: i32, cz: i32, landmarks: &Landmarks) -> Vec<Block> {
    let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
            let air = air_height(seed, wx, wz);
            if air == 0 {
                for y in 0..=HORIZEN_ROOF_Y {
                    blocks[idx(lx, y, lz)] = Block::Stone;
                }
                continue;
            }
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
            for y in GROUND_TOP + 1 + air..=HORIZEN_ROOF_Y {
                blocks[idx(lx, y, lz)] = Block::Stone;
            }
        }
    }
    if let Some(Tower { x, z }) = landmarks.horizen_tower {
        let locked = !landmarks.is_unlocked(x, z);
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let (dx, dz) = (cx * CHUNK_SIZE + lx - x, cz * CHUNK_SIZE + lz - z);
                if !tower::near(dx, dz, 0) {
                    continue;
                }
                for dy in 0..TOWER_HEIGHT {
                    if let Some(block) = tower::tower_cell(dx, dz, dy, locked, Relic::AncientCrown) {
                        blocks[idx(lx, GROUND_TOP + 1 + dy, lz)] = block;
                    }
                }
            }
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn the_cave_is_a_pure_function_of_the_seed() {
        for (x, z) in [(0, 0), (-77, 31), (150, -250), (10, 10)] {
            assert_eq!(air_height(5, x, z), air_height(5, x, z));
            assert_eq!(is_pond(5, x, z), is_pond(5, x, z));
            assert!(ground_top(5, x, z) == GROUND_TOP || ground_top(5, x, z) == GROUND_TOP - RIVER_DEPTH);
        }
        assert_ne!(cavern_centre(1), cavern_centre(2), "another seed, another way to the cavern");
    }

    #[test]
    fn the_arrival_point_is_a_pool_in_a_chamber_with_room_round_it() {
        for seed in 0..40 {
            let (x, z) = HORIZEN_SPAWN;
            assert!(is_pond(seed, x, z), "seed {seed}: the arrival pool is missing");
            assert!(!is_pond(seed, x + 8, z), "seed {seed}: the pool is too large");
            assert!(air_height(seed, x, z) >= HORIZEN_CHAMBER_HEIGHT - HORIZEN_CEILING_JAG, "seed {seed}: a high dome over the pool");
        }
    }

    #[test]
    fn every_open_column_has_room_to_stand_and_the_rest_is_rock() {
        let seed = 7;
        let (mut open, mut solid) = (0, 0);
        for z in -80..260 {
            for x in -80..260 {
                match air_height(seed, x, z) {
                    0 => solid += 1,
                    a => {
                        open += 1;
                        assert!(a >= HORIZEN_MIN_AIR, "{a} blocks of air at {x},{z}");
                    }
                }
            }
        }
        assert!(open > 500 && solid > open, "a cave in the rock: {open} open, {solid} solid");
    }

    /// The columns reachable on foot from the arrival point (every open column is flat floor, ponds are only waded).
    fn reachable(seed: i64, limit: i32) -> HashSet<(i32, i32)> {
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([HORIZEN_SPAWN]);
        seen.insert(HORIZEN_SPAWN);
        while let Some((x, z)) = queue.pop_front() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (x + dx, z + dz);
                if (next.0 - HORIZEN_SPAWN.0).abs() > limit || (next.1 - HORIZEN_SPAWN.1).abs() > limit {
                    continue;
                }
                if is_open(seed, next.0, next.1) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        seen
    }

    #[test]
    fn the_tunnel_leads_on_foot_from_the_arrival_pool_to_the_great_cavern_for_every_seed() {
        for seed in 0..12 {
            let seen = reachable(seed, 260);
            let centre = cavern_centre(seed);
            assert!(seen.contains(&centre), "seed {seed}: the cavern's middle at {centre:?} cannot be reached");
            let wide = (HORIZEN_CAVERN_RADIUS * 0.5) as i32;
            for (dx, dz) in [(wide, 0), (-wide, 0), (0, wide), (0, -wide)] {
                assert!(seen.contains(&(centre.0 + dx, centre.1 + dz)), "seed {seed}: the cavern is wide open");
            }
        }
    }

    #[test]
    fn the_middle_of_the_tunnel_is_open_all_the_way() {
        for seed in 0..12 {
            let mut t = 0.0;
            while t <= HORIZEN_CAVERN_DISTANCE {
                let (x, z) = tunnel_point(seed, t);
                assert!(air_height(seed, x.floor() as i32, z.floor() as i32) >= HORIZEN_MIN_AIR, "seed {seed}: closed at {t}");
                t += 1.0;
            }
        }
    }

    #[test]
    fn the_great_cavern_is_very_large_and_tall_enough_for_the_tower() {
        for seed in 0..12 {
            let (cx, cz) = cavern_centre(seed);
            let open = (-80..80).flat_map(|dz| (-80..80).map(move |dx| (dx, dz))).filter(|&(dx, dz)| is_open(seed, cx + dx, cz + dz)).count();
            assert!(open > 9000, "seed {seed}: only {open} open columns near the cavern");
            for dz in -TOWER_REACH - 1..=TOWER_REACH + 1 {
                for dx in -TOWER_REACH - 1..=TOWER_REACH + 1 {
                    assert!(air_height(seed, cx + dx, cz + dz) > TOWER_HEIGHT + 4, "seed {seed}: the tower would touch the roof");
                    assert!(!is_pond(seed, cx + dx, cz + dz), "seed {seed}: the tower stands on dry floor");
                }
            }
            assert!(cx.abs().max(cz.abs()) > 100, "the cavern is a long way from the arrival point");
        }
    }

    #[test]
    fn there_are_small_ponds_on_the_floor_and_none_in_the_rock() {
        let seed = 9;
        let (cx, cz) = cavern_centre(seed);
        let (mut wet, mut floor) = (0, 0);
        for z in cz - 70..cz + 70 {
            for x in cx - 70..cx + 70 {
                if is_open(seed, x, z) {
                    floor += 1;
                    wet += is_pond(seed, x, z) as i32;
                } else {
                    assert!(!is_pond(seed, x, z), "no pond inside rock");
                }
            }
        }
        let share = wet as f32 / floor as f32;
        assert!(share > 0.02 && share < 0.35, "pond share {share}");
        for p in (-10..10).flat_map(|cz| (-10..10).map(move |cx| pond_in_cell(seed, cx, cz))).flatten() {
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
    fn the_route_follows_the_tunnel_to_the_cavern() {
        let seed = 4;
        let start = (HORIZEN_SPAWN.0 as f32 + 0.5, HORIZEN_SPAWN.1 as f32 + 0.5);
        let path = route(seed, start);
        assert_eq!(path[0], start);
        assert!(path.len() > 8, "the tunnel is long: {} corners", path.len());
        for &(x, z) in &path[1..] {
            assert!(is_open(seed, x.floor() as i32, z.floor() as i32), "a corner of the route in rock at {x},{z}");
        }
        let (cx, cz) = cavern_centre(seed);
        let inside = route(seed, (cx as f32, cz as f32));
        assert_eq!(inside.len(), 1, "a player in the cavern has no bends left");
    }

    #[test]
    fn a_tower_stands_in_the_cavern_with_the_crown_at_the_top() {
        let seed = 11;
        let (x, z) = cavern_centre(seed);
        let mut marks = Landmarks::default();
        marks.horizen_tower = Some(Tower { x, z });
        let (cx, cz) = (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        let (lx, lz) = (x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));
        let top = (TOWER_STOREYS - 1) * TOWER_STOREY;
        let blocks = generate_chunk(seed, cx, cz, &marks);
        assert_eq!(blocks[idx(lx, GROUND_TOP + 1 + top, lz)], Block::CrownPlinth);
        assert!(generate_chunk(seed, cx, cz, &Landmarks::default()).iter().all(|b| *b != Block::CrownPlinth));
        // The door is locked until it has been unlocked.
        let door = |m: &Landmarks| {
            let b = generate_chunk(seed, x.div_euclid(CHUNK_SIZE), (z + TOWER_HALF).div_euclid(CHUNK_SIZE), m);
            b[idx(lx, GROUND_TOP + 1, (z + TOWER_HALF).rem_euclid(CHUNK_SIZE))]
        };
        assert_eq!(door(&marks), Block::TowerDoor);
        marks.towers_unlocked.push((x, z));
        assert_eq!(door(&marks), Block::Air);
    }

    #[test]
    fn a_chunk_is_rock_with_a_dirt_floored_cave_in_it_and_nothing_stands_on_it() {
        let seed = 11;
        let (mut open, mut ponds) = (0, 0);
        for cz in -3..3 {
            for cx in -3..3 {
                let blocks = generate_chunk(seed, cx, cz, &Landmarks::default());
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        assert_eq!(blocks[idx(lx, 0, lz)], Block::Stone);
                        assert_eq!(blocks[idx(lx, HORIZEN_ROOF_Y, lz)], Block::Stone, "the roof is solid everywhere");
                        assert_eq!(blocks[idx(lx, HORIZEN_ROOF_Y + 1, lz)], Block::Air);
                        let air = air_height(seed, wx, wz);
                        if air == 0 {
                            assert!((0..=HORIZEN_ROOF_Y).all(|y| blocks[idx(lx, y, lz)] == Block::Stone), "solid rock at {wx},{wz}");
                            continue;
                        }
                        open += 1;
                        let wet = is_pond(seed, wx, wz);
                        ponds += wet as i32;
                        assert_eq!(blocks[idx(lx, GROUND_TOP, lz)], if wet { Block::Water } else { Block::Dirt });
                        assert_eq!(blocks[idx(lx, GROUND_TOP - 1, lz)], Block::Dirt);
                        for y in GROUND_TOP + 1..=GROUND_TOP + air {
                            assert_eq!(blocks[idx(lx, y, lz)], Block::Air, "air above the floor at {lx},{y},{lz}");
                        }
                        assert_eq!(blocks[idx(lx, GROUND_TOP + air + 1, lz)], Block::Stone, "the ceiling");
                    }
                }
            }
        }
        assert!(open > 0 && ponds > 0);
    }

    #[test]
    fn the_horizen_is_full_of_caves_and_every_one_can_be_reached_on_foot_from_the_arrival_chamber() {
        for seed in 0..6 {
            let seen = reachable(seed, 330);
            let mut caves = 0;
            for cj in -3..4 {
                for ci in -3..4 {
                    let cave = cave_in_cell(seed, ci, cj);
                    let (x, z) = (cave.x.floor() as i32, cave.z.floor() as i32);
                    assert!(is_open(seed, x, z), "seed {seed}: the cave of {ci},{cj} is open at its middle");
                    assert!(air_height(seed, x, z) >= HORIZEN_MIN_AIR);
                    if seen.contains(&(x, z)) {
                        caves += 1;
                    } else {
                        assert!((x - HORIZEN_SPAWN.0).abs() > 330 || (z - HORIZEN_SPAWN.1).abs() > 330, "seed {seed}: the cave of {ci},{cj} at {x},{z} is cut off");
                    }
                }
            }
            assert!(caves >= 12, "seed {seed}: only {caves} caves within reach");
        }
    }

    #[test]
    fn the_caves_differ_in_size_and_place_and_the_arrival_chamber_and_the_great_cavern_are_among_them() {
        let seed = 3;
        let spawn = cave_in_cell(seed, 0, 0);
        assert_eq!((spawn.x, spawn.z), spawn_centre());
        assert_eq!(spawn.radius, HORIZEN_CHAMBER_RADIUS);
        let great = (-4..6).flat_map(|j| (-4..6).map(move |i| cave_in_cell(seed, i, j))).filter(|c| c.great).collect::<Vec<_>>();
        assert_eq!(great.len(), 1, "one great cavern");
        let (cx, cz) = cavern_centre(seed);
        assert_eq!((great[0].x.floor() as i32, great[0].z.floor() as i32), (cx, cz));
        let others: Vec<Cave> = (-4..6).flat_map(|j| (-4..6).map(move |i| cave_in_cell(seed, i, j))).filter(|c| !c.great && c.cell != (0, 0)).collect();
        let radii: std::collections::HashSet<i32> = others.iter().map(|c| c.radius as i32).collect();
        assert!(radii.len() >= 4, "caves of many sizes: {radii:?}");
        assert!(others.iter().all(|c| c.radius >= HORIZEN_CAVE_MIN_RADIUS && c.radius <= HORIZEN_CAVE_MAX_RADIUS && c.height >= HORIZEN_CAVE_MIN_HEIGHT && c.height <= HORIZEN_CAVE_MAX_HEIGHT));
        assert!(great[0].radius > 3.0 * HORIZEN_CAVE_MAX_RADIUS, "the great cavern is far larger than any other");
    }

    #[test]
    fn a_cave_counts_as_entered_near_its_middle_only() {
        let seed = 5;
        let cave = cave_in_cell(seed, 2, 1);
        let (x, z) = (cave.x.floor() as i32, cave.z.floor() as i32);
        assert_eq!(cave_at(seed, x, z).map(|c| c.cell), Some((2, 1)));
        assert_eq!(cave_at(seed, x + (cave.radius * 0.9) as i32, z), None, "the rim is not the heart");
        assert_eq!(cave_at(seed, HORIZEN_SPAWN.0, HORIZEN_SPAWN.1).map(|c| c.cell), Some((0, 0)), "you arrive in the first cave");
    }
}
