//! Block types, their shapes, and their break and collision rules.

use crate::config::*;
use crate::item::ItemKind;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Block {
    Air,
    /// The ground. It cannot be broken (or made by the player), so the land is as it was generated.
    Dirt,
    /// The unbreakable bottom layer of the world (not the same as a placed stone item).
    Stone,
    /// A tree trunk: a thin column, broken with a stone axe.
    Wood,
    Leaves,
    /// Dark evergreen foliage on pine trees. Breakable, like leaves.
    Needles,
    /// Plants that grow in clearings. All walk-through.
    Grass,
    Flower,
    Bush,
    /// Items the player has put on the ground. They are small and break back into the item.
    PlacedLog,
    PlacedPebble,
    PlacedStone,
    /// A burning placed log. Its remaining time lives in `World::fires`.
    Fire,
    /// What a fire leaves behind.
    Charcoal,
    /// A full cube of wooden planks, for walls and ceilings. Placed by the player.
    Planks,
    /// Charcoal that has been lit. Its remaining time lives in `World::fires`, like a burning log.
    Embers,
    /// What burnt-out embers leave behind.
    Ash,
    /// River water: still, walk-through and unbreakable. It only fills river beds, which no one can dig
    /// into (dirt cannot be broken), so it never needs to flow.
    Water,
    /// Things lying on a river bed under the water (the cell is still full of water). Breaking one by hand
    /// takes it and leaves plain water behind.
    WetPebble,
    WetStone,
    WetClay,
    /// A big rock lying on the ground, a little wider than its cell. Only a bronze mallet breaks it; it drops stone, a
    /// pebble and iron ore.
    Boulder,
    /// A small dark lodestone lying on the ground. It is generated, never placed; broken by hand it drops a loadstone.
    Loadstone,
    /// A forge of clay the player has put down, empty. Charcoal is put in it by right click.
    ClayForge,
    /// A clay forge with charcoal in it, ready to be lit with a bow drill.
    ForgeCharged,
    /// A burning clay forge. Its remaining time lives in `World::fires`; when it runs out the forge is empty again.
    ForgeLit,
}

/// One coloured box inside a block's cell, in cell-local coordinates (0..=1).
pub struct Part {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub color: (f32, f32, f32),
}

const fn part(min: [f32; 3], max: [f32; 3], color: (f32, f32, f32)) -> Part {
    Part { min, max, color }
}

const WOOD_PARTS: &[Part] =
    &[part([TRUNK_INSET, 0.0, TRUNK_INSET], [1.0 - TRUNK_INSET, 1.0, 1.0 - TRUNK_INSET], (0.36, 0.24, 0.12))];
