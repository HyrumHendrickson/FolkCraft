//! The mineshafts under the mountain.
//!
//! Five entrances stand at the foot of the mountain, spread round it. Each is a stair that goes down into the
//! hillside and then a long shaft that runs towards the heart of the mountain, climbing one block for every two it
//! goes, so it can be walked up by hopping. Three rings of tunnel (`MINE_RING_SHARES`) run round the mountain and cross
//! every shaft, with a chamber at each crossing, so all the shafts join up under the mountain; the rings lie higher
//! the nearer the summit, and the highest are well over a hundred blocks up.
//!
//! The network is a pure function of the mountain, so every chunk can carve its own share (`carve`) without asking its
//! neighbours. It is worked out once per mountain and kept (`network`). A tunnel is `MINE_HEIGHT` high and about
//! three wide, always `MINE_COVER` blocks under the surface once it is inside the mountain; only the entrance stair is
//! open to the sky.

use std::cell::RefCell;
use std::rc::Rc;

use crate::block::Block;
use crate::chunk::idx;
use crate::config::*;
use crate::landmarks::Mountain;
use crate::rng::hash3;

/// A straight piece of tunnel: the middles of its floor at each end, as (x, y of the lowest air cell, z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub a: (f32, f32, f32),
    pub b: (f32, f32, f32),
}

/// One entrance: the column where the stair begins on the slope, and the direction in (unit x, z) it leads inwards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entrance {
    pub x: i32,
    pub z: i32,
    pub dir: (f32, f32),
}

/// Everything under the mountain: the tunnels, the chambers at the crossings and the timber frames at the entrances.
#[derive(Clone, Debug)]
pub struct Network {
    /// Read by the tests (and kept for anything that wants to point at the entrances).
    #[allow(dead_code)]
    pub entrances: Vec<Entrance>,
    /// The tunnels of the shafts, in order from each entrance inwards (`shaft_spans`), then the rings.
    pub spans: Vec<Span>,
    /// Chambers: (x, z, y of the floor).
    pub rooms: Vec<(i32, i32, i32)>,
    /// Cells of timber over each entrance (posts and a lintel).
    pub frames: Vec<(i32, i32, i32)>,
    /// How many of `spans` belong to the shafts; the rest are rings. Read by the tests.
    #[allow(dead_code)]
    pub shaft_spans: usize,
}

fn surface(m: &Mountain, x: i32, z: i32) -> i32 {
    GROUND_TOP + m.rise_at(x, z)
}

/// The column at `share` of the way from the summit to the foot in the direction `angle`.
fn point_at(m: &Mountain, angle: f32, share: f32) -> (f32, f32) {
    let reach = share * m.foot_at(angle);
    (m.x as f32 + angle.cos() * reach, m.z as f32 + angle.sin() * reach)
}

/// The lowest surface within a block or two of the column (x, z): where the roof of a tunnel must stay under.
fn low_surface(m: &Mountain, x: f32, z: f32) -> i32 {
    let (cx, cz) = (x.floor() as i32, z.floor() as i32);
    let mut low = i32::MAX;
    for (dx, dz) in [(0, 0), (2, 0), (-2, 0), (0, 2), (0, -2)] {
        low = low.min(surface(m, cx + dx, cz + dz));
    }
    low
}

/// The highest floor a tunnel may have at (x, z): its roof and the cover over it stay under the surface.
fn floor_cap(m: &Mountain, x: f32, z: f32) -> i32 {
    low_surface(m, x, z) - MINE_COVER - (MINE_HEIGHT - 1)
}

struct Shaft {
    points: Vec<(f32, f32)>,
    /// Blocks walked from the entrance to each point.
    dist: Vec<f32>,
    /// Index of the point where each ring crosses.
    crossing: [usize; 3],
    ground: i32,
}

