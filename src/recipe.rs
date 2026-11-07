//! Crafting recipes. A recipe consumes a fixed number of items; there is no shape or
//! placement, only counts. To add a recipe, append an entry to `RECIPES`.

use crate::config::JERKY_SALT_COST;
use crate::inventory::Inventory;
use crate::item::ItemKind;

pub struct Recipe {
    pub output: ItemKind,
    pub output_count: u32,
    pub inputs: &'static [(ItemKind, u32)],
}

pub const RECIPES: &[Recipe] = &[
    Recipe { output: ItemKind::Twine, output_count: 1, inputs: &[(ItemKind::Grass, 3)] },
    Recipe { output: ItemKind::LeatherCord, output_count: 3, inputs: &[(ItemKind::Leather, 1)] },
    Recipe { output: ItemKind::BowDrill, output_count: 1, inputs: &[(ItemKind::Twine, 1), (ItemKind::Stick, 1)] },
    Recipe {
        output: ItemKind::StoneAxe,
        output_count: 1,
        inputs: &[(ItemKind::Stick, 1), (ItemKind::Stone, 1), (ItemKind::Twine, 1)],
    },
    Recipe { output: ItemKind::Bow, output_count: 1, inputs: &[(ItemKind::Twine, 1), (ItemKind::Stick, 1)] },
    Recipe {
        output: ItemKind::Bellows,
        output_count: 1,
        inputs: &[(ItemKind::Twine, 1), (ItemKind::Leather, 1), (ItemKind::Stick, 2)],
    },
    Recipe { output: ItemKind::ClayForge, output_count: 1, inputs: &[(ItemKind::Stone, 2), (ItemKind::Clay, 8)] },
    Recipe { output: ItemKind::TinPot, output_count: 1, inputs: &[(ItemKind::Tin, 3)] },
    Recipe { output: ItemKind::LeatherBackpack, output_count: 1, inputs: &[(ItemKind::Leather, 4), (ItemKind::LeatherCord, 2)] },
    Recipe { output: ItemKind::TinPan, output_count: 1, inputs: &[(ItemKind::Tin, 2)] },
    Recipe { output: ItemKind::ClayJar, output_count: 1, inputs: &[(ItemKind::Clay, 4)] },
    Recipe {
        output: ItemKind::CopperAxe,
        output_count: 1,
        inputs: &[(ItemKind::Stick, 1), (ItemKind::Copper, 2), (ItemKind::LeatherCord, 1)],
    },
    Recipe {
        output: ItemKind::BronzeAxe,
        output_count: 1,
        inputs: &[(ItemKind::Stick, 1), (ItemKind::Bronze, 2), (ItemKind::LeatherCord, 1)],
    },
    Recipe {
        output: ItemKind::IronAxe,
        output_count: 1,
        inputs: &[(ItemKind::Stick, 1), (ItemKind::Iron, 2), (ItemKind::LeatherCord, 1)],
    },
    Recipe {
        output: ItemKind::BronzeMallet,
        output_count: 1,
        inputs: &[(ItemKind::Bronze, 2), (ItemKind::LeatherCord, 1), (ItemKind::Stick, 1)],
    },
    Recipe { output: ItemKind::Sling, output_count: 1, inputs: &[(ItemKind::Leather, 1)] },
    Recipe { output: ItemKind::Compass, output_count: 1, inputs: &[(ItemKind::Loadstone, 1), (ItemKind::IronOre, 1)] },
    Recipe { output: ItemKind::BoneTotem, output_count: 1, inputs: &[(ItemKind::Antlers, 1), (ItemKind::Gold, 2)] },
    Recipe { output: ItemKind::PeltCloak, output_count: 1, inputs: &[(ItemKind::BearPelt, 1)] },
    Recipe { output: ItemKind::MusicalPipes, output_count: 1, inputs: &[(ItemKind::Bone, 3)] },
    Recipe {
        output: ItemKind::Arrow,
        output_count: 1,
        inputs: &[(ItemKind::Pebble, 1), (ItemKind::Stick, 1), (ItemKind::Feather, 1)],
    },
    Recipe {
        output: ItemKind::PhoenixArrow,
        output_count: 1,
        inputs: &[(ItemKind::Pebble, 1), (ItemKind::Stick, 1), (ItemKind::PhoenixFeather, 1)],
    },
    Recipe {
        output: ItemKind::PoisonArrow,
        output_count: 1,
        inputs: &[(ItemKind::Pebble, 1), (ItemKind::Stick, 1), (ItemKind::Feather, 1), (ItemKind::SnakeFang, 1)],
    },
    // Jerky: raw meat cured with a lump of pink salt, one kind for each meat. No fire is needed.
    Recipe { output: ItemKind::VenisonJerky, output_count: 1, inputs: &[(ItemKind::Venison, 1), (ItemKind::PinkSalt, JERKY_SALT_COST)] },
    Recipe { output: ItemKind::PoultryJerky, output_count: 1, inputs: &[(ItemKind::Poultry, 1), (ItemKind::PinkSalt, JERKY_SALT_COST)] },
    Recipe { output: ItemKind::BearJerky, output_count: 1, inputs: &[(ItemKind::BearMeat, 1), (ItemKind::PinkSalt, JERKY_SALT_COST)] },
    Recipe { output: ItemKind::RabbitJerky, output_count: 1, inputs: &[(ItemKind::RabbitMeat, 1), (ItemKind::PinkSalt, JERKY_SALT_COST)] },
    Recipe { output: ItemKind::MooseJerky, output_count: 1, inputs: &[(ItemKind::MooseMeat, 1), (ItemKind::PinkSalt, JERKY_SALT_COST)] },
];

