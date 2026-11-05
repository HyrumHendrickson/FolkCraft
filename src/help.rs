//! The help screen: how to play, and what can be made and how.
//!
//! The text is built by `help_lines` from the game's own tables and constants (the recipe list, the fire
//! and smelting times, the animals' health), so it cannot drift away from the rules. Drawing is a thin
//! wrapper (`help_screen`) over `layout`, which wraps the text to a width and is plain logic.

use macroquad::prelude::*;

use crate::app::{App, Mode};
use crate::config::*;
use crate::recipe::RECIPES;
use crate::ui::*;

/// One line of help text, before it is wrapped to the screen.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Heading(String),
    Text(String),
    /// A bulleted entry.
    Item(String),
}

fn heading(s: &str) -> Line {
    Line::Heading(s.to_string())
}

fn text(s: impl Into<String>) -> Line {
    Line::Text(s.into())
}

fn item(s: impl Into<String>) -> Line {
    Line::Item(s.into())
}

/// The whole help text, section by section.
pub fn help_lines() -> Vec<Line> {
    let mut out = vec![
        heading("Getting started"),
        text("Folk Craft is a survival sandbox. You start in a forest with nothing. Gather what lies about, make tools, light a fire, hunt, and smelt metal. Your world is saved automatically as you play, and again when you leave it."),
        heading("Controls"),
        item("W A S D: walk. Hold Shift to sprint. Space: jump."),
        item("Mouse (or the arrow keys): look around."),
        item("Left mouse: hold it on a block to break it, or click to swing an axe or shoot a bow. A bar shows how far along the break is."),
        item("Right mouse: place the item you hold, eat it, or work a fire (see below)."),
        item("1 to 9 or the mouse wheel: choose what you hold from the hotbar. E: open the inventory. Esc: close it, or save and return to the menu."),
        item("Inventory: drag a stack onto another slot to move or merge it. The panel on the right lists everything you can make right now; click one to make it."),
        item("Dev mode: when you create a world you can choose Dev mode instead of Normal. The game is the same, except that in the inventory you can scroll the list on the right past the recipes to a list of every item; click one to add it, right click to add a stack."),
        heading("Staying alive"),
        item(format!("You have {MAX_HEALTH} hearts. A fire burns you for {FIRE_DAMAGE} heart every second you touch it, and a bear that reaches you takes {BEAR_DAMAGE}.")),
        item("If you run out of hearts you start again at the beginning of the world. You keep your inventory."),
        item(format!(
            "Right click food to eat it when you are hurt: berries +{BERRY_HEAL}, cooked venison +{VENISON_HEAL}, cooked poultry +{POULTRY_HEAL}, cooked bear meat +{BEAR_MEAT_HEAL}. Raw meat cannot be eaten: cook it first."
        )),
        heading("The land"),
        item("The world is forest, with clearings full of grass, flowers and berry bushes, broad gentle hills and valleys, and winding rivers."),
        item("The ground cannot be dug. You can walk up any one-block step by jumping."),
        item("Rivers are one block deep: wade straight through them, and jump to climb out the other side. Look down at the river bed to see what lies on it."),
        heading("Gathering"),
        item("Leaves and pine needles: sometimes drop a stick. Break them by hand."),
        item("Grass (in clearings): sometimes drops grass. Berry bushes: drop berries."),
        item("Stones and pebbles lie on the ground everywhere, and on the river bed under the water. Hold left mouse on one to pick it up."),
        item(format!("A Rabbit Foot in your hotbar gives broken stones a {}% chance to drop a Loadstone, boulders a {}% chance to drop Gold Ore, pebbles a {}% chance to add a Peep Stone, and leaves or pine needles a {}% chance to add a Holly Rod. Peep Stones and Holly Rods do nothing yet. Gold Ore smelts like Iron Ore. A Loadstone and an Iron Ore make a Compass, which does nothing yet.", RABBIT_FOOT_LOADSTONE_CHANCE, RABBIT_FOOT_GOLD_CHANCE, RABBIT_FOOT_PEEP_STONE_CHANCE, RABBIT_FOOT_HOLLY_ROD_CHANCE)),
        item(format!("Clay lies on the river bed. Hold left mouse on a lump to dig {CLAY_DROP} clay out of it. Clay makes a clay forge.")),
        item(format!(
            "Boulders are big grey rocks, a little wider than a block, scattered on the ground (more on hills). Only a Bronze Mallet breaks one: hold left mouse on it for {} seconds. It drops a stone, a pebble and {BOULDER_IRON_MIN} to {BOULDER_IRON_MAX} iron ore.",
            BREAK_SECONDS_BOULDER as i32
        )),
        item("Tree trunks: you need a Stone Axe in your hand. Hold left mouse on a trunk for 5 seconds to get a log."),
        heading("Crafting"),
        text("Open the inventory with E. Recipes need only the right amounts of each item, with no pattern. These are all of them:"),
    ];
    for r in RECIPES {
        out.push(item(format!("{}: {}", r.output.name(), r.describe())));
    }
    out.push(text("A piece of leather makes three leather cords. Every axe except the stone one is bound with a leather cord, not twine. Wooden planks can no longer be made."));
    out.extend([
        heading("Fire and cooking"),
        item("Right click a log on the ground to put it down. Hold a Bow Drill and left click the log to light it. It burns for a minute and then leaves charcoal."),
        item(format!("Right click a burning log with another log or charcoal to add {} seconds.", FIRE_SECONDS as i32)),
        item(format!(
            "Right click a burning fire with raw meat to lay it on. It cooks in {} seconds. Right click again with your hand empty, or holding something that cannot cook, to take it off.",
            COOK_SECONDS as i32
        )),
        item("Charcoal can be put down and lit just like a log. It becomes embers, which burn for a minute and then leave ash. Do not stand in a fire or in embers."),
        heading("Smelting metal"),
        item(format!(
            "Copper and tin: put down charcoal, light it with the Bow Drill, and right click the burning charcoal while holding a stone (makes copper) or a pebble (makes tin). It smelts in {} seconds. Right click again with your hand empty to take the metal.",
            SMELT_SECONDS as i32
        )),
        item(format!(
            "Clay forge: craft one from stones and clay, put it down, and right click it with charcoal to fill it. Hold a Bow Drill and left click it to light it. It burns for {} seconds and is empty again afterwards. Break an empty or filled forge by hand to get {FORGE_CLAY_DROP} clay back.",
            FIRE_SECONDS as i32
        )),
        item(format!(
            "Bronze: right click a lit forge while holding a tin or a copper, with the other in your inventory. One of each is used, and the bronze is ready in {} seconds. No bellows are needed.",
            SMELT_SECONDS as i32
        )),
        item(format!(
            "Iron: a lit forge is not hot enough. Hold Bellows and left click the lit forge so it glows for {} seconds (click again to renew it), then right click it with iron ore. Iron only smelts while the forge glows.",
            BELLOWS_GLOW_SECONDS as i32
        )),
        text("Bronze, leather cords and a stick make a Bronze Mallet, which breaks boulders and also hurts animals. Pelts have no use yet."),
        heading("Water and stew"),
        item("A Clay Jar (4 clay) is the only way to carry water. Hold an empty jar, look at a river and right click it to fill the jar."),
        item("A Tin Pot (3 tin) is set on a burning log or burning charcoal in the open: right click the fire with the pot in hand. Right click the pot with a jar of water to fill it (you get the empty jar back)."),
        item(format!(
            "Then right click the pot with any raw meat (venison, poultry, bear, rabbit or moose). It makes a stew of that meat in {} seconds. Right click with your hand empty to take the stew out; right click an empty pot to take it off the fire.",
            STEW_SECONDS as i32
        )),
        item(format!("A stew restores {STEW_HEAL_FACTOR} times as much life as the same meat cooked: right click it to eat it when you are hurt. Cooked rabbit gives {RABBIT_MEAT_HEAL} and cooked moose {MOOSE_MEAT_HEAL}.")),
        heading("Animals and hunting"),
        item(format!(
            "Deer ({DEER_HEALTH} health) and quail ({QUAIL_HEALTH} health) run or fly away from you, and from bears. Deer are faster than you even when you sprint."
        )),
        item(format!(
            "Bears ({BEAR_HEALTH} health) are dangerous. Within {} blocks of you a bear wanders towards you. Within {} blocks it charges, and it keeps charging until you are {} blocks away. It runs a little slower than you sprint, so you can outrun it.",
            BEAR_NOTICE_DISTANCE as i32, BEAR_CHARGE_DISTANCE as i32, BEAR_CALM_DISTANCE as i32
        )),
        item(format!(
            "Only an axe (hold left mouse on the animal: a Stone Axe does {AXE_DAMAGE} damage a blow, a Copper Axe one more, a Bronze Axe two more, an Iron Axe three more), a Bronze Mallet ({MALLET_DAMAGE}), a Bow (left click to shoot an arrow from your inventory, {ARROW_DAMAGE} damage) or a Sling (left click to sling a pebble from your inventory, {SLING_DAMAGE} damage) hurts animals. A bear you hurt will charge you."
        )),
        item(format!(
            "A deer drops {DEER_VENISON_DROP} venison and {DEER_LEATHER_DROP} leather. A quail drops {QUAIL_POULTRY_DROP} poultry and {QUAIL_FEATHER_DROP} feathers. A bear drops {BEAR_MEAT_DROP} bear meat and {BEAR_PELT_DROP} pelt. A startled quail also drops a feather as it takes off."
        )),
        item(format!(
            "Rabbits ({RABBIT_HEALTH} health) run from you like deer. A rabbit drops {RABBIT_MEAT_DROP} rabbit meat, and one time in four a rabbit's foot, which has no use."
        )),
        item(format!(
            "Moose ({MOOSE_HEALTH} health) do not run. Leave one alone and it ignores you, but once you hurt it, it chases you until one of you is dead or you are {} blocks away, and hits for {MOOSE_DAMAGE}. A moose drops {MOOSE_MEAT_DROP} moose meat and {MOOSE_LEATHER_DROP} leather.",
            MOOSE_GIVE_UP_DISTANCE as i32
        )),
        item(format!(
            "Will-o-whisps appear at night. They float above the ground and flee at {WILL_O_WHISP_FLEE_SPEED} blocks per second when you come within {WILL_O_WHISP_FLEE_DISTANCE} blocks. They have no attacks or drops yet."
        )),
        item("A blow throws the animal back, and a blow that hurts you (or a burn) throws you back, so you can see when something lands."),
        item("An arrow that hits an animal is used up. An arrow that misses lands on the ground: walk over it to pick it up again. Feathers are what you need to make more."),
        item("A Sling is made of one leather and slings pebbles the way a bow shoots arrows: a pebble that hits an animal is used up, and one that misses lands on the ground to be picked up again."),
        heading("Tips"),
        item("Start with grass for twine, sticks from leaves, and a stone from the ground: that is a Stone Axe. Then logs, a Bow Drill, a fire, and food."),
        item("Keep an eye out for bears before you wander far from your fire, and keep your bow stocked with arrows or your sling with pebbles."),
        item("On the menu, the Delete button beside a world removes it for good, after asking you to confirm."),
    ]);
    out
}

