//! Animated effects drawn on top of the static chunk meshes.

use macroquad::prelude::*;

use crate::block::Block;
use crate::config::*;
use crate::dropped::draw_box;
use crate::landmarks::Rainbow;
use crate::tornado::FireTornado;
use crate::world::{in_render_range, World};


/// Seconds of fuel left at which the flames start to shrink.
const FLAME_FADE_SECONDS: f32 = 8.0;

/// Draw flickering flames above every burning log, burning block of charcoal and lit forge in range.
/// Embers burn lower than a log; a forge worked with bellows glows brighter and burns hotter.
/// Call while the 3D camera is active. Flames die down as the fuel runs out.
pub fn draw_flames(world: &World, time: f32, player_chunk: (i32, i32)) {
    for (&(x, y, z), &remaining) in &world.fires {
        if !in_render_range(player_chunk, World::chunk_of(x, z)) {
            continue;
        }
        let block = world.block_at(x, y, z);
        let embers = matches!(block, Block::Embers | Block::ForgeLit);
        // Height of the coals in the cell: in the open they lie low, in a forge they sit in its opening.
        let coal_y = if block == Block::ForgeLit { 0.58 } else { 0.30 };
        let glowing = world.is_glowing((x, y, z));
        let fade = (remaining / FLAME_FADE_SECONDS).clamp(0.2, 1.0);
        let size = if embers { fade * if glowing { 0.8 } else { 0.5 } } else { fade };
        let phase = (x * 7 + z * 13) as f32;
        let flicker = |speed: f32, offset: f32| 0.5 + 0.5 * (time * speed + phase + offset).sin();
        // The block is centred in its cell; the flames rise from its top.
        let top = world.block_at(x, y, z).bounds().map_or(0.5, |(_, hi)| hi[1]);
        let base = vec3(x as f32 + 0.5, y as f32 + top, z as f32 + 0.5);
        if embers {
            // A glow over the coals: dull red, or yellow-white once worked with bellows.
            let pulse = 0.85 + 0.15 * flicker(6.0, 0.0);
            let color = if glowing {
                Color::new(1.0 * pulse, 0.82 * pulse, 0.38 * pulse, 1.0)
            } else {
                Color::new(0.78 * pulse, 0.22, 0.06, 1.0)
            };
            let half = if block == Block::ForgeLit { vec3(0.20, 0.05, 0.20) } else { vec3(0.37, 0.07, 0.31) };
            draw_box(vec3(base.x, y as f32 + coal_y, base.z), half, 0.0, color);
        }
        let flames = [
            // (offset x, offset z, half width, height, speed, colour)
            (0.00, 0.00, 0.17, 0.34, 9.0, Color::new(0.95, 0.35, 0.08, 1.0)),
            (0.10, -0.08, 0.11, 0.30, 12.0, Color::new(0.98, 0.62, 0.12, 1.0)),
            (-0.12, 0.08, 0.10, 0.26, 10.5, Color::new(0.90, 0.25, 0.06, 1.0)),
            (0.00, 0.02, 0.08, 0.40, 14.0, if glowing { Color::new(1.0, 0.97, 0.7, 1.0) } else { Color::new(1.00, 0.88, 0.35, 1.0) }),
        ];
        for (i, &(ox, oz, half, height, speed, color)) in flames.iter().enumerate() {
            let h = height * size * (0.65 + 0.7 * flicker(speed, i as f32 * 1.7));
            let centre = base + vec3(ox * size, h / 2.0, oz * size);
            draw_box(centre, vec3(half * size, h / 2.0, half * size), time * 1.3 + i as f32, color);
        }
    }
}

/// Draw whatever lies on burning blocks: meat darkening as it cooks, stone, pebble, tin or iron ore turning into
/// metal as it smelts. Call while the 3D camera is active.
pub fn draw_meat(world: &World, player_chunk: (i32, i32)) {
    for (&(x, y, z), piece) in &world.cooking {
        if !in_render_range(player_chunk, World::chunk_of(x, z)) {
            continue;
        }
        let raw = piece.raw.color();
        let done = piece.raw.processed().map_or(raw, |k| k.color());
        let t = piece.progress();
        let color = Color::new(raw.r + (done.r - raw.r) * t, raw.g + (done.g - raw.g) * t, raw.b + (done.b - raw.b) * t, 1.0);
        let top = world.block_at(x, y, z).bounds().map_or(0.5, |(_, hi)| hi[1]);
        let centre = vec3(x as f32 + 0.5, y as f32 + top + 0.07, z as f32 + 0.5);
        draw_box(centre, vec3(0.16, 0.07, 0.10), 0.5, color);
    }
}

