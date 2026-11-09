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
pub const INVENTORY_SLOTS: usize = 45;
pub const HOTBAR_SLOTS: usize = 9;
/// A leather backpack, worn, adds this many rows of inventory slots (and takes them away again when it is taken off).
pub const BACKPACK_EXTRA_ROWS: usize = 3;
pub const BACKPACK_SLOTS: usize = BACKPACK_EXTRA_ROWS * HOTBAR_SLOTS;
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

pub const RIVER_BIG_SCALE: f32 = 150.0;
pub const RIVER_SMALL_SCALE: f32 = 55.0;
pub const RIVER_WIGGLE: f32 = 0.85;
/// Rivers run in parallel lanes `RIVER_LANE_SPACING` blocks apart, each bent sideways by up to about `RIVER_BEND` blocks (all by the same amount at a given point along them, so lanes never meet);
/// `RIVER_LANE_PERCENT` of the lanes carry a river, and a river dwindles away over `RIVER_SPAWN_TAPER` blocks beyond `RIVER_SPAWN_CLEARANCE`.
pub const RIVER_LANE_SPACING: f32 = 130.0;
pub const RIVER_BEND: f32 = 45.0;
pub const RIVER_LANE_PERCENT: u64 = 75;
pub const RIVER_SPAWN_TAPER: f32 = 10.0;
pub const RIVER_MIN_HALF_WIDTH: f32 = 2.0;
pub const RIVER_MAX_HALF_WIDTH: f32 = 3.5;
pub const RIVER_WIDTH_SCALE: f32 = 90.0;
pub const RIVER_DEPTH: i32 = 1;
pub const WATER_SURFACE_DROP: f32 = 0.12;
pub const WATER_ALPHA: f32 = 0.5;
pub const RIVER_SPAWN_CLEARANCE: f32 = 34.0;
pub const RIVER_RELIEF_MARGIN: i32 = 4;
pub const RELIEF_PLACEMENT_TRIES: i32 = 8;

// --- Landmarks: lakes, mountains and the rainbow -----------------------------------------

/// A walk this many chunks long through a river, without leaving the water, ends at a lake.
pub const LAKE_TREK_CHUNKS: i32 = 6;
/// Radius of a lake in blocks (the shore is wavy, so the real radius varies by about a quarter). The lake
/// is large: about a hundred blocks from shore to shore each way.
pub const LAKE_RADIUS: i32 = 100;
/// Gap in blocks between the player and the near shore of a new lake: `structure_distance()`, more than the
/// generated ring, so the lake appears beyond the edge of the generated land instead of in front of the player.
pub fn lake_distance() -> i32 {
    structure_distance()
}
/// The island in the middle of a lake has this share of the lake's radius (about 58 blocks for a big lake).
pub const LAKE_ISLAND_SHARE: f32 = 0.45;
/// The island is a hill: it rises this many blocks above the plain to a flat top, which the small castle stands
/// on. The top is a disc of this radius (the tower's corners reach 6 blocks); the slope runs from it down to the
/// shore, gently enough to be climbed one block at a time.
pub const ISLAND_HILL_HEIGHT: i32 = 18;
pub const ISLAND_PLATEAU_RADIUS: f32 = 12.0;
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
pub const MOUNTAIN_LEDGE_SPACING_HEIGHT: i32 = 6;
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

/// The silver tower: three identical towers of `Block::SilverStone` stand in the worlds (on a lake's island, in the
/// Skylands and in the Horizen). Its outer wall stands `TOWER_HALF` blocks from the centre (so it is `2 * TOWER_HALF + 1`
/// across), and `TOWER_REACH` is the farthest any part reaches from the centre. It has `TOWER_STOREYS` floors,
/// `TOWER_STOREY` blocks apart (a floor slab and four blocks of room), under a roof with battlements one block high.
pub const TOWER_HALF: i32 = 4;
pub const TOWER_REACH: i32 = 4;
pub const TOWER_STOREY: i32 = 5;
pub const TOWER_STOREYS: i32 = 5;
pub const TOWER_HEIGHT: i32 = TOWER_STOREY * TOWER_STOREYS + 1;
/// No tree grows within this many blocks of a tower's outer edge.
pub const TOWER_TREE_MARGIN: i32 = 6;
/// The ruined tower of a lake's island (`tower::ruined_tower_cell`), as it stands until the lake's Loch Ness Monster is killed:
/// its wall stumps are `1..=RUINED_WALL_MAX` blocks high and its corners `RUINED_CORNER_HEIGHT`; the stair is broken off after
/// `RUINED_STAIR_STEPS` steps (the stair climbs one block a step, so that is as high as it goes); `RUINED_RUBBLE_PERCENT` of
/// the floor's other cells hold a block of rubble. `RUINED_TOWER_SEED` seeds the ragged edges. A test checks that nothing a
/// player can hop from one block to the next gets higher than the stumps, so there is no way to the top.
pub const RUINED_WALL_MAX: i32 = 3;
pub const RUINED_CORNER_HEIGHT: i32 = 6;
pub const RUINED_STAIR_STEPS: i32 = 2;
pub const RUINED_RUBBLE_PERCENT: i32 = 12;
pub const RUINED_TOWER_SEED: i64 = 0x70E5;
/// A tower in the Horizen stands on dry cave floor: no pond comes within this many blocks of it.
pub const TOWER_POND_MARGIN: i32 = 4;
/// The player must have walked on this many different Skylands islands before the next island to appear carries a tower.
pub const SKY_TOWER_VISITS: usize = 10;

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
/// The well is a stone ring, a wooden frame of four corner posts under a roof, and a rope hanging over the water: this many
/// blocks high.
pub const WELL_HEIGHT: i32 = 7;
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

// --- The Horizen: a cave dimension ------------------------------------------------------

/// The Horizen is a solid mass of stone, hollowed by the player's way in: a round arrival chamber, a winding tunnel
/// and, at the far end, one very large cavern with the silver tower in it (`horizen.rs`). The floor is flat, at the
/// level of the overworld's plain; the roof is stone up to `HORIZEN_ROOF_Y`. Small ponds lie on the cave floor: a
/// pond lies in each square of `HORIZEN_POND_CELL` blocks with `HORIZEN_POND_CHANCE` percent chance (where there is
/// floor), is round-ish, between the two radii, and one block deep like a river. No pond comes within
/// `HORIZEN_SPAWN_CLEARANCE` blocks of where the player arrives.
pub const HORIZEN_POND_CELL: i32 = 22;
pub const HORIZEN_POND_CHANCE: u64 = 55;
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
/// The share of it spent fading to black (and the same again fading back); the player is moved to the Horizen at the black.
pub const HORIZEN_FADE_SHARE: f32 = 0.30;
/// The stone roof of the cave ends at this height (the top of the dimension).
pub const HORIZEN_ROOF_Y: i32 = GROUND_TOP + 72;
/// The least free height (blocks of air above the floor) any part of the cave has: a player is under two.
pub const HORIZEN_MIN_AIR: i32 = 3;
/// The ceiling of the cave is rough: it hangs down by up to this many blocks (never below `HORIZEN_MIN_AIR`).
pub const HORIZEN_CEILING_JAG: i32 = 2;
/// The arrival chamber: a dome of this radius and this much air at its middle.
pub const HORIZEN_CHAMBER_RADIUS: f32 = 14.0;
pub const HORIZEN_CHAMBER_HEIGHT: i32 = 12;
/// The tunnel from the arrival chamber to the great cavern: half its width, the air above its middle, how far it winds
/// to either side of a straight line and the length of one wave of that winding.
pub const HORIZEN_TUNNEL_RADIUS: f32 = 5.5;
pub const HORIZEN_TUNNEL_HEIGHT: i32 = 8;
pub const HORIZEN_TUNNEL_WIGGLE: f32 = 9.0;
pub const HORIZEN_TUNNEL_WAVELENGTH: f32 = 110.0;
/// The great cavern holding the silver tower: its centre is this far from the arrival point, it is about this wide (a
/// radius, which its outline varies by `HORIZEN_CAVERN_RAGGEDNESS` of) and it has this much air at its middle (the tower
/// is `TOWER_HEIGHT` tall).
pub const HORIZEN_CAVERN_DISTANCE: f32 = 190.0;
pub const HORIZEN_CAVERN_RADIUS: f32 = 64.0;
pub const HORIZEN_CAVERN_RAGGEDNESS: f32 = 0.14;
pub const HORIZEN_CAVERN_HEIGHT: i32 = 56;
/// Beside the long way to the great cavern, the Horizen is full of caves: one in each square of `HORIZEN_CAVE_CELL`
/// blocks (its middle somewhere in the square, a dome of radius `HORIZEN_CAVE_MIN_RADIUS` to `MAX_RADIUS` and middle height
/// `HORIZEN_CAVE_MIN_HEIGHT` to `MAX_HEIGHT`), each joined to the caves east and south of it by a tunnel of half-width
/// `HORIZEN_LINK_RADIUS` and middle height `HORIZEN_LINK_HEIGHT`. So every cave can be reached on foot from the arrival chamber,
/// which is the cave of the square it stands in.
pub const HORIZEN_CAVE_CELL: i32 = 80;
pub const HORIZEN_CAVE_MIN_RADIUS: f32 = 9.0;
pub const HORIZEN_CAVE_MAX_RADIUS: f32 = 17.0;
pub const HORIZEN_CAVE_MIN_HEIGHT: i32 = 8;
pub const HORIZEN_CAVE_MAX_HEIGHT: i32 = 14;
pub const HORIZEN_LINK_RADIUS: f32 = 4.0;
pub const HORIZEN_LINK_HEIGHT: i32 = 6;
/// A cave counts as entered when the player comes within this share of its radius of its middle.
pub const HORIZEN_CAVE_ENTER_SHARE: f32 = 0.6;
/// Every this many caves entered (the arrival chamber is the first), the one just entered has a camp: a burning log with a tin
/// pot of berry stew, red or purple, ready to eat. The fire burns this many seconds.
pub const HORIZEN_CAMP_EVERY: usize = 4;
pub const HORIZEN_CAMP_FIRE_SECONDS: f32 = 600.0;

// --- Plants, drops and fire -------------------------------------------------------------

pub const GRASS_DROP_CHANCE: u64 = 80;
pub const BUSH_BERRY_ROLLS: u32 = 5;
pub const BUSH_BERRY_CHANCE: u64 = 50;
pub const PURPLE_BUSH_ONE_IN: u64 = 10;
pub const FIRE_SECONDS: f32 = 60.0;

// --- Skylands fire tornadoes -------------------------------------------------------------

/// A fire tornado in the Skylands lasts this many seconds (it stays where it arrives until then).
pub const SKY_TORNADO_LIFETIME: f32 = 30.0;
/// A new fire tornado appears in the Skylands this often during the day: four times as often as one lasts, so there are
/// sometimes four at once.
pub const SKY_TORNADO_INTERVAL: f32 = SKY_TORNADO_LIFETIME / 4.0;
/// The tornado begins this far from the player and travels toward the position it recorded at spawn.
pub const SKY_TORNADO_SPAWN_DISTANCE: f32 = 40.0;
pub const SKY_TORNADO_SPEED: f32 = 7.0;
/// Angular speed of the tornado's visible fire layers. The tornado was originally drawn at 4 radians per second,
/// then at 8; the current value is half as fast again (12). The smoke cap turns 50% faster too.
pub const SKY_TORNADO_SPIN_SPEED: f32 = 12.0;
pub const SKY_TORNADO_CAP_SPIN_SPEED: f32 = 2.1;
/// The tornado is a funnel: this share of the full radius at its foot, widening to the full radius at its top.
pub const SKY_TORNADO_FOOT_SHARE: f32 = 0.28;
pub const SKY_TORNADO_RADIUS: f32 = 1.8;
pub const SKY_TORNADO_HEIGHT: f32 = 6.0;
pub const SKY_TORNADO_DAMAGE: u32 = 4;
/// The fire tornado that always whirls in the Phoenix's crater by day (and is gone at night): this many times the size of the
/// tornadoes that sweep across the islands, and it burns for `SKY_TORNADO_DAMAGE` a second like them.
pub const VOLCANO_TORNADO_SCALE: f32 = 2.2;
pub const SKY_TORNADO_DAMAGE_INTERVAL: f32 = 1.0;

