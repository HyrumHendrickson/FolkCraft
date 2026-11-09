//! The in-game screen: input, simulation, rendering and HUD.

use macroquad::prelude::*;

use crate::app::{App, Mode};
use crate::config::*;
use crate::animal_draw::{draw_animals, draw_arrows, draw_ghosts};
use crate::culling::chunks_to_draw;
use crate::dropped::draw_dropped_items;
use crate::effects::{draw_acid, draw_bursts, draw_falling_stars, draw_projectiles, draw_cyclone, draw_fire_tornado, draw_flames, draw_meat, draw_pots, draw_rainbow, draw_storm_lightning, draw_storm_rain, draw_trail, lightning_flash_alpha};
use crate::game::Game;
use crate::item::ItemKind;
use crate::hud::{draw_compass, CompassTarget, draw_hotbar, inventory_screen};
use crate::player::MoveInput;
use crate::sky::{draw_horizen_sky, draw_sky};
use crate::world::{Hit, World};

type Target = Option<Hit>;

pub fn play_screen(app: &mut App) {
    // Escape closes the inventory if it is open; otherwise it saves and returns to the menu.
    if is_key_pressed(KeyCode::Escape) {
        match app.game.as_mut() {
            Some(game) if game.king_dialog.is_some() => game.king_dialog = None,
            Some(game) if game.shop_open => game.shop_open = false,
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

    // F1 opens the help without leaving the game.
    if is_key_pressed(KeyCode::F1) {
        if game.inventory_open {
            set_inventory_open(game, false);
        }
        app.open_help_in_game();
        return;
    }

    // F2 opens the settings without leaving the game.
    if is_key_pressed(KeyCode::F2) {
        if game.inventory_open {
            set_inventory_open(game, false);
        }
        app.open_settings_in_game();
        return;
    }

    if is_key_pressed(KeyCode::E) && game.king_dialog.is_none() && !game.shop_open {
        let open = !game.inventory_open;
        set_inventory_open(game, open);
    }

    sync_dialog_cursor(game);
    handle_look(game, dt);
    handle_hotbar_selection(game);

    game.world.stream_chunks(game.player.pos);
    game.world.rebuild_dirty(game.player.pos, MESH_BUDGET_PER_FRAME);
    let input = if game.ui_open() || game.horizen_transition.is_some() { MoveInput::default() } else { MoveInput::from_keyboard() };
    game.apply_worn_effects();
    game.player.update(&game.world, dt, &input);
    game.update_landmarks();
    game.update_idol();

    let eye = game.player.eye();
    let dir = game.player.look_dir();
    let target = if game.ui_open() { None } else { game.world.raycast_hit(eye, dir, REACH) };
    // With a stone axe or bronze mallet in hand, an animal under the crosshair is attacked rather than whatever
    // stands behind it.
    let melee = if game.ui_open() { None } else { game.melee_target(eye, dir) };
    let rod_strike = if game.ui_open() { None } else { game.holly_rod_strike_point(eye, dir) };
    handle_attacking(game, eye, dir, melee.is_some(), rod_strike);
    handle_breaking(game, dt, if melee.is_some() { None } else { target });
    handle_placing(game, eye, dir, target);
    game.update_items(dt);
    game.burn_fires(dt);
    game.update_king_dungeon();
    game.update_animals(dt);
    game.update_travel(dt);
    game.update_storm(dt);
    game.update_arrows(dt);
    game.update_health(dt);
    game.update_blackout(dt);
    game.update_effects(dt);
    game.update_horizen_transition(dt);

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

/// While the king's pop-up is open the mouse cursor is released, as it is for the inventory, and taken again when it closes.
fn sync_dialog_cursor(game: &mut Game) {
    let open = game.king_dialog.is_some() || game.shop_open;
    if open != game.dialog_cursor_free {
        game.dialog_cursor_free = open;
        if !game.inventory_open {
            set_cursor_grab(!open);
            show_mouse(open);
        }
        game.skip_look_frames = 3;
    }
}

/// Mouse look plus arrow-key look. Ignored while the inventory is open.
fn handle_look(game: &mut Game, dt: f32) {
    if game.ui_open() {
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
    if game.ui_open() {
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
    let wheel = crate::ui::wheel_notches();
    if wheel != 0.0 {
        game.scroll_selection(if wheel < 0.0 { 1 } else { -1 });
    }
}

/// Left click with a bow shoots an arrow along the crosshair, and with a sling a pebble. With any axe or a bronze
/// mallet, holding the button strikes the animal under the crosshair (`aimed`), once per swing cooldown
/// (`ItemKind::melee_cooldown`). The rules live in `Game::attack`.
fn handle_attacking(game: &mut Game, eye: Vec3, dir: Vec3, aimed: bool, rod_strike: Option<Vec3>) {
    if game.ui_open() {
        return;
    }
    let (clicked, held_down) = (is_mouse_button_pressed(MouseButton::Left), is_mouse_button_down(MouseButton::Left));
    if !game.attack(eye, dir, clicked, held_down, aimed) && clicked {
        game.use_holly_rod_on_target(rod_strike);
    }
}

/// Hold the left mouse button on a block until it breaks (`Block::break_seconds`). What can be
/// broken depends on the held item (hands for leaves, plants and placed items, a stone
/// axe for wood); breaking may drop items. With a bow drill in hand, a placed log is set on
/// fire instead.
fn handle_breaking(game: &mut Game, dt: f32, target: Target) {
    // A Holly Rod is a storm tool, not a mining tool. This also lets a held rod hit a block without starting a
    // second, unrelated break action on the same click.
    if game.held_item() == Some(ItemKind::HollyRod) {
        game.mining = None;
        return;
    }
    let holding = !game.ui_open() && is_mouse_button_down(MouseButton::Left);
    game.update_mining(target, holding, dt);
}

/// Right click eats held food when hurt, places the held log, pebble, stone or plank, or works a
/// fire (add a log, lay raw meat on it, take cooked meat off it).
fn handle_placing(game: &mut Game, eye: Vec3, dir: Vec3, target: Target) {
    if game.ui_open() || !is_mouse_button_pressed(MouseButton::Right) {
        return;
    }
    if game.fill_jar(eye, dir) {
        return;
    }
    // Ash in hand at a fairy: trade it for a vial of moonlight.
    if game.trade_with_fairy(eye, dir) {
        return;
    }
    // Right click at the store's ghost opens his shop.
    if game.open_shop(eye, dir) {
        return;
    }
    // Pipes in hand: right click plays a tune.
    if game.play_pipes() {
        return;
    }
    // The Horn of the Hunt in hand: right click blows it (it calls the Wild Hunt in Stonehenge).
    if game.blow_horn() {
        return;
    }
    // A bush is an explicit harvest target, even when the player happens to be holding food.
    // This keeps right-clicking a bush from eating the held berries instead.
    if let Some(hit) = target {
        if hit.block.is_pickable() {
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
    if game.wear_held() {
        return;
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
    let camera = |position: Vec3| Camera3D {
        position,
        target: position + dir,
        up: vec3(0.0, 1.0, 0.0),
        fovy: 70.0_f32.to_radians(),
        ..Default::default()
    };
    set_camera(&camera(eye));
    // The sun and the clouds go first, so the land is drawn over them.
    if game.in_horizen() {
        draw_horizen_sky(game.world.seed, eye, get_time() as f32, game.world.night);
    } else {
        draw_sky(game.world.seed, eye, get_time() as f32, game.world.night, game.storm_active);
    }
    // Only chunks within the render radius (and inside the view cone) are drawn.
    let aspect = screen_width() / screen_height();
    let half_fov = ((35.0_f32).to_radians().tan() * (1.0 + aspect * aspect).sqrt()).atan();
    // The Skylands show no ground. Only while the screen is white from a fall off an island does the normal world appear
    // far below, seen from above the summit: its camera is the player's, carried over there.
    if let Some((over, from)) = game.overworld_backdrop(eye) {
        set_camera(&camera(from));
        for key in &chunks_to_draw(over, from, dir, half_fov) {
            for mesh in &over.chunks[key].meshes {
                draw_mesh(mesh);
            }
        }
        set_camera(&camera(eye));
    }
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
    for tornado in game.tornadoes.iter().chain(game.crater_tornado.iter()) {
        draw_fire_tornado(tornado, get_time() as f32, World::chunk_of_pos(eye));
    }
    if let Some(cyclone) = game.cyclone() {
        draw_cyclone(&cyclone, get_time() as f32, World::chunk_of_pos(eye));
    }
    draw_animals(&game.animals, get_time() as f32, World::chunk_of_pos(eye), game.world.night);
    draw_arrows(&game.arrows);
    draw_arrows(&game.hostile_arrows);
    draw_projectiles(&game.projectiles, get_time() as f32);
    // The water goes on last: it is translucent and the bed, the items and the animals behind it show through.
    for key in &drawn {
        for mesh in &game.world.chunks[key].water {
            draw_mesh(mesh);
        }
    }
    draw_acid(&game.world, &game.acid, get_time() as f32);
    draw_ghosts(&game.animals, get_time() as f32, World::chunk_of_pos(eye));
    if let Some(post) = game.shop_post() {
        crate::effects::draw_shopkeeper(post, eye, get_time() as f32);
    }
    draw_bursts(&game.bursts);
    draw_falling_stars(&game.falling_stars, get_time() as f32);
    if game.storm_active {
        draw_storm_rain(eye, get_time() as f32);
        if let Some((pos, flash)) = game.storm_lightning() {
            draw_storm_lightning(eye, pos, get_time() as f32, flash);
        }
    }
    // The rainbow shows only by day, to a peep stone in the hand, and over everything, water included.
    if let Some(rainbow) = game.visible_rainbow() {
        draw_rainbow(&game.world, &rainbow, get_time() as f32);
    }
    // A peep stone in the hand also shows a ghostly trail: to the grand castle by night, to the end of the rainbow by day.
    if let Some(path) = game.trail_path() {
        draw_trail(&game.world, &path, game.trail_color(), get_time() as f32, game.player.pos.y);
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
    // Apply the night reduction after every 3D layer, including animals, water, fires and the moving tornado, while
    // leaving the HUD readable.
    let scene_brightness = if game.world.night {
        NIGHT_SCENE_BRIGHTNESS
    } else if game.storm_active {
        WHISP_STORM_SCENE_BRIGHTNESS
    } else {
        1.0
    };
    if scene_brightness < 1.0 {
        set_default_camera();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 1.0 - scene_brightness));
    }
    // A strike lights up the whole sky for a moment, after the storm's darkening so that it is not dimmed by it.
    if let Some((pos, flash)) = game.storm_lightning() {
        let alpha = lightning_flash_alpha(flash, (pos - eye).length());
        if alpha > 0.0 {
            set_default_camera();
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.92, 0.95, 1.0, alpha));
        }
    }
    drawn.len()
}

fn render_hud(game: &mut Game, drawn: usize) {
    set_default_camera();
    let (w, h) = (screen_width(), screen_height());
    if game.inventory_open {
        if let Some(slot) = inventory_screen(&mut game.inventory, &mut game.worn, &mut game.inventory_ui, game.selected, &mut game.toasts, game.dev_mode) {
            game.drop_inventory_stack(slot);
        }
    } else {
        draw_line(w / 2.0 - 10.0, h / 2.0, w / 2.0 + 10.0, h / 2.0, 2.0, WHITE);
        draw_line(w / 2.0, h / 2.0 - 10.0, w / 2.0, h / 2.0 + 10.0, 2.0, WHITE);
        draw_hotbar(&game.inventory, game.selected);
        if let Some(progress) = game.mining_progress() {
            draw_break_bar(progress);
        }
        draw_health(game.health, game.max_health());
        if game.held_item() == Some(ItemKind::Compass) {
            let target = if !game.in_overworld() {
                CompassTarget::Spawn
            } else {
                CompassTarget::Mountain
            };
            draw_compass(game.player.yaw, game.compass_bearing(), target);
        }
    }
    if game.king_dialog.is_some() {
        draw_king_dialog(game);
    }
    if game.shop_open {
        draw_shop_dialog(game);
    }
    game.toasts.draw();
    let p = game.player.pos;
    draw_text(
        &format!("{}   XYZ {:.1} {:.1} {:.1}   Chunk {:?}   Drawing {}/{} chunks   FPS {}", game.info.name, p.x, p.y, p.z,
            World::chunk_of_pos(p), drawn, game.world.chunks.len(), get_fps()),
        10.0, 24.0, 22.0, WHITE,
    );
    draw_text(
        "WASD move | Mouse look | Space jump | Shift sprint | Left break/attack/shoot | Right place/eat/cook | 1-9/wheel hotbar | E inventory | F1 help | F2 settings | Esc save & exit",
        10.0, h - 12.0, 18.0, WHITE,
    );
    if let Some((name, share)) = game.boss_bar() {
        draw_boss_bar(name, share);
    }
    if let Some((text, left)) = game.ghost_text.as_ref() {
        draw_ghost_text(text, *left);
    }
    if let Some(left) = game.victory {
        draw_victory(left);
    }
    // Between day and night, or while passing through the well, the whole screen goes black for a moment.
    let darkness = game.blackout.map_or(0.0, |b| b.darkness()).max(game.horizen_transition_darkness());
    if darkness > 0.0 {
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, darkness));
    }
    // A cyclone taking hold, arriving by one, or falling from an island of the Skylands: the whole screen goes white.
    let white = game.white_alpha();
    if white > 0.0 {
        draw_rectangle(0.0, 0.0, w, h, Color::new(1.0, 1.0, 1.0, white));
    }
}

/// A boss's name and health bar across the top of the screen.
/// The pop-up in which the king's ghost is given gold: choose how many offerings of `GOLD_PER_SPIRIT` gold (one spirit archer each) with
/// the minus and plus buttons, then give them or leave. The rules live in `Game::give_gold_to_king`.
fn draw_king_dialog(game: &mut Game) {
    use crate::ui::{button, small_button, text_centered, ACCENT};
    let Some(offerings) = game.king_dialog else { return };
    let (w, h) = (screen_width(), screen_height());
    let panel = Rect::new(w / 2.0 - 280.0, h / 2.0 - 170.0, 560.0, 340.0);
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.45));
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, Color::new(0.07, 0.09, 0.15, 0.96));
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 3.0, Color::new(0.62, 0.78, 1.0, 1.0));
    let cx = panel.x + panel.w / 2.0;
    text_centered("The King Eternal", cx, panel.y + 48.0, 40, Color::new(0.80, 0.90, 1.0, 1.0));
    text_centered("Give me gold and I shall send my spirits to guard you.", cx, panel.y + 86.0, 22, WHITE);
    text_centered(&format!("Each {GOLD_PER_SPIRIT} gold sends one spirit archer. It shoots {SPIRIT_SHOTS} arrows at what you hit, then fades."), cx, panel.y + 114.0, 19, Color::new(0.75, 0.80, 0.90, 1.0));
    let gold = game.inventory.count(ItemKind::Gold);
    text_centered(&format!("You have {gold} gold."), cx, panel.y + 150.0, 24, ItemKind::Gold.color());
    let ceiling = (gold / GOLD_PER_SPIRIT).min(MAX_SPIRIT_ARCHERS.saturating_sub(game.spirit_count()) as u32).max(1);
    let row = panel.y + 200.0;
    if small_button(Rect::new(cx - 190.0, row - 28.0, 44.0, 44.0), "-", ACCENT) {
        game.king_dialog = Some(offerings.saturating_sub(1).max(1));
    }
    text_centered(&format!("{offerings} for {} gold", offerings * GOLD_PER_SPIRIT), cx, row + 2.0, 30, WHITE);
    if small_button(Rect::new(cx + 146.0, row - 28.0, 44.0, 44.0), "+", ACCENT) {
        game.king_dialog = Some((offerings + 1).min(ceiling));
    }
    if button(Rect::new(cx - 230.0, panel.y + 250.0, 210.0, 56.0), "Give", gold >= GOLD_PER_SPIRIT) {
        if game.give_gold_to_king(offerings) > 0 {
            game.king_dialog = None;
        }
    }
    if button(Rect::new(cx + 20.0, panel.y + 250.0, 210.0, 56.0), "Leave", true) {
        game.king_dialog = None;
    }
}