/// Draw the tin pots standing on fires: the pot, the water in it (blue) and the meat or stew floating there.
/// Call while the 3D camera is active.
pub fn draw_pots(world: &World, player_chunk: (i32, i32)) {
    for (&(x, y, z), pot) in &world.pots {
        if !in_render_range(player_chunk, World::chunk_of(x, z)) {
            continue;
        }
        let top = world.block_at(x, y, z).bounds().map_or(0.5, |(_, hi)| hi[1]);
        let (cx, cz) = (x as f32 + 0.5, z as f32 + 0.5);
        let base = y as f32 + top;
        draw_box(vec3(cx, base + 0.17, cz), vec3(0.24, 0.17, 0.24), 0.0, Color::new(0.62, 0.66, 0.72, 1.0));
        draw_box(vec3(cx, base + 0.35, cz), vec3(0.27, 0.025, 0.27), 0.0, Color::new(0.46, 0.50, 0.56, 1.0));
        if pot.water {
            let color = match pot.meat {
                Some(meat) => {
                    let (raw, done) = (meat.color(), meat.stewed().map_or(meat.color(), |s| s.color()));
                    let t = pot.progress();
                    Color::new(raw.r + (done.r - raw.r) * t, raw.g + (done.g - raw.g) * t, raw.b + (done.b - raw.b) * t, 1.0)
                }
                None => Color::new(0.28, 0.48, 0.78, 1.0),
            };
            draw_box(vec3(cx, base + 0.32, cz), vec3(0.21, 0.02, 0.21), 0.0, color);
        }
    }
}

/// Draw the active Horizen hazard as a tall, layered spiral of fire. The simulation keeps only one tornado alive;
/// once it reaches its recorded target these layers continue to whirl in place until the next spawn.
pub fn draw_fire_tornado(tornado: &FireTornado, time: f32, player_chunk: (i32, i32)) {
    if !in_render_range(player_chunk, World::chunk_of_pos(tornado.pos)) {
        return;
    }
    for layer in 0..9 {
        let t = layer as f32 / 8.0;
        let radius = HORIZEN_TORNADO_RADIUS * (1.0 - 0.72 * t);
        let angle = time * HORIZEN_TORNADO_SPIN_SPEED + layer as f32 * 1.35;
        let centre = tornado.pos
            + vec3(angle.cos() * radius * 0.48, t * HORIZEN_TORNADO_HEIGHT + 0.35, angle.sin() * radius * 0.48);
        let fire = if layer % 2 == 0 {
            Color::new(1.0, 0.18 + 0.12 * (1.0 - t), 0.015, 0.88)
        } else {
            Color::new(1.0, 0.62, 0.04, 0.82)
        };
        draw_box(centre, vec3(radius, HORIZEN_TORNADO_HEIGHT / 18.0, radius), angle, fire);
        if layer % 2 == 0 {
            draw_box(
                centre + vec3(0.0, 0.18, 0.0),
                vec3(radius * 0.55, HORIZEN_TORNADO_HEIGHT / 24.0, radius * 0.55),
                -angle * 1.4,
                Color::new(1.0, 0.90, 0.18, 0.70),
            );
        }
    }
    // A dark, translucent cap makes the top read as smoke rather than a stack of unrelated boxes.
    draw_box(
        tornado.pos + vec3(0.0, HORIZEN_TORNADO_HEIGHT + 0.35, 0.0),
        vec3(HORIZEN_TORNADO_RADIUS * 0.42, 0.32, HORIZEN_TORNADO_RADIUS * 0.42),
        time * HORIZEN_TORNADO_CAP_SPIN_SPEED,
        Color::new(0.16, 0.10, 0.07, 0.42),
    );
}

