//! Drawing for animals and arrows: simple box models, posed each frame, so chunk meshes stay static.

use macroquad::prelude::*;

use crate::animal::{Animal, AnimalKind, BossAction, HuntPhase, Mode};
use crate::arrow::Arrow;
use crate::config::*;
use crate::dropped::{draw_box, draw_box_tilted};
use crate::world::{in_render_range, World};

/// One box of an animal model: offset from the feet (x forward, y up, z to the side),
/// half-extents, and colour.
type Part = ((f32, f32, f32), (f32, f32, f32), Color);

fn col(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

/// A box of a posed model and the tilt it is drawn at (see `draw_box_tilted`).
type Piece = (Part, f32);

fn block(offset: (f32, f32, f32), half: (f32, f32, f32), color: Color) -> Piece {
    ((offset, half, color), 0.0)
}

fn tilted(offset: (f32, f32, f32), half: (f32, f32, f32), color: Color, pitch: f32) -> Piece {
    ((offset, half, color), pitch)
}

/// Whether the animal is moving along the ground (or in the air) right now.
fn is_moving(a: &Animal) -> f32 {
    if a.vel.x.abs() + a.vel.z.abs() > 0.1 { 1.0 } else { 0.0 }
}

/// The colour of a creature's eyes at night: a hot, glowing red.
pub const NIGHT_EYE: (f32, f32, f32) = (1.0, 0.02, 0.01);

/// Whether a drawn colour is the night-red of an eye or the halo round it.
#[cfg(test)]
pub fn is_night_eye(c: Color) -> bool {
    c.r > 0.9 && c.g < 0.1 && c.b < 0.1
}

/// Two eyes on the sides of a head whose middle is at `head` with half-width `half_z`. Each is a small box that sticks
/// out past the side of the head (so it can be seen, which a box buried in the head never could), dark by day and
/// glowing red at night, with a faint wider halo round it. `size` is the eye's half-extent.
fn eyes(parts: &mut Vec<Piece>, head: (f32, f32, f32), half_z: f32, size: f32, night: bool, day: Color) {
    for side in [-1.0, 1.0] {
        let at = (head.0, head.1, head.2 + side * (half_z + size * 0.3));
        if night {
            parts.push(block(at, (size, size * 1.2, size * 0.8), col(NIGHT_EYE.0, NIGHT_EYE.1, NIGHT_EYE.2)));
            parts.push(block(at, (size * 2.0, size * 2.2, size * 1.6), Color::new(NIGHT_EYE.0, NIGHT_EYE.1, NIGHT_EYE.2, 0.28)));
        } else {
            parts.push(block(at, (size, size * 1.2, size * 0.8), day));
        }
    }
}

/// A two-jointed leg hanging from `hip` (x forward, y up, z to the side): the upper segment swings by `angle` (positive
/// puts the foot forward), the lower follows with the knee bending as the leg goes back, and a hoof or paw ends it.
fn leg(parts: &mut Vec<Piece>, hip: (f32, f32, f32), upper: f32, lower: f32, thickness: f32, angle: f32, hide: Color, hoof: Color) {
    let lower_angle = if angle >= 0.0 { angle * 0.4 } else { angle - (-angle) * 0.9 };
    let knee = (hip.0 + angle.sin() * upper, hip.1 - angle.cos() * upper, hip.2);
    parts.push(tilted((hip.0 + angle.sin() * upper / 2.0, hip.1 - angle.cos() * upper / 2.0, hip.2), (thickness, upper / 2.0, thickness), hide, angle));
    let foot = (knee.0 + lower_angle.sin() * lower, knee.1 - lower_angle.cos() * lower, knee.2);
    parts.push(tilted(
        (knee.0 + lower_angle.sin() * lower / 2.0, knee.1 - lower_angle.cos() * lower / 2.0, knee.2),
        (thickness * 0.78, lower / 2.0, thickness * 0.78),
        hide,
        lower_angle,
    ));
    parts.push(tilted((foot.0, foot.1 + thickness * 0.4, foot.2), (thickness * 1.15, thickness * 0.5, thickness * 1.15), hoof, lower_angle));
}

/// How far a deer or moose has its head down grazing (0 head up, 1 right down): now and then, while it stands still.
fn graze(a: &Animal, time: f32) -> f32 {
    if a.mode == Mode::Idle { ((time * 0.35 + (a.pos.x * 1.7 + a.pos.z * 0.9)).sin() * 2.2).clamp(0.0, 1.0) } else { 0.0 }
}

/// The boxes of a deer, posed: it breathes, swings four two-jointed legs in diagonal pairs, bobs its head as it walks,
/// lowers it to graze when it stands, flicks its ears and tail, and rocks as it gallops.
fn deer_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let hide = col(0.60, 0.40, 0.22);
    let dark = col(0.36, 0.23, 0.12);
    let belly = col(0.84, 0.72, 0.54);
    let horn = col(0.80, 0.74, 0.60);
    let moving = is_moving(a);
    let running = if a.mode == Mode::Flee { 1.0 } else { 0.0 };
    let breath = (time * 2.0 + a.phase).sin() * 0.012;
    let gallop = (a.phase * 2.6).sin() * 0.07 * running;
    let bob = (a.phase * 2.2).sin().abs() * 0.04 * running;
    let g = graze(a, time);
    let rate = if running > 0.0 { 3.0 } else { 2.2 };
    let amp = if running > 0.0 { 0.75 } else { 0.38 };
    let mut parts = vec![
        tilted((0.0, 0.98 + breath + bob, 0.0), (0.55, 0.26 + breath, 0.24), hide, gallop),
        tilted((0.0, 0.76 + bob, 0.0), (0.46, 0.07, 0.19), belly, gallop),
        tilted((-0.38, 1.00 + bob, 0.0), (0.22, 0.27, 0.25), hide, gallop),
        tilted((0.38, 1.04 + bob, 0.0), (0.2, 0.27, 0.23), hide, gallop),
        tilted((-0.60, 1.13 + bob, 0.0), (0.05, 0.1, 0.05), col(0.95, 0.92, 0.85), 0.5 + (time * 6.0 + a.phase).sin() * 0.25 * (1.0 - moving)),
    ];
    // Neck and head: the head dips as the deer walks and drops right down to graze.
    let nod = (a.phase * 2.2).sin() * 0.06 * moving;
    let neck_pitch = -0.55 - 0.75 * g;
    let neck_top = (0.52 - neck_pitch.sin() * 0.0 + 0.18 + 0.30 * g, 1.2 + 0.36 - 0.7 * g + nod, 0.0);
    parts.push(tilted((0.64 + 0.12 * g, 1.34 - 0.30 * g + bob, 0.0), (0.11, 0.32, 0.10), hide, neck_pitch));
    let head = (neck_top.0 + 0.30, neck_top.1 + 0.05, 0.0);
    let head_pitch = -0.25 - 0.55 * g;
    parts.push(tilted(head, (0.20, 0.12, 0.11), hide, head_pitch));
    parts.push(tilted((head.0 + 0.21, head.1 - 0.05, 0.0), (0.09, 0.07, 0.075), dark, head_pitch));
    parts.push(tilted((head.0 + 0.29, head.1 - 0.04, 0.0), (0.025, 0.03, 0.04), col(0.08, 0.06, 0.05), head_pitch));
    eyes(&mut parts, (head.0 + 0.07, head.1 + 0.04, 0.0), 0.11, 0.028, night, col(0.06, 0.04, 0.03));
    let flick = (time * 0.9 + a.phase).sin().powi(15).max(0.0);
    for side in [-1.0, 1.0] {
        parts.push(tilted((head.0 - 0.10, head.1 + 0.15, 0.12 * side), (0.03, 0.10, 0.025), hide, 0.35 + flick * 0.9 * side.max(0.0) + flick * 0.5));
        parts.push(tilted((head.0 - 0.02, head.1 + 0.28, 0.08 * side), (0.02, 0.15, 0.02), horn, 0.2));
        parts.push(tilted((head.0 + 0.02, head.1 + 0.47, 0.15 * side), (0.02, 0.09, 0.02), horn, -0.35));
        parts.push(tilted((head.0 - 0.12, head.1 + 0.40, 0.13 * side), (0.02, 0.07, 0.02), horn, 0.7));
    }
    // Four legs, swinging in diagonal pairs while the deer moves.
    for (i, &(lx, lz)) in [(0.40, 0.15), (0.40, -0.15), (-0.40, 0.15), (-0.40, -0.15)].iter().enumerate() {
        let swing = (a.phase * rate + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * amp * moving;
        leg(&mut parts, (lx, 0.82 + bob, lz), 0.42, 0.42, 0.065, swing, hide, dark);
    }
    parts
}

/// A rabbit: it hops (the whole body leaps and tips nose-up as it rises), its hind legs kick out, its ears lie back as
/// it runs and twitch when it sits, and its nose works.
fn rabbit_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let fur = col(0.58, 0.48, 0.38);
    let light = col(0.88, 0.84, 0.78);
    let dark = col(0.34, 0.27, 0.20);
    let moving = is_moving(a);
    let running = if a.mode == Mode::Flee { 1.0 } else { 0.0 };
    let cycle = a.phase * 3.0;
    let hop = cycle.sin().abs() * (0.10 + 0.14 * running) * moving;
    let tip = cycle.cos() * (0.30 + 0.2 * running) * moving;
    let kick = cycle.sin() * 0.06 * moving;
    let breath = (time * 4.0 + a.phase).sin() * 0.01 * (1.0 - moving);
    let twitch = (time * 14.0 + a.phase).sin() * 0.03 * (1.0 - moving);
    let ear_back = 0.25 + 0.55 * moving.max(running) + (time * 1.1 + a.phase).sin().powi(9).max(0.0) * 0.5 * (1.0 - moving);
    let mut parts = vec![
        tilted((0.0, 0.25 + hop + breath, 0.0), (0.20, 0.12 + breath, 0.11), fur, -tip * 0.4),
        tilted((-0.10, 0.26 + hop, 0.0), (0.13, 0.13, 0.135), fur, -tip * 0.4),
        tilted((0.22, 0.36 + hop, 0.0), (0.09, 0.08, 0.075), fur, tip * 0.4 + twitch),
        tilted((0.30, 0.34 + hop + twitch, 0.0), (0.03, 0.03, 0.03), light, 0.0),
        block((-0.24, 0.29 + hop, 0.0), (0.06, 0.06, 0.06), light),
    ];
    eyes(&mut parts, (0.26, 0.40 + hop, 0.0), 0.075, 0.022, night, col(0.05, 0.04, 0.03));
    for side in [-1.0, 1.0] {
        parts.push(tilted((0.17, 0.56 + hop, 0.045 * side), (0.022, 0.12, 0.016), fur, ear_back));
        parts.push(tilted((0.17, 0.56 + hop, 0.045 * side), (0.012, 0.10, 0.018), col(0.85, 0.65, 0.62), ear_back));
        // Hind foot: long, thrown out behind at the top of the hop. Front paw: short.
        parts.push(block((-0.10 + kick, 0.06 + hop * 0.2, 0.11 * side), (0.14, 0.04, 0.035), dark));
        parts.push(tilted((-0.12 + kick * 0.6, 0.17 + hop * 0.6, 0.11 * side), (0.07, 0.10, 0.04), fur, 0.6 + kick * 3.0));
        parts.push(block((0.16 - kick * 0.5, 0.06 + hop * 0.6, 0.07 * side), (0.03, 0.06, 0.025), dark));
    }
    parts
}

/// A moose: tall and heavy, with a humped shoulder, a long head with a dewlap and a wide palmate rack of antlers. It
/// walks on long two-jointed legs, sways its head as it goes, grazes when it stands, and lowers the rack to charge.
fn moose_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let hide = col(0.30, 0.22, 0.16);
    let shoulder = col(0.38, 0.28, 0.20);
    let muzzle = col(0.20, 0.15, 0.11);
    let leg_color = col(0.22, 0.17, 0.13);
    let sock = col(0.55, 0.50, 0.42);
    let horn = col(0.82, 0.76, 0.62);
    let moving = is_moving(a);
    let charging: f32 = if a.mode == Mode::Charge || a.enraged { 1.0 } else { 0.0 };
    let breath = (time * 1.6 + a.phase).sin() * 0.015;
    let g = graze(a, time);
    let sway = (a.phase * 1.6).sin() * 0.05 * moving;
    let lower = (0.45 * charging).max(0.55 * g);
    let rock = (a.phase * 1.6).sin() * 0.03 * moving;
    let mut parts = vec![
        tilted((0.0, 1.38 + breath, 0.0), (0.72, 0.40 + breath, 0.38), hide, rock),
        tilted((0.42, 1.82, 0.0), (0.36, 0.22, 0.32), shoulder, rock),
        tilted((-0.55, 1.42, 0.0), (0.2, 0.34, 0.34), hide, rock),
        block((-0.76, 1.52, 0.0), (0.05, 0.10, 0.05), shoulder),
        block((0.62, 1.05, 0.0), (0.22, 0.12, 0.22), shoulder),
    ];
    // The neck rises from the hump; the long head hangs forward of it, lower when grazing or charging.
    let neck_pitch = -0.75 - lower * 0.9;
    parts.push(tilted((0.98, 1.85 - lower * 0.55, 0.0), (0.2, 0.36, 0.2), hide, neck_pitch));
    let head = (1.42 + lower * 0.12, 1.92 - lower * 0.95 + sway * 0.4, sway);
    let head_pitch = -0.55 - lower * 0.5;
    parts.push(tilted(head, (0.34, 0.18, 0.15), hide, head_pitch));
    parts.push(tilted((head.0 + 0.36, head.1 - 0.14, head.2), (0.16, 0.14, 0.13), muzzle, head_pitch));
    parts.push(block((head.0 + 0.18, head.1 - 0.38 + lower * 0.1, head.2), (0.05, 0.22, 0.04), muzzle));
    eyes(&mut parts, (head.0 + 0.04, head.1 + 0.07, head.2), 0.15, 0.032, night, col(0.06, 0.04, 0.03));
    for side in [-1.0, 1.0] {
        parts.push(tilted((head.0 - 0.25, head.1 + 0.22, head.2 + 0.19 * side), (0.05, 0.12, 0.04), hide, 0.9));
        // The rack: a stem, a wide flat palm and tines along its edge.
        parts.push(tilted((head.0 - 0.1, head.1 + 0.26, head.2 + 0.20 * side), (0.05, 0.05, 0.12), horn, 0.0));
        parts.push(tilted((head.0 - 0.12, head.1 + 0.32, head.2 + 0.52 * side), (0.17, 0.03, 0.26), horn, 0.0));
        for k in 0..4 {
            let kz = head.2 + side * (0.36 + 0.14 * k as f32);
            parts.push(tilted((head.0 + 0.06 - 0.01 * k as f32, head.1 + 0.36, kz), (0.05 + 0.02 * (k % 2) as f32, 0.05, 0.03), horn, 0.0));
        }
    }
    for (i, &(lx, lz)) in [(0.52, 0.26), (0.52, -0.26), (-0.52, 0.26), (-0.52, -0.26)].iter().enumerate() {
        let swing = (a.phase * 1.6 + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * (0.33 + 0.12 * charging) * moving;
        leg(&mut parts, (lx, 1.02, lz), 0.52, 0.5, 0.09, swing, leg_color, sock);
    }
    parts
}

