//! Tunable game constants, grouped by the system that consumes them.

use std::sync::atomic::{AtomicI32, Ordering};

// --- World and rendering --------------------------------------------------------------

pub const CHUNK_SIZE: i32 = 20;
/// Tall enough for the mountain (`MOUNTAIN_HEIGHT`) and the sky above its summit.
pub const WORLD_HEIGHT: i32 = 180;
pub const STONE_LAYERS: i32 = 1;
pub const DIRT_LAYERS: i32 = 5;
/// Y of the highest dirt block; players stand at `GROUND_TOP + 1`.
pub const GROUND_TOP: i32 = STONE_LAYERS + DIRT_LAYERS - 1;

/// Chunks within this many chunks of the player are rendered. The player picks it in the Settings screen, between
/// `MIN_RENDER_RADIUS` and `MAX_RENDER_RADIUS`; read it with `render_radius()`.
pub const MIN_RENDER_RADIUS: i32 = 3;
pub const MAX_RENDER_RADIUS: i32 = 10;
pub const DEFAULT_RENDER_RADIUS: i32 = 3;

static RENDER_RADIUS_SETTING: AtomicI32 = AtomicI32::new(DEFAULT_RENDER_RADIUS);

/// The render radius in chunks that is in force now.
pub fn render_radius() -> i32 {
    RENDER_RADIUS_SETTING.load(Ordering::Relaxed)
}

/// Set the render radius, kept between `MIN_RENDER_RADIUS` and `MAX_RENDER_RADIUS`. Returns the value stored.
pub fn set_render_radius(radius: i32) -> i32 {
    let radius = radius.clamp(MIN_RENDER_RADIUS, MAX_RENDER_RADIUS);
    RENDER_RADIUS_SETTING.store(radius, Ordering::Relaxed);
    radius
}

/// Chunks are generated one ring beyond the render radius so outer mesh edges cull correctly.
pub fn gen_radius() -> i32 {
    render_radius() + 1
}

/// How far (blocks) from the player a new structure's nearest edge lies: one chunk beyond the generated ring, so
/// that it appears outside the render distance and is never seen to pop in.
pub fn structure_distance() -> i32 {
    (gen_radius() + 1) * CHUNK_SIZE
}
/// Chunk meshes rebuilt per frame to keep frame times steady while exploring.
pub const MESH_BUDGET_PER_FRAME: usize = 2;
/// Ambient-occlusion brightness for 0..=3 neighboring solid corners.
pub const AO_LEVELS: [f32; 4] = [0.50, 0.68, 0.84, 1.0];
/// Macroquad's safe mesh size in quads (800 = 3,200 vertices / 4,800 indices).
pub const MAX_QUADS_PER_MESH: usize = 800;

// --- Player and input ------------------------------------------------------------------

pub const REACH: f32 = 5.0;
pub const PLAYER_HALF_WIDTH: f32 = 0.3;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
pub const WALK_SPEED: f32 = 5.3;
pub const SPRINT_SPEED: f32 = 8.2;
pub const JUMP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = 25.0;
pub const MAX_FALL_SPEED: f32 = 40.0;
/// Radians per mouse pixel.
pub const LOOK_SENSITIVITY: f32 = 0.0025;
/// Keyboard look speed in radians per second.
pub const KEY_LOOK_SPEED: f32 = 1.8;
/// `true` means moving the mouse up looks down.
pub const INVERT_LOOK_Y: bool = true;
pub const AUTOSAVE_INTERVAL: f32 = 20.0;

// --- Inventory and item pickup ---------------------------------------------------------

/// The first `HOTBAR_SLOTS` inventory slots form the hotbar.
pub const INVENTORY_SLOTS: usize = 36;
pub const HOTBAR_SLOTS: usize = 9;
pub const MAX_STACK: u32 = 100;
/// Chance (percent) that leaves or pine needles drop a stick.
pub const LEAF_STICK_CHANCE: u64 = 70;
/// Items lying on the ground can be picked up once they are this old.
pub const PICKUP_DELAY: f32 = 0.5;
/// Horizontal pickup distance in blocks.
pub const PICKUP_RADIUS: f32 = 1.2;
/// How far below the player's feet an item can be picked up.
pub const PICKUP_BELOW: f32 = 1.6;
/// Distance at which an inventory drop lands in front of the player.
pub const INVENTORY_DROP_DISTANCE: f32 = 2.2;
/// How long a pickup message stays on screen.
pub const TOAST_SECONDS: f32 = 2.5;

// --- Terrain: trees, clearings, hills, valleys and rivers ------------------------------

pub const TRUNK_INSET: f32 = 0.15;
pub const CLEARING_REGION_CHUNKS: i32 = 3;
pub const CLEARING_CHANCE: u64 = 50;
pub const CLEARING_MIN_RADIUS: i32 = 6;
pub const CLEARING_MAX_RADIUS: i32 = 30;
pub const CLEARING_TREE_BUFFER: i32 = 3;
pub const GRASS_PERCENT: u64 = 22;
pub const FLOWER_PERCENT: u64 = 5;
pub const BUSH_PERCENT: u64 = 4;

pub const HILL_REGION_CHUNKS: i32 = 5;
pub const HILL_CHANCE: u64 = 50;
pub const HILL_MIN_RADIUS: i32 = 20;
pub const HILL_MAX_RADIUS: i32 = 32;
pub const HILL_MIN_HEIGHT: i32 = 3;
pub const HILL_MAX_HEIGHT: i32 = 5;
pub const HILL_SPAWN_CLEARANCE: i32 = 16;

