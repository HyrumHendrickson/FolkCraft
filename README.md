# Folk Craft

A small Minecraft-style voxel game in Rust using [macroquad](https://macroquad.rs) 0.4.
First-person view, a chunked procedural forest with clearings, gentle hills and valleys and winding rivers,
block breaking and placing, item drops, an inventory, shapeless crafting, fire and cooking, wild deer, quail,
bears, rabbits, moose and nighttime will-o-whisps, and hunting with an axe or a bow. This file is the starting point for agents
(and humans) changing the code.

## Build, run, test

```sh
cargo run            # play (opens a 1280x720 window; saves go to ./saves relative to the CWD)
cargo test           # all unit tests; no window or GPU needed
cargo build          # must stay free of warnings
```

- Dependencies: only `macroquad = "0.4"`. Avoid adding crates without a strong reason.
- `[profile.dev] opt-level = 2` is intentional (mesh building is too slow unoptimised).
- On Linux, `macroquad` needs ALSA headers to compile (`libasound2-dev` on Debian/Ubuntu).
- Tests construct `Game`/`World` directly and never touch the GL context. Keep it that way:
  put logic in plain functions or methods on data types and unit-test those; keep macroquad
  drawing and input calls in thin wrappers (`hud.rs`, `play.rs`, `menu.rs`, `effects.rs`).
- Always run `cargo test` and `cargo build` before finishing a change. Add tests for new logic.

## Agent contributor guide

Use this loop for every change:

1. Read the relevant section of this README and the owning module before editing.
2. Put gameplay rules in the logic modules (`block.rs`, `item.rs`, `inventory.rs`, `recipe.rs`, `world.rs`, `animal.rs`, or `game.rs`). Keep input and drawing in the thin UI wrappers.
3. Add tunable numbers to the matching section of `config.rs`; keep constants grouped by system and document non-obvious values.
4. Add or update unit tests beside the logic you changed. Tests must not require a window or GPU.
5. Run `cargo fmt --check`, `cargo test`, and `cargo build`. Inspect `git diff --check` before handing off.

For a new item, update `ItemKind`, `ALL`, `name`, `id`, `color`, dropped-item geometry, and the HUD icon. For a new block, update its shape and rules, drop handling, generation if needed, and save id if players can place it. For a new creature, update `AnimalKind`, movement/spawn rules, rendering, weapon targeting, and tests. Keep save compatibility: item and block ids are part of the plain-text world format.

The main gameplay call path is:

`play::play_screen` → input/raycast → `Game` action methods → world/inventory/entity state → item pickup/fire/animal updates → rendering/HUD.

Before adding a helper, search for an existing rule owner. In particular, use `Game::held_item()`, `Block::can_break_with`, `ItemKind` conversion methods, `Inventory` mutation methods, `World::set_block`/`break_block`, and `Animal::update` instead of duplicating their logic.

### Looking at the game without a display (optional)

The game runs under Xvfb with software GL, which is enough to take screenshots:

```sh
Xvfb :99 -screen 0 1280x720x24 &
DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1 ./target/debug/folk_craft &
DISPLAY=:99 xdotool mousemove 640 180 click 1      # open the first world in the list
DISPLAY=:99 import -window root shot.png           # ImageMagick screenshot
```

Pre-seed `saves/<Name>.world` (format below) to start with a chosen inventory, position, view
direction, and even placed blocks and fires. Frame rate is low under software GL, so aiming and
timing with `xdotool` are imprecise: put the player and the target at known coordinates in the
save file and compute the pitch instead of nudging the mouse. Prefer unit tests for logic.

## Gameplay rules (current)

- World: `y = 0` bedrock stone (unbreakable), `y = 1..=5` dirt (more on hills, less in valleys and river beds),
  a forest of oaks and pines above. **Dirt cannot be broken**: the land stays as generated. Tree trunks are thin (0.7 block wide, `TRUNK_INSET`); leaves next to a trunk reach in
  to meet it. `WORLD_HEIGHT` is 180 so the 170-block mountain fits. The mesher only scans a chunk up to its highest block (`chunk::used_height`), and view culling treats a chunk as a column (`culling::chunk_visible`).
- **Stones and pebbles** lie about on the ground everywhere (forest floor, clearings, hills; hills are
  twice as rocky): about 2% of columns hold a pebble and 0.8% a stone (`GROUND_PEBBLE_PER_MILLE`,
  `GROUND_STONE_PER_MILLE`, `HILL_ROCK_FACTOR`). They are ordinary `PlacedPebble`/`PlacedStone` blocks in the
  generated terrain, so they break by hand, drop themselves, and stay gone (recorded in `World::removed`).
- A **Rabbit Foot** in the hotbar gives a broken pebble a `RABBIT_FOOT_PEEP_STONE_CHANCE` (20%) chance to
  add a Peep Stone, and leaves or pine needles a `RABBIT_FOOT_HOLLY_ROD_CHANCE` (20%) chance to add a Holly Rod.
  It also keeps the existing Loadstone and Gold Ore bonuses. A Holly Rod in the hotbar draws will-o-whisps (see Animals). A Peep Stone shows the rainbow by day and the grand castle by night, each with a ghostly trail, and a Compass leads to a mountain (see "Landmarks" below).
- **Boulders** (`Block::Boulder`) lie among the stones at `GROUND_BOULDER_PER_MILLE` 2 per mille of columns (twice as often on
  hills). They are solid, rust-streaked rocks that only a **bronze mallet** breaks (`BREAK_SECONDS_BOULDER` 4 s). A broken boulder
  drops one stone, one pebble and `BOULDER_IRON_MIN..=BOULDER_IRON_MAX` (1 to 3) iron ore, and stays gone.
- **Clearings** appear now and then in the forest (`terrain.rs`): round, tree-free, and planted with
  grass, flowers and bushes. See "Terrain, hills and clearings" below.
- **Valleys** are hills turned upside down (`Relief` with a negative height): radius 20 to 32, 2 to 4 blocks deep,
  never a step of more than one block, and always one layer of dirt left. They carry trees or clearings like the flat forest.
- **Rivers** (`river.rs`) wind across the world as bands 4 to 7 blocks wide. A river column is cut one block
  (`RIVER_DEPTH`) below the ground and filled with `Block::Water`, which does not collide (you wade through it and hop
  out), cannot be aimed at or broken, and never flows: no one can dig the bank away. No hill or valley comes within
  `RIVER_RELIEF_MARGIN` of a river, nothing grows or lies in the water, animals do not spawn in it, and the spawn
  area is dry.
- **The river bed** holds clay, pebbles and stones under the water, with the shares `RIVER_CLAY_PER_MILLE` 60,
  `RIVER_PEBBLE_PER_MILLE` 50 and `RIVER_STONE_PER_MILLE` 25 per mille of river columns. They are `Block::WetClay`,
  `WetPebble` and `WetStone`: the water's own cell (still water, one block deep) with a small object on the bed. They can be
  aimed at through the water and break by hand (0.5 s) into clay (`CLAY_DROP` 2 lumps), a pebble or a stone, and
  `Block::remainder` leaves plain water behind (`World::break_block`, and the replay of saved removals in `ensure_chunk`), so a river
  is never dug deeper. Clay makes a clay forge.
- **Water is translucent** (`WATER_ALPHA`). Its surface is meshed apart from the opaque terrain (`World::build_water_meshes`,
  `Chunk::water`) and drawn last in `play::render_world`, after the items, animals and arrows, so the mud of the bed and
  whatever lies on it, and anything under the surface, show through. Every wet cell (`Block::is_wet`) gets a surface.
- **Hills** are broad (radius 20 to 32), gentle (3 to 5 blocks high, never a step of more than one block)
  rises of the ground. They carry trees like the flat forest, and clearings can lie on them. The spawn
  area is always flat. A player (and an animal) can hop up the one-block steps.
- Left mouse (hold) breaks the targeted block (reach `REACH`). The button must stay held on the same
  block for `Block::break_seconds` (`BREAK_SECONDS_*` in `config.rs`): wood 5 s, bush 3 s,
  leaves and needles 2 s, grass and flowers 0.5 s, placed items and charcoal 0.5 s. A bar under the
  crosshair shows progress; releasing, looking at another block or changing tool resets it:
  - Leaves and pine needles: 50% chance of one stick.
  - Grass: 50% chance of one grass. Flowers: nothing. Bushes: five 50% chances of a berry each.
  - Wood (tree trunks): needs a **stone axe** held. Drops exactly one log.
  - Placed logs, pebbles, stones, charcoal and planks: break by hand and drop themselves back (one each).
    Planks take 1.5 s (`BREAK_SECONDS_PLANKS`), the others 0.5 s. **Ash** breaks by hand in 0.5 s and drops ash.
  - Dirt, bedrock stone, water and burning fire cannot be broken.
- Right mouse **places** the held log, pebble, stone, plank or charcoal in the empty cell in front of the targeted
  block. Placed items are small (pebbles and stones are a fraction of a block; logs lie 0.5 high), need
  solid ground (dirt, bedrock, leaves, planks) below, and cannot go inside the player if they are solid.
  **Planks** (no longer craftable; only ones from older saves exist) are full, solid cubes for building shelters: a plank needs only a dirt, stone or plank block
  next to it on any side (`Block::anchors_planks`, `Game::can_place`), so walls go up from the ground and a
  ceiling can reach out from the top of a wall. Foliage and trunks do not hold planks.
- **Fire**: with a **bow drill** held, left click on a *placed* log lights it (trees do not burn).
  It burns 60 seconds (`FIRE_SECONDS`). Right click a burning fire with a log held to add 60 more
  seconds (the log is used up). When the time runs out the fire leaves **charcoal**, which can be
  broken and picked up. Fire does not spread. Fires keep burning in chunks that are not loaded.
- **Charcoal fires**: a placed charcoal block (charcoal is what a burnt-out log leaves) is lit with a bow
  drill exactly like a placed log (left click) and becomes **embers** (`Block::Embers`), which burn
  `FIRE_SECONDS` (60 s) in `World::fires`. When embers burn out they leave **ash** (`Block::Ash`). Right
  click burning embers (or a burning log) with a log or charcoal held adds 60 s (`ItemKind::is_fuel`).
  Embers are walk-through but burn you like a fire, and cannot be broken.
- **Clay forge** (2 stone + 8 clay): placed like a stone (`Block::ClayForge`). Right click it with charcoal to fill it
  (`Block::ForgeCharged`), left click it with a bow drill to light it (`Block::ForgeLit`, `FIRE_SECONDS` in `World::fires`, can be
  fed more charcoal or logs). When it burns out it is an empty forge again. An empty or filled forge breaks by hand
  (`BREAK_SECONDS_FORGE`) into `FORGE_CLAY_DROP` (4) clay (plus the charcoal if filled); a lit forge cannot be broken.
- **Bellows** (1 twine + 1 leather + 2 sticks): left click a **lit forge** with bellows held and it glows brighter for
  `BELLOWS_GLOW_SECONDS` (40 s; hitting it again renews it). They do nothing to logs, open embers or an unlit forge.
- **Cooking and smelting** share one mechanism (`World::cooking`, one piece per burning block; `ItemKind::hearth` says where
  a piece may lie, `World::can_lay` checks it). Right click a burning block with raw meat to cook it (`COOK_SECONDS`, any fire).
  Smelting takes `SMELT_SECONDS` (10 s); right click again with a non-cookable item in hand to take the finished piece:
  - **Stone -> copper, pebble -> tin**: on burning **charcoal in the open** (`Block::Embers`); no bellows needed. Not on a log fire or in a forge.
  - **Bronze**: in a **lit forge**, right click holding a tin or a copper with the other in the inventory; one of each is used
    (laid as `Cooking { raw: Tin }`; `Cooking::released` gives both metals back if the forge burns out first). No bellows needed.
  - **Iron ore -> iron**: in a lit forge that is **glowing** (bellows); progress pauses when the glow fades.
  A refused piece is kept and a toast says why (`Game::refusal`). If the fire burns out first, the piece drops as it was.
- **Water, jars, pots and stew**: a **clay jar** (4 clay) filled by right clicking water (`Game::fill_jar`, `World::water_in_reach`: the only way to
  carry water; empty `ClayJar`, full `WaterJar`). A **tin pot** (3 tin) is set on a burning log or on burning charcoal in the open (not a forge) by right
  clicking the fire with it (`World::pots`, one per fire, never together with a `cooking` piece). Right click the pot with a jar of water to fill it
  (the jar comes back empty), then with any raw meat (`ItemKind::stewed`: venison, poultry, bear meat, rabbit meat, moose meat). It becomes that meat's
  stew after `STEW_SECONDS` while the fire burns (`Pot`, `World::update_fires`). Right click with an empty hand to ladle the stew out, or to take an
  empty pot off. A stew heals `STEW_HEAL_FACTOR` (2) times the cooked meat. If the fire burns out the pot and its contents drop (raw meat stays raw).
  Pots are drawn by `effects::draw_pots` and saved as `pot` records.
- **Health**: the player has `MAX_HEALTH` (10) life, shown as hearts above the hotbar. Touching a
  burning fire (`Game::touching_fire`) costs 1 life per full second of contact (`FIRE_DAMAGE_INTERVAL`);
  the count restarts when contact ends. At 0 the player respawns at the start with full health and keeps
  their inventory. Right click with food held eats one, but only when hurt (`ItemKind::heal_amount`):
  berries +1, cooked venison +4, cooked poultry +3, cooked bear meat +5, cooked rabbit +2, cooked moose +5, and every stew double its cooked meat. Raw meat cannot be eaten.
- **Animals** (`animal.rs`): deer, rabbits, quail, bears and moose wander the world, and will-o-whisps appear at night (`Game::update_animals`).
  - A **deer** wanders slowly with pauses. A player **or a bear** within `FLEE_DISTANCE` (10 blocks) makes it run
    directly away at `DEER_RUN_SPEED` (faster than sprinting) until it is `FLEE_CALM_DISTANCE` away.
  - A **rabbit** (`RABBIT_HEALTH` 2) runs from the player or a bear exactly like a deer (`RABBIT_RUN_SPEED` = `DEER_RUN_SPEED`). It drops
    `RABBIT_MEAT_DROP` rabbit meat and, with `RABBIT_FOOT_CHANCE` 25 percent, a rabbit's foot (no use).
  - A **moose** (`MOOSE_HEALTH` 20) does not run and ignores the player until hurt. A hurt moose (`Animal::enraged`) charges at
    `MOOSE_RUN_SPEED` and keeps chasing until the player dies (`Game::hurt_player` calms it) or is `MOOSE_GIVE_UP_DISTANCE` (40) away. It hits
    for `MOOSE_DAMAGE` 2 every `MOOSE_ATTACK_COOLDOWN`. It drops `MOOSE_MEAT_DROP` moose meat and `MOOSE_LEATHER_DROP` leather. At most
    `MAX_MOOSE` 2 and `MAX_RABBIT` 6 live nearby.
  - **Knockback**: a blow that hurts but does not kill an animal throws it back (`Animal::knock_back`, from `Game::hurt_animals`), and a bear or
    moose blow or a fire burn throws the player back (`Player::knock_back`, from `Game::knock_player`). The push is a horizontal speed
    (`ANIMAL_KNOCKBACK_SPEED`, `PLAYER_KNOCKBACK_SPEED`) plus a hop (`KNOCKBACK_HOP`) that decays at `KNOCKBACK_DECAY`.
  - A **quail** hops about. A player or a bear within 10 blocks makes it take off, climb above the treetops, fly
    away for a few seconds, glide down and land. **Every take-off drops a feather** where it was.
  - A **bear** (`BEAR_HEALTH` 28) wanders with pauses. Within `BEAR_NOTICE_DISTANCE` (40) of the player each walk
    points roughly towards them (within `BEAR_WANDER_SPREAD`). Within `BEAR_CHARGE_DISTANCE` (10) it charges at
    `BEAR_RUN_SPEED` (5.4, slower than the 8.2 sprint) and keeps charging until the player is
    `BEAR_CALM_DISTANCE` (20) away, then wanders towards them again. A bear that reaches the player stops against
    them and hits for `BEAR_DAMAGE` (2) every `BEAR_ATTACK_COOLDOWN` (1.2 s) (`Animal::try_attack`; dying respawns
    the player). A bear that is hurt charges the player from any distance for `ANIMAL_ALARM_SECONDS`. Bears never
    chase deer or quail. At most `MAX_BEAR` (1) lives nearby; spawns are weighted by `SPAWN_WEIGHT_*` (a bear's weight is 4, about a quarter of its old 15).
  - A **will-o-whisp** appears only while the world is night. It floats about three blocks above the terrain,
    flees at `WILL_O_WHISP_FLEE_SPEED` when the player comes within `WILL_O_WHISP_FLEE_DISTANCE`, and has no
    attacks or drops. **Holly rod:** `Game::update_animals` sets `Animal::lure` (the player's position) on every whisp
    while `Game::holly_rod_in_hotbar`. A whisp within `WILL_O_WHISP_ATTRACT_DISTANCE` floats towards the player (`Animal::float_to`, no fleeing);
    within `WILL_O_WHISP_BOND_DISTANCE` (five feet) it sets `Animal::bonded` and circles the player, keeping up (a bonded whisp beyond
    `WILL_O_WHISP_TELEPORT_DISTANCE` rejoins at once). Bonded whisps survive the despawn at day. With the rod gone from the hotbar the bond
    is dropped, and by day the whisp despawns. During the day, one or more bonded whisps gather a storm after `WHISP_STORM_GATHER_SECONDS` (20) seconds: the ordinary white clouds are replaced by denser, darker versions, rain and occasional distant lightning appear, and the scene is 20% darker. While the storm is active, hold the Holly Rod and left-click a block to consume one following whisp and call harmless lightning. Once no whisps remain, the storm lingers for `WHISP_STORM_CLEAR_SECONDS` (20) seconds before clearing. Animals are not saved, so bonds are not either. At most `MAX_WILL_O_WHISP` (12) live nearby and its spawn weight is `SPAWN_WEIGHT_WILL_O_WHISP` (100), so at night
    they are common. They are drawn translucent (`animal_draw::will_o_whisp_parts`: halo, flickering body and heart, eyes, a fading
    tail) by `draw_ghosts`, after the water, since `draw_box` honours the colour's alpha.
  - Animals hop up one-block steps and turn aside from taller obstacles. They spawn out of sight
    (`ANIMAL_SPAWN_MIN_DISTANCE`..`MAX`) up to each `MAX_*` limit, are simulated within
    `ANIMAL_UPDATE_DISTANCE`, and vanish beyond `ANIMAL_DESPAWN_DISTANCE`. They are not saved.
- **Hunting**: only an **axe** (stone, copper, bronze or iron: `AXE_DAMAGE` 4, then one more each), a **bronze mallet** or a **bow** hurts animals (the mallet does `MALLET_DAMAGE` 5 per swing, `MALLET_COOLDOWN` 0.6 s, and is also the only tool that breaks boulders); hands, sticks, stones and the rest do not.
  - Axe: with the axe held, an animal under the crosshair within `AXE_REACH` (and not behind a solid
    block) is struck instead of the block behind it, once per `AXE_COOLDOWN` while the left button is held
    (`Game::swing_melee`, `melee_target`). `AXE_DAMAGE` 2.
  - Bow: left click shoots an arrow from the inventory along the crosshair (`Game::shoot_bow`), one per
    `BOW_COOLDOWN`. With a bow in hand left click never digs or chops (`Block::can_break_with`). Arrows fly
    under gravity (`arrow.rs`) and stop at any solid block. An arrow that hits an animal is used up; one that hits a block or expires lands as a dropped
    arrow item (`Game::update_arrows`, `ARROW_DROP_BACKOFF`) that can be picked up and shot again. `ARROW_DAMAGE` 3.
  - Health: deer `DEER_HEALTH` 6, quail `QUAIL_HEALTH` 2, bear `BEAR_HEALTH` 28 (seven axe blows or ten arrows). A
    hurt deer or quail flees for `ANIMAL_ALARM_SECONDS` whatever the distance. A **deer** drops 2 venison and 1
    leather; a **quail** drops 1 poultry and `QUAIL_FEATHER_DROP` (3) feathers; a **bear** drops 3 bear meat and 1 bear pelt.
- Dropped items are small physics boxes; walking near them picks them up (`PICKUP_*` in `config.rs`).
- Inventory: 36 slots, stacks of up to 64. Slots `0..9` are the hotbar.
- Hotbar selection: keys `1`-`9` or mouse wheel. The selected slot is outlined; its item name
  shows above the hotbar. `Game::held_item()` is what gameplay code should read.
- Inventory screen (`E`): drag a stack onto another slot to move it (empty target: move; same kind:
  merge up to the stack limit; different kind: swap). The right-hand panel lists every recipe the
  current items can pay for; clicking one crafts it once.
- Recipes are **shapeless**: a fixed number of each input item, no grid. Current recipes:
  - Twine = 3 Grass.
  - Bow Drill = 1 Twine + 1 Stick.
  - Stone Axe = 1 Stick + 1 Stone + 1 Twine.
  - Copper Axe = 1 Stick + 2 Copper + 1 Leather Cord; Bronze Axe = 1 Stick + 2 Bronze + 1 Leather Cord; Iron Axe = 1 Stick + 2 Iron + 1 Leather Cord (damage 5, 6, 7).
  - Tin Pot = 3 Tin. Clay Jar = 4 Clay.
  - Bow = 1 Twine + 1 Stick.
  - Bellows = 1 Twine + 1 Leather + 2 Sticks.
  - Leather Cord x3 = 1 Leather.
  - Clay Forge = 2 Stone + 8 Clay.
  - Bronze Mallet = 2 Bronze + 1 Leather Cord + 1 Stick.
  - Arrow = 1 Pebble + 1 Stick + 1 Feather.
  Bear pelts and iron have no further use yet. The recipe list scrolls with the mouse wheel when it has more entries than rows.
- **Dev mode** is chosen on the create-world screen (`App::dev_mode`, saved as the `mode dev` record, `Game::dev_mode`). It changes only the inventory screen: after the craftable recipes, the right-hand list continues with every `ItemKind::ALL` item (`hud::craft_entries`); a click adds one (`hud::dev_give`), a right click a stack. Normal mode is unchanged.
- Controls: WASD, mouse or arrow keys to look, Space jump, Shift sprint (`SPRINT_SPEED` 7.2), left mouse break/attack/shoot,
  right mouse place/eat/cook, `E` inventory,
  `Esc` closes the inventory, or saves and returns to the menu when it is closed.

## Code map (`src/`)

| File | Responsibility |
| --- | --- |
| `main.rs` | Module list, window config, top-level loop dispatching on `app::Mode`. |
| `config.rs` | **All tunable constants** (sizes, speeds, drop chances, clearing, hill, fire, cooking, animal and hunting numbers). Add new tunables here. |
| `block.rs` | `Block` enum, block shapes (`parts`, `bounds`), and rules (`hand_breakable`, `can_break_with`, `opaque`, `collides`). |
| `item.rs` | `ItemKind` (name, save id, colour, `placeable_block`), `ItemStack`, and block drop rolls (`drops_for*`). |
| `inventory.rs` | `Inventory`: `add`, `remove`, `take_from_slot`, `count`, `move_stack`. Pure logic, fully tested. |
| `recipe.rs` | `RECIPES` table, `craftable(&Inventory)`, `craft(&mut Inventory, &Recipe)`. |
| `game.rs` | `Game`: one running session (world, player, inventory, items, animals, arrows, selected slot, `mine`, `update_mining`, `update_health`, `eat_held`, `use_held` incl. cooking, `try_ignite`, `swing_melee`, `shoot_bow`, `update_animals`, `update_arrows`). |
| `animal.rs` | `Animal`, `AnimalKind`, `Mode`: deer, quail, bear, rabbit, moose and will-o-whisp behaviour, health and drops. Pure logic. |
| `arrow.rs` | `Arrow`: flight under gravity and `Fate` (hit animal, hit block, expired). Pure logic. |
| `animal_draw.rs` | Box models for animals, and arrows (thin wrapper over `draw_box`). |
| `play.rs` | The in-game frame: input, simulation step, 3D render, HUD. Calls into `hud.rs` and `effects.rs`. |
| `hud.rs` | Hotbar, inventory screen (layout, drag and drop, crafting list), item icons, toasts. |
| `effects.rs` | Animated flames, ember glow, whatever lies cooking or smelting on a fire, the blocky rainbow (`rainbow_blocks`, `draw_rainbow`) and the ghostly trail (`trail_wisps`, `draw_trail`). Dynamic, so chunk meshes stay static. |
| `dropped.rs` | Items on the ground: spawn, physics, pickup, drawing (`draw_box` is shared with `effects.rs`). |
| `world.rs` | Chunk streaming, block access, edits (`break_block`, `set_block`), fire timers and bellows glow, cooking and smelting on fires, shape-aware collision (`box_blocked`), raycast (`raycast_hit`), `ray_box`. |
| `terrain.rs` | Generation: ground (`ground_top_with`, hills and valleys as `Relief`, river and lake beds and water, mountains and their snow), trees, clearings and their plants. `generate_chunk_with` takes the world's `Landmarks`. |
| `horizen.rs` | `Dimension` and the Horizen's land: empty flat ground with ponds and the arrival pool. |
| `tornado.rs` | The Horizen's one daytime fire tornado (movement, target capture, collision and damage timing) and the `Cyclone` that carries the player between dimensions. |
| `skylands.rs` | The third dimension: floating islands on a grid, wooden bridges between them, a few oaks (`generate_chunk`, `surface_top`, `spawn_point`, `home_cyclone`). Pure functions of the seed. |
| `landmarks.rs` | `Lake`, `Mountain`, `GrandCastle`, `Rainbow`, `RainbowState`, `Landmarks` (what the terrain consults) and `Treks` (the river and compass walk counters that bring them in). Pure logic, tested without a window. |
| `river.rs` | `is_river`, `river_near`: rivers as zero contours of a smooth noise field. Pure functions of seed and column. |
| `chunk.rs`, `mesh.rs`, `culling.rs`, `rng.rs` | Storage, meshing (AO, parts, splitting), view culling, seeded RNG. |
| `player.rs` | Player state, keyboard input, AABB physics. |
| `save.rs` | World file format, load/save, world listing, seed parsing. |
| `app.rs`, `menu.rs`, `ui.rs` | App state, world select/create screens (each world has a Delete button that asks to confirm: `App::ask_delete`, `confirm_delete`, `save::delete_world`), small immediate-mode widgets. |
| `help.rs` | The Help screen (button on the world list). `help_lines()` builds the text from `RECIPES` and the config constants, so it stays true to the rules; `layout` wraps it; `help_screen` draws and scrolls it. |

Data flow per frame in `play::play_screen`: input (`E`, hotbar keys, look) -> chunk streaming and
meshing -> player physics -> raycast -> `handle_attacking` (axe swing, bow shot) -> `handle_breaking` (`Game::update_mining`: hold to break, or ignite) -> `handle_placing` ->
`Game::update_landmarks` (treks, rainbow) -> `Game::update_items` -> `Game::burn_fires` -> `Game::update_animals` -> `Game::update_arrows` -> `Game::update_health` -> autosave -> `render_world` (chunks, items, flames,
target outline) -> `render_hud`.

## Conventions and invariants

- **Dropped items are capped** at `MAX_DROPPED_ITEMS` (100) by `Game::limit_items`, called from `update_items`. The oldest go first, judged by
  `DroppedItem::age` (not by list position: `collect_nearby` uses `swap_remove`, so the list is not in order of age). Items of equal age,
  such as those just loaded from a save, go in list order.

- **Logic vs. UI.** Rules live in `Block`, `ItemKind`, `Inventory`, `Recipe`, `World` and `Game`
  methods, each with unit tests beside the code (`#[cfg(test)] mod tests`). UI code is immediate-mode:
  it reads input and draws in the same function, and delegates every state change to those methods.
- **Block shapes.** A block is either a plain full cube (`parts()` is empty) or a set of coloured
  boxes (`parts()`), in cell-local 0..1 coordinates. `bounds()` is the box around them. Three
  separate properties decide how a block behaves, and they are independent:
  - `opaque()`: a full cube that hides its neighbours' faces (dirt, bedrock, leaves, needles). Anything
    else leaves the neighbours' faces drawn, so the ground shows around a thin trunk and under plants.
  - `collides()`: whether the player and dropped items bump into it. Plants, pebbles, stones and
    charcoal are walk-through; trunks, placed logs and fire are not.
  - `bounds()`: used for collision (`World::box_blocked`), aiming (`World::raycast_hit`) and the target
    outline, so aim and collision follow the visible shape, not the whole cell.
- **Ground height is per column.** Never assume the surface is at `GROUND_TOP`: use
  `terrain::ground_top(seed, wx, wz)` (the highest natural dirt block; `GROUND_TOP` only on flat ground and
  in the spawn area, which hills avoid). Trees, plants, animal spawns and the grass-coloured top face in
  `mesh.rs` all follow it.
- **Dirt and water are never breakable** (`Block::breakable`), so `World::ensure_chunk` ignores saved removals of them.
- **Breaking is decided by `Block::can_break_with(held)`**, used by `Game::mine`, `play::handle_breaking`
  and (with ignition, via `Game::can_interact`) the target outline colour (black = you can act on it,
  red = not). Do not duplicate the rule. `Block::breakable()` means "can ever be broken by a player"
  and is used when replaying saved edits (`World::ensure_chunk`).
- **World edits are two saved sets**, both replayed when a chunk is generated, in this order:
  `World::removed` (generated blocks the player broke) and then `World::placed` (blocks the player
  made: placed items, fire, charcoal). `break_block` forgets a placed block, otherwise records a
  removal; `set_block` records a placement. Always go through these, never write `chunk.blocks` directly
  outside tests. A block type must have a save `id()` (see `Block::PLAYER_MADE`) to be placeable.
- **Fire** state is a block (`Block::Fire` in `placed`) plus a timer in `World::fires`.
  Use `World::ignite`, `feed_fire` and `update_fires`; a burnt-out fire becomes `Block::Charcoal`.
  A fire block is `is_burning()`: `Block::Fire` (a lit log, leaves charcoal) or `Block::Embers` (lit charcoal,
  leaves ash). `World::ignite` lights either; `World::blow` sets `World::glow` for embers. Meat, stones and
  pebbles on a fire are `World::cooking` (position -> `Cooking { raw, left }`; what each becomes is
  `ItemKind::processed`, and `needs_glow` marks the ones that need glowing embers); use `lay_on_fire`,
  `take_product` and `drain_released` (what fell off burnt-out fires; `Game::burn_fires` turns it into items).
- **Animals are entities, not blocks.** They live in `Game::animals`, are advanced by `Animal::update`
  (given the point it reacts to: the player, or for deer and quail the nearer of the player and a bear; it returns a dropped item, e.g. a quail's feather; bears strike through `Animal::try_attack`) and are never written to the world or the save.
  Damage goes through `Game::hurt_animals`, which removes the dead and spawns `AnimalKind::drops`.
- **Breaking time** comes from `Block::break_seconds`; `Game::mine` is the instant break (drops and world
  edit) that `Game::update_mining` calls once the hold time is reached.
- **Drops** come only from `item::drops_for`. It does not check tools; `Game::mine` does.
- **Inventory mutations** must go through `Inventory` methods. `craft` is all-or-nothing (it works on
  a clone and commits only if the output fits).
- **Slot layout** in `hud.rs` comes from `layout()`/`slot_rect()`/`slot_at()`. Draw and hit-test with
  the same functions so they cannot drift apart.
- When the inventory is open, player movement, look, breaking, placing and hotbar selection are
  disabled, the cursor is released, and any drag is cancelled on close (`play::set_inventory_open`).
- **Meshing** (`mesh.rs`): plain cubes get face culling and ambient occlusion; part-based blocks get flat
  shading. Leaves and needles beside a trunk are pushed out by `TRUNK_INSET` so no gap shows, and the
  face between two leaves of different reach is not culled. Macroquad silently truncates oversized
  meshes: `MeshBuilder` splits at `MAX_QUADS_PER_MESH`; keep it.
- Keep `cargo build` warning-free. Match the existing style: short doc comments, no dead code.

## Terrain, hills, valleys, rivers and clearings

Hills and valleys (`terrain::relief_in_region`) follow the same scheme as clearings, in regions of `HILL_REGION_CHUNKS`
chunks, at most one per region and wholly inside it, so a chunk only asks about its own region (`relief_for_column`).
One roll per region decides: a hill (`HILL_CHANCE`), a valley (`VALLEY_CHANCE`) or nothing. A feature is a disc with a
smoothstep profile (`Relief::rise_at`, negative for a valley); a hill has radius `HILL_MIN_RADIUS..=HILL_MAX_RADIUS` and
rises `HILL_MIN_HEIGHT..=HILL_MAX_HEIGHT` blocks, a valley sinks `VALLEY_MIN_DEPTH..=VALLEY_MAX_DEPTH`. It never comes
within `HILL_SPAWN_CLEARANCE` of the spawn point nor within `RIVER_RELIEF_MARGIN` of a river (it is moved up to
`RELIEF_PLACEMENT_TRIES` times, then dropped, so rivers leave fewer hills and valleys than the chances say). Because
that check scans the whole disc, answers are memoised per region in a thread-local cache. `generate_chunk_blocks`
works out every column's surface once (`surface`), fills dirt up to it, fills river columns with water up to
`GROUND_TOP`, and stands trees, clearing plants and rocks on dry columns. Relief and clearings are independent layers,
so a clearing can lie on a hill or in a valley.

Rivers (`river.rs`) are where `|field| / |gradient|` is below the local half-width (`RIVER_MIN_HALF_WIDTH..=RIVER_MAX_HALF_WIDTH`),
the field being two octaves of value noise (`RIVER_BIG_SCALE`, `RIVER_SMALL_SCALE` weighted by `RIVER_WIGGLE`) lifted
around the spawn point so rivers bend round it. Water is drawn as a slab `WATER_SURFACE_DROP` below the ground level;
the water's own mesh has only its top and any side that meets neither water nor a solid block; the bed beneath it is
drawn as mud.

Clearings (`CLEARING_CHANCE` 50%, radius 6 to 9) are generated per square region of `CLEARING_REGION_CHUNKS` chunks, at most one per region. A clearing and its tree buffer (`CLEARING_TREE_BUFFER`) always lie wholly inside
its region, so a chunk only asks about its own region (`terrain::clearing_for_chunk`) and chunks stay
independent. Inside the disc: no tree trunks, and each column rolls (by position hash) for grass, a
flower or a bush on the ground (`GRASS_PERCENT`, `FLOWER_PERCENT`, `BUSH_PERCENT`). Flower colours are
picked per position by the mesher. The spawn point is kept clear of trees as before.

## Landmarks: lakes, mountains and the rainbow

The land is a pure function of the seed, except for three things the player brings in by exploring. They live in
`World::landmarks` (`landmarks.rs`), are saved with the world, and the terrain generator consults them. Adding a lake
or mountain (`World::add_lake`, `add_mountain`) regenerates the loaded chunks it touches, and the player's edits are
applied to them again as usual. Every number is in the "Landmarks" section of `config.rs`.

- **Lake.** `Treks::step` is called every frame (`Game::update_landmarks`). Wading (`Game::player_in_water`: water in the
  feet cell or the two below, so a jump does not count) until the player is `LAKE_TREK_CHUNKS` (10) chunks from where they
  entered, along either axis, makes a `Lake` ahead of them, beyond `GEN_RADIUS` so it does not pop in. It is a wavy basin of
  radius about 130 (`LAKE_RADIUS`), one block deep (`LAKE_DEPTH`), plus a straight channel from the player's column so the water is
  continuous. In the middle is an island (`LAKE_ISLAND_SHARE` of the radius, so about 58 blocks): a hill (`Lake::hill_rise`) `ISLAND_HILL_HEIGHT` blocks
  high with a flat top of `ISLAND_PLATEAU_RADIUS`, climbable by jumping (a test checks every step), with forest on its low slopes
  (`TREE_LINE_RISE`), and on the flat top a castle of `Block::CastleStone` (`Lake::castle_block`, shape in `castle_is_solid`: an outer wall with a gate in
  the south side, four corner towers, a keep with a door and roof, all with battlements). The castle square is cleared of
  plants and rocks and kept free of trees (`CASTLE_TREE_MARGIN`). A lake with too small an island has no castle (`Lake::has_castle`).
  Hills fade out beside it (`Landmarks::relief_scale`), so no cliff is left.
- **Mountain.** Holding a Compass, `Treks` adds the distance walked north (`COMPASS_NORTH_BEARING`, towards smaller z); putting it
  away resets the count, walking south subtracts. At `COMPASS_TREK_CHUNKS` (10) a `Mountain` of `MOUNTAIN_HEIGHT` 170 (ten times the first version) and `MOUNTAIN_RADIUS` 400 appears
  ahead. Its base slope is under one block per column, but `MOUNTAIN_MAX_LEDGES` (12) rings of `MOUNTAIN_LEDGE_HEIGHT` (2) block cliff break it
  (`Mountain::rise_at`). Each ring has one gap, a ramp, and the gaps alternate sides (`Mountain::gap_angle`), so the way up zigzags. Tests check
  that a path of one-block steps reaches the summit and that no straight line from any of 72 directions can be climbed. From
  `SNOW_LINE_RISE` up the ground is `Block::Snow`: solid, opaque and unbreakable. No trees grow above `TREE_LINE_RISE`, and the exposed ground above it is drawn as grey rock instead of grass (`mesh.rs`).
  A mountain stands over any river or lake beneath it.
- **Rainbow.** With a Peep Stone in hand and `RainbowState::Unseen`, `Game::try_show_rainbow` places a `Block::GoldPile`
  26 to 36 blocks away on dry bare ground (saved like any placed block) and sets `RainbowState::Active`. The arc is built of translucent blocks (`effects::rainbow_blocks`) and is drawn
  only by day while a Peep Stone is held (`Game::visible_rainbow`); at night a held Peep Stone makes the grand castle instead. Mining the pile drops `RAINBOW_GOLD_DROP` gold and sets
  `RainbowState::Spent`, which is saved and never reverts: there is no further rainbow.
- **Ghostly trail.** `Game::trail_path` gives the path a held Peep Stone shows: by day to `Landmarks::rainbow_end` (kept in
  `spent_gold` once the gold is taken), by night to the grand castle's gate and keep door. `effects::trail_wisps` is the geometry and
  `draw_trail` draws it, `TRAIL_LENGTH` blocks ahead.
- **Grand castle.** At night, with a Peep Stone in hand and no castle yet, `Game::try_show_grand_castle` places a `GrandCastle` at least
  `GRAND_DISTANCE` blocks away (along either axis) in a direction `Landmarks::has_room_for` finds clear of lakes and mountains. Hills fade out
  round it and no river or tree is allowed in its clearing, so it stands level. `grand_castle_is_solid` is the shape: outer wall, gate, towers,
  and a keep of `GRAND_STOREYS` floors with stairs; a test walks from the gate to the roof.
- **Day and night.** Drinking a stew calls `Game::set_night`, which starts a `Blackout` of `DAY_NIGHT_BLACKOUT_SECONDS`; the world changes
  once the screen is black (`Game::update_blackout`), and `play.rs` draws the black overlay last.
- Trek progress is not saved; it starts over when a world is loaded. A step of more than 12 blocks in one frame (a respawn)
  also resets it.

## Save format

Plain text, one record per line, in `saves/<sanitized name>.world`:

```
<world name>
<seed>
<px> <py> <pz> <yaw> <pitch>
<x> <y> <z>                      one line per removed block
inv <slot> <item id> <count>     one line per occupied inventory slot
item <item id> <x> <y> <z>       one line per item lying on the ground
place <x> <y> <z> <block id>     one line per player-made block
fire <x> <y> <z> <seconds>       seconds left for the fire block at that position
cook <x> <y> <z> <item id> <s>   meat, stone, pebble, tin or iron ore on that fire, with the seconds of heat still needed
pot <x> <y> <z> <0|1> <meat id or -> <s>   a tin pot on that fire: water (0/1), raw meat in it, seconds of heat the stew still needs
glow <x> <y> <z> <seconds>       seconds of bellows glow left on the embers at that position
health <n>                       life points, 1 to 10 (missing or invalid: full health)
mode dev                         a dev-mode world (no line: normal mode)
lake <x> <z> <radius> <phase> <from x> <from z>   a lake and its channel to the river
mountain <x> <z> <radius> <height> <phase>        a mountain
rainbow spent                    the gold was taken: never another rainbow
rainbow <from x> <from z> <x> <y> <z>   the rainbow is out, its gold pile at (x, y, z)
rainbowend <x> <y> <z>           where a spent rainbow's gold lay (the peep stone still leads there)
grandcastle <x> <z>              the grand castle's centre
```

Unknown or malformed records are ignored, and older saves without the later lines still load.
Old item ids `copper_ore` and `tin_ore` load as `stone` and `pebble` (what smelts into those metals now).

Item ids come from `ItemKind::id()`: `stick`, `pebble`, `stone`, `log`, `stone_axe`, `grass`, `berries`,
`twine`, `bow_drill`, `charcoal`, `plank`, `feather`, `bow`, `arrow`, `venison`, `poultry`, `cooked_venison`,
`cooked_poultry`, `leather`, `bear_meat`, `cooked_bear_meat`, `bear_pelt`, `bellows`, `ash`, `copper`, `tin`, `iron`, `clay`, `iron_ore`, `leather_cord`, `bronze`, `bronze_mallet`, `clay_forge`, `rabbit_meat`, `cooked_rabbit`, `moose_meat`, `cooked_moose`, `rabbit_foot`, `peep_stone`, `holly_rod`, `copper_axe`, `bronze_axe`, `iron_axe`, `tin_pot`, `clay_jar`, `water_jar`, `venison_stew`, `poultry_stew`, `bear_stew`, `rabbit_stew`, `moose_stew`, `red_berry_stew`, `purple_berry_stew`, `sling`, `loadstone`, `compass`. Block ids come from `Block::id()`: `placed_log`,
`placed_pebble`, `placed_stone`, `fire`, `charcoal`, `planks`, `embers`, `ash`, `clay_forge`, `forge_charged`, `forge_lit`, `gold_pile`. `Snow` and `CastleStone` are never saved (they are generated). Animals, arrows and treks in progress are not saved. The selected hotbar slot is not saved (it resets to slot 1 on load).

## How to extend

**New item**
1. Add a variant to `ItemKind` in `item.rs`, and to `ItemKind::ALL`. Fill in `name`, `id`, `color`.
2. In `dropped.rs`, add its `rest_half_height` and `draw_half_extents` (the ground box).
3. In `hud.rs`, add an icon arm in `draw_item_icon`.
4. To make it placeable, return a block from `ItemKind::placeable_block`.

**New craftable**: append a `Recipe` to `RECIPES` in `recipe.rs` and add a test. The crafting panel
shows only recipes the player can currently afford, and at most as many rows as fit beside the
inventory grid; add scrolling in `hud::inventory_screen` before the list grows past that.

**New block**: add a variant in `block.rs` with its `parts()` (or leave them empty for a full cube),
colours, and the `hand_breakable`/`breakable`/`can_break_with`/`opaque`/`collides` rules; add drops in
`item::drops_for`; handle it in `terrain.rs` if it is generated. If players can make it, add it to
`Block::PLAYER_MADE` with an `id()`. Add the variant to the `ALL` list in the `block.rs` tests.

**New tool or held-item behaviour**: read `game.held_item()`; extend `Block::can_break_with` for
tool-gated breaking, or add an action next to `Game::try_ignite` and call it from `play::handle_breaking`.

**New creature**: add the `AnimalKind` data and movement rules in `animal.rs`, add spawn limits and
night/day gating in `Game::spawn_wildlife`, add a model in `animal_draw.rs`, then decide explicitly whether
the creature can be hunted, attacks, or drops anything. Add focused behaviour tests before wiring it into the UI.

## Known limitations

- Placed logs always lie along the x axis (the cut ends face +x and -x), whatever way you face.
- Burning fire cannot be put out or broken; it only burns down.
- Dropped items inside a cell where a solid block (log, fire) is then placed can get stuck in it.
- The remaining fire time is not shown on screen.
- Animals are not saved: killed ones come back, and new ones wander in, when a world is loaded.
- Arrows are used up on impact (none can be picked up again), and an arrow that hits a plant-covered
  cell is not stopped by the plant (plants do not collide).
- Bear pelts and iron have no use yet. Planks cannot be made any more, so there is
  no way to build walls.
- Rivers are one block deep: you wade through them, there is no swimming, and water does not slow you down.
- A player respawns at the spawn point, which may be where a bear is waiting if they died near it.

## Update: one mountain, the bone totem, the Horizen, wearing and the day/night castle

- **One mountain.** `Landmarks::mountain()`; `World::add_mountain` ignores a second. Once it is out, `compass_bearing` points the
  compass needle at it and `Treks::mountain_found` stops the north trek. The rainbow and the grand castle were already single, and
  a peep stone's trail already leads to them for ever. Lakes remain unlimited.
- **Axes** deal `AXE_DAMAGE` 4, `COPPER_AXE_DAMAGE` 6, `BRONZE_AXE_DAMAGE` 8, `IRON_AXE_DAMAGE` 10.
- **Moose** drop `Antlers`; Antlers + 2 Gold craft a `BoneTotem`. Held by day it trails to the single `Well`, by night to the
  single `Graveyard` (`try_show_well_or_graveyard`, `trail_path`).
- **Well:** 4x4 water sunk to the stone floor, one-block rim. `Game::update_dimension` waits for the player's feet to collide with that floor before starting the blacked-out transfer.
- **Horizen** (`horizen.rs`): a second `World` (`World::dimension`), kept in `Game::away` while the player is elsewhere, with its own
  edits, items and day/night state. It has no wildlife or landmarks. The well only transports the player after their feet hit the
  stone floor; the screen blacks out and the player appears on the dedicated arrival pool. At night the stars rotate quickly.
  By day, `Game::tornado` holds one fire tornado: it spawns every `HORIZEN_TORNADO_INTERVAL` seconds 40 blocks away, travels to
  the player's captured spawn position, stays there, deals periodic damage on contact, and is cleared by nightfall. Saves: `dimension
  horizen` plus `h <record>` lines, including the Horizen's own `time` record.
- **Wearing:** `Game::worn` holds one `PeltCloak` or `RoyalArmor` (right click, or the Worn slot in the inventory; saved as `worn`).
  A cloak sets `Animal::pacified` on bears that are not `provoked`; the armour halves damage, rounding up.
- **Grand castle by day and night:** `grand_castle_cell` (night whole, gate and throne-room door shut, stair works, armour in the
  throne room; day ruined, gate open, first stair broken, throne door open). `World::set_night` regenerates the castle's chunks.

## Update: settings, mineshafts, lightning, rainbow and animals

- **Settings** (`settings.rs`): a button on the world list, and F2 in a game. A slider sets the render distance, 3 to 10 chunks (default 3), kept in `saves/settings.cfg`. `config::render_radius()` / `gen_radius()` replace the old `RENDER_RADIUS` / `GEN_RADIUS` constants. Creature spawn, update and despawn distances are plain constants and do not follow it. New structures (lake, mountain, grand castle, well, graveyard) are placed `config::structure_distance()` away (`lake_distance()`, `mountain_distance()`, ...), beyond the generated ring, and the rainbow and its gold at `rainbow_distance()`, so none appears inside the render distance. `World::is_bare_dry_ground` checks a column even if its chunk is not loaded.
- **Mineshafts** (`mines.rs`): `MINE_ENTRANCES` (5) timber-framed stairs at the foot of the mountain lead into shafts that climb one block per two towards the core; three rings (`MINE_RING_SHARES`) join them, with a chamber at each crossing. The network is a pure function of the `Mountain` (cached per thread), carved per chunk by `terrain::generate_chunk_for` before trees, plants and rocks, and those skip open columns. Tunnels keep `MINE_COVER` blocks of ground over their roof.
- **Lightning**: `Game::trigger_storm_lightning` hurts creatures within `LIGHTNING_RADIUS` (`LIGHTNING_DAMAGE` at the centre, half at the edge) and never the player. A held Holly Rod strikes the creature or block it points at within `HOLLY_ROD_REACH` (`Game::holly_rod_strike_point`). The bolt is drawn as thick beams (`effects::draw_storm_lightning`, `bolt_points`) with a screen flash (`lightning_flash_alpha`). Storm clouds use `STORM_CLOUD_THRESHOLD` / `STORM_CLOUD_RADIUS_CELLS` and cover nearly the whole sky.
- **Fire tornado** is a funnel (`effects::tornado_radius_share`), spinning at `HORIZEN_TORNADO_SPIN_SPEED` 12 rad/s.
- **Rainbow**: `rainbow_blocks_on` drops cells buried in the ground and carries each band's ends down to it, so all seven colours reach the ground; the blocks are cached (`RAINBOW_CACHE`).
- **Animals** (`animal_draw.rs`): the five animals are posed from two-jointed legs (`leg`), tilted boxes (`dropped::draw_box_tilted`), breathing, head bobs, grazing, pecking, hopping and ear/tail motion. Their eyes (`eyes`) stick out of the head and glow red at night with a halo.
- Six tests (bear health, mallet damage, stick rate, well pool) already failed before these changes; they assert old balance numbers.

## Update: the Yeti, the Loch Ness Monster, the cyclone and the Skylands

- **Item art.** `hud::draw_item_icon` draws every item lit from the upper left (outline, shaded underside, highlight; helpers `ball`, `rod`,
  `slab`, `quad`, `tri`). A **jar of water** is clear glass with blue water in it, as an icon and as a dropped item (`dropped::draw_jar`, translucent);
  an empty clay jar is opaque terracotta.
- **Bosses** are `AnimalKind::Yeti` and `AnimalKind::LochNessMonster` (`AnimalKind::is_boss`), so axes, arrows, the mallet, lightning and knockback
  work on them like on any animal. They are never wildlife (`Game::spawn_bosses` brings them out; they are not saved, so a boss that is not dead comes back at
  full health when the player returns). A boss is simulated within `BOSS_UPDATE_DISTANCE` and a **health bar** with its name is drawn at the top of the
  screen while the nearest boss is within `BOSS_BAR_DISTANCE` (`Game::boss_bar`, `play::draw_boss_bar`).
  - **Yeti** (`YETI_HEALTH` 300, `YETI_DAMAGE` 3 every `YETI_ATTACK_COOLDOWN`): lives on the summit, which is now a flat arena of
    `SUMMIT_PLATEAU_RADIUS` (`Mountain::rise_at`). It appears when the player comes within `YETI_SPAWN_DISTANCE`, charges a player within
    `YETI_NOTICE_DISTANCE` (slower than a walking player), never strays more than `YETI_LEASH` from the summit, and below half health is enraged (faster, quicker blows).
    Drops bear pelts and gold. Killing it sets `Landmarks::yeti_slain` (saved as `yetislain`).
  - **Loch Ness Monster** (`LOCH_NESS_HEALTH` 320): one for each lake with a castle, rising in the water between the island and the far shore when the
    player is within the lake's radius plus `LOCH_NESS_SPAWN_MARGIN`. It swims only on wet columns (`World::is_wet_column`), turns aside from the shore, hunts a
    player within `LOCH_NESS_NOTICE_DISTANCE` as near as the water lets it, and strikes with its neck at `LOCH_NESS_REACH`. Drops gold and iron.
    The outer **gate of the castle is shut** (`Block::CastleDoor` in the gate's opening, `Lake::castle_cell(.., gate_shut)`, `Landmarks::gate_shut`) until that lake's monster dies;
    `World::open_lake_gate` then records the lake in `Landmarks::monsters_slain` (saved as `nessie <x> <z>`) and generates the castle's chunks again.
- **Cyclone** (`tornado::Cyclone`, `Game::cyclone`, `effects::draw_cyclone`): once the Yeti is dead one stands on the summit. Standing in it for `CYCLONE_SECONDS` (5)
  carries the player to the Skylands; the screen fades to white over the last `CYCLONE_FADE_SECONDS` (`Game::cyclone_whiteness`) and clears again on arrival. A second
  cyclone on the Skylands' first island carries the player home, setting them down beside the summit cyclone, not in it.
- **Skylands** (`Dimension::Skylands`, `skylands.rs`): no ground, water, creatures or landmarks; own day and night, edits and items. Islands (grass on dirt over
  tapering rock, `SKY_CELL` apart, tops within `SKY_HEIGHT_JITTER`) are joined by `SKY_BRIDGE_WIDTH`-wide decks of `Block::BridgePlank` with `Block::BridgePost` rails: every
  island to its east neighbour, and to its south one (always in column 0, otherwise `SKY_SOUTH_BRIDGE_PERCENT`), so everything is reachable from the first island, where the
  player arrives (`SKY_SPAWN_CENTRE`). Tests walk every bridge a step at a time. Dirt is grass-coloured on top whatever the height (`mesh.rs`).
  - **Falling off** (`Game::update_travel`, `Whiteout`, `WhiteoutKind::Fall`): below `SKY_VOID_Y` the screen whites out over `SKY_FALL_FADE_IN`; once white the player leaves the Skylands and is put
    `SKY_FALL_DROP` (50) blocks above the normal world's spawn point, and the white clears over `SKY_FALL_FADE_OUT` as they fall onto it. The Skylands never draw the overworld, except behind
    that white screen (`Game::overworld_backdrop`, a second camera pass in `play::render_world` standing over the summit).
  - Dying in the Skylands, like dying in the Horizen, returns the player to the start of the overworld.
- **Dimensions** are parked in `Game::away` and `Game::shelf` (`Game::travel_to`, `parked`, `take_parked`), so all three can be held at once.
- **Save format** gains `dimension skylands`, `yetislain`, `nessie <x> <z>` and `s <record>` lines (the Skylands' own `pos`, `time`, edits and `item`s, exactly like the `h` lines).
- Tests: `game::boss_and_skylands_tests`, `skylands::tests`, `landmarks::boss_landmark_tests`, and the boss models in `animal_draw`.