/// A quail: a plump round body with a curled topknot, that bobs its head as it walks on thin legs and pecks at the
/// ground when it stands, flaps its wings in flight and tucks its legs up.
fn quail_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    bird_parts(a, time, night, Plumage {
        feathers: col(0.56, 0.46, 0.34),
        speckle: col(0.38, 0.30, 0.22),
        belly: col(0.80, 0.70, 0.52),
        face: col(0.34, 0.26, 0.20),
        beak: col(0.85, 0.65, 0.20),
        legs: col(0.85, 0.55, 0.20),
    })
}

/// A parrot: the quail's plump shape in bright colours: a red body and head, blue and yellow wings, a green back and a hooked pale beak.
fn parrot_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let mut parts = bird_parts(a, time, night, Plumage {
        feathers: col(0.85, 0.12, 0.12),
        speckle: col(0.15, 0.45, 0.85),
        belly: col(0.95, 0.80, 0.15),
        face: col(0.92, 0.88, 0.80),
        beak: col(0.95, 0.90, 0.78),
        legs: col(0.45, 0.45, 0.48),
    });
    // A green saddle and a long blue tail streak over the back of the bird.
    parts.push(tilted((-0.02, 0.32, 0.0), (0.12, 0.015, 0.09), col(0.12, 0.65, 0.22), 0.0));
    parts.push(tilted((-0.27, 0.25, 0.0), (0.10, 0.02, 0.04), col(0.15, 0.40, 0.90), 0.3));
    parts
}

/// A crow: the quail's shape in glossy black, with a dark beak and legs.
fn crow_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    bird_parts(a, time, night, Plumage {
        feathers: col(0.10, 0.10, 0.13),
        speckle: col(0.05, 0.05, 0.07),
        belly: col(0.17, 0.17, 0.21),
        face: col(0.07, 0.07, 0.09),
        beak: col(0.22, 0.20, 0.20),
        legs: col(0.14, 0.12, 0.12),
    })
}

/// The Iron Quail: a quail more than five times the size and made of metal. Its plumage is blue-grey steel in overlapping plates
/// with dark seams between them, a polished ridge along the back, rivets, a brass beak and dark iron feet, and a gleam of
/// light sweeps over it as the seconds pass, which is what makes steel look like steel when every box is flat-coloured.
fn iron_quail_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    metal_bird_parts(a, time, night, false)
}

/// The Bronze Parrot: the same armoured bird, but in warm brown-gold bronze with a green verdigris seam and a red crest.
fn bronze_parrot_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    metal_bird_parts(a, time, night, true)
}

/// A quail of metal, steel (`bronze` false) or bronze.
fn metal_bird_parts(a: &Animal, time: f32, night: bool, bronze: bool) -> Vec<Piece> {
    const SCALE: f32 = 5.5;
    let mut parts = bird_parts(a, time, night, if bronze {
        Plumage {
            feathers: col(0.62, 0.42, 0.18),
            speckle: col(0.40, 0.26, 0.10),
            belly: col(0.80, 0.60, 0.28),
            face: col(0.30, 0.20, 0.10),
            beak: col(0.12, 0.10, 0.08),
            legs: col(0.36, 0.26, 0.12),
        }
    } else {
        Plumage {
            feathers: col(0.45, 0.49, 0.56),
            speckle: col(0.25, 0.28, 0.34),
            belly: col(0.67, 0.71, 0.78),
            face: col(0.19, 0.21, 0.26),
            beak: col(0.86, 0.64, 0.24),
            legs: col(0.30, 0.31, 0.35),
        }
    });
    for ((off, half, _), _) in parts.iter_mut() {
        *off = (off.0 * SCALE, off.1 * SCALE, off.2 * SCALE);
        *half = (half.0 * SCALE, half.1 * SCALE, half.2 * SCALE);
    }
    // The gleam: a bright band travelling along the bird that lightens whatever steel it crosses.
    let gleam = |x: f32, y: f32| (0.5 + 0.5 * (time * 2.2 - x * 1.1 + y * 0.7).sin()).powi(8);
    for ((off, _, c), _) in parts.iter_mut() {
        if c.a < 1.0 || (c.r > 0.9 && c.g < 0.1 && c.b < 0.1) {
            continue;
        }
        let k = gleam(off.0, off.1) * 0.5;
        *c = Color::new(c.r + (1.0 - c.r) * k, c.g + (1.0 - c.g) * k, c.b + (1.0 - c.b) * k, 1.0);
    }
    let flying = a.mode == Mode::Fly || a.is_airborne();
    let lean: f32 = if flying { -0.45 } else { 0.0 };
    let (sl, cl) = lean.sin_cos();
    // A point on the (tilted) body, given relative to the middle of its torso.
    let body = |x: f32, y: f32, z: f32| (x * cl - y * sl, 1.155 + x * sl + y * cl, z);
    let polish = |x: f32, y: f32| {
        let k = 0.55 + 0.45 * gleam(x, y);
        if bronze {
            col(0.80 + 0.2 * k, 0.55 + 0.25 * k, 0.22 + 0.2 * k)
        } else {
            col(0.78 + 0.2 * k, 0.82 + 0.17 * k, 0.88 + 0.12 * k)
        }
    };
    let seam = if bronze { col(0.16, 0.34, 0.26) } else { col(0.10, 0.11, 0.14) };
    // A polished ridge along the back, with a rivet on either side of it every few steps.
    parts.push(tilted(body(0.05, 0.55, 0.0), (0.72, 0.045, 0.30), polish(0.0, 0.6), lean));
    for i in 0..6 {
        let x = -0.62 + 0.25 * i as f32;
        for side in [-1.0, 1.0] {
            let (px, py, pz) = body(x, 0.58, 0.40 * side);
            parts.push(block((px, py, pz), (0.055, 0.055, 0.055), polish(x, 0.6)));
        }
    }
    // Overlapping plates down the flanks, each divided from the next by a dark seam and edged with a bright lip.
    for i in 0..5 {
        let x = -0.58 + 0.29 * i as f32;
        for side in [-1.0, 1.0] {
            let (sx, sy, sz) = body(x, 0.0, 0.64 * side);
            parts.push(tilted((sx, sy, sz), (0.012, 0.52, 0.05), seam, lean));
            let (lx, ly, lz) = body(x + 0.14, 0.18, 0.65 * side);
            parts.push(tilted((lx, ly, lz), (0.12, 0.05, 0.02), polish(x, 0.2), lean));
        }
    }
    // Armour on the head: a brow plate over the eyes and a steel crest instead of feathers.
    let head = body(0.95, 0.38, 0.0);
    parts.push(tilted((head.0, head.1 + 0.30, 0.0), (0.18, 0.04, 0.26), polish(0.9, 0.9), lean));
    for i in 0..3 {
        let t = i as f32;
        parts.push(tilted((head.0 - 0.10 - 0.10 * t, head.1 + 0.50 + 0.04 * t, 0.0), (0.04, 0.22 - 0.03 * t, 0.025), polish(0.8 - 0.1 * t, 1.0), 0.6 + 0.1 * t));
    }
    metal_bird_features(&mut parts, a, time, bronze, flying, lean, &body, &polish);
    parts
}

/// What makes the big metal birds read as birds rather than as armoured blocks: a ruff of neck feathers, a hooked beak, folded wings of
/// layered flight feathers (or, in the air, long primaries spread past the wing), a fanned tail of long quills, and taloned feet.
/// `body` places a point on the tilted torso and `polish` gives the burnished colour at a point (see `metal_bird_parts`).
#[allow(clippy::too_many_arguments)]
fn metal_bird_features(parts: &mut Vec<Piece>, a: &Animal, time: f32, bronze: bool, flying: bool, lean: f32, body: &dyn Fn(f32, f32, f32) -> (f32, f32, f32), polish: &dyn Fn(f32, f32) -> Color) {
    let beak = if bronze { col(0.16, 0.12, 0.08) } else { col(0.86, 0.64, 0.24) };
    let talon = if bronze { col(0.20, 0.15, 0.10) } else { col(0.20, 0.21, 0.25) };
    let dark = if bronze { col(0.34, 0.22, 0.09) } else { col(0.30, 0.33, 0.40) };
    // A ruff of feathers round the base of the neck.
    for k in 0..3 {
        let (x, y, z) = body(0.72 + 0.05 * k as f32, 0.22 - 0.10 * k as f32, 0.0);
        parts.push(tilted((x, y, z), (0.12, 0.20 - 0.03 * k as f32, 0.30 - 0.04 * k as f32), if k % 2 == 0 { polish(0.7, 0.2) } else { dark }, lean));
    }
    // The hooked beak: a deep upper bill with a hook curving down at its tip.
    let head = body(0.95, 0.38, 0.0);
    parts.push(tilted((head.0 + 0.50, head.1 - 0.10, 0.0), (0.26, 0.10, 0.09), beak, lean));
    parts.push(tilted((head.0 + 0.72, head.1 - 0.24, 0.0), (0.06, 0.16, 0.07), beak, lean + 0.35));
    // The tail: a fan of five long quills, the middle ones longest.
    for k in -2i32..=2 {
        let len = 0.62 - 0.07 * k.abs() as f32;
        let (x, y, z) = body(-0.95 - len * 0.5, 0.10 - 0.03 * k.abs() as f32, 0.15 * k as f32);
        parts.push(tilted((x, y, z), (len, 0.03, 0.07), if k % 2 == 0 { polish(-0.9, 0.1) } else { dark }, lean + 0.05));
    }
    if flying {
        // Long primaries fanned out from the tips of the beating wings.
        let flap = (time * 24.0).sin();
        for side in [-1.0, 1.0] {
            for k in 0..4 {
                let (x, y, z) = body(-0.05 - 0.22 * k as f32, 0.55 + 0.35 * flap, side * (1.55 + 0.30 * k as f32));
                parts.push(tilted((x, y, z), (0.30, 0.03, 0.22), if k % 2 == 0 { polish(0.0, 0.5) } else { dark }, lean - flap * 0.25));
            }
        }
        // Feet tucked up, talons curled.
        for side in [-1.0, 1.0] {
            let (x, y, z) = body(-0.10, -0.62, 0.25 * side);
            for t in -1i32..=1 {
                parts.push(block((x + 0.20, y - 0.04, z + 0.07 * t as f32), (0.18, 0.03, 0.025), talon));
            }
        }
    } else {
        // Wings folded along the flanks: three rows of flight feathers, each longer than the last and swept back past the tail.
        for side in [-1.0, 1.0] {
            for row in 0..3 {
                let r = row as f32;
                let (x, y, z) = body(-0.20 - 0.22 * r, 0.12 - 0.17 * r, 0.70 * side);
                parts.push(tilted((x, y, z), (0.52 + 0.14 * r, 0.075, 0.045), if row % 2 == 0 { polish(-0.2 - 0.2 * r, 0.1) } else { dark }, lean + 0.22));
            }
        }
        // Taloned feet: three toes forward and one back.
        let moving = is_moving(a);
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let step = (a.phase * 6.0 + if i == 0 { 0.0 } else { std::f32::consts::PI }).sin() * 0.30 * moving;
            for t in -1i32..=1 {
                parts.push(block((0.30 + step, 0.05, 0.25 * side + 0.13 * t as f32), (0.28, 0.045, 0.04), talon));
            }
            parts.push(block((-0.22 + step, 0.05, 0.25 * side), (0.14, 0.04, 0.04), talon));
        }
    }
}

/// A newt: a low, long-tailed amphibian (drawn `NEWT_MODEL_SCALE` times its first size), dark olive above and orange beneath, that sways its body as it creeps along.
fn newt_parts(a: &Animal, _time: f32, night: bool) -> Vec<Piece> {
    // A bright olive-yellow back with an orange stripe down it: the moss floor is green, so a dark green newt cannot be seen on it.
    let back = col(0.62, 0.66, 0.14);
    let belly = col(0.95, 0.52, 0.12);
    let stripe = col(1.0, 0.42, 0.08);
    let dark = col(0.09, 0.11, 0.05);
    let moving = is_moving(a);
    let sway = (a.phase * 6.0).sin() * moving;
    let mut parts = Vec::new();
    // Body and tail: a chain of segments, each swaying a little more than the one before.
    for i in 0..4 {
        let x = 0.12 - 0.08 * i as f32;
        let z = sway * 0.012 * i as f32;
        parts.push(block((x, 0.075, z), (0.05, 0.04, 0.04), back));
        parts.push(block((x, 0.034, z), (0.045, 0.012, 0.035), belly));
        parts.push(block((x, 0.118, z), (0.04, 0.006, 0.012), stripe));
    }
    for i in 0..5 {
        let x = -0.22 - 0.065 * i as f32;
        let z = sway * 0.02 * (i as f32 + 3.0);
        parts.push(block((x, 0.065 - 0.004 * i as f32, z), (0.04, 0.03 - 0.004 * i as f32, 0.014), back));
    }
    // A flat head with eyes on top, and a stripe of orange under the jaw.
    parts.push(block((0.21, 0.075, -sway * 0.012), (0.055, 0.032, 0.045), back));
    parts.push(block((0.21, 0.04, -sway * 0.012), (0.05, 0.01, 0.04), belly));
    eyes(&mut parts, (0.235, 0.11, -sway * 0.012), 0.032, 0.014, night, dark);
    // Four splayed legs, stepping in diagonal pairs.
    for (x, side, offset) in [(0.12, 1.0f32, 0.0f32), (0.12, -1.0, std::f32::consts::PI), (-0.06, 1.0, std::f32::consts::PI), (-0.06, -1.0, 0.0)] {
        let step = (a.phase * 6.0 + offset).sin() * 0.035 * moving;
        parts.push(block((x + step, 0.032, 0.085 * side), (0.032, 0.014, 0.022), back));
        parts.push(block((x + step + 0.02, 0.014, 0.10 * side), (0.03, 0.01, 0.02), dark));
    }
    scaled(parts, NEWT_MODEL_SCALE)
}

/// A fairy: a tiny pale figure in a petal dress with a head of bright hair and four wings that beat fast, with a soft glow about it.
fn fairy_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let skin = col(0.98, 0.86, 0.78);
    let dress = if a.enraged { col(0.90, 0.35, 0.55) } else { col(0.62, 0.82, 1.0) };
    let hair = col(0.98, 0.78, 0.30);
    let dark = col(0.12, 0.08, 0.10);
    let beat = (time * 24.0 + a.phase * 3.0).sin();
    let bob = (time * 3.0 + a.phase).sin() * 0.02;
    let mut parts = vec![
        block((0.0, 0.18 + bob, 0.0), (0.07, 0.05, 0.07), dress),
        block((0.0, 0.28 + bob, 0.0), (0.05, 0.07, 0.045), dress),
        block((0.0, 0.46 + bob, 0.0), (0.06, 0.06, 0.06), skin),
        block((-0.02, 0.50 + bob, 0.0), (0.065, 0.045, 0.065), hair),
        block((0.0, 0.10 + bob, 0.0), (0.03, 0.04, 0.03), skin),
    ];
    eyes(&mut parts, (0.05, 0.47 + bob, 0.0), 0.055, 0.012, night, dark);
    // Arms out at the sides.
    for side in [-1.0, 1.0] {
        parts.push(block((0.0, 0.30 + bob, 0.085 * side), (0.018, 0.04, 0.018), skin));
    }
    // Two wings a side, translucent, beating up and down.
    let wing = Color::new(0.85, 0.95, 1.0, 0.55);
    for side in [-1.0f32, 1.0] {
        let flap = beat * 0.7 * side;
        parts.push(tilted((-0.03, 0.40 + bob + 0.05 * beat.abs(), 0.14 * side), (0.025, 0.12, 0.08), wing, flap));
        parts.push(tilted((-0.03, 0.30 + bob + 0.03 * beat.abs(), 0.11 * side), (0.02, 0.07, 0.05), wing, flap * 0.8));
    }
    // A faint glow, brighter at night.
    parts.push(block((0.0, 0.32 + bob, 0.0), (0.22, 0.26, 0.22), Color::new(0.80, 0.92, 1.0, if night { 0.22 } else { 0.08 })));
    parts
}

