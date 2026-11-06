//! Animated effects drawn on top of the static chunk meshes.

use macroquad::prelude::*;

use crate::block::Block;
use crate::config::*;
use crate::dropped::draw_box;
use crate::horizen::Dimension;
use crate::landmarks::Rainbow;
use crate::rng::hash3;
use crate::tornado::{Cyclone, FireTornado};
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
        let coal_y = if block == Block::ForgeLit { 0.97 } else { 0.30 };
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
            let half = if block == Block::ForgeLit { vec3(0.10, 0.04, 0.10) } else { vec3(0.37, 0.07, 0.31) };
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

/// How wide a layer of the fire tornado is, as a share of `SKY_TORNADO_RADIUS`, at height share `t` (0 at the
/// ground, 1 at the top): a funnel that is narrowest at its foot and widest at its top.
pub fn tornado_radius_share(t: f32) -> f32 {
    SKY_TORNADO_FOOT_SHARE + (1.0 - SKY_TORNADO_FOOT_SHARE) * t.clamp(0.0, 1.0)
}

/// Draw the active Horizen hazard as a tall, layered spiral of fire. The simulation keeps only one tornado alive;
/// once it reaches its recorded target these layers continue to whirl in place until the next spawn.
pub fn draw_fire_tornado(tornado: &FireTornado, time: f32, player_chunk: (i32, i32)) {
    if !in_render_range(player_chunk, World::chunk_of_pos(tornado.pos)) {
        return;
    }
    for layer in 0..9 {
        let t = layer as f32 / 8.0;
        // A funnel: narrow at the foot, wide at the top.
        let radius = SKY_TORNADO_RADIUS * tornado_radius_share(t);
        let angle = time * SKY_TORNADO_SPIN_SPEED + layer as f32 * 1.35;
        let centre = tornado.pos
            + vec3(angle.cos() * radius * 0.48, t * SKY_TORNADO_HEIGHT + 0.35, angle.sin() * radius * 0.48);
        let fire = if layer % 2 == 0 {
            Color::new(1.0, 0.18 + 0.12 * (1.0 - t), 0.015, 0.88)
        } else {
            Color::new(1.0, 0.62, 0.04, 0.82)
        };
        draw_box(centre, vec3(radius, SKY_TORNADO_HEIGHT / 18.0, radius), angle, fire);
        if layer % 2 == 0 {
            draw_box(
                centre + vec3(0.0, 0.18, 0.0),
                vec3(radius * 0.55, SKY_TORNADO_HEIGHT / 24.0, radius * 0.55),
                -angle * 1.4,
                Color::new(1.0, 0.90, 0.18, 0.70),
            );
        }
    }
    // A dark, translucent cap makes the top read as smoke rather than a stack of unrelated boxes.
    draw_box(
        tornado.pos + vec3(0.0, SKY_TORNADO_HEIGHT + 0.35, 0.0),
        vec3(SKY_TORNADO_RADIUS * 0.8, 0.32, SKY_TORNADO_RADIUS * 0.8),
        time * SKY_TORNADO_CAP_SPIN_SPEED,
        Color::new(0.16, 0.10, 0.07, 0.42),
    );
}

