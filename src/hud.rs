//! Inventory HUD: hotbar, full inventory screen (drag and drop, crafting panel), item
//! icons and pickup messages.

use macroquad::prelude::*;

use crate::config::*;
use crate::inventory::Inventory;
use crate::item::{ItemKind, ItemStack};
use crate::landmarks::compass_needle;
use crate::recipe::{craft, craftable, Recipe};
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

fn lighten(c: Color, amount: f32) -> Color {
    Color::new((c.r + amount).min(1.0), (c.g + amount).min(1.0), (c.b + amount).min(1.0), c.a)
}

/// Draw a flat icon for an item inside `r`.
pub fn draw_item_icon(kind: ItemKind, r: Rect) {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let color = kind.color();
    let outline = Color::new(0.05, 0.05, 0.05, 0.8);
    match kind {
        ItemKind::Stick => {
            draw_line(r.x + r.w * 0.26, r.y + r.h * 0.76, r.x + r.w * 0.74, r.y + r.h * 0.24, 6.0, outline);
            draw_line(r.x + r.w * 0.26, r.y + r.h * 0.76, r.x + r.w * 0.74, r.y + r.h * 0.24, 4.0, color);
            draw_line(r.x + r.w * 0.30, r.y + r.h * 0.70, r.x + r.w * 0.70, r.y + r.h * 0.30, 1.5, lighten(color, 0.15));
        }
        ItemKind::Pebble => {
            for (dx, dy, rad) in [(-0.10, 0.08, 0.12), (0.12, 0.10, 0.09), (0.02, -0.10, 0.10)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w + 1.0, outline);
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w, color);
            }
        }
        ItemKind::PeepStone => {
            draw_circle(cx, cy + 1.0, r.w * 0.27, outline);
            draw_circle(cx, cy + 1.0, r.w * 0.25, color);
            draw_circle(cx - r.w * 0.07, cy - r.h * 0.07, r.w * 0.09, lighten(color, 0.18));
        }
        ItemKind::HollyRod => {
            draw_line(r.x + r.w * 0.25, r.y + r.h * 0.76, r.x + r.w * 0.75, r.y + r.h * 0.24, 6.0, outline);
            draw_line(r.x + r.w * 0.25, r.y + r.h * 0.76, r.x + r.w * 0.75, r.y + r.h * 0.24, 4.0, color);
            for (dx, dy) in [(0.38, 0.60), (0.60, 0.38)] {
                draw_circle(r.x + r.w * dx, r.y + r.h * dy, 3.0, Color::new(0.78, 0.10, 0.16, 1.0));
            }
        }
        ItemKind::Stone => {
            draw_circle(cx, cy + 1.0, r.w * 0.27, outline);
            draw_circle(cx, cy + 1.0, r.w * 0.25, color);
            draw_circle(cx - r.w * 0.07, cy - r.h * 0.07, r.w * 0.09, lighten(color, 0.14));
        }
        ItemKind::Log => {
            let (x0, y0, x1, y1) = (r.x + r.w * 0.22, r.y + r.h * 0.74, r.x + r.w * 0.74, r.y + r.h * 0.30);
            draw_line(x0, y0, x1, y1, 15.0, outline);
            draw_line(x0, y0, x1, y1, 12.0, color);
            draw_line(x0 + 3.0, y0 - 3.0, x1 + 3.0, y1 - 3.0, 2.0, lighten(color, 0.12));
            // The cut end, with a growth ring.
            draw_circle(x1, y1, 7.5, outline);
            draw_circle(x1, y1, 6.5, Color::new(0.74, 0.58, 0.34, 1.0));
            draw_circle_lines(x1, y1, 3.0, 1.0, Color::new(0.50, 0.36, 0.18, 1.0));
        }
        ItemKind::StoneAxe | ItemKind::CopperAxe | ItemKind::BronzeAxe | ItemKind::IronAxe => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (h0, h1) = (p(0.24, 0.80), p(0.66, 0.32));
            draw_line(h0.x, h0.y, h1.x, h1.y, 6.5, outline);
            draw_line(h0.x, h0.y, h1.x, h1.y, 4.5, ItemKind::Stick.color());
            // Axe head: a quad drawn as two triangles, with an outline.
            let (a, b, c, d) = (p(0.46, 0.24), p(0.70, 0.12), p(0.88, 0.40), p(0.62, 0.50));
            draw_triangle(a, b, c, color);
            draw_triangle(a, c, d, color);
            for (u, v) in [(a, b), (b, c), (c, d), (d, a)] {
                draw_line(u.x, u.y, v.x, v.y, 1.5, outline);
            }
            draw_line(a.x + 2.0, a.y + 1.0, b.x - 1.0, b.y + 2.0, 1.5, lighten(color, 0.16));
        }
        ItemKind::Grass => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            for (bx, tx, ty, shade) in [(0.36, 0.24, 0.20, -0.06), (0.50, 0.52, 0.12, 0.04), (0.64, 0.78, 0.26, 0.0)] {
                let (b, t) = (p(bx, 0.80), p(tx, ty));
                let c = Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0);
                draw_line(b.x, b.y, t.x, t.y, 5.0, outline);
                draw_line(b.x, b.y, t.x, t.y, 3.0, c);
            }
        }
        ItemKind::Berries | ItemKind::PurpleBerries => {
            for (dx, dy, rad) in [(-0.14, 0.10, 0.13), (0.14, 0.08, 0.12), (0.00, -0.12, 0.13)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w + 1.0, outline);
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w, color);
                draw_circle(cx + (dx - 0.04) * r.w, cy + (dy - 0.04) * r.h, rad * r.w * 0.3, lighten(color, 0.35));
            }
            draw_line(cx, cy - 0.30 * r.h, cx + 0.07 * r.w, cy - 0.38 * r.h, 2.5, Color::new(0.25, 0.55, 0.20, 1.0));
        }
        ItemKind::Twine => {
            draw_circle_lines(cx, cy, r.w * 0.26, 7.0, outline);
            draw_circle_lines(cx, cy, r.w * 0.26, 5.0, color);
            draw_circle_lines(cx, cy, r.w * 0.26, 1.0, lighten(color, 0.15));
            // A loose end trailing off the coil.
            draw_line(cx + r.w * 0.24, cy + r.h * 0.08, cx + r.w * 0.40, cy + r.h * 0.30, 3.0, color);
        }
        ItemKind::BowDrill => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let bow = [p(0.72, 0.16), p(0.84, 0.50), p(0.72, 0.84)];
            for w in bow.windows(2) {
                draw_line(w[0].x, w[0].y, w[1].x, w[1].y, 5.5, outline);
                draw_line(w[0].x, w[0].y, w[1].x, w[1].y, 3.5, color);
            }
            draw_line(bow[0].x, bow[0].y, bow[2].x, bow[2].y, 1.5, ItemKind::Twine.color());
            // The spindle, looped in the string and pointing at a fireboard.
            let (s0, s1) = (p(0.22, 0.30), p(0.72, 0.52));
            draw_line(s0.x, s0.y, s1.x, s1.y, 5.0, outline);
            draw_line(s0.x, s0.y, s1.x, s1.y, 3.0, lighten(color, 0.12));
            draw_rectangle(r.x + r.w * 0.12, r.y + r.h * 0.66, r.w * 0.42, r.h * 0.12, outline);
            draw_rectangle(r.x + r.w * 0.13, r.y + r.h * 0.67, r.w * 0.40, r.h * 0.10, Color::new(0.40, 0.27, 0.13, 1.0));
        }
        ItemKind::Plank => {
            let (x, y, w, h) = (r.x + r.w * 0.14, r.y + r.h * 0.34, r.w * 0.72, r.h * 0.32);
            draw_rectangle(x - 1.5, y - 1.5, w + 3.0, h + 3.0, outline);
            draw_rectangle(x, y, w, h, color);
            for fy in [0.30, 0.62] {
                draw_line(x + 3.0, y + h * fy, x + w - 3.0, y + h * fy, 1.2, Color::new(color.r * 0.78, color.g * 0.78, color.b * 0.78, 1.0));
            }
            draw_line(x + 2.0, y + 2.0, x + w - 2.0, y + 2.0, 1.5, lighten(color, 0.14));
            for nx in [x + 4.0, x + w - 4.0] {
                draw_circle(nx, y + h / 2.0, 1.6, Color::new(0.25, 0.25, 0.27, 1.0));
            }
        }
        ItemKind::Feather => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (tip, base) = (p(0.80, 0.16), p(0.24, 0.82));
            let (a, b, c, d) = (p(0.46, 0.20), tip, p(0.62, 0.58), p(0.30, 0.64));
            draw_triangle(a, b, c, color);
            draw_triangle(a, c, d, color);
            for (u, v) in [(a, b), (b, c), (c, d), (d, a)] {
                draw_line(u.x, u.y, v.x, v.y, 1.5, outline);
            }
            draw_line(base.x, base.y, tip.x, tip.y, 2.5, outline);
            draw_line(base.x, base.y, tip.x, tip.y, 1.2, lighten(color, 0.10));
            draw_line(p(0.40, 0.36).x, p(0.40, 0.36).y, p(0.54, 0.46).x, p(0.54, 0.46).y, 1.0, Color::new(0.62, 0.58, 0.50, 1.0));
        }
        ItemKind::Bow => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let arc = [p(0.30, 0.12), p(0.50, 0.28), p(0.58, 0.50), p(0.50, 0.72), p(0.30, 0.88)];
            for w in arc.windows(2) {
                draw_line(w[0].x, w[0].y, w[1].x, w[1].y, 6.5, outline);
                draw_line(w[0].x, w[0].y, w[1].x, w[1].y, 4.5, color);
            }
            let (s0, s1) = (arc[0], arc[4]);
            draw_line(s0.x, s0.y, s1.x, s1.y, 1.6, ItemKind::Twine.color());
        }
        ItemKind::Arrow => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (tail, tip) = (p(0.22, 0.78), p(0.76, 0.24));
            draw_line(tail.x, tail.y, tip.x, tip.y, 4.5, outline);
            draw_line(tail.x, tail.y, tip.x, tip.y, 2.5, color);
            // Stone head and feather fletching.
            let (h0, h1, h2) = (p(0.66, 0.18), p(0.88, 0.12), p(0.82, 0.34));
            draw_triangle(h0, h1, h2, ItemKind::Stone.color());
            for (a, b, c) in [(p(0.22, 0.78), p(0.14, 0.62), p(0.34, 0.68)), (p(0.22, 0.78), p(0.38, 0.86), p(0.30, 0.66))] {
                draw_triangle(a, b, c, ItemKind::Feather.color());
            }
        }
        ItemKind::Venison | ItemKind::CookedVenison | ItemKind::RabbitMeat | ItemKind::CookedRabbit | ItemKind::MooseMeat | ItemKind::CookedMoose => {
            let cooked = kind == ItemKind::CookedVenison;
            draw_circle(cx - r.w * 0.04, cy, r.w * 0.30, outline);
            draw_circle(cx - r.w * 0.04, cy, r.w * 0.28, color);
            draw_circle(cx + r.w * 0.12, cy - r.h * 0.10, r.w * 0.14, if cooked { lighten(color, 0.10) } else { lighten(color, 0.12) });
            if cooked {
                draw_line(cx - r.w * 0.20, cy + r.h * 0.10, cx + r.w * 0.04, cy + r.h * 0.14, 2.0, Color::new(0.25, 0.13, 0.07, 1.0));
            } else {
                draw_line(cx - r.w * 0.20, cy + r.h * 0.06, cx + r.w * 0.02, cy + r.h * 0.12, 2.0, Color::new(0.92, 0.80, 0.78, 1.0));
            }
        }
        ItemKind::Poultry | ItemKind::CookedPoultry => {
            let cooked = kind == ItemKind::CookedPoultry;
            let (b0, b1) = (vec2(cx + r.w * 0.04, cy + r.h * 0.04), vec2(cx + r.w * 0.30, cy + r.h * 0.30));
            draw_line(b0.x, b0.y, b1.x, b1.y, 6.5, outline);
            draw_line(b0.x, b0.y, b1.x, b1.y, 4.0, Color::new(0.93, 0.90, 0.82, 1.0));
            draw_circle(b1.x + 1.5, b1.y + 1.5, 4.5, Color::new(0.93, 0.90, 0.82, 1.0));
            draw_circle(cx - r.w * 0.08, cy - r.h * 0.08, r.w * 0.26, outline);
            draw_circle(cx - r.w * 0.08, cy - r.h * 0.08, r.w * 0.24, color);
            draw_circle(cx - r.w * 0.15, cy - r.h * 0.15, r.w * 0.08, lighten(color, if cooked { 0.12 } else { 0.08 }));
        }
        ItemKind::Leather => {
            let (x, y, w, h) = (r.x + r.w * 0.18, r.y + r.h * 0.26, r.w * 0.64, r.h * 0.48);
            draw_rectangle(x - 1.5, y - 1.5, w + 3.0, h + 3.0, outline);
            draw_rectangle(x, y, w, h, color);
            for i in 0..5 {
                let sx = x + 5.0 + i as f32 * (w - 10.0) / 4.0;
                draw_line(sx, y + 4.0, sx + 2.5, y + 4.0, 1.5, lighten(color, 0.25));
                draw_line(sx, y + h - 4.0, sx + 2.5, y + h - 4.0, 1.5, lighten(color, 0.25));
            }
        }
        ItemKind::BearMeat | ItemKind::CookedBearMeat => {
            let cooked = kind == ItemKind::CookedBearMeat;
            // A thick slab with a band of fat across it and the knob of a bone at one end.
            let (x, y, w, h) = (r.x + r.w * 0.14, r.y + r.h * 0.30, r.w * 0.62, r.h * 0.44);
            let bone = Color::new(0.93, 0.90, 0.82, 1.0);
            draw_circle(x + w + 1.0, y + h * 0.35, 6.0, outline);
            draw_rectangle(x - 1.5, y - 1.5, w + 3.0, h + 3.0, outline);
            draw_circle(x + w + 1.0, y + h * 0.35, 4.5, bone);
            draw_rectangle(x, y, w, h, color);
            let fat = if cooked { lighten(color, 0.12) } else { Color::new(0.93, 0.82, 0.78, 1.0) };
            draw_rectangle(x, y + h * 0.60, w, h * 0.14, fat);
            draw_line(x + 4.0, y + 4.0, x + w * 0.55, y + 5.0, 1.5, lighten(color, 0.14));
        }
        ItemKind::BearPelt => {
            // A flat hide with a shaggy lower edge and streaks of fur.
            let (x, y, w, h) = (r.x + r.w * 0.16, r.y + r.h * 0.22, r.w * 0.68, r.h * 0.52);
            draw_rectangle(x - 1.5, y - 1.5, w + 3.0, h + 3.0, outline);
            draw_rectangle(x, y, w, h, color);
            for i in 0..6 {
                let fx = x + 3.0 + i as f32 * (w - 6.0) / 5.0;
                let drop = 4.0 + (i % 2) as f32 * 3.0;
                draw_line(fx, y + h - 1.0, fx, y + h + drop, 4.0, outline);
                draw_line(fx, y + h - 1.0, fx, y + h + drop - 1.0, 2.5, color);
            }
            for i in 0..4 {
                let fx = x + 7.0 + i as f32 * (w - 14.0) / 3.0;
                draw_line(fx, y + 5.0, fx + 2.0, y + h - 7.0, 1.5, lighten(color, 0.12));
            }
        }
        ItemKind::Bellows => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            // Two boards joined by a folded leather skin, and a nozzle at the narrow end.
            let (a0, a1, b0, b1) = (p(0.14, 0.62), p(0.52, 0.24), p(0.30, 0.82), p(0.68, 0.44));
            draw_triangle(a0, a1, b1, Color::new(0.58, 0.38, 0.22, 1.0));
            draw_triangle(a0, b1, b0, Color::new(0.58, 0.38, 0.22, 1.0));
            for t in [0.25, 0.5, 0.75] {
                let (u, v) = (a0 + (a1 - a0) * t, b0 + (b1 - b0) * t);
                draw_line(u.x, u.y, v.x, v.y, 1.5, outline);
            }
            for (u, v) in [(a0, a1), (b0, b1)] {
                draw_line(u.x, u.y, v.x, v.y, 6.0, outline);
                draw_line(u.x, u.y, v.x, v.y, 4.0, color);
            }
            let (n0, n1) = (p(0.60, 0.34), p(0.88, 0.14));
            draw_line(n0.x, n0.y, n1.x, n1.y, 5.0, outline);
            draw_line(n0.x, n0.y, n1.x, n1.y, 3.0, Color::new(0.35, 0.35, 0.38, 1.0));
        }
        ItemKind::Ash => {
            for (dx, dy, rad, shade) in [(-0.16, 0.14, 0.15, 0.0), (0.14, 0.12, 0.13, 0.05), (0.0, 0.0, 0.17, 0.08), (-0.04, 0.20, 0.10, -0.04)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w + 1.0, outline);
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w, Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0));
            }
        }
        ItemKind::Clay => {
            // A damp lump with a darker underside and a pale highlight.
            draw_circle(cx, cy + 3.0, r.w * 0.29, outline);
            draw_circle(cx, cy + 3.0, r.w * 0.27, Color::new(color.r * 0.82, color.g * 0.82, color.b * 0.82, 1.0));
            draw_circle(cx, cy - 1.0, r.w * 0.25, color);
            draw_circle(cx - r.w * 0.08, cy - r.h * 0.08, r.w * 0.08, lighten(color, 0.16));
        }
        ItemKind::IronOre | ItemKind::GoldOre => {
            // The stone icon with three rust-coloured veins.
            let stone = ItemKind::Stone.color();
            draw_circle(cx, cy + 1.0, r.w * 0.27, outline);
            draw_circle(cx, cy + 1.0, r.w * 0.25, stone);
            draw_circle(cx - r.w * 0.07, cy - r.h * 0.07, r.w * 0.09, lighten(stone, 0.14));
            for (x0, y0, x1, y1) in [(-0.16, 0.02, 0.06, -0.12), (-0.06, 0.16, 0.16, 0.04), (0.04, -0.14, 0.18, -0.02)] {
                draw_line(cx + x0 * r.w, cy + y0 * r.h, cx + x1 * r.w, cy + y1 * r.h, 3.0, color);
            }
        }
        ItemKind::VenisonStew | ItemKind::PoultryStew | ItemKind::BearStew | ItemKind::RabbitStew | ItemKind::MooseStew
        | ItemKind::RedBerryStew | ItemKind::PurpleBerryStew => {
            // A tin bowl of stew with a few lumps in it.
            let bowl = ItemKind::TinPot.color();
            draw_circle(cx, cy + 4.0, r.w * 0.31, outline);
            draw_circle(cx, cy + 4.0, r.w * 0.29, Color::new(bowl.r * 0.8, bowl.g * 0.8, bowl.b * 0.8, 1.0));
            draw_circle(cx, cy + 1.0, r.w * 0.25, color);
            for (dx, dy) in [(-0.10, 0.0), (0.08, -0.04), (0.02, 0.08)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, r.w * 0.04, lighten(color, 0.18));
            }
        }
        ItemKind::Sling => {
            // A leather pouch on two cords that meet in a loop.
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (pouch, loop_a, loop_b) = (p(0.50, 0.30), p(0.22, 0.80), p(0.78, 0.80));
            for end in [loop_a, loop_b] {
                draw_line(pouch.x, pouch.y, end.x, end.y, 4.0, outline);
                draw_line(pouch.x, pouch.y, end.x, end.y, 2.0, ItemKind::LeatherCord.color());
            }
            draw_circle(loop_a.x, loop_a.y, 4.0, outline);
            draw_circle(loop_a.x, loop_a.y, 2.5, ItemKind::LeatherCord.color());
            draw_circle(loop_b.x, loop_b.y, 4.0, outline);
            draw_circle(loop_b.x, loop_b.y, 2.5, ItemKind::LeatherCord.color());
            draw_circle(pouch.x, pouch.y, r.w * 0.17, outline);
            draw_circle(pouch.x, pouch.y, r.w * 0.15, color);
            draw_circle(pouch.x - r.w * 0.04, pouch.y - r.h * 0.04, r.w * 0.05, lighten(color, 0.14));
        }
        ItemKind::Loadstone => {
            // A dark, slightly bluish rock with a metallic glint.
            draw_circle(cx, cy + 1.0, r.w * 0.27, outline);
            draw_circle(cx, cy + 1.0, r.w * 0.25, color);
            draw_circle(cx - r.w * 0.06, cy - r.h * 0.06, r.w * 0.09, Color::new(0.46, 0.48, 0.58, 1.0));
            draw_circle(cx - r.w * 0.08, cy - r.h * 0.08, r.w * 0.03, Color::new(0.80, 0.82, 0.90, 1.0));
        }
        ItemKind::Compass => {
            // A brass case with a glass face and a red and white needle.
            draw_circle(cx, cy, r.w * 0.31, outline);
            draw_circle(cx, cy, r.w * 0.29, color);
            draw_circle(cx, cy, r.w * 0.23, Color::new(0.90, 0.88, 0.80, 1.0));
            let (n, sth) = (vec2(cx + r.w * 0.09, cy - r.h * 0.16), vec2(cx - r.w * 0.09, cy + r.h * 0.16));
            draw_line(cx, cy, n.x, n.y, 3.0, Color::new(0.80, 0.14, 0.14, 1.0));
            draw_line(cx, cy, sth.x, sth.y, 3.0, Color::new(0.96, 0.96, 0.96, 1.0));
            draw_circle(cx, cy, 2.0, outline);
        }
        ItemKind::RabbitFoot => {
            // A small furry foot with three toes.
            draw_circle(cx, cy + 3.0, r.w * 0.20, outline);
            draw_circle(cx, cy + 3.0, r.w * 0.18, color);
            for dx in [-0.12, 0.0, 0.12] {
                draw_circle(cx + dx * r.w, cy - 0.12 * r.h, r.w * 0.065, outline);
                draw_circle(cx + dx * r.w, cy - 0.12 * r.h, r.w * 0.05, lighten(color, 0.10));
            }
        }
        ItemKind::TinPot => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (tl, br) = (p(0.22, 0.36), p(0.78, 0.78));
            draw_rectangle(tl.x - 1.5, tl.y - 1.5, br.x - tl.x + 3.0, br.y - tl.y + 3.0, outline);
            draw_rectangle(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            draw_rectangle(tl.x, tl.y, br.x - tl.x, 4.0, lighten(color, 0.16));
            draw_line(tl.x, p(0.0, 0.46).y, tl.x - 5.0, p(0.0, 0.40).y, 3.0, outline);
            draw_line(br.x, p(0.0, 0.46).y, br.x + 5.0, p(0.0, 0.40).y, 3.0, outline);
        }
        ItemKind::ClayJar | ItemKind::WaterJar => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (bl, br) = (p(0.24, 0.40), p(0.76, 0.82));
            draw_rectangle(bl.x - 1.5, bl.y - 1.5, br.x - bl.x + 3.0, br.y - bl.y + 3.0, outline);
            draw_rectangle(bl.x, bl.y, br.x - bl.x, br.y - bl.y, color);
            let (nl, nr) = (p(0.36, 0.22), p(0.64, 0.42));
            draw_rectangle(nl.x - 1.5, nl.y - 1.5, nr.x - nl.x + 3.0, nr.y - nl.y + 3.0, outline);
            draw_rectangle(nl.x, nl.y, nr.x - nl.x, nr.y - nl.y, color);
            if kind == ItemKind::WaterJar {
                draw_rectangle(bl.x + 3.0, bl.y + 4.0, br.x - bl.x - 6.0, br.y - bl.y - 8.0, Color::new(0.25, 0.48, 0.85, 1.0));
            } else {
                draw_rectangle(bl.x + 3.0, bl.y + 3.0, 4.0, br.y - bl.y - 6.0, lighten(color, 0.14));
            }
        }
        ItemKind::LeatherCord => {
            // Two short strips of leather lying crossed.
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            for (a, b) in [(p(0.22, 0.30), p(0.78, 0.66)), (p(0.24, 0.70), p(0.76, 0.28))] {
                draw_line(a.x, a.y, b.x, b.y, 6.0, outline);
                draw_line(a.x, a.y, b.x, b.y, 4.0, color);
            }
        }
        ItemKind::BronzeMallet => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (h0, h1) = (p(0.22, 0.84), p(0.62, 0.38));
            draw_line(h0.x, h0.y, h1.x, h1.y, 6.5, outline);
            draw_line(h0.x, h0.y, h1.x, h1.y, 4.5, ItemKind::Stick.color());
            // A blocky head set across the top of the handle, bound with a leather cord.
            let (a, b, c, d) = (p(0.44, 0.22), p(0.62, 0.10), p(0.88, 0.34), p(0.70, 0.52));
            draw_triangle(a, b, c, color);
            draw_triangle(a, c, d, color);
            for (u, v) in [(a, b), (b, c), (c, d), (d, a)] {
                draw_line(u.x, u.y, v.x, v.y, 1.5, outline);
            }
            draw_line(a.x + 2.0, a.y + 1.0, b.x - 1.0, b.y + 2.0, 1.5, lighten(color, 0.16));
            let bind = p(0.54, 0.46);
            draw_line(bind.x - 4.0, bind.y - 3.0, bind.x + 4.0, bind.y + 3.0, 2.5, ItemKind::LeatherCord.color());
        }
        ItemKind::ClayForge => {
            // A squat clay block with a dark opening on top.
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            let (tl, br) = (p(0.16, 0.42), p(0.84, 0.82));
            draw_rectangle(tl.x - 1.5, tl.y - 1.5, br.x - tl.x + 3.0, br.y - tl.y + 3.0, outline);
            draw_rectangle(tl.x, tl.y, br.x - tl.x, br.y - tl.y, color);
            draw_rectangle(tl.x, br.y - 5.0, br.x - tl.x, 5.0, Color::new(color.r * 0.78, color.g * 0.78, color.b * 0.78, 1.0));
            let (ol, or) = (p(0.28, 0.30), p(0.72, 0.46));
            draw_rectangle(ol.x - 1.5, ol.y - 1.5, or.x - ol.x + 3.0, or.y - ol.y + 3.0, outline);
            draw_rectangle(ol.x, ol.y, or.x - ol.x, or.y - ol.y, Color::new(0.40, 0.31, 0.26, 1.0));
        }
        ItemKind::Copper | ItemKind::Tin | ItemKind::Iron | ItemKind::Gold | ItemKind::Bronze => {
            let p = |fx: f32, fy: f32| vec2(r.x + r.w * fx, r.y + r.h * fy);
            // An ingot: a wide trapezoid with a bright top face.
            let (a, b, c, d) = (p(0.26, 0.46), p(0.74, 0.46), p(0.84, 0.72), p(0.16, 0.72));
            for (u, v) in [(a, b), (b, c), (c, d), (d, a)] {
                draw_line(u.x, u.y, v.x, v.y, 3.5, outline);
            }
            draw_triangle(a, b, c, color);
            draw_triangle(a, c, d, color);
            let (e, f) = (p(0.32, 0.50), p(0.68, 0.50));
            draw_line(e.x, e.y, f.x, f.y, 3.0, lighten(color, 0.18));
            draw_line(p(0.22, 0.68).x, p(0.22, 0.68).y, p(0.78, 0.68).x, p(0.78, 0.68).y, 2.0, Color::new(color.r * 0.7, color.g * 0.7, color.b * 0.7, 1.0));
        }
        ItemKind::Charcoal => {
            for (dx, dy, rad, shade) in [(-0.10, 0.08, 0.17, 0.0), (0.13, 0.04, 0.13, 0.03), (0.0, -0.12, 0.13, 0.06)] {
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w + 1.0, outline);
                draw_circle(cx + dx * r.w, cy + dy * r.h, rad * r.w, Color::new(color.r + shade, color.g + shade, color.b + shade, 1.0));
            }
            draw_circle(cx - 0.12 * r.w, cy + 0.02 * r.h, r.w * 0.04, lighten(color, 0.30));
        }
    }
}