pub const VALLEY_CHANCE: u64 = 25;
pub const VALLEY_MIN_RADIUS: i32 = 20;
pub const VALLEY_MAX_RADIUS: i32 = 32;
pub const VALLEY_MIN_DEPTH: i32 = 2;
pub const VALLEY_MAX_DEPTH: i32 = 4;

pub const RIVER_BIG_SCALE: f32 = 230.0;
pub const RIVER_SMALL_SCALE: f32 = 55.0;
pub const RIVER_WIGGLE: f32 = 0.85;
pub const RIVER_MIN_HALF_WIDTH: f32 = 2.0;
pub const RIVER_MAX_HALF_WIDTH: f32 = 3.5;
pub const RIVER_WIDTH_SCALE: f32 = 90.0;
pub const RIVER_DEPTH: i32 = 1;
pub const WATER_SURFACE_DROP: f32 = 0.12;
pub const WATER_ALPHA: f32 = 0.5;
pub const RIVER_SPAWN_CLEARANCE: f32 = 34.0;
pub const RIVER_SPAWN_FADE: f32 = 70.0;
pub const RIVER_RELIEF_MARGIN: i32 = 4;
pub const RELIEF_PLACEMENT_TRIES: i32 = 8;

// --- Landmarks: lakes, mountains and the rainbow -----------------------------------------

/// A walk this many chunks long through a river, without leaving the water, ends at a lake.
pub const LAKE_TREK_CHUNKS: i32 = 8;
/// Radius of a lake in blocks (the shore is wavy, so the real radius varies by about a quarter). The lake
/// is large: about a hundred blocks from shore to shore each way.
pub const LAKE_RADIUS: i32 = 130;
/// Gap in blocks between the player and the near shore of a new lake: `structure_distance()`, more than the
/// generated ring, so the lake appears beyond the edge of the generated land instead of in front of the player.
pub fn lake_distance() -> i32 {
    structure_distance()
}
/// The island in the middle of a lake has this share of the lake's radius (about 58 blocks for a big lake).
pub const LAKE_ISLAND_SHARE: f32 = 0.45;
/// The island is a hill: it rises this many blocks above the plain to a flat top, which the small castle stands
/// on. The top is a disc of this radius (the castle's corners reach 14 blocks); the slope runs from it down to the
/// shore, gently enough to be climbed one block at a time.
pub const ISLAND_HILL_HEIGHT: i32 = 18;
pub const ISLAND_PLATEAU_RADIUS: f32 = 18.0;
/// Half the width of the stretch of water that joins the river to the lake.
pub const LAKE_CHANNEL_HALF_WIDTH: f32 = 2.5;
/// Lakes are one block deep, like rivers.
pub const LAKE_DEPTH: i32 = RIVER_DEPTH;

/// Following a held compass this many chunks leads to a mountain.
pub const COMPASS_TREK_CHUNKS: i32 = 8;
/// The compass needle points this way: north, which is towards smaller z.
pub const COMPASS_NORTH_BEARING: f32 = -std::f32::consts::FRAC_PI_2;
/// Wide enough that the slope never exceeds one block per column, so the summit can be climbed by jumping.
pub const MOUNTAIN_RADIUS: i32 = 400;
/// Blocks the ground is raised at the summit: ten times what it once was (17). Hills are at most `HILL_MAX_HEIGHT`.
pub const MOUNTAIN_HEIGHT: i32 = 170;
/// Gap in blocks between the player and the foot of a new mountain (see `lake_distance`).
pub fn mountain_distance() -> i32 {
    structure_distance()
}
/// A mountain is steeper in parts: its slope is broken by ledges, rings of cliff `MOUNTAIN_LEDGE_HEIGHT` blocks high
/// (more than a jump clears) that run round it. Each ring has one gap, a ramp, and the gaps of one ring and the next
/// lie on opposite sides of the mountain, so the climb zigzags and cannot be run straight up. A mountain has one
/// ledge for every `MOUNTAIN_LEDGE_SPACING_HEIGHT` blocks of height, at most `MOUNTAIN_MAX_LEDGES`.
pub const MOUNTAIN_LEDGE_HEIGHT: i32 = 2;
pub const MOUNTAIN_MAX_LEDGES: i32 = 12;
pub const MOUNTAIN_LEDGE_SPACING_HEIGHT: i32 = 14;
/// Half the angle (radians) each gap in a ledge is wide, how far from the mountain's axis the gaps lie (alternately
/// to one side and the other), and the angle over which a gap blends into the cliff.
pub const MOUNTAIN_GAP_HALF_ANGLE: f32 = 0.20;
pub const MOUNTAIN_GAP_OFFSET_ANGLE: f32 = 0.50;
pub const MOUNTAIN_GAP_BLEND_ANGLE: f32 = 0.20;
/// A ledge's face is this many blocks wide (a sheer cliff, so that no aliasing of the grid leaves a way up) and its ramp at most this many.
pub const MOUNTAIN_CLIFF_WIDTH: f32 = 0.01;
pub const MOUNTAIN_RAMP_WIDTH: f32 = 10.0;
/// Ground this many blocks above the plain is covered in snow.
pub const SNOW_LINE_RISE: i32 = 120;
/// No trees grow where the ground is more than this many blocks above the plain (a pine is ten tall).
pub const TREE_LINE_RISE: i32 = 6;
/// Hills and valleys fade out over this many blocks beside a lake or a mountain, so no cliff is left behind.
pub const LANDMARK_BLEND: i32 = 16;