/// What is drawn on one screen row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Heading,
    Text,
    /// The first row of a bulleted entry.
    Bullet,
    /// A later row of a bulleted entry, indented under its bullet.
    Continued,
}

/// A wrapped row of help: its text, kind and distance (pixels) from the top of the text.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub y: f32,
    pub kind: Kind,
    pub text: String,
}

pub const TEXT_SIZE: u16 = 22;
pub const HEADING_SIZE: u16 = 30;
pub const ROW_HEIGHT: f32 = 28.0;
pub const HEADING_HEIGHT: f32 = 40.0;
/// Space above a heading, and how far a bullet's text is indented.
pub const SECTION_GAP: f32 = 16.0;
pub const BULLET_INDENT: f32 = 26.0;

/// Break `text` into lines no wider than `max_width` (words are never split; a word wider than the
/// line gets a line of its own). `width` measures a string.
pub fn wrap(text: &str, max_width: f32, width: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let joined = if current.is_empty() { word.to_string() } else { format!("{current} {word}") };
        if current.is_empty() || width(&joined) <= max_width {
            current = joined;
        } else {
            lines.push(std::mem::replace(&mut current, word.to_string()));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Wrap the help to `max_width` pixels. Returns the rows and the total height of the text.
pub fn layout(lines: &[Line], max_width: f32, width: &dyn Fn(&str, u16) -> f32) -> (Vec<Row>, f32) {
    let mut rows = Vec::new();
    let mut y = 0.0;
    for line in lines {
        match line {
            Line::Heading(t) => {
                if y > 0.0 {
                    y += SECTION_GAP;
                }
                rows.push(Row { y, kind: Kind::Heading, text: t.clone() });
                y += HEADING_HEIGHT;
            }
            Line::Text(t) => {
                for part in wrap(t, max_width, &|s| width(s, TEXT_SIZE)) {
                    rows.push(Row { y, kind: Kind::Text, text: part });
                    y += ROW_HEIGHT;
                }
                y += 6.0;
            }
            Line::Item(t) => {
                for (i, part) in wrap(t, max_width - BULLET_INDENT, &|s| width(s, TEXT_SIZE)).into_iter().enumerate() {
                    rows.push(Row { y, kind: if i == 0 { Kind::Bullet } else { Kind::Continued }, text: part });
                    y += ROW_HEIGHT;
                }
                y += 6.0;
            }
        }
    }
    (rows, y)
}

/// The help screen: the text scrolls with the mouse wheel, the arrow keys and Page Up / Page Down.
pub fn help_screen(app: &mut App) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    let (top, bottom) = (110.0, h - 100.0);
    let content_w = (w - 80.0).min(860.0);
    let x = (w - content_w) / 2.0;

    let measure = |s: &str, size: u16| measure_text(s, None, size, 1.0).width;
    let (rows, total) = layout(&help_lines(), content_w, &measure);
    let view = bottom - top;
    let max_scroll = (total - view + 20.0).max(0.0);
    let mut scroll = app.help_scroll - mouse_wheel().1 * 40.0;
    if is_key_down(KeyCode::Down) {
        scroll += 600.0 * get_frame_time();
    }
    if is_key_down(KeyCode::Up) {
        scroll -= 600.0 * get_frame_time();
    }
    if is_key_pressed(KeyCode::PageDown) {
        scroll += view * 0.9;
    }
    if is_key_pressed(KeyCode::PageUp) {
        scroll -= view * 0.9;
    }
    app.help_scroll = scroll.clamp(0.0, max_scroll);

    for row in &rows {
        let y = top + row.y - app.help_scroll;
        if y < top - HEADING_HEIGHT || y > bottom + ROW_HEIGHT {
            continue;
        }
        match row.kind {
            Kind::Heading => {
                draw_text(&row.text, x, y + 30.0, HEADING_SIZE as f32, ACCENT);
            }
            Kind::Text => {
                draw_text(&row.text, x, y + 20.0, TEXT_SIZE as f32, WHITE);
            }
            Kind::Bullet => {
                draw_text("-", x + 8.0, y + 20.0, TEXT_SIZE as f32, ACCENT);
                draw_text(&row.text, x + BULLET_INDENT, y + 20.0, TEXT_SIZE as f32, WHITE);
            }
            Kind::Continued => {
                draw_text(&row.text, x + BULLET_INDENT, y + 20.0, TEXT_SIZE as f32, WHITE);
            }
        }
    }

    // Cover the overflow above and below the text, then draw the title and the button over it.
    draw_rectangle(0.0, 0.0, w, top - 4.0, BG);
    draw_rectangle(0.0, bottom + 2.0, w, h - bottom, BG);
    text_centered("How to Play", w / 2.0, 70.0, 56, ACCENT);
    if max_scroll > 0.0 {
        let bar_h = (view * view / (total + 20.0)).max(30.0);
        let bar_y = top + (view - bar_h) * app.help_scroll / max_scroll;
        draw_rectangle(w - 16.0, bar_y, 6.0, bar_h, Color::new(0.45, 0.75, 0.40, 0.6));
    }
    let back = button(Rect::new(w / 2.0 - 110.0, h - 84.0, 220.0, 56.0), "Back", true);
    if back || is_key_pressed(KeyCode::Escape) {
        app.mode = Mode::Worlds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten pixels a character, whatever the size.
    fn ten_per_char(s: &str, _size: u16) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    fn all_text() -> String {
        help_lines()
            .into_iter()
            .map(|l| match l {
                Line::Heading(t) | Line::Text(t) | Line::Item(t) => t,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_help_covers_the_controls_and_every_part_of_the_game() {
        let t = all_text();
        for word in [
            "W A S D", "Shift", "Space", "Left mouse", "Right mouse", "inventory", "Esc", "hearts", "Bow Drill", "Bellows", "bears",
            "Rivers", "Clay", "Delete", "Stone Axe", "Bronze Mallet", "Boulders", "Sling", "Loadstone", "Compass", "clay forge", "Tin Pot", "Clay Jar", "Moose", "Rabbits", "stew", "Iron Axe",
        ] {
            assert!(t.contains(word), "the help mentions {word:?}");
        }
        for heading in ["Controls", "Crafting", "Fire and cooking", "Smelting metal", "Water and stew", "Animals and hunting"] {
            assert!(help_lines().contains(&Line::Heading(heading.into())), "{heading}");
        }
    }

    #[test]
    fn every_recipe_is_listed_with_its_ingredients() {
        let items: Vec<String> = help_lines().into_iter().filter_map(|l| if let Line::Item(t) = l { Some(t) } else { None }).collect();
        for r in RECIPES {
            let line = format!("{}: {}", r.output.name(), r.describe());
            assert!(items.contains(&line), "{line:?} is missing from the help");
        }
        assert!(!all_text().contains("Wooden Plank:"), "planks have no recipe");
    }

    #[test]
    fn the_help_explains_every_step_from_stone_and_pebble_to_bronze_and_iron() {
        let t = all_text();
        for part in [
            "a stone (makes copper) or a pebble (makes tin)",
            "Clay forge:",
            "Bronze:",
            "Iron:",
            "Boulders",
            "A piece of leather makes three leather cords",
            "Bronze Mallet",
        ] {
            assert!(t.contains(part), "the help says {part:?}");
        }
        assert!(!t.contains("veined") && !t.contains("Veined"), "veined stones are gone");
        assert!(!t.contains("Plain stones and pebbles do not smelt"));
    }

    #[test]
    fn the_numbers_in_the_help_are_the_games_numbers() {
        let t = all_text();
        for n in [BEAR_HEALTH, DEER_HEALTH, QUAIL_HEALTH, MAX_HEALTH as i32] {
            assert!(t.contains(&n.to_string()), "{n}");
        }
        assert!(t.contains(&format!("{} seconds", SMELT_SECONDS as i32)) && t.contains(&format!("{} seconds", COOK_SECONDS as i32)));
        assert!(t.contains(&format!("{} blocks away", BEAR_CALM_DISTANCE as i32)));
    }

    #[test]
    fn wrapping_breaks_between_words_and_keeps_every_word() {
        let measure = |s: &str| s.chars().count() as f32 * 10.0;
        let lines = wrap("one two three four five six", 100.0, &measure);
        assert_eq!(lines, vec!["one two", "three four", "five six"]);
        assert!(lines.iter().all(|l| measure(l) <= 100.0));
        assert_eq!(wrap("short", 100.0, &measure), vec!["short"]);
        assert!(wrap("", 100.0, &measure).is_empty());
        assert!(wrap("   ", 100.0, &measure).is_empty());
        assert_eq!(wrap("a verylongwordthatdoesnotfit b", 50.0, &measure), vec!["a", "verylongwordthatdoesnotfit", "b"]);
    }

    #[test]
    fn the_layout_never_overruns_its_width_and_rows_run_downwards() {
        for width in [300.0, 520.0, 860.0] {
            let (rows, total) = layout(&help_lines(), width, &ten_per_char);
            assert!(!rows.is_empty() && total > 0.0);
            assert!(rows.windows(2).all(|w| w[1].y > w[0].y), "rows are in order");
            for r in &rows {
                let limit = if matches!(r.kind, Kind::Bullet | Kind::Continued) { width - BULLET_INDENT } else { width };
                let single_long_word = !r.text.contains(' ');
                assert!(ten_per_char(&r.text, TEXT_SIZE) <= limit || single_long_word || r.kind == Kind::Heading, "{:?} at width {width}", r.text);
            }
            assert!(rows.last().unwrap().y < total);
        }
        // A narrower screen needs more rows.
        let (narrow, _) = layout(&help_lines(), 300.0, &ten_per_char);
        let (wide, _) = layout(&help_lines(), 860.0, &ten_per_char);
        assert!(narrow.len() > wide.len());
    }

    #[test]
    fn a_bullet_is_followed_by_continuations_and_never_starts_with_one() {
        let (rows, _) = layout(&help_lines(), 400.0, &ten_per_char);
        assert!(rows.iter().any(|r| r.kind == Kind::Continued), "long entries wrap");
        for pair in rows.windows(2) {
            if pair[1].kind == Kind::Continued {
                assert!(matches!(pair[0].kind, Kind::Bullet | Kind::Continued));
            }
        }
        assert_eq!(rows[0].kind, Kind::Heading);
    }
}
