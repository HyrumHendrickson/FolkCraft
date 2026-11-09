//! Inventory HUD: hotbar, full inventory screen (drag and drop, crafting panel), item
//! icons and pickup messages.

use macroquad::prelude::*;

use crate::config::*;
use crate::inventory::Inventory;
use crate::item::{ItemKind, ItemStack};
use crate::landmarks::compass_needle;
use crate::recipe::{craft, craftable, Recipe, RECIPES};
use crate::ui::ACCENT;

const SLOT_SIZE: f32 = 48.0;
const SLOT_GAP: f32 = 4.0;
const COLUMNS: usize = HOTBAR_SLOTS;
/// Extra space between the main grid and the hotbar row on the inventory screen.
const HOTBAR_SEPARATION: f32 = 14.0;
/// Crafting panel (right of the slot grid on the inventory screen).
const CRAFT_W: f32 = 310.0;
const CRAFT_GAP: f32 = 22.0;
const CRAFT_ROW_H: f32 = 56.0;
const CRAFT_ROW_GAP: f32 = 6.0;
/// Rows of the recipe book are taller: a name, the ingredients and what is still missing.
const GUIDE_ROW_H: f32 = 66.0;

fn lighten(c: Color, amount: f32) -> Color {
    Color::new((c.r + amount).min(1.0), (c.g + amount).min(1.0), (c.b + amount).min(1.0), c.a)
}

/// `c` scaled towards black by `factor` (below 1 darkens).
fn darken(c: Color, factor: f32) -> Color {
    Color::new(c.r * factor, c.g * factor, c.b * factor, c.a)
}

const OUTLINE: Color = Color::new(0.05, 0.05, 0.05, 0.85);

/// A round, shaded blob lit from the upper left: dark outline, a darker underside, the body, and a small highlight.
fn ball(x: f32, y: f32, r: f32, color: Color) {
    draw_circle(x, y, r + 1.2, OUTLINE);
    draw_circle(x, y, r, darken(color, 0.72));
    draw_circle(x - r * 0.12, y - r * 0.14, r * 0.86, color);
    draw_circle(x - r * 0.34, y - r * 0.38, r * 0.30, lighten(color, 0.20));
}

/// A line with an outline, a body and a thin highlight along its upper side.
fn rod(a: Vec2, b: Vec2, width: f32, color: Color) {
    draw_line(a.x, a.y, b.x, b.y, width + 2.2, OUTLINE);
    draw_line(a.x, a.y, b.x, b.y, width, darken(color, 0.82));
    draw_line(a.x - 0.6, a.y - 0.6, b.x - 0.6, b.y - 0.6, width * 0.55, color);
    draw_line(a.x - width * 0.18, a.y - width * 0.25, b.x - width * 0.18, b.y - width * 0.25, (width * 0.18).max(1.0), lighten(color, 0.20));
}

/// An outlined box: a lighter top edge, a darker bottom edge, and the body between.
fn slab(x: f32, y: f32, w: f32, h: f32, color: Color) {
    draw_rectangle(x - 1.5, y - 1.5, w + 3.0, h + 3.0, OUTLINE);
    draw_rectangle(x, y, w, h, color);
    draw_rectangle(x, y + h - (h * 0.22).max(2.0), w, (h * 0.22).max(2.0), darken(color, 0.80));
    draw_rectangle(x, y, w, (h * 0.16).max(1.5), lighten(color, 0.16));
    draw_rectangle(x, y, (w * 0.10).max(1.5), h, lighten(color, 0.07));
}

/// A filled quadrilateral with an outline.
fn quad(p: [Vec2; 4], color: Color) {
    draw_triangle(p[0], p[1], p[2], color);
    draw_triangle(p[0], p[2], p[3], color);
    for i in 0..4 {
        let (u, v) = (p[i], p[(i + 1) % 4]);
        draw_line(u.x, u.y, v.x, v.y, 1.6, OUTLINE);
    }
}

/// A filled triangle with an outline.
fn tri(a: Vec2, b: Vec2, c: Vec2, color: Color) {
    draw_triangle(a, b, c, color);
    for (u, v) in [(a, b), (b, c), (c, a)] {
        draw_line(u.x, u.y, v.x, v.y, 1.4, OUTLINE);
    }
}

