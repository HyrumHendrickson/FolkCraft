//! The Host Game and Join Game screens: how a game on the local network is opened, found and entered.

use macroquad::prelude::*;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::{App, LobbyField, Mode};
use crate::game::Game;
use crate::net::{self, connect, parse_address, Finder, Link, ToGuest, ToHost, PORT, PROTOCOL};
use crate::save::WorldInfo;
use crate::ui::*;

/// What the player is called until they choose: the account name of the computer, if it has one.
pub fn default_player_name() -> String {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok().map(|n| n.chars().take(16).collect::<String>()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Player".to_string())
}

/// A connection being made to a host in the background: the screen keeps drawing while it waits.
pub struct JoinTask {
    rx: Receiver<Result<Welcome, String>>,
    pub target: String,
    started: Instant,
}

pub struct Welcome {
    pub link: Link,
    pub id: u32,
    pub host_name: String,
    pub save: String,
    /// Whatever the host sent in the same breath as the welcome (what this player carried before), for `Game::replay_messages`.
    pub rest: Vec<Vec<u8>>,
}

impl JoinTask {
    /// Connect to `addr`, say hello as `name`, and wait (up to 20 seconds) for the host to send the world.
    pub fn start(addr: std::net::SocketAddr, name: String) -> JoinTask {
        let (tx, rx) = channel();
        thread::spawn(move || {
            let _ = tx.send(join(addr, &name));
        });
        JoinTask { rx, target: addr.to_string(), started: Instant::now() }
    }

    /// `None` while still trying.
    pub fn poll(&self) -> Option<Result<Welcome, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("The connection attempt was lost.".to_string())),
        }
    }

    pub fn seconds(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }
}

