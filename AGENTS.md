# Folk Craft: guide for agents (and humans) who change the code

Folk Craft is a small Minecraft-style voxel game in Rust using [macroquad](https://macroquad.rs) 0.4. This file is the technical
reference: how to build and test, where each rule lives, the conventions to keep, the save format, and how to add an item, block,
recipe, creature, dimension feature or shop ware. Players should read `README.md` instead.

## The short version

1. Read the section of this file and the module that owns what you are changing (see the code map).
2. Put gameplay rules in the logic modules and tunable numbers in `config.rs`; keep input and drawing in the thin UI wrappers.
3. Add or update unit tests beside the logic. Tests must not need a window or GPU.
4. Run `cargo build` (no warnings) and `cargo test` (every test passes: there are no known failures). Check `git diff --check`.
5. Update this file (the right section, plus a short "Update" entry at the end when the change is big) and the in-game help
   (`help.rs`, which builds its text from `RECIPES` and the `config.rs` constants).
6. Keep old saves loading: item and block ids and the save records are a plain-text format that must stay readable.

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

## The change loop

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
- Inventory: 45 slots (`INVENTORY_SLOTS`), stacks of up to 100. Slots `0..9` are the hotbar.
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
| `horizen.rs` | `Dimension` and the Horizen, a cave: arrival chamber and pool, a winding tunnel (`tunnel_point`, `route`), one very large cavern (`cavern_centre`) and ponds on the floor. `air_height` is the cave's shape. |
| `hazard.rs` | The Yeti's ice balls and the Phoenix's fireballs (`Projectile`, `Burst`) and the line of acid water Nessie makes (`acid_line`, `AcidCell`). Pure logic. |
| `tornado.rs` | The Skylands' one daytime fire tornado (movement, target capture, collision and damage timing) and the `Cyclone` that carries the player between dimensions. |
| `skylands.rs` | The third dimension: floating islands on a grid, wooden bridges between them, a few oaks (`generate_chunk`, `surface_top`, `spawn_point`, `home_cyclone`). Pure functions of the seed. |
| `tower.rs` | The silver tower: `tower_cell` (shape, locked door, spiral stair, relic plinth), `Relic`, and `Tower` (the Horizen's, with its trail approach). Pure functions. |
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
- An arrow that hits a plant-covered cell is not stopped by the plant (plants do not collide). Arrows that miss lie on the ground and can be picked up.
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
  edits, items and day/night state. It has no wildlife (except bats) or landmarks. The well only transports the player after their feet hit the
  stone floor; the screen blacks out and the player appears on the dedicated arrival pool. At night the stars rotate quickly.
  By day, `Game::tornado` holds one fire tornado: it spawns every `SKY_TORNADO_INTERVAL` seconds 40 blocks away, travels to
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
- **Fire tornado** is a funnel (`effects::tornado_radius_share`), spinning at `SKY_TORNADO_SPIN_SPEED` 12 rad/s.
- **Rainbow**: `rainbow_blocks_on` drops cells buried in the ground and carries each band's ends down to it, so all seven colours reach the ground; the blocks are cached (`RAINBOW_CACHE`).
- **Animals** (`animal_draw.rs`): the five animals are posed from two-jointed legs (`leg`), tilted boxes (`dropped::draw_box_tilted`), breathing, head bobs, grazing, pecking, hopping and ear/tail motion. Their eyes (`eyes`) stick out of the head and glow red at night with a halo.

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

## Update: silver towers, the salt mine, the jungle and deer bones

- **Silver towers** (`tower.rs`): one design (9x9, 26 high, five floors of `TOWER_STOREY` blocks, a spiral stair that turns a quarter each floor, slit windows,
  battlements) built three times: on every lake island in place of the old castle (`Lake::tower_cell`, relic Philosopher's Stone), on one Skylands island
  (`Landmarks::sky_tower`, relic Horn of the Hunt) and on the Horizen's plain (`Landmarks::horizen_tower`, relic Ancient Crown). The door is `Block::TowerDoor` +
  `TowerDoorTop` until a `SilverKey` is used on it (`Game::try_unlock`, right click; the key is kept: it only turns the lock); `World::unlock_tower` records it in `Landmarks::towers_unlocked`
  and remakes the chunks. Killing the Loch Ness Monster no longer opens anything (`record_monster_slain`). The relic is on a plinth block (`StonePlinth`, `HornPlinth`,
  `CrownPlinth`) taken by hand.
- **Horizen tower:** hold a peep stone by day in the Horizen (`Game::try_show_horizen_tower`): the tower appears beyond the generated land on dry ground
  (`horizen::site_is_dry`) and the peep stone's trail leads to it (`trail_path`).
- **Skylands tower:** `Game::update_sky_tower` notes each island walked on (`Landmarks::sky_visited`); at `SKY_TOWER_VISITS` the nearest island not yet made
  (`skylands::pick_tower_cell`) gets the tower, once, and it is saved (`skytower`, `skyvisit`; Horizen: `tower`; all: `unlocked`).
- **Compass** in the Horizen and Skylands points to where the player arrived (`CompassTarget::Spawn`).
- **Mines** (`mines::furnish`): pink salt crystals (`Block::PinkSalt`, `MINE_SALT_PER_MILLE`) on tunnel walls, and a silver key on a `KeyStand` in the middle ring's chamber of
  shafts `MINE_KEY_SHAFTS` (three keys).
- **Jungle** (`terrain::jungle_centre`, `in_jungle`, `place_jungle_tree`): a pure function of the seed, `JUNGLE_DISTANCE_CHUNKS` from spawn, trunks 22 to 32 tall, a bare
  middle. A fall out of the Skylands lands there (`Game::jungle_landing`), `SKY_FALL_DROP` above the ground.
- **Deer** leave `DEER_BONE_DROP` Deer Bones `DEER_BONE_CHANCE` percent of the time; 3 bones craft Musical Pipes. Pipes, pink salt and the three relics do nothing yet.

## Update: the cave Horizen, crows, the Iron Quail, the tin pan and the king's relics

- **The Horizen is a cave** (`horizen.rs`). Solid stone to `HORIZEN_ROOF_Y` with a flat dirt floor at `GROUND_TOP`; `air_height(seed, x, z)` is the
  free height above the floor (0 = rock, otherwise at least `HORIZEN_MIN_AIR`, with a rough ceiling). Three parts, one connected way (tests walk it for 12
  seeds): the arrival chamber (dome, `HORIZEN_CHAMBER_*`, with the arrival pool), a winding tunnel (`HORIZEN_TUNNEL_*`, direction from the seed) and the one
  great cavern at `HORIZEN_CAVERN_DISTANCE` (`HORIZEN_CAVERN_*`, ragged outline, tall enough for the tower). Ponds lie only on open floor and not near the tower.
  The silver tower is placed at `cavern_centre` by a peep stone (`Game::try_show_horizen_tower`); with no sky to read, the peep stone works at any hour and its
  trail follows the tunnel (`horizen::route`). A Horizen saved as flat land that leaves the player in rock puts them back on the arrival pool (`Game::travel_to`).
- **Fire tornadoes belong to the Skylands** (`SKY_TORNADO_*`, `Game::update_sky_tornado`): by day every `SKY_TORNADO_INTERVAL` seconds, `SKY_TORNADO_SPAWN_DISTANCE`
  blocks from the player at the player's height. None in the Horizen or the overworld.
- **Crow** (`AnimalKind::Crow`): quail behaviour (same flight constants, feather on take-off), but a take-off heads due west when `Animal::can_fly_west`
  (nothing solid for `CROW_WEST_LOOKAHEAD` blocks at flying height). Drops `CROW_FEATHER_DROP` feathers, no poultry. `Animal::west_followed` counts blocks flown west
  while the player is within `CROW_FOLLOW_DISTANCE`; beyond `CROW_LOSE_DISTANCE` it resets. At `CROW_ALTAR_WEST_BLOCKS` (two chunks) the crow, as it comes down,
  lands on a new **ancient altar** (`Game::land_crows_at_altars`): `Block::AltarLoadstone` placed on bare ground near it (saved as a placed block). Taking the loadstone by hand
  (`Game::mine`) leaves `Block::AncientAltar`.
- **Iron Quail** (`AnimalKind::IronQuail`, a boss with a health bar, `is_leashed` false): every `QUAIL_KILLS_PER_IRON_QUAIL` quail killed (`Game::quail_kills`, saved as
  `quailkills`) one comes (`Game::summon_iron_quails`, `iron_quails_owed`). She charges at all times, never despawns and is removed when the player dies; one left behind in
  another dimension returns on re-entering the overworld. Drops a Silver Key.
- **Tin Pan** (2 Tin): breaking `WetClay` with it held adds a Gold with `TIN_PAN_GOLD_CHANCE` percent, `TIN_PAN_RABBIT_FOOT_GOLD_CHANCE` with a rabbit foot in the hotbar.
- **Philosopher's Stone**: right click a `Boulder` (`Game::turn_boulder_to_gold`) to make a `GoldBoulder` (saved as a placed block; the boulder's removal is recorded so
  it never returns). It breaks by hand into `GOLD_BOULDER_GOLD_DROP` gold. The stone is kept.
- **King's Crown** (the Ancient Crown, shown by that name; save id still `ancient_crown`) is wearable. Worn in the overworld it brings out the one **ruined castle**
  (`Landmarks::ruined_castle`, saved as `ruinedcastle`, `Game::try_show_ruined_castle`) straight ahead of the way the player faces. It reuses the grand castle's
  footprint and ground rules (`Landmarks::castles`); its shape is `landmarks::ruins_cell`: broken wall with a gap in the south, tumbled corner towers, keep stumps, rubble,
  and the `SwordInStone` / `SwordHilt` pair on a dais at the centre. Right click either while wearing the crown (`Game::try_pull_sword`): the stone becomes `PulledStone`
  (placed, saved), the hilt is removed, and the player gets **Excalibur** (`EXCALIBUR_DAMAGE`, the highest of any weapon; `ItemKind::melee_reach`).
- Save format additions: `quailkills <n>`, `ruinedcastle <x> <z>`; items `tin_pan`, `excalibur`; placed blocks `gold_boulder`, `ancient_altar`, `altar_loadstone`, `pulled_stone`.

## Update: lots of caves, camps, day in the cave, the well, the Yeti, the recipe book and the jungle temple

- **Cave network** (`horizen.rs`): besides the long tunnel to the great cavern, every square of `HORIZEN_CAVE_CELL` blocks has a cave (`cave_in_cell`: a dome of
  `HORIZEN_CAVE_*` size, in the square of the arrival point the arrival chamber, in the square of the great cavern that cavern), each joined to its east and south
  neighbours by a tunnel (`HORIZEN_LINK_*`, `network_air`). Tests check that every cave within reach is open and connected to the arrival chamber for several seeds. A
  peep stone still brings out the tower in the great cavern and the trail along the long tunnel (`route`).
- **Camps**: `Game::update_cave_visits` notes each cave the player comes within `HORIZEN_CAVE_ENTER_SHARE` of its middle (`Landmarks::caves_visited`, saved as
  `cavevisit`, in the Horizen's records). Every `HORIZEN_CAMP_EVERY`th cave (the arrival chamber is the first; the great cavern counts but gets no camp) gets a camp
  (`Game::build_camp`): a placed log lit for `HORIZEN_CAMP_FIRE_SECONDS` with a tin pot of water holding berries, already stewed (`Pot { left: 0 }`), red or purple.
- **Day in the cave**: `Game::enter_horizen` always sets day. Night in the Horizen (`Game::apply_night`, only the Purple Berry Stew brings it) makes the player black out:
  `Game::faint_in_cave` takes them to Stonehenge in the overworld (the cave itself stays day).
- **The well** (`Well::block`, `WELL_HEIGHT`): stone rim one block high (still enterable), a post at each corner of the rim, a plank roof with a ridge and a rope over the water.
  The frame is measured from the ground round the well, not from the sunk water columns.
- **Yeti**: leaving the Skylands (`travel_to` from Skylands to the overworld: the cyclone, a fall or death) clears `Landmarks::yeti_slain`, so it spawns again and the summit
  cyclone is gone until it is killed again. Each lake with a castle already has its own Loch Ness Monster (`monsters_slain` is per lake and saved); a dead one never returns.
  The monsters are coloured by lake (`animal_draw::loch_ness_parts`).
- **Recipe Book**: a button on the inventory screen (`InventoryUi::guide`) switches the list to every recipe in `RECIPES` with the ingredients and what is still missing
  (`Recipe::missing`, `describe_missing`); craftable ones are marked and can be clicked.
- **Jungle temple** (`temple.rs`, `temple_cell`): a stepped pyramid of `Block::MayaStone` (`TEMPLE_*`), a stair up the south face, a shrine with a roof comb and a
  `GoldPile` on top, at `terrain::jungle_centre`, built by `terrain::generate_chunk_for` with stone under it down to the ground. The jungle's bare middle was widened
  to `JUNGLE_CLEARING_RADIUS` 34 to hold it, and a fall from the Skylands lands beside it (`Game::jungle_landing`).
- **Musical Pipes** are now 3 Deer Bones and nothing else.

## Update: reusable key, a flying Mother, the forge, the backpack, ghosts, Stonehenge and the pipes

- **Silver key** is kept: `Game::try_unlock` opens the door for good and does not take the key.
- **Iron Quail flies** (`Animal::update`): while charging she holds a height just above the player's head (`threat.y + 2`; level with them once within three
  blocks, higher while `stuck` against something), with no gravity, and is drawn with wings beating whenever she is off the ground (`Animal::is_airborne`).
- **Clay forge model** (`block.rs`, `forge_parts!`): footing, rounded body, shoulder, chimney with a rim, a stoking mouth under an arch with a bellows nozzle, and buttresses. The coal
  is `parts()[5]` at the chimney's mouth, and `effects::draw_flames` puts the glow and flames there.
- **Leather Backpack** (4 Leather + 2 Leather Cord; `ItemKind::is_backpack`): worn in the one worn place `Game::worn`, shared with the cloak, armour and crown (see "Update: bats, the Wild Hunt"; saves of that time wrote it as `back`).
  While worn the inventory has `BACKPACK_EXTRA_ROWS` more rows: `Inventory::slots` grows to `INVENTORY_SLOTS + BACKPACK_SLOTS` (`add_pack_rows`) and shrinks again
  (`remove_pack_rows`, which moves what is in the rows into free room in the other slots, or refuses). The inventory screen lays out for however many slots there are
  (`hud::layout(slots)`), (the Pack slot is gone: see the last update). Saves: `inv` slots 36 to 53 belong to the pack; without a `back` line they are moved to ordinary slots on load.
- **Pipes** (`Game::play_pipes`, right click with Musical Pipes held; `play::handle_placing`): by day (overworld) every quail and crow within `PIPES_CALL_RANGE` is called
  (`Animal::call_to`): it flies to a spot `PIPES_LAND_MIN`..`PIPES_LAND_MAX` from the player, lands there and stands still for `PIPES_CALM_SECONDS` (`Animal::calm`). A called or calm bird is
  not scared by the player, only by a blow.
- **Ghosts** (`ghosts.rs`; they are only words on the screen, `Game::ghost_text`, drawn by `play::draw_ghost_text`). The graveyard's ghost (`Game::update_graveyard_ghost`) asks for a song when
  the player walks in; playing the pipes inside the graveyard gives a silver key, a thanks, and sets `Landmarks::graveyard_rested` (saved as `ghostrest`): it never speaks again. In each of the three
  silver towers (`Game::tower_ghost_here`: the Skylands', the Horizen's and a lake island's) playing the pipes brings a hint: the Skylands tower tells the player to wade through the river to the island and
  not leave the water, the cave tower that things work differently at night, the lake tower to blow the horn in the stonehenge.
- **Stonehenge** (`stonehenge.rs`): `STONEHENGE_STONES` stones in a ring of radius `STONEHENGE_RADIUS`, lintels over pairs, and an altar stone, `STONEHENGE_DISTANCE_CHUNKS` (5) chunks from spawn
  in a direction the seed decides; built by `terrain::generate_chunk_for` (no trees within `STONEHENGE_TREE_MARGIN`). A player who blacks out in the cave wakes in its ring
  (`stonehenge::arrival`), instead of by the well. The Horn of the Hunt is blown there (see "Update: bats, the Wild Hunt").

## Update: bats, the Wild Hunt, the ruined tower, the shared worn slot, the totem and more tornadoes

- **Worn slot.** The backpack no longer has a place of its own: `Game::worn` holds one of `PeltCloak`, `RoyalArmor`, `AncientCrown` or `LeatherBackpack` (`ItemKind::is_wearable` includes the pack). Putting one on and taking one off go through `Inventory::put_on` / `Inventory::take_off`, which are all-or-nothing: the old garment returns to the inventory, a pack brings its `BACKPACK_EXTRA_ROWS` rows with it and takes them away again, and a swap or removal is refused (with a toast) if what lies in the pack's rows does not fit in the other slots. The inventory screen has one Worn slot (`hud::inventory_screen` no longer takes `back`). Saves: only `worn <item>` is written; slots 36 to 53 of `inv` belong to the pack while the worn item is a backpack. A `back <item>` line from an older save is still read (`game::one_worn_place`): the pack is worn and any other garment the save listed goes back into the inventory.
- **Bone totem in the Skylands.** `Game::trail_path` leads a held totem, by day or night, to the Skylands tower (`Landmarks::sky_tower`, by way of `Tower::approach`) once it has appeared. `effects::trail_floor` keeps the wisps over each island's top, or at the player's feet height over the void.
- **Ruined lake tower** (`tower::ruined_tower_cell`, `Lake::tower_cell(.., ruined)`, `Landmarks::tower_ruined`). While a lake's Loch Ness Monster lives (`monster_alive`), its island's tower is a ruin: wall stumps of `1..=RUINED_WALL_MAX` blocks (corners `RUINED_CORNER_HEIGHT`), a doorway gap and no door, no floors, roof or plinth, rubble, and a stair broken after `RUINED_STAIR_STEPS` steps. A test searches every place a player can reach by hopping one block at a time and finds nothing above the stumps. `World::record_monster_slain` now regenerates the tower's chunks: it is whole again, with its door locked until a silver key is used on it (`Game::try_unlock`, unchanged).
- **Bats** (`AnimalKind::Bat`): fly (no gravity; they hover about the player's body, weaving), bite for `BAT_DAMAGE` every `BAT_ATTACK_COOLDOWN`, `BAT_HEALTH` 2, no drops. They are never normal-world wildlife (`Game::spawn_wildlife` gives them no room there). `Game::play_pipes` calls `Game::call_bats` at night in the overworld: `BATS_PER_PIPES` appear `BAT_PIPES_MIN_DISTANCE..MAX` away in the air, `Animal::hunting` set, never more than `MAX_BAT_OVERWORLD` in all; `update_animals` removes overworld bats once it is day. In the Horizen `update_animals` now runs (it used to return at once) and `Game::spawn_cave_bats` is the only spawn, in open cave with `BAT_CAVE_MIN_AIR` of air, up to `MAX_BAT_CAVE`; a cave bat attacks within `BAT_NOTICE_DISTANCE`. Drawn by `animal_draw::bat_parts`.
- **The Wild Hunt** (final boss). The Horn of the Hunt (the Skylands tower's relic) is blown with right click (`Game::blow_horn`, called from `play::handle_placing`). Outside Stonehenge (`Game::in_stonehenge`: within `STONEHENGE_REACH` of the middle, overworld) only the wind answers; inside, three separate bosses (`AnimalKind::HuntSwordsman`, `HuntArcher`, `HuntStalker`, `is_wild_hunt`) appear `HUNT_SPAWN_MIN_DISTANCE..MAX` from the player, one health bar between them (`Game::boss_bar`). Blowing again while any lives calls no more. They never despawn (`AnimalKind::never_gives_up`, shared with the Iron Quail) and are removed, with their arrows, when the player dies or changes dimension.
  - Swordsman: charges at `HUNT_SWORDSMAN_SPEED` (below `SPRINT_SPEED`), strikes through `Animal::try_attack`.
  - Archer: keeps `HUNT_ARCHER_MIN_RANGE..MAX_RANGE` from the player and side-steps. `Animal::ready_to_shoot` says when an arrow is nocked; `Game::update_animals` aims it (leading the player and allowing for gravity, `HUNT_ARROW_SPREAD` of error), checks `clear_line`, pushes an `Arrow::from_archer` into `Game::hostile_arrows` and calls `Animal::shot_fired(slowdown)`. A Rabbit Foot in the hotbar makes `slowdown` `HUNT_ARCHER_FOOT_FACTOR` (2): half as often. `Game::update_hostile_arrows` flies them; `Arrow::hit_player` tests the segment each frame. They hurt the player only and are not dropped as items.
  - Stalker: `Animal::hunt_phase` (`HuntPhase::Circle`, `Dash`, `Retreat`). It works round the player at `HUNT_STALKER_CIRCLE_RADIUS` to the point straight behind the way they face (`Animal::player_facing`, set each frame from `Player::yaw`); within `HUNT_STALKER_BEHIND_ANGLE` of it, it dashes at `HUNT_STALKER_DASH_SPEED`. `try_attack` lets it stab only while dashing, and a stab (or `HUNT_STALKER_DASH_SECONDS` without one) starts a `HUNT_STALKER_RETREAT_SECONDS` retreat.
  - `Game::swing_melee` multiplies Excalibur's damage by `EXCALIBUR_HUNT_FACTOR` (3) against the three.
  - Killing the last of the three calls `Game::win_the_game`: `Landmarks::game_won` (saved as `won`), a toast and `Game::victory`, a message drawn by `play::draw_victory` for `VICTORY_TEXT_SECONDS`. Play goes on, and the horn can call the Hunt again.
  - The warriors are translucent (`animal_draw::hunt_parts`, drawn by `draw_ghosts` with the whisps).
- **Graveyard ghost** (`ghosts::GRAVEYARD_THANKS`) now also says that in three lonely towers there are others who may like to hear the music.
- **Fire tornadoes** (Skylands): `Game::tornadoes` is a list. One spawns every `SKY_TORNADO_INTERVAL` (15 s, half what it was) and each lasts `SKY_TORNADO_LIFETIME` (30 s, as long as before: `FireTornado::age`), so there are sometimes two. Night clears them all.
- Save format: `won` (the Wild Hunt is beaten). `back` is read but no longer written.

## Update: the boss update, ravens, sky islands, the Phoenix

- **Boss movement** (`Animal::steer`, `way_is_free`, `can_hop`, `check_progress`): every boss that walks feels `BOSS_FEEL_DISTANCE` ahead for something in its way, picks the nearest free turn
  on the side it last went round by (so it does not dither along a wall) and keeps it for `BOSS_DETOUR_SECONDS`. Bosses jump with `BOSS_JUMP_SPEED` and climb ledges of two blocks, but jump only at what they can get over. One
  that has made less than `BOSS_UNSTICK_PROGRESS` of headway in `BOSS_UNSTICK_SECONDS` takes a forced stride the other way.
- **Yeti**: close up (`YETI_SMASH_TRIGGER_RANGE`) it rears up for `YETI_SMASH_WINDUP` and smashes: `BossEvent::Smash` -> `Game::carry_out` hurts everything within `YETI_SMASH_RADIUS` of its body (4 damage) and throws it
  `YETI_SMASH_KNOCKBACK` blocks (`Player::launch`, `Animal::launch`, which decay at `LAUNCH_DECAY` so the throw carries a set distance), other yetis excepted. More than `YETI_ICE_MIN_RANGE` away it throws ice
  (`BossEvent::IceThrow`, `Projectile` + `Burst`, `ICE_BURST_RADIUS`, `ICE_BURST_DAMAGE`). Its fists still hit.
- **Loch Ness Monster**: keeps `LOCH_NESS_SHORE_MARGIN` blocks from the shore (`shore_clearance`; one nearer the shore may still move out). Within `LOCH_NESS_RAVEN_DISTANCE`, while the player is more than `LOCH_NESS_RAVEN_RANGE`
  from her, she calls ravens (`BossEvent::CallRavens`, `Animal::summoned`, they act as if it were night and stay by day) and every `LOCH_NESS_ACID_COOLDOWN` taints a strip of water towards the player (`BossEvent::Acid`,
  `Game::acid`, drawn brown by `effects::draw_acid`): for `ACID_SECONDS` wading in it costs `ACID_DAMAGE` a second (`Game::standing_in_acid`). A bow or sling shot (`Game::alarm_nessie`) sends her to open water for
  `LOCH_NESS_FLEE_SECONDS` and calls `LOCH_NESS_ALARM_RAVENS` more ravens.
- **Ravens** (`AnimalKind::Raven`, `AirPhase`): at night up to `MAX_RAVEN` circle the player at `RAVEN_ORBIT_RADIUS`, swoop for `RAVEN_DAMAGE`, fly off for `RAVEN_FLEE_SECONDS`, and go at dawn.
- **Iron Quail** (was the Mother of Quail): `IRON_QUAILS_PER_SUMMON` (3) come for every `QUAIL_KILLS_PER_IRON_QUAIL` quail. Each alternates closing in to strike with keeping `IRON_QUAIL_KITE_MIN..MAX` off and shooting iron feathers
  (`Arrow::from_iron_quail`, `Game::hostile_arrows`). A missed **Iron Feather** lies on the ground; held, a click throws it (`Game::throw_feather`, `IRON_FEATHER_THROW_DAMAGE`): used up only on hitting an animal.
- **Silver key**: `Game::key_or_gold` turns any further silver key drop (mine stand, graveyard ghost, Iron Quail) into a Gold if the player has a key or one lies on the ground (`Game::has_silver_key`).
- **Wild Hunt**: swordsman, two archers, two **lancers** (`HuntLancer`: `HuntPhase::Dash`/`Retreat`/`Rest`, replacing the stalker), `HUNT_GHOST_BEARS` ghostly bears and `HUNT_GHOST_CROWS` ghostly crows. Bears and crows
  with `Animal::splits` are replaced by `HUNT_SPLIT_INTO` of their kind (which do not split) when killed (`Game::remove_dead`). `Game::hunt_total` is the shared bar's whole.
- **Fire tornadoes**: `SKY_TORNADO_INTERVAL` is a quarter of the lifetime (up to four at once), `SKY_TORNADO_DAMAGE` 4. `Landmarks::phoenix_slain` (saved as `phoenixslain` in the Skylands' records) stops them for good.
- **Cyclone**: `CYCLONE_SECONDS` 2.5. The Skylands have no cyclone; the way down is to fall.
- **Salt and bats in the mines**: `Block::PinkSalt` is a crystal cluster on tunnel floors, only within `MINE_SALT_MAX_SHARE` of the mountain's radius, `MINE_SALT_FLOOR_PER_MILLE` of floor cells. Up to `MAX_BAT_MINE` bats
  (`Animal::lives_in_mine`) come out in the tunnels (`Game::player_in_mine`, `spawn_mine_bats`) and stay by day.
- **Sky islands** (`skylands::IslandKind`): wooded, ruins (`decoration`), hill (`Island::rise_at`) or altar (`Block::AncientAltar` and `Block::Bones`); one in `SKY_LARGE_ONE_IN` is `SKY_LARGE_FACTOR` wider. Only wooded
  islands take the tower. The **giant island** (`giant_island`) replaces a block of nine cells a few cells from the first island; a held Peep Stone leads to the foot of its volcano (`volcano_foot`, `Game::trail_path`).
- **Phoenix** (`AnimalKind::Phoenix`, `skylands::volcano_crater`, `Block::Magma`): spawns in the crater when the player comes within `PHOENIX_SPAWN_DISTANCE` (`Game::spawn_bosses`), hovers and throws fireballs
  (`BossEvent::Fireball`) and swoops. It drops a **Phoenix Feather**; Pebble + Stick + Phoenix Feather make a **Phoenix Arrow**, which a bow shoots in preference to ordinary arrows, never used up and never landing.

## Update: metal quails, broken night bridges, the crater tornado, the king's castle, the cursed idol and the jungle beasts

(Where this contradicts an earlier section, this one holds.)

- **Iron Quail** (`animal_draw::iron_quail_parts`): drawn as steel: blue-grey plates, dark seams, polished edges and rivets, a brass beak, and a gleam that sweeps over it with time (the boxes are flat-coloured, so the gleam is what makes it read as metal).
- **Bones**: `ItemKind::DeerBone` is `ItemKind::Bone` (name "Bone", save id `bone`; `deer_bone` still loads). Deer and the Skylands' `Block::Bones` give the same item; 3 Bones make Musical Pipes.
- **Salt**: a pink salt crystal drops `PINK_SALT_DROP_MIN..=MAX` (8 to 16) lumps. Holding salt (`Game::held_item`) when an ordinary bat strikes kills the bat instead and uses up one lump; no damage (`Game::update_animals`). The Vampire Bat is not affected.
- **Bats**: the cave brings a bat in only on every `BAT_CAVE_SPAWN_EVERY`th (2nd) spawn tick (`Game::cave_spawn_tick`). Every `BATS_PER_VAMPIRE` (20) bats killed in the Horizen (`Game::bat_kills`, saved as `batkills`) a **Vampire Bat** boss (`AnimalKind::VampireBat`, one at a time, `vampire_bats_owed`) appears: it uses only the circle/swoop/flee cycle of `Animal::update_flier`.
- **Broken bridges** (`skylands::Bridge::deck_at_time`, `deck_at(.., night)`, `surface_top(.., night)`, `generate_chunk(.., night)`): by night a bridge keeps only a ragged stub of `SKY_BROKEN_STUB` at each end (none at all if the deck is shorter than `SKY_BROKEN_MIN_DECK`). `World::set_night` rebuilds every Skylands chunk, and a player on a deck when it breaks is set on the nearest footing (`skylands::nearest_footing`). A test checks no walkable path remains.
- **Volcano**: `Block::Magma` is gone (the crater is `Stone`). `Game::crater_tornado` (`FireTornado::crater`, scale `VOLCANO_TORNADO_SCALE`) whirls in the crater all day, outlives the Phoenix, and, like every tornado, is cleared at night.
- **Crows and the altar**: the crow is set on top of the altar's collision box (`Block::bounds`), not inside it (it used to be embedded and could never move).
- **King's cave and castle** (`horizen::king_cell`, `king_centre`, `king_air`, `kingcastle.rs`): a ragged giant cave `KING_CAVE_CELLS` cave-squares from the arrival point holds the largest castle (`KING_WALL_HALF` 44 against the grand castle's 26): wall, gate, four towers, a keep and a dais with a `Block::BootsPlinth`. A held bone totem in the Horizen leads there (`horizen::king_route`). Playing Musical Pipes inside the walls (`Game::in_king_castle`) makes the king speak (`ghosts::KING_GREETING`) and opens a pop-up (`Game::king_dialog`, `play::draw_king_dialog`; `Game::ui_open` is true while it or the inventory is up). `Game::give_gold_to_king` takes `GOLD_PER_SPIRIT` gold per **spirit archer** (`AnimalKind::SpiritArcher`, friendly, not huntable, never gives up, not saved). It follows the player (`Animal::follow_player`) and shoots `SPIRIT_SHOTS` arrows (`Arrow::from_spirit`, `Game::update_spirits`) at creatures the player has hit in the last `SPIRIT_MARK_SECONDS` (`Animal::marked`, set in `Game::hurt_animals`), then vanishes.
- **Boots of the Sky** (`ItemKind::BootsOfTheSky`, wearable): `Game::apply_worn_effects` sets `Player::speed_factor` 1.5 and `jump_factor` 4/3, which makes a jump twice as far (distance = speed x air time, and air time grows with take-off speed). A test measures it.
- **Cursed Idol** (`ItemKind::CursedIdol`, on `Block::IdolPlinth` in a chamber at the temple's heart, reached by a passage in the north face, `temple.rs`): anywhere in the hotbar it doubles the player's damage (`Game::hurt_animals`) and `Game::eat_held` refuses every heal.
- **Jungle**: `JUNGLE_RADIUS` 180 (was 90: four times the area), `JUNGLE_DISTANCE_CHUNKS` 20 (was 10, so it does not reach the spawn point). **Orangutan** (`AnimalKind::Orangutan`, boss, `temple::orangutan_post`, `Landmarks::orangutan_slain` saved as `orangutanslain`): walks and climbs, hits, hurls rocks (`ProjectileKind::Rock`, `BossEvent::RockThrow`) and smashes (`BossEvent::OrangutanSmash`, `Game::smash`, shared with the Yeti). **Jaguar** (`Pounce`: stalk, crouch, leap, retreat) and **flying snake** (helix, dive, flee) spawn only in the jungle (`Game::spawn_jungle_wildlife`).
- Save format additions: `batkills <n>`, `orangutanslain`; items `boots_of_the_sky`, `cursed_idol`, `bone` (replacing `deer_bone`).

## Update: ceilings, the leaping Orangutan, Nessie's turn, parrots and clear ground

(Where this contradicts an earlier section, this one holds.)

- **Vampire Bat** now comes for every `BATS_PER_VAMPIRE` (10) cave bats killed. **Ceilings** (`Animal::ceiling_limit`, `floor_hint`, `unwedge`): a flier never aims above the lowest solid block over it
  (or over its goal), keeping `FLIER_CEILING_CLEARANCE` of air; blocked sideways under a roof it slides round (`strafe`) instead of climbing (it used to raise its goal 8 blocks into the rock and stay pinned), and a body found inside rock is set down. Bats use the same clamp.
- **Orangutan.** It used to be set down 0.35 blocks inside the wall of the tier above (a 2-wide ledge, centre off the block grid), so it could not move on any axis: it is now set on the ledge's middle
  (`Game::spawn_bosses`), `ORANGUTAN_HALF_WIDTH` is 0.75, and every walking boss that overlaps blocks moves the shortest way clear (`Animal::free_itself`). Beyond its leash it walks back until within
  `ORANGUTAN_HOME_RETURN_SHARE` of it (`Animal::homing`). The smash is a **leap**: crouch (`BossAction::Smash`, `ORANGUTAN_LEAP_WINDUP`), then `BossAction::Leap`, a ballistic arc (`Animal::begin_leap`) whose top is
  `ORANGUTAN_LEAP_APEX` above the higher end, landing `ORANGUTAN_LEAP_LAND_GAP` from the player; landing (or `ORANGUTAN_LEAP_MAX_SECONDS`) raises `BossEvent::OrangutanSmash`. Range `ORANGUTAN_LEAP_MAX_RANGE`.
- **Idol passage**: the mouth of the passage in the temple's north face is `Block::CastleDoor` (unbreakable planking, `TEMPLE_BOARD_DEPTH` deep) until `Landmarks::orangutan_slain`; `World::open_temple` sets it and makes the temple's chunks again
  (`temple::temple_cell_boarded`, `terrain::generate_chunk_for`).
- **Nessie** keeps `LOCH_NESS_SHORE_MARGIN` (8) from the shore. Meeting the edge she turns back toward open water (`Animal::open_water_heading`, biased towards where she came up) for `LOCH_NESS_TURN_SECONDS`
  instead of stopping; the progress check ignores her. `LOCH_NESS_MAX_RAVENS` is 4 and the shot alarm obeys the same cap.
- **Structures never overlap**: `Landmarks::has_room` also checks `Landmarks::reserved` (Stonehenge, the jungle's bare middle and temple; set by `World`, not saved), and lakes and mountains from treks are
  checked (`has_room_for_lake`, `has_room_for_mountain`) and pushed `SITE_PUSH_BLOCKS` further out until clear. The well, graveyard and both castles use `game::clear_site` and no longer fall back to a site that
  has no room; with none clear the call returns false and tries other headings on a later frame.
- **Jungle**: no ordinary wildlife (`AnimalKind::is_ordinary_wildlife`) spawns in it or is brought in by `spawn_animal`, and no ravens; only jaguars, flying snakes and **parrots** (`AnimalKind::Parrot`, quail behaviour,
  colourful model, `MAX_PARROT`). Every `PARROTS_PER_BRONZE_PARROT` parrots killed (`Game::parrot_kills`, saved as `parrotkills`) `BRONZE_PARROTS_PER_SUMMON` **Bronze Parrots** (`AnimalKind::BronzeParrot`, `is_metal_bird`,
  same behaviour as the Iron Quail, bronze model, drop Bronze) come (`summon_bronze_parrots`, `bronze_parrots_owed`). Their feather missile is the Iron Feather.

## Update: long snakes, poison arrows, jerky, falling stars, the Phoenix's return and varied caves

- **Flying snakes** (`animal.rs`, `animal_draw.rs`): `SNAKE_SEGMENTS` (64) segments of `SNAKE_SEGMENT_SPACING`, about 17 blocks, and thicker (`SNAKE_HALF_WIDTH`).
  The head's path is kept in `Animal::trail` (`lay_trail`); `body_points` is the body, drawn by `snake_body_boxes` and hit-tested by `Animal::hitboxes`
  (arrows and melee use every box). Flight is slow (`SNAKE_SPIRAL_SPEED`, `SNAKE_DIVE_SPEED`): a wide circle (`SNAKE_SPIRAL_RADIUS`, `SNAKE_LOOP_RATE`) with a
  smaller one riding on it (`SNAKE_COIL_*`) makes the loops. A dead snake drops `SNAKE_FANG_DROP_MIN..=MAX` **Snake Fangs**.
- **Poison arrows**: Pebble + Stick + Feather + Snake Fang. `Game::shoot` fires poison first, then Phoenix, then ordinary arrows. A poison arrow is used up and
  vanishes where it lands (`Arrow::vanishes_on_landing`); a hit calls `Animal::poison`, and `Game::update_poison` takes `POISON_DAMAGE` every `POISON_INTERVAL`
  for `POISON_SECONDS`.
- **Jerky**: Raw meat + `JERKY_SALT_COST` Pink Salt (`recipe.rs`), one per meat (`ItemKind::cured`). `heal_amount` is the cooked meat's (`jerky_base`); stews still double.
- **Spirits** cross dimensions: `Game::travel_to` collects them (with `shots_left`) and `bring_spirits` sets them down round the player.
- **Entering the Horizen**: `Game::begin_horizen_fall` starts the fade (`horizen_pending`); `update_horizen_transition` moves the player at full black
  (`HORIZEN_FADE_SHARE`) by `enter_horizen`, which always puts them in the arrival pool.
- **Cave kinds** (`horizen::CaveKind`): ordinary caves are Plain, Mossy (`moss_pools`, `moss_at`: floor, walls and pool water are tinted by `mesh.rs`),
  River (`river_at`, a meandering band) or Crystal (`crystal_at`, `Block::RedCrystal`: solid, not breakable, not saved). Rivers and pools count as ponds (`is_pond`).
- **Silver key**: `mines::furnish` fills the column under the key stand with dirt, so it rests on the ground.
- **Phoenix**: `Landmarks::phoenix_dead_for` (saved as `phoenixdead`) counts up while it is dead, in whichever dimension the player is in; at
  `PHOENIX_RESPAWN_SECONDS` `phoenix_slain` clears (`Game::update_phoenix_respawn`) and it returns when the player nears the crater.
- **Falling stars** (`stars.rs`, `Game::update_falling_stars`, `effects::draw_falling_stars`): in the Skylands at night, one every `FALLING_STAR_INTERVAL`; `pick_landing`
  chooses an island within range in front of the player (or the one they are on).

## Update: poison batches, bowls, cold pots, stardust and one-of-a-kind finds

(Where this contradicts an earlier section, this one holds.)

- **Poison arrows** have no poison effect (`Animal::poison`, `update_poison` and the `POISON_*` timings are gone). They are ordinary arrows that do
  `POISON_ARROW_DAMAGE_FACTOR` (2) times the damage (`POISON_ARROW_DAMAGE`), are used up only by hitting an animal, and lie on the ground to be picked up when they
  miss (only the Phoenix Arrow `vanishes_on_landing`). Recipe: `POISON_ARROW_BATCH` (25) Arrows + 1 Snake Fang make 25 Poison Arrows; the old recipe is gone. A bow still shoots poison, then Phoenix, then ordinary arrows.
- **Spirit archers**: at most `MAX_SPIRIT_ARCHERS` (8) follow at once; the king refuses (`ghosts::KING_FULL`, nothing taken) until some are gone.
- **Inventory**: `INVENTORY_SLOTS` 45 (one more row) and the backpack adds `BACKPACK_EXTRA_ROWS` 3. Slot indices are unchanged, so old saves load.
- **Wooden Bowl** (`ItemKind::WoodenBowl`, `wooden_bowl`): 1 log makes `BOWLS_PER_LOG` (5). A finished pot holds `STEW_SERVINGS` (4) (`Pot::served`, `Pot::servings_left`, saved as a seventh
  number on the `pot` record; older saves read 0). Right click the pot with a bowl to fill it (`World::take_stew`); an empty hand no longer takes stew. The stew items are bowls of stew:
  drinking or eating one gives back an empty bowl (`Game::return_bowl`).
- **Pots outlive their fire**: when the fire under a pot burns out the pot stays on the charcoal or ash (nothing drops) with its contents; cooking only advances while a fire burns, and
  relighting the charcoal resumes it. A cold pot can be used like a hot one (`Game::cold_pot_at`, `can_use_pot`). `World::take_pot` returns `Some(raw meat)`; it refuses while the fire is
  lit and meat is in the pot, and always refuses finished stew (serve it first). Breaking the block under a pot releases the pot (and raw meat). `with_pots` keeps pots on fire, embers, charcoal or ash.
- **Pipes** by day call quail, parrots and crows alike (parrots were already in the list; now tested).
- **Falling stars** never land on the island the player stands on. On landing (`Game::leave_stardust`) a `Block::StardustPile` (white gold pile, `STARDUST_PILE_SECONDS` 30) is left;
  `Game::update_stardust` fades it, breaking it first drops `STARDUST_DROP` `ItemKind::Stardust`. Piles are not saved and are cleared on leaving the Skylands.
- **One of a kind** (`Game::owns`: inventory or on the ground): a second Philosopher's Stone drops as Gold; no Peep Stone comes from pebbles while one is owned; no Loadstone from stones
  while a Loadstone or Compass is owned; crows (the birds that build altars) build none then either.

## Update: the general store, newts, the Unicorn, birds, rivers and a green test suite

(Where this contradicts an earlier section, this one holds.)

- **Rivers** (`river.rs`) are no longer zero contours of a noise field (those close into circular rivers). The world is cut into parallel lanes
  `RIVER_LANE_SPACING` apart in a direction chosen by the seed (`lane_coordinate`), each bent sideways by the same smooth noise
  (`RIVER_BEND`, `RIVER_BIG_SCALE`, `RIVER_SMALL_SCALE`), so lanes wind but never close or touch. `RIVER_LANE_PERCENT` of lanes carry a river;
  a river dwindles to nothing over `RIVER_SPAWN_TAPER` blocks beyond `RIVER_SPAWN_CLEARANCE` (the spawn area is dry). `field`, `is_river` and
  `river_near` keep their meaning; a test flood-fills the rivers and fails on any ring or pond. Terrain is a pure function of the seed, so
  existing worlds get the new rivers wherever the player has not built.
- **Island hill**: `ISLAND_PLATEAU_RADIUS` is 12 (the tower needs about 6), so the lake island's slope can be climbed one block at a time.
- **Compass** = Loadstone + Iron (smelted), no longer Iron Ore.
- **Skylands' first island** is a safe haven: `Game::update_sky_tornado` sends no tornado after a player on it (`skylands::on_spawn_island`) and removes
  one that drifts over it. The **general store** (`shop.rs`, `Game::shop_post`, `open_shop`, `buy`, `near_shop`, `effects::draw_shopkeeper`,
  `play::draw_shop_dialog`, `skylands::shop_post`): a ghostly figure `SHOP_OFFSET` blocks from the arrival point. Right click him (within `SHOP_REACH`, looking at him)
  to open the pop-up (`Game::shop_open`, counted by `ui_open`). `shop::wares()` is the stock: `SHOP_ARROWS` arrows for `SHOP_ARROW_PRICE`, a bow for
  `SHOP_BOW_PRICE`, a sword for `SHOP_SWORD_PRICE`, each meat stew for `SHOP_STEW_PRICE` gold. `Game::buy` is all-or-nothing (gold, room, distance).
  Pipes played within `SHOP_NEAR_RADIUS` of him bring `ghosts::SHOP_THANKS`. He is not an animal and is not saved. To add a ware, add it to `wares()`.
- **Sword** (`ItemKind::Sword`, `SWORD_DAMAGE` = iron axe + 2): a melee item through `ItemKind::melee_damage`. **Eye of Newt** (`ItemKind::EyeOfNewt`) has no use yet.
- **Newts** (`AnimalKind::Newt`): harmless (`attack_damage` 0), `NEWT_HEALTH` 4, `NEWT_EYE_CHANCE` percent drop an Eye of Newt. `Game::spawn_newts` (called from the Horizen branch of
  `spawn_wildlife`) puts up to `MAX_NEWT` on `horizen::is_mossy_floor` (open floor of a `CaveKind::Mossy` cave). Model: `animal_draw::newt_parts`.
- **Unicorn** (`AnimalKind::Unicorn`, a boss that never gives up): `Game::use_idol` (right click with the Cursed Idol held, from `play::handle_placing`) the first time sets
  `Landmarks::unicorn_summoned` (saved as `unicornsummoned`); `Game::unicorn_due` / `summon_unicorn` bring it out `UNICORN_SPAWN_DISTANCE` away whenever it is due (idol used, not
  slain, none out, player in the overworld), so it returns after a dimension trip or a load. `Game::update_animals` sets `Animal::pacified` on it while `player_in_jungle`: a pacified
  unicorn wanders and never strikes (`try_attack`); otherwise it charges at `UNICORN_RUN_SPEED` for `UNICORN_DAMAGE`. Killing it sets `unicorn_slain` (`unicornslain`) and it drops `UNICORN_GOLD_DROP` gold.
- **Big birds** (`metal_bird_features`): the Iron Quail and Bronze Parrot have a neck ruff, a hooked beak, a fanned tail, folded wings of layered feathers (spread primaries in the air) and talons.
  The Vampire Bat is smaller: model scale 3.4 (was 5) and `VAMPIRE_BAT_HEIGHT` 0.5. Its collision width stays 0.7: a narrower box made it pin itself in low caves (`a_vampire_bat_in_a_low_cave_never_pins_itself_against_the_ceiling`).
- **Flying snake hit boxes** (`Animal::hitboxes`): a box `SNAKE_HIT_HALF_WIDTH` round the head and the beating wings, and a box of `SNAKE_BODY_HIT_HALF` at every second body point. Tests in
  `animal_draw` check that every drawn part and every point along the body is a target.
- **Tests**: the suite is green. Balance tests compute their expectations from `config.rs` (for example a bear dies in `ceil(BEAR_HEALTH / AXE_DAMAGE)` blows) instead of hard-coding old numbers: do the same.
- Save format additions: `unicornsummoned`, `unicornslain`; items `sword`, `eye_of_newt`.

## Update: the backpack swap fix, common newts, the idol wakes by itself and the Great Bear

(Where this contradicts an earlier section, this one holds.)

- **Inventory screen** (`hud::inventory_screen`): putting on or taking off a backpack changes `Inventory::slots.len()` in the middle of the frame, so the slot count, layout and Worn rect are measured again after the input is handled, and raw slot reads use `get`. (It used to index past the end and crash.)
- **Newts**: `horizen::is_mossy_floor` is all open floor inside a mossy cave's green walls (`mossy_wall`), not just its heart. `horizen::mossy_caves_near` lists mossy caves within `NEWT_CAVE_REACH`; `Game::spawn_newts` keeps each stocked with `NEWTS_PER_CAVE`, at most `NEWT_SPAWNS_PER_TICK` per tick, `MAX_NEWT` (24) overall, never within `NEWT_SPAWN_MIN_DISTANCE` of the player. The model has a bright back and orange stripe.
- **Cursed Idol** is no longer right-clicked (`use_idol` is gone). `Game::update_idol` (every frame, `play.rs`) calls the Unicorn the first time the idol is in the hotbar in the overworld outside the jungle. `summon_unicorn` also runs on the spawn tick.
- **Great Bear** (`AnimalKind::BearBoss`, boss, `never_gives_up`, not saved): killing a `Bear` while `worn == PeltCloak` in the overworld makes one due (`note_bear_killed`, `bear_bosses_owed`); `summon_bear_bosses` places it `BEAR_BOSS_SPAWN_DISTANCE` away (retried on the spawn tick; never two). It always charges, ignores the cloak, and is quicker below half health. A blow on it (`hurt_animals_from`) sets `Game::bear_rage` to `BEAR_BOSS_RAGE_SECONDS` and `Animal::enrage_bear`s every bear; while the rage lasts bears are re-enraged each frame, moved at any distance and not despawned, so bears that spawn then come too. Death of the player removes it; leaving the dimension makes it due again; its own death ends the rage.
