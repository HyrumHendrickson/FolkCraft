//! Tunable game constants.

pub const CHUNK_SIZE: i32 = 20;
pub const WORLD_HEIGHT: i32 = 24;
pub const STONE_LAYERS: i32 = 1;
pub const DIRT_LAYERS: i32 = 5;
/// y of the highest dirt block; players stand at y = GROUND_TOP + 1.
pub const GROUND_TOP: i32 = STONE_LAYERS + DIRT_LAYERS - 1;

/// Chunks within this many chunks of the player (square radius) are rendered; the rest are not.
pub const RENDER_RADIUS: i32 = 3;
/// Chunks are generated one ring beyond the render radius so mesh edges next to the
/// outermost rendered chunks are culled correctly.
pub const GEN_RADIUS: i32 = RENDER_RADIUS + 1;

pub const REACH: f32 = 5.0;
pub const PLAYER_HALF_WIDTH: f32 = 0.3;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
pub const WALK_SPEED: f32 = 5.3;
pub const SPRINT_SPEED: f32 = 8.2;
pub const JUMP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = 25.0;
pub const MAX_FALL_SPEED: f32 = 40.0;
pub const LOOK_SENSITIVITY: f32 = 0.0025; // radians per pixel
pub const AUTOSAVE_INTERVAL: f32 = 20.0;
/// Chunk meshes rebuilt per frame (keeps frame times steady while exploring).
pub const MESH_BUDGET_PER_FRAME: usize = 2;
/// Vertical mouse/trackpad look direction. `true` = moving up looks down.
pub const INVERT_LOOK_Y: bool = true;
/// Macroquad silently truncates any single mesh larger than its draw-call buffers
/// (10,000 vertices / 5,000 indices by default). Meshes are split at this many quads
/// (800 quads = 3,200 vertices / 4,800 indices) so nothing is ever cut off.
pub const MAX_QUADS_PER_MESH: usize = 800;
/// Keyboard look speed for the arrow keys, in radians per second.
pub const KEY_LOOK_SPEED: f32 = 1.8;
/// Ambient-occlusion brightness for 0..=3 (0 = fully enclosed corner).
pub const AO_LEVELS: [f32; 4] = [0.50, 0.68, 0.84, 1.0];

// --- Inventory and drops -------------------------------------------------------------
/// Total inventory slots: the first `HOTBAR_SLOTS` form the hotbar, the rest the main grid.
pub const INVENTORY_SLOTS: usize = 36;
pub const HOTBAR_SLOTS: usize = 9;
/// Maximum items per stack.
pub const MAX_STACK: u32 = 64;
/// Chance (percent) that breaking a leaf block drops a stick.
pub const LEAF_STICK_CHANCE: u64 = 50;
/// Items lying on the ground can be picked up once they are this old (seconds)...
pub const PICKUP_DELAY: f32 = 0.5;
/// ...and when the player is within this horizontal distance (blocks).
pub const PICKUP_RADIUS: f32 = 1.2;
/// Items up to this far below the player's feet can be picked up, so an item that fell into
/// a one-block dip beside you (a river bed, say) is still reachable.
pub const PICKUP_BELOW: f32 = 1.6;
/// Seconds a pickup message stays on screen.
pub const TOAST_SECONDS: f32 = 2.5;

// --- Trees and clearings -------------------------------------------------------------
/// Tree trunks are this much thinner than a full block on each side (so 0.7 wide).
pub const TRUNK_INSET: f32 = 0.15;
/// Clearings are generated per square region of this many chunks, at most one per region.
pub const CLEARING_REGION_CHUNKS: i32 = 3;
/// Chance (percent) that a region has a clearing.
pub const CLEARING_CHANCE: u64 = 50;
pub const CLEARING_MIN_RADIUS: i32 = 6;
pub const CLEARING_MAX_RADIUS: i32 = 30;
/// No tree trunk grows within this many blocks outside a clearing's edge, so no canopy
/// overhangs the clearing.
pub const CLEARING_TREE_BUFFER: i32 = 3;
/// Per-column chances (percent) inside a clearing of grass, a flower, or a bush.
pub const GRASS_PERCENT: u64 = 22;
pub const FLOWER_PERCENT: u64 = 5;
pub const BUSH_PERCENT: u64 = 4;