/// The general store's pop-up: each ware with its price and a Buy button (greyed out when the player has too little gold). The rules
/// live in `Game::buy`.
fn draw_shop_dialog(game: &mut Game) {
    use crate::ui::{button, text_centered};
    let wares = crate::shop::wares();
    let (w, h) = (screen_width(), screen_height());
    let row_h = 46.0;
    let panel = Rect::new(w / 2.0 - 300.0, h / 2.0 - (wares.len() as f32 * row_h + 150.0) / 2.0, 600.0, wares.len() as f32 * row_h + 150.0);
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.45));
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, Color::new(0.07, 0.09, 0.15, 0.96));
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 3.0, Color::new(0.62, 0.78, 1.0, 1.0));
    let cx = panel.x + panel.w / 2.0;
    text_centered("The General Store", cx, panel.y + 44.0, 38, Color::new(0.80, 0.90, 1.0, 1.0));
    let gold = game.inventory.count(ItemKind::Gold);
    text_centered(&format!("You have {gold} gold."), cx, panel.y + 76.0, 22, ItemKind::Gold.color());
    for (i, ware) in wares.iter().enumerate() {
        let y = panel.y + 100.0 + i as f32 * row_h;
        let label = if ware.count > 1 { format!("{} {}s", ware.count, ware.item.name()) } else { ware.item.name().to_string() };
        draw_text(&label, panel.x + 30.0, y + 28.0, 26.0, WHITE);
        draw_text(&format!("{} gold", ware.price), panel.x + 330.0, y + 28.0, 24.0, ItemKind::Gold.color());
        if button(Rect::new(panel.x + panel.w - 150.0, y, 120.0, row_h - 8.0), "Buy", gold >= ware.price) {
            game.buy(i);
        }
    }
    if button(Rect::new(cx - 90.0, panel.y + panel.h - 56.0, 180.0, 40.0), "Leave", true) {
        game.shop_open = false;
    }
}

