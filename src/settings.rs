//! The Settings screen and the settings file (`saves/settings.cfg`).
//!
//! There is one setting so far: how many chunks around the player are drawn (`config::render_radius`), from
//! `MIN_RENDER_RADIUS` to `MAX_RENDER_RADIUS`. Creatures always spawn, move and vanish at the same distances
//! whatever the render distance, so a long view does not thin them out. New structures (lakes, the mountain, the
//! castle, the well, the graveyard and the rainbow) are placed beyond the render distance in force when they
//! appear, so they are never seen to pop in.

use macroquad::prelude::*;
use std::fs;
use std::path::PathBuf;

use crate::app::App;
use crate::config::*;
use crate::ui::*;

/// Where the settings are kept: beside the saved worlds.
pub fn settings_path() -> PathBuf {
    PathBuf::from(SAVE_DIR).join("settings.cfg")
}

/// The text of the settings file for a render radius.
pub fn format_settings(radius: i32) -> String {
    format!("render_radius {radius}\n")
}

/// The render radius a settings file holds, kept within range; the default for a missing or broken line.
pub fn parse_render_radius(text: &str) -> i32 {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("render_radius"))
        .filter_map(|rest| rest.trim().parse::<i32>().ok())
        .next()
        .map_or(DEFAULT_RENDER_RADIUS, |r| r.clamp(MIN_RENDER_RADIUS, MAX_RENDER_RADIUS))
}

/// Read the settings file (if there is one) and put its render radius in force.
pub fn load_settings() {
    let radius = fs::read_to_string(settings_path()).map_or(DEFAULT_RENDER_RADIUS, |t| parse_render_radius(&t));
    set_render_radius(radius);
}

/// Write the render radius in force to the settings file. Failure is ignored: the setting still holds this session.
pub fn save_settings() {
    let _ = fs::create_dir_all(SAVE_DIR);
    let _ = fs::write(settings_path(), format_settings(render_radius()));
}

/// The render radius a slider whose track starts at `track_x` and is `track_w` wide shows with the mouse at `mouse_x`.
pub fn slider_value(mouse_x: f32, track_x: f32, track_w: f32) -> i32 {
    let share = ((mouse_x - track_x) / track_w).clamp(0.0, 1.0);
    let steps = (MAX_RENDER_RADIUS - MIN_RENDER_RADIUS) as f32;
    MIN_RENDER_RADIUS + (share * steps).round() as i32
}

/// Where along the track (pixels from its start) the handle for `radius` sits.
pub fn slider_offset(radius: i32, track_w: f32) -> f32 {
    let steps = (MAX_RENDER_RADIUS - MIN_RENDER_RADIUS) as f32;
    (radius.clamp(MIN_RENDER_RADIUS, MAX_RENDER_RADIUS) - MIN_RENDER_RADIUS) as f32 / steps * track_w
}