// --- Hills -------------------------------------------------------------------------------
/// Hills are generated per square region of this many chunks, at most one per region.
pub const HILL_REGION_CHUNKS: i32 = 5;
/// Chance (percent) that a region has a hill.
pub const HILL_CHANCE: u64 = 50;
pub const HILL_MIN_RADIUS: i32 = 20;
pub const HILL_MAX_RADIUS: i32 = 32;
/// Blocks a hill rises above the flat ground at its centre.
pub const HILL_MIN_HEIGHT: i32 = 3;
pub const HILL_MAX_HEIGHT: i32 = 5;
/// No hill reaches closer than this many blocks to the spawn point, which stays flat.
pub const HILL_SPAWN_CLEARANCE: i32 = 16;

// --- Valleys -----------------------------------------------------------------------------
/// A valley is a hill turned upside down and shares the hills' regions (at most one hill or valley each).
/// One roll per region decides: a hill for the first `HILL_CHANCE` percent, a valley for the next
/// `VALLEY_CHANCE` percent, nothing for the rest.
pub const VALLEY_CHANCE: u64 = 25;
/// Radius and depth (blocks the ground is lowered at the centre) of a valley. The deepest valley still
/// leaves one layer of dirt above the bedrock.
pub const VALLEY_MIN_RADIUS: i32 = 20;
pub const VALLEY_MAX_RADIUS: i32 = 32;
pub const VALLEY_MIN_DEPTH: i32 = 2;
pub const VALLEY_MAX_DEPTH: i32 = 4;

// --- Rivers ------------------------------------------------------------------------------
/// Rivers follow the zero contours of a smooth noise field. The big lattice sets how far apart they lie
/// and the small one makes them wind (both in blocks).
pub const RIVER_BIG_SCALE: f32 = 230.0;
pub const RIVER_SMALL_SCALE: f32 = 55.0;
/// Weight of the small lattice against the big one: more of it, more bends.
pub const RIVER_WIGGLE: f32 = 0.85;
/// Half the width of a river, in blocks: each river varies smoothly between these (so 4 to 7 blocks wide).
pub const RIVER_MIN_HALF_WIDTH: f32 = 2.0;
pub const RIVER_MAX_HALF_WIDTH: f32 = 3.5;
/// Distance (blocks) over which a river swells and narrows.
pub const RIVER_WIDTH_SCALE: f32 = 90.0;
/// A river is one block deep: the bed lies this far below the surrounding ground and the water fills the gap
/// up to it, so a player can always wade out (the jump clears a block, not two).
pub const RIVER_DEPTH: i32 = 1;
/// The fraction of a block the water stands below the level of the ground beside it.
pub const WATER_SURFACE_DROP: f32 = 0.12;
/// Opacity of the water's surface: the bed shows through it.
pub const WATER_ALPHA: f32 = 0.5;
/// Rivers keep this far from the spawn point (they bend around it), and are free to run beyond the second distance.
pub const RIVER_SPAWN_CLEARANCE: f32 = 34.0;
pub const RIVER_SPAWN_FADE: f32 = 70.0;
/// A hill or valley never comes within this many blocks of a river...
pub const RIVER_RELIEF_MARGIN: i32 = 4;
/// ...and one that would is moved to up to this many other places in its region before it is given up.
pub const RELIEF_PLACEMENT_TRIES: i32 = 8;

// --- Plant drops and fire --------------------------------------------------------------
/// Chance (percent) that breaking grass drops one grass.
pub const GRASS_DROP_CHANCE: u64 = 70;
/// Breaking a bush rolls this many independent berry chances (so 0 to 5 berries)...
pub const BUSH_BERRY_ROLLS: u32 = 5;
/// ...each succeeding with this chance (percent).
pub const BUSH_BERRY_CHANCE: u64 = 50;
/// A lit log burns this long, and each log added to the fire extends it by this much (seconds).
pub const FIRE_SECONDS: f32 = 60.0;