/// Draw an icon for an item inside `r`, lit from the upper left.
pub fn draw_item_icon(kind: ItemKind, r: Rect) {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let color = kind.color();
    let outline = OUTLINE;
    let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
    let u = r.w / 48.0;
    match kind {
        ItemKind::Stick => {
            rod(p(0.24, 0.78), p(0.76, 0.22), 5.0 * u, color);
            let twig = p(0.52, 0.50);
            rod(twig, p(0.66, 0.60), 2.6 * u, color);
            draw_circle(twig.x - 3.0 * u, twig.y + 3.0 * u, 1.6 * u, darken(color, 0.55));
            draw_line(p(0.30, 0.70).x, p(0.30, 0.70).y, p(0.34, 0.66).x, p(0.34, 0.66).y, 1.2, darken(color, 0.6));
        }
        ItemKind::Pebble => {
            for (dx, dy, rad, shade) in [(-0.12, 0.10, 0.12, 0.0), (0.12, 0.12, 0.09, -0.05), (0.01, -0.10, 0.10, 0.05)] {
                ball(cx + dx * r.w, cy + dy * r.h, rad * r.w, Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0));
            }
            draw_circle(cx - 0.10 * r.w, cy + 0.13 * r.h, 0.9 * u, darken(color, 0.55));
        }
        ItemKind::PeepStone => {
            let rad = r.w * 0.27;
            ball(cx, cy + 1.0, rad, color);
            draw_circle_lines(cx, cy + 1.0, rad * 0.62, 1.4, Color::new(0.70, 0.94, 1.0, 0.75));
            draw_circle(cx + rad * 0.20, cy + rad * 0.25, rad * 0.22, Color::new(0.80, 0.98, 1.0, 0.45));
            // A glint of light, as on a gem.
            let g = p(0.40, 0.36);
            draw_line(g.x - 3.0 * u, g.y, g.x + 3.0 * u, g.y, 1.2, WHITE);
            draw_line(g.x, g.y - 3.0 * u, g.x, g.y + 3.0 * u, 1.2, WHITE);
        }
        ItemKind::HollyRod => {
            rod(p(0.25, 0.78), p(0.75, 0.22), 4.5 * u, color);
            let leaf = Color::new(0.10, 0.42, 0.16, 1.0);
            for (fx, fy, dir) in [(0.40, 0.58, -1.0_f32), (0.56, 0.42, 1.0), (0.64, 0.32, -1.0)] {
                let at = p(fx, fy);
                let tip = at + vec2(dir * 8.0 * u, -dir * 3.0 * u + 1.0);
                let side = vec2(4.0 * u, 4.0 * u);
                tri(at - side * 0.5, at + side * 0.5, tip, leaf);
                draw_line(at.x, at.y, tip.x, tip.y, 1.0, lighten(leaf, 0.25));
            }
            for (dx, dy) in [(0.37, 0.64), (0.47, 0.50), (0.60, 0.37)] {
                let b = p(dx, dy);
                ball(b.x, b.y, 2.8 * u, Color::new(0.82, 0.10, 0.16, 1.0));
            }
        }
        ItemKind::Stone => {
            let rad = r.w * 0.27;
            ball(cx, cy + 1.0, rad, color);
            draw_line(cx - rad * 0.2, cy - rad * 0.2, cx + rad * 0.1, cy + rad * 0.35, 1.2, darken(color, 0.55));
            draw_line(cx + rad * 0.1, cy + rad * 0.35, cx + rad * 0.45, cy + rad * 0.30, 1.2, darken(color, 0.55));
            draw_circle(cx + rad * 0.40, cy - rad * 0.10, 1.1 * u, darken(color, 0.62));
        }
        ItemKind::Log => {
            let (a, b) = (p(0.22, 0.74), p(0.72, 0.30));
            rod(a, b, 13.0 * u, color);
            // Furrows of bark running along the log.
            for off in [-3.5_f32, 0.5, 4.0] {
                draw_line(a.x + off * u + 4.0, a.y - off * u - 4.0, b.x + off * u - 4.0, b.y - off * u + 4.0, 1.0, darken(color, 0.58));
            }
            // The cut end, with growth rings and a crack.
            draw_circle(b.x, b.y, 8.4 * u, outline);
            draw_circle(b.x, b.y, 7.4 * u, Color::new(0.76, 0.60, 0.36, 1.0));
            draw_circle_lines(b.x, b.y, 5.0 * u, 1.0, Color::new(0.52, 0.38, 0.20, 1.0));
            draw_circle_lines(b.x, b.y, 2.6 * u, 1.0, Color::new(0.52, 0.38, 0.20, 1.0));
            draw_line(b.x, b.y, b.x + 5.0 * u, b.y - 2.0 * u, 1.0, Color::new(0.46, 0.32, 0.16, 1.0));
        }
        ItemKind::StoneAxe | ItemKind::CopperAxe | ItemKind::BronzeAxe | ItemKind::IronAxe => {
            let (h0, h1) = (p(0.24, 0.82), p(0.66, 0.32));
            rod(h0, h1, 4.6 * u, ItemKind::Stick.color());
            let (a, b, c, d) = (p(0.44, 0.22), p(0.72, 0.10), p(0.90, 0.42), p(0.62, 0.52));
            quad([a, b, c, d], color);
            // A bevelled face: a lighter upper half, and a bright cutting edge along the outer side.
            draw_triangle(a, b, (a + c) / 2.0, lighten(color, 0.12));
            draw_line(b.x, b.y, c.x, c.y, 2.4, lighten(color, 0.30));
            draw_line(a.x, a.y, d.x, d.y, 1.4, darken(color, 0.65));
            // The head is lashed to the handle with twine.
            let bind = p(0.54, 0.44);
            draw_line(bind.x - 4.0 * u, bind.y - 2.5 * u, bind.x + 4.0 * u, bind.y + 3.0 * u, 2.4, ItemKind::Twine.color());
        }
        ItemKind::Grass => {
            let blade = |bx: f32, tx: f32, ty: f32, shade: f32| {
                let c = Color::new((color.r + shade).clamp(0.0, 1.0), (color.g + shade).clamp(0.0, 1.0), (color.b + shade).clamp(0.0, 1.0), 1.0);
                let (base, tip) = (p(bx, 0.82), p(tx, ty));
                let w = 3.4 * u;
                tri(base - vec2(w, 0.0), base + vec2(w, 0.0), tip, c);
                draw_line(base.x, base.y, tip.x, tip.y, 1.0, lighten(c, 0.20));
            };
            blade(0.34, 0.22, 0.20, -0.08);
            blade(0.66, 0.80, 0.24, -0.04);
            blade(0.50, 0.50, 0.10, 0.05);
            blade(0.42, 0.40, 0.34, 0.0);
        }
        ItemKind::Berries | ItemKind::PurpleBerries => {
            draw_line(cx, cy - 0.30 * r.h, cx + 0.07 * r.w, cy - 0.40 * r.h, 2.5, Color::new(0.30, 0.20, 0.10, 1.0));
            let leaf = Color::new(0.22, 0.58, 0.20, 1.0);
            tri(p(0.55, 0.14), p(0.76, 0.12), p(0.64, 0.26), leaf);
            for (dx, dy, rad) in [(-0.14, 0.10, 0.13), (0.14, 0.08, 0.12), (0.0, -0.12, 0.13)] {
                ball(cx + dx * r.w, cy + dy * r.h, rad * r.w, color);
                draw_circle(cx + dx * r.w + rad * r.w * 0.45, cy + dy * r.h - rad * r.w * 0.55, 0.9 * u, darken(color, 0.4));
            }
        }
        ItemKind::Twine => {
            // A coil: rings of twisted strands, with a loose end trailing off.
            for (rad, w) in [(0.26, 7.0), (0.17, 5.0)] {
                draw_circle_lines(cx, cy, r.w * rad, w * u + 2.0, outline);
            }
            for (rad, w) in [(0.26, 7.0), (0.17, 5.0)] {
                draw_circle_lines(cx, cy, r.w * rad, w * u, darken(color, 0.85));
                draw_circle_lines(cx - 0.6, cy - 0.6, r.w * rad, w * u * 0.5, lighten(color, 0.08));
            }
            for i in 0..14 {
                let a = i as f32 * std::f32::consts::TAU / 14.0;
                let (x0, y0) = (cx + a.cos() * r.w * 0.20, cy + a.sin() * r.w * 0.20);
                let (x1, y1) = (cx + (a + 0.35).cos() * r.w * 0.29, cy + (a + 0.35).sin() * r.w * 0.29);
                draw_line(x0, y0, x1, y1, 1.0, darken(color, 0.62));
            }
            draw_circle(cx, cy, 3.0 * u, darken(color, 0.6));
            rod(p(0.72, 0.58), p(0.88, 0.80), 3.0 * u, color);
        }
        ItemKind::BowDrill => {
            let bow = [p(0.72, 0.16), p(0.85, 0.50), p(0.72, 0.84)];
            for w in bow.windows(2) {
                rod(w[0], w[1], 3.6 * u, color);
            }
            draw_line(bow[0].x, bow[0].y, bow[2].x, bow[2].y, 1.5, ItemKind::Twine.color());
            // The spindle, looped in the string, with a charred tip on the fireboard.
            let (s0, s1) = (p(0.22, 0.30), p(0.72, 0.52));
            rod(s0, s1, 3.4 * u, lighten(color, 0.10));
            slab(r.x + r.w * 0.10, r.y + r.h * 0.68, r.w * 0.44, r.h * 0.13, Color::new(0.46, 0.31, 0.15, 1.0));
            draw_circle(r.x + r.w * 0.28, r.y + r.h * 0.69, 2.0 * u, Color::new(0.10, 0.07, 0.05, 1.0));
        }
        ItemKind::Plank => {
            let (x, y, w, h) = (r.x + r.w * 0.12, r.y + r.h * 0.33, r.w * 0.76, r.h * 0.34);
            slab(x, y, w, h, color);
            for fy in [0.32, 0.50, 0.70] {
                draw_line(x + 4.0, y + h * fy, x + w - 4.0, y + h * fy, 1.0, darken(color, 0.76));
            }
            draw_circle_lines(x + w * 0.62, y + h * 0.52, 2.6 * u, 1.0, darken(color, 0.70));
            for nx in [x + 4.5 * u, x + w - 4.5 * u] {
                draw_circle(nx, y + h / 2.0, 1.7 * u, Color::new(0.22, 0.22, 0.25, 1.0));
                draw_circle(nx - 0.5, y + h / 2.0 - 0.5, 0.7 * u, Color::new(0.65, 0.65, 0.70, 1.0));
            }
        }
        ItemKind::Feather => {
            let (tip, base) = (p(0.82, 0.14), p(0.20, 0.86));
            // The vane: a leaf shape either side of the quill, drawn as a fan of barbs.
            let n = (tip - base).normalize();
            let perp = vec2(-n.y, n.x);
            for i in 0..14 {
                let t = 0.12 + i as f32 / 14.0 * 0.84;
                let on = base + (tip - base) * t;
                let width = (t * (1.0 - t)).sqrt() * 0.55 * r.w * 0.30 * 2.0;
                let shade = if i % 2 == 0 { 0.0 } else { -0.05 };
                let c = Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0);
                draw_line(on.x - perp.x * width * 0.6, on.y - perp.y * width * 0.6 - 1.5 * u, on.x + perp.x * width, on.y + perp.y * width, 3.2 * u, c);
            }
            draw_line(base.x, base.y, tip.x, tip.y, 2.6 * u, outline);
            draw_line(base.x, base.y, tip.x, tip.y, 1.3 * u, Color::new(0.80, 0.76, 0.66, 1.0));
        }
        ItemKind::Bow => {
            let arc = [p(0.30, 0.10), p(0.50, 0.28), p(0.58, 0.50), p(0.50, 0.72), p(0.30, 0.90)];
            for w in arc.windows(2) {
                rod(w[0], w[1], 5.0 * u, color);
            }
            // A leather-wrapped grip, and the string.
            let (g0, g1) = (p(0.57, 0.43), p(0.57, 0.57));
            draw_line(g0.x, g0.y, g1.x, g1.y, 6.0 * u, ItemKind::Leather.color());
            draw_line(arc[0].x, arc[0].y, arc[4].x, arc[4].y, 1.6, lighten(ItemKind::Twine.color(), 0.15));
            for e in [arc[0], arc[4]] {
                draw_circle(e.x, e.y, 2.0 * u, darken(color, 0.55));
            }
        }
        ItemKind::Arrow => {
            let (tail, tip) = (p(0.20, 0.80), p(0.76, 0.24));
            rod(tail, tip, 3.0 * u, color);
            let dir = (tip - tail).normalize();
            let perp = vec2(-dir.y, dir.x);
            let head = ItemKind::Stone.color();
            tri(tip + dir * 9.0 * u, tip + perp * 5.0 * u - dir * 3.0 * u, tip - perp * 5.0 * u - dir * 3.0 * u, head);
            draw_line(tip.x + dir.x * 8.0 * u, tip.y + dir.y * 8.0 * u, tip.x - perp.x * 3.0 * u, tip.y - perp.y * 3.0 * u, 1.0, lighten(head, 0.25));
            for k in 0..3 {
                let at = tail + dir * (k as f32 * 4.5 * u);
                for s in [-1.0_f32, 1.0] {
                    tri(at, at + perp * s * 6.0 * u - dir * 2.0 * u, at + dir * 6.0 * u + perp * s * 1.0, ItemKind::Feather.color());
                }
            }
        }
        ItemKind::Venison | ItemKind::CookedVenison | ItemKind::RabbitMeat | ItemKind::CookedRabbit | ItemKind::MooseMeat | ItemKind::CookedMoose => {
            let cooked = matches!(kind, ItemKind::CookedVenison | ItemKind::CookedRabbit | ItemKind::CookedMoose);
            // A steak: a thick lobe with a pale rim of fat and marbling (raw), or a seared crust with grill marks (cooked).
            let rim = if cooked { darken(color, 0.62) } else { Color::new(0.93, 0.82, 0.78, 1.0) };
            draw_circle(cx - r.w * 0.05, cy + 1.0, r.w * 0.32, outline);
            draw_circle(cx - r.w * 0.05, cy + 1.0, r.w * 0.30, rim);
            draw_circle(cx - r.w * 0.03, cy + 0.5, r.w * 0.255, color);
            draw_circle(cx - r.w * 0.12, cy - r.h * 0.08, r.w * 0.09, lighten(color, 0.14));
            draw_circle(cx + r.w * 0.20, cy + r.h * 0.10, r.w * 0.07, darken(color, 0.85));
            if cooked {
                for k in 0..3 {
                    let o = (k as f32 - 1.0) * 5.5 * u;
                    draw_line(cx - 8.0 * u + o, cy - 6.0 * u, cx + 3.0 * u + o, cy + 8.0 * u, 2.0, Color::new(0.20, 0.10, 0.05, 1.0));
                }
            } else {
                draw_line(cx - 8.0 * u, cy + 3.0 * u, cx + 4.0 * u, cy + 6.0 * u, 1.6, Color::new(0.94, 0.86, 0.84, 1.0));
                draw_line(cx - 3.0 * u, cy - 5.0 * u, cx + 8.0 * u, cy - 2.0 * u, 1.4, Color::new(0.94, 0.86, 0.84, 1.0));
            }
        }
        ItemKind::Poultry | ItemKind::CookedPoultry => {
            let cooked = kind == ItemKind::CookedPoultry;
            let bone = Color::new(0.94, 0.91, 0.84, 1.0);
            let (b0, b1) = (vec2(cx + r.w * 0.02, cy + r.h * 0.02), vec2(cx + r.w * 0.30, cy + r.h * 0.30));
            rod(b0, b1, 4.5 * u, bone);
            ball(b1.x + 2.5 * u, b1.y + 1.0, 3.8 * u, bone);
            ball(b1.x - 1.0 * u, b1.y + 4.0 * u, 3.4 * u, bone);
            ball(cx - r.w * 0.08, cy - r.h * 0.06, r.w * 0.25, color);
            if cooked {
                draw_circle(cx - r.w * 0.02, cy - r.h * 0.02, r.w * 0.06, darken(color, 0.72));
                draw_line(cx - r.w * 0.20, cy, cx - r.w * 0.04, cy + r.h * 0.12, 1.6, darken(color, 0.55));
            } else {
                draw_circle(cx - r.w * 0.04, cy - r.h * 0.04, r.w * 0.05, lighten(color, 0.10));
            }
        }
        ItemKind::Leather => {
            let (x, y, w, h) = (r.x + r.w * 0.16, r.y + r.h * 0.26, r.w * 0.68, r.h * 0.48);
            slab(x, y, w, h, color);
            draw_rectangle(x + w * 0.55, y + h * 0.20, w * 0.28, h * 0.30, darken(color, 0.88));
            draw_rectangle(x + w * 0.10, y + h * 0.52, w * 0.30, h * 0.28, darken(color, 0.90));
            for i in 0..6 {
                let sx = x + 4.0 + i as f32 * (w - 8.0) / 5.0;
                draw_line(sx, y + 3.5, sx + 2.5, y + 3.5, 1.4, lighten(color, 0.30));
                draw_line(sx, y + h - 3.5, sx + 2.5, y + h - 3.5, 1.4, lighten(color, 0.30));
            }
        }
        ItemKind::BearMeat | ItemKind::CookedBearMeat => {
            let cooked = kind == ItemKind::CookedBearMeat;
            let (x, y, w, h) = (r.x + r.w * 0.12, r.y + r.h * 0.30, r.w * 0.62, r.h * 0.44);
            let bone = Color::new(0.94, 0.91, 0.84, 1.0);
            rod(vec2(x + w - 2.0, y + h * 0.40), vec2(x + w + 6.0 * u, y + h * 0.40), 4.0 * u, bone);
            ball(x + w + 8.0 * u, y + h * 0.40, 4.2 * u, bone);
            slab(x, y, w, h, color);
            let fat = if cooked { lighten(color, 0.14) } else { Color::new(0.94, 0.84, 0.80, 1.0) };
            draw_rectangle(x + 1.0, y + h * 0.58, w - 2.0, h * 0.16, fat);
            if cooked {
                for k in 0..3 {
                    draw_line(x + 6.0 * u + k as f32 * 8.0 * u, y + 3.0, x + 12.0 * u + k as f32 * 8.0 * u, y + h * 0.5, 1.8, Color::new(0.18, 0.09, 0.05, 1.0));
                }
            }
        }
        ItemKind::BearPelt => {
            let (x, y, w, h) = (r.x + r.w * 0.14, r.y + r.h * 0.20, r.w * 0.72, r.h * 0.52);
            // Paws sticking out at the corners, a shaggy lower edge, and streaks of fur.
            for (px, py) in [(x - 2.0, y + 1.0), (x + w - 5.0, y + 1.0)] {
                draw_circle(px + 3.0, py + 3.0, 5.0 * u, outline);
                draw_circle(px + 3.0, py + 3.0, 4.0 * u, darken(color, 0.8));
            }
            slab(x, y, w, h, color);
            for i in 0..7 {
                let fx = x + 3.0 + i as f32 * (w - 6.0) / 6.0;
                let drop = 4.0 + (i % 2) as f32 * 3.5;
                draw_line(fx, y + h - 1.0, fx - 0.5, y + h + drop, 4.2, outline);
                draw_line(fx, y + h - 1.0, fx - 0.5, y + h + drop - 1.0, 2.8, darken(color, 0.9));
            }
            for i in 0..5 {
                let fx = x + 6.0 + i as f32 * (w - 12.0) / 4.0;
                draw_line(fx, y + 5.0, fx + 2.0, y + h - 7.0, 1.4, lighten(color, 0.14));
                draw_line(fx + 3.0, y + 8.0, fx + 4.0, y + h - 4.0, 1.0, darken(color, 0.70));
            }
        }
        ItemKind::Bellows => {
            let (a0, a1, b0, b1) = (p(0.14, 0.62), p(0.52, 0.24), p(0.30, 0.84), p(0.68, 0.46));
            let skin = Color::new(0.62, 0.40, 0.22, 1.0);
            draw_triangle(a0, a1, b1, skin);
            draw_triangle(a0, b1, b0, darken(skin, 0.88));
            for t in [0.2, 0.4, 0.6, 0.8] {
                let (u0, v0) = (a0 + (a1 - a0) * t, b0 + (b1 - b0) * t);
                draw_line(u0.x, u0.y, v0.x, v0.y, 1.6, darken(skin, 0.55));
            }
            for (s, e) in [(a0, a1), (b0, b1)] {
                rod(s, e, 4.4 * u, color);
            }
            // A handle on the upper board and an iron nozzle.
            rod(p(0.20, 0.52), p(0.12, 0.42), 2.6 * u, color);
            rod(p(0.62, 0.36), p(0.90, 0.14), 4.4 * u, Color::new(0.42, 0.43, 0.48, 1.0));
        }
        ItemKind::Ash => {
            for (dx, dy, rad, shade) in [(-0.16, 0.14, 0.15, 0.0), (0.14, 0.12, 0.13, 0.05), (0.0, 0.0, 0.17, 0.08), (-0.04, 0.20, 0.10, -0.04)] {
                ball(cx + dx * r.w, cy + dy * r.h, rad * r.w, Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0));
            }
            for (dx, dy) in [(-0.10, 0.02), (0.08, 0.14), (0.14, -0.04)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, 1.1 * u, darken(color, 0.55));
            }
        }
        ItemKind::Clay => {
            draw_circle(cx, cy + 3.0, r.w * 0.30, outline);
            draw_circle(cx, cy + 3.0, r.w * 0.28, darken(color, 0.78));
            draw_circle(cx, cy - 0.5, r.w * 0.26, color);
            draw_circle(cx - r.w * 0.09, cy - r.h * 0.10, r.w * 0.09, lighten(color, 0.16));
            // Thumb prints in the damp surface, and a glisten.
            draw_circle_lines(cx + r.w * 0.08, cy + r.h * 0.04, 2.6 * u, 1.0, darken(color, 0.78));
            draw_circle_lines(cx - r.w * 0.04, cy + r.h * 0.12, 2.0 * u, 1.0, darken(color, 0.80));
        }
        ItemKind::IronOre | ItemKind::GoldOre => {
            let stone = ItemKind::Stone.color();
            let rad = r.w * 0.27;
            ball(cx, cy + 1.0, rad, stone);
            for (x0, y0, x1, y1) in [(-0.16, 0.02, 0.06, -0.12), (-0.06, 0.16, 0.16, 0.04), (0.04, -0.14, 0.18, -0.02)] {
                draw_line(cx + x0 * r.w, cy + y0 * r.h, cx + x1 * r.w, cy + y1 * r.h, 3.4 * u, darken(color, 0.78));
                draw_line(cx + x0 * r.w, cy + y0 * r.h - 0.8, cx + x1 * r.w, cy + y1 * r.h - 0.8, 1.6 * u, lighten(color, 0.15));
            }
            if kind == ItemKind::GoldOre {
                draw_circle(cx + 0.10 * r.w, cy - 0.16 * r.h, 1.2 * u, Color::new(1.0, 0.95, 0.55, 1.0));
            }
        }
        ItemKind::VenisonStew | ItemKind::PoultryStew | ItemKind::BearStew | ItemKind::RabbitStew | ItemKind::MooseStew
        | ItemKind::RedBerryStew | ItemKind::PurpleBerryStew => {
            // A wooden bowl brimming with stew: a rim, the surface with lumps, a few bubbles and steam.
            let bowl = ItemKind::WoodenBowl.color();
            draw_circle(cx, cy + 5.0 * u, r.w * 0.32, outline);
            draw_circle(cx, cy + 5.0 * u, r.w * 0.30, darken(bowl, 0.74));
            draw_circle(cx - 1.0, cy + 3.5 * u, r.w * 0.27, bowl);
            draw_circle(cx, cy + 1.0, r.w * 0.25, darken(color, 0.82));
            draw_circle(cx - 1.5, cy - 0.5, r.w * 0.22, color);
            for (dx, dy) in [(-0.10, 0.0), (0.08, -0.04), (0.02, 0.08), (-0.02, -0.08)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, r.w * 0.04, lighten(color, 0.18));
                draw_circle(cx + dx * r.w - 1.0, cy + dy * r.h - 1.0, r.w * 0.015, lighten(color, 0.40));
            }
            for sx in [-0.10, 0.06] {
                draw_line(cx + sx * r.w, cy - 0.22 * r.h, cx + (sx + 0.03) * r.w, cy - 0.34 * r.h, 1.4, Color::new(1.0, 1.0, 1.0, 0.55));
            }
        }
        ItemKind::Sling => {
            let (pouch, loop_a, loop_b) = (p(0.50, 0.30), p(0.22, 0.82), p(0.78, 0.82));
            let cord = ItemKind::LeatherCord.color();
            for end in [loop_a, loop_b] {
                rod(pouch, end, 2.2 * u, cord);
                draw_circle_lines(end.x, end.y, 4.0 * u, 2.0, outline);
                draw_circle_lines(end.x, end.y, 4.0 * u, 1.0, cord);
            }
            ball(pouch.x, pouch.y, r.w * 0.16, color);
            draw_line(pouch.x - 5.0 * u, pouch.y + 1.0, pouch.x + 5.0 * u, pouch.y + 1.0, 1.0, darken(color, 0.6));
            ball(pouch.x, pouch.y - 3.0 * u, 3.4 * u, ItemKind::Stone.color());
        }
        ItemKind::Loadstone => {
            let rad = r.w * 0.27;
            ball(cx, cy + 1.0, rad, color);
            // Metallic sheen and a spark.
            draw_circle(cx - rad * 0.30, cy - rad * 0.30, rad * 0.28, Color::new(0.46, 0.48, 0.60, 1.0));
            draw_circle(cx - rad * 0.38, cy - rad * 0.40, rad * 0.10, Color::new(0.88, 0.90, 0.98, 1.0));
            draw_line(cx + rad * 0.30, cy + rad * 0.10, cx + rad * 0.50, cy - rad * 0.05, 1.3, Color::new(0.60, 0.66, 0.86, 1.0));
        }
        ItemKind::Compass => {
            let rad = r.w * 0.30;
            ball(cx, cy, rad, color);
            draw_circle(cx, cy, rad * 0.78, Color::new(0.92, 0.90, 0.82, 1.0));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let len = if i % 2 == 0 { 0.16 } else { 0.10 };
                draw_line(cx + a.cos() * rad * 0.62, cy + a.sin() * rad * 0.62, cx + a.cos() * rad * (0.62 + len), cy + a.sin() * rad * (0.62 + len), 1.2, Color::new(0.30, 0.26, 0.20, 1.0));
            }
            let (n, s) = (vec2(cx + r.w * 0.09, cy - r.h * 0.17), vec2(cx - r.w * 0.09, cy + r.h * 0.17));
            let perp = vec2(0.14, 0.08) * 0.5 * r.w * 0.30;
            tri(n, vec2(cx, cy) + perp, vec2(cx, cy) - perp, Color::new(0.82, 0.14, 0.14, 1.0));
            tri(s, vec2(cx, cy) + perp, vec2(cx, cy) - perp, Color::new(0.96, 0.96, 0.96, 1.0));
            draw_circle(cx, cy, 2.0 * u, outline);
            draw_circle(cx - 0.4, cy - 0.4, 0.8 * u, lighten(color, 0.4));
            draw_circle(cx, cy - rad - 3.0 * u, 2.6 * u, darken(color, 0.8));
        }
        ItemKind::RabbitFoot => {
            let fur = color;
            ball(cx, cy + 4.0 * u, r.w * 0.19, fur);
            for dx in [-0.12, 0.0, 0.12] {
                ball(cx + dx * r.w, cy - 0.10 * r.h, r.w * 0.06, lighten(fur, 0.08));
            }
            // The clasp and a chain at the top.
            draw_circle_lines(cx, cy + 14.0 * u, 3.0 * u, 1.6, Color::new(0.80, 0.66, 0.30, 1.0));
            draw_circle_lines(cx, cy + 14.0 * u, 3.0 * u, 3.0, outline);
            draw_circle_lines(cx, cy + 14.0 * u, 3.0 * u, 1.6, Color::new(0.80, 0.66, 0.30, 1.0));
        }
        ItemKind::TinPot => {
            let (tl, br) = (p(0.22, 0.36), p(0.78, 0.80));
            slab(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            // The dark mouth, a bright rim, rivets and the handles.
            draw_rectangle(tl.x + 2.0, tl.y - 1.0, br.x - tl.x - 4.0, 4.0, darken(color, 0.35));
            draw_rectangle(tl.x, tl.y, br.x - tl.x, 2.0, lighten(color, 0.22));
            draw_line(tl.x + 5.0, tl.y + 8.0, tl.x + 5.0, br.y - 5.0, 2.0, lighten(color, 0.20));
            for rx in [tl.x + 6.0, br.x - 6.0] {
                draw_circle(rx, tl.y + 8.0, 1.3 * u, darken(color, 0.65));
            }
            for (a, b) in [(vec2(tl.x, p(0.0, 0.46).y), vec2(tl.x - 6.0 * u, p(0.0, 0.40).y)), (vec2(br.x, p(0.0, 0.46).y), vec2(br.x + 6.0 * u, p(0.0, 0.40).y))] {
                draw_line(a.x, a.y, b.x, b.y, 3.6, outline);
                draw_line(a.x, a.y, b.x, b.y, 1.8, lighten(color, 0.1));
            }
        }
        ItemKind::ClayJar => {
            // A terracotta jar: a round belly, a narrow neck with a flared lip, a painted band.
            ball(cx, cy + 5.0 * u, r.w * 0.27, color);
            let neck = Color::new(color.r * 0.96, color.g * 0.94, color.b * 0.94, 1.0);
            slab(cx - r.w * 0.11, cy - r.h * 0.26, r.w * 0.22, r.h * 0.20, neck);
            slab(cx - r.w * 0.16, cy - r.h * 0.31, r.w * 0.32, r.h * 0.08, lighten(color, 0.04));
            draw_line(cx - r.w * 0.22, cy + 2.0 * u, cx + r.w * 0.22, cy + 2.0 * u, 2.0, Color::new(0.50, 0.28, 0.20, 1.0));
            draw_line(cx - r.w * 0.21, cy + 6.0 * u, cx + r.w * 0.21, cy + 6.0 * u, 1.2, Color::new(0.50, 0.28, 0.20, 1.0));
        }
        ItemKind::WaterJar => {
            // The jar has turned clear: glass you can see through, with blue water inside it, a bright surface, and the
            // clay rim and cork still on.
            let glass = Color::new(0.82, 0.93, 1.0, 0.30);
            let water = Color::new(0.20, 0.50, 0.92, 0.80);
            draw_circle(cx, cy + 5.0 * u, r.w * 0.285, Color::new(0.35, 0.50, 0.65, 0.75));
            draw_circle(cx, cy + 5.0 * u, r.w * 0.27, glass);
            // Water fills the belly to the neck: a disc cut off flat at the top.
            let belly = r.w * 0.25;
            for i in 0..=20 {
                let t = i as f32 / 20.0;
                let dy = -belly * 0.55 + t * belly * 1.55;
                let half = (belly * belly - dy * dy).max(0.0).sqrt();
                draw_line(cx - half, cy + 5.0 * u + dy, cx + half, cy + 5.0 * u + dy, 1.6, water);
            }
            draw_line(cx - belly * 0.82, cy + 5.0 * u - belly * 0.55, cx + belly * 0.82, cy + 5.0 * u - belly * 0.55, 1.6, Color::new(0.70, 0.90, 1.0, 0.95));
            slab(cx - r.w * 0.10, cy - r.h * 0.27, r.w * 0.20, r.h * 0.20, Color::new(0.80, 0.92, 1.0, 0.45));
            slab(cx - r.w * 0.14, cy - r.h * 0.34, r.w * 0.28, r.h * 0.09, ItemKind::ClayJar.color());
            // Glints on the glass.
            draw_line(cx - belly * 0.70, cy + 5.0 * u - belly * 0.30, cx - belly * 0.70, cy + 5.0 * u + belly * 0.35, 2.4 * u, Color::new(1.0, 1.0, 1.0, 0.85));
            draw_circle(cx - belly * 0.40, cy + 5.0 * u - belly * 0.72, 1.4 * u, Color::new(1.0, 1.0, 1.0, 0.9));
            draw_circle(cx + belly * 0.35, cy + 5.0 * u + belly * 0.30, 1.8 * u, Color::new(0.75, 0.92, 1.0, 0.7));
        }
        ItemKind::LeatherCord => {
            for (a, b) in [(p(0.22, 0.30), p(0.78, 0.66)), (p(0.24, 0.70), p(0.76, 0.28))] {
                rod(a, b, 4.0 * u, color);
            }
            draw_circle(cx, cy, 2.0 * u, darken(color, 0.65));
        }
        ItemKind::BronzeMallet => {
            let (h0, h1) = (p(0.22, 0.86), p(0.62, 0.40));
            rod(h0, h1, 4.6 * u, ItemKind::Stick.color());
            let (a, b, c, d) = (p(0.42, 0.24), p(0.62, 0.08), p(0.90, 0.34), p(0.70, 0.54));
            quad([a, b, c, d], color);
            draw_triangle(a, b, (a + c) / 2.0, lighten(color, 0.14));
            draw_line(a.x, a.y, d.x, d.y, 2.0, darken(color, 0.70));
            draw_line(b.x, b.y, c.x, c.y, 1.4, lighten(color, 0.28));
            let bind = p(0.54, 0.48);
            draw_line(bind.x - 4.0 * u, bind.y - 3.0 * u, bind.x + 4.0 * u, bind.y + 3.0 * u, 2.6, ItemKind::LeatherCord.color());
        }
        ItemKind::ClayForge => {
            let (tl, br) = (p(0.14, 0.42), p(0.86, 0.84));
            slab(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            let (ol, or) = (p(0.28, 0.28), p(0.72, 0.46));
            slab(ol.x, ol.y, or.x - ol.x, or.y - ol.y, Color::new(0.38, 0.29, 0.24, 1.0));
            draw_rectangle(ol.x + 3.0, ol.y + 3.0, or.x - ol.x - 6.0, 3.0, Color::new(0.10, 0.08, 0.08, 1.0));
            // A tuyère hole in the front and cracks in the clay.
            draw_circle(cx, br.y - 8.0 * u, 3.4 * u, Color::new(0.14, 0.10, 0.09, 1.0));
            draw_line(tl.x + 6.0, tl.y + 6.0, tl.x + 10.0, tl.y + 12.0, 1.0, darken(color, 0.62));
        }
        ItemKind::Copper | ItemKind::Tin | ItemKind::Iron | ItemKind::Gold | ItemKind::Bronze => {
            // An ingot: a bright top face with a stamp, and a darker front face.
            let (a, b, c, d) = (p(0.28, 0.44), p(0.76, 0.44), p(0.88, 0.62), p(0.16, 0.62));
            let front = [p(0.16, 0.62), p(0.88, 0.62), p(0.84, 0.78), p(0.20, 0.78)];
            quad(front, darken(color, 0.72));
            quad([a, b, c, d], lighten(color, 0.08));
            draw_line(p(0.30, 0.48).x, p(0.30, 0.48).y, p(0.70, 0.48).x, p(0.70, 0.48).y, 2.4, lighten(color, 0.30));
            draw_rectangle(cx - 5.0 * u, cy - 0.5, 10.0 * u, 3.0 * u, darken(color, 0.85));
            draw_line(p(0.20, 0.64).x, p(0.20, 0.64).y, p(0.20, 0.76).x, p(0.20, 0.76).y, 1.4, lighten(color, 0.12));
        }
        ItemKind::Charcoal => {
            for (dx, dy, rad, shade) in [(-0.10, 0.08, 0.17, 0.0), (0.13, 0.04, 0.13, 0.03), (0.0, -0.12, 0.13, 0.06)] {
                ball(cx + dx * r.w, cy + dy * r.h, rad * r.w, Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0));
            }
            draw_line(cx - 0.14 * r.w, cy + 0.02 * r.h, cx - 0.04 * r.w, cy + 0.10 * r.h, 1.0, Color::new(0.45, 0.45, 0.50, 1.0));
            draw_circle(cx - 0.12 * r.w, cy - 0.02 * r.h, 1.6 * u, Color::new(0.50, 0.50, 0.58, 1.0));
        }
        ItemKind::Antlers => {
            for side in [-1.0_f32, 1.0] {
                let base = p(0.5, 0.86);
                let beam = [base, p(0.5 + side * 0.10, 0.60), p(0.5 + side * 0.20, 0.36), p(0.5 + side * 0.18, 0.14)];
                for w in beam.windows(2) {
                    rod(w[0], w[1], 5.0 * u, color);
                }
                for (from, to) in [(beam[1], p(0.5 + side * 0.34, 0.50)), (beam[2], p(0.5 + side * 0.38, 0.26)), (beam[2], p(0.5 + side * 0.04, 0.30))] {
                    rod(from, to, 3.4 * u, lighten(color, 0.04));
                }
            }
            draw_rectangle(cx - 6.0 * u, p(0.0, 0.84).y, 12.0 * u, 4.0 * u, darken(color, 0.7));
        }
        ItemKind::BoneTotem => {
            let (tl, br) = (p(0.40, 0.42), p(0.60, 0.90));
            slab(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            let (hl, hr) = (p(0.28, 0.10), p(0.72, 0.44));
            slab(hl.x, hl.y, hr.x - hl.x, hr.y - hl.y, lighten(color, 0.04));
            for ex in [0.40, 0.60] {
                ball(r.x + r.w * ex, r.y + r.h * 0.26, 3.6 * u, Color::new(0.10, 0.09, 0.08, 1.0));
            }
            draw_triangle(p(0.47, 0.34), p(0.53, 0.34), p(0.50, 0.28), Color::new(0.20, 0.17, 0.15, 1.0));
            for tx in [0.38, 0.46, 0.54, 0.62] {
                draw_line(r.x + r.w * tx, hr.y - 1.0, r.x + r.w * tx, hr.y - 5.0 * u, 2.0, Color::new(0.96, 0.94, 0.88, 1.0));
            }
            rod(p(0.28, 0.60), p(0.72, 0.60), 4.0 * u, color);
            rod(p(0.30, 0.74), p(0.70, 0.74), 3.0 * u, color);
        }
        ItemKind::PeltCloak => {
            let (a, b, c, d) = (p(0.28, 0.16), p(0.72, 0.16), p(0.88, 0.86), p(0.12, 0.86));
            quad([a, b, c, d], color);
            draw_triangle(a, (a + b) / 2.0 + vec2(0.0, 8.0 * u), p(0.20, 0.86), darken(color, 0.86));
            for i in 0..7 {
                let fx = d.x + 4.0 + i as f32 * (c.x - d.x - 8.0) / 6.0;
                draw_line(fx, d.y - 1.0, fx, d.y + 3.0 + (i % 2) as f32 * 3.0, 3.6, color);
            }
            draw_line(p(0.22, 0.74).x, p(0.22, 0.74).y, p(0.78, 0.74).x, p(0.78, 0.74).y, 4.0, lighten(color, 0.14));
            for fx in [0.36, 0.50, 0.64] {
                draw_line(r.x + r.w * fx, r.y + r.h * 0.28, r.x + r.w * (fx + 0.03), r.y + r.h * 0.64, 1.2, lighten(color, 0.12));
            }
            ball(r.x + r.w * 0.50, r.y + r.h * 0.18, 3.8 * u, ItemKind::Gold.color());
        }
        ItemKind::RoyalArmor => {
            let (tl, br) = (p(0.28, 0.24), p(0.72, 0.86));
            slab(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            for sx in [0.12, 0.64] {
                let (sl, sr) = (p(sx, 0.14), p(sx + 0.24, 0.40));
                slab(sl.x, sl.y, sr.x - sl.x, sr.y - sl.y, lighten(color, 0.08));
            }
            draw_rectangle(tl.x, p(0.0, 0.58).y, br.x - tl.x, 5.0 * u, ItemKind::Gold.color());
            for sx in [0.34, 0.50, 0.66] {
                draw_circle(r.x + r.w * sx, p(0.0, 0.58).y + 2.5 * u, 1.2 * u, lighten(ItemKind::Gold.color(), 0.25));
            }
            draw_line(cx, tl.y + 3.0, cx, p(0.0, 0.56).y, 2.0, lighten(color, 0.24));
            draw_circle(cx, tl.y + 9.0 * u, 3.0 * u, ItemKind::Gold.color());
        }
        ItemKind::SilverKey => {
            // A ring for the bow, a long shaft and two teeth at the tip.
            let bow = p(0.30, 0.30);
            draw_circle(bow.x, bow.y, 8.5 * u, outline);
            draw_circle(bow.x, bow.y, 7.0 * u, color);
            draw_circle(bow.x, bow.y, 3.4 * u, Color::new(0.10, 0.10, 0.13, 1.0));
            let (from, to) = (p(0.38, 0.38), p(0.80, 0.80));
            rod(from, to, 4.6 * u, color);
            for t in [0.66, 0.86] {
                let at = from + (to - from) * t;
                rod(at, at + vec2(7.0 * u, -7.0 * u), 3.4 * u, darken(color, 0.9));
            }
            draw_line(bow.x - 3.0 * u, bow.y - 5.0 * u, bow.x + 3.0 * u, bow.y - 5.5 * u, 1.2, lighten(color, 0.30));
        }
        ItemKind::PinkSalt => {
            // Three crystals standing together, each a long tip over a faceted body, lit from the upper left.
            for (fx, base, top, half) in [(0.30, 0.82, 0.38, 0.11), (0.52, 0.86, 0.14, 0.13), (0.72, 0.80, 0.44, 0.10)] {
                let (l, rt, tip) = (p(fx - half, base), p(fx + half, base), p(fx, top));
                tri(l, rt, tip, darken(color, 0.86));
                tri(l, p(fx, base - 0.02), tip, lighten(color, 0.10));
                draw_line(tip.x, tip.y, p(fx, base).x, p(fx, base).y, 1.0, lighten(color, 0.34));
            }
            draw_line(p(0.24, 0.84).x, p(0.24, 0.84).y, p(0.78, 0.84).x, p(0.78, 0.84).y, 2.0, darken(color, 0.62));
        }
        ItemKind::PhilosophersStone => {
            let rad = r.w * 0.28;
            ball(cx, cy + 1.0, rad, color);
            // An inner fire, and a gold ring round it like a halo.
            draw_circle(cx + rad * 0.10, cy + rad * 0.15, rad * 0.55, lighten(color, 0.14));
            draw_circle(cx - rad * 0.05, cy, rad * 0.26, Color::new(1.0, 0.62, 0.40, 0.85));
            draw_circle_lines(cx, cy + 1.0, rad * 1.22, 1.6, ItemKind::Gold.color());
            let g = p(0.38, 0.34);
            draw_line(g.x - 3.0 * u, g.y, g.x + 3.0 * u, g.y, 1.2, WHITE);
            draw_line(g.x, g.y - 3.0 * u, g.x, g.y + 3.0 * u, 1.2, WHITE);
        }
        ItemKind::BootsOfTheSky => {
            // A pair of boots with a cuff, a heel and a curled toe, a wing of feathers at the ankle and a silver sole.
            for (dx, tone) in [(0.0, 1.0_f32), (0.30, 0.86)] {
                let c = darken(color, tone);
                let (sl, sr) = (p(0.10 + dx * 0.3, 0.16), p(0.34 + dx * 0.3, 0.64));
                slab(sl.x, sl.y, sr.x - sl.x, sr.y - sl.y, c);
                let (fl, fr) = (p(0.10 + dx * 0.3, 0.60), p(0.62 + dx * 0.3, 0.78));
                slab(fl.x, fl.y, fr.x - fl.x, fr.y - fl.y, c);
                tri(p(0.62 + dx * 0.3, 0.60), p(0.62 + dx * 0.3, 0.78), p(0.72 + dx * 0.3, 0.56), lighten(c, 0.10));
                draw_rectangle(fl.x, p(0.0, 0.76).y, fr.x - fl.x + 3.0 * u, 3.0 * u, Color::new(0.86, 0.88, 0.94, 1.0));
                let (cl, cr) = (p(0.08 + dx * 0.3, 0.14), p(0.36 + dx * 0.3, 0.22));
                slab(cl.x, cl.y, cr.x - cl.x, cr.y - cl.y, lighten(c, 0.12));
                for k in 0..3 {
                    tri(p(0.10 + dx * 0.3, 0.32 + 0.07 * k as f32), p(0.10 + dx * 0.3, 0.38 + 0.07 * k as f32), p(0.00 + dx * 0.3, 0.30 + 0.07 * k as f32), Color::new(0.96, 0.97, 1.0, 1.0));
                }
            }
        }
        ItemKind::CursedIdol => {
            // A squat stone figure with a heavy brow, glaring red eyes and a wide fanged mouth.
            let (bl, br) = (p(0.28, 0.46), p(0.72, 0.90));
            slab(bl.x, bl.y, br.x - bl.x, br.y - bl.y, darken(color, 0.8));
            let (hl, hr) = (p(0.24, 0.12), p(0.76, 0.50));
            slab(hl.x, hl.y, hr.x - hl.x, hr.y - hl.y, color);
            slab(hl.x, p(0.0, 0.22).y, hr.x - hl.x, 4.0 * u, darken(color, 0.6));
            for ex in [0.38, 0.62] {
                ball(r.x + r.w * ex, r.y + r.h * 0.32, 4.2 * u, Color::new(0.95, 0.08, 0.06, 1.0));
            }
            draw_rectangle(p(0.34, 0.0).x, p(0.0, 0.42).y, 0.32 * r.w, 4.0 * u, Color::new(0.08, 0.08, 0.06, 1.0));
            for tx in [0.40, 0.50, 0.60] {
                tri(p(tx - 0.03, 0.42), p(tx + 0.03, 0.42), p(tx, 0.49), Color::new(0.94, 0.92, 0.84, 1.0));
            }
            draw_line(bl.x, p(0.0, 0.62).y, br.x, p(0.0, 0.62).y, 2.0, lighten(color, 0.1));
        }
        ItemKind::AncientCrown => {
            let (bl, br) = (p(0.18, 0.54), p(0.82, 0.80));
            slab(bl.x, bl.y, br.x - bl.x, br.y - bl.y, color);
            // Five points, the middle one the tallest, each with a pearl.
            for (i, fx) in [0.22, 0.36, 0.50, 0.64, 0.78].into_iter().enumerate() {
                let height = if i == 2 { 0.16 } else if i % 2 == 0 { 0.26 } else { 0.30 };
                tri(p(fx - 0.09, 0.56), p(fx + 0.09, 0.56), p(fx, height), lighten(color, 0.06));
                ball(p(fx, height).x, p(fx, height).y, 2.6 * u, Color::new(0.96, 0.94, 0.90, 1.0));
            }
            draw_rectangle(bl.x, p(0.0, 0.62).y, br.x - bl.x, 3.0 * u, darken(color, 0.76));
            for (fx, jewel) in [(0.30, Color::new(0.16, 0.72, 0.40, 1.0)), (0.50, Color::new(0.78, 0.12, 0.18, 1.0)), (0.70, Color::new(0.20, 0.42, 0.86, 1.0))] {
                ball(p(fx, 0.71).x, p(fx, 0.71).y, 3.4 * u, jewel);
            }
        }
        ItemKind::HornOfTheHunt => {
            // A curved horn: thick at the mouth (upper right), tapering round to the tip (lower left), with gold bands.
            let pts = [p(0.80, 0.22), p(0.66, 0.46), p(0.48, 0.64), p(0.30, 0.74), p(0.18, 0.70)];
            for (i, w) in pts.windows(2).enumerate() {
                rod(w[0], w[1], (11.0 - 2.4 * i as f32) * u, color);
            }
            ball(pts[0].x, pts[0].y, 8.0 * u, darken(color, 0.62));
            ball(pts[0].x - 1.0 * u, pts[0].y + 1.0 * u, 5.4 * u, Color::new(0.20, 0.12, 0.08, 1.0));
            for t in [0.22, 0.58] {
                let at = pts[1] + (pts[2] - pts[1]) * t;
                draw_circle(at.x, at.y, 5.2 * u, ItemKind::Gold.color());
                draw_circle(at.x, at.y, 3.6 * u, color);
            }
            draw_line(pts[1].x - 4.0 * u, pts[1].y - 4.0 * u, pts[3].x, pts[3].y - 5.0 * u, 1.2, lighten(color, 0.22));
        }
        ItemKind::Bone => {
            // A long bone with a knuckle at each end.
            let (a, b) = (p(0.26, 0.74), p(0.74, 0.26));
            rod(a, b, 6.0 * u, color);
            for end in [a, b] {
                let side = vec2(-(b.y - a.y), b.x - a.x).normalize() * 4.0 * u;
                ball(end.x + side.x, end.y + side.y, 4.6 * u, color);
                ball(end.x - side.x, end.y - side.y, 4.6 * u, color);
            }
            draw_line(p(0.40, 0.60).x, p(0.40, 0.60).y, p(0.58, 0.42).x, p(0.58, 0.42).y, 1.2, lighten(color, 0.28));
            draw_line(p(0.44, 0.64).x, p(0.44, 0.64).y, p(0.62, 0.46).x, p(0.62, 0.46).y, 1.0, darken(color, 0.70));
        }
        ItemKind::IronFeather | ItemKind::PhoenixFeather => {
            // A feather like the quail's, but of metal plates (iron) or of fire (phoenix), with a bright quill.
            let (tip, base) = (p(0.82, 0.14), p(0.20, 0.86));
            let n = (tip - base).normalize();
            let perp = vec2(-n.y, n.x);
            let iron = kind == ItemKind::IronFeather;
            for i in 0..12 {
                let t = 0.12 + i as f32 / 12.0 * 0.84;
                let on = base + (tip - base) * t;
                let width = (t * (1.0 - t)).sqrt() * 0.55 * r.w * 0.30 * 2.0;
                let shade = if i % 2 == 0 { 0.0 } else { -0.07 };
                let c = if iron {
                    Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0)
                } else {
                    Color::new((color.r + shade).clamp(0.0, 1.0), (color.g + shade + 0.30 * t).clamp(0.0, 1.0), (color.b + shade).clamp(0.0, 1.0), 1.0)
                };
                draw_line(on.x - perp.x * width * 0.6, on.y - perp.y * width * 0.6 - 1.5 * u, on.x + perp.x * width, on.y + perp.y * width, 3.2 * u, c);
            }
            draw_line(base.x, base.y, tip.x, tip.y, 2.6 * u, outline);
            draw_line(base.x, base.y, tip.x, tip.y, 1.3 * u, if iron { Color::new(0.86, 0.88, 0.92, 1.0) } else { Color::new(1.0, 0.92, 0.50, 1.0) });
        }
        ItemKind::PhoenixArrow => {
            // An arrow with a fiery head and phoenix fletching.
            let (tail, tip) = (p(0.20, 0.80), p(0.76, 0.24));
            rod(tail, tip, 3.0 * u, ItemKind::Arrow.color());
            let dir = (tip - tail).normalize();
            let perp = vec2(-dir.y, dir.x);
            let head = Color::new(1.0, 0.62, 0.12, 1.0);
            tri(tip + dir * 10.0 * u, tip + perp * 5.5 * u - dir * 3.0 * u, tip - perp * 5.5 * u - dir * 3.0 * u, head);
            tri(tip + dir * 6.0 * u, tip + perp * 2.5 * u - dir * 2.0 * u, tip - perp * 2.5 * u - dir * 2.0 * u, Color::new(1.0, 0.92, 0.45, 1.0));
            for k in 0..3 {
                let at = tail + dir * (k as f32 * 4.5 * u);
                for s in [-1.0_f32, 1.0] {
                    tri(at, at + perp * s * 6.5 * u - dir * 2.0 * u, at + dir * 6.5 * u + perp * s * 1.0, ItemKind::PhoenixFeather.color());
                }
            }
        }
        ItemKind::WoodenBowl => {
            // An empty wooden bowl seen from the side: a dark inside, a rim and a rounded body.
            draw_circle(cx, cy + 3.0 * u, r.w * 0.32, outline);
            draw_circle(cx, cy + 3.0 * u, r.w * 0.30, darken(color, 0.74));
            draw_circle(cx - 1.0, cy + 1.5 * u, r.w * 0.27, color);
            draw_circle(cx, cy - 1.0 * u, r.w * 0.24, darken(color, 0.55));
            draw_circle(cx - 1.5, cy - 2.0 * u, r.w * 0.20, darken(color, 0.40));
        }
        ItemKind::Stardust => {
            // A small heap of glittering white grains.
            for (dx, dy, k) in [(-0.14, 0.10, 4.5), (0.12, 0.12, 4.0), (0.0, -0.04, 5.5), (-0.04, 0.20, 3.5), (0.16, -0.10, 2.5)] {
                ball(cx + dx * r.w, cy + dy * r.h, k * u, color);
            }
            draw_circle(cx - 0.02 * r.w, cy - 0.08 * r.h, 1.4 * u, WHITE);
        }
        ItemKind::SnakeFang => {
            // A curved, pointed fang: wide at the root (upper left), curving down to a sharp tip.
            let pts = [p(0.30, 0.22), p(0.42, 0.40), p(0.54, 0.58), p(0.62, 0.78)];
            for (i, w) in pts.windows(2).enumerate() {
                rod(w[0], w[1], (11.0 - 3.6 * i as f32) * u, color);
            }
            tri(pts[3] - vec2(3.0 * u, 0.0), pts[3] + vec2(3.0 * u, 0.0), p(0.64, 0.88), color);
            ball(pts[0].x, pts[0].y, 6.0 * u, darken(color, 0.78));
            draw_line(pts[0].x + 2.0 * u, pts[0].y + 2.0 * u, pts[2].x + 1.0 * u, pts[2].y, 1.2, WHITE);
            ball(p(0.70, 0.60).x, p(0.70, 0.60).y, 1.8 * u, Color::new(0.50, 0.85, 0.22, 1.0));
        }
        ItemKind::PoisonArrow => {
            // An arrow with a green, fang-shaped head and a drip of poison.
            let (tail, tip) = (p(0.20, 0.80), p(0.76, 0.24));
            rod(tail, tip, 3.0 * u, ItemKind::Arrow.color());
            let dir = (tip - tail).normalize();
            let perp = vec2(-dir.y, dir.x);
            let head = Color::new(0.42, 0.80, 0.20, 1.0);
            tri(tip + dir * 10.0 * u, tip + perp * 5.0 * u - dir * 3.0 * u, tip - perp * 5.0 * u - dir * 3.0 * u, head);
            tri(tip + dir * 6.0 * u, tip + perp * 2.2 * u - dir * 2.0 * u, tip - perp * 2.2 * u - dir * 2.0 * u, ItemKind::SnakeFang.color());
            ball(tip.x + 4.0 * u, tip.y + 9.0 * u, 2.0 * u, head);
            for k in 0..3 {
                let at = tail + dir * (k as f32 * 4.5 * u);
                for s in [-1.0_f32, 1.0] {
                    tri(at, at + perp * s * 6.0 * u - dir * 2.0 * u, at + dir * 6.0 * u + perp * s * 1.0, ItemKind::Feather.color());
                }
            }
        }
        ItemKind::VenisonJerky | ItemKind::PoultryJerky | ItemKind::BearJerky | ItemKind::RabbitJerky | ItemKind::MooseJerky => {
            // A dry, dark strip of meat, twisted a little, flecked with white salt.
            let (a, b) = (p(0.20, 0.70), p(0.80, 0.34));
            rod(a, b, 13.0 * u, darken(color, 0.55));
            rod(a, b, 10.0 * u, color);
            let mid = (a + b) / 2.0;
            draw_line(a.x + 4.0 * u, a.y - 2.0 * u, mid.x, mid.y - 3.0 * u, 1.6, lighten(color, 0.22));
            draw_line(mid.x + 2.0 * u, mid.y + 3.0 * u, b.x - 4.0 * u, b.y + 3.0 * u, 1.4, darken(color, 0.6));
            for (fx, fy) in [(0.34, 0.56), (0.46, 0.52), (0.58, 0.44), (0.68, 0.42), (0.40, 0.62)] {
                draw_circle(p(fx, fy).x, p(fx, fy).y, 1.1 * u, Color::new(0.97, 0.94, 0.92, 1.0));
            }
        }
        ItemKind::LeatherBackpack => {
            // A pack seen from the front: a rounded body, a flap with a buckle, a front pocket and two straps.
            let (tl, br) = (p(0.24, 0.20), p(0.76, 0.84));
            slab(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            draw_rectangle(tl.x, tl.y, br.x - tl.x, 12.0 * u, darken(color, 0.78));
            draw_rectangle(tl.x + 6.0 * u, br.y - 18.0 * u, br.x - tl.x - 12.0 * u, 12.0 * u, lighten(color, 0.12));
            draw_rectangle(cx - 3.0 * u, tl.y + 8.0 * u, 6.0 * u, 7.0 * u, Color::new(0.85, 0.75, 0.40, 1.0));
            for sx in [tl.x + 7.0 * u, br.x - 7.0 * u] {
                draw_line(sx, tl.y + 12.0 * u, sx, br.y - 2.0 * u, 2.4 * u, ItemKind::LeatherCord.color());
            }
            draw_rectangle_lines(tl.x, tl.y, br.x - tl.x, br.y - tl.y, 1.4, OUTLINE);
        }
        ItemKind::TinPan => {
            // A wide shallow dish seen from the side-on, with a short handle and a dark inner rim.
            let (a, b) = (p(0.14, 0.52), p(0.86, 0.52));
            quad([a, b, p(0.76, 0.72), p(0.24, 0.72)], color);
            draw_line(a.x, a.y, b.x, b.y, 3.0 * u, lighten(color, 0.25));
            draw_rectangle(a.x + 4.0 * u, a.y + 1.5 * u, b.x - a.x - 8.0 * u, 3.0 * u, darken(color, 0.45));
            rod(p(0.80, 0.56), p(0.94, 0.40), 3.4 * u, darken(color, 0.8));
            draw_line(p(0.30, 0.64).x, p(0.30, 0.64).y, p(0.62, 0.64).x, p(0.62, 0.64).y, 1.2, lighten(color, 0.3));
        }
        ItemKind::Sword => {
            // A plain steel blade, a dark crosspiece and a leather grip.
            let (tip, base) = (p(0.82, 0.14), p(0.36, 0.60));
            rod(base, tip, 5.0 * u, color);
            rod(base, tip, 1.4 * u, darken(color, 0.62));
            rod(p(0.24, 0.58), p(0.42, 0.76), 3.6 * u, Color::new(0.34, 0.30, 0.28, 1.0));
            rod(p(0.32, 0.68), p(0.16, 0.84), 4.2 * u, Color::new(0.30, 0.18, 0.10, 1.0));
        }
        ItemKind::EyeOfNewt => {
            // A slimy yellow eye with a slit pupil.
            ball(cx, cy, 9.0 * u, color);
            draw_circle(cx - 3.0 * u, cy - 3.0 * u, 2.0 * u, Color::new(1.0, 1.0, 0.7, 1.0));
            draw_rectangle(cx - 1.2 * u, cy - 5.0 * u, 2.4 * u, 10.0 * u, Color::new(0.06, 0.06, 0.04, 1.0));
        }
        ItemKind::Excalibur => {
            // A long silver blade with a fuller, a golden crosspiece, a grip and a pommel.
            let (tip, base) = (p(0.84, 0.10), p(0.34, 0.60));
            rod(base, tip, 6.0 * u, color);
            rod(base, tip, 1.6 * u, darken(color, 0.62));
            let (g0, g1) = (p(0.20, 0.58), p(0.40, 0.78));
            rod(g0, g1, 4.0 * u, Color::new(0.90, 0.68, 0.18, 1.0));
            rod(p(0.30, 0.68), p(0.14, 0.86), 4.6 * u, Color::new(0.30, 0.18, 0.10, 1.0));
            draw_circle(p(0.12, 0.88).x, p(0.12, 0.88).y, 3.4 * u, Color::new(0.90, 0.68, 0.18, 1.0));
        }
        ItemKind::MusicalPipes => {
            // Five pipes of rising length bound side by side.
            for (i, top) in [0.62, 0.50, 0.38, 0.26, 0.16].into_iter().enumerate() {
                let fx = 0.20 + i as f32 * 0.15;
                let (a, b) = (p(fx, 0.84), p(fx, top));
                slab(a.x - 3.6 * u, b.y, 7.2 * u, a.y - b.y, if i % 2 == 0 { color } else { darken(color, 0.92) });
                draw_rectangle(a.x - 2.0 * u, b.y + 1.0, 4.0 * u, 3.0 * u, Color::new(0.16, 0.12, 0.08, 1.0));
            }
            draw_line(p(0.14, 0.62).x, p(0.14, 0.62).y, p(0.86, 0.62).x, p(0.86, 0.62).y, 3.6 * u, ItemKind::LeatherCord.color());
            draw_line(p(0.14, 0.72).x, p(0.14, 0.72).y, p(0.86, 0.72).x, p(0.86, 0.72).y, 3.0 * u, ItemKind::LeatherCord.color());
        }
    }
}

/// An item icon with its stack count in the corner (no frame).
/// What the compass needle points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompassTarget {
    /// North, until the world's mountain is out.
    North,
    /// The world's mountain.
    Mountain,
    /// Where the player arrived, in the Horizen and the Skylands.
    Spawn,
}