/// A mountain goat: a white, shaggy body on short sure legs, a beard, a pair of curved ridged horns and cloven hooves. It lowers its head
/// when it charges.
fn goat_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let fur = col(0.93, 0.92, 0.88);
    let shade = col(0.80, 0.78, 0.72);
    let horn = col(0.42, 0.36, 0.28);
    let hoof = col(0.14, 0.12, 0.10);
    let nose = col(0.30, 0.26, 0.24);
    let moving = is_moving(a);
    let charging: f32 = if a.enraged { 1.0 } else { 0.0 };
    let breath = (time * 1.8 + a.phase).sin() * 0.012;
    let rock = (a.phase * 2.0).sin() * 0.03 * moving;
    let lower = 0.35 * charging + 0.4 * graze(a, time);
    let mut parts = vec![
        tilted((0.0, 0.66 + breath, 0.0), (0.46, 0.24 + breath, 0.22), fur, rock),
        tilted((0.05, 0.60, 0.0), (0.38, 0.16, 0.23), shade, rock),
        tilted((-0.46, 0.70, 0.0), (0.10, 0.18, 0.15), fur, rock),
        block((-0.58, 0.82, 0.0), (0.04, 0.07, 0.03), fur),
    ];
    // Neck, head with a beard, muzzle and ears.
    parts.push(tilted((0.50, 0.90 - lower * 0.3, 0.0), (0.12, 0.22, 0.12), fur, -0.7 - lower));
    let head = (0.72 + lower * 0.05, 1.00 - lower * 0.45, 0.0);
    parts.push(tilted(head, (0.16, 0.12, 0.11), fur, -0.35 - lower * 0.5));
    parts.push(tilted((head.0 + 0.17, head.1 - 0.07, 0.0), (0.09, 0.07, 0.07), nose, -0.35 - lower * 0.5));
    parts.push(block((head.0 + 0.10, head.1 - 0.26, 0.0), (0.025, 0.10, 0.025), shade));
    eyes(&mut parts, (head.0 + 0.03, head.1 + 0.03, 0.0), 0.11, 0.022, night, col(0.08, 0.06, 0.04));
    for side in [-1.0f32, 1.0] {
        parts.push(tilted((head.0 - 0.10, head.1 + 0.08, 0.14 * side), (0.04, 0.02, 0.07), fur, 0.0));
        // The horn sweeps up and back in three stages.
        parts.push(tilted((head.0 - 0.06, head.1 + 0.16, 0.07 * side), (0.03, 0.09, 0.03), horn, 0.2));
        parts.push(tilted((head.0 - 0.14, head.1 + 0.30, 0.08 * side), (0.025, 0.08, 0.025), horn, 0.6));
        parts.push(tilted((head.0 - 0.24, head.1 + 0.38, 0.08 * side), (0.02, 0.05, 0.02), horn, 1.0));
    }
    for (i, &(lx, lz)) in [(0.34, 0.14), (0.34, -0.14), (-0.34, 0.14), (-0.34, -0.14)].iter().enumerate() {
        let swing = (a.phase * 2.0 + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * (0.35 + 0.1 * charging) * moving;
        leg(&mut parts, (lx, 0.46, lz), 0.24, 0.22, 0.045, swing, shade, hoof);
    }
    parts
}

/// A crawfish: a segmented body with a fan tail, two big claws, feelers, stalked eyes and walking legs. The three kinds share the model; `k`
/// is how much bigger than the small one it is drawn and `shell` its colour.
fn crawfish_model(a: &Animal, night: bool, k: f32, shell: Color) -> Vec<Piece> {
    let belly = col(shell.r * 1.2 + 0.1, shell.g * 1.3 + 0.1, shell.b * 1.3 + 0.08);
    let dark = col(shell.r * 0.45, shell.g * 0.4, shell.b * 0.4);
    let moving = is_moving(a);
    let scuttle = (a.phase * 7.0).sin() * moving;
    let mut parts = Vec::new();
    // Head-and-chest shield, then the jointed abdomen and the fan of the tail.
    parts.push(block((0.08, 0.10, 0.0), (0.16, 0.07, 0.10), shell));
    parts.push(block((0.08, 0.04, 0.0), (0.14, 0.02, 0.08), belly));
    parts.push(block((0.27, 0.095, 0.0), (0.05, 0.05, 0.05), shell));
    for i in 0..4 {
        let x = -0.14 - 0.085 * i as f32;
        parts.push(block((x, 0.085 - 0.004 * i as f32, 0.0), (0.045, 0.055 - 0.006 * i as f32, 0.085 - 0.012 * i as f32), if i % 2 == 0 { shell } else { dark }));
    }
    for side in [-1.0f32, 1.0] {
        parts.push(tilted((-0.52, 0.06, 0.05 * side), (0.05, 0.012, 0.035), shell, 0.0));
    }
    parts.push(block((-0.54, 0.055, 0.0), (0.05, 0.012, 0.03), shell));
    // Stalked eyes and long feelers.
    eyes(&mut parts, (0.30, 0.15, 0.0), 0.035, 0.016, night, col(0.03, 0.03, 0.03));
    for side in [-1.0f32, 1.0] {
        parts.push(block((0.40, 0.12, 0.03 * side + 0.02 * side * scuttle), (0.13, 0.006, 0.006), dark));
    }
    // Two big claws held out in front, snapping a little as it moves.
    for side in [-1.0f32, 1.0] {
        let snap = (a.phase * 5.0 + side).sin() * 0.02 * moving;
        parts.push(block((0.30, 0.08, 0.14 * side), (0.12, 0.025, 0.025), shell));
        parts.push(block((0.46, 0.08, 0.17 * side), (0.07, 0.03, 0.045 + snap), shell));
        parts.push(block((0.52, 0.08, 0.16 * side - 0.03 * side), (0.05, 0.02, 0.02), dark));
    }
    // Eight walking legs stepping in diagonal pairs.
    for (n, x) in [0.14f32, 0.06, -0.02, -0.10].into_iter().enumerate() {
        for side in [-1.0f32, 1.0] {
            let step = (a.phase * 7.0 + n as f32 * 1.3 + if side > 0.0 { 0.0 } else { std::f32::consts::PI }).sin() * 0.03 * moving;
            parts.push(block((x + step, 0.035, 0.13 * side), (0.012, 0.012, 0.07), dark));
        }
    }
    scaled(parts, k)
}

/// A small river crawfish: red-brown.
fn crawfish_parts(a: &Animal, _time: f32, night: bool) -> Vec<Piece> {
    crawfish_model(a, night, 1.0, col(0.62, 0.20, 0.12))
}

/// A big cave crawfish: `BIG_CRAWFISH_MODEL_SCALE` times as big, a brighter orange-red.
fn big_crawfish_parts(a: &Animal, _time: f32, night: bool) -> Vec<Piece> {
    crawfish_model(a, night, BIG_CRAWFISH_MODEL_SCALE, col(0.80, 0.26, 0.10))
}

/// The Giant Crawfish: `GIANT_CRAWFISH_MODEL_SCALE` times a small one, a deep crimson.
fn giant_crawfish_parts(a: &Animal, _time: f32, night: bool) -> Vec<Piece> {
    crawfish_model(a, night, GIANT_CRAWFISH_MODEL_SCALE, col(0.58, 0.07, 0.07))
}

/// The Dragon: a long green-black body with a pale belly, a neck and horned head whose jaw drops while it breathes fire, a tail that
/// sways, and two huge wings that beat. Built about one block long and drawn `DRAGON_MODEL_SCALE` times that, the biggest creature in the game.
fn dragon_parts(a: &Animal, time: f32, _night: bool) -> Vec<Piece> {
    let scale = col(0.14, 0.34, 0.20);
    let dark = col(0.07, 0.17, 0.12);
    let belly = col(0.84, 0.72, 0.40);
    let horn = col(0.92, 0.88, 0.74);
    let membrane = col(0.50, 0.12, 0.14);
    let bone = col(0.10, 0.22, 0.15);
    let flap = (time * 2.6 + a.phase * 0.02).sin();
    let breathing = a.action == BossAction::Breathe;
    let sway = (time * 1.8).sin();
    let mut parts = vec![
        block((0.0, 0.34, 0.0), (0.40, 0.17, 0.18), scale),
        block((0.0, 0.22, 0.0), (0.37, 0.06, 0.14), belly),
        block((0.05, 0.52, 0.0), (0.22, 0.03, 0.04), dark),
    ];
    // A ridge of spines down the back.
    for i in 0..6 {
        parts.push(block((0.30 - 0.12 * i as f32, 0.54, 0.0), (0.025, 0.04, 0.02), horn));
    }
    // Neck and head: rising in front of the body, the head level, a lower jaw that drops when it breathes.
    parts.push(tilted((0.48, 0.44, 0.0), (0.17, 0.10, 0.11), scale, -0.45));
    parts.push(tilted((0.64, 0.58, 0.0), (0.15, 0.09, 0.10), scale, -0.30));
    let head = (0.84, 0.64, 0.0);
    parts.push(block(head, (0.17, 0.09, 0.12), scale));
    parts.push(block((head.0 + 0.19, head.1 - 0.015, 0.0), (0.13, 0.06, 0.085), dark));
    let drop = if breathing { 0.11 } else { 0.0 };
    parts.push(tilted((head.0 + 0.14, head.1 - 0.085 - drop, 0.0), (0.15, 0.025, 0.08), belly, if breathing { 0.35 } else { 0.0 }));
    if breathing {
        parts.push(block((head.0 + 0.20, head.1 - 0.05, 0.0), (0.08, 0.03, 0.05), col(1.0, 0.55, 0.10)));
    }
    for side in [-1.0f32, 1.0] {
        parts.push(tilted((head.0 - 0.10, head.1 + 0.14, 0.08 * side), (0.12, 0.022, 0.022), horn, 0.5));
        parts.push(block((head.0 + 0.02, head.1 + 0.10, 0.10 * side), (0.05, 0.02, 0.02), dark));
    }
    eyes(&mut parts, (head.0 + 0.06, head.1 + 0.05, 0.0), 0.12, 0.03, true, col(1.0, 0.82, 0.10));
    // Tail: segments shrinking backwards and swinging from side to side.
    for i in 0..7 {
        let t = i as f32;
        let x = -0.46 - 0.17 * t;
        let z = sway * 0.06 * t * (1.0 + 0.12 * t);
        parts.push(block((x, 0.32 - 0.02 * t, z), (0.10, 0.12 - 0.014 * t, 0.12 - 0.014 * t), if i % 2 == 0 { scale } else { dark }));
    }
    parts.push(block((-1.72, 0.18, sway * 0.5), (0.05, 0.05, 0.05), horn));
    // Four short legs tucked under it.
    for (x, side) in [(0.22f32, 1.0f32), (0.22, -1.0), (-0.22, 1.0), (-0.22, -1.0)] {
        parts.push(block((x, 0.12, 0.17 * side), (0.05, 0.09, 0.05), dark));
        parts.push(block((x + 0.04, 0.025, 0.17 * side), (0.08, 0.025, 0.045), horn));
    }
    // Two wings: an arm out from the shoulder with panels of membrane that beat up and down together.
    for side in [-1.0f32, 1.0] {
        parts.push(tilted((0.14, 0.50 + 0.12 * flap, 0.30 * side), (0.035, 0.025, 0.30), bone, 0.3 * flap * side));
        for i in 0..4 {
            let t = i as f32;
            let reach = 0.30 + 0.30 * t;
            parts.push(tilted((0.10 - 0.07 * t, 0.50 + (0.12 + 0.07 * t) * flap, reach * side), (0.17 - 0.02 * t, 0.012, 0.15), membrane, 0.35 * flap * side));
            parts.push(tilted((0.16 - 0.07 * t, 0.50 + (0.12 + 0.07 * t) * flap, reach * side), (0.015, 0.014, 0.15), bone, 0.35 * flap * side));
        }
    }
    scaled(parts, DRAGON_MODEL_SCALE)
}

/// The Unicorn: a tall white horse with a golden spiralled horn, a mane and tail in three pastel colours, and golden hooves. It
/// rears its head and gallops when it charges, and stands calmly (tail swishing) while the player is in the jungle.
fn unicorn_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let coat = col(0.97, 0.97, 0.99);
    let shade = col(0.84, 0.86, 0.92);
    let hoof = col(0.88, 0.74, 0.34);
    let horn = col(1.0, 0.86, 0.32);
    let dark = col(0.16, 0.14, 0.20);
    let manes = [col(0.96, 0.46, 0.72), col(0.52, 0.60, 1.0), col(0.50, 0.92, 0.76)];
    let moving = is_moving(a);
    let charging = a.mode == Mode::Charge;
    let (rate, amp) = if charging { (2.6, 0.75) } else { (1.5, 0.40) };
    let breath = (time * 1.6 + a.phase).sin() * 0.015;
    let gallop = if charging { (a.phase * rate).sin() * 0.07 } else { 0.0 };
    let swish = (time * 1.3 + a.phase).sin() * 0.25;
    let mut parts = vec![
        tilted((0.0, 1.15 + breath, 0.0), (0.62, 0.30 + breath, 0.27), coat, gallop),
        tilted((0.45, 1.22, 0.0), (0.26, 0.34, 0.26), coat, gallop),
        tilted((-0.42, 1.18, 0.0), (0.26, 0.32, 0.28), shade, gallop),
    ];
    // A long arched neck and a fine head held high (lowered a little for the charge), with a gold horn between the ears.
    let up = if charging { 0.0 } else { 0.12 };
    parts.push(tilted((0.78, 1.58 + up * 0.3, 0.0), (0.15, 0.34, 0.13), coat, -0.55));
    let head = (1.02, 1.86 + up - if charging { 0.12 } else { 0.0 }, 0.0);
    parts.push(tilted(head, (0.24, 0.13, 0.12), coat, -0.30));
    parts.push(tilted((head.0 + 0.24, head.1 - 0.10, 0.0), (0.12, 0.09, 0.10), shade, -0.30));
    parts.push(block((head.0 + 0.34, head.1 - 0.14, 0.0), (0.03, 0.03, 0.07), dark));
    eyes(&mut parts, (head.0 + 0.08, head.1 + 0.04, 0.0), 0.12, 0.028, night, dark);
    for side in [-1.0, 1.0] {
        parts.push(block((head.0 - 0.10, head.1 + 0.18, 0.07 * side), (0.03, 0.08, 0.025), coat));
    }
    for i in 0..4 {
        let t = i as f32;
        parts.push(tilted((head.0 + 0.10 + 0.07 * t, head.1 + 0.30 + 0.16 * t, 0.0), (0.035 - 0.006 * t, 0.12, 0.035 - 0.006 * t), if i % 2 == 0 { horn } else { col(0.96, 0.72, 0.20) }, -0.45));
    }
    // A mane down the neck and a tail streaming behind, both in stripes of colour.
    for i in 0..6 {
        let t = i as f32;
        parts.push(tilted((0.92 - 0.10 * t, 1.84 - 0.12 * t + up * 0.3, 0.0), (0.05, 0.14, 0.06), manes[i % 3], -0.55));
    }
    for i in 0..5 {
        let t = i as f32;
        parts.push(tilted((-0.72 - 0.06 * t, 1.20 - 0.17 * t, swish * 0.1 * t), (0.05, 0.18, 0.07), manes[i % 3], 0.25 + swish * 0.2));
    }
    for (i, &(lx, lz)) in [(0.46, 0.17), (0.46, -0.17), (-0.42, 0.17), (-0.42, -0.17)].iter().enumerate() {
        let swing = (a.phase * rate + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * amp * moving;
        leg(&mut parts, (lx, 0.88, lz), 0.46, 0.44, 0.075, swing, coat, hoof);
    }
    parts
}

