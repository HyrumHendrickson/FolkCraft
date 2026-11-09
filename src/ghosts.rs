//! The ghosts: four places where a voice speaks (as text on the screen: ghosts cannot be seen or touched). The graveyard's
//! ghost asks for a song, and once it has had one gives a silver key and thanks the player and says no more. In each of the
//! three silver towers, playing a song brings a ghostly voice with a hint.

/// What the ghost of the graveyard says when the player walks in: it asks for a song.
pub const GRAVEYARD_ASKS: &str = "Traveller... I cannot rest. Will you play me a song, so that I may finally sleep?";
/// What it says once it has heard one (and gives the player a silver key). After this it says nothing more.
pub const GRAVEYARD_THANKS: &str = "Thank you. At last I can rest. Take this key: it is yours to keep. In three lonely towers there are others who may like to hear the music.";
/// The voice in the Skylands' tower.
pub const SKY_TOWER_GHOST: &str = "Wade through the river until you find the island, and do not leave the water until you do.";
/// The voice in the Horizen's tower, in the cave.
pub const CAVE_TOWER_GHOST: &str = "Things work differently at night. Try out all your tools at night.";
/// The voice in the tower on a lake's island (Nessie's).
pub const LAKE_TOWER_GHOST: &str = "Blow the horn in the stonehenge.";

/// What the ghost of the king says when the pipes are played in his castle in the Horizen's giant cave.
pub const KING_GREETING: &str = "I am the king eternal. Give me gold and I shall send my spirits to guard you.";
/// What he says when gold has been given, and when it was not enough or he has sent all the spirits he can.
pub const KING_SENDS: &str = "It is done. My spirits will follow you, and each will loose ten arrows at your foes.";
pub const KING_WANTS_MORE: &str = "Ten pieces of gold for each spirit. You have not that much.";
/// The general store's ghost: what he says when the player first comes to trade, when gold has been spent, and when the pipes are played for him.
pub const SHOP_GREETING: &str = "Welcome, traveller. Gold for goods, and no questions asked.";
/// What he says to a player who comes to him with ash in the inventory.
pub const SHOP_ASH_HINT: &str = "Ash, is it? The fairies of these islands will trade a vial of moonlight for a piece of ash. Gold for goods, ash for fairies.";
pub const SHOP_BOUGHT: &str = "A fair trade. Come again.";
pub const SHOP_THANKS: &str = "Thank you, friend. It has been long since anyone played for me.";
pub const KING_FULL: &str = "My spirits already throng about you. Spend them first.";

/// Cut `text` into lines of at most `max_chars` characters, at spaces (a word longer than that gets a line of its own).
pub fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= max_chars => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_cut_at_spaces_and_nothing_is_lost() {
        let lines = wrap(SKY_TOWER_GHOST, 30);
        assert!(lines.len() >= 3 && lines.iter().all(|l| l.len() <= 30));
        assert_eq!(lines.join(" "), SKY_TOWER_GHOST);
        assert_eq!(wrap("", 10), Vec::<String>::new());
    }

    #[test]
    fn each_ghost_says_what_it_was_meant_to() {
        assert!(SKY_TOWER_GHOST.contains("Wade") && SKY_TOWER_GHOST.contains("river") && SKY_TOWER_GHOST.contains("island") && SKY_TOWER_GHOST.contains("water"));
        assert!(CAVE_TOWER_GHOST.contains("night") && CAVE_TOWER_GHOST.contains("tools"));
        assert!(LAKE_TOWER_GHOST.contains("horn") && LAKE_TOWER_GHOST.contains("stonehenge"));
        assert!(KING_GREETING.starts_with("I am the king eternal") && KING_GREETING.contains("Give me gold and I shall send my spirits to guard you"));
        assert!(GRAVEYARD_ASKS.contains("song") && GRAVEYARD_THANKS.contains("Thank") && GRAVEYARD_THANKS.contains("key"));
        assert!(GRAVEYARD_THANKS.contains("three lonely towers") && GRAVEYARD_THANKS.contains("others") && GRAVEYARD_THANKS.contains("music"));
    }
}