impl CompassTarget {
    /// The letter drawn at the needle's tip.
    fn letter(self) -> &'static str {
        match self {
            CompassTarget::North => "N",
            CompassTarget::Mountain => "M",
            CompassTarget::Spawn => "S",
        }
    }
}

/// The compass in hand: a dial in the top right whose red needle points north (the way to a mountain) until the world's mountain is out, and at the mountain after and
/// whose tail points south, turning as the player turns, with the blocks walked north so far below it.
pub fn draw_compass(yaw: f32, bearing: f32, progress: f32, target: CompassTarget) {
    let (w, radius) = (screen_width(), 46.0);
    let (cx, cy) = (w - radius - 24.0, radius + 56.0);
    draw_circle(cx, cy, radius + 4.0, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_circle(cx, cy, radius, Color::new(0.90, 0.88, 0.80, 0.95));
    let (nx, ny) = compass_needle(yaw, bearing);
    draw_line(cx, cy, cx - nx * radius * 0.6, cy - ny * radius * 0.6, 5.0, Color::new(0.96, 0.96, 0.96, 1.0));
    draw_line(cx, cy, cx + nx * radius * 0.85, cy + ny * radius * 0.85, 5.0, Color::new(0.80, 0.14, 0.14, 1.0));
    draw_circle(cx, cy, 4.0, Color::new(0.05, 0.05, 0.05, 1.0));
    draw_text(target.letter(), cx + nx * (radius + 12.0) - 6.0, cy + ny * (radius + 12.0) + 6.0, 22.0, WHITE);
    let line = match target {
        CompassTarget::Mountain => "Points to the mountain".to_string(),
        CompassTarget::Spawn => "Points to where you arrived".to_string(),
        CompassTarget::North => format!("North {:.0}/{} blocks", progress.min((COMPASS_TREK_CHUNKS * CHUNK_SIZE) as f32), COMPASS_TREK_CHUNKS * CHUNK_SIZE),
    };
    draw_text(&line, cx - radius - 12.0, cy + radius + 26.0, 20.0, WHITE);
}

fn draw_stack(r: Rect, s: &ItemStack) {
    draw_item_icon(s.kind, r);
    if s.count > 1 {
        let text = s.count.to_string();
        let d = measure_text(&text, None, 20, 1.0);
        draw_text(&text, r.x + r.w - d.width - 4.0, r.y + r.h - 5.0, 20.0, WHITE);
    }
}

/// One inventory slot: frame, icon and stack count.
pub fn draw_slot(r: Rect, stack: Option<&ItemStack>, hovered: bool) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.07, 0.08, 0.07, 0.85));
    let border = if hovered { ACCENT } else { Color::new(0.35, 0.38, 0.35, 1.0) };
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, border);
    if let Some(s) = stack {
        draw_stack(r, s);
    }
}

