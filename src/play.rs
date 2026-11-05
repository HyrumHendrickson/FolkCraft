//! The in-game screen: input, simulation, rendering and HUD.

use macroquad::prelude::*;

use crate::app::{App, Mode};
use crate::config::*;
use crate::animal_draw::{draw_animals, draw_arrows, draw_ghosts};
use crate::culling::chunks_to_draw;
use crate::dropped::draw_dropped_items;
use crate::effects::{draw_flames, draw_meat, draw_pots, draw_rainbow};
use crate::game::Game;
use crate::item::ItemKind;
use crate::hud::{draw_compass, draw_hotbar, inventory_screen};
use crate::player::MoveInput;
use crate::sky::draw_sky;
use crate::world::{Hit, World};

type Target = Option<Hit>;

pub fn play_screen(app: &mut App) {
    // Escape closes the inventory if it is open; otherwise it saves and returns to the menu.
    if is_key_pressed(KeyCode::Escape) {
        match app.game.as_mut() {
            Some(game) if game.inventory_open => set_inventory_open(game, false),
            _ => app.leave_game(),
        }
        return;
    }
    let game = match app.game.as_mut() {
        Some(g) => g,
        None => {
            app.mode = Mode::Worlds;
            return;
        }
    };
    let dt = get_frame_time().min(0.05);

    if is_key_pressed(KeyCode::E) {
        let open = !game.inventory_open;
        set_inventory_open(game, open);
    }

    handle_look(game, dt);
    handle_hotbar_selection(game);

    game.world.stream_chunks(game.player.pos);
    game.world.rebuild_dirty(game.player.pos, MESH_BUDGET_PER_FRAME);
    let input = if game.inventory_open { MoveInput::default() } else { MoveInput::from_keyboard() };
    game.player.update(&game.world, dt, &input);
    game.update_landmarks();

    let eye = game.player.eye();
    let dir = game.player.look_dir();
    let target = if game.inventory_open { None } else { game.world.raycast_hit(eye, dir, REACH) };
    // With a stone axe or bronze mallet in hand, an animal under the crosshair is attacked rather than whatever
    // stands behind it.
    let melee = if game.inventory_open { None } else { game.melee_target(eye, dir) };
    handle_attacking(game, eye, dir, melee.is_some());
    handle_breaking(game, dt, if melee.is_some() { None } else { target });
    handle_placing(game, eye, dir, target);
    game.update_items(dt);
    game.burn_fires(dt);
    game.update_animals(dt);
    game.update_arrows(dt);
    game.update_health(dt);

    game.autosave_timer += dt;
    if game.autosave_timer >= AUTOSAVE_INTERVAL {
        game.autosave_timer = 0.0;
        game.save();
    }

    let drawn = render_world(game, eye, dir, target);
    render_hud(game, drawn);
}

/// Show or hide the inventory screen; the mouse cursor is released while it is open.
fn set_inventory_open(game: &mut Game, open: bool) {
    game.inventory_open = open;
    game.inventory_ui.cancel_drag();
    set_cursor_grab(!open);
    show_mouse(open);
    // Re-capturing the cursor can produce one large bogus mouse delta; ignore it.
    game.skip_look_frames = 3;
}

/// Mouse look plus arrow-key look. Ignored while the inventory is open.
fn handle_look(game: &mut Game, dt: f32) {
    if game.inventory_open {
        return;
    }
    if game.skip_look_frames > 0 {
        game.skip_look_frames -= 1;
    } else {
        let d = mouse_delta_position();
        let pitch_sign = if INVERT_LOOK_Y { 1.0 } else { -1.0 };
        game.player.yaw += -d.x * screen_width() * 0.5 * LOOK_SENSITIVITY;
        game.player.pitch += pitch_sign * d.y * screen_height() * 0.5 * LOOK_SENSITIVITY;
    }
    // Keyboard look works even when the laptop trackpad is suppressed while WASD is held.
    if is_key_down(KeyCode::Left) { game.player.yaw -= KEY_LOOK_SPEED * dt; }
    if is_key_down(KeyCode::Right) { game.player.yaw += KEY_LOOK_SPEED * dt; }
    if is_key_down(KeyCode::Up) { game.player.pitch += KEY_LOOK_SPEED * dt; }
    if is_key_down(KeyCode::Down) { game.player.pitch -= KEY_LOOK_SPEED * dt; }
    game.player.pitch = game.player.pitch.clamp(-1.5, 1.5);
}

