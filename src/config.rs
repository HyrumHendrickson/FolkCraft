//! Tunable game constants, grouped by the system that consumes them.

// --- World and rendering --------------------------------------------------------------

pub const CHUNK_SIZE: i32 = 20;
/// Tall enough for the mountain (`MOUNTAIN_HEIGHT`) and the sky above its summit.
pub const WORLD_HEIGHT: i32 = 180;
pub const STONE_LAYERS: i32 = 1;
pub const DIRT_LAYERS: i32 = 5;
/// Y of the highest dirt block; players stand at `GROUND_TOP + 1`.
pub const GROUND_TOP: i32 = STONE_LAYERS + DIRT_LAYERS - 1;

/// Chunks within this many chunks of the player are rendered.
pub const RENDER_RADIUS: i32 = 3;
/// Chunks are generated one ring beyond the render radius so outer mesh edges cull correctly.
pub const GEN_RADIUS: i32 = RENDER_RADIUS + 1;
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
/// Gap in blocks between the player and the near shore of a new lake. It is more than `GEN_RADIUS` chunks, so
/// the lake appears beyond the edge of the generated land instead of in front of the player.
pub const LAKE_DISTANCE: i32 = (GEN_RADIUS + 1) * CHUNK_SIZE;
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
/// Gap in blocks between the player and the foot of a new mountain (see `LAKE_DISTANCE`).
pub const MOUNTAIN_DISTANCE: i32 = (GEN_RADIUS + 1) * CHUNK_SIZE;
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
pub const GRAND_DISTANCE: i32 = (GEN_RADIUS + 1) * CHUNK_SIZE + 20;

/// The bone totem's two landmarks, each of which the world has only once. The well is a square of water `WELL_SIZE`
/// blocks across, sunk to the stone floor of the world (so `GROUND_TOP - STONE_LAYERS + 1` blocks deep) and ringed by
/// a one-block rim; the nearest edge of the well is `WELL_DISTANCE` blocks from the player when it appears. The
/// graveyard is a walled square (`GRAVEYARD_HALF` blocks from the centre to its wall) of gravestones, with a gate
/// in its south side, and it appears `GRAVEYARD_DISTANCE` blocks from the player.
pub const WELL_SIZE: i32 = 4;
/// The rim round the well: one block high and this many blocks wide.
pub const WELL_RIM_WIDTH: i32 = 1;
pub const WELL_DISTANCE: i32 = (GEN_RADIUS + 1) * CHUNK_SIZE + 20;
pub const WELL_TREE_MARGIN: i32 = 6;
pub const GRAVEYARD_HALF: i32 = 12;
pub const GRAVEYARD_REACH: i32 = 14;
pub const GRAVEYARD_DISTANCE: i32 = (GEN_RADIUS + 1) * CHUNK_SIZE + 20;
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

/// Holding a peep stone shows a rainbow once. Its gold lies this far from the player (min, max), in blocks.
pub const RAINBOW_GOLD_MIN_DISTANCE: f32 = 26.0;
pub const RAINBOW_GOLD_MAX_DISTANCE: f32 = 36.0;
/// Places tried per frame for the rainbow's gold, and how far to the side of the player its arc begins.
pub const RAINBOW_SPOT_TRIES: usize = 30;
pub const RAINBOW_START_OFFSET: f32 = 5.0;
/// How high the rainbow's arc rises above the line between its ends, as a share of the span.
pub const RAINBOW_ARC_RATIO: f32 = 0.45;
/// Gold in the pile at the end of the rainbow.
pub const RAINBOW_GOLD_DROP: u32 = 6;
pub const BREAK_SECONDS_GOLD_PILE: f32 = 1.5;
/// Taking the royal armour off its stand.
pub const BREAK_SECONDS_ARMOR: f32 = 1.0;

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
pub const CLOUD_DRIFT_SPEED: f32 = 0.8;
pub const CLOUD_NOISE_CELLS: f32 = 4.0;
pub const STAR_COUNT: u32 = 120;
pub const STAR_DISTANCE: f32 = 780.0;
pub const STAR_RADIUS: f32 = 1.8;
pub const MOON_ELEVATION_DEGREES: f32 = 44.0;
pub const MOON_BEARING_DEGREES: f32 = 55.0;
pub const MOON_RADIUS: f32 = 30.0;
/// The Horizen's night stars orbit quickly instead of hanging still.
pub const HORIZEN_STAR_ROTATION_SPEED: f32 = 2.4;
/// A night scene receives a final 20% brightness reduction, including dynamic models and water.
pub const NIGHT_SCENE_BRIGHTNESS: f32 = 0.80;