/// The recipe that makes `output`, if there is one.
#[cfg(test)]
pub fn recipe_for(output: ItemKind) -> &'static Recipe {
    RECIPES.iter().find(|r| r.output == output).expect("recipe exists")
}

impl Recipe {
    /// Whether the inventory holds every ingredient.
    pub fn can_afford(&self, inventory: &Inventory) -> bool {
        self.inputs.iter().all(|&(kind, n)| inventory.count(kind) >= n)
    }

    /// What the inventory still lacks for this recipe: each ingredient it has too few of, and how many more it needs.
    pub fn missing(&self, inventory: &Inventory) -> Vec<(ItemKind, u32)> {
        self.inputs.iter().filter_map(|&(kind, n)| inventory.count(kind).checked_sub(n).map_or_else(|| Some((kind, n - inventory.count(kind))), |_| None)).collect()
    }

    /// The same as text, such as "2 more Tin, 1 more Stick" (empty when nothing is missing).
    pub fn describe_missing(&self, inventory: &Inventory) -> String {
        let parts: Vec<String> = self.missing(inventory).iter().map(|&(k, n)| format!("{} more {}", n, k.name())).collect();
        parts.join(", ")
    }

    /// Ingredient summary such as "1 Stick + 1 Stone".
    pub fn describe(&self) -> String {
        let parts: Vec<String> = self.inputs.iter().map(|&(k, n)| format!("{} {}", n, k.name())).collect();
        parts.join(" + ")
    }
}

/// Every recipe the inventory can currently pay for, in `RECIPES` order.
pub fn craftable(inventory: &Inventory) -> Vec<&'static Recipe> {
    RECIPES.iter().filter(|r| r.can_afford(inventory)).collect()
}

