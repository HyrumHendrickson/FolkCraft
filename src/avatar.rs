//! Other players, as drawn: a blocky figure that walks, swings what it holds and wears its name overhead.

use macroquad::prelude::*;

use crate::dropped::draw_box;
use crate::game::RemotePlayer;
use crate::item::ItemKind;
use crate::net::{item_from_wire, FLAG_SWINGING};

const SHIRTS: [(f32, f32, f32); 8] = [
    (0.20, 0.45, 0.85),
    (0.85, 0.30, 0.25),
    (0.25, 0.70, 0.35),
    (0.85, 0.65, 0.20),
    (0.60, 0.30, 0.75),
    (0.20, 0.70, 0.70),
    (0.90, 0.50, 0.70),
    (0.55, 0.55, 0.55),
];

/// The colour of a player's shirt: each id gets its own.
pub fn shirt_color(id: u32) -> Color {
    let (r, g, b) = SHIRTS[id as usize % SHIRTS.len()];
    Color::new(r, g, b, 1.0)
}

/// Draw every other player. Call while the 3D camera is active.
pub fn draw_players(players: &[RemotePlayer], time: f32, eye: Vec3) {
    for p in players {
        if (p.shown - eye).length() > 220.0 {
            continue;
        }
        draw_player(p, time);
    }
}

fn draw_player(p: &RemotePlayer, time: f32) {
    let yaw = p.state.yaw;
    let (s, c) = yaw.sin_cos();
    // A point in the player's own frame (x forward, y up, z to the right) as a place in the world.
    let at = |fx: f32, fy: f32, fz: f32| p.shown + vec3(fx * c - fz * s, fy, fx * s + fz * c);
    let moving = vec2(p.state.vel.x, p.state.vel.z).length() > 0.3;
    let swing = if moving { (p.walked * 2.6).sin() * 0.28 } else { 0.0 };
    let skin = Color::new(0.93, 0.76, 0.62, 1.0);
    let pants = Color::new(0.16, 0.18, 0.30, 1.0);
    let shirt = shirt_color(p.id);
    // Legs.
    draw_box(at(swing, 0.38, -0.13), vec3(0.12, 0.38, 0.12), yaw, pants);
    draw_box(at(-swing, 0.38, 0.13), vec3(0.12, 0.38, 0.12), yaw, pants);
    // Body and head.
    draw_box(at(0.0, 1.06, 0.0), vec3(0.17, 0.30, 0.27), yaw, shirt);
    draw_box(at(0.04, 1.62, 0.0), vec3(0.20, 0.20, 0.20), yaw, skin);
    draw_box(at(0.17, 1.66, 0.0), vec3(0.03, 0.05, 0.14), yaw, Color::new(0.1, 0.1, 0.12, 1.0));
    // Arms: the right one swings when the player is hitting something, the left one with the stride.
    let striking = p.state.flags & FLAG_SWINGING != 0;
    let strike = if striking { (time * 14.0).sin() * 0.45 + 0.35 } else { -swing };
    draw_box(at(-swing, 1.04, -0.38), vec3(0.09, 0.30, 0.09), yaw, skin);
    draw_box(at(strike, 1.04 + strike.abs() * 0.1, 0.38), vec3(0.09, 0.30, 0.09), yaw, skin);
    // What is held, in the right hand.
    if let Some(item) = item_from_wire(p.state.held) {
        draw_box(at(strike + 0.18, 0.86, 0.40), vec3(0.07, 0.07, 0.07), yaw, held_color(item));
    }
}

fn held_color(item: ItemKind) -> Color {
    item.color()
}

/// Where a point in the world lands on the screen for a camera at `eye` looking along `dir` (70 degrees of view), if it is in front.
pub fn project(eye: Vec3, dir: Vec3, point: Vec3, width: f32, height: f32) -> Option<Vec2> {
    let forward = dir.normalize_or_zero();
    let right = forward.cross(vec3(0.0, 1.0, 0.0)).normalize_or_zero();
    let up = right.cross(forward);
    let rel = point - eye;
    let depth = rel.dot(forward);
    if depth < 0.2 {
        return None;
    }
    let half = (35.0_f32).to_radians().tan();
    let aspect = width / height;
    let x = rel.dot(right) / (depth * half * aspect);
    let y = rel.dot(up) / (depth * half);
    Some(vec2(width / 2.0 * (1.0 + x), height / 2.0 * (1.0 - y)))
}

/// Each other player's name over their head, and a small health bar. Call with the default 2D camera.
pub fn draw_name_tags(players: &[RemotePlayer], eye: Vec3, dir: Vec3) {
    let (w, h) = (screen_width(), screen_height());
    for p in players {
        let head = p.shown + vec3(0.0, 2.05, 0.0);
        let dist = (head - eye).length();
        if dist > 120.0 {
            continue;
        }
        let Some(at) = project(eye, dir, head, w, h) else { continue };
        let size = (26.0 - dist * 0.12).clamp(15.0, 26.0);
        let d = measure_text(&p.name, None, size as u16, 1.0);
        draw_rectangle(at.x - d.width / 2.0 - 6.0, at.y - size, d.width + 12.0, size + 6.0, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_text(&p.name, at.x - d.width / 2.0, at.y - 2.0, size, lighter(shirt_color(p.id)));
        let bar = 46.0;
        let share = (p.state.health as f32 / crate::config::MAX_HEALTH as f32).clamp(0.0, 1.0);
        draw_rectangle(at.x - bar / 2.0, at.y + 8.0, bar, 5.0, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(at.x - bar / 2.0, at.y + 8.0, bar * share, 5.0, Color::new(0.85, 0.15, 0.2, 1.0));
    }
}

/// A colour brought most of the way to white, to read well over the world.
fn lighter(c: Color) -> Color {
    Color::new(c.r * 0.4 + 0.6, c.g * 0.4 + 0.6, c.b * 0.4 + 0.6, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_straight_ahead_is_in_the_middle_of_the_screen_and_one_behind_is_not_shown() {
        let eye = vec3(0.0, 1.6, 0.0);
        let dir = vec3(1.0, 0.0, 0.0);
        let mid = project(eye, dir, vec3(10.0, 1.6, 0.0), 1280.0, 720.0).unwrap();
        assert!((mid.x - 640.0).abs() < 0.5 && (mid.y - 360.0).abs() < 0.5, "{mid:?}");
        assert!(project(eye, dir, vec3(-10.0, 1.6, 0.0), 1280.0, 720.0).is_none());
        // Higher up is higher on the screen (smaller y); to the right is further right.
        let up = project(eye, dir, vec3(10.0, 3.6, 0.0), 1280.0, 720.0).unwrap();
        assert!(up.y < mid.y);
        let right = project(eye, dir, vec3(10.0, 1.6, 3.0), 1280.0, 720.0).unwrap();
        assert!(right.x > mid.x, "the player's right (z positive when facing +x) is the screen's right: {right:?}");
    }

    #[test]
    fn each_player_has_a_shirt_of_their_own_for_the_first_eight() {
        let colours: Vec<_> = (0..8).map(|i| { let c = shirt_color(i); (c.r.to_bits(), c.g.to_bits(), c.b.to_bits()) }).collect();
        let mut unique = colours.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 8);
    }
}