/// The whole handshake, for the background thread (and the tests).
pub fn join(addr: std::net::SocketAddr, name: &str) -> Result<Welcome, String> {
    let link = connect(addr).map_err(|e| format!("Could not reach {addr}: {e}"))?;
    link.send(ToHost::Hello { protocol: PROTOCOL, name: name.to_string() }.encode());
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let mut batch = link.poll().into_iter();
        while let Some(bytes) = batch.next() {
            match ToGuest::decode(&bytes) {
                Some(ToGuest::Welcome { protocol, id, host_name, save }) if protocol == PROTOCOL => {
                    return Ok(Welcome { link, id, host_name, save, rest: batch.collect() });
                }
                Some(ToGuest::Welcome { .. }) => return Err("That game uses a different version of multiplayer.".to_string()),
                Some(ToGuest::Refused(why)) => return Err(why),
                _ => {}
            }
        }
        if !link.alive() {
            return Err("The host closed the connection.".to_string());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("The host did not answer.".to_string())
}

/// Type into `target` (up to `max` characters) and delete with backspace, with key repeat.
fn type_into(target: &mut String, max: usize, hold: &mut f32, accept: impl Fn(char) -> bool) {
    while let Some(c) = get_char_pressed() {
        if !c.is_control() && accept(c) && target.chars().count() < max {
            target.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        target.pop();
        *hold = -0.4;
    } else if is_key_down(KeyCode::Backspace) {
        *hold += get_frame_time();
        if *hold > 0.05 {
            target.pop();
            *hold = 0.0;
        }
    }
}

fn name_field(app: &mut App, rect: Rect) {
    draw_text("Your name", rect.x, rect.y - 8.0, 24.0, LIGHTGRAY);
    if is_mouse_button_pressed(MouseButton::Left) && mouse_in(rect) {
        app.lobby_field = LobbyField::Name;
    }
    text_field(rect, &app.player_name, app.lobby_field == LobbyField::Name);
}

/// The host screen: pick the world to open to the network. The world is the host's own save, played as usual; guests come and go.
pub fn host_screen(app: &mut App) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    text_centered("Host a Game", w / 2.0, 80.0, 56, ACCENT);
    text_centered("Friends on the same wifi can then join from their own copy of Folk Craft.", w / 2.0, 118.0, 24, LIGHTGRAY);
    let fw = 420.0_f32.min(w - 60.0);
    let name_rect = Rect::new((w - fw) / 2.0, 170.0, fw, 48.0);
    app.lobby_field = LobbyField::Name;
    {
        let mut hold = app.backspace_hold;
        type_into(&mut app.player_name, 16, &mut hold, |_| true);
        app.backspace_hold = hold;
    }
    name_field(app, name_rect);

    let (list_top, list_bottom) = (250.0, h - 110.0);
    let row_h = 56.0;
    let row_w = (w - 80.0).min(700.0);
    let row_x = (w - row_w) / 2.0;
    let content_h = app.worlds.len() as f32 * (row_h + 8.0);
    let max_scroll = (content_h - (list_bottom - list_top)).max(0.0);
    app.scroll = (app.scroll - wheel_notches() * 30.0).clamp(0.0, max_scroll);
    text_centered("Choose a world to host", w / 2.0, list_top - 12.0, 24, WHITE);
    if app.worlds.is_empty() {
        text_centered("No worlds yet. Go back and create one first.", w / 2.0, list_top + 60.0, 26, GRAY);
    }
    let mut chosen: Option<WorldInfo> = None;
    for (i, info) in app.worlds.iter().enumerate() {
        let y = list_top + i as f32 * (row_h + 8.0) - app.scroll;
        if y < list_top - row_h || y > list_bottom {
            continue;
        }
        let r = Rect::new(row_x, y, row_w, row_h);
        let hover = crate::menu::my_in_list(list_top, list_bottom) && mouse_in(r);
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { PANEL_HOVER } else { PANEL });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { ACCENT } else { DARKGRAY });
        draw_text(&info.name, r.x + 16.0, r.y + 34.0, 30.0, WHITE);
        if hover {
            let label = "Click to host";
            let d = measure_text(label, None, 24, 1.0);
            draw_text(label, r.x + r.w - d.width - 16.0, r.y + 34.0, 24.0, ACCENT);
            if is_mouse_button_pressed(MouseButton::Left) {
                chosen = Some(info.clone());
            }
        }
    }
    draw_rectangle(0.0, 0.0, w, list_top - 40.0 + 0.0, BG);
    draw_rectangle(0.0, list_bottom + 2.0, w, h - list_bottom, BG);
    text_centered("Host a Game", w / 2.0, 80.0, 56, ACCENT);
    text_centered("Friends on the same wifi can then join from their own copy of Folk Craft.", w / 2.0, 118.0, 24, LIGHTGRAY);
    name_field(app, name_rect);
    text_centered("Choose a world to host", w / 2.0, list_top - 12.0, 24, WHITE);
    if !app.error.is_empty() {
        text_centered(&app.error, w / 2.0, 150.0, 22, Color::new(0.95, 0.4, 0.4, 1.0));
    }
    if button(Rect::new(w / 2.0 - 110.0, h - 90.0, 220.0, 56.0), "Back", true) || is_key_pressed(KeyCode::Escape) {
        app.error.clear();
        app.mode = Mode::Worlds;
        return;
    }
    if let Some(info) = chosen {
        let name = app.player_name.trim().to_string();
        app.error.clear();
        app.enter_game(info);
        let started = match app.game.as_mut() {
            Some(game) => game.start_hosting(&name, PORT).or_else(|_| game.start_hosting(&name, 0)),
            None => return,
        };
        match started {
            Ok(port) => {
                let ip = net::local_ip().map_or("your computer's address".to_string(), |ip| ip.to_string());
                if let Some(game) = app.game.as_mut() {
                    game.toasts.push(format!("Hosting on {ip} (port {port}). Friends on the same wifi can join now."));
                }
            }
            Err(e) => {
                app.leave_game();
                app.error = format!("Could not open the game to the network: {e}");
                app.mode = Mode::Host;
            }
        }
    }
}