/// Craft once: consume the ingredients and add the output. Returns false, leaving the
/// inventory untouched, if the ingredients are missing or the output would not fit.
pub fn craft(inventory: &mut Inventory, recipe: &Recipe) -> bool {
    if !recipe.can_afford(inventory) {
        return false;
    }
    let mut trial = inventory.clone();
    for &(kind, n) in recipe.inputs {
        trial.remove(kind, n);
    }
    if trial.add(recipe.output, recipe.output_count) != 0 {
        return false;
    }
    *inventory = trial;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::item::ItemStack;

    fn inv_with(items: &[(ItemKind, u32)]) -> Inventory {
        let mut inv = Inventory::new();
        for &(k, n) in items {
            assert_eq!(inv.add(k, n), 0);
        }
        inv
    }

    #[test]
    fn recipe_costs() {
        assert_eq!(recipe_for(ItemKind::Twine).inputs, &[(ItemKind::Grass, 3)]);
        assert_eq!(recipe_for(ItemKind::BowDrill).inputs, &[(ItemKind::Twine, 1), (ItemKind::Stick, 1)]);
        assert_eq!(
            recipe_for(ItemKind::StoneAxe).inputs,
            &[(ItemKind::Stick, 1), (ItemKind::Stone, 1), (ItemKind::Twine, 1)]
        );
        assert_eq!(recipe_for(ItemKind::StoneAxe).describe(), "1 Stick + 1 Stone + 1 Twine");
        assert_eq!(recipe_for(ItemKind::Bow).inputs, &[(ItemKind::Twine, 1), (ItemKind::Stick, 1)]);
        assert_eq!(
            recipe_for(ItemKind::Arrow).inputs,
            &[(ItemKind::Pebble, 1), (ItemKind::Stick, 1), (ItemKind::Feather, 1)]
        );
        for r in RECIPES {
            let expected = if r.output == ItemKind::LeatherCord { 3 } else { 1 };
            assert_eq!(r.output_count, expected, "{:?}", r.output);
        }
    }

    #[test]
    fn a_piece_of_leather_makes_three_leather_cords() {
        let cords = recipe_for(ItemKind::LeatherCord);
        assert_eq!((cords.inputs, cords.output_count), (&[(ItemKind::Leather, 1)][..], 3));
        assert_eq!(cords.describe(), "1 Leather");
        let mut inv = inv_with(&[(ItemKind::Leather, 2)]);
        assert!(craft(&mut inv, cords));
        assert_eq!((inv.count(ItemKind::Leather), inv.count(ItemKind::LeatherCord)), (1, 3));
        assert!(craft(&mut inv, cords));
        assert_eq!((inv.count(ItemKind::Leather), inv.count(ItemKind::LeatherCord)), (0, 6));
        assert!(!craft(&mut inv, cords), "no leather left");
    }

    #[test]
    fn a_clay_forge_costs_two_stones_and_eight_clay() {
        let forge = recipe_for(ItemKind::ClayForge);
        assert_eq!(forge.describe(), "2 Stone + 8 Clay");
        assert!(!forge.can_afford(&inv_with(&[(ItemKind::Stone, 1), (ItemKind::Clay, 8)])));
        assert!(!forge.can_afford(&inv_with(&[(ItemKind::Stone, 2), (ItemKind::Clay, 7)])));
        assert!(!forge.can_afford(&inv_with(&[(ItemKind::Pebble, 2), (ItemKind::Clay, 8)])), "pebbles are not stones");
        let mut inv = inv_with(&[(ItemKind::Stone, 3), (ItemKind::Clay, 9)]);
        assert!(craft(&mut inv, forge));
        assert_eq!((inv.count(ItemKind::ClayForge), inv.count(ItemKind::Stone), inv.count(ItemKind::Clay)), (1, 1, 1));
    }

    #[test]
    fn a_bronze_mallet_needs_two_bronze_a_leather_cord_and_a_stick() {
        let mallet = recipe_for(ItemKind::BronzeMallet);
        assert_eq!(mallet.describe(), "2 Bronze + 1 Leather Cord + 1 Stick");
        assert!(!mallet.can_afford(&inv_with(&[(ItemKind::Bronze, 1), (ItemKind::LeatherCord, 1), (ItemKind::Stick, 1)])));
        assert!(!mallet.can_afford(&inv_with(&[(ItemKind::Bronze, 2), (ItemKind::Twine, 1), (ItemKind::Stick, 1)])), "twine is not a cord");
        let mut inv = inv_with(&[(ItemKind::Bronze, 2), (ItemKind::LeatherCord, 3), (ItemKind::Stick, 1)]);
        assert!(craft(&mut inv, mallet));
        assert_eq!((inv.count(ItemKind::BronzeMallet), inv.count(ItemKind::Bronze), inv.count(ItemKind::LeatherCord)), (1, 0, 2));
        assert_eq!(inv.count(ItemKind::Stick), 0);
    }

    #[test]
    fn bellows_need_twine_leather_and_two_sticks() {
        let bellows = recipe_for(ItemKind::Bellows);
        assert_eq!(bellows.describe(), "1 Twine + 1 Leather + 2 Stick");
        assert!(!bellows.can_afford(&inv_with(&[(ItemKind::Twine, 1), (ItemKind::Leather, 1), (ItemKind::Stick, 1)])));
        assert!(!bellows.can_afford(&inv_with(&[(ItemKind::Twine, 1), (ItemKind::Stick, 2)])));
        let mut inv = inv_with(&[(ItemKind::Twine, 2), (ItemKind::Leather, 1), (ItemKind::Stick, 3)]);
        assert!(craft(&mut inv, bellows));
        assert_eq!(inv.count(ItemKind::Bellows), 1);
        assert_eq!((inv.count(ItemKind::Twine), inv.count(ItemKind::Leather), inv.count(ItemKind::Stick)), (1, 0, 1));
    }

    #[test]
    fn wooden_planks_can_no_longer_be_crafted() {
        assert!(RECIPES.iter().all(|r| r.output != ItemKind::Plank));
        let inv = inv_with(&[(ItemKind::Log, 5)]);
        assert!(craftable(&inv).is_empty(), "a log makes nothing");
    }

    #[test]
    fn a_bow_needs_twine_and_a_stick_and_an_arrow_needs_a_pebble_stick_and_feather() {
        let mut inv = inv_with(&[(ItemKind::Twine, 1), (ItemKind::Stick, 3), (ItemKind::Pebble, 1)]);
        let arrow = recipe_for(ItemKind::Arrow);
        assert!(!arrow.can_afford(&inv), "no feather yet");
        assert!(craft(&mut inv, recipe_for(ItemKind::Bow)));
        assert_eq!((inv.count(ItemKind::Bow), inv.count(ItemKind::Twine), inv.count(ItemKind::Stick)), (1, 0, 2));
        inv.add(ItemKind::Feather, 1);
        assert!(craft(&mut inv, arrow));
        assert_eq!(inv.count(ItemKind::Arrow), 1);
        assert_eq!((inv.count(ItemKind::Pebble), inv.count(ItemKind::Stick), inv.count(ItemKind::Feather)), (0, 1, 0));
    }

    #[test]
    fn the_stone_axe_now_needs_twine() {
        let axe = recipe_for(ItemKind::StoneAxe);
        assert!(!axe.can_afford(&inv_with(&[(ItemKind::Stick, 1), (ItemKind::Stone, 1)])));
        assert!(axe.can_afford(&inv_with(&[(ItemKind::Stick, 1), (ItemKind::Stone, 1), (ItemKind::Twine, 1)])));
    }

    #[test]
    fn craftable_lists_only_affordable_recipes() {
        let mut inv = Inventory::new();
        assert!(craftable(&inv).is_empty());
        inv.add(ItemKind::Grass, 2);
        assert!(craftable(&inv).is_empty(), "twine needs three grass");
        inv.add(ItemKind::Grass, 1);
        let outputs: Vec<_> = craftable(&inv).iter().map(|r| r.output).collect();
        assert_eq!(outputs, vec![ItemKind::Twine]);
        inv.add(ItemKind::Stick, 1);
        inv.add(ItemKind::Twine, 1);
        let outputs: Vec<_> = craftable(&inv).iter().map(|r| r.output).collect();
        assert_eq!(outputs, vec![ItemKind::Twine, ItemKind::BowDrill, ItemKind::Bow]);
    }

    #[test]
    fn grass_becomes_twine_then_a_bow_drill_and_an_axe() {
        let mut inv = inv_with(&[(ItemKind::Grass, 7), (ItemKind::Stick, 2), (ItemKind::Stone, 1)]);
        assert!(craft(&mut inv, recipe_for(ItemKind::Twine)));
        assert!(craft(&mut inv, recipe_for(ItemKind::Twine)));
        assert_eq!((inv.count(ItemKind::Grass), inv.count(ItemKind::Twine)), (1, 2));
        assert!(!craft(&mut inv, recipe_for(ItemKind::Twine)), "only one grass left");
        assert!(craft(&mut inv, recipe_for(ItemKind::BowDrill)));
        assert!(craft(&mut inv, recipe_for(ItemKind::StoneAxe)));
        assert_eq!(inv.count(ItemKind::BowDrill), 1);
        assert_eq!(inv.count(ItemKind::StoneAxe), 1);
        assert_eq!((inv.count(ItemKind::Twine), inv.count(ItemKind::Stick), inv.count(ItemKind::Stone)), (0, 0, 0));
    }

    #[test]
    fn crafting_leaves_unrelated_items_alone() {
        let mut inv = inv_with(&[(ItemKind::Stick, 3), (ItemKind::Twine, 2), (ItemKind::Pebble, 4)]);
        assert!(craft(&mut inv, recipe_for(ItemKind::BowDrill)));
        assert_eq!(inv.count(ItemKind::Stick), 2);
        assert_eq!(inv.count(ItemKind::Twine), 1);
        assert_eq!(inv.count(ItemKind::Pebble), 4);
    }

    fn full_of_pebbles() -> Inventory {
        let mut inv = Inventory::new();
        for i in 0..INVENTORY_SLOTS {
            inv.slots[i] = Some(ItemStack { kind: ItemKind::Pebble, count: MAX_STACK });
        }
        inv
    }

    #[test]
    fn crafting_fails_cleanly_when_the_output_does_not_fit() {
        let mut inv = full_of_pebbles();
        inv.slots[0] = Some(ItemStack { kind: ItemKind::Stick, count: MAX_STACK });
        inv.slots[1] = Some(ItemStack { kind: ItemKind::Twine, count: MAX_STACK });
        let before = inv.clone();
        // Removing one stick and one twine frees no slot, and a bow drill needs its own.
        assert!(!craft(&mut inv, recipe_for(ItemKind::BowDrill)));
        assert_eq!(inv, before);
    }

    #[test]
    fn crafting_can_use_the_slot_freed_by_its_own_ingredients() {
        let mut inv = full_of_pebbles();
        inv.slots[0] = Some(ItemStack { kind: ItemKind::Stick, count: 1 });
        inv.slots[1] = Some(ItemStack { kind: ItemKind::Twine, count: 1 });
        assert!(craft(&mut inv, recipe_for(ItemKind::BowDrill)));
        assert_eq!(inv.count(ItemKind::BowDrill), 1);
    }

    #[test]
    fn metal_axes_are_bound_with_leather_cord_and_the_stone_axe_still_uses_twine() {
        for (axe, metal) in [(ItemKind::CopperAxe, ItemKind::Copper), (ItemKind::BronzeAxe, ItemKind::Bronze), (ItemKind::IronAxe, ItemKind::Iron)] {
            let r = recipe_for(axe);
            assert_eq!(r.inputs, &[(ItemKind::Stick, 1), (metal, 2), (ItemKind::LeatherCord, 1)], "{axe:?}");
            assert!(r.inputs.iter().all(|&(k, _)| k != ItemKind::Twine), "{axe:?} takes no twine");
            assert!(!r.can_afford(&inv_with(&[(ItemKind::Stick, 1), (metal, 2), (ItemKind::Twine, 1)])), "{axe:?}");
            let mut inv = inv_with(&[(ItemKind::Stick, 1), (metal, 2), (ItemKind::LeatherCord, 2)]);
            assert!(craft(&mut inv, r));
            assert_eq!((inv.count(axe), inv.count(ItemKind::LeatherCord)), (1, 1));
        }
        assert!(recipe_for(ItemKind::StoneAxe).inputs.contains(&(ItemKind::Twine, 1)));
    }

    #[test]
    fn a_tin_pot_costs_three_tin_and_a_clay_jar_four_clay() {
        assert_eq!(recipe_for(ItemKind::TinPot).describe(), "3 Tin");
        assert_eq!(recipe_for(ItemKind::ClayJar).describe(), "4 Clay");
        let mut inv = inv_with(&[(ItemKind::Clay, 9), (ItemKind::Tin, 2)]);
        assert!(craft(&mut inv, recipe_for(ItemKind::ClayJar)) && craft(&mut inv, recipe_for(ItemKind::ClayJar)));
        assert_eq!((inv.count(ItemKind::ClayJar), inv.count(ItemKind::Clay)), (2, 1));
        assert!(!craft(&mut inv, recipe_for(ItemKind::ClayJar)) && !craft(&mut inv, recipe_for(ItemKind::TinPot)));
    }

    #[test]
    fn a_sling_is_one_leather_and_a_compass_one_loadstone_and_one_iron_ore() {
        assert_eq!(recipe_for(ItemKind::Sling).inputs, &[(ItemKind::Leather, 1)]);
        assert_eq!(recipe_for(ItemKind::Sling).describe(), "1 Leather");
        assert_eq!(recipe_for(ItemKind::Compass).inputs, &[(ItemKind::Loadstone, 1), (ItemKind::IronOre, 1)]);
        let mut inv = inv_with(&[(ItemKind::Leather, 2)]);
        assert!(craft(&mut inv, recipe_for(ItemKind::Sling)));
        assert_eq!((inv.count(ItemKind::Leather), inv.count(ItemKind::Sling)), (1, 1));
        let mut inv = inv_with(&[(ItemKind::Loadstone, 1)]);
        assert!(!craft(&mut inv, recipe_for(ItemKind::Compass)), "the iron ore is missing");
        assert_eq!(inv.count(ItemKind::Loadstone), 1);
    }

    #[test]
    fn what_is_missing_for_a_recipe_is_counted_ingredient_by_ingredient() {
        let r = recipe_for(ItemKind::BronzeAxe);
        let inv = inv_with(&[(ItemKind::Stick, 1), (ItemKind::Bronze, 1)]);
        assert_eq!(r.missing(&inv), vec![(ItemKind::Bronze, 1), (ItemKind::LeatherCord, 1)]);
        assert_eq!(r.describe_missing(&inv), "1 more Bronze, 1 more Leather Cord");
        let full = inv_with(&[(ItemKind::Stick, 1), (ItemKind::Bronze, 2), (ItemKind::LeatherCord, 1)]);
        assert!(r.missing(&full).is_empty() && r.describe_missing(&full).is_empty() && r.can_afford(&full));
        assert_eq!(recipe_for(ItemKind::TinPan).describe_missing(&Inventory::new()), "2 more Tin");
    }

    #[test]
    fn musical_pipes_are_three_deer_bones_and_nothing_else() {
        assert_eq!(recipe_for(ItemKind::MusicalPipes).inputs, &[(ItemKind::Bone, 3)]);
        let mut inv = inv_with(&[(ItemKind::Bone, 2), (ItemKind::Twine, 1)]);
        assert!(!craft(&mut inv, recipe_for(ItemKind::MusicalPipes)), "one bone short");
        assert_eq!((inv.count(ItemKind::Bone), inv.count(ItemKind::Twine)), (2, 1));
        let mut inv = inv_with(&[(ItemKind::Bone, 3)]);
        assert!(craft(&mut inv, recipe_for(ItemKind::MusicalPipes)), "no twine needed");
        assert_eq!((inv.count(ItemKind::Bone), inv.count(ItemKind::MusicalPipes)), (0, 1));
    }
}
