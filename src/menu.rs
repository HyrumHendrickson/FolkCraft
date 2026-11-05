//! World-select and create-world screens.

use macroquad::prelude::*;
use std::collections::HashSet;

use crate::app::{App, Field, Mode};
use crate::inventory::Inventory;
use crate::player::Player;
use crate::save::{parse_seed, save_world, world_path, WorldInfo};
use crate::ui::*;
use crate::world::World;

pub fn worlds_screen(app: &mut App) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    text_centered("Folk Craft", w / 2.0, 80.0, 64, ACCENT);
    text_centered("Select a world", w / 2.0, 120.0, 28, LIGHTGRAY);

    let list_top = 150.0;
    let list_bottom = h - 110.0;
    let row_h = 64.0;
    let row_w = (w - 80.0).min(700.0);
    let row_x = (w - row_w) / 2.0;

    let content_h = app.worlds.len() as f32 * (row_h + 8.0);
    let max_scroll = (content_h - (list_bottom - list_top)).max(0.0);
    app.scroll = (app.scroll - wheel_notches() * 30.0).clamp(0.0, max_scroll);

    if app.worlds.is_empty() {
        text_centered("No worlds yet. Create one below.", w / 2.0, list_top + 60.0, 28, GRAY);
    }

    // While the delete dialog is open nothing behind it reacts.
    let modal = app.pending_delete.is_some();
    let mut to_play: Option<WorldInfo> = None;
    let mut to_delete: Option<WorldInfo> = None;
    for (i, info) in app.worlds.iter().enumerate() {
        let y = list_top + i as f32 * (row_h + 8.0) - app.scroll;
        if y < list_top - row_h || y > list_bottom {
            continue;
        }
        let r = Rect::new(row_x, y, row_w, row_h);
        let delete_rect = Rect::new(r.x + r.w - 110.0, r.y + 14.0, 94.0, 36.0);
        let in_view = my_in_list(list_top, list_bottom) && !modal;
        let on_delete = in_view && mouse_in(delete_rect);
        let hover = in_view && mouse_in(r) && !on_delete;
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { PANEL_HOVER } else { PANEL });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { ACCENT } else { DARKGRAY });
        draw_text(&info.name, r.x + 16.0, r.y + 28.0, 30.0, WHITE);
        draw_text(&format!("Seed: {}", info.seed), r.x + 16.0, r.y + 52.0, 22.0, LIGHTGRAY);
        if hover {
            let d = measure_text("Click to play", None, 24, 1.0);
            draw_text("Click to play", delete_rect.x - d.width - 16.0, r.y + 38.0, 24.0, ACCENT);
            if is_mouse_button_pressed(MouseButton::Left) {
                to_play = Some(info.clone());
            }
        }
        // Drawn without the hover colour while a dialog is open, and only clickable inside the list.
        let clicked = small_button(delete_rect, "Delete", DANGER) && in_view;
        if clicked {
            to_delete = Some(info.clone());
        }
    }

    // Cover list overflow so rows do not draw over the title or the button.
    draw_rectangle(0.0, 0.0, w, list_top - 2.0, BG);
    draw_rectangle(0.0, list_bottom + 2.0, w, h - list_bottom, BG);
    text_centered("Folk Craft", w / 2.0, 80.0, 64, ACCENT);
    text_centered("Select a world", w / 2.0, 120.0, 28, LIGHTGRAY);
    if !app.error.is_empty() {
        text_centered(&app.error, w / 2.0, 146.0, 22, Color::new(0.95, 0.4, 0.4, 1.0));
    }

    let create_rect = Rect::new(w / 2.0 - 290.0, h - 90.0, 360.0, 56.0);
    let help_rect = Rect::new(w / 2.0 + 90.0, h - 90.0, 200.0, 56.0);
    if button(create_rect, "Create New World", !modal) {
        app.name.clear();
        app.seed.clear();
        app.dev_mode = false;
        app.error.clear();
        app.field = Field::Name;
        app.mode = Mode::Create;
        while get_char_pressed().is_some() {}
        return;
    }
    if button(help_rect, "Help", !modal) {
        app.error.clear();
        app.open_help();
        return;
    }

    if let Some(info) = to_delete {
        app.ask_delete(info);
    } else if !modal {
        if let Some(info) = to_play {
            app.enter_game(info);
            return;
        }
    }
    if modal {
        delete_dialog(app);
    }
}