/// A ghost's words, as pale text in the upper part of the screen (ghosts are not seen): they fade in, and out over the last second.
fn draw_ghost_text(text: &str, left: f32) {
    let (w, h) = (screen_width(), screen_height());
    let alpha = ((GHOST_TEXT_SECONDS - left) * 2.0).min(left).clamp(0.0, 1.0);
    let lines = crate::ghosts::wrap(text, 52);
    let size = 30.0;
    let top = h * 0.24;
    for (i, line) in lines.iter().enumerate() {
        let width = measure_text(line, None, size as u16, 1.0).width;
        let (x, y) = ((w - width) / 2.0, top + i as f32 * (size + 6.0));
        draw_rectangle(x - 10.0, y - size + 4.0, width + 20.0, size + 6.0, Color::new(0.02, 0.04, 0.10, 0.45 * alpha));
        draw_text(line, x, y, size, Color::new(0.80, 0.90, 1.0, alpha));
    }
}

/// The victory message after the Wild Hunt is beaten: it fades in and, over the last two seconds, out again.
fn draw_victory(left: f32) {
    let (w, h) = (screen_width(), screen_height());
    let alpha = ((VICTORY_TEXT_SECONDS - left) * 1.0).min(left / 2.0).clamp(0.0, 1.0);
    let lines = [("VICTORY", 72.0), ("You have beaten the Wild Hunt and won Folk Craft.", 30.0), ("You may keep playing.", 26.0)];
    let mut y = h * 0.30;
    for (line, size) in lines {
        let width = measure_text(line, None, size as u16, 1.0).width;
        let x = (w - width) / 2.0;
        draw_text(line, x + 2.0, y + 2.0, size, Color::new(0.0, 0.0, 0.0, 0.8 * alpha));
        draw_text(line, x, y, size, Color::new(1.0, 0.90, 0.45, alpha));
        y += size + 14.0;
    }
}