// --- Block breaking ---------------------------------------------------------------------

pub const BREAK_SECONDS_WOOD: f32 = 5.0;
pub const BREAK_SECONDS_BUSH: f32 = 3.0;
pub const BREAK_SECONDS_FOLIAGE: f32 = 2.0;
/// Ground foliage of the jungle (`Block::Foliage`), and the same with figs on it (`Block::FigFoliage`).
pub const BREAK_SECONDS_GROUND_FOLIAGE: f32 = 1.0;
pub const BREAK_SECONDS_FIG_FOLIAGE: f32 = 2.0;
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
/// A finished pot of stew holds this many servings. Each takes one empty Wooden Bowl (right click the pot with the bowl in hand);
/// when the last is taken the pot is empty again.
pub const STEW_SERVINGS: u32 = 4;
/// One log makes this many Wooden Bowls.
pub const BOWLS_PER_LOG: u32 = 5;

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
pub const CYCLONE_SECONDS: f32 = 2.5;
pub const CYCLONE_FADE_SECONDS: f32 = 0.75;
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
/// By night the bridges are broken: a ragged stub of `SKY_BROKEN_STUB` blocks is left at each end of a deck at least
/// `SKY_BROKEN_MIN_DECK` long, and nothing at all of a shorter one, so no bridge can be crossed.
pub const SKY_BROKEN_STUB: f32 = 3.0;
pub const SKY_BROKEN_MIN_DECK: f32 = 12.0;
/// A player on a deck when it breaks is put on the nearest footing within this many blocks.
pub const SKY_BROKEN_RESCUE_REACH: i32 = 40;
/// Percent of the south bridges that exist (the east ones, and every south bridge of column 0, always do).
pub const SKY_SOUTH_BRIDGE_PERCENT: u64 = 60;
/// The first island: centre, radius and the cyclone that leads home, a few blocks from where the player arrives.
pub const SKY_SPAWN_CENTRE: (i32, i32) = (8, 8);
pub const SKY_SPAWN_RADIUS: f32 = 15.0;
/// Below this height the player has fallen off every island.
pub const SKY_VOID_Y: f32 = 55.0;
/// A fall off an island: the white fades in over `SKY_FALL_FADE_IN`, the player is taken to the jungle of the normal world
/// and put `SKY_FALL_DROP` blocks above its middle, and the white fades out again over `SKY_FALL_FADE_OUT` as they fall.
pub const SKY_FALL_FADE_IN: f32 = 0.9;
pub const SKY_FALL_FADE_OUT: f32 = 3.2;
pub const SKY_FALL_DROP: f32 = 50.0;
/// Trees on an island: percent of columns that try to grow one.
pub const SKY_TREE_PERCENT: u64 = 3;

// --- Silver towers, the salt mine, the jungle and the deer's bones ------------------------------

/// Breaking a pink salt crystal, a silver key off its stand and a relic off its plinth.
pub const BREAK_SECONDS_SALT: f32 = 1.2;
/// A cluster of ruby, sapphire or amethyst crystals, and a cave mushroom.
pub const BREAK_SECONDS_CRYSTAL: f32 = 1.5;
pub const BREAK_SECONDS_MUSHROOM: f32 = 0.5;
pub const BREAK_SECONDS_KEY: f32 = 0.6;
pub const BREAK_SECONDS_RELIC: f32 = 1.0;
/// Lumps of pink salt a crystal drops: this many to this many, inclusive (a random number each time).
pub const PINK_SALT_DROP_MIN: u32 = 8;
pub const PINK_SALT_DROP_MAX: u32 = 16;
/// A silver key lies on a stand in the middle of the chamber where the middle ring crosses the shafts numbered in
/// `MINE_KEY_SHAFTS` (one key for each of the three towers).
pub const MINE_KEY_RING: usize = 1;
pub const MINE_KEY_SHAFTS: [usize; 3] = [0, 2, 4];

/// The jungle: a round stretch of the normal world, `JUNGLE_DISTANCE_CHUNKS` chunks from the spawn point (in a direction
/// the seed decides) and `JUNGLE_RADIUS` blocks across from the middle to the edge, thick with tall trees. Its middle,
/// `JUNGLE_CLEARING_RADIUS` blocks round, is bare so that whoever falls out of the Skylands lands on the ground.
pub const JUNGLE_DISTANCE_CHUNKS: i32 = 20;
pub const JUNGLE_RADIUS: i32 = 180;
pub const JUNGLE_CLEARING_RADIUS: i32 = 34;
/// A jungle tree's trunk is this many blocks tall (inclusive range), and a chunk of jungle tries to grow this many trees.
/// The jungle temple in the middle of the clearing (`temple.rs`): a stepped pyramid `2 * TEMPLE_BASE_HALF + 1` blocks across at
/// the foot, of `TEMPLE_TIERS` tiers each `TEMPLE_TIER_HEIGHT` high, a stair `2 * TEMPLE_STAIR_HALF + 1` wide up its south
/// face and a shrine on top; `TEMPLE_HEIGHT` is as high as anything of it can reach.
pub const TEMPLE_BASE_HALF: i32 = 20;
pub const TEMPLE_TIERS: i32 = 9;
pub const TEMPLE_TIER_HEIGHT: i32 = 2;
pub const TEMPLE_STAIR_HALF: i32 = 2;
pub const TEMPLE_HEIGHT: i32 = 30;
pub const JUNGLE_TRUNK_MIN: i32 = 22;
pub const JUNGLE_TRUNK_MAX: i32 = 32;
pub const JUNGLE_TREES_MIN: i32 = 12;
pub const JUNGLE_TREES_MAX: i32 = 18;

/// Killing a deer leaves `DEER_BONE_DROP` bones one time in five (`DEER_BONE_CHANCE` percent). Three bones
/// make a set of musical pipes.
pub const DEER_BONE_CHANCE: u64 = 20;
pub const DEER_BONE_DROP: u32 = 2;

// --- Tin pan, gold boulders, the altar, Excalibur ------------------------------------------

/// Breaking river clay with a tin pan in hand also gives a gold with this chance (percent): one time in twenty, and one
/// time in five with a rabbit's foot in the hotbar.
pub const TIN_PAN_GOLD_CHANCE: u64 = 5;
pub const TIN_PAN_RABBIT_FOOT_GOLD_CHANCE: u64 = 20;
/// A boulder turned to gold by the Philosopher's Stone breaks by hand and drops this much gold.
pub const GOLD_BOULDER_GOLD_DROP: u32 = 6;
pub const BREAK_SECONDS_GOLD_BOULDER: f32 = 2.0;
/// Excalibur: the best weapon in the game.
pub const EXCALIBUR_DAMAGE: i32 = 24;
pub const EXCALIBUR_COOLDOWN: f32 = 0.35;
pub const EXCALIBUR_REACH: f32 = 4.5;

// --- Crows and the ancient altar ----------------------------------------------------------

pub const CROW_HEALTH: i32 = 4;
pub const CROW_WALK_SPEED: f32 = 1.1;
pub const CROW_FEATHER_DROP: u32 = 3;
pub const MAX_CROW: usize = 4;
pub const SPAWN_WEIGHT_CROW: u64 = 30;
/// A crow's take-off checks that the way west is clear for this many blocks at flying height.
pub const CROW_WEST_LOOKAHEAD: i32 = 24;
/// Westward blocks a crow must fly with the player following (within `CROW_FOLLOW_DISTANCE`) to land at an altar:
/// two chunks. A crow the player lets get more than `CROW_LOSE_DISTANCE` away forgets it.
pub const CROW_ALTAR_WEST_BLOCKS: f32 = 2.0 * CHUNK_SIZE as f32;
pub const CROW_FOLLOW_DISTANCE: f32 = 30.0;
pub const CROW_LOSE_DISTANCE: f32 = 45.0;
/// How far from where the crow comes down the game looks for a spot for the altar.
pub const ALTAR_SEARCH_RADIUS: i32 = 6;

// --- The Iron Quail -------------------------------------------------------------------

/// Every this many quail the player kills, `IRON_QUAILS_PER_SUMMON` Iron Quails come for them.
pub const QUAIL_KILLS_PER_IRON_QUAIL: u32 = 10;
pub const IRON_QUAILS_PER_SUMMON: u32 = 3;
pub const IRON_QUAIL_HEALTH: i32 = 120;
pub const IRON_QUAIL_RUN_SPEED: f32 = 6.8;
pub const IRON_QUAIL_DAMAGE: u32 = 2;
pub const IRON_QUAIL_ATTACK_COOLDOWN: f32 = 1.1;
pub const IRON_QUAIL_REACH: f32 = 0.9;
/// Each appears this far from the player (the three are spread round them).
pub const IRON_QUAIL_SPAWN_DISTANCE: f32 = 14.0;
/// An Iron Quail also shoots iron feathers (`IRON_FEATHER_DAMAGE` each, `IRON_FEATHER_SPEED` blocks per second) at a player it can
/// see between `IRON_QUAIL_FEATHER_MIN_RANGE` and `IRON_QUAIL_FEATHER_MAX_RANGE` blocks away, once every
/// `IRON_QUAIL_FEATHER_INTERVAL` seconds. A feather that misses lands as an item the player can pick up and throw back:
/// holding one, a click throws it (`IRON_FEATHER_THROW_COOLDOWN` apart).
pub const IRON_QUAIL_FEATHER_INTERVAL: f32 = 2.4;
pub const IRON_QUAIL_FEATHER_MIN_RANGE: f32 = 4.0;
pub const IRON_QUAIL_FEATHER_MAX_RANGE: f32 = 32.0;
pub const IRON_FEATHER_SPEED: f32 = 28.0;
pub const IRON_FEATHER_DAMAGE: i32 = 2;
pub const IRON_FEATHER_THROW_DAMAGE: i32 = 10;
pub const IRON_FEATHER_THROW_COOLDOWN: f32 = 0.45;
pub const IRON_FEATHER_SPREAD: f32 = 0.02;

// --- The ruined castle (found by wearing the crown) -----------------------------------------

/// The ruin fills the same square as the grand castle (`GRAND_REACH`). Its outer wall is `RUINS_WALL_HALF` from the
/// centre, with a gap in the south side; the sword stands in the courtyard at the centre.
pub const RUINS_WALL_HALF: i32 = 24;
pub const RUINS_WALL_HEIGHT: i32 = 9;
pub const RUINS_TOWER_HALF: i32 = 3;
pub const RUINS_GAP_HALF: i32 = 3;

// --- Pipes, birds and ghosts -----------------------------------------------------------------

