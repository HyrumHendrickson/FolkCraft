//! Animated effects drawn on top of the static chunk meshes.

use macroquad::prelude::*;

use crate::block::Block;
use crate::config::*;
use crate::dropped::draw_box;
use crate::landmarks::Rainbow;
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
/// Pieces each band of the arc is cut into, and how wide a band is, in blocks.
const RAINBOW_SEGMENTS: usize = 36;
const RAINBOW_BAND_WIDTH: f32 = 0.8;

/// The point of the rainbow's arc a share `t` (0 to 1) of the way from `from` to `to`: the straight line between
/// the ends, lifted by a parabola that is `RAINBOW_ARC_RATIO` of the span high at the middle.
pub fn rainbow_point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let span = (to - from).length();
    from + (to - from) * t + Vec3::Y * (RAINBOW_ARC_RATIO * span * 4.0 * t * (1.0 - t))
}

/// One flat piece of one band of the rainbow: its four corners, and the band it belongs to (0 is the red one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RainbowQuad {
    pub corners: [Vec3; 4],
    pub band: usize,
}

/// The pieces the rainbow from `from` to `to` is drawn from: seven bands side by side across the arc, red on the
/// outside, each cut into `RAINBOW_SEGMENTS` pieces. Plain geometry; `draw_rainbow` puts it on the screen.
pub fn rainbow_quads(from: Vec3, to: Vec3) -> Vec<RainbowQuad> {
    let chord = vec3(to.x - from.x, 0.0, to.z - from.z);
    // Horizontal, across the arc: the arc lies in the vertical plane through its ends.
    let across = Vec3::Y.cross(chord).try_normalize().unwrap_or(Vec3::X);
    let mut quads = Vec::with_capacity(RAINBOW_COLORS.len() * RAINBOW_SEGMENTS);
    // The point of the arc, and the direction pointing outwards from the arc's curve, at a share `t` of the way.
    let frame = |t: f32| {
        let step = 1e-3;
        let tangent = (rainbow_point(from, to, (t + step).min(1.0)) - rainbow_point(from, to, (t - step).max(0.0))).normalize_or_zero();
        (rainbow_point(from, to, t), tangent.cross(across).normalize_or_zero())
    };
    for band in 0..RAINBOW_COLORS.len() {
        let centre = (RAINBOW_COLORS.len() as f32 / 2.0 - 0.5 - band as f32) * RAINBOW_BAND_WIDTH;
        let (inner, outer) = (centre - RAINBOW_BAND_WIDTH / 2.0, centre + RAINBOW_BAND_WIDTH / 2.0);
        for i in 0..RAINBOW_SEGMENTS {
            let ((p0, n0), (p1, n1)) = (frame(i as f32 / RAINBOW_SEGMENTS as f32), frame((i + 1) as f32 / RAINBOW_SEGMENTS as f32));
            quads.push(RainbowQuad { corners: [p0 + n0 * inner, p1 + n1 * inner, p1 + n1 * outer, p0 + n0 * outer], band });
        }
    }
    quads
}

/// Draw the rainbow, a translucent shimmering arc from beside the player to the pile of gold. Both faces of every
/// piece are drawn, so it shows from either side. Call while the 3D camera is active, after the water.
pub fn draw_rainbow(world: &World, rainbow: &Rainbow, time: f32) {
    let ground = |x: i32, z: i32| world.ground_top(x, z) as f32 + 1.0;
    let from = vec3(rainbow.from.0 as f32 + 0.5, ground(rainbow.from.0, rainbow.from.1), rainbow.from.1 as f32 + 0.5);
    let (gx, gy, gz) = rainbow.gold;
    let to = vec3(gx as f32 + 0.5, gy as f32 + 0.3, gz as f32 + 0.5);
    let mut vertices = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    for quad in rainbow_quads(from, to) {
        let (r, g, b) = RAINBOW_COLORS[quad.band];
        let alpha = 0.50 + 0.10 * (time * 1.7 + quad.band as f32 * 0.6).sin();
        let base = vertices.len() as u16;
        for corner in quad.corners {
            vertices.push(Vertex::new(corner.x, corner.y, corner.z, 0.0, 0.0, Color::new(r, g, b, alpha)));
        }
        // Both windings, so neither side is culled.
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3, base + 2, base + 1, base, base + 3, base + 2, base]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
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
    fn the_rainbow_has_seven_bands_of_equal_pieces_that_follow_the_arc() {
        let (from, to) = (vec3(0.0, 6.0, 0.0), vec3(30.0, 6.0, 0.0));
        let quads = rainbow_quads(from, to);
        assert_eq!(quads.len(), 7 * RAINBOW_SEGMENTS);
        assert!(quads.len() * 2 <= MAX_QUADS_PER_MESH, "drawn both sides in one mesh");
        for band in 0..7 {
            assert_eq!(quads.iter().filter(|q| q.band == band).count(), RAINBOW_SEGMENTS);
        }
        // At the top of the arc the red band is the highest and the violet one the lowest.
        let top = |band: usize| {
            quads.iter().filter(|q| q.band == band).map(|q| q.corners.iter().map(|c| c.y).fold(f32::MIN, f32::max)).fold(f32::MIN, f32::max)
        };
        assert!(top(0) > top(3) && top(3) > top(6), "{} {} {}", top(0), top(3), top(6));
        // Every piece is a real quadrilateral, and the arc reaches (nearly) down to the ground at both ends.
        for q in &quads {
            assert!(q.corners.iter().all(|c| c.is_finite()));
            assert!((q.corners[0] - q.corners[3]).length() > 0.5, "a band is wide");
        }
        let lowest = quads.iter().flat_map(|q| q.corners).map(|c| c.y).fold(f32::MAX, f32::min);
        assert!(lowest < from.y + 4.0, "the arc comes down to the ground, lowest point {lowest}");
        let highest = quads.iter().flat_map(|q| q.corners).map(|c| c.y).fold(f32::MIN, f32::max);
        assert!(highest > from.y + RAINBOW_ARC_RATIO * 30.0, "and is tall: {highest}");
    }

    #[test]
    fn a_rainbow_with_both_ends_in_one_column_does_not_break() {
        let quads = rainbow_quads(vec3(5.0, 6.0, 5.0), vec3(5.0, 6.0, 5.0));
        assert!(quads.iter().all(|q| q.corners.iter().all(|c| c.is_finite())));
    }
}