/// Draw rain around the camera while a will-o-whisp storm is active. The drops are regenerated every frame from
/// their index, so there is no particle list to maintain and the effect follows the player naturally.
pub fn draw_storm_rain(eye: Vec3, time: f32) {
    for i in 0..WHISP_STORM_RAIN_COUNT {
        let n = i as f32;
        let x = eye.x + (n * 12.9898 + 17.3).sin() * WHISP_STORM_RAIN_RADIUS;
        let z = eye.z + (n * 78.233 + 41.7).sin() * WHISP_STORM_RAIN_RADIUS;
        let phase = (time * WHISP_STORM_RAIN_SPEED + n * 0.91).rem_euclid(WHISP_STORM_RAIN_HEIGHT + 4.0);
        let y = eye.y + WHISP_STORM_RAIN_HEIGHT - phase;
        let end = y - WHISP_STORM_RAIN_LENGTH;
        draw_line_3d(vec3(x, y, z), vec3(x, end, z), Color::new(0.62, 0.72, 0.88, 0.46));
    }
}

/// Draw a brief distant lightning bolt. `base` is the ground position where the strike lands and `flash` is the
/// remaining flash time, which makes the bolt disappear automatically after the strike.
pub fn draw_storm_lightning(base: Vec3, time: f32, flash: f32) {
    if flash <= 0.0 {
        return;
    }
    let top = base + vec3(0.0, 24.0, 0.0);
    let mut previous = top;
    let brightness = (flash / WHISP_STORM_LIGHTNING_FLASH_SECONDS).clamp(0.0, 1.0);
    for i in 1..=7 {
        let t = i as f32 / 7.0;
        let sway = (time * 17.0 + i as f32 * 4.7).sin() * (0.7 + t * 0.7);
        let next = base + vec3(sway, 24.0 * (1.0 - t), (time * 13.0 + i as f32 * 2.9).cos() * 0.5);
        draw_line_3d(previous, next, Color::new(0.82, 0.92, 1.0, 0.92 * brightness));
        previous = next;
    }
}

/// The colours of the rainbow, outermost first.
pub const RAINBOW_COLORS: [(f32, f32, f32); 7] = [
    (0.92, 0.16, 0.16),
    (0.97, 0.52, 0.12),
    (0.98, 0.88, 0.20),
    (0.25, 0.76, 0.28),
    (0.20, 0.52, 0.92),
    (0.30, 0.24, 0.78),
    (0.60, 0.28, 0.82),
];
/// Pieces the arc is sampled in, to find the blocks it passes through (fine enough that no block is skipped).
const RAINBOW_SAMPLES: usize = 160;
/// How thick the arc is, in blocks, from its red outer edge to its violet inner one: one block to a band.
const RAINBOW_BAND_BLOCKS: i32 = 7;

/// The point of the rainbow's arc a share `t` (0 to 1) of the way from `from` to `to`: the straight line between
/// the ends, lifted by a parabola that is `RAINBOW_ARC_RATIO` of the span high at the middle.
pub fn rainbow_point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let span = (to - from).length();
    from + (to - from) * t + Vec3::Y * (RAINBOW_ARC_RATIO * span * 4.0 * t * (1.0 - t))
}

/// One block of the rainbow: the cell it fills (its lowest corner) and the band it belongs to (0 is the red one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RainbowBlock {
    pub cell: (i32, i32, i32),
    pub band: usize,
}

/// The blocks the rainbow from `from` to `to` is built of: the arc is lifted into a stack of seven bands, red on the
/// outside and violet on the inside, one block thick each, and every band is the cells that its curve passes through.
/// A cell is in one band only (the outer band wins where two overlap), and a rainbow with both ends in one column
/// does not break. Plain geometry; `draw_rainbow` puts it on the screen.
pub fn rainbow_blocks(from: Vec3, to: Vec3) -> Vec<RainbowBlock> {
    let chord = vec3(to.x - from.x, 0.0, to.z - from.z);
    // Horizontal, across the arc: the arc lies in the vertical plane through its ends.
    let across = Vec3::Y.cross(chord).try_normalize().unwrap_or(Vec3::X);
    let mut taken: std::collections::HashSet<(i32, i32, i32)> = std::collections::HashSet::new();
    let mut blocks = Vec::new();
    for band in 0..RAINBOW_COLORS.len() {
        let inward = band as i32 - (RAINBOW_BAND_BLOCKS - 1) / 2;
        for i in 0..=RAINBOW_SAMPLES {
            let t = i as f32 / RAINBOW_SAMPLES as f32;
            let step = 1e-3;
            let tangent = (rainbow_point(from, to, (t + step).min(1.0)) - rainbow_point(from, to, (t - step).max(0.0))).normalize_or_zero();
            let outwards = tangent.cross(across).normalize_or_zero();
            // The band lies `inward` blocks towards the middle of the curve from the line of the arc.
            let spot = rainbow_point(from, to, t) - outwards * inward as f32;
            let cell = (spot.x.floor() as i32, spot.y.floor() as i32, spot.z.floor() as i32);
            if taken.insert(cell) {
                blocks.push(RainbowBlock { cell, band });
            }
        }
    }
    blocks
}