/// The castle in the middle of a lake's island: its walls are `Block::CastleStone`. These are its half-width
/// (the outer wall stands this many blocks from the centre), the farthest any part of it reaches from the centre
/// (the corner towers) and its height in blocks.
pub const CASTLE_WALL_HALF: i32 = 8;
pub const CASTLE_REACH: i32 = 10;
pub const CASTLE_HEIGHT: i32 = 11;
/// No tree grows within this many blocks of the castle's outer edge.
pub const CASTLE_TREE_MARGIN: i32 = 6;

/// The grand castle: a large castle of grey stone with five floors, found by following the ghostly trail that a
/// peep stone shows at night. These are the outer wall's half-width, the farthest any part reaches from the centre,
/// the height of the tallest tower and the keep's half-width, and the clearing round it where no tree grows.
pub const GRAND_WALL_HALF: i32 = 26;
pub const GRAND_REACH: i32 = 30;
pub const GRAND_HEIGHT: i32 = 27;
pub const GRAND_KEEP_HALF: i32 = 11;
pub const GRAND_TREE_MARGIN: i32 = 8;
/// Blocks between floors of the keep, and how many floors it has.
pub const GRAND_STOREY: i32 = 5;
pub const GRAND_STOREYS: i32 = 5;
/// The grand castle rises this far beyond the generated land (the nearest part is at least this far from the player).
pub fn grand_distance() -> i32 {
    structure_distance() + 20
}

/// The bone totem's two landmarks, each of which the world has only once. The well is a square of water `WELL_SIZE`
/// blocks across, sunk to the stone floor of the world (so `GROUND_TOP - STONE_LAYERS + 1` blocks deep) and ringed by
/// a one-block rim; the nearest edge of the well is `well_distance()` blocks from the player when it appears. The
/// graveyard is a walled square (`GRAVEYARD_HALF` blocks from the centre to its wall) of gravestones, with a gate
/// in its south side, and it appears `graveyard_distance()` blocks from the player.
pub const WELL_SIZE: i32 = 4;
/// The rim round the well: one block high and this many blocks wide.
pub const WELL_RIM_WIDTH: i32 = 1;
pub fn well_distance() -> i32 {
    structure_distance() + 20
}
pub const WELL_TREE_MARGIN: i32 = 6;
pub const GRAVEYARD_HALF: i32 = 12;
pub const GRAVEYARD_REACH: i32 = 14;
pub fn graveyard_distance() -> i32 {
    structure_distance() + 20
}
pub const GRAVEYARD_TREE_MARGIN: i32 = 6;
/// Gravestones stand this many blocks apart inside the graveyard.
pub const GRAVEYARD_SPACING: i32 = 3;

/// The grand castle by day is a ruin and by night whole (see `landmarks::grand_castle_is_solid`). The throne room is on
/// the second floor, west of a wall `THRONE_WALL_X` blocks from the keep's centre, and is entered by a door in that
/// wall `THRONE_DOOR_HALF` blocks either side of the middle.
pub const THRONE_WALL_X: i32 = -3;
pub const THRONE_DOOR_HALF: i32 = 1;
/// The ruined outer wall is no more than this many blocks high.
pub const RUIN_WALL_MAX: i32 = 5;

/// The ghostly trail a peep stone shows: how far ahead of the player it is drawn, the gap between its wisps (blocks)
/// and how high they float above the ground.
pub const TRAIL_LENGTH: f32 = 70.0;
pub const TRAIL_SPACING: f32 = 1.6;
pub const TRAIL_HEIGHT: f32 = 1.1;

/// Day turns to night, and night to day, behind a blackout of this many seconds in all: the screen fades to black,
/// stays black while the sky changes, and fades back.
pub const DAY_NIGHT_BLACKOUT_SECONDS: f32 = 1.0;

/// Holding a peep stone shows a rainbow once. The whole rainbow, both ends and every block of the arc, appears
/// beyond the render distance, so it is never seen to pop in: the gold lies `RAINBOW_DISTANCE_MARGIN` blocks beyond the
/// farthest chunk that is drawn, plus up to `RAINBOW_GOLD_EXTRA_DISTANCE` more (counted along either axis, as the
/// chunks are), and the arc's other end stands `RAINBOW_SPAN` blocks from the gold, sideways.
pub const RAINBOW_DISTANCE_MARGIN: f32 = 12.0;
pub const RAINBOW_GOLD_EXTRA_DISTANCE: f32 = 24.0;
pub const RAINBOW_SPAN: f32 = 90.0;
/// Places tried per frame for the rainbow's gold.
pub const RAINBOW_SPOT_TRIES: usize = 60;

/// How far (blocks, along either axis from the player) the nearest gold of a new rainbow lies: beyond every chunk that
/// is drawn, which reaches at most `render_radius() + 1` chunks from the player, plus a margin.
pub fn rainbow_distance() -> f32 {
    ((render_radius() + 1) * CHUNK_SIZE) as f32 + RAINBOW_DISTANCE_MARGIN
}
/// How high the rainbow's arc rises above the line between its ends, as a share of the span.
pub const RAINBOW_ARC_RATIO: f32 = 0.45;
/// Gold in the pile at the end of the rainbow.
pub const RAINBOW_GOLD_DROP: u32 = 6;
pub const BREAK_SECONDS_GOLD_PILE: f32 = 1.5;
/// Taking the royal armour off its stand.
pub const BREAK_SECONDS_ARMOR: f32 = 1.0;

// --- Mineshafts under the mountain --------------------------------------------------------