/// The boxes of a model, all `k` times the size.
fn scaled(mut parts: Vec<Piece>, k: f32) -> Vec<Piece> {
    for ((off, half, _), _) in parts.iter_mut() {
        *off = (off.0 * k, off.1 * k, off.2 * k);
        *half = (half.0 * k, half.1 * k, half.2 * k);
    }
    parts
}

/// A raven: a crow half as big again, black with a blue sheen, always on the wing.
fn raven_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    scaled(
        bird_parts(a, time, night, Plumage {
            feathers: col(0.06, 0.06, 0.09),
            speckle: col(0.03, 0.03, 0.05),
            belly: col(0.10, 0.11, 0.17),
            face: col(0.04, 0.04, 0.06),
            beak: col(0.16, 0.15, 0.16),
            legs: col(0.09, 0.08, 0.08),
        }),
        1.6,
    )
}

/// The Phoenix: a bird of fire many times the size of a quail, orange and gold, with long trailing tail plumes that wave and a
/// crest of flame, its wings beating.
fn phoenix_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let mut parts = scaled(
        bird_parts(a, time, night, Plumage {
            feathers: col(0.96, 0.38, 0.08),
            speckle: col(0.80, 0.16, 0.05),
            belly: col(1.0, 0.74, 0.20),
            face: col(0.92, 0.30, 0.06),
            beak: col(1.0, 0.86, 0.30),
            legs: col(0.92, 0.62, 0.12),
        }),
        6.0,
    );
    // Tail plumes streaming out behind, fading from red to gold, and a crest of flame on the head.
    for i in 0..5 {
        let t = i as f32;
        let sway = (time * 3.0 + t * 0.9).sin() * 0.25;
        let c = if i % 2 == 0 { col(1.0, 0.55 + 0.08 * t, 0.10) } else { col(0.95, 0.25, 0.06) };
        parts.push(tilted((-1.6 - 0.55 * t, 1.1 - 0.12 * t, sway * t * 0.5), (0.7, 0.07, 0.16), c, 0.25 + sway * 0.4));
    }
    for i in 0..3 {
        let t = i as f32;
        parts.push(tilted((1.05 - 0.1 * t, 2.35 + 0.15 * t, (t - 1.0) * 0.12), (0.06, 0.32, 0.05), col(1.0, 0.85 - 0.2 * t, 0.2), 0.5 + (time * 6.0 + t).sin() * 0.2));
    }
    parts
}

/// The colours of a quail-shaped bird.
struct Plumage {
    feathers: Color,
    speckle: Color,
    belly: Color,
    face: Color,
    beak: Color,
    legs: Color,
}

fn bird_parts(a: &Animal, time: f32, night: bool, p: Plumage) -> Vec<Piece> {
    let Plumage { feathers, speckle, belly, face, beak, legs } = p;
    let flying = a.mode == Mode::Fly || (a.kind.is_metal_bird() && a.is_airborne()) || a.kind.flies_always();
    let moving = is_moving(a);
    let breath = (time * 5.0 + a.phase).sin() * 0.006;
    let peck = if a.mode == Mode::Idle { (time * 1.3 + a.phase * 2.0).sin().powi(11).max(0.0) } else { 0.0 };
    let bob = (a.phase * 6.0).sin() * 0.03 * moving;
    let lean = if flying { -0.45 } else { 0.0 };
    let mut parts = vec![
        tilted((0.0, 0.21 + breath, 0.0), (0.15, 0.10 + breath, 0.11), feathers, lean),
        tilted((0.0, 0.14, 0.0), (0.12, 0.05, 0.09), belly, lean),
        tilted((0.02, 0.28, 0.0), (0.11, 0.04, 0.09), speckle, lean),
        tilted((-0.20, 0.26, 0.0), (0.09, 0.03, 0.05), speckle, 0.4 + if flying { -0.6 } else { 0.0 }),
    ];
    let head = (0.17 + bob + peck * 0.07, 0.36 - peck * 0.20, 0.0);
    parts.push(tilted(head, (0.06, 0.06, 0.05), face, peck * -0.9));
    parts.push(tilted((head.0 + 0.08, head.1 - 0.015, 0.0), (0.03, 0.015, 0.02), beak, peck * -0.9));
    parts.push(tilted((head.0 - 0.03, head.1 + 0.10, 0.0), (0.012, 0.05, 0.012), speckle, 0.6 + (time * 3.0 + a.phase).sin() * 0.15));
    parts.push(block((head.0 + 0.02, head.1 + 0.07, 0.0), (0.04, 0.012, 0.03), belly));
    eyes(&mut parts, (head.0 + 0.03, head.1 + 0.02, 0.0), 0.05, 0.016, night, col(0.06, 0.05, 0.04));
    if flying {
        // Both wings beat together: up on the upstroke, down on the downstroke, swept back at the tips.
        let flap = (time * 32.0).sin();
        for side in [-1.0, 1.0] {
            parts.push(tilted((0.0, 0.27 + 0.09 * flap, 0.17 * side), (0.09, 0.012, 0.15), feathers, -flap * 0.2));
            parts.push(tilted((-0.06, 0.28 + 0.16 * flap, 0.33 * side), (0.07, 0.01, 0.10), speckle, -flap * 0.3));
        }
        for side in [-1.0, 1.0] {
            parts.push(tilted((-0.04, 0.09, 0.04 * side), (0.012, 0.045, 0.012), legs, 0.9));
        }
    } else {
        for side in [-1.0, 1.0] {
            parts.push(block((0.0, 0.20, 0.115 * side), (0.09, 0.05, 0.016), speckle));
            let stride = (a.phase * 6.0 + if side > 0.0 { 0.0 } else { std::f32::consts::PI }).sin() * 0.5 * moving;
            leg(&mut parts, (0.0, 0.10, 0.045 * side), 0.05, 0.05, 0.011, stride, legs, legs);
        }
    }
    parts
}

/// A bear: heavy, with a shoulder hump, a broad head and snout, round ears and thick legs. Its walk is a lumbering roll
/// (the body sways from side to side), it sniffs and swings its head when it stands, and it lowers its head and drives its
/// legs hard when it charges.
fn bear_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let fur = col(0.31, 0.21, 0.14);
    let shoulder = col(0.36, 0.25, 0.17);
    let snout = col(0.58, 0.45, 0.32);
    let dark = col(0.12, 0.09, 0.07);
    let moving = is_moving(a);
    let charging = a.mode == Mode::Charge;
    let (rate, amp) = if charging { (2.2, 0.55) } else { (1.4, 0.34) };
    let roll = (a.phase * rate * 0.5).sin() * 0.05 * moving;
    let breath = (time * 1.8 + a.phase).sin() * 0.015;
    let sniff = if moving == 0.0 { (time * 1.2 + a.phase).sin() * 0.12 } else { 0.0 };
    let head_y = if charging { 0.86 } else { 1.0 } + (a.phase * rate).sin().abs() * 0.03 * moving;
    let gallop = if charging { (a.phase * rate).sin() * 0.06 } else { 0.0 };
    let mut parts = vec![
        tilted((0.0, 0.82 + breath, roll), (0.62, 0.38 + breath, 0.43), fur, gallop),
        tilted((0.30, 1.12, roll), (0.32, 0.17, 0.38), shoulder, gallop),
        tilted((-0.38, 0.84, roll), (0.28, 0.36, 0.44), fur, gallop),
        block((-0.68, 0.94, 0.0), (0.07, 0.07, 0.07), fur),
    ];
    let head = (0.93, head_y, roll * 2.0 + sniff * 0.3);
    parts.push(tilted(head, (0.26, 0.23, 0.21), fur, -0.1 + sniff - gallop));
    parts.push(tilted((head.0 + 0.27, head.1 - 0.07, head.2), (0.15, 0.11, 0.11), snout, -0.1 + sniff));
    parts.push(tilted((head.0 + 0.41, head.1 - 0.03, head.2), (0.04, 0.04, 0.05), dark, -0.1 + sniff));
    eyes(&mut parts, (head.0 + 0.13, head.1 + 0.07, head.2), 0.21, 0.028, night, dark);
    for side in [-1.0, 1.0] {
        parts.push(block((head.0 - 0.08, head.1 + 0.27, head.2 + 0.16 * side), (0.07, 0.07, 0.045), fur));
    }
    for (i, &(lx, lz)) in [(0.42, 0.27), (0.42, -0.27), (-0.42, 0.27), (-0.42, -0.27)].iter().enumerate() {
        let swing = (a.phase * rate + if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI }).sin() * amp * moving;
        leg(&mut parts, (lx, 0.62, lz + roll), 0.31, 0.31, 0.12, swing, dark, col(0.07, 0.05, 0.04));
    }
    parts
}

/// The Great Bear: a bear's body drawn `BEAR_BOSS_MODEL_SCALE` times the size with much darker fur and eyes that glow red by day as well as
/// by night. (The glow and the halo round it keep their colours; only the fur is darkened.)
fn bear_boss_parts(a: &Animal, time: f32, _night: bool) -> Vec<Piece> {
    let mut parts = scaled(bear_parts(a, time, true), BEAR_BOSS_MODEL_SCALE);
    for ((_, _, c), _) in parts.iter_mut() {
        if c.a >= 1.0 && c.r < 0.9 {
            *c = Color::new(c.r * BEAR_BOSS_FUR_SHADE, c.g * BEAR_BOSS_FUR_SHADE, c.b * BEAR_BOSS_FUR_SHADE, 1.0);
        }
    }
    parts
}

/// The Yeti: a broad, shaggy white ape. Its arms hang to the knuckles and swing as it walks; when it charges they rise and
/// pound, and below half health it glows red-eyed and heaves for breath.
fn yeti_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let fur = col(0.92, 0.94, 0.97);
    let shade = col(0.74, 0.78, 0.86);
    let face = col(0.30, 0.34, 0.42);
    let dark = col(0.10, 0.11, 0.14);
    let ice = col(0.62, 0.84, 0.95);
    let moving = is_moving(a);
    let charging = a.mode == Mode::Charge;
    let rage = if a.boss_enraged() { 1.0 } else { 0.0 };
    let (rate, amp) = if charging { (2.4, 0.6) } else { (1.5, 0.3) };
    let breath = (time * (1.6 + rage * 2.4) + a.phase).sin() * 0.03;
    let roll = (a.phase * rate * 0.5).sin() * 0.05 * moving;
    let lean = if charging { 0.22 } else { 0.08 };
    let mut parts = vec![
        // Hips, belly and a huge chest with shoulders.
        tilted((0.0, 1.35, roll), (0.55, 0.45 + breath, 0.55), fur, lean),
        tilted((0.12, 2.0 + breath, roll), (0.62, 0.50 + breath, 0.80), fur, lean),
        tilted((0.14, 2.38, roll), (0.50, 0.14, 0.90), shade, lean),
        tilted((-0.10, 1.55, 0.0), (0.40, 0.30, 0.50), shade, lean),
    ];
    // The head, low between the shoulders, with a dark face, a heavy brow, tusks and ice spikes along the back.
    let head = (0.82 + lean, 2.55 + breath, roll * 2.0);
    parts.push(tilted(head, (0.34, 0.32, 0.34), fur, -0.1));
    parts.push(tilted((head.0 + 0.30, head.1 - 0.08, head.2), (0.12, 0.22, 0.26), face, -0.1));
    parts.push(tilted((head.0 + 0.22, head.1 + 0.20, head.2), (0.16, 0.06, 0.34), shade, -0.1));
    parts.push(tilted((head.0 + 0.44, head.1 - 0.30, head.2), (0.05, 0.05, 0.20), dark, 0.0));
    for side in [-1.0, 1.0] {
        parts.push(tilted((head.0 + 0.40, head.1 - 0.30, head.2 + 0.15 * side), (0.04, 0.13, 0.03), col(0.96, 0.94, 0.86), 0.0));
    }
    let eye_day = if rage > 0.0 { col(0.9, 0.1, 0.05) } else { dark };
    eyes(&mut parts, (head.0 + 0.30, head.1 + 0.08, head.2), 0.26, 0.05, night || rage > 0.0, eye_day);
    for i in 0..4 {
        let t = i as f32;
        parts.push(tilted((-0.45 - t * 0.02, 2.55 - t * 0.28, roll), (0.07, 0.22 - t * 0.03, 0.07), ice, 0.4));
    }
    // Arms: long, swinging in opposition, raised and pounding in a charge.
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let swing = (a.phase * rate + if i == 0 { 0.0 } else { std::f32::consts::PI }).sin() * amp * moving;
        let pound = if charging { 0.9 + (time * 7.0 + i as f32 * 3.1).sin() * 0.35 } else { 0.0 };
        // Winding up a smash or a throw it lifts both fists high over its head (a throw: one hand, with a lump of ice in it).
        let raised = match a.action {
            BossAction::Smash => -2.6,
            BossAction::IceThrow if i == 1 => -2.4,
            _ => 0.0,
        };
        let shoulder = (0.12, 2.25, 0.98 * side + roll);
        if raised != 0.0 {
            // Straight up: an upper arm, a forearm and a fist high over the head.
            parts.push(block((shoulder.0, shoulder.1 + 0.8, shoulder.2), (0.22, 0.85, 0.22), fur));
            parts.push(block((shoulder.0, shoulder.1 + 1.95, shoulder.2), (0.20, 0.55, 0.20), fur));
            parts.push(block((shoulder.0, shoulder.1 + 2.6, shoulder.2), (0.28, 0.2, 0.28), shade));
        } else {
            leg(&mut parts, shoulder, 0.85, 0.80, 0.22, swing + pound, fur, shade);
        }
    }
    if a.action == BossAction::IceThrow {
        // The lump of ice held up in the raised hand.
        parts.push(tilted((0.12, 4.5, 0.98 + roll), (0.32, 0.32, 0.32), ice, 0.3));
        parts.push(tilted((0.12, 4.5, 0.98 + roll), (0.22, 0.40, 0.22), col(0.85, 0.95, 1.0), -0.3));
    }
    // Legs: short and thick.
    for (i, &(lx, lz)) in [(0.12, 0.34), (0.12, -0.34)].iter().enumerate() {
        let swing = (a.phase * rate + if i == 0 { 0.0 } else { std::f32::consts::PI }).sin() * amp * moving;
        leg(&mut parts, (lx, 1.0, lz + roll), 0.50, 0.48, 0.24, swing, fur, shade);
    }
    parts
}