/// Draw the rainbow, a translucent shimmering arch of blocks from beside the player to the pile of gold. Faces
/// that two blocks of the rainbow share are left out, so it shows as a clean, blocky arch; every other face is drawn
/// from both sides. Call while the 3D camera is active, after the water.
pub fn draw_rainbow(world: &World, rainbow: &Rainbow, time: f32) {
    let ground = |x: i32, z: i32| world.ground_top(x, z) as f32 + 1.0;
    let from = vec3(rainbow.from.0 as f32 + 0.5, ground(rainbow.from.0, rainbow.from.1), rainbow.from.1 as f32 + 0.5);
    let (gx, gy, gz) = rainbow.gold;
    let to = vec3(gx as f32 + 0.5, gy as f32 + 0.3, gz as f32 + 0.5);
    let blocks = rainbow_blocks(from, to);
    // Keep the band owning each cell. The old renderer culled a face whenever *any* rainbow block touched it,
    // which enclosed middle colours behind neighbouring bands. Faces between different bands are part of the
    // rainbow's visible colour separation and must remain, especially when the camera is close to the arch.
    let filled: std::collections::HashMap<(i32, i32, i32), usize> = blocks.iter().map(|b| (b.cell, b.band)).collect();
    // The neighbour each face looks at, and the corners of the face (unit cube).
    const FACES: [((i32, i32, i32), [(f32, f32, f32); 4]); 6] = [
        ((1, 0, 0), [(1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (1.0, 1.0, 1.0), (1.0, 0.0, 1.0)]),
        ((-1, 0, 0), [(0.0, 0.0, 0.0), (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (0.0, 1.0, 0.0)]),
        ((0, 1, 0), [(0.0, 1.0, 0.0), (0.0, 1.0, 1.0), (1.0, 1.0, 1.0), (1.0, 1.0, 0.0)]),
        ((0, -1, 0), [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 0.0, 1.0), (0.0, 0.0, 1.0)]),
        ((0, 0, 1), [(0.0, 0.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 1.0), (0.0, 1.0, 1.0)]),
        ((0, 0, -1), [(0.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0), (1.0, 0.0, 0.0)]),
    ];
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let flush = |vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>| {
        if !vertices.is_empty() {
            draw_mesh(&Mesh { vertices: std::mem::take(vertices), indices: std::mem::take(indices), texture: None });
        }
    };
    for block in &blocks {
        let (r, g, b) = RAINBOW_COLORS[block.band];
        let alpha = 0.78 + 0.12 * (time * 1.7 + block.band as f32 * 0.6).sin();
        let (cx, cy, cz) = block.cell;
        for &((nx, ny, nz), corners) in FACES.iter() {
            match filled.get(&(cx + nx, cy + ny, cz + nz)) {
                // Same-colour neighbours share their internal face.
                Some(&other) if other == block.band => continue,
                // At a colour boundary, let the inner band own the face so every colour has a visible surface.
                Some(&other) if other > block.band => continue,
                _ => {}
            }
            // A little shading by direction, so the blocks read as blocks.
            let shade = match (nx, ny, nz) {
                (0, 1, 0) => 1.0,
                (0, -1, 0) => 0.7,
                (_, 0, 0) => 0.85,
                _ => 0.92,
            };
            if vertices.len() + 4 > MAX_QUADS_PER_MESH * 4 {
                flush(&mut vertices, &mut indices);
            }
            let base = vertices.len() as u16;
            for (ox, oy, oz) in corners {
                vertices.push(Vertex::new(cx as f32 + ox, cy as f32 + oy, cz as f32 + oz, 0.0, 0.0, Color::new(r * shade, g * shade, b * shade, alpha)));
            }
            // Both windings, so neither side is culled.
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3, base + 2, base + 1, base, base + 3, base + 2, base]);
        }
    }
    flush(&mut vertices, &mut indices);
}