// --- Breaking time ---------------------------------------------------------------------
/// Seconds the left mouse button must be held on a block before it breaks.
pub const BREAK_SECONDS_WOOD: f32 = 5.0;
pub const BREAK_SECONDS_BUSH: f32 = 3.0;
pub const BREAK_SECONDS_FOLIAGE: f32 = 2.0;
pub const BREAK_SECONDS_PLANT: f32 = 0.5;
/// Placed logs, pebbles, stones and charcoal.
pub const BREAK_SECONDS_PLACED: f32 = 0.5;
/// Placed wooden planks.
pub const BREAK_SECONDS_PLANKS: f32 = 1.5;
/// A boulder (bronze mallet) and a clay forge (by hand).
pub const BREAK_SECONDS_BOULDER: f32 = 4.0;
pub const BREAK_SECONDS_FORGE: f32 = 1.0;

// --- Health ------------------------------------------------------------------------------
pub const MAX_HEALTH: u32 = 10;
/// Touching a burning fire costs one life every this many seconds.
pub const FIRE_DAMAGE_INTERVAL: f32 = 1.0;
/// Life lost per burn tick.
pub const FIRE_DAMAGE: u32 = 1;
/// The player counts as touching a fire when within this many blocks of its shape.
pub const FIRE_TOUCH_MARGIN: f32 = 0.1;
/// Life restored by eating one berry.
pub const BERRY_HEAL: u32 = 1;
/// Life restored by eating one piece of cooked venison or cooked poultry.
pub const VENISON_HEAL: u32 = 4;
pub const POULTRY_HEAL: u32 = 3;
pub const BEAR_MEAT_HEAL: u32 = 5;

// --- Cooking -----------------------------------------------------------------------------
/// Seconds a piece of meat lying on a burning log takes to cook.
pub const COOK_SECONDS: f32 = 8.0;

/// Seconds a stone or pebble on burning charcoal takes to smelt into copper or tin, and a piece of
/// tin and copper or an iron ore in a lit clay forge takes to smelt into bronze or iron.
pub const SMELT_SECONDS: f32 = 10.0;
/// A lit clay forge hit with bellows glows brighter for this long (seconds); hitting it again renews it.
/// Iron only smelts while the forge glows.
pub const BELLOWS_GLOW_SECONDS: f32 = 40.0;
/// Lumps of clay a broken clay forge gives back.
pub const FORGE_CLAY_DROP: u32 = 4;

// --- Stones and pebbles lying on the ground ----------------------------------------------
/// Chance (per mille, per ground column) of a pebble or a stone lying on the forest floor, in
/// clearings and on hills alike. The columns of a hill are `HILL_ROCK_FACTOR` times as rocky.
pub const GROUND_PEBBLE_PER_MILLE: u64 = 8;
pub const GROUND_STONE_PER_MILLE: u64 = 4;
pub const HILL_ROCK_FACTOR: u64 = 2;
/// Chance (per mille, per ground column, on top of the above and scaled by `HILL_ROCK_FACTOR` on hills) of a
/// boulder: a big rock that only a bronze mallet breaks, and the only source of iron.
pub const GROUND_BOULDER_PER_MILLE: u64 = 2;
/// A broken boulder drops one stone, one pebble and this many iron ores (inclusive range).
pub const BOULDER_IRON_MIN: i32 = 1;
pub const BOULDER_IRON_MAX: i32 = 3;
/// Chances (per mille, per river column) of clay, a pebble or a stone lying on the river bed under the water.
pub const RIVER_CLAY_PER_MILLE: u64 = 40;
pub const RIVER_PEBBLE_PER_MILLE: u64 = 20;
pub const RIVER_STONE_PER_MILLE: u64 = 15;
/// Lumps of clay dropped by one clay deposit.
pub const CLAY_DROP: u32 = 2;