/// The Loch Ness Monster: three dark-green humps along the water, a tail, and a long neck that sways and rears up to strike,
/// ending in a small head with a pale throat. Most of the body is meant to be half in the water.
fn loch_ness_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    // Each lake has a monster of its own, and it looks it: the colouring is picked from where its lake is.
    let (skin, back, belly) = match crate::rng::hash3(0, a.home.x.floor() as i32, a.home.z.floor() as i32, 0x4E53) % 4 {
        0 => (col(0.18, 0.36, 0.28), col(0.12, 0.27, 0.21), col(0.62, 0.70, 0.52)),
        1 => (col(0.22, 0.32, 0.46), col(0.14, 0.22, 0.34), col(0.66, 0.72, 0.80)),
        2 => (col(0.40, 0.26, 0.40), col(0.28, 0.16, 0.30), col(0.78, 0.66, 0.74)),
        _ => (col(0.46, 0.30, 0.18), col(0.32, 0.20, 0.12), col(0.82, 0.72, 0.52)),
    };
    let dark = col(0.05, 0.08, 0.06);
    let charging = a.mode == Mode::Charge;
    let swim = (time * 1.4 + a.phase * 0.2).sin();
    let rear = if charging { 0.35 + 0.25 * (time * 3.0).sin().abs() } else { 0.0 };
    let mut parts = vec![
        // The humps rise and sink in turn.
        block((-0.9, 0.55 + 0.12 * (time * 1.2).sin(), 0.0), (0.85, 0.55, 0.95), back),
        block((0.55, 0.60 + 0.12 * (time * 1.2 + 2.0).sin(), 0.0), (0.80, 0.60, 0.90), skin),
        block((-2.2, 0.40 + 0.10 * (time * 1.2 + 4.0).sin(), 0.0), (0.70, 0.40, 0.70), back),
        block((0.3, 0.22, 0.0), (1.0, 0.18, 0.80), belly),
    ];
    // The tail tapers away and sways.
    for i in 1..=4 {
        let t = i as f32;
        parts.push(block((-2.9 - t * 0.55, 0.28, 0.35 * swim * t * 0.5), (0.35 - t * 0.05, 0.22 - t * 0.03, 0.30 - t * 0.05), back));
    }
    // The neck: a stack of segments curving up and forward, swaying side to side.
    let segments = 7;
    let mut top = (1.1, 1.0, 0.0);
    for i in 0..segments {
        let t = i as f32 / (segments - 1) as f32;
        let reach = 0.25 + 0.65 * t + rear * t * 1.2;
        let sway = swim * 0.28 * t + (time * 0.7 + t * 2.0).sin() * 0.1 * t;
        let pos = (1.1 + reach * 1.2, 1.0 + t * (1.9 + rear), sway);
        parts.push(block(pos, (0.30 - 0.06 * t, 0.26, 0.28 - 0.06 * t), if i % 2 == 0 { skin } else { back }));
        top = pos;
    }
    let head = (top.0 + 0.35, top.1 + 0.12, top.2);
    parts.push(tilted(head, (0.34, 0.2, 0.22), skin, -0.2));
    parts.push(tilted((head.0 + 0.30, head.1 - 0.06, head.2), (0.16, 0.10, 0.14), belly, -0.2));
    eyes(&mut parts, (head.0 + 0.1, head.1 + 0.1, head.2), 0.22, 0.04, night, dark);
    parts
}

/// A bat: a small furry body, a head with big ears and a red mouth, and two long wings of fingered membrane that beat fast (and
/// fold when it hangs about). It always seems to be flying, so there are no legs to walk on. Its eyes glow red at night, like the
/// other creatures'.
fn bat_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let fur = col(0.16, 0.12, 0.13);
    let dark = col(0.09, 0.07, 0.09);
    let membrane = col(0.24, 0.16, 0.19);
    let flap = (time * 26.0 + a.phase * 2.0).sin();
    let bob = 0.02 * (time * 7.0 + a.phase).sin();
    let mut parts = vec![
        block((0.0, 0.18 + bob, 0.0), (0.10, 0.07, 0.07), fur),
        block((-0.09, 0.17 + bob, 0.0), (0.05, 0.03, 0.03), dark),
        block((0.12, 0.20 + bob, 0.0), (0.06, 0.05, 0.05), fur),
        block((0.17, 0.17 + bob, 0.0), (0.02, 0.015, 0.03), col(0.55, 0.15, 0.18)),
    ];
    for side in [-1.0, 1.0] {
        parts.push(block((0.10, 0.27 + bob, 0.035 * side), (0.025, 0.05, 0.015), dark));
        // Each wing: an arm bone out from the shoulder, then a wide membrane that swings up and down with it.
        parts.push(tilted((0.0, 0.20 + bob + 0.10 * flap, 0.20 * side), (0.025, 0.012, 0.18), dark, 0.0));
        parts.push(tilted((-0.03, 0.20 + bob + 0.16 * flap, 0.42 * side), (0.09, 0.008, 0.10), membrane, flap * 0.35));
        parts.push(tilted((-0.07, 0.20 + bob + 0.07 * flap, 0.20 * side), (0.07, 0.008, 0.16), membrane, flap * 0.20));
    }
    eyes(&mut parts, (0.13, 0.215 + bob, 0.0), 0.05, 0.014, night, col(0.02, 0.02, 0.02));
    parts
}

/// A vampire bat: a bat `VAMPIRE_BAT_MODEL_SCALE` times the size, with crimson wings, a blood-red gleam in its eyes by day and night, and two white fangs.
fn vampire_bat_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    const K: f32 = VAMPIRE_BAT_MODEL_SCALE;
    let mut parts = scaled(bat_parts(a, time, night), K);
    for ((_, _, c), _) in parts.iter_mut() {
        // The membrane of the small bat becomes a deep crimson, and its fur a purple-black.
        if (c.r - 0.24).abs() < 0.01 && (c.g - 0.16).abs() < 0.01 {
            *c = col(0.46, 0.05, 0.10);
        } else if (c.r - 0.16).abs() < 0.01 && (c.g - 0.12).abs() < 0.01 {
            *c = col(0.14, 0.06, 0.16);
        }
    }
    for side in [-1.0, 1.0] {
        parts.push(block((0.17 * K, 0.135 * K, 0.025 * K * side), (0.008 * K, 0.04 * K, 0.008 * K), col(0.96, 0.95, 0.90)));
        parts.push(block((0.13 * K, 0.22 * K, 0.05 * K * side), (0.014 * K, 0.014 * K, 0.014 * K), col(0.95, 0.10, 0.12)));
    }
    parts
}

/// A limb with two joints and explicit angles (positive swings the end forward): the upper segment from `shoulder`, the lower from its end.
fn limb(parts: &mut Vec<Piece>, shoulder: (f32, f32, f32), upper: f32, lower: f32, thick: f32, up_angle: f32, low_angle: f32, hide: Color, end: Color) {
    let elbow = (shoulder.0 + up_angle.sin() * upper, shoulder.1 - up_angle.cos() * upper, shoulder.2);
    parts.push(tilted((shoulder.0 + up_angle.sin() * upper / 2.0, shoulder.1 - up_angle.cos() * upper / 2.0, shoulder.2), (thick, upper / 2.0, thick), hide, up_angle));
    let hand = (elbow.0 + low_angle.sin() * lower, elbow.1 - low_angle.cos() * lower, elbow.2);
    parts.push(tilted((elbow.0 + low_angle.sin() * lower / 2.0, elbow.1 - low_angle.cos() * lower / 2.0, elbow.2), (thick * 0.85, lower / 2.0, thick * 0.85), hide, low_angle));
    parts.push(block(hand, (thick * 1.1, thick * 1.0, thick * 1.1), end));
}

/// The Orangutan: a huge, hunched, long-armed ape with shaggy orange-red fur, a dark face with broad cheek pads and a beard. Its arms
/// swing as it lumbers; they go up over its head as it winds up a smash, and one lifts a rock as it winds up a throw.
fn orangutan_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let fur = col(0.66, 0.31, 0.12);
    let shaggy = col(0.54, 0.24, 0.09);
    let face = col(0.24, 0.17, 0.14);
    let moving = is_moving(a);
    let swing = (a.phase * 1.6).sin() * 0.55 * moving;
    let breath = (time * 1.6 + a.phase).sin() * 0.02;
    let smash = matches!(a.action, BossAction::Smash | BossAction::Leap);
    let throwing = a.action == BossAction::RockThrow;
    let mut parts = vec![
        tilted((0.0, 1.45 + breath, 0.0), (0.62, 0.72 + breath, 0.55), fur, -0.22),
        tilted((0.12, 1.15, 0.0), (0.5, 0.5, 0.44), col(0.78, 0.45, 0.22), -0.22),
        tilted((-0.35, 1.9, 0.0), (0.4, 0.3, 0.6), shaggy, -0.22),
    ];
    // The head sits forward of the shoulders, on a short neck, with cheek pads, a beard and a crown of long hair.
    let nod = if smash { 0.25 } else { (a.phase * 1.6).sin().abs() * 0.05 * moving };
    let head = (0.72, 2.18 + nod, 0.0);
    parts.push(block(head, (0.32, 0.30, 0.30), face));
    parts.push(block((head.0 - 0.08, head.1 + 0.28, 0.0), (0.30, 0.14, 0.32), fur));
    parts.push(block((head.0 + 0.30, head.1 - 0.08, 0.0), (0.10, 0.12, 0.17), col(0.34, 0.24, 0.18)));
    for side in [-1.0, 1.0] {
        parts.push(block((head.0 + 0.10, head.1 - 0.02, 0.40 * side), (0.14, 0.22, 0.09), col(0.30, 0.21, 0.16)));
    }
    parts.push(block((head.0 + 0.16, head.1 - 0.40, 0.0), (0.13, 0.22, 0.26), shaggy));
    eyes(&mut parts, (head.0 + 0.24, head.1 + 0.08, 0.0), 0.30, 0.055, night, col(0.04, 0.03, 0.03));
    // Four limbs: short bowed legs, and long arms that reach nearly to the ground.
    for side in [-1.0, 1.0] {
        let stride = if side > 0.0 { swing } else { -swing };
        leg(&mut parts, (-0.12, 0.95, 0.38 * side), 0.5, 0.5, 0.22, stride * 0.8, fur, face);
        let shoulder = (0.32, 2.0, 0.82 * side);
        if smash {
            limb(&mut parts, shoulder, 0.95, 0.95, 0.2, 2.9, 2.2, fur, face);
        } else if throwing && side > 0.0 {
            limb(&mut parts, shoulder, 0.95, 0.95, 0.2, 2.6, 3.0, fur, face);
        } else {
            limb(&mut parts, shoulder, 0.95, 0.95, 0.2, -stride * 0.9 + 0.1, -stride * 0.5 + 0.25, fur, face);
        }
    }
    if throwing {
        // The rock held up over its head in the raised hand.
        parts.push(tilted((0.32 + 2.6_f32.sin() * 0.95 + 3.0_f32.sin() * 0.95 + 0.05, 2.0 - 2.6_f32.cos() * 0.95 - 3.0_f32.cos() * 0.95 + 0.45, 0.82), (0.42, 0.38, 0.42), col(0.50, 0.47, 0.44), 0.4));
    }
    parts
}

/// A jaguar: a golden cat with black rosettes, low and long, a swaying tail, and legs that bend. It creeps with its belly near the
/// ground while it stalks, flattens to crouch, and stretches right out, legs reaching forward and back, in its leap.
fn jaguar_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    use crate::animal::Pounce;
    let coat = col(0.80, 0.58, 0.18);
    let belly = col(0.93, 0.84, 0.62);
    let spot = col(0.12, 0.09, 0.06);
    let moving = is_moving(a);
    let stalking = a.mode == Mode::Charge;
    let (drop, stretch) = match a.pounce {
        Pounce::Crouch => (0.34, 0.0),
        Pounce::Leap => (0.0, 1.0),
        Pounce::Stalk if stalking => (0.24, 0.0),
        _ => (0.0, 0.0),
    };
    let y = 0.78 - drop;
    let breath = (time * 3.0 + a.phase).sin() * 0.01;
    let mut parts = vec![
        block((0.0, y + breath, 0.0), (0.78, 0.27 + breath, 0.30), coat),
        block((0.0, y - 0.20, 0.0), (0.66, 0.07, 0.22), belly),
        block((0.05, y + 0.15, 0.0), (0.74, 0.05, 0.20), coat),
    ];
    for (i, (x, z)) in [(-0.5, 0.12), (-0.25, -0.15), (0.0, 0.13), (0.25, -0.12), (0.5, 0.1), (-0.4, -0.05), (0.15, 0.0), (0.4, -0.22)].into_iter().enumerate() {
        parts.push(block((x, y + 0.29 + breath, z), (0.07, 0.015, 0.07), spot));
        if i % 2 == 0 {
            parts.push(block((x, y + 0.02, if z > 0.0 { 0.31 } else { -0.31 }), (0.08, 0.07, 0.012), spot));
        }
    }
    let head = (0.98, y + 0.12 - drop * 0.1, 0.0);
    parts.push(block(head, (0.26, 0.21, 0.23), coat));
    parts.push(block((head.0 + 0.24, head.1 - 0.06, 0.0), (0.11, 0.09, 0.14), belly));
    parts.push(block((head.0 + 0.34, head.1 - 0.01, 0.0), (0.03, 0.03, 0.05), spot));
    for side in [-1.0, 1.0] {
        parts.push(block((head.0 - 0.06, head.1 + 0.25, 0.14 * side), (0.05, 0.07, 0.05), coat));
    }
    eyes(&mut parts, (head.0 + 0.14, head.1 + 0.06, 0.0), 0.23, 0.04, night, col(0.85, 0.75, 0.15));
    // The tail sways, low while it stalks.
    for i in 0..5 {
        let t = i as f32;
        let sway = (time * 3.0 + t * 0.7 + a.phase).sin() * 0.12 * (1.0 + t * 0.3);
        parts.push(tilted((-0.82 - 0.2 * t, y + 0.1 - 0.04 * t + if stalking { 0.0 } else { 0.05 * t }, sway), (0.12, 0.06, 0.06), if i % 2 == 0 { coat } else { spot }, 0.2));
    }
    // Four legs: walking strides, bent low in a crouch, flung out front and back in the leap.
    let rate = if stalking { 3.0 } else { 2.2 };
    for (hx, fore) in [(0.55, true), (-0.55, false)] {
        for side in [-1.0, 1.0] {
            let phase = (a.phase * rate + if (side > 0.0) == fore { 0.0 } else { std::f32::consts::PI }).sin();
            let angle = if stretch > 0.0 { if fore { 1.1 } else { -1.1 } } else { phase * 0.5 * moving };
            leg(&mut parts, (hx, y - 0.1, 0.22 * side), 0.34, 0.34, 0.09, angle, coat, spot);
        }
    }
    parts
}

