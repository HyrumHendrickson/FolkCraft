//! Folk Craft - a minimal Minecraft-style voxel game.
//!
//! World column layout (y axis is up):
//!   y = 0        stone (unbreakable, nothing below it)
//!   y = 1..=5    dirt  (cannot be broken), up to 5 blocks more on hills, fewer in valleys and river beds
//!   above        air, trees (wood needs a stone axe, leaves break by hand), and in
//!                clearings grass, flowers and bushes, rivers of shallow translucent water with clay, pebbles and stones on the bed, plus whatever the
//!                player placed (logs, ...) and the deer, quail and bears that wander about
//!
//! Module overview:
//!   config    - tunable constants           block     - block types
//!   rng       - hashing and seeded RNG      chunk     - chunk storage
//!   terrain   - hills, valleys, forest        river     - winding rivers
//!   world     - chunk streaming, edits, fire, cooking, collision, raycast
//!   mesh      - chunk meshing               culling   - which chunks to draw
//!   effects   - flames and meat over logs      animal    - deer, quail and bear behaviour
//!   arrow     - arrows in flight               animal_draw - animal and arrow models
//!   player    - input and physics           item      - item kinds and block drops
//!   inventory - slots, stacking, moving     dropped   - items on the ground
//!   recipe    - crafting recipes            hud       - hotbar, inventory screen, crafting UI
//!   help      - the how-to-play screen
//!   game      - a running session (mining, placing, fire, held item, pickup)
//!   save      - persistence                 app       - shared app state
//!   sky       - the sun and the clouds
//!   ui        - menu widgets                menu      - world list / create
//!   play      - in-game loop and rendering

mod animal;
mod animal_draw;
mod app;
mod arrow;
mod block;
mod chunk;
mod config;
mod culling;
mod dropped;
mod effects;
mod game;
mod help;
mod hud;
mod inventory;
mod item;
mod menu;
mod mesh;
mod play;
mod player;
mod recipe;
mod river;
mod rng;
mod save;
mod sky;
mod terrain;
mod ui;
mod world;

use macroquad::prelude::*;

use app::{App, Mode};

fn window_conf() -> Conf {
    Conf {
        window_title: "Folk Craft".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = App::new();
    loop {
        match app.mode {
            Mode::Worlds => menu::worlds_screen(&mut app),
            Mode::Create => menu::create_screen(&mut app),
            Mode::Help => help::help_screen(&mut app),
            Mode::Playing => play::play_screen(&mut app),
        }
        next_frame().await;
    }
}
