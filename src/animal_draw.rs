//! Drawing for animals and arrows: simple box models, posed each frame, so chunk meshes stay static.

use macroquad::prelude::*;

use crate::animal::{Animal, AnimalKind, Mode};
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
    let feathers = col(0.56, 0.46, 0.34);
    let speckle = col(0.38, 0.30, 0.22);
    let belly = col(0.80, 0.70, 0.52);
    let face = col(0.34, 0.26, 0.20);
    let flying = a.mode == Mode::Fly;
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
    parts.push(tilted((head.0 + 0.08, head.1 - 0.015, 0.0), (0.03, 0.015, 0.02), col(0.85, 0.65, 0.20), peck * -0.9));
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
            parts.push(tilted((-0.04, 0.09, 0.04 * side), (0.012, 0.045, 0.012), col(0.85, 0.55, 0.20), 0.9));
        }
    } else {
        for side in [-1.0, 1.0] {
            parts.push(block((0.0, 0.20, 0.115 * side), (0.09, 0.05, 0.016), speckle));
            let stride = (a.phase * 6.0 + if side > 0.0 { 0.0 } else { std::f32::consts::PI }).sin() * 0.5 * moving;
            leg(&mut parts, (0.0, 0.10, 0.045 * side), 0.05, 0.05, 0.011, stride, col(0.85, 0.55, 0.20), col(0.85, 0.55, 0.20));
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
            AnimalKind::WillOWhisp => continue, // drawn by `draw_ghosts`, over the water
            AnimalKind::Deer => deer_parts(a, time, night),
            AnimalKind::Quail => quail_parts(a, time, night),
            AnimalKind::Bear => bear_parts(a, time, night),
            AnimalKind::Rabbit => rabbit_parts(a, time, night),
            AnimalKind::Moose => moose_parts(a, time, night),
        };
        let (s, c) = a.heading.sin_cos();
        for (((fx, fy, fz), (hx, hy, hz), color), pitch) in parts {
            // Turn the model to face the heading: local x runs along it.
            let offset = vec3(fx * c - fz * s, fy, fx * s + fz * c);
            draw_box_tilted(a.pos + offset, vec3(hx, hy, hz), a.heading, pitch, color);
        }
    }
}

/// Draw the will-o-whisps in range, translucent. They go on after the water, with the other translucent things, so
/// that the water and the world behind them show through. Call while the 3D camera is active.
pub fn draw_ghosts(animals: &[Animal], time: f32, player_chunk: (i32, i32)) {
    for a in animals.iter().filter(|a| a.kind == AnimalKind::WillOWhisp) {
        if !in_render_range(player_chunk, World::chunk_of_pos(a.pos)) {
            continue;
        }
        let (s, c) = a.heading.sin_cos();
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
            AnimalKind::Bear => bear_parts(a, time, night),
            AnimalKind::Rabbit => rabbit_parts(a, time, night),
            AnimalKind::Moose => moose_parts(a, time, night),
            AnimalKind::WillOWhisp => unreachable!(),
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
    fn a_will_o_whisp_flickers_over_time() {
        let a = whisp();
        let alpha = |t: f32| will_o_whisp_parts(&a, t)[1].2.a;
        let samples: Vec<f32> = (0..20).map(|i| alpha(i as f32 * 0.13)).collect();
        let (lo, hi) = (samples.iter().cloned().fold(f32::MAX, f32::min), samples.iter().cloned().fold(f32::MIN, f32::max));
        assert!(hi - lo > 0.05, "the glow flickers ({lo}..{hi})");
    }
}