/// Playing the pipes by day calls every quail and crow within this many blocks: they fly to the player, land within a few
/// blocks of them and stand still for `PIPES_CALM_SECONDS`.
pub const PIPES_CALL_RANGE: f32 = 70.0;
pub const PIPES_CALM_SECONDS: f32 = 3.0;
pub const PIPES_LAND_MIN: f32 = 2.0;
pub const PIPES_LAND_MAX: f32 = 4.5;
/// How long a ghost's words stay on the screen.
pub const GHOST_TEXT_SECONDS: f32 = 8.0;

// --- Stonehenge ------------------------------------------------------------------------------

/// Stonehenge (`stonehenge.rs`): a ring of `STONEHENGE_STONES` standing stones of radius `STONEHENGE_RADIUS`, `STONEHENGE_STONE_HEIGHT`
/// high, joined in pairs by lintels, round an altar stone, `STONEHENGE_DISTANCE_CHUNKS` chunks from the spawn point in a
/// direction the seed decides. A player who blacks out in the cave wakes in its ring.
pub const STONEHENGE_DISTANCE_CHUNKS: i32 = 5;
pub const STONEHENGE_RADIUS: f32 = 11.0;
pub const STONEHENGE_STONES: i32 = 10;
pub const STONEHENGE_STONE_HEIGHT: i32 = 5;
pub const STONEHENGE_REACH: i32 = 14;
/// No tree grows within this many blocks of the ring.
pub const STONEHENGE_TREE_MARGIN: i32 = 4;

// --- Bats -----------------------------------------------------------------------------------

/// A bat is small and flies without ever touching the ground: it bites for `BAT_DAMAGE` once every `BAT_ATTACK_COOLDOWN` seconds
/// and flutters at `BAT_SPEED` (a little slower than a walking player runs). Bats do not live in the normal world: playing the
/// Musical Pipes at night calls `BATS_PER_PIPES` of them (at most `MAX_BAT_OVERWORLD` at once) from `BAT_PIPES_MIN_DISTANCE`..`MAX`
/// blocks away, and they go with the dawn. In the Horizen's cave they are part of the wildlife: up to `MAX_BAT_CAVE`, spawning
/// every `ANIMAL_SPAWN_INTERVAL`, and they attack a player who comes within `BAT_NOTICE_DISTANCE`.
pub const BAT_HEALTH: i32 = 2;
pub const BAT_HALF_WIDTH: f32 = 0.22;
pub const BAT_HEIGHT: f32 = 0.35;
pub const BAT_SPEED: f32 = 5.5;
pub const BAT_WANDER_SPEED: f32 = 2.4;
pub const BAT_DAMAGE: u32 = 1;
pub const BAT_ATTACK_COOLDOWN: f32 = 1.2;
pub const BAT_REACH: f32 = 0.45;
pub const BAT_NOTICE_DISTANCE: f32 = 22.0;
pub const BATS_PER_PIPES: usize = 4;
pub const MAX_BAT_OVERWORLD: usize = 8;
pub const MAX_BAT_CAVE: usize = 6;
pub const BAT_PIPES_MIN_DISTANCE: f32 = 9.0;
pub const BAT_PIPES_MAX_DISTANCE: f32 = 15.0;
/// How high above the ground a bat called by the pipes first appears.
pub const BAT_SPAWN_HEIGHT: f32 = 3.0;
/// A cave bat needs at least this much open air above the floor to appear.
pub const BAT_CAVE_MIN_AIR: i32 = 5;

// --- The Wild Hunt ---------------------------------------------------------------------------

/// Blowing the Horn of the Hunt inside Stonehenge (within `STONEHENGE_REACH` of its middle) calls the Wild Hunt, the last boss:
/// ghostly warriors who appear `HUNT_SPAWN_MIN_DISTANCE`..`MAX` blocks from the player and hunt them until they are dead (or the
/// player is). They are separate creatures with a shared health bar: a swordsman, two archers, two lancers on horses,
/// `HUNT_GHOST_BEARS` ghostly bears and `HUNT_GHOST_CROWS` ghostly crows.
/// - The **swordsman** runs at the player at `HUNT_SWORDSMAN_SPEED` (slower than a sprint) and cuts for `HUNT_SWORD_DAMAGE` every
///   `HUNT_SWORD_COOLDOWN` seconds.
/// - Each **archer** keeps between `HUNT_ARCHER_MIN_RANGE` and `HUNT_ARCHER_MAX_RANGE` of the player, side-stepping, and shoots an arrow
///   every `HUNT_ARCHER_INTERVAL` seconds when it can see them (`HUNT_ARROW_DAMAGE` each, `HUNT_ARROW_SPREAD` radians of error). A Rabbit
///   Foot in the player's hotbar makes it fire only half as often (`HUNT_ARCHER_FOOT_FACTOR`).
/// - Each **lancer** rides a ghostly horse. It charges straight at the player at `HUNT_LANCER_CHARGE_SPEED` (faster than a sprint) and
///   runs them through with its lance for `HUNT_LANCER_DAMAGE`, then gallops away for `HUNT_LANCER_RETREAT_SECONDS`, stands
///   and rests for `HUNT_LANCER_REST_SECONDS`, and charges again. A charge that finds no one in `HUNT_LANCER_CHARGE_SECONDS` ends the same way.
/// - The **ghostly bears** charge like bears (`HUNT_BEAR_*`) and the **ghostly crows** fly at the player and peck (`HUNT_CROW_*`). The first
///   `HUNT_GHOST_BEARS` bears and the first `HUNT_GHOST_CROWS` crows each split in two when killed: `HUNT_SPLIT_INTO` more of the same kind
///   come (and those do not split again).
/// Excalibur does `EXCALIBUR_HUNT_FACTOR` times its damage to all of them. Beating them all wins the game (the game goes on).
pub const HUNT_SWORDSMAN_HEALTH: i32 = 200;
pub const HUNT_ARCHER_HEALTH: i32 = 120;
pub const HUNT_LANCER_HEALTH: i32 = 140;
pub const HUNT_BEAR_HEALTH: i32 = 60;
pub const HUNT_CROW_HEALTH: i32 = 10;
pub const HUNT_GHOST_BEARS: usize = 2;
pub const HUNT_GHOST_CROWS: usize = 4;
pub const HUNT_SPLIT_INTO: usize = 2;
pub const HUNT_HALF_WIDTH: f32 = 0.4;
pub const HUNT_HEIGHT: f32 = 1.9;
pub const HUNT_SPAWN_MIN_DISTANCE: f32 = 10.0;
pub const HUNT_SPAWN_MAX_DISTANCE: f32 = 16.0;
pub const HUNT_SWORDSMAN_SPEED: f32 = 6.0;
pub const HUNT_SWORD_DAMAGE: u32 = 3;
pub const HUNT_SWORD_COOLDOWN: f32 = 1.1;
pub const HUNT_SWORD_REACH: f32 = 0.9;
pub const HUNT_ARCHER_SPEED: f32 = 5.5;
pub const HUNT_ARCHER_STRAFE_SPEED: f32 = 2.6;
pub const HUNT_ARCHER_MIN_RANGE: f32 = 14.0;
pub const HUNT_ARCHER_MAX_RANGE: f32 = 28.0;
pub const HUNT_ARCHER_INTERVAL: f32 = 3.0;
pub const HUNT_ARCHER_FOOT_FACTOR: f32 = 2.0;
pub const HUNT_ARROW_SPEED: f32 = 30.0;
pub const HUNT_ARROW_DAMAGE: u32 = 2;
pub const HUNT_ARROW_SPREAD: f32 = 0.03;
pub const HUNT_ARCHER_RANGE: f32 = 45.0;
/// A lancer's horse and rider are bigger than a walking warrior.
pub const HUNT_LANCER_HALF_WIDTH: f32 = 0.6;
pub const HUNT_LANCER_HEIGHT: f32 = 2.4;
pub const HUNT_LANCER_CHARGE_SPEED: f32 = 10.5;
pub const HUNT_LANCER_RETREAT_SPEED: f32 = 9.0;
pub const HUNT_LANCER_CHARGE_SECONDS: f32 = 4.0;
pub const HUNT_LANCER_RETREAT_SECONDS: f32 = 2.5;
pub const HUNT_LANCER_REST_SECONDS: f32 = 3.0;
pub const HUNT_LANCER_DAMAGE: u32 = 3;
pub const HUNT_LANCER_COOLDOWN: f32 = 0.5;
/// The lance reaches well past the horse's head.
pub const HUNT_LANCER_REACH: f32 = 2.2;
pub const HUNT_BEAR_DAMAGE: u32 = 2;
pub const HUNT_BEAR_COOLDOWN: f32 = 1.2;
pub const HUNT_BEAR_SPEED: f32 = 5.6;
pub const HUNT_CROW_DAMAGE: u32 = 1;
pub const HUNT_CROW_COOLDOWN: f32 = 1.3;
pub const HUNT_CROW_SPEED: f32 = 7.0;
pub const HUNT_CROW_REACH: f32 = 0.5;
/// How long the victory message stays on the screen.
pub const VICTORY_TEXT_SECONDS: f32 = 14.0;

// --- Boss update: movement, the Yeti's attacks, Nessie, ravens, the Phoenix ---------------------

/// Bosses climb ledges of up to two blocks (the mountain's cliffs, rocks and tree roots) instead of getting stuck on them: their jump
/// is stronger than a deer's, and they feel for a free heading a few blocks ahead (`Animal::steer`) rather than walking into a wall.
pub const BOSS_JUMP_SPEED: f32 = 11.5;
/// How far ahead (blocks) a boss looks for something in its way, and for how long (seconds) it keeps to the way round it.
pub const BOSS_FEEL_DISTANCE: f32 = 1.6;
pub const BOSS_DETOUR_SECONDS: f32 = 0.8;
/// A boss that has made less than `BOSS_UNSTICK_PROGRESS` blocks of headway in `BOSS_UNSTICK_SECONDS` takes a few strides the other way.
pub const BOSS_UNSTICK_SECONDS: f32 = 1.2;
pub const BOSS_UNSTICK_PROGRESS: f32 = 0.6;

/// The Yeti's **smash**: within `YETI_SMASH_TRIGGER_RANGE` it may hit the ground (after `YETI_SMASH_WINDUP` seconds of rearing up), and
/// everything within `YETI_SMASH_RADIUS` blocks takes `YETI_SMASH_DAMAGE` and is thrown `YETI_SMASH_KNOCKBACK` blocks away.
pub const YETI_SMASH_RADIUS: f32 = 6.0;
pub const YETI_SMASH_DAMAGE: u32 = 4;
pub const YETI_SMASH_KNOCKBACK: f32 = 10.0;
pub const YETI_SMASH_TRIGGER_RANGE: f32 = 6.0;
pub const YETI_SMASH_WINDUP: f32 = 0.7;
pub const YETI_SMASH_COOLDOWN: f32 = 6.0;
/// The Yeti's **ice throw**, used only while the player is more than `YETI_ICE_MIN_RANGE` blocks away (and within `YETI_ICE_MAX_RANGE`):
/// a lump of ice that bursts on landing, hurting everything within `ICE_BURST_RADIUS` for `ICE_BURST_DAMAGE`.
pub const YETI_ICE_MIN_RANGE: f32 = 6.0;
pub const YETI_ICE_MAX_RANGE: f32 = 40.0;
pub const YETI_ICE_WINDUP: f32 = 0.6;
pub const YETI_ICE_COOLDOWN: f32 = 4.0;
pub const ICE_BALL_SPEED: f32 = 22.0;
pub const ICE_BALL_GRAVITY: f32 = 9.0;
pub const ICE_BURST_RADIUS: f32 = 2.5;
pub const ICE_BURST_DAMAGE: u32 = 3;