/// A flying snake's head: a broad flat head with red eyes and a flickering tongue, and two leaf-green wings beating at the neck. The
/// head is at the animal's position, half the snake's height up; the long body is drawn by `draw_snake_body`.
fn flying_snake_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    let mut parts = Vec::new();
    let y = SNAKE_HEIGHT * 0.5;
    parts.push(block((0.0, y, 0.0), (0.46, 0.20, 0.32), col(0.14, 0.46, 0.18)));
    parts.push(block((0.40, y - 0.05, 0.0), (0.16, 0.12, 0.20), col(0.70, 0.78, 0.22)));
    let flick = (time * 14.0).sin().max(0.0);
    parts.push(block((0.62 + 0.14 * flick, y - 0.06, 0.0), (0.18 + 0.10 * flick, 0.02, 0.03), col(0.88, 0.12, 0.16)));
    eyes(&mut parts, (0.18, y + 0.10, 0.0), 0.32, 0.07, night, col(0.80, 0.10, 0.12));
    let flap = (time * 24.0 + a.phase).sin();
    for side in [-1.0, 1.0] {
        parts.push(tilted((-0.42, y + 0.18 + 0.20 * flap, 0.62 * side), (0.44, 0.02, 0.52), col(0.30, 0.78, 0.34), -flap * 0.4 * side));
    }
    parts
}

/// The colours of the bands along a snake's body.
fn snake_band(i: usize) -> Color {
    match i % 4 {
        0 => col(0.16, 0.52, 0.20),
        1 => col(0.62, 0.74, 0.18),
        2 => col(0.12, 0.40, 0.17),
        _ => col(0.78, 0.20, 0.14),
    }
}

/// The boxes of a flying snake's body, tail first or head first as `(centre, half extents, yaw, pitch, colour)`: one at each point of
/// `Animal::body_points`, turned along the line to the next, thickest at the neck and thinning to the tail, rippling from side to side.
pub fn snake_body_boxes(a: &Animal, time: f32) -> Vec<(Vec3, Vec3, f32, f32, Color)> {
    let pts = a.body_points();
    let n = pts.len();
    let mut out = Vec::with_capacity(n);
    for i in 1..n {
        let along = i as f32 / n as f32;
        let dir = pts[i - 1] - pts[i];
        let flat = vec2(dir.x, dir.z).length();
        let (yaw, pitch) = if dir.length() < 1e-4 { (a.heading, 0.0) } else { (dir.z.atan2(dir.x), dir.y.atan2(flat.max(1e-4))) };
        let thick = 0.34 * (1.0 - 0.7 * along) + 0.06;
        let ripple = (time * 4.0 + i as f32 * 0.45 + a.phase).sin() * 0.22 * (0.3 + along.min(0.7));
        let side = vec3(-yaw.sin(), 0.0, yaw.cos()) * ripple;
        out.push((pts[i] + side, vec3(SNAKE_SEGMENT_SPACING * 0.8, thick, thick), yaw, pitch, snake_band(i)));
    }
    out
}

/// Draw a flying snake's body (its head is drawn as an ordinary model).
fn draw_snake_body(a: &Animal, time: f32) {
    for (centre, half, yaw, pitch, color) in snake_body_boxes(a, time) {
        draw_box_tilted(centre, half, yaw, pitch, color);
    }
}

/// A spirit archer: the Wild Hunt's archer model, made pale blue-white and see-through.
fn spirit_parts(a: &Animal, time: f32) -> Vec<Piece> {
    let mut body = a.clone();
    body.kind = AnimalKind::HuntArcher;
    ghostly(hunt_parts(&body, time), (0.74, 0.90, 1.0))
}

/// The look of a warrior of the Wild Hunt: the colour of its ghostly light, and its weapon.
fn hunt_tint(kind: AnimalKind) -> (f32, f32, f32) {
    match kind {
        AnimalKind::HuntSwordsman => (0.72, 0.86, 1.0),
        AnimalKind::HuntArcher => (0.66, 1.0, 0.82),
        AnimalKind::HuntLancer => (1.0, 0.84, 0.62),
        _ => (0.80, 0.68, 1.0),
    }
}

/// A warrior of the Wild Hunt, all of it translucent: ragged robes that trail behind, a helmed head with two cold bright eyes, and
/// a weapon in the hand that shows what it does: a long sword (the swordsman, who holds it forward as it runs), a drawn bow (the
/// archer) or a dagger (the stalker, who runs bent low). Its legs stride as it moves and the robe streams out and sways.
pub fn hunt_parts(a: &Animal, time: f32) -> Vec<Piece> {
    match a.kind {
        AnimalKind::HuntBear => return ghostly(bear_parts(a, time, false), (0.72, 0.88, 1.0)),
        AnimalKind::HuntCrow => return ghostly(crow_parts(a, time, false), (0.72, 0.88, 1.0)),
        AnimalKind::HuntLancer => return lancer_parts(a, time),
        _ => {}
    }
    let (r, g, b) = hunt_tint(a.kind);
    let body = ghost(r, g, b, 0.42);
    let dim = ghost(r * 0.6, g * 0.6, b * 0.7, 0.34);
    let bright = ghost((r + 1.0) / 2.0, (g + 1.0) / 2.0, (b + 1.0) / 2.0, 0.85);
    let steel = ghost(0.92, 0.96, 1.0, 0.80);
    let moving = is_moving(a);
    let stride = (a.phase * 2.2).sin() * 0.7 * moving;
    let sway = (time * 2.0 + a.phase).sin();
    let lean = if a.kind == AnimalKind::HuntSwordsman { 0.15 * moving } else { 0.0 };
    let (crouch, low) = (0.0_f32, 0.0_f32);
    let mut parts: Vec<Piece> = Vec::new();
    for side in [-1.0, 1.0] {
        let angle = if side > 0.0 { stride } else { -stride };
        leg(&mut parts, (0.0, 0.92 + low, 0.14 * side), 0.46, 0.46, 0.07, angle, dim, dim);
    }
    // Torso, belt, shoulders, and the robe: a skirt below the waist and a long tatter streaming out behind.
    parts.push(tilted((0.0, 1.22 + low, 0.0), (0.17, 0.30, 0.24), body, lean));
    parts.push(tilted((0.0, 0.98 + low, 0.0), (0.19, 0.04, 0.26), dim, lean));
    parts.push(tilted((-0.02, 0.62 + low, 0.0), (0.21, 0.30, 0.27), dim, lean * 0.5));
    for i in 1..=3 {
        let t = i as f32;
        parts.push(tilted((-0.28 * t, 0.95 + low - 0.10 * t, 0.07 * t * (sway + t).sin()), (0.14, 0.22 - 0.04 * t, 0.20 - 0.03 * t), ghost(r, g, b, 0.30 - 0.07 * t), -0.25));
    }
    // The head: a helm with a crest and a dark face, with two bright eyes in it.
    let head = (0.06 + 0.08 * crouch, 1.74 + low, 0.0);
    parts.push(tilted(head, (0.12, 0.13, 0.12), body, lean * 0.5));
    parts.push(tilted((head.0, head.1 + 0.17, 0.0), (0.14, 0.04, 0.03), bright, 0.0));
    parts.push(block((head.0 + 0.11, head.1 - 0.02, 0.0), (0.02, 0.08, 0.10), ghost(0.04, 0.06, 0.12, 0.7)));
    for side in [-1.0, 1.0] {
        parts.push(block((head.0 + 0.125, head.1 + 0.01, 0.055 * side), (0.012, 0.022, 0.03), bright));
    }
    // Arms and weapon.
    match a.kind {
        AnimalKind::HuntSwordsman => {
            let swing = 0.25 * (a.phase * 2.2).sin() * moving;
            parts.push(tilted((0.20, 1.28, 0.30), (0.05, 0.26, 0.05), body, -1.2 + swing));
            parts.push(tilted((0.28, 1.05, -0.28), (0.05, 0.26, 0.05), body, 0.3));
            // A long straight blade held out in front, with a cross-guard.
            parts.push(tilted((0.86, 1.36 + swing * 0.3, 0.30), (0.62, 0.025, 0.05), steel, -0.12 + swing));
            parts.push(block((0.30, 1.30, 0.30), (0.03, 0.03, 0.14), bright));
        }
        AnimalKind::HuntArcher => {
            // The left arm holds a bow out; the right is drawn back to the string.
            parts.push(tilted((0.34, 1.34, 0.26), (0.05, 0.30, 0.05), body, -1.35));
            parts.push(tilted((0.14, 1.34, -0.20), (0.05, 0.25, 0.05), body, -1.0));
            parts.push(tilted((0.68, 1.34, 0.26), (0.03, 0.46, 0.03), steel, 0.12));
            parts.push(block((0.62, 1.34, 0.26), (0.006, 0.44, 0.006), ghost(1.0, 1.0, 1.0, 0.45)));
        }
        _ => {}
    }
    parts
}

/// A model made translucent and tinted, for the ghostly bears and crows of the Wild Hunt.
fn ghostly(parts: Vec<Piece>, (r, g, b): (f32, f32, f32)) -> Vec<Piece> {
    parts.into_iter().map(|((off, half, c), pitch)| ((off, half, ghost(c.r * 0.35 + r * 0.65, c.g * 0.35 + g * 0.65, c.b * 0.35 + b * 0.65, 0.5)), pitch)).collect()
}

/// A lancer of the Wild Hunt: a ghostly horse that gallops (four jointed legs, a tossing head, a streaming tail) with a helmed rider
/// on its back couching a long lance at the player. At rest it stands and the lance is raised.
fn lancer_parts(a: &Animal, time: f32) -> Vec<Piece> {
    let (r, g, b) = hunt_tint(a.kind);
    let body = ghost(r, g, b, 0.42);
    let dim = ghost(r * 0.6, g * 0.6, b * 0.7, 0.34);
    let bright = ghost((r + 1.0) / 2.0, (g + 1.0) / 2.0, (b + 1.0) / 2.0, 0.85);
    let steel = ghost(0.92, 0.96, 1.0, 0.80);
    let moving = is_moving(a);
    let charging = a.hunt_phase == HuntPhase::Dash;
    let gallop = (a.phase * 1.6).sin() * 0.9 * moving;
    let toss = (a.phase * 1.6).sin() * 0.08 * moving;
    let mut parts: Vec<Piece> = vec![
        // The horse: barrel, rump, neck and head.
        tilted((0.0, 1.35, 0.0), (0.85, 0.36, 0.32), body, toss),
        tilted((-0.65, 1.42, 0.0), (0.34, 0.38, 0.30), body, 0.0),
        tilted((0.85, 1.75, 0.0), (0.18, 0.55, 0.22), body, -0.7 + toss),
        tilted((1.25, 2.20, 0.0), (0.34, 0.18, 0.17), body, 0.3 + toss),
        tilted((1.05, 2.30, 0.0), (0.06, 0.20, 0.03), bright, -0.2),
    ];
    // A streaming mane and tail.
    for i in 1..=3 {
        let t = i as f32;
        parts.push(tilted((-1.0 - 0.28 * t, 1.55 - 0.18 * t, 0.07 * t * (time * 3.0 + t).sin()), (0.20, 0.05, 0.08), ghost(r, g, b, 0.30), -0.4));
        parts.push(tilted((0.95 - 0.1 * t, 2.25 - 0.12 * t, 0.0), (0.05, 0.12, 0.06), dim, 0.0));
    }
    // Two bright eyes.
    for side in [-1.0, 1.0] {
        parts.push(block((1.52, 2.24, 0.12 * side), (0.02, 0.03, 0.03), bright));
    }
    // Four legs, the pairs a half stride apart.
    for (hip_x, phase) in [(0.6, 0.0), (0.6, std::f32::consts::PI), (-0.6, std::f32::consts::PI), (-0.6, 0.0)] {
        for side in [-1.0_f32, 1.0] {
            let swing = (a.phase * 1.6 + phase + if side > 0.0 { 0.0 } else { 0.5 }).sin() * 0.9 * moving;
            leg(&mut parts, (hip_x, 1.05, 0.2 * side), 0.55, 0.5, 0.07, swing, dim, dim);
        }
    }
    // The rider: robe, torso, a helm with a crest and bright eyes.
    parts.push(tilted((-0.05, 2.0, 0.0), (0.2, 0.34, 0.26), body, 0.1 + gallop * 0.03));
    parts.push(tilted((-0.05, 1.72, 0.0), (0.26, 0.26, 0.30), dim, 0.0));
    let head = (0.02, 2.62, 0.0);
    parts.push(tilted(head, (0.12, 0.13, 0.12), body, 0.0));
    parts.push(tilted((head.0, head.1 + 0.17, 0.0), (0.14, 0.04, 0.03), bright, 0.0));
    for side in [-1.0, 1.0] {
        parts.push(block((head.0 + 0.125, head.1 + 0.01, 0.055 * side), (0.012, 0.022, 0.03), bright));
    }
    // The lance: long and level at the charge (couched under the arm), raised when resting; a pennon streams from it.
    let tilt = if charging { -0.05 } else { 0.9 };
    let (lx, ly) = if charging { (1.6, 2.15) } else { (0.5, 3.0) };
    parts.push(tilted((lx, ly, 0.38), (1.7, 0.035, 0.035), steel, tilt));
    parts.push(tilted((lx + if charging { 1.65 } else { 0.9 }, ly + if charging { 0.0 } else { 1.0 }, 0.38), (0.22, 0.05, 0.05), bright, tilt));
    parts.push(tilted((lx + 0.5, ly - 0.14, 0.38), (0.30, 0.12, 0.01), ghost(0.9, 0.3, 0.3, 0.6), tilt + (time * 9.0).sin() * 0.2));
    parts.push(tilted((0.35, 1.95, 0.30), (0.05, 0.28, 0.05), body, -1.1));
    parts.push(tilted((0.2, 1.9, -0.28), (0.05, 0.28, 0.05), body, -0.4));
    parts
}

/// A colour with transparency, for the ghostly parts of a will-o-whisp.
fn ghost(r: f32, g: f32, b: f32, alpha: f32) -> Color {
    Color::new(r, g, b, alpha)
}