/// An item icon with its stack count in the corner (no frame).
/// The compass in hand: a dial in the top right whose red needle points north (the way to a mountain) and
/// whose tail points south, turning as the player turns, with the blocks walked north so far below it.
pub fn draw_compass(yaw: f32, progress: f32) {
    let (w, radius) = (screen_width(), 46.0);
    let (cx, cy) = (w - radius - 24.0, radius + 56.0);
    draw_circle(cx, cy, radius + 4.0, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_circle(cx, cy, radius, Color::new(0.90, 0.88, 0.80, 0.95));
    let (nx, ny) = compass_needle(yaw);
    draw_line(cx, cy, cx - nx * radius * 0.6, cy - ny * radius * 0.6, 5.0, Color::new(0.96, 0.96, 0.96, 1.0));
    draw_line(cx, cy, cx + nx * radius * 0.85, cy + ny * radius * 0.85, 5.0, Color::new(0.80, 0.14, 0.14, 1.0));
    draw_circle(cx, cy, 4.0, Color::new(0.05, 0.05, 0.05, 1.0));
    draw_text("N", cx + nx * (radius + 12.0) - 6.0, cy + ny * (radius + 12.0) + 6.0, 22.0, WHITE);
    let line = format!("North {:.0}/{} blocks", progress.min((COMPASS_TREK_CHUNKS * CHUNK_SIZE) as f32), COMPASS_TREK_CHUNKS * CHUNK_SIZE);
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
}

impl InventoryUi {
    pub fn new() -> Self {
        InventoryUi { drag_from: None, craft_scroll: 0 }
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

fn main_rows() -> usize {
    (INVENTORY_SLOTS - HOTBAR_SLOTS) / COLUMNS
}

fn grid_size() -> Vec2 {
    let w = COLUMNS as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP;
    let h = (main_rows() + 1) as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP + HOTBAR_SEPARATION;
    vec2(w, h)
}

fn layout() -> Layout {
    let (w, h) = (screen_width(), screen_height());
    let grid = grid_size();
    let (pad, title_h, footer_h) = (24.0, 46.0, 96.0);
    let size = vec2(grid.x + CRAFT_GAP + CRAFT_W + 2.0 * pad, grid.y + 2.0 * pad + title_h + footer_h);
    let panel = Rect::new((w - size.x) / 2.0, (h - size.y) / 2.0, size.x, size.y);
    let origin = vec2(panel.x + pad, panel.y + pad + title_h);
    Layout { panel, grid: origin, craft: vec2(origin.x + grid.x + CRAFT_GAP, origin.y) }
}

/// Screen rectangle of inventory slot `slot`: the main grid on top, the hotbar row below.
fn slot_rect(grid: Vec2, slot: usize) -> Rect {
    let (col, row, extra) = if slot >= HOTBAR_SLOTS {
        ((slot - HOTBAR_SLOTS) % COLUMNS, (slot - HOTBAR_SLOTS) / COLUMNS, 0.0)
    } else {
        (slot, main_rows(), HOTBAR_SEPARATION)
    };
    Rect::new(
        grid.x + col as f32 * (SLOT_SIZE + SLOT_GAP),
        grid.y + row as f32 * (SLOT_SIZE + SLOT_GAP) + extra,
        SLOT_SIZE,
        SLOT_SIZE,
    )
}

fn slot_at(grid: Vec2, mouse: Vec2) -> Option<usize> {
    (0..INVENTORY_SLOTS).find(|&slot| slot_rect(grid, slot).contains(mouse))
}

/// Join `parts` with spacing, starting a new line whenever one would exceed `max_width`.
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
pub fn inventory_screen(inventory: &mut Inventory, ui: &mut InventoryUi, selected: usize, toasts: &mut Toasts, dev_mode: bool) -> Option<usize> {
    let (w, h) = (screen_width(), screen_height());
    let lay = layout();
    let (mx, my) = mouse_position();
    let mouse = vec2(mx, my);
    let hovered_slot = slot_at(lay.grid, mouse);

    // --- Drag and drop input ---
    if is_mouse_button_pressed(MouseButton::Left) {
        if let Some(slot) = hovered_slot {
            if inventory.slots[slot].is_some() {
                ui.drag_from = Some(slot);
            }
        }
    }
    let mut dropped_outside = None;
    if is_mouse_button_released(MouseButton::Left) {
        if let Some(from) = ui.drag_from.take() {
            if let Some(to) = hovered_slot {
                inventory.move_stack(from, to);
            } else if !lay.panel.contains(mouse) {
                dropped_outside = Some(from);
            }
        }
    }
    let dragged = ui.drag_from.and_then(|s| inventory.slots[s].map(|stack| (s, stack)));

    // --- Panel ---
    let (pad, panel) = (24.0, lay.panel);
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, Color::new(0.14, 0.17, 0.14, 0.97));
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 3.0, ACCENT);
    draw_text("Inventory", lay.grid.x, panel.y + pad + 22.0, 34.0, WHITE);
    draw_text("Crafting", lay.craft.x, panel.y + pad + 22.0, 34.0, WHITE);

    // --- Slots ---
    let mut hovered_stack: Option<ItemStack> = None;
    for slot in 0..INVENTORY_SLOTS {
        let r = slot_rect(lay.grid, slot);
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
    let entries = craft_entries(craftable(inventory), dev_mode);
    let max_rows = ((grid_size().y + CRAFT_ROW_GAP) / (CRAFT_ROW_H + CRAFT_ROW_GAP)) as usize;
    let wheel = mouse_wheel().1;
    if wheel != 0.0 {
        // The item list is long, so dev mode scrolls a few rows to the notch.
        let step = if dev_mode { 3 } else { 1 } * if wheel < 0.0 { 1 } else { -1 };
        ui.craft_scroll = (ui.craft_scroll as i32 + step).max(0) as usize;
    }
    ui.craft_scroll = ui.craft_scroll.min(entries.len().saturating_sub(max_rows));
    if entries.len() > max_rows {
        draw_text(if dev_mode { "Mouse wheel: recipes, then every item" } else { "Mouse wheel: more recipes" }, lay.craft.x + 130.0, panel.y + pad + 20.0, 18.0, LIGHTGRAY);
    }
    if entries.is_empty() {
        draw_text("Nothing to craft yet.", lay.craft.x, lay.craft.y + 24.0, 22.0, LIGHTGRAY);
    }
    let mut message: Option<String> = None;
    for (i, entry) in entries.iter().skip(ui.craft_scroll).take(max_rows).enumerate() {
        let r = Rect::new(lay.craft.x, lay.craft.y + i as f32 * (CRAFT_ROW_H + CRAFT_ROW_GAP), CRAFT_W, CRAFT_ROW_H);
        let hover = r.contains(mouse);
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { Color::new(0.26, 0.31, 0.26, 1.0) } else { Color::new(0.07, 0.08, 0.07, 0.85) });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { ACCENT } else { Color::new(0.35, 0.38, 0.35, 1.0) });
        let icon = Rect::new(r.x + 4.0, r.y + 4.0, r.h - 8.0, r.h - 8.0);
        match *entry {
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
        for slot in 0..INVENTORY_SLOTS {
            let r = slot_rect(grid, slot);
            assert_eq!(slot_at(grid, r.center()), Some(slot));
            for other in 0..slot {
                assert!(!r.overlaps(&slot_rect(grid, other)), "slots {slot} and {other} overlap");
            }
        }
        assert_eq!(slot_at(grid, vec2(0.0, 0.0)), None);
    }

    #[test]
    fn hotbar_slots_are_the_bottom_row() {
        let grid = vec2(0.0, 0.0);
        let hotbar_y = slot_rect(grid, 0).y;
        for slot in 0..HOTBAR_SLOTS {
            assert_eq!(slot_rect(grid, slot).y, hotbar_y);
        }
        assert!(slot_rect(grid, HOTBAR_SLOTS).y < hotbar_y);
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