const GRASS_PARTS: &[Part] = &[
    part([0.15, 0.0, 0.20], [0.27, 0.50, 0.32], (0.30, 0.62, 0.22)),
    part([0.45, 0.0, 0.55], [0.57, 0.65, 0.67], (0.36, 0.68, 0.25)),
    part([0.70, 0.0, 0.25], [0.82, 0.40, 0.37], (0.26, 0.56, 0.20)),
    part([0.28, 0.0, 0.70], [0.38, 0.35, 0.80], (0.33, 0.64, 0.24)),
];
/// Part 1 (the head) is recoloured per flower by the mesher.
const FLOWER_PARTS: &[Part] = &[
    part([0.46, 0.0, 0.46], [0.54, 0.45, 0.54], (0.25, 0.55, 0.20)),
    part([0.36, 0.45, 0.36], [0.64, 0.62, 0.64], (0.90, 0.25, 0.30)),
];
const BUSH_PARTS: &[Part] = &[
    part([0.12, 0.0, 0.12], [0.88, 0.60, 0.88], (0.14, 0.45, 0.16)),
    part([0.25, 0.60, 0.25], [0.75, 0.78, 0.75], (0.18, 0.52, 0.18)),
    // Berries studding each side.
    part([0.30, 0.30, 0.08], [0.40, 0.40, 0.14], (0.75, 0.12, 0.20)),
    part([0.62, 0.38, 0.86], [0.72, 0.48, 0.92], (0.75, 0.12, 0.20)),
    part([0.08, 0.35, 0.60], [0.14, 0.45, 0.70], (0.75, 0.12, 0.20)),
    part([0.86, 0.25, 0.30], [0.92, 0.35, 0.40], (0.75, 0.12, 0.20)),
];
/// A log lying along the x axis. The mesher paints the two x ends as cut wood.
const LOG_PARTS: &[Part] = &[part([0.05, 0.0, 0.25], [0.95, 0.5, 0.75], (0.36, 0.24, 0.12))];
const FIRE_PARTS: &[Part] = &[part([0.05, 0.0, 0.25], [0.95, 0.5, 0.75], (0.17, 0.11, 0.08))];
/// Lit charcoal: the charcoal shape, red-hot. Flames and glow are drawn on top by `effects.rs`.
const EMBER_PARTS: &[Part] = &[
    part([0.15, 0.0, 0.20], [0.85, 0.25, 0.75], (0.40, 0.13, 0.06)),
    part([0.30, 0.25, 0.30], [0.65, 0.38, 0.62], (0.62, 0.24, 0.07)),
];
const ASH_PARTS: &[Part] = &[
    part([0.12, 0.0, 0.18], [0.88, 0.10, 0.80], (0.66, 0.64, 0.62)),
    part([0.28, 0.10, 0.28], [0.68, 0.16, 0.64], (0.74, 0.72, 0.70)),
];
const PEBBLE_PARTS: &[Part] = &[
    part([0.30, 0.0, 0.30], [0.48, 0.12, 0.46], (0.64, 0.64, 0.66)),
    part([0.52, 0.0, 0.42], [0.68, 0.10, 0.58], (0.58, 0.58, 0.60)),
    part([0.38, 0.0, 0.54], [0.52, 0.08, 0.66], (0.68, 0.68, 0.70)),
];
const STONE_PARTS: &[Part] = &[
    part([0.25, 0.0, 0.25], [0.75, 0.30, 0.72], (0.42, 0.42, 0.46)),
    part([0.32, 0.30, 0.32], [0.62, 0.38, 0.60], (0.50, 0.50, 0.54)),
];
/// A lump of clay lying flat.
const CLAY_PARTS: &[Part] = &[
    part([0.18, 0.0, 0.22], [0.82, 0.12, 0.78], (0.66, 0.52, 0.43)),
    part([0.32, 0.12, 0.34], [0.68, 0.19, 0.66], (0.72, 0.58, 0.48)),
];
/// A big plain rock. It bulges `BOULDER_OVERHANG` past its cell on every side and is built from stacked, narrowing
/// layers whose corners are cut (a wide box along x, a wide box along z and a square one between them), so it
/// reads as roundish rather than as a cube. Neighbouring boxes differ a little in height so no two faces share a
/// plane. The collider is the one box around all of it (`Block::bounds`).
const BOULDER_PARTS: &[Part] = &[
    // Lowest layer: widest.
    part([-0.15, 0.0, 0.12], [1.15, 0.34, 0.88], (0.39, 0.39, 0.42)),
    part([0.12, 0.0, -0.15], [0.88, 0.35, 1.15], (0.40, 0.40, 0.43)),
    part([0.0, 0.0, 0.0], [1.0, 0.33, 1.0], (0.38, 0.38, 0.41)),
    // Middle layer: narrower, cut at the corners.
    part([-0.03, 0.34, 0.16], [1.03, 0.68, 0.84], (0.44, 0.44, 0.47)),
    part([0.16, 0.35, -0.03], [0.84, 0.69, 1.03], (0.45, 0.45, 0.48)),
    part([0.08, 0.33, 0.08], [0.92, 0.67, 0.92], (0.43, 0.43, 0.46)),
    // Crown, a little off centre.
    part([0.16, 0.68, 0.22], [0.82, 0.92, 0.84], (0.50, 0.50, 0.53)),
    part([0.28, 0.92, 0.30], [0.66, 1.0, 0.70], (0.54, 0.54, 0.57)),
    // Lumps on the flanks, so the outline is not symmetrical.
    part([0.88, 0.0, 0.58], [1.12, 0.26, 0.92], (0.36, 0.36, 0.39)),
    part([-0.10, 0.0, 0.08], [0.14, 0.22, 0.36], (0.37, 0.37, 0.40)),
];
/// A small dark lodestone with a faint metallic glint on top.
const LOADSTONE_PARTS: &[Part] = &[
    part([0.27, 0.0, 0.27], [0.73, 0.20, 0.72], (0.20, 0.21, 0.26)),
    part([0.34, 0.20, 0.34], [0.64, 0.30, 0.62], (0.27, 0.28, 0.34)),
    part([0.44, 0.30, 0.43], [0.55, 0.33, 0.53], (0.46, 0.48, 0.58)),
];
/// A forge of clay: a squat block with a dark opening on top. The coal in it is a third part, black when
/// filled and red-hot when lit (`effects.rs` draws the flames and the glow on top).
macro_rules! forge_parts {
    ($($coal:expr)?) => {
        &[
            part([0.08, 0.0, 0.08], [0.92, 0.50, 0.92], (0.68, 0.54, 0.45)),
            part([0.20, 0.50, 0.20], [0.80, 0.56, 0.80], (0.40, 0.31, 0.26)),
            $(part([0.28, 0.56, 0.28], [0.72, 0.64, 0.72], $coal),)?
        ]
    };
}
const FORGE_PARTS: &[Part] = forge_parts!();
const FORGE_CHARGED_PARTS: &[Part] = forge_parts!((0.12, 0.12, 0.13));
const FORGE_LIT_PARTS: &[Part] = forge_parts!((0.62, 0.24, 0.07));
const CHARCOAL_PARTS: &[Part] = &[
    part([0.15, 0.0, 0.20], [0.85, 0.25, 0.75], (0.12, 0.12, 0.13)),
    part([0.30, 0.25, 0.30], [0.65, 0.38, 0.62], (0.17, 0.17, 0.18)),
];