/// The parts of a will-o-whisp, all of them translucent: a faint pale halo, a brighter core that flickers, two
/// dark hollows for eyes on the side it faces, and a tail of ever smaller, ever fainter wisps trailing down and
/// behind it, swaying as it drifts.
pub fn will_o_whisp_parts(a: &Animal, time: f32) -> Vec<Part> {
    let flicker = 0.5 + 0.5 * (time * 5.0 + a.phase).sin();
    let slow = (time * 1.6 + a.phase * 1.3).sin();
    let w = WILL_O_WHISP_HALF_WIDTH;
    let mut parts: Vec<Part> = vec![
        // The halo.
        ((0.0, 0.32, 0.0), (w * 1.9, 0.40, w * 1.9), ghost(0.55, 0.85, 1.0, 0.10 + 0.05 * flicker)),
        // The body.
        ((0.0, 0.32, 0.0), (w, 0.24, w), ghost(0.78, 0.95, 1.0, 0.34 + 0.16 * flicker)),
        // The bright heart.
        ((0.0, 0.34, 0.0), (w * 0.5, 0.12, w * 0.5), ghost(0.95, 1.0, 1.0, 0.55 + 0.25 * flicker)),
        // Eyes: two dark hollows on the front.
        ((w * 0.95, 0.40, 0.09), (0.02, 0.04, 0.03), ghost(0.05, 0.10, 0.20, 0.75)),
        ((w * 0.95, 0.40, -0.09), (0.02, 0.04, 0.03), ghost(0.05, 0.10, 0.20, 0.75)),
    ];
    // The tail.
    for i in 1..=4 {
        let t = i as f32;
        let sway = 0.10 * t * (slow + t * 0.7).sin();
        let size = w * (1.0 - 0.17 * t);
        parts.push(((-0.07 * t, 0.32 - 0.17 * t, sway), (size, 0.10, size), ghost(0.70, 0.92, 1.0, (0.26 - 0.055 * t).max(0.03))));
    }
    parts
}

/// Draw every animal within the render range. Call while the 3D camera is active.
pub fn draw_animals(animals: &[Animal], time: f32, player_chunk: (i32, i32), night: bool) {
    for a in animals {
        if !in_render_range(player_chunk, World::chunk_of_pos(a.pos)) {
            continue;
        }
        let parts = match a.kind {
            // Drawn by `draw_ghosts`, over the water.
            AnimalKind::WillOWhisp => continue,
            kind if kind.is_wild_hunt() => continue,
            AnimalKind::Deer => deer_parts(a, time, night),
            AnimalKind::Quail => quail_parts(a, time, night),
            AnimalKind::Crow => crow_parts(a, time, night),
            AnimalKind::IronQuail => iron_quail_parts(a, time, night),
            AnimalKind::BronzeParrot => bronze_parrot_parts(a, time, night),
            AnimalKind::Parrot => parrot_parts(a, time, night),
            AnimalKind::Bear => bear_parts(a, time, night),
            AnimalKind::Rabbit => rabbit_parts(a, time, night),
            AnimalKind::Moose => moose_parts(a, time, night),
            AnimalKind::Yeti => yeti_parts(a, time, night),
            AnimalKind::LochNessMonster => loch_ness_parts(a, time, night),
            AnimalKind::Bat => bat_parts(a, time, night),
            AnimalKind::Raven => raven_parts(a, time, night),
            AnimalKind::Phoenix => phoenix_parts(a, time, night),
            AnimalKind::VampireBat => vampire_bat_parts(a, time, night),
            AnimalKind::Orangutan => orangutan_parts(a, time, night),
            AnimalKind::Jaguar => jaguar_parts(a, time, night),
            AnimalKind::FlyingSnake => flying_snake_parts(a, time, night),
            AnimalKind::Newt => newt_parts(a, time, night),
            AnimalKind::Fairy => fairy_parts(a, time, night),
            AnimalKind::MountainGoat => goat_parts(a, time, night),
            AnimalKind::Unicorn => unicorn_parts(a, time, night),
            AnimalKind::BearBoss => bear_boss_parts(a, time, night),
            AnimalKind::Crawfish => crawfish_parts(a, time, night),
            AnimalKind::BigCrawfish => big_crawfish_parts(a, time, night),
            AnimalKind::GiantCrawfish => giant_crawfish_parts(a, time, night),
            AnimalKind::Dragon => dragon_parts(a, time, night),
            // Drawn by `draw_ghosts`, see-through.
            AnimalKind::SpiritArcher => continue,
            _ => continue,
        };
        let (s, c) = a.heading.sin_cos();
        if a.kind == AnimalKind::FlyingSnake {
            draw_snake_body(a, time);
        }
        for (((fx, fy, fz), (hx, hy, hz), color), pitch) in parts {
            // Turn the model to face the heading: local x runs along it.
            let offset = vec3(fx * c - fz * s, fy, fx * s + fz * c);
            draw_box_tilted(a.pos + offset, vec3(hx, hy, hz), a.heading, pitch, color);
        }
    }
}