/// The mountain is hollow in a network of mineshafts (`mines.rs`): this many entrances at its foot, each leading
/// down a stair and then along a shaft towards the heart of the mountain.
pub const MINE_ENTRANCES: usize = 5;
/// Rings of tunnel that join the shafts, as shares of the distance from the summit to the foot, outermost first.
pub const MINE_RING_SHARES: [f32; 3] = [0.72, 0.48, 0.26];
/// A tunnel is this many blocks wide (carved within `MINE_HALF_WIDTH` of its centre line) and this many high.
pub const MINE_HALF_WIDTH: f32 = 1.6;
pub const MINE_HEIGHT: i32 = 4;
/// Solid ground left over the roof of a tunnel under the mountain, in blocks.
pub const MINE_COVER: i32 = 5;
/// Rise of a stair or ramp in the shafts per block walked (one block every two blocks: easy to hop up).
pub const MINE_SLOPE: f32 = 0.5;
/// Half the width of the chamber at each place a ring crosses a shaft, and its height.
pub const MINE_ROOM_HALF: i32 = 4;
pub const MINE_ROOM_HEIGHT: i32 = 5;

// --- The Horizen ------------------------------------------------------------------------

/// The Horizen is empty flat land at the level of the overworld's plain, broken by many small ponds. A pond lies in
/// each square of `HORIZEN_POND_CELL` blocks with `HORIZEN_POND_CHANCE` percent chance; it is round-ish, between the two
/// radii, and one block deep like a river. No pond comes within `HORIZEN_SPAWN_CLEARANCE` blocks of where the player
/// arrives.
pub const HORIZEN_POND_CELL: i32 = 22;
pub const HORIZEN_POND_CHANCE: u64 = 85;
pub const HORIZEN_POND_MIN_RADIUS: f32 = 2.5;
pub const HORIZEN_POND_MAX_RADIUS: f32 = 7.5;
pub const HORIZEN_SPAWN_CLEARANCE: f32 = 12.0;
/// Where the player arrives in the Horizen, at the centre of its dedicated arrival pool, as a column.
pub const HORIZEN_SPAWN: (i32, i32) = (CHUNK_SIZE / 2, CHUNK_SIZE / 2);
/// The player must be standing on the well's stone floor before the fall is complete.
pub const WELL_FLOOR_Y: f32 = STONE_LAYERS as f32;
/// The pool shown immediately after arriving in the Horizen.
pub const HORIZEN_ARRIVAL_POOL_RADIUS: f32 = 3.5;
/// How long the black screen lasts while the player passes through the well.
pub const HORIZEN_TRANSITION_SECONDS: f32 = 1.2;

// --- Plants, drops and fire -------------------------------------------------------------

pub const GRASS_DROP_CHANCE: u64 = 80;
pub const BUSH_BERRY_ROLLS: u32 = 5;
pub const BUSH_BERRY_CHANCE: u64 = 50;
pub const PURPLE_BUSH_ONE_IN: u64 = 10;
pub const FIRE_SECONDS: f32 = 60.0;

// --- Horizen fire tornadoes -------------------------------------------------------------

/// A new fire tornado appears in the Horizen this often during the day.
pub const HORIZEN_TORNADO_INTERVAL: f32 = 30.0;
/// The tornado begins this far from the player and travels toward the position it recorded at spawn.
pub const HORIZEN_TORNADO_SPAWN_DISTANCE: f32 = 40.0;
pub const HORIZEN_TORNADO_SPEED: f32 = 7.0;
/// Angular speed of the tornado's visible fire layers. The tornado was originally drawn at 4 radians per second,
/// then at 8; the current value is half as fast again (12). The smoke cap turns 50% faster too.
pub const HORIZEN_TORNADO_SPIN_SPEED: f32 = 12.0;
pub const HORIZEN_TORNADO_CAP_SPIN_SPEED: f32 = 2.1;
/// The tornado is a funnel: this share of the full radius at its foot, widening to the full radius at its top.
pub const HORIZEN_TORNADO_FOOT_SHARE: f32 = 0.28;
pub const HORIZEN_TORNADO_RADIUS: f32 = 1.8;
pub const HORIZEN_TORNADO_HEIGHT: f32 = 6.0;
pub const HORIZEN_TORNADO_DAMAGE: u32 = 2;
pub const HORIZEN_TORNADO_DAMAGE_INTERVAL: f32 = 1.0;

// --- Block breaking ---------------------------------------------------------------------

pub const BREAK_SECONDS_WOOD: f32 = 5.0;
pub const BREAK_SECONDS_BUSH: f32 = 3.0;
pub const BREAK_SECONDS_FOLIAGE: f32 = 2.0;
pub const BREAK_SECONDS_PLANT: f32 = 0.5;
/// Placed logs, pebbles, stones and charcoal.
pub const BREAK_SECONDS_PLACED: f32 = 0.5;
pub const BREAK_SECONDS_PLANKS: f32 = 1.5;
pub const BREAK_SECONDS_BOULDER: f32 = 4.0;
pub const BREAK_SECONDS_FORGE: f32 = 1.0;

// --- Health and damage ------------------------------------------------------------------