/// A smash's expanding ring and a burst's flash stay on screen this many seconds.
pub const BURST_SHOW_SECONDS: f32 = 0.6;
/// A hard knock (`Player::launch`, `Animal::launch`) throws its victim a given number of blocks: the push dies away at this rate.
pub const LAUNCH_DECAY: f32 = 2.5;

/// **Ravens** come out at night (up to `MAX_RAVEN`, weight `SPAWN_WEIGHT_RAVEN`) and go with the dawn. A raven keeps about `RAVEN_ORBIT_RADIUS`
/// blocks from the player and `RAVEN_ORBIT_HEIGHT` above them, circling; every `RAVEN_ATTACK_MIN..MAX` seconds it swoops down at
/// `RAVEN_DIVE_SPEED`, pecks for `RAVEN_DAMAGE`, and flies away again for `RAVEN_FLEE_SECONDS`. A raven called by the Loch Ness
/// Monster (`Animal::summoned`) behaves as if it were night, by day as well, and does not leave at dawn.
pub const RAVEN_HEALTH: i32 = 3;
pub const RAVEN_HALF_WIDTH: f32 = 0.25;
pub const RAVEN_HEIGHT: f32 = 0.5;
pub const RAVEN_ORBIT_RADIUS: f32 = 13.0;
pub const RAVEN_ORBIT_HEIGHT: f32 = 9.0;
pub const RAVEN_CIRCLE_SPEED: f32 = 6.5;
pub const RAVEN_DIVE_SPEED: f32 = 15.0;
pub const RAVEN_FLEE_SPEED: f32 = 9.0;
pub const RAVEN_FLEE_SECONDS: f32 = 3.0;
pub const RAVEN_DIVE_SECONDS: f32 = 2.5;
pub const RAVEN_ATTACK_MIN: f32 = 3.0;
pub const RAVEN_ATTACK_MAX: f32 = 7.0;
pub const RAVEN_DAMAGE: u32 = 1;
pub const RAVEN_REACH: f32 = 0.6;
pub const RAVEN_FEATHER_DROP: u32 = 1;
pub const MAX_RAVEN: usize = 4;
pub const SPAWN_WEIGHT_RAVEN: u64 = 60;

/// **Nessie** (the Loch Ness Monster) will not come within `LOCH_NESS_SHORE_MARGIN` blocks of the shore. While the player is more than
/// `LOCH_NESS_RAVEN_RANGE` blocks from her she calls `LOCH_NESS_RAVENS_PER_CALL` ravens (day or night) every `LOCH_NESS_RAVEN_INTERVAL`
/// seconds, up to `LOCH_NESS_MAX_RAVENS` of hers at once. Every `LOCH_NESS_ACID_COOLDOWN` seconds she taints a line of water
/// `LOCH_NESS_ACID_WIDTH` blocks wide, from herself towards the player, up to `LOCH_NESS_ACID_LENGTH` blocks long: for
/// `ACID_SECONDS` it is brown and acidic, and a player wading in it loses `ACID_DAMAGE` every `ACID_DAMAGE_INTERVAL` seconds.
/// A bow or a sling shot within `LOCH_NESS_SHOT_HEARING` blocks of her sends her off to the middle of the lake for `LOCH_NESS_FLEE_SECONDS`
/// and calls `LOCH_NESS_ALARM_RAVENS` more ravens.
pub const LOCH_NESS_SHORE_MARGIN: f32 = 8.0;
pub const LOCH_NESS_RAVEN_RANGE: f32 = 6.0;
/// She sets ravens and acid on a player anywhere within this many blocks of her (she hunts one within `LOCH_NESS_NOTICE_DISTANCE`).
pub const LOCH_NESS_RAVEN_DISTANCE: f32 = 90.0;
pub const LOCH_NESS_RAVENS_PER_CALL: usize = 2;
pub const LOCH_NESS_RAVEN_INTERVAL: f32 = 9.0;
pub const LOCH_NESS_MAX_RAVENS: usize = 4;
pub const LOCH_NESS_ACID_COOLDOWN: f32 = 11.0;
pub const LOCH_NESS_ACID_LENGTH: f32 = 36.0;
pub const LOCH_NESS_ACID_WIDTH: f32 = 3.0;
pub const LOCH_NESS_ACID_RANGE: f32 = 70.0;
pub const ACID_SECONDS: f32 = 10.0;
pub const ACID_DAMAGE: u32 = 1;
pub const ACID_DAMAGE_INTERVAL: f32 = 1.0;
pub const LOCH_NESS_SHOT_HEARING: f32 = 120.0;
pub const LOCH_NESS_FLEE_SECONDS: f32 = 10.0;
pub const LOCH_NESS_ALARM_RAVENS: usize = 4;
pub const LOCH_NESS_RAVEN_SPAWN_HEIGHT: f32 = 12.0;

// --- Sky islands, the volcano and the Phoenix -------------------------------------------------

/// A Skylands island is of one of four kinds (`skylands::IslandKind`): wooded, with ancient ruins, with a hill, or with an ancient altar
/// and bones. One island in `SKY_LARGE_ONE_IN` is the large form of its kind, `SKY_LARGE_FACTOR` times as wide.
pub const SKY_LARGE_ONE_IN: u64 = 3;
pub const SKY_LARGE_FACTOR: f32 = 1.3;
/// The hill in the middle of a hill island rises this high, over this share of the island's radius.
pub const SKY_HILL_HEIGHT: i32 = 3;
pub const SKY_HILL_SHARE: f32 = 0.7;
/// Ruins: a ring of broken wall at this share of the island's radius, and this many standing pillars inside it.
pub const SKY_RUIN_RING_SHARE: f32 = 0.55;
pub const SKY_RUIN_WALL_HEIGHT: i32 = 3;
pub const SKY_RUIN_PILLARS: usize = 5;
/// Bones lying about an altar island, per mille of the columns of its inner half.
pub const SKY_BONES_PER_MILLE: u64 = 45;
/// The giant island of the Phoenix: `PHOENIX_ISLAND_RADIUS` wide (about four times a normal island), `PHOENIX_ISLAND_DISTANCE` blocks from
/// the middle of the Skylands in a direction the seed decides. A peep stone held in the Skylands leads there until the Phoenix is dead. The
/// volcano is a cone `VOLCANO_RADIUS` wide at its foot and `VOLCANO_HEIGHT` high with a crater `VOLCANO_CRATER_RADIUS` wide.
pub const PHOENIX_ISLAND_RADIUS: f32 = 38.0;
pub const PHOENIX_ISLAND_DISTANCE: f32 = 320.0;
pub const PHOENIX_ISLAND_DEPTH: i32 = 22;
pub const VOLCANO_RADIUS: f32 = 22.0;
pub const VOLCANO_HEIGHT: i32 = 16;
pub const VOLCANO_CRATER_RADIUS: f32 = 9.0;
/// The crater's floor lies this many blocks above the island's top.
pub const VOLCANO_FLOOR: i32 = 3;
/// The Phoenix lives in the crater and appears when the player comes within `PHOENIX_SPAWN_DISTANCE` of it. It flies, never straying more
/// than `PHOENIX_LEASH` from the crater. It **swoops** (dives at the player for `PHOENIX_SWOOP_DAMAGE`) every `PHOENIX_SWOOP_INTERVAL`
/// seconds and in between hovers and throws **fireballs** (`FIREBALL_DAMAGE` at the centre of a burst of `FIREBALL_RADIUS`) every
/// `PHOENIX_FIREBALL_INTERVAL` seconds. Killing it ends the fire tornadoes for good and drops a Phoenix Feather.
pub const PHOENIX_HEALTH: i32 = 260;
pub const PHOENIX_HALF_WIDTH: f32 = 1.4;
pub const PHOENIX_HEIGHT: f32 = 2.0;
pub const PHOENIX_SPAWN_DISTANCE: f32 = 110.0;
pub const PHOENIX_LEASH: f32 = 70.0;
pub const PHOENIX_HOVER_HEIGHT: f32 = 14.0;
pub const PHOENIX_HOVER_RADIUS: f32 = 16.0;
pub const PHOENIX_CIRCLE_SPEED: f32 = 8.0;
pub const PHOENIX_SWOOP_SPEED: f32 = 17.0;
pub const PHOENIX_SWOOP_DAMAGE: u32 = 4;
pub const PHOENIX_SWOOP_INTERVAL: f32 = 7.0;
pub const PHOENIX_SWOOP_SECONDS: f32 = 3.0;
pub const PHOENIX_SWOOP_REACH: f32 = 1.4;
pub const PHOENIX_FIREBALL_INTERVAL: f32 = 3.2;
pub const PHOENIX_FIREBALL_RANGE: f32 = 60.0;
pub const FIREBALL_SPEED: f32 = 20.0;
pub const FIREBALL_RADIUS: f32 = 2.2;
pub const FIREBALL_DAMAGE: u32 = 3;
pub const PHOENIX_FEATHER_DROP: u32 = 1;

// --- Salt crystals, mine bats -----------------------------------------------------------------

/// Pink salt crystals stand on the floors of the mines only deep inside the mountain (within `MINE_SALT_MAX_SHARE` of its radius from the
/// middle), and are rare: `MINE_SALT_FLOOR_PER_MILLE` of the floor cells there.
pub const MINE_SALT_MAX_SHARE: f32 = 0.5;
pub const MINE_SALT_FLOOR_PER_MILLE: u64 = 14;
/// Bats live in the mineshafts (`MAX_BAT_MINE` at a time, never in the open air): they come out of sight in the tunnels near the player.
pub const MAX_BAT_MINE: usize = 5;
pub const BAT_MINE_MIN_DISTANCE: f32 = 8.0;
pub const BAT_MINE_MAX_DISTANCE: f32 = 26.0;

/// An Iron Quail that is keeping her distance to shoot closes in below `IRON_QUAIL_KITE_MIN` blocks' worth of room and backs off above
/// `IRON_QUAIL_KITE_MAX`: she holds between the two, strafing. A bat of the mines notices a player within `BAT_MINE_NOTICE_DISTANCE`.
pub const IRON_QUAIL_KITE_MIN: f32 = 10.0;
pub const IRON_QUAIL_KITE_MAX: f32 = 16.0;
pub const BAT_MINE_NOTICE_DISTANCE: f32 = 14.0;
pub const EXCALIBUR_HUNT_FACTOR: i32 = 3;


// --- Upgrade: the vampire bat, the king's spirits, the cursed idol, the orangutan, jaguars and flying snakes ---------------------

/// Cave bats come half as often as they did: the cave tries to bring one in only on every second wildlife spawn tick.
pub const BAT_CAVE_SPAWN_EVERY: u32 = 2;
/// A bat that strikes a player holding pink salt dies instead, and takes one lump of salt with it.
/// For every `BATS_PER_VAMPIRE` (ten) bats killed in the Horizen's caves a vampire bat boss comes out of the dark. It has one attack: it
/// circles the player at `VAMPIRE_BAT_ORBIT_RADIUS`, then swoops (`VAMPIRE_BAT_DIVE_SPEED`) for one blow and flies off again.
pub const BATS_PER_VAMPIRE: u32 = 10;
pub const VAMPIRE_BAT_HEALTH: i32 = 70;
pub const VAMPIRE_BAT_HALF_WIDTH: f32 = 0.6;
pub const VAMPIRE_BAT_HEIGHT: f32 = 0.5;
pub const VAMPIRE_BAT_DAMAGE: u32 = 3;
pub const VAMPIRE_BAT_REACH: f32 = 0.9;
pub const VAMPIRE_BAT_ORBIT_RADIUS: f32 = 8.0;
pub const VAMPIRE_BAT_ORBIT_HEIGHT: f32 = 2.4;
pub const VAMPIRE_BAT_CIRCLE_SPEED: f32 = 7.0;
pub const VAMPIRE_BAT_DIVE_SPEED: f32 = 16.0;
pub const VAMPIRE_BAT_DIVE_SECONDS: f32 = 1.5;
pub const VAMPIRE_BAT_FLEE_SECONDS: f32 = 2.0;
pub const VAMPIRE_BAT_ATTACK_MIN: f32 = 2.0;
pub const VAMPIRE_BAT_ATTACK_MAX: f32 = 3.5;
pub const VAMPIRE_BAT_SPAWN_DISTANCE: f32 = 12.0;
pub const VAMPIRE_BAT_GOLD_DROP: u32 = 3;