/// Draw the will-o-whisps and the warriors of the Wild Hunt in range, translucent. They go on after the water, with the other
/// translucent things, so that the water and the world behind them show through. Call while the 3D camera is active.
pub fn draw_ghosts(animals: &[Animal], time: f32, player_chunk: (i32, i32)) {
    for a in animals.iter().filter(|a| a.kind == AnimalKind::WillOWhisp || a.kind.is_wild_hunt() || a.kind == AnimalKind::SpiritArcher) {
        if !in_render_range(player_chunk, World::chunk_of_pos(a.pos)) {
            continue;
        }
        let (s, c) = a.heading.sin_cos();
        if a.kind == AnimalKind::SpiritArcher {
            for (((fx, fy, fz), (hx, hy, hz), color), pitch) in spirit_parts(a, time) {
                let offset = vec3(fx * c - fz * s, fy, fx * s + fz * c);
                draw_box_tilted(a.pos + offset, vec3(hx, hy, hz), a.heading, pitch, color);
            }
            continue;
        }
        if a.kind.is_wild_hunt() {
            for (((fx, fy, fz), (hx, hy, hz), color), pitch) in hunt_parts(a, time) {
                let offset = vec3(fx * c - fz * s, fy, fx * s + fz * c);
                draw_box_tilted(a.pos + offset, vec3(hx, hy, hz), a.heading, pitch, color);
            }
            continue;
        }
        for ((fx, fy, fz), (hx, hy, hz), color) in will_o_whisp_parts(a, time) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    /// A flying snake whose head has flown along a curve, so its body bends.
    fn coiled_snake(heading: f32) -> Animal {
        let mut a = Animal::new(AnimalKind::FlyingSnake, vec3(10.0, 40.0, 10.0), &mut Rng::new(3));
        a.heading = heading;
        a.trail = (0..SNAKE_SEGMENTS).map(|i| {
            let t = i as f32 * 0.09;
            vec3(10.0 - 3.6 * t.sin() - i as f32 * 0.12, 40.0 - 0.05 * i as f32 + 0.8 * (t * 1.7).sin(), 10.0 + 3.6 * (1.0 - t.cos()))
        }).collect();
        a
    }

    fn inside_any(boxes: &[(Vec3, Vec3)], p: Vec3) -> bool {
        boxes.iter().any(|&(lo, hi)| p.cmpge(lo).all() && p.cmple(hi).all())
    }

    #[test]
    fn every_part_of_a_drawn_flying_snake_is_a_target_wings_head_and_the_whole_body() {
        for heading in [0.0_f32, 0.7, 1.57, 2.4, 3.9] {
            let a = coiled_snake(heading);
            let boxes = a.hitboxes();
            let (s, c) = heading.sin_cos();
            // The head and the wings, whatever the beat of the wings: every corner of every part.
            for time in [0.0_f32, 0.03, 0.06, 0.1, 0.2] {
                for (((fx, fy, fz), (hx, hy, hz), _), pitch) in flying_snake_parts(&a, time, false) {
                    let reach_y = hz * pitch.abs().sin();
                    for (sx, sy, sz) in [(-1.0_f32, -1.0_f32, -1.0_f32), (1.0, 1.0, 1.0), (-1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (1.0, 1.0, -1.0), (-1.0, -1.0, 1.0)] {
                        let (lx, lz) = (fx + sx * hx, fz + sz * hz);
                        let p = a.pos + vec3(lx * c - lz * s, fy + sy * (hy + reach_y), lx * s + lz * c);
                        assert!(inside_any(&boxes, p), "heading {heading}, t {time}: part corner {p:?} is not a target");
                    }
                }
            }
            // The body: every box drawn, and every point on the line between the centres of neighbouring boxes, can be hit.
            let drawn = snake_body_boxes(&a, 0.0);
            for w in drawn.windows(2) {
                for k in 0..=4 {
                    let p = w[0].0.lerp(w[1].0, k as f32 / 4.0);
                    assert!(inside_any(&boxes, p), "heading {heading}: a gap in the body at {p:?}");
                }
            }
        }
    }

    #[test]
    fn a_ray_aimed_at_any_segment_of_a_flying_snakes_body_hits_it() {
        let a = coiled_snake(0.7);
        for (i, (centre, ..)) in snake_body_boxes(&a, 0.0).into_iter().enumerate() {
            for dir in [vec3(1.0, 0.2, 0.0), vec3(0.0, 0.3, 1.0), vec3(-0.7, -0.1, 0.7), vec3(0.0, 1.0, 0.0)] {
                let dir = dir.normalize();
                let origin = centre - dir * 3.0;
                let hit = a.hitboxes().iter().any(|&(lo, hi)| crate::world::ray_box(origin, dir, lo.to_array(), hi.to_array()).map_or(false, |t| t <= 3.0 + 1e-3));
                assert!(hit, "segment {i} was missed along {dir:?}");
            }
        }
    }

    #[test]
    fn the_big_metal_birds_have_wings_a_fanned_tail_a_hooked_beak_and_talons_and_the_vampire_bat_is_smaller() {
        for kind in [AnimalKind::IronQuail, AnimalKind::BronzeParrot] {
            let mut a = creature(kind);
            a.on_ground = true;
            let perched = models(&a, 0.0, false);
            // Folded wings (three long feather rows a side), five tail quills, a hooked beak and a ruff: 24 more than the old plate-and-rivet model (55).
            assert!(perched.len() > 70, "{kind:?}: {} boxes", perched.len());
            let longest_back = perched.iter().map(|((off, half, _), _)| off.0 - half.0).fold(f32::INFINITY, f32::min);
            assert!(longest_back < -1.5, "{kind:?}: the tail and folded primaries stream out behind: {longest_back}");
            let low = perched.iter().filter(|((off, half, _), _)| off.1 - half.1 < 0.12 && half.0 > 0.2 && half.2 < 0.06).count();
            assert!(low >= 6, "{kind:?}: talons on the ground ({low})");
            a.on_ground = false;
            a.mode = Mode::Fly;
            let flying = models(&a, 0.3, false);
            let wing_span = flying.iter().map(|((off, half, _), _)| off.2.abs() + half.2).fold(0.0_f32, f32::max);
            assert!(wing_span > 2.5, "{kind:?}: wings spread {wing_span} blocks out");
        }
        assert!(VAMPIRE_BAT_HEIGHT < 0.7, "lower than it was");
        let bat = creature(AnimalKind::VampireBat);
        let span = models(&bat, 0.0, false).iter().map(|((off, half, _), _)| off.2.abs() + half.2).fold(0.0_f32, f32::max);
        assert!(span < 2.0, "its wings reach {span} (they reached about 2.6 at the old size)");
    }

    #[test]
    fn the_newt_is_small_and_low_and_the_unicorn_is_a_tall_white_horse_with_a_golden_horn() {
        let newt = models(&creature(AnimalKind::Newt), 0.0, false);
        let reach = newt.iter().map(|((off, half, _), _)| (off.0.abs() + half.0).max(off.2.abs() + half.2)).fold(0.0_f32, f32::max);
        let top = newt.iter().map(|((off, half, _), _)| off.1 + half.1).fold(0.0_f32, f32::max);
        assert!(reach < 1.2 && top < 0.5, "a newt is small: {reach} long, {top} high");
        assert!(newt.iter().any(|((_, _, c), _)| c.r > 0.9 && c.g < 0.6 && c.b < 0.2), "orange beneath");
        let unicorn = models(&creature(AnimalKind::Unicorn), 0.0, false);
        let top = unicorn.iter().map(|((off, half, _), _)| off.1 + half.1).fold(0.0_f32, f32::max);
        assert!(top > 2.0, "tall: {top}");
        assert!(unicorn.iter().filter(|((_, _, c), _)| c.r > 0.9 && c.g > 0.9 && c.b > 0.9).count() >= 5, "white");
        assert!(unicorn.iter().filter(|((_, _, c), _)| c.r > 0.9 && c.g > 0.65 && c.b < 0.4).count() >= 4, "a golden horn");
    }

    #[test]
    fn the_newt_has_a_bright_back_and_stripe_so_it_stands_out_on_the_green_moss() {
        let newt = models(&creature(AnimalKind::Newt), 0.0, false);
        // The moss floor is (0.20, 0.46, 0.16): no part of the newt's back may be a dark green like it.
        let backs: Vec<_> = newt.iter().filter(|((off, _, _), _)| off.1 > 0.05).collect();
        assert!(backs.iter().all(|((_, _, c), _)| c.r + c.g + c.b > 0.5 || c.r < 0.15), "a dark green back is camouflage: {:?}", backs.iter().map(|((_, _, c), _)| (c.r, c.g, c.b)).collect::<Vec<_>>());
        assert!(newt.iter().any(|((off, _, c), _)| off.1 > 0.1 && c.r > 0.9 && c.g < 0.5 && c.b < 0.15), "an orange stripe down the back");
    }

    #[test]
    fn the_great_bear_is_drawn_bigger_and_darker_than_a_bear_with_glowing_eyes() {
        let bear = models(&creature(AnimalKind::Bear), 0.0, false);
        let boss = models(&creature(AnimalKind::BearBoss), 0.0, false);
        let top = |m: &Vec<Piece>| m.iter().map(|((off, half, _), _)| off.1 + half.1).fold(0.0_f32, f32::max);
        let reach = |m: &Vec<Piece>| m.iter().map(|((off, half, _), _)| (off.0.abs() + half.0).max(off.2.abs() + half.2)).fold(0.0_f32, f32::max);
        assert!(top(&boss) > top(&bear) * 1.5 && reach(&boss) > reach(&bear) * 1.5, "much bigger");
        // Its model fits its collision box, as a bear's does.
        assert!(top(&boss) <= AnimalKind::BearBoss.height() * 1.25, "{} high against a {} box", top(&boss), AnimalKind::BearBoss.height());
        let brightness = |m: &Vec<Piece>| {
            let furs: Vec<f32> = m.iter().filter(|((_, _, c), _)| c.a >= 1.0 && c.r < 0.9).map(|((_, _, c), _)| c.r + c.g + c.b).collect();
            furs.iter().sum::<f32>() / furs.len() as f32
        };
        assert!(brightness(&boss) < brightness(&bear) * 0.7, "darker fur");
        let glow = boss.iter().any(|((_, _, c), _)| c.r > 0.95 && c.g < 0.1 && c.b < 0.1);
        assert!(glow, "red eyes by day");
    }

    fn whisp() -> Animal {
        Animal::new(AnimalKind::WillOWhisp, vec3(5.5, 9.0, 5.5), &mut Rng::new(1))
    }

    #[test]
    fn a_will_o_whisp_is_see_through_with_a_halo_a_tail_and_eyes() {
        let a = whisp();
        for time in [0.0, 0.7, 2.3, 9.1] {
            let parts = will_o_whisp_parts(&a, time);
            assert!(parts.len() >= 9, "halo, body, heart, two eyes and a tail: {} parts", parts.len());
            assert!(parts.iter().all(|&(_, _, c)| c.a < 0.9), "every part is translucent at {time}");
            assert!(parts.iter().filter(|&&(_, _, c)| c.a <= 0.2).count() >= 4, "a faint halo and a fading tail");
            assert!(parts.iter().any(|&(_, _, c)| c.a >= 0.55), "a brighter heart");
            // The tail fades: each wisp is fainter and lower than the one before.
            let tail: Vec<_> = parts.iter().skip(5).collect();
            assert!(tail.windows(2).all(|w| w[1].2.a < w[0].2.a && w[1].0 .1 < w[0].0 .1), "the tail thins out and sinks");
        }
    }

    fn creature(kind: AnimalKind) -> Animal {
        Animal::new(kind, vec3(5.5, 6.0, 5.5), &mut Rng::new(3))
    }

    fn models(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
        match a.kind {
            AnimalKind::Deer => deer_parts(a, time, night),
            AnimalKind::Quail => quail_parts(a, time, night),
            AnimalKind::Crow => crow_parts(a, time, night),
            AnimalKind::IronQuail => iron_quail_parts(a, time, night),
            AnimalKind::BronzeParrot => bronze_parrot_parts(a, time, night),
            AnimalKind::Parrot => parrot_parts(a, time, night),
            AnimalKind::Bear => bear_parts(a, time, night),
            AnimalKind::Rabbit => rabbit_parts(a, time, night),
            AnimalKind::Moose => moose_parts(a, time, night),
            AnimalKind::Yeti => yeti_parts(a, time, night),
            AnimalKind::LochNessMonster => loch_ness_parts(a, time, night),
            AnimalKind::Bat => bat_parts(a, time, night),
            AnimalKind::Raven => raven_parts(a, time, night),
            AnimalKind::Phoenix => phoenix_parts(a, time, night),
            AnimalKind::VampireBat => vampire_bat_parts(a, time, night),
            AnimalKind::Orangutan => orangutan_parts(a, time, night),
            AnimalKind::Jaguar => jaguar_parts(a, time, night),
            AnimalKind::FlyingSnake => flying_snake_parts(a, time, night),
            AnimalKind::Newt => newt_parts(a, time, night),
            AnimalKind::Fairy => fairy_parts(a, time, night),
            AnimalKind::MountainGoat => goat_parts(a, time, night),
            AnimalKind::Unicorn => unicorn_parts(a, time, night),
            AnimalKind::BearBoss => bear_boss_parts(a, time, night),
            AnimalKind::Crawfish => crawfish_parts(a, time, night),
            AnimalKind::BigCrawfish => big_crawfish_parts(a, time, night),
            AnimalKind::GiantCrawfish => giant_crawfish_parts(a, time, night),
            AnimalKind::Dragon => dragon_parts(a, time, night),
            AnimalKind::SpiritArcher => spirit_parts(a, time),
            kind if kind.is_wild_hunt() => hunt_parts(a, time),
            AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer | AnimalKind::HuntBear | AnimalKind::HuntCrow => hunt_parts(a, time),
            AnimalKind::WillOWhisp => unreachable!(),
        }
    }

    #[test]
    fn the_bosses_are_big_and_have_two_glowing_red_eyes_at_night() {
        for kind in [AnimalKind::Yeti, AnimalKind::LochNessMonster] {
            let a = creature(kind);
            let pieces = models(&a, 0.0, true);
            assert!(pieces.len() > 15, "{kind:?} is a detailed model");
            let eyes = pieces.iter().filter(|((_, _, c), _)| c.a >= 1.0 && is_night_eye(*c)).count();
            assert_eq!(eyes, 2, "{kind:?}");
            let top = pieces.iter().map(|((off, half, _), _)| off.1 + half.1).fold(0.0_f32, f32::max);
            assert!(top > 2.0, "{kind:?} stands tall: {top}");
        }
    }

    const FIVE: [AnimalKind; 5] = [AnimalKind::Moose, AnimalKind::Deer, AnimalKind::Quail, AnimalKind::Bear, AnimalKind::Rabbit];

    #[test]
    fn the_five_animals_have_two_glowing_red_eyes_at_night_that_stick_out_of_the_head() {
        for kind in FIVE {
            let a = creature(kind);
            let pieces = models(&a, 0.0, true);
            let eyes: Vec<_> = pieces.iter().filter(|((_, _, c), _)| c.a >= 1.0 && is_night_eye(*c)).collect();
            assert_eq!(eyes.len(), 2, "{kind:?} has two red eyes at night");
            assert!(eyes.iter().any(|((o, _, _), _)| o.2 > 0.0) && eyes.iter().any(|((o, _, _), _)| o.2 < 0.0), "{kind:?}: one on each side");
            assert!(pieces.iter().any(|((_, _, c), _)| c.a < 1.0 && is_night_eye(*c)), "{kind:?} has a glow round the eyes");
            // The eye is not buried: it reaches farther out than any solid box it sits inside.
            for &((eo, eh, _), _) in &eyes {
                let outer = eo.2.abs() + eh.2;
                for &((o, h, c), _) in &pieces {
                    let inside = c.a >= 1.0
                        && !is_night_eye(c)
                        && (eo.0 - o.0).abs() < h.0
                        && (eo.1 - o.1).abs() < h.1
                        && o.2.abs() > 0.02 - 1.0
                        && (eo.2.abs() - o.2.abs()).abs() < h.2 + eh.2;
                    if inside && h.2 > eh.2 * 2.0 && (eo.1 - o.1).abs() < h.1 * 0.9 {
                        assert!(outer > o.2.abs() + h.2 - 1e-4, "{kind:?}: an eye is buried in a box reaching {}", o.2.abs() + h.2);
                    }
                }
            }
        }
    }

    #[test]
    fn by_day_nothing_glows_red() {
        for kind in FIVE {
            let a = creature(kind);
            assert!(models(&a, 0.0, false).iter().all(|((_, _, c), _)| !is_night_eye(*c)), "{kind:?}");
        }
    }

    #[test]
    fn the_five_animals_are_posed_differently_as_they_walk_and_the_legs_bend() {
        for kind in FIVE {
            let mut a = creature(kind);
            a.vel = vec3(1.0, 0.0, 0.0);
            let still = creature(kind);
            let at = |a: &Animal, phase: f32| {
                let mut a = a.clone();
                a.phase = phase;
                models(&a, 1.0, false)
            };
            let (p0, p1) = (at(&a, 0.0), at(&a, 0.9));
            assert_eq!(p0.len(), p1.len());
            let moved = p0.iter().zip(&p1).filter(|(x, y)| x.0 .0 != y.0 .0 || x.1 != y.1).count();
            assert!(moved >= 4, "{kind:?}: only {moved} boxes move as it walks");
            assert!(p0.iter().any(|(_, pitch)| pitch.abs() > 0.05) || p1.iter().any(|(_, pitch)| pitch.abs() > 0.05), "{kind:?} has tilted limbs");
            let rest = at(&still, 0.0);
            let rest2 = at(&still, 0.9);
            let same = rest.iter().zip(&rest2).filter(|(x, y)| x.0 .0 == y.0 .0).count();
            assert!(same >= rest.len() / 2, "{kind:?} stands still when it is not moving");
        }
    }

    #[test]
    fn a_hopping_rabbit_leaves_the_ground_and_a_grazing_deer_lowers_its_head() {
        let mut rabbit = creature(AnimalKind::Rabbit);
        rabbit.vel = vec3(2.0, 0.0, 0.0);
        let height = |phase: f32| {
            let mut r = rabbit.clone();
            r.phase = phase;
            rabbit_parts(&r, 0.0, false)[0].0 .0 .1
        };
        assert!((0..40).map(|i| height(i as f32 * 0.1)).fold(0.0_f32, f32::max) > 0.3, "it hops");
        let mut deer = creature(AnimalKind::Deer);
        deer.mode = Mode::Idle;
        let head_y = |time: f32| deer_parts(&deer, time, false).iter().map(|((o, _, _), _)| o.1).fold(0.0_f32, f32::max);
        let heights: Vec<f32> = (0..200).map(|i| head_y(i as f32 * 0.1)).collect();
        let (lo, hi) = (heights.iter().cloned().fold(f32::MAX, f32::min), heights.iter().cloned().fold(0.0, f32::max));
        assert!(hi - lo > 0.3, "the deer's highest point drops by {} when it grazes", hi - lo);
    }

    #[test]
    fn a_bat_has_two_wings_that_beat_and_red_eyes_at_night() {
        let a = creature(AnimalKind::Bat);
        let pieces = models(&a, 0.0, true);
        assert_eq!(pieces.iter().filter(|((_, _, c), _)| c.a >= 1.0 && is_night_eye(*c)).count(), 2);
        assert!(models(&a, 0.0, false).iter().all(|((_, _, c), _)| !is_night_eye(*c)), "nothing glows by day");
        let wing_tip = |time: f32| models(&a, time, false).iter().map(|((o, _, _), _)| o.1).fold(0.0_f32, f32::max);
        let heights: Vec<f32> = (0..40).map(|i| wing_tip(i as f32 * 0.013)).collect();
        let (lo, hi) = (heights.iter().cloned().fold(f32::MAX, f32::min), heights.iter().cloned().fold(0.0, f32::max));
        assert!(hi - lo > 0.05, "the wings beat ({lo}..{hi})");
        assert!(models(&a, 0.0, false).iter().map(|((o, h, _), _)| o.2.abs() + h.2).fold(0.0_f32, f32::max) > 0.4, "a wide wingspan");
    }

    #[test]
    fn the_warriors_of_the_wild_hunt_are_see_through_and_each_has_its_own_weapon() {
        let mut sizes = Vec::new();
        for kind in [AnimalKind::HuntSwordsman, AnimalKind::HuntArcher, AnimalKind::HuntLancer, AnimalKind::HuntBear, AnimalKind::HuntCrow] {
            let a = creature(kind);
            let parts = hunt_parts(&a, 0.5);
            assert!(parts.iter().all(|((_, _, c), _)| c.a < 0.9), "{kind:?}: every part is translucent");
            if matches!(kind, AnimalKind::HuntSwordsman | AnimalKind::HuntArcher | AnimalKind::HuntLancer) {
                assert!(parts.iter().filter(|((_, _, c), _)| c.a >= 0.8).count() >= 3, "{kind:?}: bright eyes and a gleaming weapon");
            }
            let reach = parts.iter().map(|((o, h, _), _)| o.0 + h.0).fold(0.0_f32, f32::max);
            sizes.push((kind, parts.len(), reach));
        }
        let sword = sizes[0].2;
        assert!(sizes[2].2 > sword + 0.5, "the lance reaches well past the sword: {sizes:?}");
        let top = |kind| hunt_parts(&creature(kind), 0.5).iter().map(|((o, h, _), _)| o.1 + h.1).fold(0.0_f32, f32::max);
        assert!(top(AnimalKind::HuntSwordsman) > 1.5 && top(AnimalKind::HuntSwordsman) < 2.6, "a man-sized warrior");
        assert!(top(AnimalKind::HuntLancer) > top(AnimalKind::HuntSwordsman) + 0.4, "a rider on a horse stands taller");
        // The lancer's lance is levelled at the charge and raised at rest.
        let mut lancer = creature(AnimalKind::HuntLancer);
        let rest = hunt_parts(&lancer, 0.0);
        lancer.hunt_phase = crate::animal::HuntPhase::Dash;
        assert_ne!(rest.iter().map(|((o, _, _), p)| (o.1 * 100.0) as i32 + (p * 100.0) as i32).collect::<Vec<_>>(), hunt_parts(&lancer, 0.0).iter().map(|((o, _, _), p)| (o.1 * 100.0) as i32 + (p * 100.0) as i32).collect::<Vec<_>>());
    }

    #[test]
    fn the_raven_and_the_phoenix_are_black_and_fiery_birds_on_the_wing() {
        let raven = models(&creature(AnimalKind::Raven), 0.3, false);
        assert!(raven.iter().filter(|((_, _, c), _)| c.r < 0.2 && c.g < 0.2).count() >= 4, "black plumage");
        let phoenix = models(&creature(AnimalKind::Phoenix), 0.3, false);
        assert!(phoenix.iter().filter(|((_, _, c), _)| c.r > 0.8 && c.b < 0.4).count() >= 8, "orange and gold plumage");
        let span = |p: &[Piece]| p.iter().map(|((o, h, _), _)| o.2.abs() + h.2).fold(0.0_f32, f32::max);
        assert!(span(&phoenix) > 4.0 * span(&raven) / 1.6 * 1.6 / 4.0 * 3.0, "a huge wingspan");
        assert!(span(&phoenix) > span(&raven) * 3.0);
    }

    #[test]
    fn the_yeti_lifts_its_fists_to_smash() {
        let mut yeti = creature(AnimalKind::Yeti);
        let top = |a: &Animal| models(a, 0.0, false).iter().map(|((o, h, _), _)| o.1 + h.1).fold(0.0_f32, f32::max);
        let calm = top(&yeti);
        yeti.action = crate::animal::BossAction::Smash;
        assert!(top(&yeti) > calm + 1.0, "its fists go up over its head");
    }

    #[test]
    fn the_iron_quail_is_made_of_metal_with_plates_rivets_and_a_moving_gleam() {
        let a = creature(AnimalKind::IronQuail);
        let pieces = models(&a, 0.0, false);
        let plain = models(&creature(AnimalKind::Quail), 0.0, false);
        assert!(pieces.len() > plain.len() + 20, "plates, seams and rivets on top of the bird: {} vs {}", pieces.len(), plain.len());
        // Steel is grey with a cold blue cast: little saturation, never a warm brown.
        let steel = |c: Color| (c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)) < 0.30 && c.b >= c.r - 0.01;
        let body: Vec<&Piece> = pieces.iter().filter(|((_, h, _), _)| h.0 * h.1 * h.2 > 0.008).collect();
        assert!(body.len() >= 6 && body.iter().all(|((_, _, c), _)| steel(*c)), "the large parts are steel-coloured");
        assert!(pieces.iter().filter(|((_, _, c), _)| c.r > 0.78 && c.g > 0.8 && c.b > 0.85).count() >= 8, "bright polished edges and rivets");
        assert!(pieces.iter().filter(|((_, _, c), _)| c.r < 0.14 && c.b < 0.2).count() >= 8, "dark seams between the plates");
        // The gleam moves: the same bird looks different a moment later.
        let at = |t: f32| models(&a, t, false).iter().map(|((_, _, c), _)| (c.r * 1000.0) as i32).collect::<Vec<_>>();
        assert_ne!(at(0.0), at(0.7), "light sweeps over the metal");
        assert!(models(&a, 0.0, false).iter().all(|((_, _, c), _)| !is_night_eye(*c)), "no red by day");
        assert_eq!(models(&a, 0.0, true).iter().filter(|((_, _, c), _)| c.a >= 1.0 && is_night_eye(*c)).count(), 2, "its eyes still glow at night");
    }

    #[test]
    fn a_will_o_whisp_flickers_over_time() {
        let a = whisp();
        let alpha = |t: f32| will_o_whisp_parts(&a, t)[1].2.a;
        let samples: Vec<f32> = (0..20).map(|i| alpha(i as f32 * 0.13)).collect();
        let (lo, hi) = (samples.iter().cloned().fold(f32::MAX, f32::min), samples.iter().cloned().fold(f32::MIN, f32::max));
        assert!(hi - lo > 0.05, "the glow flickers ({lo}..{hi})");
    }
}