/// Number keys 1-9 and the mouse wheel choose which hotbar slot is held.
/// Ignored while the inventory is open.
fn handle_hotbar_selection(game: &mut Game) {
    if game.inventory_open {
        return;
    }
    const KEYS: [KeyCode; HOTBAR_SLOTS] = [
        KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5,
        KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9,
    ];
    for (slot, key) in KEYS.iter().enumerate() {
        if is_key_pressed(*key) {
            game.select_slot(slot);
        }
    }
    // Wheel down moves right along the hotbar, like Minecraft.
    let wheel = mouse_wheel().1;
    if wheel != 0.0 {
        game.scroll_selection(if wheel < 0.0 { 1 } else { -1 });
    }
}

/// Left click with a bow shoots an arrow along the crosshair, and with a sling a pebble. With any axe or a bronze
/// mallet, holding the button strikes the animal under the crosshair (`aimed`), once per swing cooldown
/// (`ItemKind::melee_cooldown`). The rules live in `Game::attack`.
fn handle_attacking(game: &mut Game, eye: Vec3, dir: Vec3, aimed: bool) {
    if game.inventory_open {
        return;
    }
    let (clicked, held_down) = (is_mouse_button_pressed(MouseButton::Left), is_mouse_button_down(MouseButton::Left));
    game.attack(eye, dir, clicked, held_down, aimed);
}

/// Hold the left mouse button on a block until it breaks (`Block::break_seconds`). What can be
/// broken depends on the held item (hands for leaves, plants and placed items, a stone
/// axe for wood); breaking may drop items. With a bow drill in hand, a placed log is set on
/// fire instead.
fn handle_breaking(game: &mut Game, dt: f32, target: Target) {
    let holding = !game.inventory_open && is_mouse_button_down(MouseButton::Left);
    game.update_mining(target, holding, dt);
}

/// Right click eats held food when hurt, places the held log, pebble, stone or plank, or works a
/// fire (add a log, lay raw meat on it, take cooked meat off it).
fn handle_placing(game: &mut Game, eye: Vec3, dir: Vec3, target: Target) {
    if game.inventory_open || !is_mouse_button_pressed(MouseButton::Right) {
        return;
    }
    if game.fill_jar(eye, dir) {
        return;
    }
    // A bush is an explicit harvest target, even when the player happens to be holding food.
    // This keeps right-clicking a bush from eating the held berries instead.
    if let Some(hit) = target {
        if hit.block == crate::block::Block::Bush {
            game.use_held(&hit);
            return;
        }
        // Raw berries are valid pot ingredients. Let the pot consume them before the generic
        // food action gets a chance to eat them; this must work at any health level.
        if game.can_use_fire(&hit) {
            game.use_held(&hit);
            return;
        }
    }
    if game.eat_held() {
        return;
    }
    if let Some(hit) = target {
        game.use_held(&hit);
    }
}

/// Draw the 3D scene. Returns how many chunks were drawn.
fn render_world(game: &Game, eye: Vec3, dir: Vec3, target: Target) -> usize {
    clear_background(if game.world.night {
        Color::new(0.015, 0.025, 0.075, 1.0)
    } else {
        Color::new(0.53, 0.81, 0.92, 1.0)
    });
    set_camera(&Camera3D {
        position: eye,
        target: eye + dir,
        up: vec3(0.0, 1.0, 0.0),
        fovy: 70.0_f32.to_radians(),
        ..Default::default()
    });
    // The sun and the clouds go first, so the land is drawn over them.
    draw_sky(game.world.seed, eye, get_time() as f32, game.world.night);
    // Only chunks within the render radius (and inside the view cone) are drawn.
    let aspect = screen_width() / screen_height();
    let half_fov = ((35.0_f32).to_radians().tan() * (1.0 + aspect * aspect).sqrt()).atan();
    let drawn = chunks_to_draw(&game.world, eye, dir, half_fov);
    for key in &drawn {
        for mesh in &game.world.chunks[key].meshes {
            draw_mesh(mesh);
        }
    }
    draw_dropped_items(&game.items, get_time() as f32, World::chunk_of_pos(eye));
    draw_flames(&game.world, get_time() as f32, World::chunk_of_pos(eye));
    draw_meat(&game.world, World::chunk_of_pos(eye));
    draw_pots(&game.world, World::chunk_of_pos(eye));
    draw_animals(&game.animals, get_time() as f32, World::chunk_of_pos(eye));
    draw_arrows(&game.arrows);
    // The water goes on last: it is translucent and the bed, the items and the animals behind it show through.
    for key in &drawn {
        for mesh in &game.world.chunks[key].water {
            draw_mesh(mesh);
        }
    }
    draw_ghosts(&game.animals, get_time() as f32, World::chunk_of_pos(eye));
    // The rainbow shows only to a peep stone in the hand, and over everything, water included.
    if let Some(rainbow) = game.visible_rainbow() {
        draw_rainbow(&game.world, &rainbow, get_time() as f32);
    }
    if let Some(hit) = target {
        // Outline the block's actual shape, so a pebble gets a small box and a trunk a thin one.
        if let Some((lo, hi)) = hit.block.bounds() {
            let origin = vec3(hit.x as f32, hit.y as f32, hit.z as f32);
            let (lo, hi) = (origin + Vec3::from(lo), origin + Vec3::from(hi));
            let col = if game.can_interact(hit.block) || game.can_use_fire(&hit) { BLACK } else { Color::new(0.8, 0.1, 0.1, 1.0) };
            draw_cube_wires((lo + hi) / 2.0, hi - lo + Vec3::splat(0.004), col);
        }
    }
    drawn.len()
}