fn build_shaft(m: &Mountain, angle: f32) -> Shaft {
    // Start where the slope has risen at least two blocks: dry, and clear of any river.
    let mut share = 1.0_f32;
    while share > 0.3 {
        let (x, z) = point_at(m, angle, share);
        if m.rise_at(x.floor() as i32, z.floor() as i32) >= 2 {
            break;
        }
        share -= 0.002;
    }
    let end = MINE_RING_SHARES[2] - 0.05;
    let length = (share - end) * m.foot_at(angle);
    let steps = (length / 1.5).ceil().max(2.0) as usize;
    let mut points = Vec::with_capacity(steps + 1);
    let mut dist = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        points.push(point_at(m, angle, share + (end - share) * t));
        dist.push(i as f32 * length / steps as f32);
    }
    let crossing = MINE_RING_SHARES.map(|ring| {
        let t = ((share - ring) / (share - end)).clamp(0.0, 1.0);
        (t * steps as f32).round() as usize
    });
    let (sx, sz) = points[0];
    Shaft { points, dist, crossing, ground: surface(m, sx.floor() as i32, sz.floor() as i32) }
}

/// The floor of shaft point `j`: the stair down from the ground at the entrance, or the climb up to the next ring's
/// height, whichever is higher, and never below the lowest layer of dirt.
fn shaft_floor(shaft: &Shaft, heights: &[i32; 3], j: usize) -> f32 {
    let down = shaft.ground as f32 + 1.0 - MINE_SLOPE * shaft.dist[j];
    let mut floor = down.max(1.0);
    for k in 0..3 {
        if j <= shaft.crossing[k] {
            floor = floor.max(heights[k] as f32 - MINE_SLOPE * (shaft.dist[shaft.crossing[k]] - shaft.dist[j]));
        } else {
            // Past a ring the shaft never drops below it.
            floor = floor.max(heights[k] as f32);
        }
    }
    floor
}

