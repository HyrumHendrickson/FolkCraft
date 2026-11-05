//! Top-level application state shared by the menu and play screens.

use macroquad::prelude::*;

use crate::game::Game;
use crate::save::{delete_world, list_worlds, WorldInfo};

#[derive(PartialEq)]
pub enum Mode {
    Worlds,
    Create,
    Help,
    Playing,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Field {
    Name,
    Seed,
}

pub struct App {
    pub mode: Mode,
    pub worlds: Vec<WorldInfo>,
    pub scroll: f32,
    pub name: String,
    pub seed: String,
    /// The game mode chosen on the create screen: `true` for dev mode.
    pub dev_mode: bool,
    pub field: Field,
    pub error: String,
    pub backspace_hold: f32,
    pub game: Option<Game>,
    /// The world the player has asked to delete and has yet to confirm.
    pub pending_delete: Option<WorldInfo>,
    /// How far (pixels) the help text is scrolled.
    pub help_scroll: f32,
}

impl App {
    pub fn new() -> Self {
        App {
            mode: Mode::Worlds,
            worlds: list_worlds(),
            scroll: 0.0,
            name: String::new(),
            seed: String::new(),
            dev_mode: false,
            field: Field::Name,
            error: String::new(),
            backspace_hold: 0.0,
            game: None,
            pending_delete: None,
            help_scroll: 0.0,
        }
    }

    /// Ask for confirmation before deleting `info`.
    pub fn ask_delete(&mut self, info: WorldInfo) {
        self.pending_delete = Some(info);
    }

    /// Give up on deleting the world that was being asked about.
    pub fn cancel_delete(&mut self) {
        self.pending_delete = None;
    }

    /// Delete the world that was being asked about and take it off the list. On failure the world stays
    /// listed and `error` says why. Returns whether a world was deleted.
    pub fn confirm_delete(&mut self) -> bool {
        let Some(info) = self.pending_delete.take() else { return false };
        match delete_world(&info) {
            Ok(()) => {
                self.worlds.retain(|w| w.path != info.path);
                self.scroll = 0.0;
                self.error.clear();
                true
            }
            Err(e) => {
                self.error = format!("Could not delete \"{}\": {e}", info.name);
                false
            }
        }
    }

    pub fn open_help(&mut self) {
        self.help_scroll = 0.0;
        self.mode = Mode::Help;
    }

    pub fn enter_game(&mut self, info: WorldInfo) {
        self.game = Some(Game::load(info));
        self.mode = Mode::Playing;
        set_cursor_grab(true);
        show_mouse(false);
        while get_char_pressed().is_some() {}
    }

    pub fn leave_game(&mut self) {
        if let Some(g) = self.game.take() {
            g.save();
        }
        set_cursor_grab(false);
        show_mouse(true);
        self.worlds = list_worlds();
        self.mode = Mode::Worlds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn world_in(dir: &std::path::Path, name: &str) -> WorldInfo {
        let path = dir.join(format!("{name}.world"));
        fs::write(&path, format!("{name}\n1\n0 6 0 0 0\n")).unwrap();
        WorldInfo { name: name.into(), seed: 1, path }
    }

    #[test]
    fn deleting_asks_first_and_then_removes_the_world_from_disk_and_list() {
        let dir = std::env::temp_dir().join(format!("folkcraft_app_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (a, b) = (world_in(&dir, "Alpha"), world_in(&dir, "Beta"));
        let mut app = App::new();
        app.worlds = vec![a.clone(), b.clone()];
        app.scroll = 40.0;

        assert!(!app.confirm_delete(), "nothing has been asked about");
        app.ask_delete(a.clone());
        assert!(a.path.exists(), "asking deletes nothing");
        app.cancel_delete();
        assert!(app.pending_delete.is_none() && !app.confirm_delete() && a.path.exists(), "cancelled");
        assert_eq!(app.worlds.len(), 2);

        app.ask_delete(a.clone());
        assert!(app.confirm_delete());
        assert!(!a.path.exists() && b.path.exists());
        assert_eq!(app.worlds.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), vec!["Beta"]);
        assert!(app.pending_delete.is_none() && app.error.is_empty());
        assert_eq!(app.scroll, 0.0);

        // A world that cannot be deleted stays listed and the reason is shown.
        fs::remove_file(&b.path).unwrap();
        app.ask_delete(b.clone());
        assert!(!app.confirm_delete());
        assert!(app.error.contains("Beta"), "{}", app.error);
        assert_eq!(app.worlds.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn opening_help_starts_at_the_top() {
        let mut app = App::new();
        app.help_scroll = 300.0;
        app.open_help();
        assert!(app.mode == Mode::Help && app.help_scroll == 0.0);
    }
}