/// The king's castle (`kingcastle.rs`) in the Horizen's giant cave. The cave is a ragged dome `KING_CAVE_RADIUS` across from the middle
/// (the great cavern is `HORIZEN_CAVERN_RADIUS`) and `KING_CAVE_HEIGHT` high; the castle's outer wall is `KING_WALL_HALF` from the
/// middle (the grand castle's is `GRAND_WALL_HALF`), with a tower at each corner `KING_TOWER_HEIGHT` high, and the keep `KING_KEEP_HALF`
/// out. `KING_REACH` is the farthest any part of it reaches. The king's cave lies `KING_CAVE_CELLS` cells of `HORIZEN_CAVE_CELL` from the arrival point.
pub const KING_CAVE_RADIUS: f32 = 104.0;
pub const KING_CAVE_RAGGEDNESS: f32 = 0.08;
pub const KING_CAVE_HEIGHT: i32 = 66;
pub const KING_CAVE_CELLS: i32 = 3;
pub const KING_WALL_HALF: i32 = 44;
pub const KING_REACH: i32 = 48;
pub const KING_WALL_HEIGHT: i32 = 11;
pub const KING_TOWER_HALF: i32 = 4;
pub const KING_TOWER_HEIGHT: i32 = 26;
pub const KING_KEEP_HALF: i32 = 17;
pub const KING_KEEP_HEIGHT: i32 = 22;
pub const KING_GATE_HALF: i32 = 3;
/// What the ghost of the king says when the pipes are played in his castle.
/// Each `GOLD_PER_SPIRIT` gold given to him sends one spirit archer to follow the player. It fires `SPIRIT_SHOTS` arrows (each as
/// hard as a bow's arrow, every `SPIRIT_SHOOT_INTERVAL` seconds, within `SPIRIT_RANGE`) at whatever the player has hit in the last
/// `SPIRIT_MARK_SECONDS`, then vanishes. At most `MAX_SPIRIT_ARCHERS` follow at once.
pub const GOLD_PER_SPIRIT: u32 = 10;
pub const SPIRIT_SHOTS: u32 = 10;
pub const SPIRIT_SHOOT_INTERVAL: f32 = 1.1;
pub const SPIRIT_RANGE: f32 = 45.0;
pub const SPIRIT_MARK_SECONDS: f32 = 8.0;
pub const SPIRIT_HALF_WIDTH: f32 = 0.3;
pub const SPIRIT_HEIGHT: f32 = 1.7;
pub const SPIRIT_FOLLOW_RADIUS: f32 = 2.6;
pub const SPIRIT_FOLLOW_HEIGHT: f32 = 1.0;
pub const SPIRIT_SPEED: f32 = 11.0;
pub const SPIRIT_TELEPORT_DISTANCE: f32 = 30.0;
pub const MAX_SPIRIT_ARCHERS: usize = 8;

/// The boots of the sky, worn: the player runs `BOOTS_SPEED_FACTOR` times as fast and jumps `BOOTS_JUMP_DISTANCE_FACTOR` times as far.
/// A jump's length is its speed times the time in the air, and the time in the air grows with the take-off speed, so with the
/// running speed up by 1.5 the take-off speed is raised by `BOOTS_JUMP_DISTANCE_FACTOR / BOOTS_SPEED_FACTOR` (to 4/3 of normal) to make the
/// whole jump twice as long.
pub const BOOTS_SPEED_FACTOR: f32 = 1.5;
pub const BOOTS_JUMP_DISTANCE_FACTOR: f32 = 2.0;
/// While a cursed idol is in the hotbar the player deals this many times the damage and heals not at all.
pub const IDOL_DAMAGE_FACTOR: i32 = 2;

/// The jungle is four times the area it was (its radius was 90 and is doubled, so it covers four times as much ground) and lies
/// twice as far from the spawn point (it was 10 chunks out), so that it does not reach it. The cursed idol stands on a plinth in a chamber at the heart of the
/// temple (`TEMPLE_CHAMBER_HALF` out from its middle, `TEMPLE_CHAMBER_HEIGHT` high), reached by a passage in the north face.
pub const TEMPLE_CHAMBER_HALF: i32 = 3;
pub const TEMPLE_CHAMBER_HEIGHT: i32 = 5;
pub const TEMPLE_PASSAGE_HALF: i32 = 1;
pub const TEMPLE_PASSAGE_HEIGHT: i32 = 3;
/// The passage to the idol is boarded up (with `Block::CastleDoor`) this many blocks deep at its mouth until the Orangutan is dead.
pub const TEMPLE_BOARD_DEPTH: i32 = 2;

/// The orangutan boss on the temple's upper tiers. It walks (and climbs the tiers) towards a player within `ORANGUTAN_NOTICE_DISTANCE`,
/// never straying more than `ORANGUTAN_LEASH` from the top, hits for `ORANGUTAN_DAMAGE`, hurls rocks from between `ORANGUTAN_ROCK_MIN_RANGE`
/// and `MAX_RANGE` (a rock bursts for `ROCK_BURST_DAMAGE` within `ROCK_BURST_RADIUS`), and, close up, rears and smashes the ground, hurting
/// everything within `ORANGUTAN_SMASH_RADIUS` of its body and throwing it `ORANGUTAN_SMASH_KNOCKBACK` blocks.
pub const ORANGUTAN_HEALTH: i32 = 260;
pub const ORANGUTAN_HALF_WIDTH: f32 = 0.75;
pub const ORANGUTAN_HEIGHT: f32 = 2.7;
pub const ORANGUTAN_WALK_SPEED: f32 = 2.0;
pub const ORANGUTAN_RUN_SPEED: f32 = 5.0;
pub const ORANGUTAN_DAMAGE: u32 = 3;
pub const ORANGUTAN_ATTACK_COOLDOWN: f32 = 1.3;
pub const ORANGUTAN_REACH: f32 = 1.3;
pub const ORANGUTAN_NOTICE_DISTANCE: f32 = 45.0;
pub const ORANGUTAN_LEASH: f32 = 20.0;
pub const ORANGUTAN_SPAWN_DISTANCE: f32 = 70.0;
pub const ORANGUTAN_SMASH_RADIUS: f32 = 5.0;
pub const ORANGUTAN_SMASH_DAMAGE: u32 = 3;
pub const ORANGUTAN_SMASH_KNOCKBACK: f32 = 7.0;
pub const ORANGUTAN_SMASH_COOLDOWN: f32 = 5.0;
pub const ORANGUTAN_ROCK_MIN_RANGE: f32 = 7.0;
pub const ORANGUTAN_ROCK_MAX_RANGE: f32 = 38.0;
pub const ORANGUTAN_ROCK_WINDUP: f32 = 0.8;
pub const ORANGUTAN_ROCK_COOLDOWN: f32 = 3.0;
pub const ORANGUTAN_ENRAGED_FACTOR: f32 = 0.6;
pub const ORANGUTAN_GOLD_DROP: u32 = 6;
pub const ROCK_SPEED: f32 = 21.0;
pub const ROCK_GRAVITY: f32 = 11.0;
pub const ROCK_BURST_RADIUS: f32 = 1.7;
pub const ROCK_BURST_DAMAGE: u32 = 2;

/// Jaguars (jungle only, at most `MAX_JAGUAR`) sneak up on a player within `JAGUAR_NOTICE_DISTANCE` at `JAGUAR_STALK_SPEED`, crouch for
/// `JAGUAR_CROUCH_SECONDS` once within `JAGUAR_POUNCE_RANGE`, then leap (`JAGUAR_LEAP_SPEED`, `JAGUAR_LEAP_RISE`) and strike for
/// `JAGUAR_DAMAGE` if they land on the player, and slink off for `JAGUAR_RETREAT_SECONDS` before stalking again.
pub const MAX_JAGUAR: usize = 3;
pub const JAGUAR_HEALTH: i32 = 16;
pub const JAGUAR_HALF_WIDTH: f32 = 0.5;
pub const JAGUAR_HEIGHT: f32 = 0.95;
pub const JAGUAR_NOTICE_DISTANCE: f32 = 40.0;
pub const JAGUAR_STALK_SPEED: f32 = 2.0;
pub const JAGUAR_POUNCE_RANGE: f32 = 7.5;
pub const JAGUAR_CROUCH_SECONDS: f32 = 0.8;
pub const JAGUAR_LEAP_SPEED: f32 = 13.0;
pub const JAGUAR_LEAP_RISE: f32 = 6.5;
pub const JAGUAR_LEAP_SECONDS: f32 = 1.0;
pub const JAGUAR_DAMAGE: u32 = 3;
pub const JAGUAR_REACH: f32 = 0.9;
pub const JAGUAR_ATTACK_COOLDOWN: f32 = 1.0;
pub const JAGUAR_RETREAT_SECONDS: f32 = 3.5;
pub const JAGUAR_RETREAT_SPEED: f32 = 5.0;
pub const SPAWN_WEIGHT_JAGUAR: u64 = 10;

/// Flying snakes (jungle only, at most `MAX_FLYING_SNAKE`) drift through the air in slow, wide spirals and loops round a player within
/// `SNAKE_NOTICE_DISTANCE`, then slowly swoop down on them for `SNAKE_DAMAGE` and spiral away. A snake is `SNAKE_SEGMENTS` segments
/// of `SNAKE_SEGMENT_SPACING` blocks long (about seventeen blocks nose to tail), and its body follows the path its head has flown.
/// Its loop is a wide circle (`SNAKE_SPIRAL_RADIUS`, `SNAKE_LOOP_RATE` radians a second) with a smaller circle riding on it
/// (`SNAKE_COIL_RADIUS`, `SNAKE_COIL_RATE`), which is what makes the loops.
pub const MAX_FLYING_SNAKE: usize = 5;
pub const FLYING_SNAKE_HEALTH: i32 = 6;
pub const SNAKE_HALF_WIDTH: f32 = 0.5;
pub const SNAKE_HEIGHT: f32 = 0.8;
/// What can be hit of a flying snake: a box `SNAKE_HIT_HALF_WIDTH` to each side (and `SNAKE_HIT_HEIGHT` tall) round the head and the beating wings,
/// and a box `SNAKE_BODY_HIT_HALF` to each side of every other body point (`SNAKE_BODY_HIT_HALF_HEIGHT` up and down), so the whole drawn snake
/// is a target, wings included, and no gap is left along the body.
pub const SNAKE_HIT_HALF_WIDTH: f32 = 1.5;
pub const SNAKE_HIT_HEIGHT: f32 = 1.3;
pub const SNAKE_BODY_HIT_HALF: f32 = 0.6;
pub const SNAKE_BODY_HIT_HALF_HEIGHT: f32 = 0.6;
pub const SNAKE_NOTICE_DISTANCE: f32 = 45.0;
pub const SNAKE_SPIRAL_SPEED: f32 = 3.2;
pub const SNAKE_SPIRAL_RADIUS: f32 = 12.0;
pub const SNAKE_SPIRAL_HEIGHT: f32 = 6.0;
pub const SNAKE_LOOP_RATE: f32 = 0.28;
pub const SNAKE_COIL_RADIUS: f32 = 3.6;
pub const SNAKE_COIL_RATE: f32 = 0.84;
pub const SNAKE_DIVE_SPEED: f32 = 7.5;
pub const SNAKE_DIVE_SECONDS: f32 = 2.2;
pub const SNAKE_FLEE_SECONDS: f32 = 2.4;
pub const SNAKE_ATTACK_MIN: f32 = 5.0;
pub const SNAKE_ATTACK_MAX: f32 = 9.0;
pub const SNAKE_DAMAGE: u32 = 2;
pub const SNAKE_REACH: f32 = 1.1;
pub const SNAKE_SEGMENTS: usize = 64;
pub const SNAKE_SEGMENT_SPACING: f32 = 0.27;
/// Snake fangs a dead snake drops (this many to this many, inclusive).
pub const SNAKE_FANG_DROP_MIN: u32 = 2;
pub const SNAKE_FANG_DROP_MAX: u32 = 3;
pub const SPAWN_WEIGHT_FLYING_SNAKE: u64 = 12;