/// The Settings screen: a slider for the render distance, and a Back button (Esc also goes back).
pub fn settings_screen(app: &mut App) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    text_centered("Settings", w / 2.0, 90.0, 56, ACCENT);

    let panel_w = (w - 60.0).min(700.0);
    let px = (w - panel_w) / 2.0;
    let radius = render_radius();
    draw_text("Render distance", px, 190.0, 30.0, WHITE);
    let value = format!("{radius} chunks ({} blocks)", radius * CHUNK_SIZE);
    let d = measure_text(&value, None, 26, 1.0);
    draw_text(&value, px + panel_w - d.width, 190.0, 26.0, ACCENT);

    let (track_x, track_y, track_w) = (px + 12.0, 244.0, panel_w - 24.0);
    let grab = Rect::new(px, track_y - 22.0, panel_w, 44.0);
    draw_rectangle(track_x, track_y - 4.0, track_w, 8.0, Color::new(0.05, 0.06, 0.05, 1.0));
    draw_rectangle_lines(track_x, track_y - 4.0, track_w, 8.0, 2.0, GRAY);
    for r in MIN_RENDER_RADIUS..=MAX_RENDER_RADIUS {
        let x = track_x + slider_offset(r, track_w);
        draw_line(x, track_y + 8.0, x, track_y + 16.0, 2.0, GRAY);
        text_centered(&r.to_string(), x, track_y + 38.0, 20, LIGHTGRAY);
    }
    let hover = mouse_in(grab);
    if is_mouse_button_pressed(MouseButton::Left) && hover {
        app.slider_dragging = true;
    }
    if !is_mouse_button_down(MouseButton::Left) {
        app.slider_dragging = false;
    }
    let mut chosen = radius;
    if app.slider_dragging {
        chosen = slider_value(mouse_position().0, track_x, track_w);
    }
    if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Down) {
        chosen -= 1;
    }
    if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Up) {
        chosen += 1;
    }
    if hover {
        let notches = wheel_notches();
        if notches != 0.0 {
            chosen += if notches > 0.0 { 1 } else { -1 };
        }
    }
    if chosen != radius {
        set_render_radius(chosen);
    }
    let handle_x = track_x + slider_offset(render_radius(), track_w);
    draw_rectangle(handle_x - 8.0, track_y - 16.0, 16.0, 32.0, if app.slider_dragging || hover { WHITE } else { ACCENT });
    draw_rectangle_lines(handle_x - 8.0, track_y - 16.0, 16.0, 32.0, 2.0, BLACK);

    draw_text("Drag the slider, or use the arrow keys. The default is 3 chunks.", px, 340.0, 22.0, LIGHTGRAY);
    draw_text("A longer view draws more of the world and costs frame rate.", px, 370.0, 22.0, GRAY);
    draw_text("Creatures always appear and vanish at the same distances, so a long view does not thin them out.", px, 400.0, 22.0, GRAY);
    draw_text("New lakes, mountains, castles, wells and graveyards appear beyond the render distance.", px, 430.0, 22.0, GRAY);

    let back = button(Rect::new(w / 2.0 - 110.0, h - 120.0, 220.0, 56.0), "Back", true);
    if back || is_key_pressed(KeyCode::Escape) {
        app.slider_dragging = false;
        save_settings();
        app.close_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_file_round_trips_and_is_kept_in_range() {
        for r in MIN_RENDER_RADIUS..=MAX_RENDER_RADIUS {
            assert_eq!(parse_render_radius(&format_settings(r)), r);
        }
        assert_eq!(parse_render_radius("render_radius 99\n"), MAX_RENDER_RADIUS);
        assert_eq!(parse_render_radius("render_radius 0\n"), MIN_RENDER_RADIUS);
        assert_eq!(parse_render_radius(""), DEFAULT_RENDER_RADIUS);
        assert_eq!(parse_render_radius("render_radius many\n"), DEFAULT_RENDER_RADIUS);
        assert_eq!(DEFAULT_RENDER_RADIUS, 3, "the default is three chunks");
        assert_eq!((MIN_RENDER_RADIUS, MAX_RENDER_RADIUS), (3, 10));
    }

    #[test]
    fn the_slider_maps_the_mouse_to_whole_chunks_from_three_to_ten() {
        let (x, w) = (100.0, 700.0);
        assert_eq!(slider_value(x - 50.0, x, w), MIN_RENDER_RADIUS);
        assert_eq!(slider_value(x, x, w), MIN_RENDER_RADIUS);
        assert_eq!(slider_value(x + w, x, w), MAX_RENDER_RADIUS);
        assert_eq!(slider_value(x + w + 80.0, x, w), MAX_RENDER_RADIUS);
        for r in MIN_RENDER_RADIUS..=MAX_RENDER_RADIUS {
            assert_eq!(slider_value(x + slider_offset(r, w), x, w), r, "the handle for {r} reads back as {r}");
        }
        assert_eq!(slider_offset(MIN_RENDER_RADIUS, w), 0.0);
        assert_eq!(slider_offset(MAX_RENDER_RADIUS, w), w);
    }

    #[test]
    fn setting_the_radius_clamps_and_moves_the_generated_ring_with_it() {
        let before = render_radius();
        assert_eq!(set_render_radius(50), MAX_RENDER_RADIUS);
        assert_eq!(gen_radius(), MAX_RENDER_RADIUS + 1);
        assert_eq!(structure_distance(), (MAX_RENDER_RADIUS + 2) * CHUNK_SIZE, "structures lie beyond the generated ring");
        assert_eq!(set_render_radius(-4), MIN_RENDER_RADIUS);
        set_render_radius(before);
    }
}