// --- Animals -----------------------------------------------------------------------------
/// A deer or quail within this many blocks of the player runs or flies away.
pub const FLEE_DISTANCE: f32 = 10.0;
/// A fleeing deer calms down again once it is this far from the player.
pub const FLEE_CALM_DISTANCE: f32 = 16.0;
pub const DEER_HEALTH: i32 = 6;
pub const QUAIL_HEALTH: i32 = 2;
pub const BEAR_HEALTH: i32 = 14;
pub const DEER_WALK_SPEED: f32 = 1.6;
/// Faster than the player's sprint, so a deer cannot be run down.
pub const DEER_RUN_SPEED: f32 = 8.0;
pub const QUAIL_WALK_SPEED: f32 = 1.0;
pub const BEAR_WALK_SPEED: f32 = 2.2;
/// Slower than a sprinting player, so a bear can be outrun, but faster than walking.
pub const BEAR_RUN_SPEED: f32 = 5.4;
pub const QUAIL_FLY_SPEED: f32 = 7.0;
/// Blocks a startled quail climbs above where it took off before it levels out (enough to clear
/// the treetops).
pub const QUAIL_FLY_HEIGHT: f32 = 9.0;
pub const QUAIL_CLIMB_SPEED: f32 = 5.0;
/// A quail stays in the air for a random time in this range (seconds)...
pub const QUAIL_FLIGHT_MIN: f32 = 4.0;
pub const QUAIL_FLIGHT_MAX: f32 = 6.0;
/// ...and glides down at this speed during the last `QUAIL_DESCENT_TIME` seconds of it.
pub const QUAIL_DESCENT_SPEED: f32 = 4.0;
pub const QUAIL_DESCENT_TIME: f32 = 2.4;
/// A hurt animal keeps running or flying away for at least this long (seconds).
pub const ANIMAL_ALARM_SECONDS: f32 = 4.0;
/// Upward speed an animal uses to hop up a one-block step.
pub const ANIMAL_JUMP_SPEED: f32 = 8.0;
/// What a deer, a quail or a bear drops when killed.
pub const DEER_VENISON_DROP: u32 = 2;
pub const DEER_LEATHER_DROP: u32 = 1;
pub const QUAIL_POULTRY_DROP: u32 = 1;
pub const QUAIL_FEATHER_DROP: u32 = 3;
pub const BEAR_MEAT_DROP: u32 = 3;
pub const BEAR_PELT_DROP: u32 = 1;
/// Animals are simulated only this close to the player (blocks) and removed beyond the despawn distance.
pub const ANIMAL_UPDATE_DISTANCE: f32 = 70.0;
pub const ANIMAL_DESPAWN_DISTANCE: f32 = 95.0;
/// New animals appear between these distances from the player, one attempt every interval.
pub const ANIMAL_SPAWN_MIN_DISTANCE: f32 = 22.0;
pub const ANIMAL_SPAWN_MAX_DISTANCE: f32 = 60.0;
pub const ANIMAL_SPAWN_INTERVAL: f32 = 1.5;
/// At most this many of each kind live within the despawn distance.
pub const MAX_DEER: usize = 6;
pub const MAX_QUAIL: usize = 9;
pub const MAX_BEAR: usize = 2;
/// Relative chances that a spawn attempt brings in a deer, a quail or a bear (among the kinds with room left).
pub const SPAWN_WEIGHT_DEER: u64 = 30;
pub const SPAWN_WEIGHT_QUAIL: u64 = 55;
pub const SPAWN_WEIGHT_BEAR: u64 = 15;

// --- Bears -------------------------------------------------------------------------------
/// A bear this close to the player starts wandering towards them...
pub const BEAR_NOTICE_DISTANCE: f32 = 40.0;
/// ...and this close it charges, and keeps charging until the player is `BEAR_CALM_DISTANCE` away.
pub const BEAR_CHARGE_DISTANCE: f32 = 10.0;
pub const BEAR_CALM_DISTANCE: f32 = 20.0;
/// How far (radians) a wandering bear's heading may stray to either side of the direction to the player.
pub const BEAR_WANDER_SPREAD: f32 = 0.7;
/// A charging bear hurts the player when its body is this close to theirs (blocks, edge to edge)...
pub const BEAR_REACH: f32 = 0.6;
/// ...for this much life, and then needs this many seconds before its next blow.
pub const BEAR_DAMAGE: u32 = 2;
pub const BEAR_ATTACK_COOLDOWN: f32 = 1.2;