fn build(m: &Mountain) -> Network {
    let axis = m.phase as f32 * 0.1;
    let angles: Vec<f32> = (0..MINE_ENTRANCES)
        .map(|i| {
            let jitter = (hash3(m.phase as i64, m.x, m.z, i as i32) % 1000) as f32 / 1000.0 - 0.5;
            axis + 0.4 + i as f32 * std::f32::consts::TAU / MINE_ENTRANCES as f32 + jitter * 0.4
        })
        .collect();
    let shafts: Vec<Shaft> = angles.iter().map(|&a| build_shaft(m, a)).collect();

    // The height of each ring: as high as the lowest ground along it allows, and no higher than every shaft can climb.
    let ring_samples = 96;
    let ring_cap: Vec<i32> = MINE_RING_SHARES
        .iter()
        .map(|&share| {
            (0..ring_samples)
                .map(|i| {
                    let (x, z) = point_at(m, i as f32 / ring_samples as f32 * std::f32::consts::TAU, share);
                    floor_cap(m, x, z)
                })
                .min()
                .unwrap_or(1)
        })
        .collect();
    let mut heights = [1; 3];
    for k in 0..3 {
        let mut h = ring_cap[k].max(1);
        if k > 0 {
            let gap = shafts.iter().map(|s| s.dist[s.crossing[k]] - s.dist[s.crossing[k - 1]]).fold(f32::MAX, f32::min);
            h = h.min(heights[k - 1] + (MINE_SLOPE * gap) as i32);
        } else {
            let gap = shafts.iter().map(|s| s.dist[s.crossing[0]]).fold(f32::MAX, f32::min);
            h = h.min(1 + (MINE_SLOPE * gap) as i32);
        }
        // Lower the ring until every shaft can climb to it without breaking through the surface.
        while h > heights.get(k.wrapping_sub(1)).copied().unwrap_or(1) {
            heights[k] = h;
            let fits = shafts.iter().all(|s| {
                let from = if k == 0 { 0 } else { s.crossing[k - 1] };
                (from..=s.crossing[k]).all(|j| {
                    let (x, z) = s.points[j];
                    let up = h as f32 - MINE_SLOPE * (s.dist[s.crossing[k]] - s.dist[j]);
                    up <= 1.0 || up <= floor_cap(m, x, z) as f32
                })
            });
            if fits {
                break;
            }
            h -= 1;
        }
        heights[k] = h.max(heights.get(k.wrapping_sub(1)).copied().unwrap_or(1));
    }

    let mut spans = Vec::new();
    let mut entrances = Vec::new();
    let mut rooms = Vec::new();
    let mut frames = Vec::new();
    for (shaft, &angle) in shafts.iter().zip(&angles) {
        let floors: Vec<f32> = (0..shaft.points.len()).map(|j| shaft_floor(shaft, &heights, j)).collect();
        for j in 1..shaft.points.len() {
            let (a, b) = (shaft.points[j - 1], shaft.points[j]);
            spans.push(Span { a: (a.0, floors[j - 1], a.1), b: (b.0, floors[j], b.1) });
        }
        let (sx, sz) = shaft.points[0];
        let dir = (-angle.cos(), -angle.sin());
        entrances.push(Entrance { x: sx.floor() as i32, z: sz.floor() as i32, dir });
        // A timber frame two blocks inside the mouth: two posts, and a lintel over the way in.
        let (ax, az) = (sx + dir.0 * 2.0, sz + dir.1 * 2.0);
        let side = (-dir.1, dir.0);
        let base = surface(m, ax.floor() as i32, az.floor() as i32) + 1;
        for l in -5..=5_i32 {
            let off = l as f32 * 0.5;
            let (cx, cz) = ((ax + side.0 * off).floor() as i32, (az + side.1 * off).floor() as i32);
            let ground = surface(m, cx, cz);
            if l.abs() == 5 {
                for y in ground + 1..=base + MINE_HEIGHT {
                    frames.push((cx, y, cz));
                }
            } else {
                frames.push((cx, base + MINE_HEIGHT, cz));
            }
        }
        for k in 0..3 {
            let (x, z) = shaft.points[shaft.crossing[k]];
            rooms.push((x.floor() as i32, z.floor() as i32, heights[k]));
        }
    }
    let shaft_spans = spans.len();
    for k in 0..3 {
        let share = MINE_RING_SHARES[k];
        let mut prev = point_at(m, 0.0, share);
        for i in 1..=ring_samples {
            let p = point_at(m, i as f32 / ring_samples as f32 * std::f32::consts::TAU, share);
            spans.push(Span { a: (prev.0, heights[k] as f32, prev.1), b: (p.0, heights[k] as f32, p.1) });
            prev = p;
        }
    }
    Network { entrances, spans, rooms, frames, shaft_spans }
}

type Key = (i32, i32, i32, i32, i32);

thread_local! {
    static CACHE: RefCell<Option<(Key, Rc<Network>)>> = const { RefCell::new(None) };
}

/// The mineshafts of a mountain, worked out once and kept.
pub fn network(m: &Mountain) -> Rc<Network> {
    let key = (m.x, m.z, m.radius, m.height, m.phase);
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((k, n)) = cache.as_ref() {
            if *k == key {
                return n.clone();
            }
        }
        let n = Rc::new(build(m));
        *cache = Some((key, n.clone()));
        n
    })
}