pub const MAX_HEALTH: u32 = 10;
pub const FIRE_DAMAGE_INTERVAL: f32 = 1.0;
pub const FIRE_DAMAGE: u32 = 1;
pub const FIRE_TOUCH_MARGIN: f32 = 0.1;
pub const BERRY_HEAL: u32 = 1;
pub const PURPLE_BERRY_DAMAGE: u32 = 1;
pub const BERRY_STEW_HEAL: u32 = 2;
pub const PURPLE_BERRY_STEW_DAMAGE: u32 = 2;
pub const VENISON_HEAL: u32 = 4;
pub const POULTRY_HEAL: u32 = 3;
pub const BEAR_MEAT_HEAL: u32 = 5;
/// Every blow the player takes is divided by this while the royal armour is worn (the result rounds up, so a blow
/// always costs at least one life).
pub const ARMOR_DAMAGE_DIVISOR: u32 = 2;

// --- Cooking and smelting ---------------------------------------------------------------

pub const COOK_SECONDS: f32 = 8.0;
pub const SMELT_SECONDS: f32 = 10.0;
pub const BELLOWS_GLOW_SECONDS: f32 = 40.0;
pub const FORGE_CLAY_DROP: u32 = 4;

// --- Stones, pebbles and rabbit-foot bonuses --------------------------------------------

pub const GROUND_PEBBLE_PER_MILLE: u64 = 8;
pub const GROUND_STONE_PER_MILLE: u64 = 4;
pub const HILL_ROCK_FACTOR: u64 = 2;
pub const GROUND_BOULDER_PER_MILLE: u64 = 2;
pub const BOULDER_OVERHANG: f32 = 0.15;
pub const BOULDER_IRON_MIN: i32 = 1;
pub const BOULDER_IRON_MAX: i32 = 3;
/// Rabbit-foot chance to turn a broken stone into a loadstone.
pub const RABBIT_FOOT_LOADSTONE_CHANCE: u64 = 20;
/// Rabbit-foot chance for a boulder to turn iron ore into gold ore.
pub const RABBIT_FOOT_GOLD_CHANCE: u64 = 20;
/// Rabbit-foot chance to add a peep stone when breaking a pebble.
pub const RABBIT_FOOT_PEEP_STONE_CHANCE: u64 = 20;
/// Rabbit-foot chance to add a holly rod when breaking foliage.
pub const RABBIT_FOOT_HOLLY_ROD_CHANCE: u64 = 20;
pub const RIVER_CLAY_PER_MILLE: u64 = 40;
pub const RIVER_PEBBLE_PER_MILLE: u64 = 20;
pub const RIVER_STONE_PER_MILLE: u64 = 15;
pub const CLAY_DROP: u32 = 2;

// --- Animals: shared movement and spawning ----------------------------------------------

pub const FLEE_DISTANCE: f32 = 10.0;
pub const FLEE_CALM_DISTANCE: f32 = 16.0;
pub const ANIMAL_ALARM_SECONDS: f32 = 4.0;
pub const ANIMAL_JUMP_SPEED: f32 = 8.0;
pub const ANIMAL_KNOCKBACK_SPEED: f32 = 13.0;
pub const ANIMAL_UPDATE_DISTANCE: f32 = 70.0;
pub const ANIMAL_DESPAWN_DISTANCE: f32 = 95.0;
pub const ANIMAL_SPAWN_MIN_DISTANCE: f32 = 22.0;
pub const ANIMAL_SPAWN_MAX_DISTANCE: f32 = 60.0;
pub const ANIMAL_SPAWN_INTERVAL: f32 = 1.5;

pub const DEER_HEALTH: i32 = 16;
pub const DEER_WALK_SPEED: f32 = 1.6;
pub const DEER_RUN_SPEED: f32 = 8.0;
pub const DEER_VENISON_DROP: u32 = 2;
pub const DEER_LEATHER_DROP: u32 = 1;
pub const MAX_DEER: usize = 6;
pub const SPAWN_WEIGHT_DEER: u64 = 30;

pub const QUAIL_HEALTH: i32 = 4;
pub const QUAIL_WALK_SPEED: f32 = 1.0;
pub const QUAIL_FLY_SPEED: f32 = 7.0;
pub const QUAIL_FLY_HEIGHT: f32 = 9.0;
pub const QUAIL_CLIMB_SPEED: f32 = 5.0;
pub const QUAIL_FLIGHT_MIN: f32 = 4.0;
pub const QUAIL_FLIGHT_MAX: f32 = 6.0;
pub const QUAIL_DESCENT_SPEED: f32 = 4.0;
pub const QUAIL_DESCENT_TIME: f32 = 2.4;
pub const QUAIL_POULTRY_DROP: u32 = 1;
pub const QUAIL_FEATHER_DROP: u32 = 3;
pub const MAX_QUAIL: usize = 9;
pub const SPAWN_WEIGHT_QUAIL: u64 = 55;

/// Twice the health bears had before (14).
pub const BEAR_HEALTH: i32 = 50;
pub const BEAR_WALK_SPEED: f32 = 2.2;
pub const BEAR_RUN_SPEED: f32 = 5.4;
pub const BEAR_NOTICE_DISTANCE: f32 = 40.0;
pub const BEAR_CHARGE_DISTANCE: f32 = 10.0;
pub const BEAR_CALM_DISTANCE: f32 = 20.0;
pub const BEAR_WANDER_SPREAD: f32 = 0.7;
pub const BEAR_REACH: f32 = 0.6;
pub const BEAR_DAMAGE: u32 = 2;
pub const BEAR_ATTACK_COOLDOWN: f32 = 1.2;
pub const BEAR_MEAT_DROP: u32 = 3;
pub const BEAR_PELT_DROP: u32 = 1;
/// Bears are a quarter as common as they were: at most one at a time (it was two) and a quarter of the old
/// spawn weight (it was 15; the weights of the other animals are unchanged).
pub const MAX_BEAR: usize = 1;
pub const SPAWN_WEIGHT_BEAR: u64 = 4;