/// Outline marking the hotbar slot the player is holding.
fn draw_selection(r: Rect) {
    draw_rectangle_lines(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 4.0, WHITE);
}

/// The always-visible row of the first `HOTBAR_SLOTS` slots at the bottom of the screen.
/// The `selected` slot is outlined and the name of the item held is shown above it.
pub fn draw_hotbar(inventory: &Inventory, selected: usize) {
    let (w, h) = (screen_width(), screen_height());
    let total = HOTBAR_SLOTS as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP;
    let (x0, y) = ((w - total) / 2.0, h - 78.0);
    for i in 0..HOTBAR_SLOTS {
        let r = Rect::new(x0 + i as f32 * (SLOT_SIZE + SLOT_GAP), y, SLOT_SIZE, SLOT_SIZE);
        draw_slot(r, inventory.slots[i].as_ref(), false);
        if i == selected {
            draw_selection(r);
        }
    }
    if let Some(stack) = inventory.slots[selected] {
        let name = stack.kind.name();
        let d = measure_text(name, None, 24, 1.0);
        let tx = (w - d.width) / 2.0;
        draw_text(name, tx + 1.5, y - 11.5, 24.0, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_text(name, tx, y - 13.0, 24.0, WHITE);
    }
}

/// Inventory-screen state that lives across frames: which slot is being dragged.
pub struct InventoryUi {
    drag_from: Option<usize>,
    /// How many recipes the crafting list is scrolled down by.
    craft_scroll: usize,
    /// Whether the right-hand list is the recipe book: every recipe, with what is still needed for it.
    pub guide: bool,
}

impl InventoryUi {
    pub fn new() -> Self {
        InventoryUi { drag_from: None, craft_scroll: 0, guide: false }
    }

    /// Abandon any drag in progress (the stack simply stays where it was).
    pub fn cancel_drag(&mut self) {
        self.drag_from = None;
    }
}

/// Where everything sits on the inventory screen.
struct Layout {
    panel: Rect,
    /// Top-left of the slot grid.
    grid: Vec2,
    /// Top-left of the crafting list.
    craft: Vec2,
}

/// Rows of the main grid for an inventory of `slots` slots (the hotbar is the row below it).
fn main_rows(slots: usize) -> usize {
    (slots - HOTBAR_SLOTS) / COLUMNS
}

fn grid_size(slots: usize) -> Vec2 {
    let w = COLUMNS as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP;
    let h = (main_rows(slots) + 1) as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP + HOTBAR_SEPARATION;
    vec2(w, h)
}

/// Screen rectangle of the Worn slot, beside the inventory title.
fn worn_slot_rect(lay: &Layout) -> Rect {
    Rect::new(lay.grid.x + 190.0, lay.panel.y + 24.0 + 4.0, SLOT_SIZE - 10.0, SLOT_SIZE - 10.0)
}

fn layout(slots: usize) -> Layout {
    let (w, h) = (screen_width(), screen_height());
    let grid = grid_size(slots);
    let (pad, title_h, footer_h) = (24.0, 46.0, 96.0);
    let size = vec2(grid.x + CRAFT_GAP + CRAFT_W + 2.0 * pad, grid.y + 2.0 * pad + title_h + footer_h);
    let panel = Rect::new((w - size.x) / 2.0, (h - size.y) / 2.0, size.x, size.y);
    let origin = vec2(panel.x + pad, panel.y + pad + title_h);
    Layout { panel, grid: origin, craft: vec2(origin.x + grid.x + CRAFT_GAP, origin.y) }
}

/// Screen rectangle of inventory slot `slot`: the main grid on top, the hotbar row below.
fn slot_rect(grid: Vec2, slots: usize, slot: usize) -> Rect {
    let (col, row, extra) = if slot >= HOTBAR_SLOTS {
        ((slot - HOTBAR_SLOTS) % COLUMNS, (slot - HOTBAR_SLOTS) / COLUMNS, 0.0)
    } else {
        (slot, main_rows(slots), HOTBAR_SEPARATION)
    };
    Rect::new(
        grid.x + col as f32 * (SLOT_SIZE + SLOT_GAP),
        grid.y + row as f32 * (SLOT_SIZE + SLOT_GAP) + extra,
        SLOT_SIZE,
        SLOT_SIZE,
    )
}

fn slot_at(grid: Vec2, slots: usize, mouse: Vec2) -> Option<usize> {
    (0..slots).find(|&slot| slot_rect(grid, slots, slot).contains(mouse))
}

/// Join `parts` with spacing, starting a new line whenever one would exceed `max_width`.
/// `text` cut short with dots so that it is no wider than `max` pixels at `size`.
fn fit_text(text: &str, max: f32, size: u16) -> String {
    if measure_text(text, None, size, 1.0).width <= max {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while !cut.is_empty() && measure_text(&format!("{cut}..."), None, size, 1.0).width > max {
        cut.pop();
    }
    format!("{cut}...")
}

fn wrap_totals(parts: &[String], max_width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for part in parts {
        match lines.last_mut() {
            Some(line) if measure_text(&format!("{line}    {part}"), None, 20, 1.0).width <= max_width => {
                line.push_str("    ");
                line.push_str(part);
            }
            _ => lines.push(part.clone()),
        }
    }
    lines
}

/// The full inventory overlay. Handles input and draws in one pass:
/// - press on a stack and release over another slot to move, merge or swap stacks;
/// - click an entry in the crafting list to craft it once.
/// Also shows a tooltip on hover.
pub fn inventory_screen(inventory: &mut Inventory, worn: &mut Option<ItemKind>, ui: &mut InventoryUi, selected: usize, toasts: &mut Toasts, dev_mode: bool) -> Option<usize> {
    let (w, h) = (screen_width(), screen_height());
    let slots = inventory.slots.len();
    let lay = layout(slots);
    let (mx, my) = mouse_position();
    let mouse = vec2(mx, my);
    let hovered_slot = slot_at(lay.grid, slots, mouse);
    // The worn slot, beside the title: click it to take the garment off; drop a cloak, armour, crown or backpack on it to put it
    // on. They all share it. A backpack's rows must be emptied into the other slots before it comes off.
    let worn_rect = worn_slot_rect(&lay);
    if is_mouse_button_pressed(MouseButton::Left) && worn_rect.contains(mouse) {
        match inventory.take_off(worn) {
            Ok(Some(kind)) => toasts.push(format!("You take off the {}", kind.name())),
            Ok(None) => {}
            Err(why) => toasts.push(why.to_string()),
        }
    }

    // --- Drag and drop input ---
    if is_mouse_button_pressed(MouseButton::Left) {
        if let Some(slot) = hovered_slot {
            if inventory.slots.get(slot).map_or(false, |s| s.is_some()) {
                ui.drag_from = Some(slot);
            }
        }
    }
    let mut dropped_outside = None;
    if is_mouse_button_released(MouseButton::Left) {
        if let Some(from) = ui.drag_from.take() {
            if worn_rect.contains(mouse) {
                if inventory.slots.get(from).copied().flatten().map_or(false, |s| s.kind.is_wearable()) {
                    match inventory.put_on(worn, from) {
                        Ok(kind) => toasts.push(format!("You put on the {}", kind.name())),
                        Err(why) => toasts.push(why.to_string()),
                    }
                }
            } else if let Some(to) = hovered_slot {
                inventory.move_stack(from, to);
            } else if !lay.panel.contains(mouse) {
                dropped_outside = Some(from);
            }
        }
    }
    let dragged = ui.drag_from.and_then(|s| inventory.slots.get(s).copied().flatten().map(|stack| (s, stack)));

    // Putting on or taking off a backpack adds or removes its rows, so the inventory may now have a different number of slots than
    // when this frame began. Everything from here on (drawing, the crafting list, the footer) uses the new size and layout.
    let slots = inventory.slots.len();
    let lay = layout(slots);
    let worn_rect = worn_slot_rect(&lay);

    // --- Panel ---
    let (pad, panel) = (24.0, lay.panel);
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, Color::new(0.14, 0.17, 0.14, 0.97));
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 3.0, ACCENT);
    draw_text("Inventory", lay.grid.x, panel.y + pad + 22.0, 34.0, WHITE);
    draw_text(if ui.guide { "All Recipes" } else { "Crafting" }, lay.craft.x, panel.y + pad + 22.0, 34.0, WHITE);
    // The button that switches the list between what can be crafted now and the whole recipe book.
    let book_rect = Rect::new(panel.x + panel.w - pad - 150.0, panel.y + pad - 2.0, 150.0, 30.0);
    let book_hover = book_rect.contains(mouse);
    draw_rectangle(book_rect.x, book_rect.y, book_rect.w, book_rect.h, if book_hover { Color::new(0.30, 0.36, 0.30, 1.0) } else { Color::new(0.20, 0.25, 0.20, 1.0) });
    draw_rectangle_lines(book_rect.x, book_rect.y, book_rect.w, book_rect.h, 2.0, ACCENT);
    let book_label = if ui.guide { "Back to Crafting" } else { "Recipe Book" };
    let book_size = measure_text(book_label, None, 20, 1.0);
    draw_text(book_label, book_rect.x + (book_rect.w - book_size.width) / 2.0, book_rect.y + 21.0, 20.0, WHITE);
    if book_hover && is_mouse_button_pressed(MouseButton::Left) {
        ui.guide = !ui.guide;
        ui.craft_scroll = 0;
    }
    draw_text("Worn:", lay.grid.x + 130.0, panel.y + pad + 22.0, 22.0, LIGHTGRAY);
    draw_slot(worn_rect, worn.map(|k| ItemStack { kind: k, count: 1 }).as_ref(), worn_rect.contains(mouse));

    // --- Slots ---
    let mut hovered_stack: Option<ItemStack> = None;
    for slot in 0..slots {
        let r = slot_rect(lay.grid, slots, slot);
        let is_hovered = r.contains(mouse);
        // The stack being dragged is drawn under the cursor instead of in its slot.
        let shown = if dragged.map(|(s, _)| s) == Some(slot) { None } else { inventory.slots[slot] };
        draw_slot(r, shown.as_ref(), is_hovered);
        if slot == selected {
            draw_selection(r);
        }
        if is_hovered {
            hovered_stack = shown;
        }
    }

    // --- Right-hand list: every recipe the current items can pay for and, in dev mode, then every item ---
    let entries: Vec<CraftEntry> = if ui.guide { RECIPES.iter().map(CraftEntry::Recipe).collect() } else { craft_entries(craftable(inventory), dev_mode) };
    let row_h = if ui.guide { GUIDE_ROW_H } else { CRAFT_ROW_H };
    let max_rows = ((grid_size(slots).y + CRAFT_ROW_GAP) / (row_h + CRAFT_ROW_GAP)) as usize;
    let wheel = crate::ui::wheel_notches();
    if wheel != 0.0 {
        // The item list is long, so dev mode scrolls a few rows to the notch.
        let step = if dev_mode { 3 } else { 1 } * if wheel < 0.0 { 1 } else { -1 };
        ui.craft_scroll = (ui.craft_scroll as i32 + step).max(0) as usize;
    }
    ui.craft_scroll = ui.craft_scroll.min(entries.len().saturating_sub(max_rows));
    if entries.len() > max_rows {
        draw_text(if dev_mode && !ui.guide { "Mouse wheel: recipes, then every item" } else { "Mouse wheel: more recipes" }, lay.craft.x, panel.y + pad + 42.0, 18.0, LIGHTGRAY);
    }
    if entries.is_empty() && !ui.guide {
        draw_text("Nothing to craft yet.", lay.craft.x, lay.craft.y + 24.0, 22.0, LIGHTGRAY);
    }
    let mut message: Option<String> = None;
    for (i, entry) in entries.iter().skip(ui.craft_scroll).take(max_rows).enumerate() {
        let r = Rect::new(lay.craft.x, lay.craft.y + i as f32 * (row_h + CRAFT_ROW_GAP), CRAFT_W, row_h);
        let hover = r.contains(mouse);
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { Color::new(0.26, 0.31, 0.26, 1.0) } else { Color::new(0.07, 0.08, 0.07, 0.85) });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { ACCENT } else { Color::new(0.35, 0.38, 0.35, 1.0) });
        let icon = Rect::new(r.x + 4.0, r.y + 4.0, r.h - 8.0, r.h - 8.0);
        match *entry {
            CraftEntry::Recipe(recipe) if ui.guide => {
                draw_item_icon(recipe.output, Rect::new(r.x + 4.0, r.y + 4.0, 48.0, 48.0));
                let text_w = r.w - 60.0;
                draw_text(&fit_text(recipe.output.name(), text_w, 22), r.x + 58.0, r.y + 20.0, 22.0, WHITE);
                draw_text(&fit_text(&format!("Needs: {}", recipe.describe()), text_w, 16), r.x + 58.0, r.y + 38.0, 16.0, LIGHTGRAY);
                let missing = recipe.describe_missing(inventory);
                if missing.is_empty() {
                    draw_text("You can craft this: click it", r.x + 58.0, r.y + 56.0, 16.0, Color::new(0.45, 0.90, 0.45, 1.0));
                } else {
                    draw_text(&fit_text(&format!("You still need: {missing}"), text_w, 16), r.x + 58.0, r.y + 56.0, 16.0, Color::new(1.0, 0.72, 0.35, 1.0));
                }
                if hover && is_mouse_button_pressed(MouseButton::Left) && craft(inventory, recipe) {
                    message = Some(format!("Crafted {}", recipe.output.name()));
                }
            }
            CraftEntry::Recipe(recipe) => {
                draw_item_icon(recipe.output, icon);
                draw_text(recipe.output.name(), r.x + r.h, r.y + 25.0, 24.0, WHITE);
                draw_text(&recipe.describe(), r.x + r.h, r.y + 46.0, 18.0, LIGHTGRAY);
                if hover && is_mouse_button_pressed(MouseButton::Left) && craft(inventory, recipe) {
                    message = Some(format!("Crafted {}", recipe.output.name()));
                }
            }
            CraftEntry::Item(kind) => {
                draw_item_icon(kind, icon);
                draw_text(kind.name(), r.x + r.h, r.y + 25.0, 24.0, WHITE);
                draw_text("Dev: click adds 1, right click adds a stack", r.x + r.h, r.y + 46.0, 18.0, LIGHTGRAY);
                let count = if hover && is_mouse_button_pressed(MouseButton::Left) {
                    1
                } else if hover && is_mouse_button_pressed(MouseButton::Right) {
                    MAX_STACK
                } else {
                    0
                };
                if count > 0 {
                    message = Some(match dev_give(inventory, kind, count) {
                        0 => "Inventory full".to_string(),
                        n => format!("+{n} {}", kind.name()),
                    });
                }
            }
        }
    }
    if let Some(message) = message {
        toasts.push(message);
    }

    // --- Footer: totals and hints ---
    let totals: Vec<String> = ItemKind::ALL
        .iter()
        .filter(|&&k| inventory.count(k) > 0)
        .map(|&k| format!("{}: {}", k.name(), inventory.count(k)))
        .collect();
    for (i, line) in wrap_totals(&totals, panel.w - 2.0 * pad).iter().take(3).enumerate() {
        draw_text(line, lay.grid.x, panel.y + panel.h - 76.0 + i as f32 * 20.0, 20.0, LIGHTGRAY);
    }
    draw_text(
        if dev_mode { "Drag items to move or drop them outside. Click a recipe to craft, or an item to add it. E or Esc to close." } else { "Drag items to move or drop them outside. E or Esc to close." },
        lay.grid.x,
        panel.y + panel.h - 12.0,
        20.0,
        GRAY,
    );

    // --- Dragged stack and tooltip ---
    if let Some((_, stack)) = dragged {
        draw_stack(Rect::new(mouse.x - SLOT_SIZE / 2.0, mouse.y - SLOT_SIZE / 2.0, SLOT_SIZE, SLOT_SIZE), &stack);
    } else if let Some(stack) = hovered_stack {
        let label = format!("{} x{}", stack.kind.name(), stack.count);
        let d = measure_text(&label, None, 24, 1.0);
        let (tx, ty) = (mouse.x + 16.0, mouse.y + 22.0);
        draw_rectangle(tx - 6.0, ty - 20.0, d.width + 12.0, 28.0, Color::new(0.0, 0.0, 0.0, 0.85));
        draw_text(&label, tx, ty, 24.0, WHITE);
    }
    dropped_outside
}