// --- Update: ceilings, the leaping Orangutan, Nessie's turn, parrots and bronze parrots --------------------------------------

/// A flier in the cave keeps `FLIER_CEILING_CLEARANCE` blocks of air between the top of its body and the rock above it (the goal
/// it flies for is lowered to fit), and one that has somehow ended up wedged against the ceiling is set down by `FLIER_UNWEDGE_STEP`.
pub const FLIER_CEILING_CLEARANCE: f32 = 0.6;
pub const FLIER_UNWEDGE_STEP: f32 = 0.5;
/// How far above itself (in blocks) a flier looks for a ceiling.
pub const FLIER_CEILING_LOOKUP: f32 = 12.0;

/// Nessie, on meeting the edge of her water (`LOCH_NESS_SHORE_MARGIN` from the shore), turns round for `LOCH_NESS_TURN_SECONDS` and
/// swims back towards the open middle of the lake before she tries for her goal again.
pub const LOCH_NESS_TURN_SECONDS: f32 = 2.5;

/// The Orangutan's smash is a leap: it crouches for `ORANGUTAN_LEAP_WINDUP`, springs up (the top of the arc is `ORANGUTAN_LEAP_APEX`
/// blocks above the higher of where it leaps from and where it comes down) and lands `ORANGUTAN_LEAP_LAND_GAP` blocks from the player,
/// on the side it came from, smashing the ground as it lands. It leaps at a player up to
/// `ORANGUTAN_LEAP_MAX_RANGE` away. A leap that has not come down after `ORANGUTAN_LEAP_MAX_SECONDS` smashes where it is.
pub const ORANGUTAN_LEAP_WINDUP: f32 = 0.6;
pub const ORANGUTAN_LEAP_APEX: f32 = 7.0;
pub const ORANGUTAN_LEAP_LAND_GAP: f32 = 2.6;
pub const ORANGUTAN_LEAP_MAX_RANGE: f32 = 18.0;
pub const ORANGUTAN_LEAP_MAX_SECONDS: f32 = 3.5;
/// An Orangutan more than `ORANGUTAN_LEASH` from the temple top walks back and keeps walking until it is within
/// `ORANGUTAN_HOME_RETURN_SHARE` of the leash (so it does not turn round again at the very edge).
pub const ORANGUTAN_HOME_RETURN_SHARE: f32 = 0.6;

/// Parrots: colourful quail that live in the jungle (at most `MAX_PARROT`). Every `PARROTS_PER_BRONZE_PARROT` parrots the player kills,
/// `BRONZE_PARROTS_PER_SUMMON` Bronze Parrots come for them, which fight like Iron Quails.
pub const PARROT_HEALTH: i32 = 4;
pub const MAX_PARROT: usize = 9;
pub const SPAWN_WEIGHT_PARROT: u64 = 40;
pub const PARROTS_PER_BRONZE_PARROT: u32 = 10;
pub const BRONZE_PARROTS_PER_SUMMON: u32 = 3;
pub const BRONZE_PARROT_HEALTH: i32 = 90;
pub const BRONZE_PARROT_RUN_SPEED: f32 = 6.6;
pub const BRONZE_PARROT_DAMAGE: u32 = 2;
pub const BRONZE_PARROT_ATTACK_COOLDOWN: f32 = 1.1;
pub const BRONZE_PARROT_REACH: f32 = 0.9;
pub const BRONZE_PARROT_SPAWN_DISTANCE: f32 = 14.0;
/// A Parrot gives `PARROT_FEATHER_DROP` feathers; a Bronze Parrot `BRONZE_PARROT_BRONZE_DROP` bronze and
/// `BRONZE_PARROT_IRON_FEATHER_DROP` iron feathers.
pub const PARROT_FEATHER_DROP: u32 = 3;
pub const BRONZE_PARROT_BRONZE_DROP: u32 = 2;
pub const BRONZE_PARROT_IRON_FEATHER_DROP: u32 = 3;
/// An Iron Quail no longer leaves a silver key: she drops `IRON_QUAIL_IRON_DROP` iron and `IRON_QUAIL_IRON_FEATHER_DROP` iron feathers.
pub const IRON_QUAIL_IRON_DROP: u32 = 2;
pub const IRON_QUAIL_IRON_FEATHER_DROP: u32 = 3;

/// A structure that finds something in its way is tried again `SITE_PUSH_BLOCKS` further out, up to `SITE_PUSH_STEPS` times in all.
pub const SITE_PUSH_BLOCKS: i32 = 40;
pub const SITE_PUSH_STEPS: i32 = 40;

// --- Poison arrows, jerky, falling stars, the Phoenix's return ----------------------------------

/// A poison arrow (25 ordinary arrows and a snake fang make 25 of them) is an ordinary arrow in every way, except that it does
/// `POISON_ARROW_DAMAGE_FACTOR` times the damage: an arrow that hits an animal is used up, and one that misses lands on the ground to be
/// picked up again. A bow shoots them before any other arrow.
pub const POISON_ARROW_DAMAGE_FACTOR: i32 = 2;
pub const POISON_ARROW_DAMAGE: i32 = ARROW_DAMAGE * POISON_ARROW_DAMAGE_FACTOR;
/// Arrows (and one snake fang) the poison-arrow recipe takes, which is also how many poison arrows it makes.
pub const POISON_ARROW_BATCH: u32 = 25;

/// Meat jerky: raw meat and a lump of pink salt, cured. It heals as much as the cooked meat does (a stew still heals double).
pub const JERKY_SALT_COST: u32 = 1;

/// Falling stars cross the night sky of the Skylands, one about every `FALLING_STAR_INTERVAL` seconds, and land on an island the player
/// can see (within `FALLING_STAR_RANGE` blocks, in front of them). A star starts `FALLING_STAR_START_HEIGHT` blocks above its landing
/// place and `FALLING_STAR_SLANT` blocks to one side, and falls for `FALLING_STAR_FALL_SECONDS` seconds. Its flash lasts `FALLING_STAR_FLASH_SECONDS`.
pub const FALLING_STAR_INTERVAL: f32 = 5.0;
pub const FALLING_STAR_RANGE: f32 = 220.0;
pub const FALLING_STAR_START_HEIGHT: f32 = 140.0;
pub const FALLING_STAR_SLANT: f32 = 70.0;
pub const FALLING_STAR_FALL_SECONDS: f32 = 3.2;
pub const FALLING_STAR_FLASH_SECONDS: f32 = 2.0;
pub const FALLING_STAR_TRAIL_LENGTH: f32 = 26.0;
/// The widest angle (degrees) from the player's line of sight at which an island counts as seen.
pub const FALLING_STAR_VIEW_DEGREES: f32 = 55.0;
/// A star never lands on the island the player stands on. Where it lands it leaves a white chunk of stardust (`Block::StardustPile`) that
/// stays `STARDUST_PILE_SECONDS` seconds and then fades away; broken within that time it drops `STARDUST_DROP` Stardust.
pub const STARDUST_PILE_SECONDS: f32 = 30.0;
pub const STARDUST_DROP: u32 = 1;
pub const BREAK_SECONDS_STARDUST: f32 = 1.0;

// --- The jungle's ponds, ground foliage and figs -------------------------------------------------

/// The jungle is full of small ponds. They lie on a grid of `JUNGLE_POND_CELL`-block squares; `JUNGLE_POND_PERCENT` of the squares hold one,
/// somewhere inside it, `JUNGLE_POND_MIN_RADIUS..=JUNGLE_POND_MAX_RADIUS` blocks across from the middle to the (wavy) edge. A pond is a
/// river-like cut one block into flat ground (never on a slope, and never within `JUNGLE_POND_CLEARING_MARGIN` of the bare middle).
pub const JUNGLE_POND_CELL: i32 = 16;
pub const JUNGLE_POND_PERCENT: u64 = 80;
pub const JUNGLE_POND_MIN_RADIUS: f32 = 2.0;
pub const JUNGLE_POND_MAX_RADIUS: f32 = 4.2;
pub const JUNGLE_POND_CLEARING_MARGIN: f32 = 4.0;
/// The jungle's grass is darker than the grass of the ordinary forest.
pub const JUNGLE_GRASS_COLOR: (f32, f32, f32) = (0.13, 0.36, 0.12);
/// Foliage grows on `JUNGLE_FOLIAGE_PERCENT` of the jungle's dry ground columns (not its bare middle); one clump in `JUNGLE_FIG_ONE_IN` bears figs.
pub const JUNGLE_FOLIAGE_PERCENT: u64 = 12;
pub const JUNGLE_FIG_ONE_IN: u64 = 5;
/// Picking a fig-bearing foliage (right click, like a bush: once, and the foliage stays) gives `FIG_PICK_MIN..=FIG_PICK_MAX` figs; breaking
/// it gives `FIG_BREAK_MIN..=FIG_BREAK_MAX`. Plain foliage drops nothing.
pub const FIG_PICK_MIN: u32 = 2;
pub const FIG_PICK_MAX: u32 = 4;
pub const FIG_BREAK_MIN: u32 = 2;
pub const FIG_BREAK_MAX: u32 = 3;
/// Life a fig heals when eaten.
pub const FIG_HEAL: u32 = 3;

// --- Fairies and the vial of moonlight --------------------------------------------------------------

