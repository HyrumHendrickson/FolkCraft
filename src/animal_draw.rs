//! Drawing for animals and arrows: simple box models, posed each frame, so chunk meshes stay static.

use macroquad::prelude::*;

use crate::animal::{Animal, AnimalKind, Mode};
use crate::arrow::Arrow;
use crate::dropped::draw_box;
use crate::world::{in_render_range, World};

/// One box of an animal model: offset from the feet (x forward, y up, z to the side),
/// half-extents, and colour.
type Part = ((f32, f32, f32), (f32, f32, f32), Color);

fn col(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

/// The parts of a model, posed for the animal's current movement.
fn deer_parts(a: &Animal) -> Vec<Part> {
    let hide = col(0.60, 0.40, 0.22);
    let dark = col(0.40, 0.26, 0.14);
    let belly = col(0.82, 0.70, 0.52);
    let moving = if a.vel.x.abs() + a.vel.z.abs() > 0.1 { 1.0 } else { 0.0 };
    let mut parts: Vec<Part> = vec![
        ((0.0, 0.95, 0.0), (0.55, 0.27, 0.25), hide),
        ((0.0, 0.74, 0.0), (0.45, 0.06, 0.20), belly),
        ((0.60, 1.20, 0.0), (0.16, 0.32, 0.12), hide),
        ((0.82, 1.48, 0.0), (0.20, 0.14, 0.12), hide),
        ((1.00, 1.42, 0.0), (0.06, 0.07, 0.07), dark),
        ((0.74, 1.66, 0.09), (0.03, 0.10, 0.03), belly),
        ((0.74, 1.66, -0.09), (0.03, 0.10, 0.03), belly),
        ((-0.58, 1.08, 0.0), (0.05, 0.09, 0.07), col(0.95, 0.92, 0.85)),
    ];
    // Antlers on the head.
    for side in [-1.0, 1.0] {
        parts.push(((0.78, 1.82, 0.10 * side), (0.02, 0.14, 0.02), col(0.80, 0.74, 0.60)));
        parts.push(((0.80, 1.98, 0.17 * side), (0.02, 0.05, 0.07), col(0.80, 0.74, 0.60)));
    }
    // Four legs, swinging in diagonal pairs while the deer moves.
    for (i, &(lx, lz)) in [(0.40, 0.15), (0.40, -0.15), (-0.40, 0.15), (-0.40, -0.15)].iter().enumerate() {
        let swing = (a.phase * 2.2 + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * 0.22 * moving;
        parts.push(((lx + swing, 0.34, lz), (0.06, 0.34, 0.06), dark));
    }
    parts
}

fn rabbit_parts(a: &Animal) -> Vec<Part> {
    let fur = col(0.58, 0.48, 0.38);
    let light = col(0.86, 0.82, 0.76);
    let dark = col(0.34, 0.27, 0.20);
    let moving = if a.vel.x.abs() + a.vel.z.abs() > 0.1 { 1.0 } else { 0.0 };
    let hop = (a.phase * 3.0).sin().abs() * 0.06 * moving;
    let mut parts: Vec<Part> = vec![
        ((0.0, 0.24 + hop, 0.0), (0.20, 0.12, 0.11), fur),
        ((0.22, 0.34 + hop, 0.0), (0.09, 0.08, 0.07), fur),
        ((0.30, 0.34 + hop, 0.0), (0.025, 0.03, 0.03), dark),
        ((-0.21, 0.26 + hop, 0.0), (0.05, 0.05, 0.05), light),
        ((0.12, 0.30 + hop, 0.0), (0.05, 0.05, 0.08), light),
    ];
    for side in [-1.0, 1.0] {
        parts.push(((0.19, 0.50 + hop, 0.04 * side), (0.02, 0.10, 0.015), fur));
        parts.push(((-0.10, 0.10, 0.10 * side), (0.12, 0.05, 0.03), dark));
    }
    parts
}

fn moose_parts(a: &Animal) -> Vec<Part> {
    let hide = col(0.30, 0.22, 0.16);
    let shoulder = col(0.38, 0.28, 0.20);
    let muzzle = col(0.20, 0.15, 0.11);
    let leg = col(0.22, 0.17, 0.13);
    let horn = col(0.80, 0.74, 0.60);
    let moving = if a.vel.x.abs() + a.vel.z.abs() > 0.1 { 1.0 } else { 0.0 };
    // An enraged moose lowers its head.
    let head_y = if a.mode == Mode::Charge { 1.55 } else { 1.85 };
    let mut parts: Vec<Part> = vec![
        ((0.0, 1.30, 0.0), (0.72, 0.42, 0.38), hide),
        ((0.35, 1.80, 0.0), (0.34, 0.16, 0.30), shoulder),
        ((0.95, (1.45 + head_y) / 2.0, 0.0), (0.20, (head_y - 1.45) / 2.0 + 0.30, 0.18), hide),
        ((1.30, head_y, 0.0), (0.30, 0.20, 0.16), hide),
        ((1.58, head_y - 0.10, 0.0), (0.12, 0.12, 0.13), muzzle),
        ((-0.74, 1.45, 0.0), (0.05, 0.08, 0.06), shoulder),
    ];
    for side in [-1.0, 1.0] {
        // Wide, flat antlers.
        parts.push(((1.20, head_y + 0.22, 0.20 * side), (0.05, 0.04, 0.10), horn));
        parts.push(((1.18, head_y + 0.26, 0.48 * side), (0.14, 0.03, 0.22), horn));
        parts.push(((1.30, head_y + 0.28, 0.66 * side), (0.05, 0.03, 0.08), horn));
    }
    for (i, &(lx, lz)) in [(0.50, 0.24), (0.50, -0.24), (-0.50, 0.24), (-0.50, -0.24)].iter().enumerate() {
        let swing = (a.phase * 1.6 + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * 0.24 * moving;
        parts.push(((lx + swing, 0.45, lz), (0.09, 0.45, 0.09), leg));
    }
    parts
}

fn quail_parts(a: &Animal, time: f32) -> Vec<Part> {
    let feathers = col(0.56, 0.46, 0.34);
    let speckle = col(0.38, 0.30, 0.22);
    let mut parts: Vec<Part> = vec![
        ((0.0, 0.20, 0.0), (0.14, 0.10, 0.10), feathers),
        ((0.02, 0.27, 0.0), (0.10, 0.04, 0.08), speckle),
        ((0.17, 0.33, 0.0), (0.06, 0.06, 0.05), col(0.34, 0.26, 0.20)),
        ((0.24, 0.32, 0.0), (0.025, 0.02, 0.02), col(0.85, 0.65, 0.20)),
        ((0.19, 0.42, 0.0), (0.02, 0.05, 0.015), col(0.15, 0.12, 0.10)),
        ((-0.17, 0.25, 0.0), (0.06, 0.04, 0.05), speckle),
    ];
    if a.mode == Mode::Fly {
        // Wings flapping on both sides.
        let flap = (time * 32.0).sin();
        for side in [-1.0, 1.0] {
            parts.push(((0.0, 0.26 + 0.05 * flap, 0.17 * side), (0.09, 0.012, 0.13), feathers));
        }
    } else {
        for side in [-1.0, 1.0] {
            parts.push(((0.0, 0.22, 0.10 * side), (0.09, 0.05, 0.015), speckle));
            let step = (a.phase * 6.0 + if side > 0.0 { 0.0 } else { std::f32::consts::PI }).sin() * 0.03;
            parts.push(((step, 0.05, 0.04 * side), (0.012, 0.05, 0.012), col(0.85, 0.55, 0.20)));
        }
    }
    parts
}

fn bear_parts(a: &Animal) -> Vec<Part> {
    let fur = col(0.31, 0.21, 0.14);
    let shoulder = col(0.36, 0.25, 0.17);
    let snout = col(0.58, 0.45, 0.32);
    let dark = col(0.12, 0.09, 0.07);
    let moving = if a.vel.x.abs() + a.vel.z.abs() > 0.1 { 1.0 } else { 0.0 };
    // A charging bear lowers its head and its legs pump faster.
    let charging = a.mode == Mode::Charge;
    let (head_y, rate) = if charging { (0.88, 1.9) } else { (0.98, 1.4) };
    let mut parts: Vec<Part> = vec![
        ((0.0, 0.80, 0.0), (0.62, 0.38, 0.42), fur),
        ((0.30, 1.10, 0.0), (0.30, 0.16, 0.36), shoulder),
        ((0.90, head_y, 0.0), (0.26, 0.24, 0.22), fur),
        ((1.16, head_y - 0.08, 0.0), (0.14, 0.12, 0.12), snout),
        ((1.30, head_y - 0.03, 0.0), (0.04, 0.04, 0.05), dark),
        ((1.08, head_y + 0.07, 0.12), (0.02, 0.03, 0.02), dark),
        ((1.08, head_y + 0.07, -0.12), (0.02, 0.03, 0.02), dark),
        ((-0.66, 0.92, 0.0), (0.06, 0.06, 0.06), fur),
    ];
    for side in [-1.0, 1.0] {
        parts.push(((0.80, head_y + 0.28, 0.17 * side), (0.05, 0.06, 0.05), fur));
    }
    // Four thick legs, swinging in diagonal pairs while the bear moves.
    for (i, &(lx, lz)) in [(0.42, 0.26), (0.42, -0.26), (-0.42, 0.26), (-0.42, -0.26)].iter().enumerate() {
        let swing = (a.phase * rate + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * 0.20 * moving;
        parts.push(((lx + swing, 0.34, lz), (0.12, 0.34, 0.12), dark));
    }
    parts
}

/// Draw every animal within the render range. Call while the 3D camera is active.
pub fn draw_animals(animals: &[Animal], time: f32, player_chunk: (i32, i32)) {
    for a in animals {
        if !in_render_range(player_chunk, World::chunk_of_pos(a.pos)) {
            continue;
        }
        let parts = match a.kind {
            AnimalKind::Deer => deer_parts(a),
            AnimalKind::Quail => quail_parts(a, time),
            AnimalKind::Bear => bear_parts(a),
            AnimalKind::Rabbit => rabbit_parts(a),
            AnimalKind::Moose => moose_parts(a),
        };
        let (s, c) = a.heading.sin_cos();
        for ((fx, fy, fz), (hx, hy, hz), color) in parts {
            // Turn the model to face the heading: local x runs along it.
            let offset = vec3(fx * c - fz * s, fy, fx * s + fz * c);
            draw_box(a.pos + offset, vec3(hx, hy, hz), a.heading, color);
        }
    }
}

/// Draw the arrows in flight: a thin shaft with a stone head. Call while the 3D camera is active.
pub fn draw_arrows(arrows: &[Arrow]) {
    for arrow in arrows {
        let dir = arrow.vel.normalize_or_zero();
        draw_line_3d(arrow.pos - dir * 0.7, arrow.pos, col(0.62, 0.50, 0.30));
        draw_cube(arrow.pos, Vec3::splat(0.07), None, col(0.45, 0.45, 0.50));
    }
}