/// One row of the list beside the inventory: a recipe the inventory can pay for or, in dev mode, an item to take.
#[derive(Clone, Copy)]
enum CraftEntry {
    Recipe(&'static Recipe),
    Item(ItemKind),
}

/// The rows of that list: the craftable recipes, followed in dev mode by every item in the game.
fn craft_entries(recipes: Vec<&'static Recipe>, dev_mode: bool) -> Vec<CraftEntry> {
    let items = if dev_mode { ItemKind::ALL.as_slice() } else { &[] };
    recipes.into_iter().map(CraftEntry::Recipe).chain(items.iter().copied().map(CraftEntry::Item)).collect()
}

/// Put up to `count` of `kind` into the inventory (dev mode). Returns how many fitted.
fn dev_give(inventory: &mut Inventory, kind: ItemKind, count: u32) -> u32 {
    count - inventory.add(kind, count)
}

/// Short-lived messages such as "+1 Stick".
pub struct Toasts {
    entries: Vec<(String, f32)>,
}

impl Toasts {
    const MAX_VISIBLE: usize = 6;

    pub fn new() -> Self {
        Toasts { entries: Vec::new() }
    }

    pub fn push(&mut self, message: String) {
        self.entries.push((message, TOAST_SECONDS));
        if self.entries.len() > Self::MAX_VISIBLE {
            self.entries.remove(0);
        }
    }