impl Block {
    /// Blocks a player can create, which are saved in `World::placed`.
    pub const PLAYER_MADE: [Block; 11] = [
        Block::PlacedLog,
        Block::PlacedPebble,
        Block::PlacedStone,
        Block::Fire,
        Block::Charcoal,
        Block::Planks,
        Block::Embers,
        Block::Ash,
        Block::ClayForge,
        Block::ForgeCharged,
        Block::ForgeLit,
    ];

    /// Whether a player can break this block with their bare hands. Dirt is not among them.
    pub fn hand_breakable(self) -> bool {
        matches!(
            self,
            Block::Leaves
                | Block::Needles
                | Block::Grass
                | Block::Flower
                | Block::Bush
                | Block::PlacedLog
                | Block::PlacedPebble
                | Block::PlacedStone
                | Block::Loadstone
                | Block::Charcoal
                | Block::Planks
                | Block::Ash
                | Block::WetPebble
                | Block::WetStone
                | Block::WetClay
                | Block::ClayForge
                | Block::ForgeCharged
        )
    }

    /// Whether this is plain water: it does not collide and cannot be aimed at (a ray looks through it).
    pub fn is_liquid(self) -> bool {
        self == Block::Water
    }

    /// Whether the cell is full of water: plain water, or water over something lying on the river bed. The
    /// mesher draws the translucent surface of every such cell.
    pub fn is_wet(self) -> bool {
        matches!(self, Block::Water | Block::WetPebble | Block::WetStone | Block::WetClay)
    }

    /// What is left in the cell when this block is taken away: water for something lying under water, else air.
    pub fn remainder(self) -> Block {
        if self.is_wet() { Block::Water } else { Block::Air }
    }

    /// Whether this is a burning block (a lit log, lit charcoal or a lit forge), which keeps a timer in `World::fires`.
    pub fn is_burning(self) -> bool {
        matches!(self, Block::Fire | Block::Embers | Block::ForgeLit)
    }

    /// Seconds of holding the break button needed to break this block. Zero for blocks
    /// that cannot be broken.
    pub fn break_seconds(self) -> f32 {
        match self {
            Block::Wood => BREAK_SECONDS_WOOD,
            Block::Bush => BREAK_SECONDS_BUSH,
            Block::Leaves | Block::Needles => BREAK_SECONDS_FOLIAGE,
            Block::Grass | Block::Flower => BREAK_SECONDS_PLANT,
            Block::PlacedLog
            | Block::PlacedPebble
            | Block::PlacedStone
            | Block::Loadstone
            | Block::Charcoal
            | Block::Ash
            | Block::WetPebble
            | Block::WetStone
            | Block::WetClay => BREAK_SECONDS_PLACED,
            Block::Planks => BREAK_SECONDS_PLANKS,
            Block::Boulder => BREAK_SECONDS_BOULDER,
            Block::ClayForge | Block::ForgeCharged => BREAK_SECONDS_FORGE,
            Block::Air | Block::Dirt | Block::Stone | Block::Water | Block::Fire | Block::Embers | Block::ForgeLit => 0.0,
        }
    }

    /// Whether this block can ever be broken (by hand or with the right tool).
    /// Dirt, bedrock stone, water, air and burning fire (or a lit forge) never can.
    pub fn breakable(self) -> bool {
        self.hand_breakable() || matches!(self, Block::Wood | Block::Boulder)
    }

    /// Whether a player holding `held` (or nothing) can break this block.
    /// Wood needs a stone axe and a boulder a bronze mallet. With a bow drill in hand a placed log, charcoal or
    /// a filled forge is lit instead of broken, and with a bow or a sling in hand nothing is broken: the button
    /// shoots instead.
    pub fn can_break_with(self, held: Option<ItemKind>) -> bool {
        if held.map_or(false, ItemKind::is_launcher) {
            return false;
        }
        match self {
            Block::Wood => held.map_or(false, ItemKind::is_axe),
            Block::Boulder => held == Some(ItemKind::BronzeMallet),
            Block::PlacedLog | Block::Charcoal | Block::ForgeCharged => held != Some(ItemKind::BowDrill),
            _ => self.hand_breakable(),
        }
    }