/// Carve the share of the mineshafts that lies in chunk (cx, cz) out of `blocks` (dirt and snow become air; the stone
/// floor of the world never does). Returns, for every column of the chunk (`lz * CHUNK_SIZE + lx`), whether the carving
/// came within a few blocks of the surface there, so that nothing is planted over an open stair.
pub fn carve(m: &Mountain, cx: i32, cz: i32, blocks: &mut [Block], tops: &[i32]) -> Vec<bool> {
    let net = network(m);
    let mut open = vec![false; (CHUNK_SIZE * CHUNK_SIZE) as usize];
    let (x0, z0) = (cx * CHUNK_SIZE, cz * CHUNK_SIZE);
    let mut dig = |blocks: &mut [Block], x: i32, y: i32, z: i32| {
        let (lx, lz) = (x - x0, z - z0);
        if !(0..CHUNK_SIZE).contains(&lx) || !(0..CHUNK_SIZE).contains(&lz) || !(1..WORLD_HEIGHT).contains(&y) {
            return;
        }
        let i = idx(lx, y, lz);
        if matches!(blocks[i], Block::Dirt | Block::Snow) {
            blocks[i] = Block::Air;
            if y >= tops[(lz * CHUNK_SIZE + lx) as usize] - 3 {
                open[(lz * CHUNK_SIZE + lx) as usize] = true;
            }
        }
    };
    let reach = MINE_HALF_WIDTH + 1.0;
    for span in &net.spans {
        let (lo_x, hi_x) = (span.a.0.min(span.b.0) - reach, span.a.0.max(span.b.0) + reach);
        let (lo_z, hi_z) = (span.a.2.min(span.b.2) - reach, span.a.2.max(span.b.2) + reach);
        if hi_x < x0 as f32 || lo_x > (x0 + CHUNK_SIZE) as f32 || hi_z < z0 as f32 || lo_z > (z0 + CHUNK_SIZE) as f32 {
            continue;
        }
        let (dx, dy, dz) = (span.b.0 - span.a.0, span.b.1 - span.a.1, span.b.2 - span.a.2);
        let steps = ((dx * dx + dz * dz).sqrt() / 0.4).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (px, py, pz) = (span.a.0 + dx * t, span.a.1 + dy * t, span.a.2 + dz * t);
            let floor = (py + 0.001).floor() as i32;
            for ox in -2..=2 {
                for oz in -2..=2 {
                    let (x, z) = (px.floor() as i32 + ox, pz.floor() as i32 + oz);
                    let (ex, ez) = (x as f32 + 0.5 - px, z as f32 + 0.5 - pz);
                    if ex * ex + ez * ez > MINE_HALF_WIDTH * MINE_HALF_WIDTH {
                        continue;
                    }
                    for y in floor..floor + MINE_HEIGHT {
                        dig(blocks, x, y, z);
                    }
                }
            }
        }
    }
    for &(rx, rz, floor) in &net.rooms {
        if (rx + MINE_ROOM_HALF < x0) || (rx - MINE_ROOM_HALF >= x0 + CHUNK_SIZE) || (rz + MINE_ROOM_HALF < z0) || (rz - MINE_ROOM_HALF >= z0 + CHUNK_SIZE) {
            continue;
        }
        for x in rx - MINE_ROOM_HALF..=rx + MINE_ROOM_HALF {
            for z in rz - MINE_ROOM_HALF..=rz + MINE_ROOM_HALF {
                for y in floor..floor + MINE_ROOM_HEIGHT {
                    dig(blocks, x, y, z);
                }
            }
        }
    }
    open
}

/// Put up the timber frames over the entrances that lie in chunk (cx, cz).
pub fn frame(m: &Mountain, cx: i32, cz: i32, blocks: &mut [Block]) {
    let net = network(m);
    for &(x, y, z) in &net.frames {
        let (lx, lz) = (x - cx * CHUNK_SIZE, z - cz * CHUNK_SIZE);
        if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&lz) && (1..WORLD_HEIGHT).contains(&y) {
            let i = idx(lx, y, lz);
            if !blocks[i].opaque() {
                blocks[i] = Block::Planks;
            }
        }
    }
}