/// The "are you sure" box over the world list. Delete removes the world for good; Cancel (or Esc) keeps it.
fn delete_dialog(app: &mut App) {
    let Some(name) = app.pending_delete.as_ref().map(|i| i.name.clone()) else { return };
    let (w, h) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
    let panel = Rect::new(w / 2.0 - 300.0, h / 2.0 - 120.0, 600.0, 240.0);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, BG);
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 3.0, DANGER);
    text_centered("Delete this world?", w / 2.0, panel.y + 50.0, 38, WHITE);
    let shown: String = if name.chars().count() > 36 { format!("{}...", name.chars().take(33).collect::<String>()) } else { name };
    text_centered(&format!("\"{shown}\""), w / 2.0, panel.y + 96.0, 30, ACCENT);
    text_centered("It will be gone for good. This cannot be undone.", w / 2.0, panel.y + 134.0, 24, LIGHTGRAY);
    let delete = small_button(Rect::new(panel.x + 70.0, panel.y + 164.0, 200.0, 50.0), "Delete", DANGER);
    let cancel = small_button(Rect::new(panel.x + 330.0, panel.y + 164.0, 200.0, 50.0), "Cancel", ACCENT);
    if delete {
        app.confirm_delete();
    } else if cancel || is_key_pressed(KeyCode::Escape) {
        app.cancel_delete();
    }
}

pub fn my_in_list(top: f32, bottom: f32) -> bool {
    let (_, my) = mouse_position();
    my >= top && my <= bottom
}

pub fn create_screen(app: &mut App) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    text_centered("Create New World", w / 2.0, 90.0, 56, ACCENT);

    let fw = 500.0_f32.min(w - 60.0);
    let fx = (w - fw) / 2.0;
    let name_rect = Rect::new(fx, 190.0, fw, 52.0);
    let seed_rect = Rect::new(fx, 300.0, fw, 52.0);
    draw_text("World name", fx, 180.0, 26.0, LIGHTGRAY);
    draw_text("Seed (blank = random)", fx, 290.0, 26.0, LIGHTGRAY);

    if is_mouse_button_pressed(MouseButton::Left) {
        if mouse_in(name_rect) { app.field = Field::Name; }
        if mouse_in(seed_rect) { app.field = Field::Seed; }
    }
    if is_key_pressed(KeyCode::Tab) {
        app.field = if app.field == Field::Name { Field::Seed } else { Field::Name };
    }

    // Text entry.
    let max_len = if app.field == Field::Name { 32 } else { 24 };
    {
        let target = if app.field == Field::Name { &mut app.name } else { &mut app.seed };
        while let Some(c) = get_char_pressed() {
            if !c.is_control() && target.chars().count() < max_len {
                target.push(c);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            target.pop();
            app.backspace_hold = -0.4; // initial delay before repeat
        } else if is_key_down(KeyCode::Backspace) {
            app.backspace_hold += get_frame_time();
            if app.backspace_hold > 0.05 {
                target.pop();
                app.backspace_hold = 0.0;
            }
        }
    }

    text_field(name_rect, &app.name, app.field == Field::Name);
    text_field(seed_rect, &app.seed, app.field == Field::Seed);

    // Game mode: a button that flips between normal and dev mode.
    draw_text("Game mode", fx, 400.0, 26.0, LIGHTGRAY);
    let mode_label = if app.dev_mode { "Dev mode" } else { "Normal" };
    if button(Rect::new(fx, 410.0, fw, 52.0), mode_label, true) {
        app.dev_mode = !app.dev_mode;
    }
    let mode_note = if app.dev_mode { "The inventory lists every item; click one to add it." } else { "The usual game." };
    draw_text(mode_note, fx, 490.0, 22.0, GRAY);

    if !app.error.is_empty() {
        text_centered(&app.error, w / 2.0, 530.0, 24, Color::new(0.95, 0.4, 0.4, 1.0));
    }

    let can_create = !app.name.trim().is_empty();
    let create_clicked = button(Rect::new(w / 2.0 - 230.0, h - 150.0, 220.0, 56.0), "Create", can_create);
    let back_clicked = button(Rect::new(w / 2.0 + 10.0, h - 150.0, 220.0, 56.0), "Back", true);

    if back_clicked || is_key_pressed(KeyCode::Escape) {
        app.mode = Mode::Worlds;
        return;
    }
    if create_clicked || (can_create && is_key_pressed(KeyCode::Enter)) {
        let name = app.name.trim().to_string();
        let path = world_path(&name);
        let duplicate = app.worlds.iter().any(|w| w.path == path);
        if duplicate || path.exists() {
            app.error = "A world with that name already exists.".into();
            return;
        }
        let info = WorldInfo { name, seed: parse_seed(&app.seed), path };
        save_world(&info, &World::new(info.seed, HashSet::new()), &Player::new(), &Inventory::new(), &[], crate::config::MAX_HEALTH, app.dev_mode);
        app.enter_game(info);
    }
}