    /// Full, solid cubes that hide the faces of their neighbours. Everything else
    /// (trunks, plants, small items) leaves the neighbouring faces visible.
    pub fn opaque(self) -> bool {
        matches!(self, Block::Dirt | Block::Stone | Block::Leaves | Block::Needles | Block::Planks)
    }

    /// Whether the player and dropped items bump into this block. Plants, pebbles, stones
    /// and charcoal are small enough to walk through.
    pub fn collides(self) -> bool {
        matches!(
            self,
            Block::Dirt
                | Block::Stone
                | Block::Wood
                | Block::Leaves
                | Block::Needles
                | Block::PlacedLog
                | Block::Fire
                | Block::Planks
                | Block::Boulder
                | Block::ClayForge
                | Block::ForgeCharged
                | Block::ForgeLit
        )
    }

    /// Whether items can be placed on top of this block.
    pub fn supports_placement(self) -> bool {
        self.opaque()
    }

    /// Solid, built-or-natural cubes that a plank can be attached to from the side or below,
    /// which is what lets a ceiling reach out from the top of a wall. Foliage does not count.
    pub fn anchors_planks(self) -> bool {
        matches!(self, Block::Dirt | Block::Stone | Block::Planks)
    }

    /// The boxes this block is drawn from. Empty for air and for plain full cubes.
    pub fn parts(self) -> &'static [Part] {
        match self {
            Block::Wood => WOOD_PARTS,
            Block::Grass => GRASS_PARTS,
            Block::Flower => FLOWER_PARTS,
            Block::Bush => BUSH_PARTS,
            Block::PlacedLog => LOG_PARTS,
            Block::Fire => FIRE_PARTS,
            Block::PlacedPebble => PEBBLE_PARTS,
            Block::PlacedStone => STONE_PARTS,
            Block::Charcoal => CHARCOAL_PARTS,
            Block::Embers => EMBER_PARTS,
            Block::Ash => ASH_PARTS,
            Block::WetPebble => PEBBLE_PARTS,
            Block::WetStone => STONE_PARTS,
            Block::WetClay => CLAY_PARTS,
            Block::Boulder => BOULDER_PARTS,
            Block::Loadstone => LOADSTONE_PARTS,
            Block::ClayForge => FORGE_PARTS,
            Block::ForgeCharged => FORGE_CHARGED_PARTS,
            Block::ForgeLit => FORGE_LIT_PARTS,
            _ => &[],
        }
    }

    /// Plain full cubes (drawn with face culling and ambient occlusion).
    pub fn is_full_cube(self) -> bool {
        self != Block::Air && !self.is_liquid() && self.parts().is_empty()
    }

    /// The smallest box (cell-local) around the block's shape, used for collision and
    /// for aiming. `None` for air.
    pub fn bounds(self) -> Option<([f32; 3], [f32; 3])> {
        if self == Block::Air {
            return None;
        }
        if self.is_liquid() {
            return Some(([0.0; 3], [1.0, 1.0 - WATER_SURFACE_DROP, 1.0]));
        }
        let parts = self.parts();
        if parts.is_empty() {
            return Some(([0.0; 3], [1.0; 3]));
        }
        let (mut lo, mut hi) = ([1.0_f32; 3], [0.0_f32; 3]);
        for p in parts {
            for a in 0..3 {
                lo[a] = lo[a].min(p.min[a]);
                hi[a] = hi[a].max(p.max[a]);
            }
        }
        Some((lo, hi))
    }

    /// Stable identifier used in save files, for blocks the player can create.
    pub fn id(self) -> Option<&'static str> {
        match self {
            Block::PlacedLog => Some("placed_log"),
            Block::PlacedPebble => Some("placed_pebble"),
            Block::PlacedStone => Some("placed_stone"),
            Block::Fire => Some("fire"),
            Block::Charcoal => Some("charcoal"),
            Block::Planks => Some("planks"),
            Block::Embers => Some("embers"),
            Block::Ash => Some("ash"),
            Block::ClayForge => Some("clay_forge"),
            Block::ForgeCharged => Some("forge_charged"),
            Block::ForgeLit => Some("forge_lit"),
            _ => None,
        }
    }

    pub fn from_id(id: &str) -> Option<Block> {
        Block::PLAYER_MADE.into_iter().find(|b| b.id() == Some(id))
    }

    /// Colour of a plain full cube (part-based blocks use their parts' colours).
    pub fn base_color(self) -> (f32, f32, f32) {
        match self {
            Block::Air => (0.0, 0.0, 0.0),
            Block::Dirt => (0.50, 0.34, 0.20),
            Block::Stone => (0.50, 0.50, 0.53),
            Block::Leaves => (0.16, 0.52, 0.17),
            Block::Needles => (0.09, 0.36, 0.22),
            Block::Planks => (0.72, 0.54, 0.31),
            _ => self.parts().first().map_or((0.0, 0.0, 0.0), |p| p.color),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Block; 26] = [
        Block::Air,
        Block::Dirt,
        Block::Stone,
        Block::Wood,
        Block::Leaves,
        Block::Needles,
        Block::Grass,
        Block::Flower,
        Block::Bush,
        Block::PlacedLog,
        Block::PlacedPebble,
        Block::PlacedStone,
        Block::Fire,
        Block::Charcoal,
        Block::Planks,
        Block::Embers,
        Block::Ash,
        Block::Water,
        Block::WetPebble,
        Block::WetStone,
        Block::WetClay,
        Block::Boulder,
        Block::Loadstone,
        Block::ClayForge,
        Block::ForgeCharged,
        Block::ForgeLit,
    ];

    #[test]
    fn breakability() {
        for b in [Block::Leaves, Block::Needles, Block::Grass, Block::Flower, Block::Bush] {
            assert!(b.breakable() && b.hand_breakable(), "{b:?}");
        }
        for b in [Block::PlacedLog, Block::PlacedPebble, Block::PlacedStone, Block::Charcoal, Block::Planks, Block::Ash] {
            assert!(b.hand_breakable(), "{b:?} breaks back into an item by hand");
        }
        assert!(Block::Wood.breakable() && !Block::Wood.hand_breakable());
        assert!(!Block::Stone.breakable() && !Block::Air.breakable() && !Block::Fire.breakable());
        assert!(!Block::Dirt.breakable() && !Block::Dirt.hand_breakable(), "the ground cannot be dug");
        assert!(!Block::Water.breakable() && !Block::Water.hand_breakable());
        assert!(!Block::Embers.breakable(), "burning charcoal cannot be broken, like a burning log");
    }

    #[test]
    fn breaking_takes_time_for_every_breakable_block() {
        assert_eq!(Block::Dirt.break_seconds(), 0.0);
        assert_eq!(Block::Wood.break_seconds(), 5.0);
        assert_eq!(Block::Bush.break_seconds(), 3.0);
        assert_eq!(Block::Leaves.break_seconds(), 2.0);
        assert_eq!(Block::Needles.break_seconds(), 2.0);
        assert_eq!(Block::Grass.break_seconds(), 0.5);
        assert_eq!(Block::Flower.break_seconds(), 0.5);
        assert_eq!(Block::Planks.break_seconds(), 1.5);
        for b in ALL {
            assert_eq!(b.breakable() || b.hand_breakable(), b.break_seconds() > 0.0, "{b:?}");
        }
    }

    #[test]
    fn wood_needs_a_stone_axe() {
        assert!(!Block::Wood.can_break_with(None));
        assert!(!Block::Wood.can_break_with(Some(ItemKind::Stick)));
        assert!(Block::Wood.can_break_with(Some(ItemKind::StoneAxe)));
        // Hand-breakable blocks work with anything, including an axe.
        assert!(Block::Grass.can_break_with(None));
        assert!(Block::Leaves.can_break_with(Some(ItemKind::StoneAxe)));
        // Dirt and water cannot be broken at all, even with an axe.
        for held in [None, Some(ItemKind::StoneAxe), Some(ItemKind::Stone)] {
            assert!(!Block::Dirt.can_break_with(held) && !Block::Water.can_break_with(held), "{held:?}");
        }
        // Stone and fire are never breakable, even with an axe.
        assert!(!Block::Stone.can_break_with(Some(ItemKind::StoneAxe)));
        assert!(!Block::Fire.can_break_with(Some(ItemKind::StoneAxe)));
    }

    #[test]
    fn a_bow_drill_lights_placed_logs_instead_of_breaking_them() {
        assert!(Block::PlacedLog.can_break_with(None));
        assert!(Block::PlacedLog.can_break_with(Some(ItemKind::StoneAxe)));
        assert!(!Block::PlacedLog.can_break_with(Some(ItemKind::BowDrill)));
        assert!(Block::Charcoal.can_break_with(None));
        assert!(!Block::Charcoal.can_break_with(Some(ItemKind::BowDrill)), "lit, not broken");
        assert!(Block::Grass.can_break_with(Some(ItemKind::BowDrill)));
    }

    #[test]
    fn only_full_cubes_are_opaque_and_only_bulky_blocks_collide() {
        for b in ALL {
            if b.opaque() {
                assert!(b.is_full_cube(), "{b:?}");
            }
        }
        assert!(Block::Dirt.opaque() && Block::Leaves.opaque() && Block::Planks.opaque());
        assert!(!Block::Wood.opaque() && !Block::Grass.opaque() && !Block::PlacedStone.opaque());
        for b in [Block::Grass, Block::Flower, Block::Bush, Block::PlacedPebble, Block::PlacedStone, Block::Charcoal] {
            assert!(!b.collides(), "{b:?} should be walk-through");
        }
        for b in [Block::Dirt, Block::Wood, Block::PlacedLog, Block::Fire, Block::Planks] {
            assert!(b.collides(), "{b:?}");
        }
        assert!(!Block::Air.collides());
    }

    #[test]
    fn water_is_a_walk_through_slab_a_little_below_the_ground_beside_it() {
        assert!(Block::Water.is_liquid() && !Block::Dirt.is_liquid());
        assert!(!Block::Water.collides() && !Block::Water.opaque() && !Block::Water.is_full_cube());
        assert_eq!(Block::Water.id(), None, "water is generated, never placed by the player");
        let (lo, hi) = Block::Water.bounds().unwrap();
        assert_eq!((lo[0], lo[2], hi[0], hi[2]), (0.0, 0.0, 1.0, 1.0));
        assert!((hi[1] - (1.0 - WATER_SURFACE_DROP)).abs() < 1e-6 && hi[1] < 1.0);
        assert!(!Block::Water.supports_placement() && !Block::Water.anchors_planks());
    }

    #[test]
    fn things_on_the_river_bed_break_by_hand_and_are_small_walk_through_blocks() {
        for b in [Block::WetPebble, Block::WetStone, Block::WetClay] {
            assert!(b.hand_breakable() && b.breakable() && b.can_break_with(None), "{b:?}");
            assert_eq!(b.break_seconds(), BREAK_SECONDS_PLACED, "{b:?}");
            assert!(!b.collides() && !b.opaque() && !b.is_full_cube() && b.id().is_none(), "{b:?}");
            let (lo, hi) = b.bounds().unwrap();
            assert!(hi[1] <= 0.45 && lo[1] == 0.0, "{b:?} lies low: {lo:?} {hi:?}");
            assert!(b.can_break_with(Some(ItemKind::StoneAxe)) && !b.can_break_with(Some(ItemKind::Bow)));
            assert!(b.is_wet() && !b.is_liquid(), "{b:?} sits in water but can be aimed at");
            assert_eq!(b.remainder(), Block::Water, "taking it leaves the water");
        }
        for b in [Block::Boulder, Block::Dirt, Block::PlacedPebble, Block::Air, Block::ClayForge] {
            assert!(!b.is_wet(), "{b:?}");
            assert_eq!(b.remainder(), Block::Air, "{b:?}");
        }
        assert!(Block::Water.is_wet() && Block::Water.remainder() == Block::Water);
    }

    #[test]
    fn only_a_bronze_mallet_breaks_a_boulder() {
        let b = Block::Boulder;
        assert!(b.breakable() && !b.hand_breakable() && b.id().is_none());
        assert_eq!(b.break_seconds(), BREAK_SECONDS_BOULDER);
        for held in [None, Some(ItemKind::Stone), Some(ItemKind::StoneAxe), Some(ItemKind::Bow), Some(ItemKind::Bronze)] {
            assert!(!b.can_break_with(held), "{held:?}");
        }
        assert!(b.can_break_with(Some(ItemKind::BronzeMallet)));
        assert!(b.collides() && !b.opaque() && !b.is_full_cube() && !b.supports_placement());
        assert!(!Block::Wood.can_break_with(Some(ItemKind::BronzeMallet)), "trunks still need an axe");
        assert!(Block::Grass.can_break_with(Some(ItemKind::BronzeMallet)), "a mallet still picks like a hand");
        let (lo, hi) = b.bounds().unwrap();
        assert!(hi[1] > 0.7 && hi[0] - lo[0] > 0.8, "a boulder is big: {lo:?} {hi:?}");
    }

    #[test]
    fn a_clay_forge_goes_from_empty_to_filled_to_lit() {
        for b in [Block::ClayForge, Block::ForgeCharged, Block::ForgeLit] {
            assert!(b.collides() && !b.opaque() && !b.is_full_cube() && b.id().is_some(), "{b:?}");
            assert_eq!(b.parts()[0].color, (0.68, 0.54, 0.45), "{b:?} is clay");
        }
        for b in [Block::ClayForge, Block::ForgeCharged] {
            assert!(b.hand_breakable() && b.breakable() && !b.is_burning(), "{b:?}");
            assert_eq!(b.break_seconds(), BREAK_SECONDS_FORGE, "{b:?}");
        }
        assert!(Block::ForgeLit.is_burning() && !Block::ForgeLit.breakable() && Block::ForgeLit.break_seconds() == 0.0);
        assert!(!Block::ForgeLit.can_break_with(None));
        assert!(Block::ClayForge.can_break_with(Some(ItemKind::BowDrill)), "an empty forge cannot be lit");
        assert!(!Block::ForgeCharged.can_break_with(Some(ItemKind::BowDrill)), "lit, not broken");
        assert!(Block::ForgeCharged.can_break_with(None));
        assert_eq!(Block::ClayForge.parts().len(), 2);
        assert_eq!(Block::ForgeCharged.parts().len(), 3);
        assert_eq!(Block::ForgeLit.parts().len(), 3);
        assert!(Block::ForgeLit.parts()[2].color.0 > Block::ForgeCharged.parts()[2].color.0, "lit coal is red-hot");
        assert_eq!(Block::ForgeCharged.bounds(), Block::ForgeLit.bounds(), "lighting does not change the shape");
    }

    #[test]
    fn embers_and_ash_are_small_walk_through_player_made_blocks() {
        assert!(Block::Embers.is_burning() && Block::Fire.is_burning());
        for b in [Block::Charcoal, Block::Ash, Block::Dirt, Block::PlacedLog] {
            assert!(!b.is_burning(), "{b:?}");
        }
        for b in [Block::Embers, Block::Ash] {
            assert!(!b.collides() && !b.opaque() && b.id().is_some(), "{b:?}");
            let (_, hi) = b.bounds().unwrap();
            assert!(hi[1] <= 0.4, "{b:?} lies low");
        }
        assert_eq!(Block::Embers.bounds(), Block::Charcoal.bounds(), "lighting charcoal does not change its shape");
    }

    #[test]
    fn a_bow_in_hand_breaks_nothing() {
        for b in ALL {
            assert!(!b.can_break_with(Some(ItemKind::Bow)), "{b:?}");
        }
    }

    #[test]
    fn planks_are_a_full_player_made_cube_that_anchors_more_planks() {
        assert!(Block::Planks.is_full_cube() && Block::Planks.supports_placement());
        assert_eq!(Block::Planks.id(), Some("planks"));
        assert!(Block::Planks.anchors_planks() && Block::Dirt.anchors_planks() && Block::Stone.anchors_planks());
        for b in [Block::Leaves, Block::Needles, Block::Wood, Block::PlacedLog, Block::Fire, Block::Air] {
            assert!(!b.anchors_planks(), "{b:?}");
        }
    }

    #[test]
    fn every_shape_fits_in_its_cell() {
        for b in ALL {
            // Only a boulder bulges past its cell, and only sideways.
            let slack = |a: usize| if b == Block::Boulder && a != 1 { BOULDER_OVERHANG } else { 0.0 };
            match b.bounds() {
                None => assert_eq!(b, Block::Air),
                Some((lo, hi)) => {
                    for a in 0..3 {
                        assert!(-slack(a) <= lo[a] && lo[a] < hi[a] && hi[a] <= 1.0 + slack(a), "{b:?} axis {a}");
                    }
                }
            }
            for p in b.parts() {
                for a in 0..3 {
                    assert!(-slack(a) <= p.min[a] && p.min[a] < p.max[a] && p.max[a] <= 1.0 + slack(a), "{b:?} part axis {a}");
                }
            }
        }
    }

    #[test]
    fn trunks_are_thinner_than_a_block_and_small_items_are_small() {
        let (lo, hi) = Block::Wood.bounds().unwrap();
        assert!((hi[0] - lo[0] - (1.0 - 2.0 * TRUNK_INSET)).abs() < 1e-6);
        assert_eq!((lo[1], hi[1]), (0.0, 1.0), "trunks stay full height");
        for b in [Block::PlacedPebble, Block::PlacedStone, Block::Charcoal] {
            let (lo, hi) = b.bounds().unwrap();
            assert!(hi[1] - lo[1] <= 0.4 && hi[0] - lo[0] <= 0.8 && hi[2] - lo[2] <= 0.8, "{b:?}");
        }
        let (_, hi) = Block::PlacedLog.bounds().unwrap();
        assert!(hi[1] <= 0.5, "placed logs lie low");
    }

    #[test]
    fn player_made_blocks_round_trip_through_ids() {
        for b in Block::PLAYER_MADE {
            assert_eq!(Block::from_id(b.id().unwrap()), Some(b));
        }
        assert_eq!(Block::from_id("dirt"), None);
        assert_eq!(Block::Dirt.id(), None);
        assert_eq!(Block::from_id("nonsense"), None);
    }

    #[test]
    fn only_air_has_no_bounds() {
        assert_eq!(ALL.iter().filter(|b| b.bounds().is_none()).count(), 1);
    }

    #[cfg(test)]
    mod more {
        use super::*;

        #[test]
        fn a_boulder_is_plain_grey_rock_with_no_red_iron_showing() {
            for p in Block::Boulder.parts() {
                let (r, g, b) = p.color;
                assert!((r - b).abs() < 0.06 && (r - g).abs() < 0.06, "no rust red: {:?}", p.color);
            }
        }

        #[test]
        fn a_boulder_is_a_little_wider_than_a_block_with_one_square_collider() {
            let (lo, hi) = Block::Boulder.bounds().unwrap();
            assert!((lo[0] + BOULDER_OVERHANG).abs() < 1e-6 && (hi[0] - 1.0 - BOULDER_OVERHANG).abs() < 1e-6, "{lo:?} {hi:?}");
            assert!((lo[2] + BOULDER_OVERHANG).abs() < 1e-6 && (hi[2] - 1.0 - BOULDER_OVERHANG).abs() < 1e-6, "{lo:?} {hi:?}");
            let (wx, wz) = (hi[0] - lo[0], hi[2] - lo[2]);
            assert!(wx > 1.1 && wx < 1.5 && (wx - wz).abs() < 1e-6, "square and a little over a block: {wx} x {wz}");
            assert!(hi[1] <= 1.0 && lo[1] == 0.0, "no taller than a block, so it can be hopped onto");
            assert!(Block::Boulder.collides() && !Block::Boulder.is_full_cube());
        }

        #[test]
        fn a_boulder_is_built_of_layers_with_cut_corners_not_one_cube() {
            let parts = Block::Boulder.parts();
            assert!(parts.len() >= 8, "{} boxes", parts.len());
            // The top is narrower than the bottom, and no single box fills the whole footprint.
            let widest = |f: &dyn Fn(&Part) -> bool| parts.iter().filter(|p| f(p)).map(|p| p.max[0] - p.min[0]).fold(0.0_f32, f32::max);
            assert!(widest(&|p| p.min[1] >= 0.66) < widest(&|p| p.min[1] < 0.01) - 0.3);
            let (lo, hi) = Block::Boulder.bounds().unwrap();
            for p in parts {
                assert!(p.max[0] - p.min[0] < hi[0] - lo[0] || p.max[2] - p.min[2] < hi[2] - lo[2], "a box covers the whole footprint: {:?}", p.min);
            }
            // The corners of the bounding square are empty: no part reaches into them.
            let corner = |cx: f32, cz: f32| parts.iter().any(|p| p.min[0] <= cx + 0.05 && p.max[0] >= cx - 0.05 && p.min[2] <= cz + 0.05 && p.max[2] >= cz - 0.05 && p.min[1] < 0.3);
            for (cx, cz) in [(lo[0], lo[2]), (hi[0], lo[2]), (lo[0], hi[2]), (hi[0], hi[2])] {
                assert!(!corner(cx, cz), "the corner ({cx}, {cz}) is cut");
            }
        }

        #[test]
        fn a_loadstone_is_a_small_dark_walk_through_rock_that_breaks_by_hand() {
            let b = Block::Loadstone;
            assert!(b.hand_breakable() && b.breakable() && b.can_break_with(None) && b.can_break_with(Some(ItemKind::StoneAxe)));
            assert_eq!(b.break_seconds(), BREAK_SECONDS_PLACED);
            assert!(!b.collides() && !b.opaque() && !b.is_full_cube() && !b.is_wet() && !b.is_burning());
            assert_eq!(b.id(), None, "it is found, not placed");
            let (lo, hi) = b.bounds().unwrap();
            assert!(hi[1] <= 0.45 && lo[1] == 0.0 && hi[0] - lo[0] <= 0.8);
            let (r, g, bl) = b.base_color();
            assert!(r < 0.3 && g < 0.3 && bl < 0.35, "dark");
        }

        #[test]
        fn a_sling_in_hand_breaks_nothing_just_like_a_bow() {
            for b in ALL {
                assert!(!b.can_break_with(Some(ItemKind::Sling)), "{b:?}");
            }
        }

        #[test]
        fn every_axe_chops_a_trunk_but_a_mallet_and_a_sling_do_not() {
            for axe in [ItemKind::StoneAxe, ItemKind::CopperAxe, ItemKind::BronzeAxe, ItemKind::IronAxe] {
                assert!(Block::Wood.can_break_with(Some(axe)), "{axe:?}");
            }
            assert!(!Block::Wood.can_break_with(Some(ItemKind::BronzeMallet)) && !Block::Wood.can_break_with(Some(ItemKind::Sling)));
        }
    }
}