// --- Will-o-whisps ----------------------------------------------------------------------

/// Maximum horizontal distance at which a will-o-whisp notices the player.
pub const WILL_O_WHISP_FLEE_DISTANCE: f32 = 8.0;
/// A fleeing will-o-whisp calms down beyond this distance.
pub const WILL_O_WHISP_CALM_DISTANCE: f32 = 14.0;
pub const WILL_O_WHISP_HEALTH: i32 = 1;
pub const WILL_O_WHISP_FLEE_SPEED: f32 = 6.5;
pub const WILL_O_WHISP_HALF_WIDTH: f32 = 0.24;
pub const WILL_O_WHISP_HEIGHT: f32 = 0.55;
/// Height above the terrain at which a will-o-whisp first appears.
pub const WILL_O_WHISP_SPAWN_HEIGHT: f32 = 3.0;
pub const WILL_O_WHISP_BOB_AMPLITUDE: f32 = 0.18;
pub const WILL_O_WHISP_BOB_SPEED: f32 = 2.4;
/// Many more of them, and much more often, than there once were (4 at most, weight 20).
pub const MAX_WILL_O_WHISP: usize = 12;
/// A will-o-whisp is drawn to a player with a holly rod in the hotbar from this far away, and floats towards them
/// at `WILL_O_WHISP_LURE_SPEED`. Once it is within `WILL_O_WHISP_BOND_DISTANCE` (five feet) it is bonded: it keeps
/// near the player, circling at about `WILL_O_WHISP_ORBIT_RADIUS`, for as long as the rod stays in the hotbar,
/// by night and by day. A bonded whisp more than `WILL_O_WHISP_TELEPORT_DISTANCE` away rejoins the player.
pub const WILL_O_WHISP_ATTRACT_DISTANCE: f32 = 24.0;
pub const WILL_O_WHISP_BOND_DISTANCE: f32 = 1.52;
pub const WILL_O_WHISP_LURE_SPEED: f32 = 4.0;
pub const WILL_O_WHISP_FOLLOW_SPEED: f32 = 11.0;
pub const WILL_O_WHISP_ORBIT_RADIUS: f32 = 1.4;
pub const WILL_O_WHISP_ORBIT_HEIGHT: f32 = 1.3;
pub const WILL_O_WHISP_TELEPORT_DISTANCE: f32 = 40.0;
pub const SPAWN_WEIGHT_WILL_O_WHISP: u64 = 100;

// --- Will-o-whisp storms ----------------------------------------------------------------

/// Daytime storm clouds begin gathering this long after at least one bonded whisp is following the player.
pub const WHISP_STORM_GATHER_SECONDS: f32 = 20.0;
/// Once the last bonded whisp is gone, the storm lingers for this long before clearing.
pub const WHISP_STORM_CLEAR_SECONDS: f32 = 20.0;
/// A storm darkens the complete 3D scene by 20%, just like the night overlay does.
pub const WHISP_STORM_SCENE_BRIGHTNESS: f32 = 0.80;
pub const WHISP_STORM_LIGHTNING_MIN_INTERVAL: f32 = 4.0;
pub const WHISP_STORM_LIGHTNING_MAX_INTERVAL: f32 = 11.0;
pub const WHISP_STORM_LIGHTNING_FLASH_SECONDS: f32 = 0.5;
/// A Holly Rod calls lightning onto whatever it points at within this many blocks (the ordinary reach is `REACH`).
pub const HOLLY_ROD_REACH: f32 = 64.0;
/// Every creature within this many blocks of a lightning strike is hurt: `LIGHTNING_DAMAGE` at the centre, falling
/// to half of it at the edge. Even a moose or a bear feels it (it is enough to kill a deer, a rabbit or a quail).
pub const LIGHTNING_RADIUS: f32 = 5.0;
pub const LIGHTNING_DAMAGE: i32 = 30;
/// How wide (blocks) the main trunk of a bolt is drawn, and how bright the whole screen flashes at the strike.
pub const LIGHTNING_BOLT_WIDTH: f32 = 0.34;
/// A bolt is drawn from at least this far above where it lands, even if the clouds are lower than that.
pub const LIGHTNING_MIN_HEIGHT: f32 = 24.0;
pub const LIGHTNING_SCREEN_FLASH: f32 = 0.45;
pub const WHISP_STORM_LIGHTNING_DISTANCE: f32 = 45.0;
pub const WHISP_STORM_RAIN_COUNT: usize = 140;
pub const WHISP_STORM_RAIN_RADIUS: f32 = 18.0;
pub const WHISP_STORM_RAIN_HEIGHT: f32 = 18.0;
pub const WHISP_STORM_RAIN_LENGTH: f32 = 1.5;
pub const WHISP_STORM_RAIN_SPEED: f32 = 26.0;

// --- Rabbits, moose and stews -----------------------------------------------------------

pub const RABBIT_HEALTH: i32 = 4;
pub const RABBIT_WALK_SPEED: f32 = 1.4;
pub const RABBIT_RUN_SPEED: f32 = DEER_RUN_SPEED;
pub const RABBIT_MEAT_DROP: u32 = 1;
pub const RABBIT_FOOT_CHANCE: u64 = 25;
pub const MAX_RABBIT: usize = 9;
pub const SPAWN_WEIGHT_RABBIT: u64 = 55;

