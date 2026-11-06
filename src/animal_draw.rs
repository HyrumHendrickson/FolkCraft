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

/// The Iron Quail: a quail more than five times the size, rust-brown, with a crest of gold.
fn iron_quail_parts(a: &Animal, time: f32, night: bool) -> Vec<Piece> {
    const SCALE: f32 = 5.5;
    let mut parts = bird_parts(a, time, night, Plumage {
        feathers: col(0.50, 0.36, 0.24),
        speckle: col(0.30, 0.20, 0.13),
        belly: col(0.82, 0.68, 0.46),
        face: col(0.26, 0.18, 0.13),
        beak: col(0.90, 0.70, 0.18),
        legs: col(0.80, 0.50, 0.16),
    });
    for ((off, half, _), _) in parts.iter_mut() {
        *off = (off.0 * SCALE, off.1 * SCALE, off.2 * SCALE);
        *half = (half.0 * SCALE, half.1 * SCALE, half.2 * SCALE);
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
    let flying = a.mode == Mode::Fly || (a.kind == AnimalKind::IronQuail && a.is_airborne()) || a.kind.flies_always();
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
            AnimalKind::Bear => bear_parts(a, time, night),
            AnimalKind::Rabbit => rabbit_parts(a, time, night),
            AnimalKind::Moose => moose_parts(a, time, night),
            AnimalKind::Yeti => yeti_parts(a, time, night),
            AnimalKind::LochNessMonster => loch_ness_parts(a, time, night),
            AnimalKind::Bat => bat_parts(a, time, night),
            AnimalKind::Raven => raven_parts(a, time, night),
            AnimalKind::Phoenix => phoenix_parts(a, time, night),
            _ => continue,
        };
        let (s, c) = a.heading.sin_cos();
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
    for a in animals.iter().filter(|a| a.kind == AnimalKind::WillOWhisp || a.kind.is_wild_hunt()) {
        if !in_render_range(player_chunk, World::chunk_of_pos(a.pos)) {
            continue;
        }
        let (s, c) = a.heading.sin_cos();
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
            AnimalKind::Bear => bear_parts(a, time, night),
            AnimalKind::Rabbit => rabbit_parts(a, time, night),
            AnimalKind::Moose => moose_parts(a, time, night),
            AnimalKind::Yeti => yeti_parts(a, time, night),
            AnimalKind::LochNessMonster => loch_ness_parts(a, time, night),
            AnimalKind::Bat => bat_parts(a, time, night),
            AnimalKind::Raven => raven_parts(a, time, night),
            AnimalKind::Phoenix => phoenix_parts(a, time, night),
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
    fn a_will_o_whisp_flickers_over_time() {
        let a = whisp();
        let alpha = |t: f32| will_o_whisp_parts(&a, t)[1].2.a;
        let samples: Vec<f32> = (0..20).map(|i| alpha(i as f32 * 0.13)).collect();
        let (lo, hi) = (samples.iter().cloned().fold(f32::MAX, f32::min), samples.iter().cloned().fold(f32::MIN, f32::max));
        assert!(hi - lo > 0.05, "the glow flickers ({lo}..{hi})");
    }
}