/// Make the mines a salt mine and leave the silver keys in them, in the share of the mountain that lies in chunk
/// (cx, cz): pink salt crystals stud the dirt of the walls, floors and roofs that face a tunnel (never within three blocks of
/// the surface, where the entrance stairs are), and a silver key lies on a stand in the middle of the chamber where the
/// middle ring crosses each of the shafts in `MINE_KEY_SHAFTS`. `tops` is the height of the ground in each column.
pub fn furnish(m: &Mountain, cx: i32, cz: i32, blocks: &mut [Block], tops: &[i32]) {
    let net = network(m);
    let (x0, z0) = (cx * CHUNK_SIZE, cz * CHUNK_SIZE);
    let open = |blocks: &[Block], lx: i32, y: i32, lz: i32| {
        (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&lz) && (1..WORLD_HEIGHT).contains(&y) && blocks[idx(lx, y, lz)] == Block::Air
    };
    // Salt crystals are rare, and stand only deep inside the mountain, on the floors of the tunnels.
    let mut crystals = Vec::new();
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let top = tops[(lz * CHUNK_SIZE + lx) as usize];
            // Only chunks the tunnels can reach need looking at.
            if m.rise_at(x0 + lx, z0 + lz) == 0 {
                continue;
            }
            let (dx, dz) = ((x0 + lx - m.x) as f32, (z0 + lz - m.z) as f32);
            if (dx * dx + dz * dz).sqrt() > m.radius as f32 * MINE_SALT_MAX_SHARE {
                continue;
            }
            for y in 2..(top - 3).min(WORLD_HEIGHT) {
                if open(blocks, lx, y, lz) && !open(blocks, lx, y - 1, lz) && open(blocks, lx, y + 1, lz) && hash3(m.phase as i64 ^ 0x5A17, x0 + lx, z0 + lz, y) % 1000 < MINE_SALT_FLOOR_PER_MILLE {
                    crystals.push((lx, y, lz));
                }
            }
        }
    }
    for (lx, y, lz) in crystals {
        blocks[idx(lx, y, lz)] = Block::PinkSalt;
    }
    for &shaft in &MINE_KEY_SHAFTS {
        if let Some(&(rx, rz, floor)) = net.rooms.get(shaft * 3 + MINE_KEY_RING) {
            let (lx, lz) = (rx - x0, rz - z0);
            if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&lz) && (1..WORLD_HEIGHT).contains(&floor) {
                blocks[idx(lx, floor, lz)] = Block::KeyStand;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mountain() -> Mountain {
        Mountain { x: 0, z: 0, radius: MOUNTAIN_RADIUS, height: MOUNTAIN_HEIGHT, phase: 3 }
    }

    #[test]
    fn five_entrances_stand_apart_at_the_foot_of_the_mountain() {
        let m = mountain();
        let net = network(&m);
        assert_eq!(net.entrances.len(), 5);
        for (i, a) in net.entrances.iter().enumerate() {
            let (dx, dz) = (a.x as f32 - m.x as f32, a.z as f32 - m.z as f32);
            let share = (dx * dx + dz * dz).sqrt() / m.foot_at(dz.atan2(dx));
            assert!(share > 0.85 && share <= 1.0, "entrance {i} is at {share} of the way to the foot");
            assert!(m.rise_at(a.x, a.z) >= 2, "on the slope, clear of water");
            for b in &net.entrances[i + 1..] {
                assert!((a.x - b.x).abs().max((a.z - b.z).abs()) > 100, "{a:?} and {b:?} are different entrances");
            }
        }
    }

    #[test]
    fn the_shafts_climb_gently_stay_under_the_surface_and_go_up_high() {
        let m = mountain();
        let net = network(&m);
        let mut highest = 0.0_f32;
        for span in &net.spans {
            let run = ((span.b.0 - span.a.0).powi(2) + (span.b.2 - span.a.2).powi(2)).sqrt();
            assert!((span.b.1 - span.a.1).abs() <= MINE_SLOPE * run + 0.01 + if run < 1e-3 { 1.0 } else { 0.0 }, "too steep: {span:?}");
            highest = highest.max(span.a.1).max(span.b.1);
            for p in [span.a, span.b] {
                assert!(p.1 >= 1.0, "above the stone floor");
                let roof = p.1 as i32 + MINE_HEIGHT - 1;
                let ground = surface(&m, p.0.floor() as i32, p.2.floor() as i32);
                // Only the entrance stair, near the surface, may be open to the sky.
                if p.1 > 1.0 + 0.01 && roof >= ground - 3 {
                    let near_entrance = net.entrances.iter().any(|e| ((p.0 - e.x as f32).powi(2) + (p.2 - e.z as f32).powi(2)).sqrt() < 20.0);
                    assert!(near_entrance, "a tunnel breaks the surface at {p:?} (ground {ground})");
                }
            }
        }
        assert!(highest >= 60.0, "the shafts climb to {highest}");
    }

    #[test]
    fn the_rings_join_every_shaft_with_a_chamber_at_each_crossing() {
        let m = mountain();
        let net = network(&m);
        assert_eq!(net.rooms.len(), 15);
        let ring_start = net.shaft_spans;
        assert!(net.spans.len() > ring_start + 3 * 90);
        for &(rx, rz, y) in &net.rooms {
            let touches_ring = net.spans[ring_start..].iter().any(|s| {
                (s.a.1 - y as f32).abs() < 1.0 && ((s.a.0 - rx as f32).powi(2) + (s.a.2 - rz as f32).powi(2)).sqrt() < 12.0
            });
            assert!(touches_ring, "chamber at {rx},{rz},{y} is on a ring");
        }
    }

    #[test]
    fn carving_makes_a_walkable_hollow_in_the_dirt_and_leaves_the_stone_floor() {
        let m = mountain();
        let net = network(&m);
        let e = net.entrances[0];
        let (cx, cz) = (e.x.div_euclid(CHUNK_SIZE), e.z.div_euclid(CHUNK_SIZE));
        let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
        let mut tops = vec![0; (CHUNK_SIZE * CHUNK_SIZE) as usize];
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let top = surface(&m, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                tops[(lz * CHUNK_SIZE + lx) as usize] = top;
                blocks[idx(lx, 0, lz)] = Block::Stone;
                for y in 1..=top {
                    blocks[idx(lx, y, lz)] = Block::Dirt;
                }
            }
        }
        let solid_before = blocks.iter().filter(|b| **b == Block::Dirt).count();
        let open = carve(&m, cx, cz, &mut blocks, &tops);
        let solid_after = blocks.iter().filter(|b| **b == Block::Dirt).count();
        assert!(solid_after < solid_before, "dirt was dug out");
        assert!(open.iter().any(|&o| o), "the stair is open near the surface");
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                assert_eq!(blocks[idx(lx, 0, lz)], Block::Stone, "the stone floor stays");
            }
        }
        // Somewhere in the chunk a person can stand: two cells of air over a solid cell.
        let mut standable = false;
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                for y in 1..40 {
                    if blocks[idx(lx, y, lz)] != Block::Air && blocks[idx(lx, y + 1, lz)] == Block::Air && blocks[idx(lx, y + 2, lz)] == Block::Air && blocks[idx(lx, y + 3, lz)] == Block::Air {
                        standable = true;
                    }
                }
            }
        }
        assert!(standable);
    }

    #[test]
    fn a_walk_from_an_entrance_can_climb_to_the_highest_ring_by_steps_of_one_block() {
        let m = mountain();
        let net = network(&m);
        // Follow the first shaft until the next one begins: floors never jump more than a block between neighbours.
        let mut last = net.spans[0].a.1;
        let mut end = net.spans[0].a;
        for s in &net.spans[..net.shaft_spans] {
            if s.a != end {
                break;
            }
            assert!((s.b.1 - last).abs() <= 1.5, "a step of {} blocks", s.b.1 - last);
            last = s.b.1;
            end = s.b;
        }
        assert!(last > 20.0, "it has climbed to {last}");
    }

    /// A chunk of dirt over the stone floor, carved and furnished the way `terrain` does it.
    fn furnished_chunk(m: &Mountain, cx: i32, cz: i32) -> Vec<Block> {
        let mut blocks = vec![Block::Air; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
        let mut tops = vec![0; (CHUNK_SIZE * CHUNK_SIZE) as usize];
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let top = surface(m, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                tops[(lz * CHUNK_SIZE + lx) as usize] = top;
                blocks[idx(lx, 0, lz)] = Block::Stone;
                for y in 1..=top {
                    blocks[idx(lx, y, lz)] = Block::Dirt;
                }
            }
        }
        carve(m, cx, cz, &mut blocks, &tops);
        furnish(m, cx, cz, &mut blocks, &tops);
        blocks
    }

    #[test]
    fn pink_salt_crystals_stand_rarely_on_tunnel_floors_deep_in_the_mountain_and_nowhere_else() {
        let m = mountain();
        let deep = (m.radius as f32 * MINE_SALT_MAX_SHARE) as i32;
        let (mut crystals, mut floors, mut outer) = (0, 0, 0);
        for cz in (-deep).div_euclid(CHUNK_SIZE)..=deep.div_euclid(CHUNK_SIZE) {
            for cx in (-deep).div_euclid(CHUNK_SIZE)..=deep.div_euclid(CHUNK_SIZE) {
                let blocks = furnished_chunk(&m, cx, cz);
                let at = |lx: i32, y: i32, lz: i32| blocks[idx(lx, y, lz)];
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                        let top = surface(&m, wx, wz);
                        for y in 2..WORLD_HEIGHT - 1 {
                            if at(lx, y, lz) == Block::Air && at(lx, y - 1, lz) != Block::Air {
                                floors += 1;
                            }
                            if at(lx, y, lz) != Block::PinkSalt {
                                continue;
                            }
                            crystals += 1;
                            assert!(at(lx, y - 1, lz) != Block::Air && at(lx, y + 1, lz) == Block::Air, "a crystal stands on a floor at {wx},{y},{wz}");
                            assert!(y < top - 3, "a crystal near the surface at {wx},{y},{wz}");
                            let r = ((wx - m.x) as f32).hypot((wz - m.z) as f32);
                            assert!(r <= m.radius as f32 * MINE_SALT_MAX_SHARE + 1.0, "a crystal not deep in the mountain: {r}");
                        }
                    }
                }
            }
        }
        assert!(crystals >= 3, "some crystals: {crystals}");
        assert!((crystals as f32) < floors as f32 * 0.03, "they are rare: {crystals} on {floors} floor cells");
        // Out at the first ring (near the foot) there are none.
        let (rx, rz, _) = network(&m).rooms[MINE_KEY_SHAFTS[0] * 3];
        let blocks = furnished_chunk(&m, rx.div_euclid(CHUNK_SIZE), rz.div_euclid(CHUNK_SIZE));
        outer += blocks.iter().filter(|b| **b == Block::PinkSalt).count();
        assert_eq!(outer, 0, "none in the outer ring");
    }

    #[test]
    fn three_silver_keys_lie_on_stands_in_three_chambers_of_the_middle_ring() {
        let m = mountain();
        let net = network(&m);
        assert_eq!(MINE_KEY_SHAFTS.len(), 3);
        for &shaft in &MINE_KEY_SHAFTS {
            let (rx, rz, floor) = net.rooms[shaft * 3 + MINE_KEY_RING];
            let (cx, cz) = (rx.div_euclid(CHUNK_SIZE), rz.div_euclid(CHUNK_SIZE));
            let blocks = furnished_chunk(&m, cx, cz);
            let (lx, lz) = (rx - cx * CHUNK_SIZE, rz - cz * CHUNK_SIZE);
            assert_eq!(blocks[idx(lx, floor, lz)], Block::KeyStand, "shaft {shaft}");
            assert_eq!(blocks[idx(lx, floor + 1, lz)], Block::Air, "room to stand beside it");
        }
    }
}
