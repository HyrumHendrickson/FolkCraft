# Folk Craft: guide for agents (and humans) who change the code

Folk Craft is a small Minecraft-style voxel game in Rust using [macroquad](https://macroquad.rs) 0.4. This file is the technical
reference: how to build and test, where each rule lives, the conventions to keep, the save format, and how to add an item, block,
recipe, creature, structure or shop ware. It describes the game **as it is now**; it is not a changelog. When you change a rule,
change the sentence that states it. Players should read `README.md` and the in-game Help instead.

## The short version

1. Read the section of this file and the module that owns what you are changing (see the code map).
2. Put gameplay rules in the logic modules and tunable numbers in `config.rs`; keep input and drawing in the thin UI wrappers.
3. Add or update unit tests beside the logic. Tests must not need a window or GPU.
4. Run `cargo build` (no warnings) and `cargo test` (every test passes; the suite takes about six minutes). Check `git diff --check`.
5. Update this file (the section that states the rule) and the in-game help (`help.rs`, which builds its text from `RECIPES` and the
   `config.rs` constants).
6. Keep old saves loading: item and block ids and the save records are a plain-text format that must stay readable.

## Build, run, test

```sh
cargo run            # play (opens a 1280x720 window; saves go to ./saves relative to the CWD)
cargo test           # all unit tests; no window or GPU needed
cargo build          # must stay free of warnings
cargo test <filter>  # run part of the suite (for example `cargo test kingcastle`)
```

- Dependencies: only `macroquad = "0.4"`. Avoid adding crates without a strong reason.
- `[profile.dev] opt-level = 2` is intentional (mesh building is too slow unoptimised).
- On Linux, `macroquad` needs ALSA headers to compile (`libasound2-dev` on Debian/Ubuntu).
- Tests construct `Game`/`World` directly and never touch the GL context. Keep it that way: put logic in plain functions or methods on
  data types and unit-test those; keep macroquad drawing and input calls in thin wrappers (`hud.rs`, `play.rs`, `menu.rs`, `effects.rs`).
- Balance tests compute their expectations from `config.rs` (a bear dies in `ceil(BEAR_HEALTH / AXE_DAMAGE)` blows) instead of hard-coding
  numbers: do the same.
- Tests that need a structure near the player put it there with the test-only `World::replace_sites` (it changes the landmarks and makes
  every loaded chunk again); the real ones are hundreds of blocks away.

### The change loop

1. Read the relevant section of this file and the owning module before editing.
2. Put gameplay rules in the logic modules (`block.rs`, `item.rs`, `inventory.rs`, `recipe.rs`, `world.rs`, `animal.rs`, `game.rs`).
3. Add tunable numbers to the matching section of `config.rs`; keep constants grouped by system and document non-obvious values.
4. Add or update unit tests beside the logic you changed.
5. Run `cargo build`, `cargo test`, and inspect `git diff --check` before handing off.

The main gameplay call path is `play::play_screen` -> input/raycast -> `Game` action methods -> world/inventory/entity state -> item
pickup/fire/animal updates -> rendering/HUD. Before adding a helper, search for an existing rule owner: `Game::held_item()`,
`Block::can_break_with`, `ItemKind` conversion methods, `Inventory` mutation methods, `World::set_block`/`break_block`, `Animal::update`.

Data flow per frame in `play::play_screen`: input (`E`, hotbar keys, look) -> chunk streaming and meshing -> player physics -> raycast ->
`handle_attacking` (axe swing, bow shot) -> `handle_breaking` (`Game::update_mining`) -> `handle_placing` -> `Game::update_landmarks` (the
rainbow, the graveyard ghost, the well) -> `Game::update_items` -> `Game::burn_fires` -> `Game::update_king_dungeon` ->
`Game::update_animals` -> `Game::update_arrows` -> `Game::update_health` (fire and poison ivy) -> autosave -> `render_world` (chunks, items,
flames, target outline) -> `render_hud`.

### Looking at the game without a display (optional)

The game runs under Xvfb with software GL, which is enough to take screenshots:

```sh
Xvfb :99 -screen 0 1280x720x24 &
DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1 ./target/debug/folk_craft &
DISPLAY=:99 xdotool mousemove 640 180 click 1      # open the first world in the list
DISPLAY=:99 import -window root shot.png           # ImageMagick screenshot
```

Pre-seed `saves/<n>.world` (format below) to start with a chosen inventory, position and view direction. Frame rate is low under
software GL, so put the player and the target at known coordinates in the save file instead of nudging the mouse. Prefer unit tests.

## Code map (`src/`)

| File | Responsibility |
| --- | --- |
| `main.rs` | Module list, window config, top-level loop dispatching on `app::Mode`. |
| `config.rs` | **All tunable constants.** Add new tunables here, in the section of the system that uses them. |
| `block.rs` | `Block` enum, shapes (`parts`, `bounds`) and rules (`hand_breakable`, `can_break_with`, `opaque`, `collides`, `is_pickable`). |
| `item.rs` | `ItemKind` (name, save id, colour, `placeable_block`, heal, melee data), `ItemStack`, block drop rolls (`drops_for*`). |
| `inventory.rs` | `Inventory`: `add`, `remove`, `take_from_slot`, `count`, `move_stack`, worn-slot and backpack-row handling. |
| `recipe.rs` | `RECIPES` table, `craftable(&Inventory)`, `craft(&mut Inventory, &Recipe)`. |
| `game.rs` | `Game`: one running session. Every action method (mining, using the held item, cooking, shooting, travel, the dungeon, the king, ghosts). Large; its tests are in several modules at the bottom. |
| `world.rs` | Chunk streaming, block access, edits (`break_block`, `set_block`), fire timers, cooking, pots, collision (`box_blocked`), raycast, camps (`build_camps`), door opening (`open_king_door`, `open_temple`, `unlock_tower`). |
| `terrain.rs` | Overworld generation: ground (hills, valleys, rivers, lakes, mountains), trees, clearings, the jungle, altars. Takes the world's `Landmarks`. |
| `landmarks.rs` | `Lake`, `Mountain`, `GrandCastle` (and the ruined castle), `Well`, `Graveyard`, `Rainbow`, `Landmarks` (what the terrain consults, `Landmarks::fixed`), compass maths. |
| `river.rs` | Rivers as bent parallel lanes. |
| `horizen.rs` | The Horizen (a cave dimension): caves, tunnels, ponds, cave kinds, camps (`camp_in_cell`), the king's cave. |
| `kingcastle.rs` | The king's castle as a dungeon: `castle_cell`, `ROOMS`, `DOORS`, the stair. Pure functions. |
| `skylands.rs` | The Skylands (islands, bridges, poison ivy, the tower's island `tower_cell_for`). |
| `tower.rs` | The silver tower: `tower_cell`, `ruined_tower_cell`, `Relic`, `Tower`. |
| `temple.rs`, `stonehenge.rs`, `mines.rs` | The jungle temple, Stonehenge, the mineshafts under the mountain. |
| `animal.rs` | `Animal`, `AnimalKind`, `Mode`: behaviour, health, drops and boss logic. Pure logic. |
| `animal_draw.rs` | Box models for animals, drawn each frame (thin wrapper over `draw_box`). |
| `arrow.rs`, `hazard.rs`, `tornado.rs`, `stars.rs` | Arrows and feathers; boss projectiles and acid; fire tornadoes and the cyclone; falling stars. |
| `shop.rs`, `ghosts.rs` | The general store's wares; every ghost's words. |
| `effects.rs` | Animated flames, ember glow, cooking pieces, the rainbow, the ghostly trail, tornadoes, falling stars. |
| `dropped.rs` | Items on the ground: spawn, physics, pickup, drawing. |
| `chunk.rs`, `mesh.rs`, `culling.rs`, `rng.rs`, `sky.rs` | Storage, meshing (AO, parts, splitting), view culling, seeded RNG and hashing, sun and clouds. |
| `player.rs` | Player state, keyboard input, AABB physics. |
| `save.rs` | World file format, load/save, world listing, seed parsing. |
| `net.rs` | Multiplayer transport: binary wire format (`ToHost`, `ToGuest`, `Snapshot`, `WorldOp`), framed TCP `Link`s, `Listener`, UDP `Beacon`/`Finder`. No game rules. Bump `PROTOCOL` when a message changes. |
| `game/session.rs` | Multiplayer rules: `Session`, host and guest pumps, snapshots, op replay, targets. Child module of `game` so it can reach `Game`'s private fields. |
| `lobby.rs`, `avatar.rs` | Host and Join screens (`JoinTask` connects on a thread); other players' figures, name tags, `project`. |
| `app.rs`, `menu.rs`, `ui.rs`, `settings.rs` | App state, world select and create screens, immediate-mode widgets, the settings screen and `saves/settings.cfg`. |
| `hud.rs`, `play.rs` | Hotbar, inventory screen, item icons, compass dial; the in-game frame. |
| `help.rs` | The Help screen. `help_lines()` builds the text from `RECIPES` and the config constants, so it stays true to the rules. |

## Conventions and invariants

- **Logic vs. UI.** Rules live in `Block`, `ItemKind`, `Inventory`, `Recipe`, `World`, `Animal` and `Game` methods, each with unit tests
  beside the code. UI code is immediate-mode: it reads input and draws in the same function and delegates every state change.
- **Block shapes.** A block is a plain full cube (`parts()` empty) or a set of coloured boxes in cell-local 0..1 coordinates.
  `opaque()` (hides neighbours' faces), `collides()` (the player and items bump into it) and `bounds()` (collision, aiming and the
  outline) are independent. Plants, pebbles, stones and charcoal are walk-through.
- **Ground height is per column.** Never assume the surface is at `GROUND_TOP`: use `terrain::ground_top(seed, wx, wz)` or
  `World::ground_top`.
- **Dirt and water are never breakable** (`Block::breakable`), so `World::ensure_chunk` ignores saved removals of them.
- **Breaking is decided by `Block::can_break_with(held)`**; do not duplicate the rule. `Block::breakable()` means "can ever be broken".
- **World edits are two saved sets**, replayed in this order when a chunk is generated: `World::removed`, then `World::placed`. Always go
  through `break_block` and `set_block`; never write `chunk.blocks` directly outside tests. A placeable block needs a save `id()`
  (`Block::PLAYER_MADE`).
- **Fire** state is a block (`Fire`, `Embers`, `ForgeLit`) plus a timer in `World::fires`. Use `ignite`, `feed_fire`, `update_fires`.
  Cooking and smelting share `World::cooking`; pots are `World::pots`.
- **Animals are entities, not blocks.** They live in `Game::animals`, are advanced by `Animal::update`, and are never saved. Damage goes
  through `Game::hurt_animals`.
- **Inventory mutations** go through `Inventory` methods; `craft` is all-or-nothing.
- **Slot layout** in `hud.rs` comes from `layout()`/`slot_rect()`/`slot_at()`: draw and hit-test with the same functions.
- **Meshing** (`mesh.rs`): plain cubes get face culling and AO; part-based blocks get flat shading. `MeshBuilder` splits at
  `MAX_QUADS_PER_MESH` (macroquad silently truncates oversized meshes): keep it.
- **Dropped items are capped** at `MAX_DROPPED_ITEMS` (100) by `Game::limit_items`, oldest first.
- **Structures never overlap**: `Landmarks::has_room` checks every lake, mountain, castle, well, graveyard and the `reserved` ground
  (Stonehenge, the jungle's bare middle and temple; set by `World`, not saved).
- **One of a kind** (`Game::owns`: inventory or on the ground): a second Philosopher's Stone drops as Gold; no Peep Stone comes from
  pebbles while one is owned; no Loadstone from stones while a Loadstone or Compass is owned; silver keys likewise (`key_or_gold`).
- Keep `cargo build` warning-free. Match the existing style: short doc comments, no dead code.

## Gameplay rules

### The normal world
- `y = 0` is unbreakable stone, `y = 1..=5` dirt (more on hills, less in valleys and river beds), a forest of oaks and pines above.
  **Dirt cannot be broken.** Tree trunks are thin (`TRUNK_INSET`); `WORLD_HEIGHT` is 180 so the 170-block mountain fits. The mesher only
  scans a chunk up to its highest block (`chunk::used_height`); view culling treats a chunk as a column.
- **Stones, pebbles and boulders** lie about on the ground (`GROUND_PEBBLE_PER_MILLE`, `GROUND_STONE_PER_MILLE`,
  `GROUND_BOULDER_PER_MILLE`, doubled on hills). They are ordinary placed blocks, break by hand (a boulder needs a bronze mallet) and stay
  gone (`World::removed`). A broken boulder drops a stone, a pebble and 1 to 3 iron ore.
- **Clearings** (`CLEARING_CHANCE`, radius 6 to 9) are tree-free and planted with grass, flowers and bushes (red or purple berries,
  `berry_bush_is_purple`, flipped at night). **The jungle has none of them**: no flowers, no bushes (`terrain::generate_chunk_for`).
- **Hills and valleys** (`terrain::relief_in_region`) are discs with a smoothstep profile, at most one per region, never a step of more
  than a block, kept clear of the spawn point and of rivers.
- **Rivers** (`river.rs`) are bent parallel lanes (`RIVER_LANE_SPACING`), 4 to 7 blocks wide, one block deep (`RIVER_DEPTH`), filled with
  `Block::Water`: no collision, cannot be aimed at or broken, never flows. The bed holds clay, pebbles and stones (`WetClay`,
  `WetPebble`, `WetStone`) that break by hand and leave plain water. Water is translucent and meshed apart (`Chunk::water`).
- **Breaking**: hold left mouse for `Block::break_seconds` (`BREAK_SECONDS_*`). Leaves and needles: 50% chance of a stick. Grass: 50%
  chance of grass. Bushes: five 50% chances of a berry. Trunks need a **stone axe** and give one log. Placed items, charcoal and ash
  break by hand and drop themselves. Poison ivy breaks by hand and always drops a Poison Ivy.
- **Placing**: right click places the held log, pebble, stone or charcoal in the empty cell in front of the target (on solid ground).
  Planks only exist in old saves.
- **Fire**: a bow drill lights a *placed* log (60 s, `FIRE_SECONDS`), which then leaves charcoal; charcoal lights to embers, which leave
  ash. Fire does not spread and burns on in unloaded chunks. Touching fire costs 1 life per second (`FIRE_DAMAGE_INTERVAL`) with knockback.
- **Forge** (2 stone + 8 clay): fill with charcoal, light with a bow drill; bellows make it glow. Smelting is `World::cooking`: stone ->
  copper and pebble -> tin on burning charcoal in the open; bronze (tin + copper) in a lit forge; iron ore -> iron in a *glowing* forge.
- **Water, jars, pots, stew**: a clay jar fills by right click on water. A tin pot is set on a burning log or on burning charcoal; fill
  it with a jar of water, then raw meat; it becomes `STEW_SERVINGS` servings of stew after `STEW_SECONDS` while a fire burns. Serve
  with a wooden bowl (1 log makes 5); eating returns the bowl. A pot outlives its fire (it stays on the charcoal or ash). Stew heals
  double the cooked meat. A Red Berry Stew brings day and a Purple Berry Stew night (`Game::set_night`).
- **Health**: `MAX_HEALTH` 10 hearts, more with moonlight (`Game::max_health`). Dying returns the player to the spawn point with full
  health and their inventory; from another dimension the player first returns to the normal world.
- **Hunting**: only axes (damage 4, 6, 8, 10 for stone, copper, bronze, iron), the bronze mallet, a bow, a sling, thrown feathers and
  knives, and swords hurt animals. Arrows fly under gravity (`arrow.rs`); a bow shoots Poison Arrows first, then Phoenix Arrows, then
  ordinary arrows. Poison arrows do `POISON_ARROW_DAMAGE_FACTOR` times the damage and are used up only by hitting an animal.
- **Jerky**: raw meat + `JERKY_SALT_COST` Pink Salt. **Inventory**: `INVENTORY_SLOTS` (45) with stacks of 100; slots `0..9` are the
  hotbar; a worn backpack adds `BACKPACK_EXTRA_ROWS`. **Dev mode** (chosen at world creation) lists every item on the inventory screen.
- **Wearing**: one worn thing (`Game::worn`): Pelt Cloak, Royal Armor, King's Crown, Leather Backpack, Boots of the Sky, Shell Armor.
  `Game::apply_worn_effects` applies speed and jump; armour reduces damage in `Game::hurt_player`.

### Recipes (all shapeless; see `RECIPES`)
Twine, bow drill, stone axe, copper/bronze/iron axes (leather cord), bow, sling, arrow (pebble + stick + feather), Phoenix Arrow,
**Poison Arrow x25** (25 arrows + 1 Snake Fang), **Poison Arrow x5** (5 arrows + 1 Poison Ivy), tin pot, clay jar, clay forge, bellows,
bronze mallet, wooden bowls, leather cord x3, backpack, jerkies, compass (loadstone + iron), bone totem (antlers + 2 gold), musical
pipes (3 bones), crystal ball (10 stardust), unicorn sword, ivory knives, shell armor, tin pan.

### Landmarks have set places
Everything the player can look for has a fixed place that is a pure function of the seed. Nothing appears because the player holds, wears
or walks something. The one exception is the rainbow's gold (below).

- **`Landmarks::fixed(seed, reserved)`** places the mountain, the lake, the grand castle, the ruined castle, the well and the graveyard.
  Each starts on a heading the seed picks, `FIXED_*_DISTANCE` blocks from the spawn point (650 for the well up to 1,800 for the mountain),
  and is pushed `SITE_PUSH_BLOCKS` further out until `Landmarks::has_room` says nothing is in the way. `World::new` and `World::with_landmarks`
  use it, so a loaded save gets the same sites whatever positions it recorded; what the player did (monsters killed, towers unlocked, the
  Yeti, the rainbow) is kept, and an older save's killed monster or unlocked tower carries over to the single lake.
- **Lake** (`Lake`): one great lake, radius `LAKE_RADIUS`, one block deep, with an island hill (`ISLAND_HILL_HEIGHT`, flat top
  `ISLAND_PLATEAU_RADIUS`, every step jumpable) and a silver tower on it. Nothing leads to it from a river (`from` is its centre). One
  **Loch Ness Monster** guards it.
- **Mountain** (`Mountain`): `MOUNTAIN_HEIGHT` 170, radius `MOUNTAIN_RADIUS` 400, `MOUNTAIN_MAX_LEDGES` rings of cliff each with one gap (gaps on
  alternate sides, so the climb zigzags), a flat summit arena (`SUMMIT_PLATEAU_RADIUS`) and snow from `SNOW_LINE_RISE`. The compass
  always points at it (`compass_bearing`). **Mines** (`mines.rs`): five entrances at its foot, shafts and three rings of tunnel; pink salt
  (`Block::PinkSalt`), bats, and a silver key on a stand in the middle ring of `MINE_KEY_SHAFTS`.
- **Grand castle** (`GrandCastle`, `grand_castle_cell`): whole and shut at night, a ruin by day; the royal armour lies in its throne room
  at night. `World::set_night` regenerates its chunks. **Ruined castle** (`ruins_cell`): same footprint; the sword in the stone stands on
  a dais at the centre (`Game::try_pull_sword`, crown required).
- **Well** (`Well`): 4x4 water sunk to the stone floor; falling to the floor takes the player to the Horizen (`update_dimension`).
  **Graveyard** (`Graveyard`): walled, with a ghost (see Ghosts).
- **Jungle, Stonehenge, altars**: also pure functions of the seed. `terrain::jungle_centre` (`JUNGLE_DISTANCE_CHUNKS` out, radius
  `JUNGLE_RADIUS`, bare middle `JUNGLE_CLEARING_RADIUS`, small ponds, darker grass, thick `Foliage` and `FigFoliage`, a temple);
  `stonehenge::centre`; **ancient altars** (`terrain::altar_in_region`: one in `ALTAR_REGION_PERCENT` of the squares of `ALTAR_REGION_CHUNKS`
  chunks, never within `ALTAR_MIN_SPAWN_DISTANCE` of spawn, `altar_site_ok`). An altar is a generated `Block::AltarLoadstone`; taking
  the loadstone leaves `Block::AncientAltar` (a saved placed block).
- **Crows** behave like quail but a startled crow flies to the nearest altar within `CROW_ALTAR_SEEK_DISTANCE` (`terrain::nearest_altar`,
  `Animal::altar_goal`) and comes down on it; with none in reach it flies away from the player.
- **Peep Stone**: by day shows the rainbow (see below) and a ghostly trail to its end; by night a trail to the grand castle's gate and
  keep door; in the Skylands a trail to the volcano; in the Horizen a trail down the tunnel to the tower. **Bone Totem**: by day a trail to
  the well, by night to the graveyard, in the Skylands to the silver tower, in the Horizen to the king's castle. `Game::trail_path`,
  `effects::trail_wisps`. **Compass**: the dial shows the mountain; in the other dimensions the arrival point.
- **Rainbow** (`RainbowState`): with a Peep Stone in hand by day and `RainbowState::Unseen`, `Game::try_show_rainbow` places a
  `Block::GoldPile` `rainbow_distance()` or more away and sets `Active`. Mining the pile gives `RAINBOW_GOLD_DROP` gold and sets `Spent`
  for ever; the trail keeps leading to where the gold lay (`spent_gold`).
- **Day and night**: the berry stews and the crystal ball call `Game::set_night`, which blacks the screen out (`Blackout`) and changes the
  world at full black. The Horizen is always day.
- Settings (`settings.rs`, F2) set the render distance, 3 to 10 chunks (`config::render_radius()`); sites do not depend on it.

### Dimensions
Each dimension is its own `World` (`World::dimension`), parked in `Game::away`/`Game::shelf` while the player is elsewhere
(`Game::travel_to`). Each has its own edits, items, time and landmarks.

- **Horizen** (`horizen.rs`): a cave; flat dirt floor at `GROUND_TOP`, `air_height` is the shape. Arrival chamber and pool, a winding
  tunnel to the great cavern (with the silver tower at `cavern_centre`, there from the start), a grid of caves (`HORIZEN_CAVE_CELL`) joined
  by tunnels, and the king's giant cave. Cave kinds: Plain, Mossy (newts), River (big crawfish), Crystal (ruby, sapphire, amethyst),
  Mushroom. **Camps** (`camp_in_cell`): about one cave in `HORIZEN_CAMP_EVERY`, a set spot of dry floor; `World::build_camps` lights the
  log and sets the pot the first time the chunk is made and records it in `World::camps_built`. Bats live here; night in the cave
  (`apply_night`) makes the player faint to Stonehenge.
- **Skylands** (`skylands.rs`): islands on a grid (`SKY_CELL`) joined by decks; wooded, ruins, hill, altar and the giant volcano island.
  By night the bridges break. **Poison ivy** (`Block::PoisonIvy`) grows in patches (`ivy_site`) on every island except the first and the
  volcano. The silver tower stands on the wooded island `skylands::tower_cell_for(seed)` picks. Fire tornadoes, falling stars, fairies,
  mountain goats, the Phoenix, the Dragon and the general store live here. The way in is the summit cyclone (after the Yeti); the way
  out is to fall.
- **Falling out of the Skylands** (`Game::update_travel`, `pick_fall_landing`): the screen whites out, then the player is set
  `SKY_FALL_DROP` blocks above one of three places chosen by chance: the mountain's upper slope (`mountain_column`, about
  `LANDING_BELOW_SUMMIT` under the top), the Loch Ness island beside the tower (`lake_island_column`), or the jungle beside the temple
  (`jungle_column`). The chosen spot is `Game::fall_landing`, so the picture behind the white is where they land.
- **The King's Crown** (worn): coming back to the normal world from the Skylands or the Horizen (a fall, the cave faint, any travel)
  sets the wearer in the ruined castle, on the ground `CROWN_ARRIVAL_DZ` blocks south of the dais (`Game::crown_arrival_now`,
  `crown_arrival`). Dying still returns the player to the spawn point.
- **Yeti**: leaving the Skylands clears `yeti_slain`; the summit cyclone is gone until it is killed again.

### The king's castle (a dungeon)
`kingcastle::castle_cell(dx, dz, dy, cleared)` is a pure function of a cell's place from the castle's centre. Outside: an outer wall with a
gate on the south side (open) and four sealed corner towers. Inside the wall it is solid stone carved into eight rooms on two storeys
(`ROOMS`, ground floor `dy 0..8`, a slab at `KING_SLAB_DY`, upper floor above, roof at `KING_ROOF_DY`): gatehouse, hall of pillars,
armoury, barracks, crypt, then (behind the crypt's door) a stair up to the chapel and the library, and last the throne room. Door `i`
(`DOORS`, `Block::CastleDoor`, unbreakable) joins room `i` to room `i + 1` and is shut until `Landmarks::king_rooms_cleared > i`.

- `Game::update_king_dungeon` (each frame): when the player stands in room `r < KING_THRONE_ROOM` and `r == king_rooms_cleared`,
  `KING_GUARDS_PER_ROOM` hostile **spirit guards** (`AnimalKind::SpiritGuard`: the Wild Hunt archer's behaviour with shorter ranges,
  `SPIRIT_GUARD_*`, no boss bar, not part of the Hunt) come out on free floor at least `KING_GUARD_MIN_DISTANCE` away; when the last is
  dead `World::open_king_door(r)` opens the door for good. A player who leaves the room with guards alive finds them gone
  (`Game::guard_room`), and they return on re-entry. Cleared rooms and the throne room never have guards.
- **Only in the throne room** (`Game::in_throne_room`) do Musical Pipes make the king speak and open the pop-up
  (`Game::king_dialog`); `Game::give_gold_to_king` sends one friendly **spirit archer** (`AnimalKind::SpiritArcher`, up to
  `MAX_SPIRIT_ARCHERS`) per `GOLD_PER_SPIRIT` gold. They go with the player between dimensions.
- The Boots of the Sky stand on a plinth on the dais in the throne room (`kingcastle::BOOTS_CELL`).
- Tests walk the castle with every number of rooms cleared: each room is reachable exactly when the one before is cleared, and nobody can
  climb the wall, the towers or the roof.

### Creatures (`animal.rs`)
Wildlife of the normal world: deer, rabbits, quail, crows (above), bears, moose and, at night, will-o-whisps; they spawn out of sight
(`ANIMAL_SPAWN_*`), are simulated within `ANIMAL_UPDATE_DISTANCE` and despawn beyond `ANIMAL_DESPAWN_DISTANCE`. Small crawfish live in the
rivers. Everything else is placed by a rule:

- **Bosses** (`AnimalKind::is_boss`, a health bar while near): Yeti (summit), Loch Ness Monster (the lake; ravens and acid), Iron Quail
  (3 for every `QUAIL_KILLS_PER_IRON_QUAIL` quail), Bronze Parrot (jungle), Phoenix (volcano crater, respawns), Vampire Bat (every
  `BATS_PER_VAMPIRE` cave bats), Orangutan (jungle temple; its passage is boarded until it dies), Unicorn (the Cursed Idol in the hotbar,
  outside the jungle), Great Bear (a bear killed while wearing the Pelt Cloak), Giant Crawfish (every `BIG_CRAWFISH_KILLS_PER_GIANT` big
  crawfish), Dragon (after `DRAGON_SWITCHES` day/night switches), and the Wild Hunt (blown with the Horn of the Hunt inside Stonehenge: a
  swordsman, archers, lancers, ghost bears and crows; beating them wins the game). Bosses are not saved; a living one returns whole.
- **Crawfish** (`AnimalKind::is_aquatic`): they keep to wet columns, except that a big or giant one that is attacking
  (`can_leave_water`, `Mode::Charge`) is not held to the water: it leaves it to chase the player (boss steering), and a crawfish found on dry
  land heads for the nearest water within `CRAWFISH_SHORE_SEARCH` (`Animal::heading_to_water`). Small ones never attack or leave.
- **Spirit guards** (hostile, in the dungeon) and **spirit archers** (friendly, from the king) are described above.
- Other dimension creatures: bats and newts (Horizen), jaguars, flying snakes (long, `Animal::trail`) and parrots (jungle; no ordinary
  wildlife there), fairies and mountain goats (Skylands), jungle and Skylands rules in `Game::spawn_jungle_wildlife`,
  `spawn_sky_wildlife`.

### Items worth knowing
- **Poison Ivy** (`ItemKind::PoisonIvy`): from breaking `Block::PoisonIvy`. Walking into the plant stings `POISON_IVY_DAMAGE` every
  `POISON_IVY_INTERVAL` seconds of contact (`Game::touching_ivy`, `update_ivy`, `Game::ivy_timer`), no knockback. 1 ivy + 5 arrows make 5
  poison arrows.
- **Moonlight** (extra hearts, timed), **Boots of the Sky** (speed and jump), **Cursed Idol** (double damage, no healing), **Shell Armor**,
  **Royal Armor**, **Pelt Cloak**, **Crystal Ball** (left click toggles day and night), **Ivory Knife** (thrown, never used up), **Iron
  Feather** (thrown), **Sling** (shoots crystals, then pebbles), **Excalibur** and the **Unicorn Sword** (`ItemKind::melee_damage`).
- **Shop** (`shop.rs`, `Game::buy`): a ghostly figure on the Skylands' first island sells arrows, a bow, a sword and stews for gold.
- **Ghosts** (`ghosts.rs`) are only words on screen (`Game::ghost_text`). The graveyard's ghost asks for a song (Musical Pipes) and gives
  a silver key once; in each silver tower the pipes bring a hint: the Skylands tower says "Find the great Loch guarded by that old
  serpent.", the cave tower that things work differently at night, the lake tower to blow the horn in the stonehenge. The king and the
  shopkeeper speak as described above.

## Multiplayer (`net.rs`, `game/session.rs`)

- **The host simulates everything shared.** `Game::targets()` lists the players in the world swapped in (the host's own, unless that world is swapped in for guests, then each guest in that dimension); every creature picks the nearest (`nearest_target`) in `update_animals`, and boss smashes, bursts and arrows hurt each target in reach (`hurt_target`; a guest is told by `ToGuest::Hurt` and applies its own armour). Spawn and dungeon rules that look at "the player" run per player with `Game::as_player_at`. The host keeps chunks loaded round each guest (`World::load_around`).
- **Every player travels alone.** Each dimension's creatures, projectiles, weather and clocks are a `Sim` (the fields of `Game` hold the host's own dimension's; `Away::sim` holds a parked one's, kept only while a guest is in it: `travel_to` clears it otherwise). `Game::simulate_in(dim, f)` swaps a dimension's world, items and `Sim` in for `f`, with `net.away_sim` set so `targets()` leaves the host out; `update_remote_dimensions` runs `simulate_for_guests` for each dimension guests are in, and `host_publish` builds each guest's `Snapshot` (tagged with its `dim`) inside the right one. A host's session always holds all three worlds (`ensure_dimensions`), so every `WorldOp` (tagged by dimension) has somewhere to land. Night is per dimension (`Night { dim, night }`; never sent for the Horizen).
- **A guest runs only its body, and its own travel, rainbow, ghosts and well.** Creatures, items, arrows, projectiles, acid, tornadoes and stars come from `Snapshot`s (20 a second); stand-in `Animal`s (`Animal::from_net`, `apply_net`) are drawn and aimed at, never updated (`guest_smooth`). `Animal::net` and `DroppedItem::net` are the numbers the host gives things (0 until reported). A guest's blows go to `Game::guest_hurt` (from `hurt_animals_from`) and are done by the host (`host_hurt`); items a guest makes (net 0) are sent to the host; pickups are requested and granted. What only the host's creatures can do is an `Intent` (`Horn`, `Pipes`, `Lightning`, `KingGold`) carried out by `host_acts` where the guest stands, using `summon_hunt`, `pipes_call_creatures`, `rod_lightning` and `send_spirits`. A guest's idol is seen by the host through the flags a guest reports (`update_guest_idols`).
- **World changes** are `WorldOp`s: every `World` mutator writes the state it left to `World::net_log` (`note`, `note_state`) when `World::net_on`; `World::apply_op` replays them quietly. Ops state the result, not the action, so they are idempotent and need no loaded chunk. The shared story (`Progress`: slain, summoned, rested, the rainbow) is sent whenever `World::note_progress` sees it change. Only the host burns fires out (`update_fires`); guests run `tick_timers`.
- **Guests' packs:** a guest reports `Game::profile_text` every few seconds (and on leaving) once the host has sent what the name carried before (`ToGuest::Profile`, replayed with `Game::replay_messages`); the host keeps them by name in `<world>.players` (`save_profiles`, written with the world). With others in the game a death does not clear the fights (`hurt_player`); guests never save the world (`Game::save`).
- Tests (`game/session.rs`) run a host and a guest in one process over localhost: add one for each new shared rule.

## Save format

Plain text, one record per line, in `saves/<sanitized name>.world`:

```
<world name>
<seed>
<px> <py> <pz> <yaw> <pitch>
<x> <y> <z>                      one line per removed block
inv <slot> <item id> <count>     one line per occupied inventory slot (slots 36 to 53 belong to a worn backpack)
item <item id> <x> <y> <z>       one line per item lying on the ground
place <x> <y> <z> <block id>     one line per player-made block
fire <x> <y> <z> <seconds>       seconds left for the fire block at that position
cook <x> <y> <z> <item id> <s>   a piece on that fire, with the seconds of heat still needed
pot <x> <y> <z> <0|1> <meat id or -> <s> <served>   a tin pot on that fire
glow <x> <y> <z> <seconds>       seconds of bellows glow left
berry <x> <y> <z>                a bush or fig foliage whose fruit has been picked
camp <x> <z>                     a camp of the Horizen was set out at that spot (only in the Horizen's records)
health <n>   mode dev   time day|night   worn <item id>
quailkills <n>  parrotkills <n>  batkills <n>  switches <n>  crawfishkills <n>
rainbow spent | rainbow <from x> <from z> <x> <y> <z> | rainbowend <x> <y> <z>
yetislain  orangutanslain  unicornsummoned  unicornslain  won  ghostrest
phoenixslain  phoenixdead <seconds>         (Skylands' records)
kingcleared <n>                  the first n rooms of the king's dungeon are cleared (Horizen's records)
nessie <x> <z>                   the lake's monster is dead (the lake's centre)
unlocked <x> <z>                 the silver tower centred there has had its door unlocked
dimension horizen|skylands       the player is in that dimension (everything above then describes the normal world)
h <record>  /  s <record>        a record of the Horizen / the Skylands (blocks, fires, items, `pos`, `time`, ...)
```

Unknown or malformed records are ignored, and older saves without later lines still load. **The sites of the landmarks are not saved**:
the seed sets them (`Landmarks::fixed`, `World::with_landmarks`). Saves written before that may hold `lake`, `mountain`, `grandcastle`,
`ruinedcastle`, `well`, `graveyard`, `tower`, `skytower`, `skyvisit` and `cavevisit` records (and `back <item>`, the old backpack place);
they are read and then overridden or ignored. Old item ids `copper_ore` and `tin_ore` load as `stone` and `pebble`; `deer_bone` loads as
`bone`. Animals, arrows in flight and the selected hotbar slot are not saved.

Item ids (`ItemKind::id()`): `stick`, `pebble`, `stone`, `log`, `stone_axe`, `grass`, `berries`, `purple_berries`, `twine`, `bow_drill`,
`charcoal`, `plank`, `feather`, `bow`, `arrow`, `venison`, `poultry`, `cooked_venison`, `cooked_poultry`, `leather`, `bear_meat`,
`cooked_bear_meat`, `bear_pelt`, `bellows`, `ash`, `copper`, `tin`, `iron`, `gold`, `clay`, `iron_ore`, `gold_ore`, `leather_cord`,
`bronze`, `bronze_mallet`, `clay_forge`, `rabbit_meat`, `cooked_rabbit`, `moose_meat`, `cooked_moose`, `rabbit_foot`, `peep_stone`,
`holly_rod`, `copper_axe`, `bronze_axe`, `iron_axe`, `tin_pot`, `clay_jar`, `water_jar`, `venison_stew`, `poultry_stew`, `bear_stew`,
`rabbit_stew`, `moose_stew`, `red_berry_stew`, `purple_berry_stew`, `sling`, `loadstone`, `compass`, `antlers`, `bone_totem`,
`pelt_cloak`, `royal_armor`, `silver_key`, `pink_salt`, `philosophers_stone`, `ancient_crown`, `horn_of_the_hunt`, `bone`,
`musical_pipes`, `tin_pan`, `excalibur`, `leather_backpack`, `iron_feather`, `phoenix_feather`, `phoenix_arrow`, `boots_of_the_sky`,
`cursed_idol`, `snake_fang`, `poison_ivy`, `poison_arrow`, `wooden_bowl`, `stardust`, `sword`, `eye_of_newt`, `ruby`, `sapphire`,
`amethyst`, `mushroom`, `crystal_ball`, `unicorn_horn`, `unicorn_sword`, `shell_fragment`, `shell_armor`, `fig`, `moonlight_vial`,
`goat_horn`, `ivory_knife`, `venison_jerky`, `poultry_jerky`, `bear_jerky`, `rabbit_jerky`, `moose_jerky`.

Block ids (`Block::id()`, the player-made blocks): `placed_log`, `placed_pebble`, `placed_stone`, `fire`, `charcoal`, `planks`, `embers`,
`ash`, `clay_forge`, `forge_charged`, `forge_lit`, `gold_pile`, `stardust_pile` (never written), `gold_boulder`, `ancient_altar`,
`altar_loadstone`, `pulled_stone`, `flower`. Generated blocks (snow, castle stone, doors, ivy, foliage and so on) have no id and are never
saved; the player's removals of them are, as plain coordinates.

## How to extend

**New item**: add a variant to `ItemKind` and to `ItemKind::ALL` (and fix its length), fill in `name`, `id`, `color`; in `dropped.rs` add its
`rest_half_height` and `draw_half_extents`; in `hud.rs` add an icon arm in `draw_item_icon`; to make it placeable return a block from
`ItemKind::placeable_block`.

**New craftable**: append a `Recipe` to `RECIPES` in `recipe.rs` and a test (the Help lists it automatically). Update the expected output
count in `recipe::tests::recipe_costs` if it makes more than one.

**New block**: add a variant in `block.rs` with its `parts()` (or none for a full cube), the `hand_breakable`/`breakable`/`can_break_with`/
`opaque`/`collides` rules and `break_seconds`; add drops in `item::drops_for`; generate it in `terrain.rs`, `skylands.rs` or `horizen.rs`;
if players can make it, add it to `Block::PLAYER_MADE` with an `id()`. Add it to the `ALL` list in the `block.rs` tests (and fix the length).

**New creature**: add the `AnimalKind` data and movement rules in `animal.rs` (every `match` on the kind: health, size, speed, drops,
eyes), spawn rules in `Game`, a model in `animal_draw.rs`, then decide explicitly whether it can be hunted, attacks or drops anything. Add
behaviour tests first.

**New structure or place**: make its position a pure function of the seed. For the overworld add it to `Landmarks::fixed` (so it has a
`FIXED_*_DISTANCE`, a `has_room` check and a test that it is far from spawn and clear of the others); for a dimension add it to
`World::fixed_landmarks` or generate it from the chunk coordinates. Do not make it appear because the player did something; if the player
changes it (a door opens, a monster dies), keep that as a saved record and regenerate the chunks (see `World::open_king_door`).

**New tool or held-item behaviour**: read `game.held_item()`; extend `Block::can_break_with` for tool-gated breaking, or add an action
next to `Game::try_ignite` and call it from `play::handle_breaking`.

## Known limitations

- Placed logs always lie along the x axis. Burning fire cannot be put out or broken; it only burns down.
- Dropped items inside a cell where a solid block is then placed can get stuck in it. Remaining fire time is not shown.
- Animals are not saved: killed ones come back, and new ones wander in, when a world is loaded.
- An arrow that hits a plant-covered cell is not stopped by the plant.
- Rivers are one block deep: you wade through them; there is no swimming.
- Dying respawns the player at the spawn point, which may be where a bear is waiting. This holds with the King's Crown too.
- The ruined castle, the lake and the others have no ghostly lead except the ones described above (the crown does not show a trail).
- The sites are hundreds of blocks away: tests reach them by teleporting and streaming chunks, which is slow for big loops.
