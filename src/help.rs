//! The help screen: how to play, and what can be made and how.
//!
//! The text is built by `help_lines` from the game's own tables and constants (the recipe list, the fire
//! and smelting times, the animals' health), so it cannot drift away from the rules. Drawing is a thin
//! wrapper (`help_screen`) over `layout`, which wraps the text to a width and is plain logic.

use macroquad::prelude::*;

use crate::app::App;
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
        item("F2: open the Settings in the middle of a game (the game waits); Esc closes them. The Settings screen is also a button on the world list. It has a slider for the render distance, from 3 to 10 chunks (3 by default): a longer view draws more land and costs frame rate. Creatures keep the same spawn and vanish distances at any setting, and new lakes, mountains, castles, wells, graveyards and rainbows always appear beyond the distance you can see."),
        item("F1: open this help in the middle of a game (the game waits). F1 or Esc closes it. Here, scroll with the mouse wheel, the arrow keys, Page Up and Page Down, Home and End, or by dragging the bar at the right."),
        item("Inventory: drag a stack onto another slot to move or merge it. The panel on the right lists everything you can make right now; click one to make it."),
        item("Dev mode: when you create a world you can choose Dev mode instead of Normal. The game is the same, except that in the inventory you can scroll the list on the right past the recipes to a list of every item; click one to add it, right click to add a stack."),
        heading("Staying alive"),
        item(format!("You have {MAX_HEALTH} hearts. A fire burns you for {FIRE_DAMAGE} heart every second you touch it, and a bear that reaches you takes {BEAR_DAMAGE}.")),
        item("If you run out of hearts you start again at the beginning of the world. You keep your inventory."),
        item(format!(
            "Right click food to eat it when you are hurt: berries +{BERRY_HEAL}, cooked venison +{VENISON_HEAL}, cooked poultry +{POULTRY_HEAL}, cooked bear meat +{BEAR_MEAT_HEAL}. Raw meat cannot be eaten: cook it first."
        )),
        heading("The land"),
        item("The world is forest, with clearings full of grass, flowers and berry bushes, broad gentle hills and valleys, and winding rivers. Lakes and mountains are not found by chance: see Exploring."),
        item(format!("At most {MAX_DROPPED_ITEMS} items can lie on the ground at once. Beyond that the oldest ones disappear, so pick up what you want to keep, and do not leave things lying about.")),
        item("The ground cannot be dug. You can walk up any one-block step by jumping, which is how you climb a hill. A step of two blocks is a cliff: you cannot jump it, so go round it."),
        item("Rivers are one block deep: wade straight through them, and jump to climb out the other side. Look down at the river bed to see what lies on it."),
        heading("Exploring"),
        text("Three things reward you for going out and looking. None is found by chance: each comes from doing something, and each is explained here step by step."),
        item(format!(
            "Lake, island and castle. Step into a river and wade along it, without leaving the water, until you are {LAKE_TREK_CHUNKS} chunks ({} blocks) from where you entered, counting along either direction. Jumping while you wade is fine; stepping out onto the bank starts you over. A very large lake then appears ahead, about {} blocks across, still only one block deep, joined to your river by a channel of water. Keep wading to reach it. In the middle of the lake is a large island: a hill with a flat top, {ISLAND_HILL_HEIGHT} blocks high, with forest on its lower slopes. Every step of the slope can be jumped, so climb it anywhere. On the flat top stands a small castle of grey stone. Its outer wall has a gate on the south side, and inside is a keep with a door on its south side. Castle stone cannot be broken. Lakes are made again each time you make the walk.",
            LAKE_TREK_CHUNKS * CHUNK_SIZE,
            LAKE_RADIUS * 2
        )),
        item(format!(
            "Mountain. Hold a Compass: a dial appears in the top right with a red needle that points north, and it turns as you turn. A world has only one mountain: once it is out the needle points at it from wherever you are. Walk north, the way the needle points, for {COMPASS_TREK_CHUNKS} chunks ({} blocks): the dial counts your progress. Putting the compass away starts the count over, and walking south takes progress back. A mountain then rises ahead, {MOUNTAIN_HEIGHT} blocks tall and several hundred blocks wide. Its slope is broken by {MOUNTAIN_MAX_LEDGES} rings of cliff, each {MOUNTAIN_LEDGE_HEIGHT} blocks high, which no jump can clear. Each ring has one gap, a ramp, and the gaps lie on alternate sides of the mountain, so you cannot run straight up: zigzag from one gap to the next, jumping the one-block steps between them, and bring food, since it is a long climb. Trees stop a little way up, then the slope is bare rock, and from {SNOW_LINE_RISE} blocks up the ground is snow. Snow cannot be mined yet. Five entrances stand at the foot of the mountain, each a timber frame over a stair that goes down into the hillside. The shafts run in towards the heart of the mountain, climbing one block for every two, and three rings of tunnel join them all under the mountain, with a chamber where each ring crosses a shaft. The highest rings are well over a hundred blocks up.",
            COMPASS_TREK_CHUNKS * CHUNK_SIZE
        )),
        item(format!(
            "Rainbow. Hold a Peep Stone, by day. A rainbow comes out far away, beyond the distance you can see, and comes down on a pile of gold at least {} blocks from you. It shows only while the Peep Stone is in your hand, so keep it there and follow the ghostly trail towards the end of the rainbow. The rainbow is built of translucent blocks, and all seven of its colours stand on the ground at both ends. Hold left mouse on the pile to take {RAINBOW_GOLD_DROP} gold. There is only one rainbow in a world: once its gold is taken you will never see another.",
            rainbow_distance() as i32
        )),
        item(format!(
            "Ghostly trail. Hold a Peep Stone and faint lights float over the ground ahead of you, drifting towards the place it points to, for up to {} blocks at a time. By day the trail leads to the end of the rainbow, and it keeps leading there even after the gold has been taken. At night it leads to the grand castle.",
            TRAIL_LENGTH as i32
        )),
        item("Grand castle by day and night. By night the castle is whole: its gate is shut and you cannot get in, but its stair to the second floor works, and the door of the throne room on the second floor is shut. By day it is a ruin: the gate has fallen so you can walk in, but the stair to the second floor is broken, and the door of the throne room is open. In the throne room by day there is little; by night the royal armour stands there, and it halves all damage you take. Reach it by being inside when day and night change: use a Red Berry Stew (day) or Purple Berry Stew (night)."),
        item("Bone Totem, well and graveyard. A moose drops Antlers. Craft Antlers and 2 Gold into a Bone Totem. Hold it by day and the world's one deep well appears far away (a stone ring round 4 by 4 blocks of water, with a wooden frame, a roof and a rope over it), with a ghostly trail to it; hold it at night and the world's one graveyard appears, with a trail. The trail leads to them for ever. In the Skylands the totem points the way to the silver tower, by day or night, once the tower has appeared (a ghostly trail leads there). Fall into the well and you are taken to the Horizen."),
        item("Silver towers. Three identical silver towers, each five floors high with a spiral stair, stand in the worlds: one on the island of every lake (where the castle used to be), one in the Skylands and one in the Horizen. The tower on a lake's island is a ruin while the lake's Loch Ness Monster lives: its walls are broken stumps, its stair is broken off and there is no way up and no door to open; kill the monster and the tower is made whole again, with its door locked. The Skylands tower stands on the next island to appear once you have walked on ten different islands. The Horizen tower stands in the great cavern of its cave and appears when you hold a peep stone there; a ghostly trail leads to it. Each tower's door is locked and needs a Silver Key, used with right click: it only turns the lock, and you keep the key. Three silver keys lie on stands in chambers of the mines under the mountain. At the top of each tower a relic lies on a plinth: the Philosopher's Stone (lake), the Horn of the Hunt (Skylands) and the King's Crown (Horizen). The Philosopher's Stone, the crown and the horn all have uses."),
        item(format!("Salt mine. Deep in the mines under the mountain, rarely, a cluster of pink salt crystals stands on the tunnel floor; break it by hand to collect {PINK_SALT_DROP_MIN} to {PINK_SALT_DROP_MAX} lumps of Pink Salt. Hold a lump in your hand and each bat that strikes you dies instead of hurting you, using up one lump. Bats live in the mineshafts and bite for {BAT_DAMAGE}.")),
        item("Compass in the Horizen and the Skylands. The needle points to where you arrived."),
        item(format!("The jungle. Falling off the Skylands lands you in the jungle, a stretch of the normal world {JUNGLE_DISTANCE_CHUNKS} chunks from the spawn point, four times the area it first had (about {} blocks across), thick with very tall trees. In the bare middle of it stands a big Mayan-style temple: a stepped pyramid with a broad stair up its south face and a shrine on top, with a pile of gold in it. A passage in its north face leads to a chamber at its very heart, where the Cursed Idol stands on a plinth.", JUNGLE_RADIUS * 2)),
        item("Bones. Killing a deer leaves two Bones one time in five, and Bones lie on the altar islands of the Skylands: they are all one kind. Craft 3 Bones into Musical Pipes. Hold them and right click to play a tune (see Ghosts below). By day the tune calls every quail and crow within 70 blocks: they fly to you, land near you and stand still for 3 seconds. At night it calls bats instead (see Bats)."),
        item(format!("The Horizen. A separate dimension with none of the creatures of the normal world except bats: a cave. You arrive in a pool in a domed chamber, and it is always day when you come in. Caves of every size lie all round, joined by tunnels so that you can wander from one to the next, and one winding tunnel leads to a single, far larger cavern. Nothing can be dug, so the cave is as it was made. Small ponds lie on the floor. Every {HORIZEN_CAMP_EVERY}th cave you come into has a camp: a burning log with a tin pot of red or purple berry stew, ready to ladle out and eat (right click the pot with an empty hand). Hold a Peep Stone and the silver tower appears in the great cavern, with a ghostly trail to it along the tunnel. If night falls in the cave (a Purple Berry Stew brings it) you black out and wake in the ring of Stonehenge. The Compass points back to the arrival pool; the Bone Totem does nothing there.")),
        item(format!("Bosses. On the summit of the mountain lives the Yeti ({YETI_HEALTH} health): it charges when you come within {YETI_NOTICE_DISTANCE:.0} blocks, hits for {YETI_DAMAGE}, never leaves the summit's neighbourhood and is slower than you are, but enraged below half health. In the lake lives the Loch Ness Monster ({LOCH_NESS_HEALTH} health), which swims only in its lake and strikes at you from the water with its long neck. Their health bar shows at the top of the screen. Axes, the Bronze Mallet, arrows and pebbles all hurt them. The gate of the castle on a lake's island stays shut until that lake's monster is dead.")),
        item(format!("The Skylands. Once the Yeti is dead a cyclone stands on the summit. Stand in it for {CYCLONE_SECONDS:.0} seconds, as the screen fades to white, and it carries you to the Skylands: a sky of floating islands joined by wooden bridges, with its own day and night. Step off an island and the screen goes white, and you fall from {SKY_FALL_DROP:.0} blocks above the normal world's spawn point. There is no way home but to jump off an island and fall.")),
        item("The Yeti comes back every time you leave the Skylands (by falling or by dying): the summit cyclone is gone until you have killed it again. Each lake with a tower has a Loch Ness Monster of its own, and a monster that has been killed never comes back."),
        item(format!("Fire tornadoes. By day in the Skylands a large fire tornado appears every {SKY_TORNADO_INTERVAL:.0} seconds, {SKY_TORNADO_SPAWN_DISTANCE:.0} blocks away and at your height, sweeps to where you stood when it spawned, burns you on contact and stays there for {SKY_TORNADO_LIFETIME:.0} seconds, so sometimes there are four at once, each burning for {SKY_TORNADO_DAMAGE}; night makes them all disappear, and so does killing the Phoenix, for good. In the crater of the Phoenix's volcano a much larger fire tornado whirls all day, always: step into it and it burns you for {SKY_TORNADO_DAMAGE}; it too is gone at night, and killing the Phoenix does not stop it. At night the wooden bridges between the islands break: only a short ragged stub is left at each end, so you cannot cross until day.")),
        item(format!("Crows. A crow behaves like a quail: it takes off when you come within {FLEE_DISTANCE:.0} blocks, dropping a feather, but it always flies west if the way is clear. It leaves {CROW_FEATHER_DROP} feathers and no poultry. Follow one crow west for two chunks ({CROW_ALTAR_WEST_BLOCKS:.0} blocks, staying within {CROW_FOLLOW_DISTANCE:.0} blocks of it) and it lands on a new ancient altar, a small altar with a Loadstone on it. Take the Loadstone by hand; the altar stays.")),
        item(format!("Iron Quail. Every {QUAIL_KILLS_PER_IRON_QUAIL} quail you kill, an Iron Quail ({IRON_QUAIL_HEALTH} health) appears and hunts you for as long as you both live: she never gives up, and she flies: climbing after you, over obstacles and up to you wherever you stand. She hits for {IRON_QUAIL_DAMAGE}, and when she dies she drops a Silver Key. If you die she is gone.")),
        item("Tin Pan. Made of 2 Tin. Hold it and break river clay: one time in twenty a Gold turns up as well, one time in five with a Rabbit Foot in your hotbar."),
        item("The Philosopher's Stone. Right click a boulder with it to turn the boulder to gold; the stone is kept. A gold boulder breaks by hand into gold."),
        item(format!("The King's Crown and Excalibur. The crown from the Horizen tower is worn like other garments (right click). Wear it in the normal world and a large ruined castle waits ahead of you, far off beyond the land you can see. In its courtyard a sword stands in a stone. Right click it while wearing the crown to draw Excalibur, the best weapon in the game ({EXCALIBUR_DAMAGE} damage, reaches {EXCALIBUR_REACH:.1} blocks). Without the crown the sword will not move.")),
        item("Wearing. You wear one thing at a time. Right click a Pelt Cloak (made of a Bear Pelt), the Royal Armour, the King's Crown or a Leather Backpack to put it on, or drag it onto the Worn slot in the inventory; click the Worn slot to take it off. They all share that one slot. While you wear a Pelt Cloak, bears leave you alone unless you hit them first."),
        item(format!("Leather Backpack. Made of 4 Leather and 2 Leather Cord. Right click it to put it on. It is worn in the same place as a cloak, the armour or the crown (the Worn slot), so you can wear only one of them at a time. While it is on, the inventory has {BACKPACK_EXTRA_ROWS} more rows of slots. To take it off, or to put on something else in its place, what is in its rows has to fit in the other slots first.")),
        item(format!("Ghosts. You cannot see or touch ghosts: their words just appear on the screen. In four places they speak. The ghost of the graveyard asks, as you walk in, for a song so that it may finally rest: play it a tune on Musical Pipes there and it thanks you, gives you a Silver Key, tells you that in three lonely towers there are others who may like to hear the music, and never speaks again. In each silver tower (the Skylands, the cave and a lake island), playing a tune brings a ghostly voice with a hint. The Skylands ghost tells you to wade through the river until you find the island, and not to leave the water until you do; the cave ghost says that things work differently at night, so try your tools at night; the ghost of the lake tower tells you to blow the horn in the stonehenge (see The Wild Hunt).")),
        item(format!("Stonehenge. A ring of {STONEHENGE_STONES} tall standing stones, with lintels over pairs of them, round an altar stone, about {STONEHENGE_DISTANCE_CHUNKS} chunks from where the world began. If night falls in the cave you black out and wake in its ring. It is where the Horn of the Hunt is blown (see The Wild Hunt).")),
        item(format!("Bats. Bats do not live in the normal world. Play Musical Pipes at night and {BATS_PER_PIPES} bats come out of the dark and fly straight at you (never more than {MAX_BAT_OVERWORLD} at once): each bites for {BAT_DAMAGE} every {BAT_ATTACK_COOLDOWN:.1} seconds and dies to a single arrow or blow. They are gone at dawn. In the Horizen's cave bats live naturally (up to {MAX_BAT_CAVE} near you at a time) and attack when you come within {BAT_NOTICE_DISTANCE:.0} blocks.")),
        item(format!("The Wild Hunt. The last boss. Take the Horn of the Hunt from the top of the Skylands tower, go into the ring of Stonehenge and right click with the horn to blow it. A ghostly host rides out and hunts you, with one health bar between them. The swordsman ({HUNT_SWORDSMAN_HEALTH} health) charges you and cuts for {HUNT_SWORD_DAMAGE}; it is slower than a sprint. Two archers ({HUNT_ARCHER_HEALTH} health each) stay {HUNT_ARCHER_MIN_RANGE:.0} to {HUNT_ARCHER_MAX_RANGE:.0} blocks off, side-stepping, and each shoots an arrow every {HUNT_ARCHER_INTERVAL:.0} seconds for {HUNT_ARROW_DAMAGE}; keep a Rabbit Foot in your hotbar and they fire only half as often. Two lancers on horses ({HUNT_LANCER_HEALTH} health each) charge straight at you, faster than a sprint, and run you through for {HUNT_LANCER_DAMAGE}, then gallop away, rest for a few seconds and charge again. {HUNT_GHOST_BEARS} ghostly bears and {HUNT_GHOST_CROWS} ghostly crows come too: when you kill one of the first {HUNT_GHOST_BEARS} bears or the first {HUNT_GHOST_CROWS} crows, {HUNT_SPLIT_INTO} more of its kind appear (and those do not split). Excalibur does {EXCALIBUR_HUNT_FACTOR} times its damage to all of them. If you die they are gone, and the horn can call them again. Beat them all and you win the game; you can keep playing.")),
        item(format!("Yeti attacks. Close up the Yeti rears up and smashes the ground: everything within {YETI_SMASH_RADIUS:.0} blocks of it takes {YETI_SMASH_DAMAGE} and is thrown {YETI_SMASH_KNOCKBACK:.0} blocks away. While you are more than {YETI_ICE_MIN_RANGE:.0} blocks off it throws lumps of ice that burst where they land for {ICE_BURST_DAMAGE}, and it still hits with its fists. Bosses climb ledges of two blocks and find their way round trees and rocks.")),
        item(format!("Loch Ness Monster attacks. She will not come within {LOCH_NESS_SHORE_MARGIN:.0} blocks of the shore. While you are more than {LOCH_NESS_RAVEN_RANGE:.0} blocks from her she calls ravens, by day or night, to attack you; she taints a line of water towards you brown for {ACID_SECONDS:.0} seconds, and wading in it costs {ACID_DAMAGE} every second; and she still strikes with her neck. Shoot a bow or a sling and she flees to the middle of the lake, calling more ravens.")),
        item(format!("Ravens. At night ravens circle you at a distance, then swoop down for {RAVEN_DAMAGE} and fly away again. They go with the dawn.")),
        item(format!("Iron Quail feathers. Each Iron Quail (three come every {QUAIL_KILLS_PER_IRON_QUAIL} quail) strikes you, and from a distance shoots iron feathers for {IRON_FEATHER_DAMAGE}. A feather that misses lies on the ground: pick it up, hold it and click to throw it like a throwing star ({IRON_FEATHER_THROW_DAMAGE} damage). It is used up only when it hits an animal. If you already have a Silver Key, or one lies on the ground, anything that would drop another drops a gold bar instead.")),
        item(format!("Cave bats and the Vampire Bat. Bats now come into the Horizen's caves half as often as they did. For every {BATS_PER_VAMPIRE} bats you kill in the caves a Vampire Bat ({VAMPIRE_BAT_HEALTH} health) comes out of the dark. It has one attack: it circles you, swoops down for {VAMPIRE_BAT_DAMAGE}, flies off and comes again. It drops {VAMPIRE_BAT_GOLD_DROP} Gold.")),
        item(format!("The King's castle. In the Horizen a giant cave, {KING_CAVE_CELLS} cave-squares from where you arrive, holds the largest castle in the game: an outer wall {} blocks across with a gate on the south side, four towers and a great keep. Hold a Bone Totem in the Horizen and its trail leads through the caves to it. Play Musical Pipes inside the walls and the ghost of the king speaks: \"I am the king eternal. Give me gold and I shall send my spirits to guard you.\" A pop-up lets you give him gold: for every {GOLD_PER_SPIRIT} gold one spirit archer follows you. Each shoots {SPIRIT_SHOTS} arrows at whatever you have hit lately, then vanishes. They go with you into any other dimension. In the middle of the keep, on a dais, are the Boots of the Sky.", KING_WALL_HALF * 2 + 1)),
        item(format!("Boots of the Sky. Wear them (right click, or the Worn slot; they share the one worn place) and you run {BOOTS_SPEED_FACTOR} times as fast and jump {BOOTS_JUMP_DISTANCE_FACTOR} times as far.")),
        item(format!("Parrots and Bronze Parrots. Parrots are colourful quail that live in the jungle, where only the jungle's own creatures come (no deer, quail, bears, rabbits, moose or ravens). Every {PARROTS_PER_BRONZE_PARROT} parrots you kill, {BRONZE_PARROTS_PER_SUMMON} Bronze Parrots ({BRONZE_PARROT_HEALTH} health) come for you and fight like Iron Quails; each leaves {BRONZE_PARROT_BRONZE_DROP} Bronze.")),
        item(format!("Cursed Idol. Taken from its plinth in the heart of the jungle temple, whose passage is boarded up until the Orangutan is dead. While it is anywhere in your hotbar you deal {IDOL_DAMAGE_FACTOR} times the damage, but nothing can heal you: berries, meat and red stew all refuse.")),
        item(format!("The Orangutan. A boss ({ORANGUTAN_HEALTH} health) on the upper tiers of the jungle temple, which comes when you are within {ORANGUTAN_SPAWN_DISTANCE:.0} blocks. It walks and climbs after you, hits for {ORANGUTAN_DAMAGE}, hurls rocks from {ORANGUTAN_ROCK_MIN_RANGE:.0} to {ORANGUTAN_ROCK_MAX_RANGE:.0} blocks away (each bursts for {ROCK_BURST_DAMAGE}) and, within {ORANGUTAN_LEAP_MAX_RANGE:.0} blocks, crouches, leaps high and comes down beside you to smash the ground: everything within {ORANGUTAN_SMASH_RADIUS:.0} blocks takes {ORANGUTAN_SMASH_DAMAGE} and is thrown {ORANGUTAN_SMASH_KNOCKBACK:.0} blocks. Once dead it never returns, and leaves {ORANGUTAN_GOLD_DROP} Gold.")),
        item(format!("Jaguars and flying snakes. In the jungle jaguars (at most {MAX_JAGUAR}) creep up on you, crouch, then pounce for {JAGUAR_DAMAGE} and slink away. Flying snakes (at most {MAX_FLYING_SNAKE}) are about seventeen blocks long and drift slowly in wide loops through the air round you, then swoop down to bite for {SNAKE_DAMAGE}. A dead snake drops {SNAKE_FANG_DROP_MIN} to {SNAKE_FANG_DROP_MAX} Snake Fangs; a Pebble, a Stick, a Feather and a fang make a Poison Arrow. A bow shoots Poison Arrows first, then Phoenix Arrows, then ordinary arrows. A poison arrow is used up, and what it hits loses {POISON_DAMAGE} life every {POISON_INTERVAL:.0} second for {POISON_SECONDS:.0} seconds.")),
        item(format!("The Phoenix. Hold a Peep Stone in the Skylands and its trail leads to a giant island, with a volcano on it. The Phoenix ({PHOENIX_HEALTH} health) lives in the crater: it throws fireballs ({FIREBALL_DAMAGE} in a burst) and swoops down on you for {PHOENIX_SWOOP_DAMAGE}. When it dies the wandering fire tornadoes stop (the crater's does not) and it drops a Phoenix Feather; {PHOENIX_RESPAWN_SECONDS:.0} seconds later it rises again in the crater and the tornadoes come back. A Pebble, a Stick and the feather make a Phoenix Arrow: shoot it from a bow and it is never used up, and it never lands on the ground. Skylands islands may be wooded, ruined, hilly or have an ancient altar with bones; one in {SKY_LARGE_ONE_IN} is large.")),
        item(format!("Jerky. A Pink Salt and a raw meat make jerky, crafted with no fire: Venison, Poultry, Bear, Rabbit and Moose Jerky. Each heals as much as the cooked meat; stew still heals twice as much.")),
        item(format!("Falling stars. On a Skylands night a star streaks down from the sky about every {FALLING_STAR_INTERVAL:.0} seconds and lands, in a flash of light, on an island you can see. It is not always the one you are standing on.")),
        item(format!("Caves of the Horizen. The caves are of four kinds: plain; mossy, with green pools ringed with moss; river caves, with a stream winding across the floor (shallow enough to wade); and crystal caves, with red crystals like salt crystals that cannot be mined. You always arrive in the same pool, and the screen goes black before you are moved.")),
        item(format!(
            "Grand castle. Hold a Peep Stone at night. The first time you do so in a world, a grand castle rises somewhere beyond the edge of the land, at least {} blocks away, and a ghostly trail leads towards it. It is a great castle of grey stone, about {} blocks across: an outer wall with towers and a gate on the south side, and in the middle a keep of {GRAND_STOREYS} floors with a door on its south side, windows, a stair from each floor to the next, and a roof to walk on. The trail leads to the gate and then to the door of the keep. There is only one grand castle in a world. Castle stone cannot be broken.",
            grand_distance(), GRAND_REACH * 2 + 1
        )),
        item("Getting a Peep Stone: kill a rabbit (one in four drops a Rabbit Foot), keep the foot anywhere in your hotbar, and break pebbles by hand. Each has a chance to add a Peep Stone."),
        item("Getting a Compass: with the Rabbit Foot in your hotbar, break stones by hand until one drops a Loadstone. Break a boulder with a Bronze Mallet for iron ore. A Loadstone and an Iron Ore make a Compass in the inventory."),
        heading("Gathering"),
        item("Leaves and pine needles: sometimes drop a stick. Break them by hand."),
        item("Grass (in clearings): sometimes drops grass. Berry bushes: drop berries."),
        item("Stones and pebbles lie on the ground everywhere, and on the river bed under the water. Hold left mouse on one to pick it up."),
        item(format!("A Rabbit Foot in your hotbar gives broken stones a {}% chance to drop a Loadstone, boulders a {}% chance to drop Gold Ore, pebbles a {}% chance to add a Peep Stone, and leaves or pine needles a {}% chance to add a Holly Rod (see Will-o-whisps below). Gold Ore smelts like Iron Ore. A Loadstone and an Iron Ore make a Compass. See \"Exploring\" below for the Peep Stone and the Compass.", RABBIT_FOOT_LOADSTONE_CHANCE, RABBIT_FOOT_GOLD_CHANCE, RABBIT_FOOT_PEEP_STONE_CHANCE, RABBIT_FOOT_HOLLY_ROD_CHANCE)),
        item(format!("Clay lies on the river bed. Hold left mouse on a lump to dig {CLAY_DROP} clay out of it. Clay makes a clay forge.")),
        item(format!(
            "Boulders are big grey rocks, a little wider than a block, scattered on the ground (more on hills). Only a Bronze Mallet breaks one: hold left mouse on it for {} seconds. It drops a stone, a pebble and {BOULDER_IRON_MIN} to {BOULDER_IRON_MAX} iron ore.",
            BREAK_SECONDS_BOULDER as i32
        )),
        item("Tree trunks: you need a Stone Axe in your hand. Hold left mouse on a trunk for 5 seconds to get a log."),
        heading("Crafting"),
        text("Open the inventory with E. The Recipe Book button beside the crafting list shows every recipe in the game, with what you still need to get for each. Recipes need only the right amounts of each item, with no pattern. These are all of them:"),
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
        heading("Day and night"),
        item(format!(
            "Right click Purple Berry Stew to drink it: it hurts you ({PURPLE_BERRY_STEW_DAMAGE} hearts) and turns the day to night. Right click Red Berry Stew to drink it: it heals you ({BERRY_STEW_HEAL} hearts) and turns the night to day. The screen goes black for about {} second while the sky changes.",
            DAY_NIGHT_BLACKOUT_SECONDS as i32
        )),
        item("Berry bushes change colour with the sky: red berries by day are purple by night, and the other way round. Will-o-whisps appear only at night, and the rainbow only by day."),
        heading("Animals and hunting"),
        item(format!(
            "Deer ({DEER_HEALTH} health) and quail ({QUAIL_HEALTH} health) run or fly away from you, and from bears. Deer are faster than you even when you sprint."
        )),
        item(format!(
            "Bears ({BEAR_HEALTH} health) are rare and dangerous. Within {} blocks of you a bear wanders towards you. Within {} blocks it charges, and it keeps charging until you are {} blocks away. It runs a little slower than you sprint, so you can outrun it.",
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
            "Will-o-whisps appear at night, and in numbers (up to {MAX_WILL_O_WHISP} at once). They are pale, see-through, ghostly things with a glowing heart, two dark eyes and a thin tail, and they float above the ground. They flee at {WILL_O_WHISP_FLEE_SPEED} blocks per second when you come within {WILL_O_WHISP_FLEE_DISTANCE} blocks. They have no attacks or drops. Animals' eyes glow red at night."
        )),
        item(format!(
            "Holly Rod and will-o-whisps. Keep a Holly Rod anywhere in your hotbar (you need not hold it) and every will-o-whisp within {} blocks floats towards you instead of fleeing. One that comes within five feet of you stays near you and follows you wherever you go, circling you, for as long as the rod stays in the hotbar. When you drink Red Berry Stew to make it day, those whisps come with you and follow you by day too. After about {WHISP_STORM_GATHER_SECONDS:.0} seconds with a follower, the ordinary white clouds are replaced by dense, dark ones that cover nearly the whole sky, rain begins and lightning flashes in the distance. The storm darkens the world by 20%. During the storm, hold the rod and left-click a creature or a block up to {HOLLY_ROD_REACH:.0} blocks away to call lightning down on it: it costs one following whisp and never hurts you, but it hurts every creature within {LIGHTNING_RADIUS:.0} blocks of where it lands (a deer, rabbit or quail dies at once). When the last whisp is gone, the storm clears after {WHISP_STORM_CLEAR_SECONDS:.0} seconds. Take the rod out of the hotbar and they let go: they flee again, and by day they fade away.",
            WILL_O_WHISP_ATTRACT_DISTANCE as i32
        )),
        item("A blow throws the animal back, and a blow that hurts you (or a burn) throws you back, so you can see when something lands."),
        item("An arrow that hits an animal is used up. An arrow that misses lands on the ground: walk over it to pick it up again. Feathers are what you need to make more."),
        item("A Sling is made of one leather and slings pebbles the way a bow shoots arrows: a pebble that hits an animal is used up, and one that misses lands on the ground to be picked up again."),
        heading("Tips"),
        item("Start with grass for twine, sticks from leaves, and a stone from the ground: that is a Stone Axe. Then logs, a Bow Drill, a fire, and food."),
        item("Hearts come back only by eating. Cook meat before a long trip, and carry berries, which need no cooking."),
        item("Look at what you are aiming at: a black outline means a click will do something, a red one means it will not."),
        item("Take a Bow or a Sling and plenty of ammunition when you go exploring. Bears are rare, but they have a lot of health and hit hard."),
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
    // The wheel is counted in notches (see `wheel_notches`), a few lines each.
    let mut scroll = app.help_scroll - wheel_notches() * 3.0 * ROW_HEIGHT;
    if is_key_down(KeyCode::Down) {
        scroll += 600.0 * get_frame_time();
    }
    if is_key_down(KeyCode::Up) {
        scroll -= 600.0 * get_frame_time();
    }
    if is_key_pressed(KeyCode::PageDown) || is_key_pressed(KeyCode::Space) {
        scroll += view * 0.9;
    }
    if is_key_pressed(KeyCode::PageUp) {
        scroll -= view * 0.9;
    }
    if is_key_pressed(KeyCode::Home) {
        scroll = 0.0;
    }
    if is_key_pressed(KeyCode::End) {
        scroll = max_scroll;
    }
    // Dragging the scroll bar with the mouse.
    if max_scroll > 0.0 && is_mouse_button_down(MouseButton::Left) {
        let (mx, my) = mouse_position();
        if mx >= w - 30.0 && my >= top && my <= bottom {
            scroll = ((my - top) / view * (total + 20.0) - view / 2.0).clamp(0.0, max_scroll);
        }
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
    if back || is_key_pressed(KeyCode::Escape) || (app.help_from_game && is_key_pressed(KeyCode::F1)) {
        app.close_help();
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
            "Rivers", "Exploring", "Bone Totem", "Horizen", "Wearing", "Lake, island and castle", "Mountain.", "Rainbow.", "Peep Stone", "Getting a Compass", "ghostly", "Clay", "Delete", "Stone Axe", "Bronze Mallet", "Boulders", "Sling", "Loadstone", "Compass", "clay forge", "Tin Pot", "Clay Jar", "Moose", "Rabbits", "stew", "Iron Axe",
        ] {
            assert!(t.contains(word), "the help mentions {word:?}");
        }
        for heading in ["Controls", "Exploring", "Crafting", "Fire and cooking", "Smelting metal", "Water and stew", "Animals and hunting"] {
            assert!(help_lines().contains(&Line::Heading(heading.into())), "{heading}");
        }
    }

    #[test]
    fn the_help_covers_the_cave_crows_the_mother_the_pan_and_the_relics() {
        let t = all_text();
        for word in ["Leather Backpack", "Ghosts.", "Stonehenge.", "keep the key", "stand still for 3 seconds", "a cave", "great cavern", "Fire tornadoes", "Skylands a large fire tornado", "sometimes there are four at once", "Crows.", "ancient altar", "Iron Quail", "Silver Key", "Tin Pan", "Philosopher's Stone", "Excalibur", "King's Crown", "Bats.", "Vampire Bat", "King's castle", "Boots of the Sky", "Cursed Idol", "The Orangutan", "Jaguars and flying snakes", "ragged stub", "Bones.", "The Wild Hunt.", "three lonely towers", "a ruin while"] {
            assert!(t.contains(word), "the help mentions {word:?}");
        }
        assert!(!t.contains("empty flat land"), "the Horizen is no longer flat land");
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
    fn the_help_explains_how_to_find_a_lake_a_mountain_and_the_rainbow() {
        let t = all_text();
        let parts = [
            format!("{LAKE_TREK_CHUNKS} chunks ({} blocks)", LAKE_TREK_CHUNKS * CHUNK_SIZE),
            format!("{COMPASS_TREK_CHUNKS} chunks ({} blocks)", COMPASS_TREK_CHUNKS * CHUNK_SIZE),
            format!("{MOUNTAIN_HEIGHT} blocks tall"),
            format!("from {SNOW_LINE_RISE} blocks up the ground is snow"),
            format!("about {} blocks across", LAKE_RADIUS * 2),
            format!("At most {MAX_DROPPED_ITEMS} items"),
            "large island".to_string(),
            "small castle of grey stone".to_string(),
            "gate on the south side".to_string(),
            "Snow cannot be mined yet".to_string(),
            "only one rainbow in a world".to_string(),
            "shows only while the Peep Stone is in your hand".to_string(),
            "Getting a Peep Stone".to_string(),
            "Getting a Compass".to_string(),
        ];
        for part in &parts {
            assert!(t.contains(part.as_str()), "the help says {part:?}");
        }
        assert!(!t.contains("Peep Stones and Holly Rods do nothing yet") && !t.contains("Compass, which does nothing yet"), "stale text is gone");
    }

    #[test]
    fn the_help_covers_the_newer_parts_of_the_game() {
        let t = all_text();
        for part in [
            "F1: open this help",
            "a hill with a flat top",
            "rings of cliff",
            "zigzag",
            "Ghostly trail.",
            "even after the gold has been taken",
            "Grand castle.",
            "Holly Rod and will-o-whisps.",
            "within five feet",
            "follow you by day too",
            "Day and night",
            "goes black",
            "only by day",
        ] {
            assert!(t.contains(part), "the help says {part:?}");
        }
        assert!(!t.contains("Holly Rods do nothing yet") && !t.contains("keep jumping uphill"), "stale text is gone");
        assert!(help_lines().contains(&Line::Heading("Day and night".into())));
    }

    #[test]
    fn the_help_is_long_enough_to_need_scrolling() {
        let (rows, total) = layout(&help_lines(), 860.0, &ten_per_char);
        assert!(rows.len() > 150 && total > 3000.0, "{} rows, {total} px", rows.len());
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