/// Fairies flit about the islands of the Skylands (at most `MAX_FAIRY` near the player). They have `FAIRY_HEALTH` life and never do damage, and
/// now and then one plants a flower on the grass below it (every `FAIRY_PLANT_MIN..=FAIRY_PLANT_MAX` seconds). Hit one and every fairy
/// is roused (`Animal::enraged`) until the player dies or is `FAIRY_GIVE_UP_DISTANCE` away: they close in and throw the player
/// (`FAIRY_KNOCKBACK` blocks, one push every `FAIRY_ATTACK_COOLDOWN`), choosing the side that sends the player over an edge they can
/// find within `FAIRY_EDGE_SEARCH` blocks.
pub const FAIRY_HEALTH: i32 = 8;
pub const MAX_FAIRY: usize = 6;
pub const SPAWN_WEIGHT_FAIRY: u64 = 60;
pub const FAIRY_HALF_WIDTH: f32 = 0.25;
pub const FAIRY_HEIGHT: f32 = 0.6;
pub const FAIRY_WANDER_SPEED: f32 = 1.6;
pub const FAIRY_CHASE_SPEED: f32 = 6.5;
/// A fairy hovers `FAIRY_HOVER_HEIGHT` blocks over the ground, bobbing a little.
pub const FAIRY_HOVER_HEIGHT: f32 = 1.6;
pub const FAIRY_REACH: f32 = 0.8;
pub const FAIRY_ATTACK_COOLDOWN: f32 = 1.4;
pub const FAIRY_KNOCKBACK: f32 = 5.0;
pub const FAIRY_GIVE_UP_DISTANCE: f32 = 80.0;
pub const FAIRY_EDGE_SEARCH: f32 = 14.0;
pub const FAIRY_PLANT_MIN: f32 = 12.0;
pub const FAIRY_PLANT_MAX: f32 = 30.0;
/// A fairy trades with the player (right click, holding ash): one ash for one Vial of Moonlight.
pub const FAIRY_TRADE_REACH: f32 = 4.0;
/// Drinking a Vial of Moonlight raises the most life the player can have by one for `MOONLIGHT_SECONDS`, up to `MOONLIGHT_MAX_BONUS` extra hearts
/// in all. Each vial drunk starts every running timer over at `MOONLIGHT_SECONDS`.
pub const MOONLIGHT_SECONDS: f32 = 60.0;
pub const MOONLIGHT_MAX_BONUS: u32 = 10;

// --- Mountain goats and ivory knives ----------------------------------------------------------------

/// Mountain goats live on the islands of the Skylands (at most `MAX_GOAT`). They have `GOAT_HEALTH` life and leave the player alone until they
/// are hit; then one charges (until the player dies or is `GOAT_GIVE_UP_DISTANCE` away), butting for `GOAT_DAMAGE` and throwing the player
/// `GOAT_KNOCKBACK` blocks. It drops `GOAT_HORN_DROP` horn, and a horn makes `IVORY_KNIVES_PER_HORN` ivory throwing knives.
pub const GOAT_HEALTH: i32 = 30;
pub const MAX_GOAT: usize = 4;
pub const SPAWN_WEIGHT_GOAT: u64 = 50;
pub const GOAT_HALF_WIDTH: f32 = 0.45;
pub const GOAT_HEIGHT: f32 = 1.1;
pub const GOAT_WALK_SPEED: f32 = 1.3;
pub const GOAT_RUN_SPEED: f32 = 6.2;
pub const GOAT_DAMAGE: u32 = 1;
pub const GOAT_ATTACK_COOLDOWN: f32 = 1.3;
pub const GOAT_REACH: f32 = 0.6;
pub const GOAT_KNOCKBACK: f32 = 9.0;
pub const GOAT_GIVE_UP_DISTANCE: f32 = 60.0;
pub const GOAT_HORN_DROP: u32 = 1;
pub const IVORY_KNIVES_PER_HORN: u32 = 3;
/// How the skylands' creatures are kept: within `SKY_WILDLIFE_RADIUS` blocks of the player, on islands, never on the player's own island's
/// arrival stretch (the general store's ghost), and only by `SKY_WILDLIFE_MIN_DISTANCE` or more from the player.
pub const SKY_WILDLIFE_MIN_DISTANCE: f32 = 12.0;
pub const SKY_WILDLIFE_MAX_DISTANCE: f32 = 70.0;
/// An ivory knife is thrown like an iron feather (`IVORY_KNIFE_THROW_COOLDOWN` apart, `IVORY_KNIFE_DAMAGE` to what it hits) but is never
/// used up: it drops where it hits, or where it lands, and can be picked up again.
pub const IVORY_KNIFE_DAMAGE: i32 = 12;
pub const IVORY_KNIFE_SPEED: f32 = 30.0;
pub const IVORY_KNIFE_THROW_COOLDOWN: f32 = 0.4;

/// A Phoenix that has been killed rises again in its crater after this many seconds (the fire tornadoes return with it).
pub const PHOENIX_RESPAWN_SECONDS: f32 = 60.0;

/// The Horizen's ordinary caves come in four kinds (see `horizen::CaveKind`): plain, mossy with pools, with a river through them, and
/// studded with red crystals that cannot be mined. This many percent of them are each of the three special kinds (the rest are plain).
pub const HORIZEN_MOSSY_PERCENT: u64 = 24;
pub const HORIZEN_RIVER_PERCENT: u64 = 24;
pub const HORIZEN_CRYSTAL_PERCENT: u64 = 24;
/// Mossy caves hold `HORIZEN_MOSS_POOLS` pools of radius `HORIZEN_MOSS_POOL_MIN` to `HORIZEN_MOSS_POOL_MAX`, with moss `HORIZEN_MOSS_RING` blocks round each.
pub const HORIZEN_MOSS_POOLS: i32 = 3;
pub const HORIZEN_MOSS_POOL_MIN: f32 = 2.0;
pub const HORIZEN_MOSS_POOL_MAX: f32 = 4.0;
pub const HORIZEN_MOSS_RING: f32 = 3.0;
/// A river crosses its cave `HORIZEN_RIVER_HALF_WIDTH` blocks to each side of a meandering line.
pub const HORIZEN_RIVER_HALF_WIDTH: f32 = 2.0;
pub const HORIZEN_RIVER_MEANDER: f32 = 3.5;
/// Red crystals stand on this many thousandths of a crystal cave's floor (never near its middle, where the tunnels meet).
pub const HORIZEN_CRYSTAL_PER_MILLE: u64 = 55;
pub const HORIZEN_CRYSTAL_CLEAR_RADIUS: f32 = 3.5;

// --- The general store, the sword, tornado-free spawn island ------------------------------------

/// A sword hits two harder than an iron axe.
pub const SWORD_DAMAGE: i32 = IRON_AXE_DAMAGE + 2;
/// The general store's ghost stands on the Skylands' first island, this many blocks (east, south) of where the player arrives.
pub const SHOP_OFFSET: (i32, i32) = (5, 2);
/// How close (blocks) the player must be, looking at him, to trade; and how close for the pipes to be heard.
pub const SHOP_REACH: f32 = 4.5;
pub const SHOP_NEAR_RADIUS: f32 = 14.0;
/// Prices in gold. Arrows are sold `SHOP_ARROWS` at a time. Each kind of meat stew costs `SHOP_STEW_PRICE`.
pub const SHOP_ARROWS: u32 = 10;
pub const SHOP_ARROW_PRICE: u32 = 2;
pub const SHOP_SWORD_PRICE: u32 = 20;
pub const SHOP_BOW_PRICE: u32 = 10;
pub const SHOP_STEW_PRICE: u32 = 3;

// --- Newts and the Unicorn ----------------------------------------------------------------------

/// Newts live on the floor of the Horizen's mossy (overgrown, green) caves. They are harmless, have `NEWT_HEALTH` life, and always drop
/// `NEWT_EYE_DROP_MIN..=NEWT_EYE_DROP_MAX` Eyes of Newt. Up to `MAX_NEWT` are about at once.
///
/// Newts are common: while the player is within `NEWT_CAVE_REACH` blocks of a mossy cave (past its walls), the cave is kept stocked with
/// `NEWTS_PER_CAVE` of them, up to `NEWT_SPAWNS_PER_TICK` new ones on each wildlife spawn tick, placed anywhere on the cave's open floor
/// (`horizen::is_mossy_floor`) but at least `NEWT_SPAWN_MIN_DISTANCE` blocks from the player, so one walking into a green cave finds
/// a crowd of them within a few seconds.
pub const NEWT_HEALTH: i32 = 4;
pub const NEWT_HALF_WIDTH: f32 = 0.5;
pub const NEWT_HEIGHT: f32 = 0.36;
pub const NEWT_WALK_SPEED: f32 = 0.8;
pub const NEWT_EYE_DROP_MIN: u32 = 1;
pub const NEWT_EYE_DROP_MAX: u32 = 2;
/// Life an Eye of Newt heals when eaten, and how much bigger than the first version a newt is drawn (its hit box is twice as big too).
pub const NEWT_EYE_HEAL: u32 = 1;
pub const NEWT_MODEL_SCALE: f32 = 2.0;
pub const MAX_NEWT: usize = 24;
pub const NEWTS_PER_CAVE: usize = 10;
pub const NEWT_SPAWNS_PER_TICK: usize = 3;
pub const NEWT_SPAWN_MIN_DISTANCE: f32 = 6.0;
pub const NEWT_CAVE_REACH: f32 = 40.0;

/// The Unicorn comes the first time the player carries the Cursed Idol in the hotbar in the normal world outside the jungle (nothing
/// needs to be clicked), `UNICORN_SPAWN_DISTANCE` blocks away. It
/// will not attack while the player is in the jungle; once they leave it, it charges at `UNICORN_RUN_SPEED` (a little slower than a sprint),
/// hits for `UNICORN_DAMAGE` every `UNICORN_ATTACK_COOLDOWN` seconds and never gives up. It drops `UNICORN_GOLD_DROP` gold.
pub const UNICORN_HEALTH: i32 = 240;
pub const UNICORN_HALF_WIDTH: f32 = 0.85;
pub const UNICORN_HEIGHT: f32 = 1.9;
pub const UNICORN_WALK_SPEED: f32 = 1.6;
pub const UNICORN_RUN_SPEED: f32 = 6.6;
pub const UNICORN_DAMAGE: u32 = 3;
pub const UNICORN_ATTACK_COOLDOWN: f32 = 1.1;
pub const UNICORN_GOLD_DROP: u32 = 12;
pub const UNICORN_SPAWN_DISTANCE: f32 = 14.0;

// --- The Great Bear -----------------------------------------------------------------------------

/// Killing a bear while wearing the pelt cloak brings out the Great Bear (`AnimalKind::BearBoss`), `BEAR_BOSS_SPAWN_DISTANCE` blocks from the
/// player; there is never more than one. It charges at `BEAR_BOSS_RUN_SPEED` (a little slower than a sprint) from any distance, hits for
/// `BEAR_BOSS_DAMAGE` every `BEAR_BOSS_ATTACK_COOLDOWN` seconds (quicker, and faster on its feet, below half health) and never gives up
/// until the player dies, when it has won. The cloak does not calm it.
///
/// Every blow that lands on it calls every bear in the world to the player for `BEAR_BOSS_RAGE_SECONDS` (each blow starts the time again):
/// bears that are already out, however far (they are kept and moved while the rage lasts), and any that spawn during it. Those bears are
/// provoked, so the pelt cloak does not calm them either.
pub const BEAR_BOSS_HEALTH: i32 = 220;
pub const BEAR_BOSS_HALF_WIDTH: f32 = 0.95;
pub const BEAR_BOSS_HEIGHT: f32 = 2.4;
pub const BEAR_BOSS_WALK_SPEED: f32 = 2.6;
pub const BEAR_BOSS_RUN_SPEED: f32 = 6.2;
pub const BEAR_BOSS_ENRAGED_SPEED_FACTOR: f32 = 1.15;
pub const BEAR_BOSS_REACH: f32 = 0.9;
pub const BEAR_BOSS_DAMAGE: u32 = 3;
pub const BEAR_BOSS_ATTACK_COOLDOWN: f32 = 1.1;
pub const BEAR_BOSS_ENRAGED_COOLDOWN_FACTOR: f32 = 0.75;
pub const BEAR_BOSS_MEAT_DROP: u32 = 6;
pub const BEAR_BOSS_PELT_DROP: u32 = 3;
pub const BEAR_BOSS_GOLD_DROP: u32 = 6;
pub const BEAR_BOSS_SPAWN_DISTANCE: f32 = 18.0;
pub const BEAR_BOSS_RAGE_SECONDS: f32 = 30.0;
/// How much darker than a bear's the Great Bear's fur is, and how much bigger it is drawn.
pub const BEAR_BOSS_FUR_SHADE: f32 = 0.55;
pub const BEAR_BOSS_MODEL_SCALE: f32 = 1.7;