/// One wisp of the ghostly trail: where it floats (x, z), how far along the path it is (blocks from the player),
/// how visible it is (0 to 1, fading with distance) and how brightly it pulses (0 to 1, the pulses running along
/// the trail towards its end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailWisp {
    pub x: f32,
    pub z: f32,
    pub along: f32,
    pub fade: f32,
    pub pulse: f32,
}

/// Blocks from the player where the trail begins, so that it does not crowd the screen.
const TRAIL_START: f32 = 2.5;

/// The wisps of a ghostly trail along `path` (corners (x, z), the first under the player): one every `spacing` blocks
/// from `TRAIL_START` to `length` blocks along it, or to its end if that is nearer, each swaying a little from side
/// to side. Plain geometry; `draw_trail` puts it on the screen.
pub fn trail_wisps(path: &[(f32, f32)], length: f32, spacing: f32, time: f32) -> Vec<TrailWisp> {
    let mut wisps = Vec::new();
    let mut walked = 0.0;
    let mut next = TRAIL_START;
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let leg = (dx * dx + dz * dz).sqrt();
        if leg < 1e-3 {
            continue;
        }
        let (ux, uz) = (dx / leg, dz / leg);
        while next <= walked + leg && next <= length {
            let t = next - walked;
            let sway = (next * 0.55 + time * 1.3).sin() * 0.35;
            wisps.push(TrailWisp {
                x: a.0 + ux * t - uz * sway,
                z: a.1 + uz * t + ux * sway,
                along: next,
                fade: (1.0 - next / length).clamp(0.0, 1.0).sqrt(),
                pulse: 0.5 + 0.5 * (next * 0.6 - time * 3.0).sin(),
            });
            next += spacing;
        }
        walked += leg;
        if next > length {
            break;
        }
    }
    wisps
}