pub const MOOSE_HEALTH: i32 = 50;
pub const MOOSE_WALK_SPEED: f32 = 1.5;
pub const MOOSE_RUN_SPEED: f32 = 5.8;
pub const MOOSE_MEAT_DROP: u32 = 4;
pub const MOOSE_LEATHER_DROP: u32 = 2;
pub const MOOSE_ANTLER_DROP: u32 = 1;
pub const MAX_MOOSE: usize = 2;
pub const SPAWN_WEIGHT_MOOSE: u64 = 10;
pub const MOOSE_GIVE_UP_DISTANCE: f32 = 40.0;
pub const MOOSE_DAMAGE: u32 = 2;
pub const MOOSE_ATTACK_COOLDOWN: f32 = 1.2;

pub const RABBIT_MEAT_HEAL: u32 = 2;
pub const MOOSE_MEAT_HEAL: u32 = 5;
pub const STEW_HEAL_FACTOR: u32 = 2;
pub const STEW_SECONDS: f32 = 12.0;

// --- Hunting and knockback --------------------------------------------------------------

/// Damage of one blow of each axe: stone, copper, bronze, iron. Every kind of axe hits for a different amount.
pub const AXE_DAMAGE: i32 = 6;
pub const COPPER_AXE_DAMAGE: i32 = 8;
pub const BRONZE_AXE_DAMAGE: i32 = 10;
pub const IRON_AXE_DAMAGE: i32 = 12;
pub const MALLET_DAMAGE: i32 = 6;
pub const ARROW_DAMAGE: i32 = 8;
pub const SLING_DAMAGE: i32 = 4;
pub const SLING_COOLDOWN: f32 = 0.8;
pub const SLING_SPEED: f32 = 26.0;
pub const AXE_REACH: f32 = 3.5;
pub const AXE_COOLDOWN: f32 = 0.45;
pub const MALLET_COOLDOWN: f32 = 0.6;
pub const BOW_COOLDOWN: f32 = 0.7;
pub const ARROW_SPEED: f32 = 32.0;
pub const ARROW_GRAVITY: f32 = 10.0;
pub const ARROW_LIFETIME: f32 = 6.0;
pub const PLAYER_KNOCKBACK_SPEED: f32 = 6.0;
pub const KNOCKBACK_HOP: f32 = 5.0;
pub const KNOCKBACK_DECAY: f32 = 7.0;
pub const ARROW_DROP_BACKOFF: f32 = 0.3;

/// Items lying on the ground (dropped, thrown or fallen): above this many, the oldest disappear.
pub const MAX_DROPPED_ITEMS: usize = 100;

// --- Persistence -------------------------------------------------------------------------

pub const SAVE_DIR: &str = "saves";

// --- Sky --------------------------------------------------------------------------------

pub const SUN_ELEVATION_DEGREES: f32 = 58.0;
pub const SUN_BEARING_DEGREES: f32 = 235.0;
pub const SUN_DISTANCE: f32 = 800.0;
pub const SUN_RADIUS: f32 = 38.0;
pub const CLOUD_CELL: f32 = 16.0;
pub const CLOUD_HEIGHT: f32 = 62.0;
pub const CLOUD_THICKNESS: f32 = 4.0;
pub const CLOUD_RADIUS_CELLS: i32 = 11;
pub const CLOUD_THRESHOLD: f32 = 0.56;
/// Storm clouds use the same shape and slab geometry as ordinary clouds, but this much lower threshold makes them
/// cover nearly the whole sky (ordinary clouds cover about a third of it), out to `STORM_CLOUD_RADIUS_CELLS`.
pub const STORM_CLOUD_THRESHOLD: f32 = 0.20;
pub const STORM_CLOUD_RADIUS_CELLS: i32 = 18;
pub const CLOUD_DRIFT_SPEED: f32 = 0.8;
pub const CLOUD_NOISE_CELLS: f32 = 4.0;
pub const STAR_COUNT: u32 = 120;
pub const STAR_DISTANCE: f32 = 780.0;
pub const STAR_RADIUS: f32 = 1.8;
pub const MOON_ELEVATION_DEGREES: f32 = 44.0;
pub const MOON_BEARING_DEGREES: f32 = 55.0;
pub const MOON_RADIUS: f32 = 30.0;
/// The Horizen's night stars orbit quickly instead of hanging still, but 20% slower than before.
pub const HORIZEN_STAR_ROTATION_SPEED: f32 = 1.92;
/// A night scene receives a final 20% brightness reduction, including dynamic models and water.
pub const NIGHT_SCENE_BRIGHTNESS: f32 = 0.80;

// --- Bosses: the Yeti and the Loch Ness Monster ------------------------------------------