// --- The crystal ball, the unicorn sword, crystals, mushrooms, crawfish and the dragon ----------------

/// Stardust (what the Skylands' falling stars leave) is the moon dust that `CRYSTAL_BALL_STARDUST` of makes a **Crystal Ball**. A left click with it
/// in hand turns day to night and night to day (behind the usual blackout); it does nothing in the Horizen, which is always day.
pub const CRYSTAL_BALL_STARDUST: u32 = 10;

/// The Unicorn drops `UNICORN_HORN_DROP` Unicorn Horn, which is crafted (with a leather cord) into a **Unicorn Sword**: it hits harder than
/// Excalibur, with the same reach and swing, but does no extra damage to the Wild Hunt.
pub const UNICORN_HORN_DROP: u32 = 1;
pub const UNICORN_SWORD_DAMAGE: i32 = EXCALIBUR_DAMAGE + 8;
pub const UNICORN_SWORD_COOLDOWN: f32 = EXCALIBUR_COOLDOWN;
pub const UNICORN_SWORD_REACH: f32 = EXCALIBUR_REACH;

/// Crystal caves are studded with ruby, sapphire and amethyst clusters. Broken by hand each drops `CRYSTAL_DROP_MIN..=CRYSTAL_DROP_MAX`
/// crystals of its colour. A sling shoots a crystal (any colour, rubies first) before it uses a pebble, for `CRYSTAL_DAMAGE_FACTOR` times
/// the damage.
pub const CRYSTAL_DROP_MIN: u32 = 1;
pub const CRYSTAL_DROP_MAX: u32 = 3;
pub const CRYSTAL_DAMAGE_FACTOR: i32 = 4;

/// A fifth kind of cave, the **mushroom cave** (`horizen::CaveKind::Mushroom`): this many percent of the Horizen's ordinary caves (taken from
/// the plain ones, so the mossy, river and crystal caves stay where they were). Mushrooms stand on this many thousandths of its open floor,
/// never near its middle. Eating one costs `MUSHROOM_DAMAGE` life and makes the player `MUSHROOM_SPEED_FACTOR` times as fast for
/// `MUSHROOM_BOOST_SECONDS`.
pub const HORIZEN_MUSHROOM_PERCENT: u64 = 14;
pub const HORIZEN_MUSHROOM_PER_MILLE: u64 = 110;
pub const HORIZEN_MUSHROOM_CLEAR_RADIUS: f32 = 3.0;
pub const MUSHROOM_DAMAGE: u32 = 1;
pub const MUSHROOM_SPEED_FACTOR: f32 = 2.0;
pub const MUSHROOM_BOOST_SECONDS: f32 = 15.0;

/// **Crawfish.** Small ones (`CRAWFISH_HEALTH`) live in the normal world's rivers, wander in the water and never attack. One in
/// `CRAWFISH_FRAGMENT_PERCENT` percent leaves a Shell Fragment. **Big crawfish** (`BIG_CRAWFISH_HEALTH`) live in the Horizen's river caves,
/// leave the player alone until hurt, then fight back (`BIG_CRAWFISH_DAMAGE` every `BIG_CRAWFISH_ATTACK_COOLDOWN`), and always drop
/// `BIG_CRAWFISH_FRAGMENT_MIN..=MAX` fragments. Every `BIG_CRAWFISH_KILLS_PER_GIANT` big crawfish killed brings out the **Giant Crawfish**
/// (`GIANT_CRAWFISH_HEALTH`), which always fights and drops `GIANT_CRAWFISH_FRAGMENT_MIN..=MAX` fragments. All of them keep to the water.
/// `SHELL_ARMOR_FRAGMENTS` fragments make a Shell Armor, which takes `SHELL_ARMOR_REDUCTION` life off every blow (but never more than half).
pub const CRAWFISH_HEALTH: i32 = 4;
pub const CRAWFISH_HALF_WIDTH: f32 = 0.3;
pub const CRAWFISH_HEIGHT: f32 = 0.3;
pub const CRAWFISH_WALK_SPEED: f32 = 0.9;
pub const CRAWFISH_FRAGMENT_PERCENT: u64 = 20;
pub const MAX_CRAWFISH: usize = 10;
pub const CRAWFISH_SPAWNS_PER_TICK: usize = 2;
pub const BIG_CRAWFISH_HEALTH: i32 = 20;
pub const BIG_CRAWFISH_HALF_WIDTH: f32 = 0.7;
pub const BIG_CRAWFISH_HEIGHT: f32 = 0.8;
pub const BIG_CRAWFISH_WALK_SPEED: f32 = 1.2;
pub const BIG_CRAWFISH_RUN_SPEED: f32 = 3.4;
pub const BIG_CRAWFISH_DAMAGE: u32 = 2;
pub const BIG_CRAWFISH_ATTACK_COOLDOWN: f32 = 1.2;
pub const BIG_CRAWFISH_REACH: f32 = 1.2;
pub const BIG_CRAWFISH_FRAGMENT_MIN: u32 = 1;
pub const BIG_CRAWFISH_FRAGMENT_MAX: u32 = 2;
pub const BIG_CRAWFISH_GIVE_UP_DISTANCE: f32 = 30.0;
pub const BIG_CRAWFISH_KILLS_PER_GIANT: u32 = 10;
pub const BIG_CRAWFISH_PER_CAVE: usize = 3;
pub const MAX_BIG_CRAWFISH: usize = 8;
pub const BIG_CRAWFISH_SPAWNS_PER_TICK: usize = 2;
pub const BIG_CRAWFISH_CAVE_REACH: f32 = 40.0;
pub const BIG_CRAWFISH_SPAWN_MIN_DISTANCE: f32 = 6.0;
pub const GIANT_CRAWFISH_HEALTH: i32 = 120;
pub const GIANT_CRAWFISH_HALF_WIDTH: f32 = 1.4;
pub const GIANT_CRAWFISH_HEIGHT: f32 = 1.8;
pub const GIANT_CRAWFISH_WALK_SPEED: f32 = 1.6;
pub const GIANT_CRAWFISH_RUN_SPEED: f32 = 4.2;
pub const GIANT_CRAWFISH_DAMAGE: u32 = 3;
pub const GIANT_CRAWFISH_ATTACK_COOLDOWN: f32 = 1.1;
pub const GIANT_CRAWFISH_REACH: f32 = 1.6;
pub const GIANT_CRAWFISH_FRAGMENT_MIN: u32 = 10;
pub const GIANT_CRAWFISH_FRAGMENT_MAX: u32 = 20;
pub const GIANT_CRAWFISH_SPAWN_DISTANCE: f32 = 14.0;
/// How much bigger than a crawfish the big and the giant one are drawn.
pub const BIG_CRAWFISH_MODEL_SCALE: f32 = 2.4;
pub const GIANT_CRAWFISH_MODEL_SCALE: f32 = 5.0;
pub const SHELL_ARMOR_FRAGMENTS: u32 = 50;
pub const SHELL_ARMOR_REDUCTION: u32 = 1;

/// **The Dragon**, the biggest boss in the game. After `DRAGON_SWITCHES` day/night switches (by stew or crystal ball, anywhere) one comes
/// to the Skylands, `DRAGON_SPAWN_DISTANCE` blocks from the player and `DRAGON_SPAWN_HEIGHT` above them; killing it starts the count over.
/// It is always in the air and never lands. It circles `DRAGON_HOVER_RADIUS` out and `DRAGON_HOVER_HEIGHT` up, and has two attacks:
/// it **breathes fire** (a stream of `DRAGON_BREATH_FLAMES` flames over `DRAGON_BREATH_SECONDS`, each `DRAGON_FLAME_DAMAGE`, every
/// `DRAGON_BREATH_INTERVAL`), and it **swoops** down to strike the player for `DRAGON_SWOOP_DAMAGE`, throwing them `DRAGON_SWOOP_KNOCKBACK`
/// blocks. Below half health it is quicker. It drops `DRAGON_GOLD_DROP` gold.
pub const DRAGON_HEALTH: i32 = 600;
pub const DRAGON_HALF_WIDTH: f32 = 3.0;
pub const DRAGON_HEIGHT: f32 = 3.0;
/// Its hit box is wider than its collision box: the wings count.
pub const DRAGON_HIT_HALF_WIDTH: f32 = 5.5;
pub const DRAGON_HIT_HEIGHT: f32 = 5.0;
pub const DRAGON_MODEL_SCALE: f32 = 6.0;
pub const DRAGON_SWITCHES: u32 = 10;
pub const DRAGON_SPAWN_DISTANCE: f32 = 60.0;
pub const DRAGON_SPAWN_HEIGHT: f32 = 24.0;
pub const DRAGON_HOVER_HEIGHT: f32 = 15.0;
pub const DRAGON_HOVER_RADIUS: f32 = 26.0;
pub const DRAGON_CIRCLE_SPEED: f32 = 9.0;
pub const DRAGON_SWOOP_SPEED: f32 = 24.0;
pub const DRAGON_SWOOP_DAMAGE: u32 = 5;
pub const DRAGON_SWOOP_REACH: f32 = 3.0;
pub const DRAGON_SWOOP_KNOCKBACK: f32 = 22.0;
pub const DRAGON_SWOOP_INTERVAL: f32 = 9.0;
pub const DRAGON_SWOOP_SECONDS: f32 = 3.5;
pub const DRAGON_BREATH_INTERVAL: f32 = 6.0;
pub const DRAGON_BREATH_SECONDS: f32 = 1.4;
pub const DRAGON_BREATH_FLAMES: u32 = 10;
pub const DRAGON_BREATH_RANGE: f32 = 55.0;
pub const DRAGON_BREATH_MIN_RANGE: f32 = 8.0;
pub const DRAGON_FLAME_DAMAGE: u32 = 1;
pub const DRAGON_FLAME_SPEED: f32 = 30.0;
pub const DRAGON_FLAME_RADIUS: f32 = 2.2;
pub const DRAGON_FLAME_SPREAD: f32 = 0.10;
pub const DRAGON_GOLD_DROP: u32 = 60;

/// The vampire bat is drawn `VAMPIRE_BAT_MODEL_SCALE` times a bat's size (it was 3.4). Out of its dive it flies for the middle of a passage:
/// at least `VAMPIRE_BAT_CEILING_MARGIN` blocks under the ceiling, and pushed at up to `VAMPIRE_BAT_CENTRE_SPEED` away from walls it sees
/// within `VAMPIRE_BAT_CENTRE_LOOK` blocks.
pub const VAMPIRE_BAT_MODEL_SCALE: f32 = 2.8;
pub const VAMPIRE_BAT_CEILING_MARGIN: f32 = 1.2;
pub const VAMPIRE_BAT_CENTRE_SPEED: f32 = 11.0;
pub const VAMPIRE_BAT_CENTRE_LOOK: f32 = 8.0;
/// A wall nearer than this (blocks) to a vampire bat stops its goal pulling it sideways into the rock.
pub const VAMPIRE_BAT_WALL_NEAR: f32 = 2.5;