/// Draw the ghostly trail of a peep stone: faint, drifting, pulsing lights floating over the ground along `path`, pale
/// blue at night and pale gold by day. Call while the 3D camera is active, with the other translucent things.
pub fn draw_trail(world: &World, path: &[(f32, f32)], (r, g, b): (f32, f32, f32), time: f32) {
    for wisp in trail_wisps(path, TRAIL_LENGTH, TRAIL_SPACING, time) {
        let ground = world.ground_top(wisp.x.floor() as i32, wisp.z.floor() as i32).max(GROUND_TOP) as f32 + 1.0;
        let y = ground + TRAIL_HEIGHT + 0.12 * (time * 2.0 + wisp.along).sin();
        let alpha = wisp.fade * (0.22 + 0.5 * wisp.pulse);
        let size = 0.09 + 0.07 * wisp.pulse;
        draw_box(vec3(wisp.x, y, wisp.z), vec3(size, size, size), time + wisp.along, Color::new(r, g, b, alpha));
        // A fainter, taller streak under each wisp, like a tail of mist.
        draw_box(vec3(wisp.x, y - 0.35, wisp.z), vec3(size * 0.5, 0.28, size * 0.5), 0.0, Color::new(r, g, b, alpha * 0.35));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arc_runs_from_one_end_to_the_other_and_rises_in_between() {
        let (from, to) = (vec3(0.0, 6.0, 0.0), vec3(30.0, 7.0, 10.0));
        assert!((rainbow_point(from, to, 0.0) - from).length() < 1e-4 && (rainbow_point(from, to, 1.0) - to).length() < 1e-4);
        let span = (to - from).length();
        let middle = rainbow_point(from, to, 0.5);
        let line = (from + to) / 2.0;
        assert!((middle.y - line.y - RAINBOW_ARC_RATIO * span).abs() < 1e-3, "the middle is {} above the line", middle.y - line.y);
        assert!((middle.x - line.x).abs() < 1e-4 && (middle.z - line.z).abs() < 1e-4, "straight up");
    }

    #[test]
    fn the_rainbow_is_a_blocky_arch_of_seven_colours_each_block_in_one_band() {
        let (from, to) = (vec3(0.0, 6.0, 0.0), vec3(30.0, 6.0, 0.0));
        let blocks = rainbow_blocks(from, to);
        let cells: std::collections::HashSet<_> = blocks.iter().map(|b| b.cell).collect();
        assert_eq!(cells.len(), blocks.len(), "no block twice");
        for band in 0..7 {
            assert!(blocks.iter().filter(|b| b.band == band).count() > 15, "band {band} is a long run of blocks");
        }
        // Every block is on the grid, and the arch is joined up: each block touches another by a face or an edge.
        for b in &blocks {
            let near = |dx: i32, dy: i32, dz: i32| cells.contains(&(b.cell.0 + dx, b.cell.1 + dy, b.cell.2 + dz));
            let joined = (-1..=1).any(|dx| (-1..=1).any(|dy| (-1..=1).any(|dz| (dx, dy, dz) != (0, 0, 0) && near(dx, dy, dz))));
            assert!(joined, "{b:?} stands alone");
        }
        // It rises from the ground at both ends to a high crown in the middle, a number of blocks thick.
        let top = blocks.iter().map(|b| b.cell.1).max().unwrap();
        let lowest = blocks.iter().map(|b| b.cell.1).min().unwrap();
        assert!(top as f32 > from.y + RAINBOW_ARC_RATIO * 30.0 * 0.9, "tall: {top}");
        assert!(lowest as f32 <= from.y + 1.0, "it comes down to the ground: {lowest}");
        let crown: Vec<_> = blocks.iter().filter(|b| (b.cell.0 - 15).abs() <= 1).collect();
        assert!(crown.iter().map(|b| b.cell.1).max().unwrap() - crown.iter().map(|b| b.cell.1).min().unwrap() >= 5, "several blocks thick at the crown");
        // The red band is on the outside, the violet on the inside.
        let band_top = |band: usize| crown.iter().filter(|b| b.band == band).map(|b| b.cell.1).max();
        assert!(band_top(0) > band_top(6), "red above violet: {:?} {:?}", band_top(0), band_top(6));
    }

    #[test]
    fn a_rainbow_with_both_ends_in_one_column_does_not_break() {
        let blocks = rainbow_blocks(vec3(5.0, 6.0, 5.0), vec3(5.0, 6.0, 5.0));
        assert!(!blocks.is_empty());
    }

    #[test]
    fn a_trail_runs_from_beside_the_player_along_the_path_and_fades_with_distance() {
        let path = [(0.0, 0.0), (100.0, 0.0)];
        let wisps = trail_wisps(&path, TRAIL_LENGTH, TRAIL_SPACING, 0.0);
        assert!(wisps.len() as f32 > TRAIL_LENGTH / TRAIL_SPACING * 0.9, "{} wisps", wisps.len());
        assert!(wisps[0].along >= 2.0 && wisps.last().unwrap().along <= TRAIL_LENGTH);
        assert!(wisps.windows(2).all(|w| w[1].along > w[0].along && w[1].fade <= w[0].fade), "further wisps are fainter");
        assert!(wisps.iter().all(|w| w.x > 0.0 && w.z.abs() <= 0.36 && (0.0..=1.0).contains(&w.fade) && (0.0..=1.0).contains(&w.pulse)));
        // The pulses travel: a moment later the bright ones have moved on.
        let later = trail_wisps(&path, TRAIL_LENGTH, TRAIL_SPACING, 0.2);
        assert!(wisps.iter().zip(&later).any(|(a, b)| (a.pulse - b.pulse).abs() > 0.05));
    }

    #[test]
    fn a_trail_that_ends_nearby_stops_there_and_one_with_corners_follows_them() {
        let short = trail_wisps(&[(0.0, 0.0), (10.0, 0.0)], TRAIL_LENGTH, TRAIL_SPACING, 0.0);
        assert!(short.iter().all(|w| w.x <= 10.5), "nothing beyond the end");
        assert!(short.len() >= 4);
        // Round a corner: east for 20 blocks, then north (towards smaller z).
        let bent = trail_wisps(&[(0.0, 0.0), (20.0, 0.0), (20.0, -40.0)], TRAIL_LENGTH, TRAIL_SPACING, 0.0);
        assert!(bent.iter().any(|w| w.x < 19.0 && w.z.abs() < 0.5), "along the first leg");
        assert!(bent.iter().any(|w| (w.x - 20.0).abs() < 0.5 && w.z < -10.0), "and then along the second");
        assert!(trail_wisps(&[(3.0, 3.0), (3.0, 3.0)], TRAIL_LENGTH, TRAIL_SPACING, 0.0).is_empty(), "no path, no trail");
        assert!(trail_wisps(&[], TRAIL_LENGTH, TRAIL_SPACING, 0.0).is_empty());
    }
}