fn draw_boss_bar(name: &str, share: f32) {
    let w = screen_width();
    let (bar_w, bar_h) = ((w * 0.5).min(640.0), 18.0);
    let (x, y) = (w / 2.0 - bar_w / 2.0, 66.0);
    let size = measure_text(name, None, 26, 1.0);
    draw_text(name, w / 2.0 - size.width / 2.0 + 1.0, y - 7.0, 26.0, BLACK);
    draw_text(name, w / 2.0 - size.width / 2.0, y - 8.0, 26.0, Color::new(0.97, 0.94, 0.88, 1.0));
    draw_rectangle(x - 3.0, y - 3.0, bar_w + 6.0, bar_h + 6.0, Color::new(0.0, 0.0, 0.0, 0.75));
    draw_rectangle(x, y, bar_w, bar_h, Color::new(0.22, 0.05, 0.06, 1.0));
    draw_rectangle(x, y, bar_w * share, bar_h, Color::new(0.78, 0.12, 0.14, 1.0));
    draw_rectangle(x, y, bar_w * share, bar_h * 0.35, Color::new(0.95, 0.40, 0.38, 0.55));
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
fn draw_health(health: u32, max: u32) {
    let (w, h) = (screen_width(), screen_height());
    let (size, gap) = (18.0, 4.0);
    // Ten hearts a row; hearts from a Vial of Moonlight (past ten) make a second row above, in pale blue.
    let row = |n: u32| n.min(MAX_HEALTH);
    let total = row(max) as f32 * (size + gap) - gap;
    let (x0, y0) = (w / 2.0 - total / 2.0, h - 134.0);
    for i in 0..max {
        let (col, line) = (i % MAX_HEALTH, i / MAX_HEALTH);
        let (x, y) = (x0 + col as f32 * (size + gap), y0 - line as f32 * (size + gap));
        let full = if i < MAX_HEALTH { Color::new(0.85, 0.12, 0.18, 1.0) } else { Color::new(0.45, 0.62, 0.98, 1.0) };
        let color = if i < health { full } else { Color::new(0.2, 0.2, 0.2, 0.7) };
        draw_rectangle(x - 1.0, y - 1.0, size + 2.0, size + 2.0, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_rectangle(x, y, size, size, color);
    }
}