/// The join screen: games announced on the network are listed to click; an address can also be typed.
pub fn join_screen(app: &mut App) {
    if app.finder.is_none() {
        app.finder = Some(Finder::start());
    }
    if let Some(finder) = app.finder.as_mut() {
        finder.poll();
    }
    // A connection in progress.
    let mut finished = None;
    if let Some(task) = &app.join_task {
        if let Some(result) = task.poll() {
            finished = Some(result);
        } else if task.seconds() > 25.0 {
            finished = Some(Err("The host did not answer.".to_string()));
        }
    }
    if let Some(result) = finished {
        app.join_task = None;
        match result {
            Ok(welcome) => {
                let mut game = Game::new_guest(welcome.id, &app.player_name, welcome.host_name, &welcome.save, welcome.link);
                game.replay_messages(welcome.rest);
                app.finder = None;
                app.lobby_status.clear();
                app.enter_built_game(game);
                return;
            }
            Err(why) => app.lobby_status = why,
        }
    }
    let connecting = app.join_task.is_some();

    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    text_centered("Join a Game", w / 2.0, 80.0, 56, ACCENT);
    text_centered("Both of you need to be on the same wifi, with the same version of Folk Craft.", w / 2.0, 118.0, 24, LIGHTGRAY);
    let fw = 420.0_f32.min(w - 60.0);
    let name_rect = Rect::new((w - fw) / 2.0, 170.0, fw, 48.0);

    // Typing goes to whichever box was last clicked.
    if !connecting {
        let mut hold = app.backspace_hold;
        match app.lobby_field {
            LobbyField::Name => type_into(&mut app.player_name, 16, &mut hold, |_| true),
            LobbyField::Address => type_into(&mut app.join_address, 60, &mut hold, |c| c.is_ascii_alphanumeric() || ".:-_".contains(c)),
        }
        app.backspace_hold = hold;
        if is_key_pressed(KeyCode::Tab) {
            app.lobby_field = if app.lobby_field == LobbyField::Name { LobbyField::Address } else { LobbyField::Name };
        }
    }
    name_field(app, name_rect);

    // Games heard on the network.
    let list_top = 250.0;
    text_centered("Games on this network", w / 2.0, list_top - 12.0, 24, WHITE);
    let row_w = (w - 80.0).min(700.0);
    let row_x = (w - row_w) / 2.0;
    let games = app.finder.as_ref().map(|f| f.games.clone()).unwrap_or_default();
    if games.is_empty() {
        text_centered("Looking for games... (or type the host's address below)", w / 2.0, list_top + 40.0, 24, GRAY);
    }
    let mut chosen = None;
    for (i, g) in games.iter().take(5).enumerate() {
        let r = Rect::new(row_x, list_top + i as f32 * 62.0, row_w, 54.0);
        let hover = !connecting && mouse_in(r);
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { PANEL_HOVER } else { PANEL });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { ACCENT } else { DARKGRAY });
        draw_text(&format!("{}: {}", g.host_name, g.world_name), r.x + 16.0, r.y + 26.0, 28.0, WHITE);
        draw_text(&g.addr.to_string(), r.x + 16.0, r.y + 46.0, 18.0, LIGHTGRAY);
        if hover && is_mouse_button_pressed(MouseButton::Left) {
            chosen = Some(g.addr);
        }
    }

    // Typing an address.
    let addr_y = (list_top + 5.0 * 62.0 + 40.0).min(h - 230.0);
    let addr_rect = Rect::new(row_x, addr_y, row_w - 170.0, 48.0);
    draw_text("Or type the host's address, like 192.168.1.20", addr_rect.x, addr_rect.y - 8.0, 22.0, LIGHTGRAY);
    if is_mouse_button_pressed(MouseButton::Left) && mouse_in(addr_rect) && !connecting {
        app.lobby_field = LobbyField::Address;
    }
    text_field(addr_rect, &app.join_address, app.lobby_field == LobbyField::Address);
    let parsed = parse_address(&app.join_address);
    let join_clicked = button(Rect::new(addr_rect.x + addr_rect.w + 12.0, addr_rect.y, 158.0, 48.0), "Join", parsed.is_some() && !connecting);

    if connecting {
        let dots = ".".repeat(1 + (get_time() * 2.0) as usize % 3);
        let target = app.join_task.as_ref().map_or(String::new(), |t| t.target.clone());
        text_centered(&format!("Connecting to {target}{dots}"), w / 2.0, h - 126.0, 26, ACCENT);
    } else if !app.lobby_status.is_empty() {
        text_centered(&app.lobby_status, w / 2.0, h - 126.0, 24, Color::new(0.95, 0.4, 0.4, 1.0));
    }
    if button(Rect::new(w / 2.0 - 110.0, h - 90.0, 220.0, 56.0), if connecting { "Cancel" } else { "Back" }, true) || is_key_pressed(KeyCode::Escape) {
        if connecting {
            app.join_task = None;
            app.lobby_status.clear();
        } else {
            app.finder = None;
            app.lobby_status.clear();
            app.mode = Mode::Worlds;
        }
        return;
    }
    let target = chosen.or_else(|| if join_clicked || (is_key_pressed(KeyCode::Enter) && app.lobby_field == LobbyField::Address) { parsed } else { None });
    if let Some(addr) = target {
        if !connecting {
            app.lobby_status.clear();
            app.join_task = Some(JoinTask::start(addr, app.player_name.trim().to_string()));
        }
    }
}