/// Draw a cyclone: a wide funnel of pale, translucent layers spinning round a bright core, with flecks of snow and
/// debris whirling up it. Call while the 3D camera is active.
pub fn draw_cyclone(cyclone: &Cyclone, time: f32, player_chunk: (i32, i32)) {
    if !in_render_range(player_chunk, World::chunk_of_pos(cyclone.pos)) {
        return;
    }
    for layer in 0..12 {
        let t = layer as f32 / 11.0;
        let radius = CYCLONE_RADIUS * (0.35 + 0.65 * t);
        let angle = time * CYCLONE_SPIN_SPEED * (1.0 - 0.3 * t) + layer as f32 * 1.1;
        let centre = cyclone.pos + vec3(angle.cos() * radius * 0.25, t * CYCLONE_HEIGHT + 0.3, angle.sin() * radius * 0.25);
        let shade = 0.80 + 0.18 * (layer % 2) as f32;
        draw_box(centre, vec3(radius, CYCLONE_HEIGHT / 26.0, radius), angle, Color::new(shade * 0.92, shade, shade * 1.04, 0.62));
    }
    draw_box(cyclone.pos + vec3(0.0, CYCLONE_HEIGHT * 0.5, 0.0), vec3(0.28, CYCLONE_HEIGHT * 0.5, 0.28), time * 3.0, Color::new(1.0, 1.0, 1.0, 0.55));
    for i in 0..18 {
        let n = i as f32;
        let t = (time * 0.35 + n * 0.173).rem_euclid(1.0);
        let angle = time * CYCLONE_SPIN_SPEED * 1.3 + n * 2.4;
        let radius = CYCLONE_RADIUS * (0.4 + 0.75 * t);
        let at = cyclone.pos + vec3(angle.cos() * radius, t * CYCLONE_HEIGHT, angle.sin() * radius);
        draw_box(at, Vec3::splat(0.07 + 0.05 * (i % 3) as f32), angle, if i % 4 == 0 { Color::new(0.50, 0.40, 0.30, 0.9) } else { Color::new(0.97, 0.98, 1.0, 0.85) });
    }
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

/// A straight, square-sectioned beam of half-width `half` from `a` to `b`, lit all round and drawn from every side.
/// Unlike `draw_line_3d` (always one pixel wide) it has real thickness, so a lightning bolt can be seen.
fn draw_beam(a: Vec3, b: Vec3, half: f32, color: Color) {
    let along = (b - a).normalize_or_zero();
    if along == Vec3::ZERO {
        return;
    }
    let helper = if along.y.abs() > 0.95 { Vec3::X } else { Vec3::Y };
    let p1 = along.cross(helper).normalize() * half;
    let p2 = along.cross(p1).normalize() * half;
    let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let mut vertices = Vec::with_capacity(8);
    for end in [a, b] {
        for (u, v) in corners {
            let p = end + p1 * u + p2 * v;
            vertices.push(Vertex::new(p.x, p.y, p.z, 0.0, 0.0, color));
        }
    }
    let mut indices: Vec<u16> = Vec::with_capacity(48);
    for i in 0..4u16 {
        let j = (i + 1) % 4;
        let (a0, a1, b0, b1) = (i, j, i + 4, j + 4);
        // Both windings, so neither side is culled.
        indices.extend_from_slice(&[a0, a1, b1, a0, b1, b0, b1, a1, a0, b0, b1, a0]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

/// The jagged line of a lightning bolt that lands at `base`, from the underside of the clouds (or at least `LIGHTNING_MIN_HEIGHT`
/// above the ground) down to the strike, as points from the top to `base`. It wanders sideways between its ends and is a pure
/// function of where it lands, so it keeps its shape for as long as it is drawn.
pub fn bolt_points(base: Vec3) -> Vec<Vec3> {
    let top_y = CLOUD_HEIGHT.max(base.y + LIGHTNING_MIN_HEIGHT);
    let segments = (((top_y - base.y) / 3.0).ceil() as usize).clamp(8, 48);
    let (bx, bz) = (base.x.floor() as i32, base.z.floor() as i32);
    let wobble = |i: usize, salt: i32| (hash3(0x1B07, bx, bz, i as i32 * 4 + salt) % 2001) as f32 / 1000.0 - 1.0;
    (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            let swing = (t * std::f32::consts::PI).sin() * 3.2;
            vec3(base.x + wobble(i, 0) * swing, top_y + (base.y - top_y) * t, base.z + wobble(i, 1) * swing)
        })
        .collect()
}

/// A short side branch that leaves the bolt at `from` and forks down and away, as points.
fn bolt_branch(from: Vec3, salt: i32) -> Vec<Vec3> {
    let (fx, fz) = (from.x.floor() as i32, from.z.floor() as i32);
    let angle = (hash3(0x1B08, fx, fz, salt) % 6283) as f32 / 1000.0;
    let mut point = from;
    let mut points = vec![point];
    for k in 0..5 {
        let drift = 1.8 + (hash3(0x1B09, fx, fz, salt * 8 + k) % 100) as f32 / 100.0;
        let bend = angle + (k as f32) * 0.5;
        point += vec3(bend.cos() * drift, -3.0, bend.sin() * drift);
        points.push(point);
    }
    points
}

/// How strongly the whole screen flashes white while a bolt is out: 0 to `LIGHTNING_SCREEN_FLASH`, dying away as the
/// flash ends and fainter the farther the strike is (`distance` blocks from the player).
pub fn lightning_flash_alpha(flash: f32, distance: f32) -> f32 {
    let left = (flash / WHISP_STORM_LIGHTNING_FLASH_SECONDS).clamp(0.0, 1.0);
    let near = (1.0 - distance / 120.0).clamp(0.3, 1.0);
    left * left * LIGHTNING_SCREEN_FLASH * near
}

/// Draw a lightning bolt that lands at `base` while its `flash` (seconds left) lasts, seen from `eye`: a thick white core
/// inside a blue glow, jagged, with a few forks, running from the clouds to the ground, and a bright burst where it lands.
/// The bolt is thicker the farther it is, so a distant strike is as easy to see. Call while the 3D camera is active.
pub fn draw_storm_lightning(eye: Vec3, base: Vec3, time: f32, flash: f32) {
    if flash <= 0.0 {
        return;
    }
    let left = (flash / WHISP_STORM_LIGHTNING_FLASH_SECONDS).clamp(0.0, 1.0);
    // The bolt flickers, as real ones do, but never goes out until the flash is over.
    let flicker = if (time * 45.0).sin() > -0.4 { 1.0 } else { 0.6 };
    let alpha = (0.45 + 0.55 * left) * flicker;
    let scale = 1.0 + (base - eye).length() / 35.0;
    let core = LIGHTNING_BOLT_WIDTH * 0.5 * scale;
    let points = bolt_points(base);
    let draw_path = |path: &[Vec3], width: f32, strength: f32| {
        for pair in path.windows(2) {
            draw_beam(pair[0], pair[1], width * 3.2, Color::new(0.55, 0.70, 1.0, alpha * 0.30 * strength));
            draw_beam(pair[0], pair[1], width, Color::new(1.0, 1.0, 1.0, (alpha * strength).min(1.0)));
        }
    };
    draw_path(&points, core, 1.0);
    for (k, share) in [0.3_f32, 0.5, 0.7].iter().enumerate() {
        let from = points[((points.len() - 1) as f32 * share) as usize];
        draw_path(&bolt_branch(from, k as i32), core * 0.55, 0.8);
    }
    // Where it lands: a flat burst of light on the ground.
    let burst = 0.8 + LIGHTNING_RADIUS * 0.3 * (1.0 - left);
    draw_box(base + vec3(0.0, 0.15, 0.0), vec3(burst, 0.15, burst), 0.0, Color::new(1.0, 1.0, 0.85, 0.55 * left));
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

/// The blocks the rainbow from `from` to `to` is built of over level ground (each end at its own height). See
/// `rainbow_blocks_on`.
#[cfg(test)]
pub fn rainbow_blocks(from: Vec3, to: Vec3) -> Vec<RainbowBlock> {
    let flat = |x: i32, z: i32| {
        let (dx, dz) = (x as f32 + 0.5 - from.x, z as f32 + 0.5 - from.z);
        let (ex, ez) = (x as f32 + 0.5 - to.x, z as f32 + 0.5 - to.z);
        (if dx * dx + dz * dz <= ex * ex + ez * ez { from.y } else { to.y }).floor() as i32
    };
    rainbow_blocks_on(from, to, &flat)
}

/// The blocks the rainbow from `from` to `to` is built of: the arc is lifted into a stack of seven bands, red on the
/// outside and violet on the inside, one block thick each, and every band is the cells that its curve passes through.
/// A cell is in one band only (the outer band wins where two overlap), and a rainbow with both ends in one column
/// does not break. `ground(x, z)` is the lowest cell above the ground in a column. Where the arch meets the ground its
/// inner bands would sink into the land and its outer bands would hang in the air, hiding some colours, so a cell
/// under the ground is left out and the first and last cell of every band are carried straight down to the ground:
/// all seven colours stand on the ground at each end. Plain geometry; `draw_rainbow` puts it on the screen.
pub fn rainbow_blocks_on(from: Vec3, to: Vec3, ground: &dyn Fn(i32, i32) -> i32) -> Vec<RainbowBlock> {
    let chord = vec3(to.x - from.x, 0.0, to.z - from.z);
    // Horizontal, across the arc: the arc lies in the vertical plane through its ends.
    let across = Vec3::Y.cross(chord).try_normalize().unwrap_or(Vec3::X);
    // Fine enough that no block is skipped, however long the arch (a long one is several blocks per sample otherwise).
    let samples = (((to - from).length() * 3.0).ceil() as usize).max(RAINBOW_SAMPLES);
    let mut taken: std::collections::HashSet<(i32, i32, i32)> = std::collections::HashSet::new();
    let mut blocks = Vec::new();
    for band in 0..RAINBOW_COLORS.len() {
        let inward = band as i32 - (RAINBOW_BAND_BLOCKS - 1) / 2;
        let mut ends: [Option<(i32, i32, i32)>; 2] = [None, None];
        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let step = 1e-3;
            let tangent = (rainbow_point(from, to, (t + step).min(1.0)) - rainbow_point(from, to, (t - step).max(0.0))).normalize_or_zero();
            let outwards = tangent.cross(across).normalize_or_zero();
            // The band lies `inward` blocks towards the middle of the curve from the line of the arc.
            let spot = rainbow_point(from, to, t) - outwards * inward as f32;
            let cell = (spot.x.floor() as i32, spot.y.floor() as i32, spot.z.floor() as i32);
            if cell.1 < ground(cell.0, cell.2) {
                continue; // buried in the land
            }
            ends[0].get_or_insert(cell);
            ends[1] = Some(cell);
            if taken.insert(cell) {
                blocks.push(RainbowBlock { cell, band });
            }
        }
        for (x, y, z) in ends.into_iter().flatten() {
            for below in ground(x, z)..y {
                if taken.insert((x, below, z)) {
                    blocks.push(RainbowBlock { cell: (x, below, z), band });
                }
            }
        }
    }
    blocks
}

/// Whether the whole arch from the column `from` to the column `gold` lies beyond the render distance of a player at
/// `player` (x, z): every point along it, with the width of its bands, is farther than any drawn chunk, counting
/// along either axis as the chunks are.
pub fn rainbow_clear_of_view(player: (f32, f32), from: (i32, i32), gold: (i32, i32)) -> bool {
    let end = |c: (i32, i32)| vec3(c.0 as f32 + 0.5, 0.0, c.1 as f32 + 0.5);
    let (a, b) = (end(from), end(gold));
    let seen = ((render_radius() + 1) * CHUNK_SIZE) as f32 + RAINBOW_BAND_BLOCKS as f32;
    (0..=64).all(|i| {
        let p = rainbow_point(a, b, i as f32 / 64.0);
        (p.x - player.0).abs().max((p.z - player.1).abs()) > seen
    })
}

thread_local! {
    /// The blocks of the rainbow last drawn, kept while it stays the same: working them out samples the terrain
    /// hundreds of times, which is too much to do every frame.
    static RAINBOW_CACHE: std::cell::RefCell<Option<(Rainbow, std::rc::Rc<Vec<RainbowBlock>>)>> = std::cell::RefCell::new(None);
}

/// Draw the rainbow, a translucent shimmering arch of blocks from beside the player to the pile of gold. Faces
/// that two blocks of the rainbow share are left out, so it shows as a clean, blocky arch; every other face is drawn
/// from both sides. Call while the 3D camera is active, after the water.
pub fn draw_rainbow(world: &World, rainbow: &Rainbow, time: f32) {
    let ground = |x: i32, z: i32| world.ground_top(x, z) as f32 + 1.0;
    let from = vec3(rainbow.from.0 as f32 + 0.5, ground(rainbow.from.0, rainbow.from.1), rainbow.from.1 as f32 + 0.5);
    let (gx, gy, gz) = rainbow.gold;
    let to = vec3(gx as f32 + 0.5, gy as f32 + 0.3, gz as f32 + 0.5);
    let blocks = RAINBOW_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached, blocks)) = cache.as_ref() {
            if cached == rainbow {
                return blocks.clone();
            }
        }
        let blocks = std::rc::Rc::new(rainbow_blocks_on(from, to, &|x, z| world.ground_top(x, z) + 1));
        *cache = Some((*rainbow, blocks.clone()));
        blocks
    });
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
    for block in blocks.iter() {
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

/// The height a trail's wisps float over in the column (x, z): the ground in the overworld and the Horizen; in the Skylands the
/// top of the island or bridge deck there, and over the void the height of the player's feet (`feet_y`), so that the trail
/// carries on through the air from island to island.
pub fn trail_floor(world: &World, x: i32, z: i32, feet_y: f32) -> f32 {
    if world.dimension == Dimension::Skylands {
        return crate::skylands::surface_top(world.seed, x, z).map_or(feet_y.floor(), |top| top as f32 + 1.0);
    }
    world.ground_top(x, z).max(GROUND_TOP) as f32 + 1.0
}

/// Draw the ghostly trail of a peep stone: faint, drifting, pulsing lights floating over the ground along `path`, pale
/// blue at night and pale gold by day. Call while the 3D camera is active, with the other translucent things.
pub fn draw_trail(world: &World, path: &[(f32, f32)], (r, g, b): (f32, f32, f32), time: f32, feet_y: f32) {
    for wisp in trail_wisps(path, TRAIL_LENGTH, TRAIL_SPACING, time) {
        let ground = trail_floor(world, wisp.x.floor() as i32, wisp.z.floor() as i32, feet_y);
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
    fn the_fire_tornado_is_a_funnel_wide_at_the_top_and_spins_half_again_as_fast() {
        assert!(tornado_radius_share(0.0) < tornado_radius_share(0.5) && tornado_radius_share(0.5) < tornado_radius_share(1.0));
        assert!((tornado_radius_share(0.0) - SKY_TORNADO_FOOT_SHARE).abs() < 1e-6, "narrow at the foot");
        assert!((tornado_radius_share(1.0) - 1.0).abs() < 1e-6, "full width at the top");
        assert!((SKY_TORNADO_SPIN_SPEED - 8.0 * 1.5).abs() < 1e-6, "50% faster than the 8 radians per second it had");
        assert!((SKY_TORNADO_CAP_SPIN_SPEED - 1.4 * 1.5).abs() < 1e-6);
    }

    #[test]
    fn a_lightning_bolt_runs_from_the_clouds_to_the_strike_and_keeps_its_shape() {
        let base = vec3(10.5, 7.0, -20.5);
        let points = bolt_points(base);
        assert_eq!(points, bolt_points(base), "the same strike always has the same shape");
        assert!(points.len() >= 9);
        let (first, last) = (points[0], *points.last().unwrap());
        assert!(first.y >= CLOUD_HEIGHT, "it starts in the clouds: {}", first.y);
        assert_eq!(last, base, "and ends exactly where it lands");
        assert!(points.windows(2).all(|w| w[1].y < w[0].y), "always downwards");
        assert!(points.iter().any(|p| (p.x - base.x).abs() > 0.3 || (p.z - base.z).abs() > 0.3), "jagged, not straight");
        assert!(points.iter().all(|p| (p.x - base.x).abs() < 4.0 && (p.z - base.z).abs() < 4.0), "but it stays near the strike");
        // A strike on a mountainside above the clouds still has a long bolt to be seen.
        let high = bolt_points(vec3(0.5, 150.0, 0.5));
        assert!(high[0].y >= 150.0 + LIGHTNING_MIN_HEIGHT);
    }

    #[test]
    fn the_whole_screen_flashes_at_a_strike_and_fades_with_it_and_with_distance() {
        let full = lightning_flash_alpha(WHISP_STORM_LIGHTNING_FLASH_SECONDS, 0.0);
        assert!((full - LIGHTNING_SCREEN_FLASH).abs() < 1e-6);
        assert!(lightning_flash_alpha(WHISP_STORM_LIGHTNING_FLASH_SECONDS / 2.0, 0.0) < full);
        assert_eq!(lightning_flash_alpha(0.0, 0.0), 0.0, "nothing once it is over");
        assert!(lightning_flash_alpha(WHISP_STORM_LIGHTNING_FLASH_SECONDS, 90.0) < full, "a far strike is fainter");
        assert!(lightning_flash_alpha(WHISP_STORM_LIGHTNING_FLASH_SECONDS, 90.0) > 0.0, "but still seen");
        assert!(WHISP_STORM_LIGHTNING_FLASH_SECONDS >= 0.4, "long enough to notice");
    }

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
    fn all_seven_colours_stand_on_the_ground_at_both_ends_of_the_arch() {
        for (from, to) in [
            (vec3(0.0, 6.0, 0.0), vec3(30.0, 6.0, 0.0)),
            (vec3(0.0, 6.0, 0.0), vec3(90.0, 6.0, 40.0)),
            (vec3(5.0, 8.0, 5.0), vec3(-70.0, 5.0, 60.0)),
        ] {
            let blocks = rainbow_blocks(from, to);
            for band in 0..RAINBOW_COLORS.len() {
                for end in [from, to] {
                    let near: Vec<_> = blocks
                        .iter()
                        .filter(|b| b.band == band)
                        .filter(|b| ((b.cell.0 as f32 + 0.5 - end.x).powi(2) + (b.cell.2 as f32 + 0.5 - end.z).powi(2)).sqrt() < 7.0)
                        .collect();
                    assert!(!near.is_empty(), "band {band} is missing at the end {end:?}");
                    let lowest = near.iter().map(|b| b.cell.1).min().unwrap();
                    assert_eq!(lowest, end.y.floor() as i32, "band {band} does not reach the ground at {end:?}");
                    // And it is a column or a slope, not a stray block: it joins up with the rest of its band.
                    assert!(near.len() >= 2, "band {band} at {end:?}");
                }
            }
        }
    }

    #[test]
    fn a_rainbow_over_rising_ground_is_never_buried_and_still_reaches_the_ground() {
        let (from, to) = (vec3(0.5, 6.0, 0.5), vec3(60.5, 6.0, 0.5));
        // Ground that climbs a block every four columns on the first stretch, like a hill at the start of the arch.
        let ground = |x: i32, _z: i32| 6 + (x.clamp(0, 20) / 4);
        let blocks = rainbow_blocks_on(from, to, &ground);
        assert!(blocks.iter().all(|b| b.cell.1 >= ground(b.cell.0, b.cell.2)), "no block is under the ground");
        for band in 0..RAINBOW_COLORS.len() {
            assert!(blocks.iter().filter(|b| b.band == band).count() > 15, "band {band} shows");
            let of_band = || blocks.iter().filter(|b| b.band == band);
            let first_x = of_band().map(|b| b.cell.0).min().unwrap();
            let lowest = of_band().filter(|b| b.cell.0 == first_x).map(|b| b.cell.1).min().unwrap();
            assert_eq!(lowest, ground(first_x, 0), "band {band} stands on the ground at the start");
        }
    }

    #[test]
    fn a_long_rainbow_has_no_gaps_in_its_bands() {
        let (from, to) = (vec3(0.0, 6.0, 0.0), vec3(150.0, 6.0, 60.0));
        let blocks = rainbow_blocks(from, to);
        let cells: std::collections::HashSet<_> = blocks.iter().map(|b| b.cell).collect();
        for b in &blocks {
            let joined = (-1..=1).any(|dx| (-1..=1).any(|dy| (-1..=1).any(|dz| (dx, dy, dz) != (0, 0, 0) && cells.contains(&(b.cell.0 + dx, b.cell.1 + dy, b.cell.2 + dz)))));
            assert!(joined, "{b:?} stands alone: the arch has a gap");
        }
    }

    #[test]
    fn an_arch_beyond_the_render_distance_is_clear_of_view_and_one_across_the_player_is_not() {
        let view = ((render_radius() + 1) * CHUNK_SIZE) as f32;
        let far = view as i32 + 40;
        assert!(rainbow_clear_of_view((0.0, 0.0), (far, 0), (far, 90)));
        assert!(!rainbow_clear_of_view((0.0, 0.0), (far, 0), (-far, 0)), "the middle of this arch passes over the player");
        assert!(!rainbow_clear_of_view((0.0, 0.0), (far, 0), (view as i32 - 20, 90)), "an end inside the view");
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