    pub fn update(&mut self, dt: f32) {
        for e in &mut self.entries {
            e.1 -= dt;
        }
        self.entries.retain(|e| e.1 > 0.0);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether any message on screen contains `needle`.
    #[cfg(test)]
    pub fn contains(&self, needle: &str) -> bool {
        self.entries.iter().any(|(m, _)| m.contains(needle))
    }

    pub fn draw(&self) {
        let (w, h) = (screen_width(), screen_height());
        for (i, (message, remaining)) in self.entries.iter().rev().enumerate() {
            let alpha = (remaining / 0.6).min(1.0);
            let d = measure_text(message, None, 26, 1.0);
            let y = h - 100.0 - i as f32 * 28.0;
            draw_text(message, w - d.width - 24.0, y + 1.5, 26.0, Color::new(0.0, 0.0, 0.0, alpha * 0.7));
            draw_text(message, w - d.width - 25.0, y, 26.0, Color::new(1.0, 1.0, 1.0, alpha));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_is_hit_at_its_own_centre_and_slots_do_not_overlap() {
        let grid = vec2(100.0, 80.0);
        for slots in [INVENTORY_SLOTS, INVENTORY_SLOTS + BACKPACK_SLOTS] {
            for slot in 0..slots {
                let r = slot_rect(grid, slots, slot);
                assert_eq!(slot_at(grid, slots, r.center()), Some(slot));
                for other in 0..slot {
                    assert!(!r.overlaps(&slot_rect(grid, slots, other)), "slots {slot} and {other} overlap");
                }
            }
            assert_eq!(slot_at(grid, slots, vec2(0.0, 0.0)), None);
        }
        // The backpack's rows are two more rows of the main grid, above the hotbar.
        assert_eq!(main_rows(INVENTORY_SLOTS + BACKPACK_SLOTS), main_rows(INVENTORY_SLOTS) + BACKPACK_EXTRA_ROWS);
        assert!(grid_size(INVENTORY_SLOTS + BACKPACK_SLOTS).y > grid_size(INVENTORY_SLOTS).y, "a taller panel for the pack");
    }

    #[test]
    fn hotbar_slots_are_the_bottom_row() {
        let grid = vec2(0.0, 0.0);
        let hotbar_y = slot_rect(grid, INVENTORY_SLOTS, 0).y;
        for slot in 0..HOTBAR_SLOTS {
            assert_eq!(slot_rect(grid, INVENTORY_SLOTS, slot).y, hotbar_y);
        }
        assert!(slot_rect(grid, INVENTORY_SLOTS, HOTBAR_SLOTS).y < hotbar_y);
    }

    #[test]
    fn toasts_expire_and_are_capped() {
        let mut t = Toasts::new();
        for i in 0..10 {
            t.push(format!("+1 item {i}"));
        }
        assert_eq!(t.len(), Toasts::MAX_VISIBLE);
        t.update(TOAST_SECONDS - 0.1);
        assert_eq!(t.len(), Toasts::MAX_VISIBLE);
        t.update(0.2);
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn dev_mode_lists_every_item_after_the_recipes_and_normal_mode_does_not() {
        let mut inv = Inventory::new();
        inv.add(ItemKind::Grass, 3);
        let recipes = craftable(&inv);
        assert!(!recipes.is_empty());
        let normal = craft_entries(craftable(&inv), false);
        assert_eq!(normal.len(), recipes.len());
        assert!(normal.iter().all(|e| matches!(e, CraftEntry::Recipe(_))));
        let dev = craft_entries(craftable(&inv), true);
        assert_eq!(dev.len(), recipes.len() + ItemKind::ALL.len());
        assert!(dev[..recipes.len()].iter().all(|e| matches!(e, CraftEntry::Recipe(_))), "recipes first");
        let items: Vec<ItemKind> = dev[recipes.len()..].iter().map(|e| match e { CraftEntry::Item(k) => *k, _ => panic!("recipe among the items") }).collect();
        assert_eq!(items, ItemKind::ALL.to_vec(), "every item, once");
        // With nothing craftable the dev list is still the whole item list.
        assert_eq!(craft_entries(Vec::new(), true).len(), ItemKind::ALL.len());
        assert!(craft_entries(Vec::new(), false).is_empty());
    }

    #[test]
    fn taking_an_item_in_dev_mode_adds_it_and_reports_what_fitted() {
        for kind in ItemKind::ALL {
            let mut fresh = Inventory::new();
            assert_eq!(dev_give(&mut fresh, kind, 1), 1, "{kind:?}");
            assert_eq!(fresh.count(kind), 1, "{kind:?}");
        }
        let mut inv = Inventory::new();
        assert_eq!(dev_give(&mut inv, ItemKind::Stick, 1), 1);
        assert_eq!(dev_give(&mut inv, ItemKind::Stick, 1), 1);
        assert_eq!(inv.count(ItemKind::Stick), 2);
        assert_eq!(dev_give(&mut inv, ItemKind::Stone, MAX_STACK), MAX_STACK);
        // A full inventory takes nothing more.
        let mut full = Inventory::new();
        full.add(ItemKind::Stone, INVENTORY_SLOTS as u32 * MAX_STACK);
        assert_eq!(dev_give(&mut full, ItemKind::Stick, 1), 0);
        assert_eq!(dev_give(&mut full, ItemKind::Stone, 5), 0);
    }
}