/// Health of the two bosses (a bear has `BEAR_HEALTH`; an iron axe does `IRON_AXE_DAMAGE` a blow).
pub const YETI_HEALTH: i32 = 300;
pub const LOCH_NESS_HEALTH: i32 = 320;
/// The Yeti stands on the summit and charges a player within `YETI_NOTICE_DISTANCE`. It never strays further than
/// `YETI_LEASH` from the summit, and it is slower than a walking player.
pub const YETI_NOTICE_DISTANCE: f32 = 26.0;
pub const YETI_LEASH: f32 = 36.0;
pub const YETI_WALK_SPEED: f32 = 1.8;
pub const YETI_RUN_SPEED: f32 = 5.0;
/// Below half health the Yeti is enraged: it runs and strikes faster.
pub const YETI_ENRAGED_SPEED_FACTOR: f32 = 1.35;
pub const YETI_ENRAGED_COOLDOWN_FACTOR: f32 = 0.7;
pub const YETI_DAMAGE: u32 = 3;
pub const YETI_ATTACK_COOLDOWN: f32 = 1.5;
/// How far past its own body the Yeti's arm reaches.
pub const YETI_REACH: f32 = 1.2;
pub const YETI_PELT_DROP: u32 = 3;
pub const YETI_GOLD_DROP: u32 = 3;
/// The Yeti appears when the player comes within this many blocks of the summit (and its chunk is loaded).
pub const YETI_SPAWN_DISTANCE: f32 = 90.0;
/// The Loch Ness Monster swims only in its lake, within `LOCH_NESS_LEASH` of where it appeared. It hunts a player
/// within `LOCH_NESS_NOTICE_DISTANCE`, as near as the water lets it come, and strikes with its neck at `LOCH_NESS_REACH`.
pub const LOCH_NESS_NOTICE_DISTANCE: f32 = 55.0;
pub const LOCH_NESS_LEASH: f32 = 110.0;
pub const LOCH_NESS_WALK_SPEED: f32 = 2.4;
pub const LOCH_NESS_RUN_SPEED: f32 = 4.6;
pub const LOCH_NESS_DAMAGE: u32 = 3;
pub const LOCH_NESS_ATTACK_COOLDOWN: f32 = 1.8;
pub const LOCH_NESS_REACH: f32 = 3.2;
pub const LOCH_NESS_GOLD_DROP: u32 = 4;
pub const LOCH_NESS_IRON_DROP: u32 = 2;
/// The monster rises when the player is within the lake's radius plus this margin.
pub const LOCH_NESS_SPAWN_MARGIN: f32 = 40.0;
/// A boss's health bar shows at the top of the screen while it is this close.
pub const BOSS_BAR_DISTANCE: f32 = 110.0;
/// Bosses are simulated and kept this far from the player (a lake is 130 blocks wide, so the usual
/// `ANIMAL_UPDATE_DISTANCE` is too short for the monster).
pub const BOSS_UPDATE_DISTANCE: f32 = 150.0;
pub const BOSS_DESPAWN_DISTANCE: f32 = 190.0;

// --- The summit cyclone and the Skylands ---------------------------------------------------

/// The summit is made flat within this many blocks of its centre, as the Yeti's arena.
pub const SUMMIT_PLATEAU_RADIUS: f32 = 14.0;
/// After the Yeti dies a cyclone stands on the summit. Standing in it for `CYCLONE_SECONDS` carries the player to the
/// Skylands; the screen fades to white over the last `CYCLONE_FADE_SECONDS`.
pub const CYCLONE_SECONDS: f32 = 5.0;
pub const CYCLONE_FADE_SECONDS: f32 = 1.5;
pub const CYCLONE_RADIUS: f32 = 2.2;
pub const CYCLONE_HEIGHT: f32 = 7.0;
pub const CYCLONE_SPIN_SPEED: f32 = 9.0;
/// Seconds the white fades away after arriving by cyclone.
pub const SKY_ARRIVAL_FADE_SECONDS: f32 = 1.6;

/// The Skylands: islands on a grid of `SKY_CELL` blocks, each with its own size and height, joined to their east and
/// south neighbours by wooden bridges `SKY_BRIDGE_WIDTH` wide.
pub const SKY_CELL: i32 = 56;
pub const SKY_BASE_Y: i32 = 100;
/// Islands' tops differ by at most twice this, and their centres stray at most `SKY_ISLAND_JITTER` from the middle of
/// their cell, so two neighbours are always far enough apart (over 13 blocks of bridge) for the deck to climb a block
/// at most once every three.
pub const SKY_HEIGHT_JITTER: i32 = 3;
pub const SKY_ISLAND_JITTER: i32 = 8;
pub const SKY_ISLAND_MIN_RADIUS: f32 = 8.0;
pub const SKY_ISLAND_MAX_RADIUS: f32 = 11.0;
/// How thick the rock under an island is at its middle.
pub const SKY_ISLAND_MIN_DEPTH: i32 = 9;
pub const SKY_ISLAND_MAX_DEPTH: i32 = 15;
/// Layers of dirt on top of the rock.
pub const SKY_DIRT_LAYERS: i32 = 3;
pub const SKY_BRIDGE_WIDTH: f32 = 3.0;
/// Percent of the south bridges that exist (the east ones, and every south bridge of column 0, always do).
pub const SKY_SOUTH_BRIDGE_PERCENT: u64 = 60;
/// The first island: centre, radius and the cyclone that leads home, a few blocks from where the player arrives.
pub const SKY_SPAWN_CENTRE: (i32, i32) = (8, 8);
pub const SKY_SPAWN_RADIUS: f32 = 15.0;
pub const SKY_HOME_CYCLONE_OFFSET: f32 = 9.0;
/// Below this height the player has fallen off every island.
pub const SKY_VOID_Y: f32 = 55.0;
/// A fall off an island: the white fades in over `SKY_FALL_FADE_IN`, the player is taken to the normal world and put
/// `SKY_FALL_DROP` blocks above its spawn point, and the white fades out again over `SKY_FALL_FADE_OUT` as they fall.
pub const SKY_FALL_FADE_IN: f32 = 0.9;
pub const SKY_FALL_FADE_OUT: f32 = 3.2;
pub const SKY_FALL_DROP: f32 = 50.0;
/// Trees on an island: percent of columns that try to grow one.
pub const SKY_TREE_PERCENT: u64 = 3;
