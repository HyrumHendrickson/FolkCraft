//! Small immediate-mode UI helpers for the menus.

use macroquad::prelude::*;

pub const BG: Color = Color::new(0.10, 0.12, 0.10, 1.0);
pub const PANEL: Color = Color::new(0.18, 0.21, 0.18, 1.0);
pub const PANEL_HOVER: Color = Color::new(0.26, 0.31, 0.26, 1.0);
pub const ACCENT: Color = Color::new(0.45, 0.75, 0.40, 1.0);

pub fn text_centered(text: &str, cx: f32, y: f32, size: u16, color: Color) {
    let d = measure_text(text, None, size, 1.0);
    draw_text(text, cx - d.width / 2.0, y, size as f32, color);
}

pub fn mouse_in(r: Rect) -> bool {
    let (mx, my) = mouse_position();
    r.contains(vec2(mx, my))
}

pub fn button(r: Rect, label: &str, enabled: bool) -> bool {
    let hover = enabled && mouse_in(r);
    draw_rectangle(r.x, r.y, r.w, r.h, if hover { PANEL_HOVER } else { PANEL });
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if enabled { ACCENT } else { GRAY });
    text_centered(label, r.x + r.w / 2.0, r.y + r.h / 2.0 + 8.0, 28, if enabled { WHITE } else { GRAY });
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// A small button with its own outline colour (red for something that cannot be undone).
pub fn small_button(r: Rect, label: &str, outline: Color) -> bool {
    let hover = mouse_in(r);
    draw_rectangle(r.x, r.y, r.w, r.h, if hover { PANEL_HOVER } else { PANEL });
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, outline);
    text_centered(label, r.x + r.w / 2.0, r.y + r.h / 2.0 + 7.0, 22, WHITE);
    hover && is_mouse_button_pressed(MouseButton::Left)
}

pub const DANGER: Color = Color::new(0.85, 0.30, 0.28, 1.0);

pub fn text_field(r: Rect, value: &str, focused: bool) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.05, 0.06, 0.05, 1.0));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if focused { ACCENT } else { GRAY });
    let caret = if focused && (get_time() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
    draw_text(&format!("{value}{caret}"), r.x + 10.0, r.y + r.h / 2.0 + 8.0, 28.0, WHITE);
}

/// Mouse wheel movement in notches (one notch is about 1.0; positive is up). Some platforms report a notch as 1,
/// others as 120, so a large reading is scaled down. Use this instead of `mouse_wheel().1`.
pub fn wheel_notches() -> f32 {
    normalize_wheel(mouse_wheel().1)
}

/// See `wheel_notches`.
pub fn normalize_wheel(raw: f32) -> f32 {
    if raw.abs() >= 20.0 {
        raw / 120.0
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wheel_notch_is_one_whether_the_platform_says_1_or_120() {
        assert_eq!(normalize_wheel(1.0), 1.0);
        assert_eq!(normalize_wheel(-1.0), -1.0);
        assert_eq!(normalize_wheel(120.0), 1.0);
        assert_eq!(normalize_wheel(-240.0), -2.0);
        assert_eq!(normalize_wheel(0.0), 0.0);
    }
}
