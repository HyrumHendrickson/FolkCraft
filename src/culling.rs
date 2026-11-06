//! View culling: which chunks are worth drawing this frame.

use macroquad::prelude::*;

use crate::config::*;
use crate::world::{in_render_range, World};

/// Conservative sphere-vs-view-cone test for a chunk. A chunk is a tall column (the world is `WORLD_HEIGHT` high,
/// for the mountain), so the sphere is centred on the point of the column level with the eye, the nearest part of
/// it, and a column is never dropped for being far above or below.
pub fn chunk_visible(eye: Vec3, dir: Vec3, half_fov: f32, cx: i32, cz: i32) -> bool {
    let centre = vec3(
        (cx * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
        eye.y.clamp(0.0, WORLD_HEIGHT as f32),
        (cz * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
    );
    let radius = 19.0;
    let v = centre - eye;
    let d = v.length();
    if d <= radius {
        return true;
    }
    let angle = (v.dot(dir) / d).clamp(-1.0, 1.0).acos();
    angle <= half_fov + (radius / d).asin()
}

/// Chunks to draw this frame: within the render radius of the player, holding a mesh,
/// and inside the view cone.
pub fn chunks_to_draw(world: &World, eye: Vec3, dir: Vec3, half_fov: f32) -> Vec<(i32, i32)> {
    let centre = World::chunk_of_pos(eye);
    world
        .chunks
        .iter()
        .filter(|(&key, chunk)| {
            in_render_range(centre, key) && !chunk.meshes.is_empty() && chunk_visible(eye, dir, half_fov, key.0, key.1)
        })
        .map(|(&key, _)| key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn chunk_culling_keeps_nearby_and_visible_drops_behind() {
        let eye = vec3(10.0, 8.0, 10.0);
        let fwd = vec3(1.0, 0.0, 0.0);
        let half = 60.0_f32.to_radians();
        assert!(chunk_visible(eye, fwd, half, 0, 0)); // own chunk
        assert!(chunk_visible(eye, fwd, half, 3, 0)); // ahead
        assert!(!chunk_visible(eye, fwd, half, -4, 0)); // behind
    }

    #[test]
    fn only_chunks_within_three_chunks_are_drawn() {
        let mut w = World::new(1, HashSet::new());
        let eye = vec3(10.0, 7.62, 10.0);
        w.stream_chunks(eye);
        w.rebuild_dirty(eye, usize::MAX);
        let wide = 170.0_f32.to_radians(); // effectively no cone culling
        let drawn = chunks_to_draw(&w, eye, vec3(1.0, 0.0, 0.0), wide);
        let side = (2 * render_radius() + 1) as usize;
        assert_eq!(drawn.len(), side * side);
        assert!(drawn.iter().all(|&(x, z)| x.abs() <= render_radius() && z.abs() <= render_radius()));
        // Generated-but-distant ring chunks exist yet are not drawn.
        assert!(w.chunks.len() > drawn.len());
        assert!(!drawn.contains(&(gen_radius(), 0)));

        // With a normal view cone, chunks directly behind are skipped too.
        let narrow = chunks_to_draw(&w, eye, vec3(1.0, 0.0, 0.0), 60.0_f32.to_radians());
        assert!(narrow.contains(&(0, 0)) && narrow.contains(&(render_radius(), 0)));
        assert!(!narrow.contains(&(-render_radius(), 0)));
    }
}