// --- Hunting -----------------------------------------------------------------------------
/// Damage of one stone axe swing, one bronze mallet swing and one arrow.
pub const AXE_DAMAGE: i32 = 4;
pub const MALLET_DAMAGE: i32 = 5;
pub const ARROW_DAMAGE: i32 = 3;
/// How far a stone axe reaches an animal (blocks), and the pause between swings or shots (seconds).
pub const AXE_REACH: f32 = 3.5;
pub const AXE_COOLDOWN: f32 = 0.45;
pub const MALLET_COOLDOWN: f32 = 0.6;
pub const BOW_COOLDOWN: f32 = 0.7;
pub const ARROW_SPEED: f32 = 32.0;
pub const ARROW_GRAVITY: f32 = 10.0;
/// An arrow that has not hit anything disappears after this many seconds.
pub const ARROW_LIFETIME: f32 = 6.0;

pub const SAVE_DIR: &str = "saves";

// --- Rabbits, moose, stews, knockback and arrows -----------------------------------------
pub const RABBIT_HEALTH: i32 = 2;
pub const MOOSE_HEALTH: i32 = 20;
pub const RABBIT_WALK_SPEED: f32 = 1.4;
/// A frightened rabbit bolts at this speed, the same as a deer.
pub const RABBIT_RUN_SPEED: f32 = DEER_RUN_SPEED;
pub const MOOSE_WALK_SPEED: f32 = 1.5;
/// An enraged moose charges at this speed, slower than a sprinting player.
pub const MOOSE_RUN_SPEED: f32 = 5.8;
/// A rabbit drops this much rabbit meat, and a rabbit's foot with this chance (percent).
pub const RABBIT_MEAT_DROP: u32 = 1;
pub const RABBIT_FOOT_CHANCE: u64 = 25;
pub const MOOSE_MEAT_DROP: u32 = 4;
pub const MOOSE_LEATHER_DROP: u32 = 2;
pub const MAX_RABBIT: usize = 9;
pub const MAX_MOOSE: usize = 2;
pub const SPAWN_WEIGHT_RABBIT: u64 = 55;
pub const SPAWN_WEIGHT_MOOSE: u64 = 10;
/// A moose that has been hurt chases the player until one of them is dead or they are this far apart (blocks).
pub const MOOSE_GIVE_UP_DISTANCE: f32 = 40.0;
pub const MOOSE_DAMAGE: u32 = 2;
pub const MOOSE_ATTACK_COOLDOWN: f32 = 1.2;
/// Life restored by cooked rabbit and cooked moose meat. A stew restores `STEW_HEAL_FACTOR` times as much as the
/// cooked meat it is made of.
pub const RABBIT_MEAT_HEAL: u32 = 2;
pub const MOOSE_MEAT_HEAL: u32 = 5;
pub const STEW_HEAL_FACTOR: u32 = 2;
/// Seconds a pot with water and raw meat needs on a fire to make a stew.
pub const STEW_SECONDS: f32 = 12.0;
/// Knockback: horizontal speed given to the player and to a hurt animal, the hop that goes with it, and how
/// quickly the push dies away (fraction lost per second).
pub const PLAYER_KNOCKBACK_SPEED: f32 = 6.0;
pub const ANIMAL_KNOCKBACK_SPEED: f32 = 6.0;
pub const KNOCKBACK_HOP: f32 = 4.0;
pub const KNOCKBACK_DECAY: f32 = 7.0;
/// An arrow that hit a block or ran out of flight lands this far back along its path (blocks).
pub const ARROW_DROP_BACKOFF: f32 = 0.3;