fn render_hud(game: &mut Game, drawn: usize) {
    set_default_camera();
    let (w, h) = (screen_width(), screen_height());
    if game.inventory_open {
        if let Some(slot) = inventory_screen(&mut game.inventory, &mut game.inventory_ui, game.selected, &mut game.toasts, game.dev_mode) {
            game.drop_inventory_stack(slot);
        }
    } else {
        draw_line(w / 2.0 - 10.0, h / 2.0, w / 2.0 + 10.0, h / 2.0, 2.0, WHITE);
        draw_line(w / 2.0, h / 2.0 - 10.0, w / 2.0, h / 2.0 + 10.0, 2.0, WHITE);
        draw_hotbar(&game.inventory, game.selected);
        if let Some(progress) = game.mining_progress() {
            draw_break_bar(progress);
        }
        draw_health(game.health);
        if game.held_item() == Some(ItemKind::Compass) {
            draw_compass(game.player.yaw, game.treks.compass_progress());
        }
    }
    game.toasts.draw();
    let p = game.player.pos;
    draw_text(
        &format!("{}   XYZ {:.1} {:.1} {:.1}   Chunk {:?}   Drawing {}/{} chunks   FPS {}", game.info.name, p.x, p.y, p.z,
            World::chunk_of_pos(p), drawn, game.world.chunks.len(), get_fps()),
        10.0, 24.0, 22.0, WHITE,
    );
    draw_text(
        "WASD move | Mouse look | Space jump | Shift sprint | Left break/attack/shoot | Right place/eat/cook | 1-9/wheel hotbar | E inventory | Esc save & exit",
        10.0, h - 12.0, 18.0, WHITE,
    );
}

/// A small progress bar under the crosshair while a block is being broken.
fn draw_break_bar(progress: f32) {
    let (w, h) = (screen_width(), screen_height());
    let (bar_w, bar_h) = (80.0, 8.0);
    let (x, y) = (w / 2.0 - bar_w / 2.0, h / 2.0 + 24.0);
    draw_rectangle(x - 2.0, y - 2.0, bar_w + 4.0, bar_h + 4.0, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, bar_w * progress, bar_h, Color::new(0.95, 0.85, 0.35, 1.0));
}

/// Ten hearts above the hotbar, one per life point.
fn draw_health(health: u32) {
    let (w, h) = (screen_width(), screen_height());
    let (size, gap) = (18.0, 4.0);
    let total = MAX_HEALTH as f32 * (size + gap) - gap;
    let (x0, y) = (w / 2.0 - total / 2.0, h - 134.0);
    for i in 0..MAX_HEALTH {
        let x = x0 + i as f32 * (size + gap);
        let color = if i < health { Color::new(0.85, 0.12, 0.18, 1.0) } else { Color::new(0.2, 0.2, 0.2, 0.7) };
        draw_rectangle(x - 1.0, y - 1.0, size + 2.0, size + 2.0, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_rectangle(x, y, size, size, color);
    }
}
