//! Animated effects drawn on top of the static chunk meshes.

use macroquad::prelude::*;

use crate::block::Block;
use crate::dropped::draw_box;
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
