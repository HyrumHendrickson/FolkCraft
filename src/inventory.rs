//! The player's inventory: a fixed grid of stackable slots.

use crate::config::*;
use crate::item::{ItemKind, ItemStack};

#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory { slots: vec![None; INVENTORY_SLOTS] }
    }

    /// Whether the extra rows of a worn backpack are there.
    pub fn has_pack_rows(&self) -> bool {
        self.slots.len() > INVENTORY_SLOTS
    }

    /// Add the rows of a backpack (empty).
    pub fn add_pack_rows(&mut self) {
        self.slots.resize(INVENTORY_SLOTS + BACKPACK_SLOTS, None);
    }

    /// Take the rows of a backpack away: what lies in them moves into free room in the other slots. All-or-nothing: if it
    /// would not all fit, nothing changes and this returns false.
    pub fn remove_pack_rows(&mut self) -> bool {
        if !self.has_pack_rows() {
            return true;
        }
        let mut trial = self.clone();
        let extra: Vec<ItemStack> = trial.slots.split_off(INVENTORY_SLOTS).into_iter().flatten().collect();
        for stack in extra {
            if trial.add(stack.kind, stack.count) != 0 {
                return false;
            }
        }
        *self = trial;
        true
    }

    /// Put on the garment in `slot` (a cloak, the armour, the crown or a backpack) in the one worn place. Whatever was worn
    /// goes back into the inventory. A backpack brings its extra rows with it and takes them away again when it comes off
    /// (what lies in them has to fit in the other slots). All-or-nothing: on `Err` nothing has changed.
    pub fn put_on(&mut self, worn: &mut Option<ItemKind>, slot: usize) -> Result<ItemKind, &'static str> {
        let kind = self.slots.get(slot).copied().flatten().map(|s| s.kind).filter(|k| k.is_wearable()).ok_or("That cannot be worn")?;
        let mut trial = self.clone();
        trial.take_from_slot(slot);
        if let Some(old) = *worn {
            if old.is_backpack() && !trial.remove_pack_rows() {
                return Err("Empty the backpack first: its things do not fit in the other slots");
            }
            if trial.add(old, 1) != 0 {
                return Err("Inventory full");
            }
        }
        if kind.is_backpack() {
            trial.add_pack_rows();
        }
        *self = trial;
        *worn = Some(kind);
        Ok(kind)
    }

    /// Take off what is worn, into the inventory. A backpack's rows must be emptied into the other slots first. All-or-nothing:
    /// on `Err` nothing has changed (and `Ok(None)` means nothing was worn).
    pub fn take_off(&mut self, worn: &mut Option<ItemKind>) -> Result<Option<ItemKind>, &'static str> {
        let Some(old) = *worn else { return Ok(None) };
        let mut trial = self.clone();
        if old.is_backpack() && !trial.remove_pack_rows() {
            return Err("Empty the backpack first: its things do not fit in the other slots");
        }
        if trial.add(old, 1) != 0 {
            return Err("Inventory full");
        }
        *self = trial;
        *worn = None;
        Ok(Some(old))
    }

    /// Add `count` items, topping up existing stacks first, then using empty slots.
    /// Returns how many items did not fit.
    pub fn add(&mut self, kind: ItemKind, count: u32) -> u32 {
        let mut left = count;
        for stack in self.slots.iter_mut().flatten() {
            if left == 0 {
                return 0;
            }
            if stack.kind == kind && stack.count < MAX_STACK {
                let n = (MAX_STACK - stack.count).min(left);
                stack.count += n;
                left -= n;
            }
        }
        for slot in self.slots.iter_mut() {
            if left == 0 {
                return 0;
            }
            if slot.is_none() {
                let n = left.min(MAX_STACK);
                *slot = Some(ItemStack { kind, count: n });
                left -= n;
            }
        }
        left
    }

    /// Remove `count` items of `kind`, taking from the last slots first so the hotbar is
    /// emptied last. All-or-nothing: returns false (changing nothing) if there are too few.
    pub fn remove(&mut self, kind: ItemKind, count: u32) -> bool {
        if self.count(kind) < count {
            return false;
        }
        let mut left = count;
        for slot in self.slots.iter_mut().rev() {
            if left == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.kind == kind {
                    let n = stack.count.min(left);
                    stack.count -= n;
                    left -= n;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        true
    }

    /// Take one item out of `slot` (clearing the slot when it was the last one).
    pub fn take_from_slot(&mut self, slot: usize) -> Option<ItemKind> {
        let stack = self.slots.get_mut(slot)?.as_mut()?;
        let kind = stack.kind;
        stack.count -= 1;
        if stack.count == 0 {
            self.slots[slot] = None;
        }
        Some(kind)
    }

    /// Drag-and-drop move of the stack in slot `from` onto slot `to`:
    /// - empty target: the stack moves;
    /// - same kind: as much as fits merges into the target, the rest stays in `from`;
    /// - different kind: the two stacks swap.
    /// Out-of-range or identical slots, and empty sources, do nothing.
    pub fn move_stack(&mut self, from: usize, to: usize) {
        if from == to || from >= self.slots.len() || to >= self.slots.len() {
            return;
        }
        let Some(src) = self.slots[from] else { return };
        match self.slots[to] {
            None => {
                self.slots[to] = Some(src);
                self.slots[from] = None;
            }
            Some(dst) if dst.kind == src.kind => {
                let n = (MAX_STACK - dst.count).min(src.count);
                if n == 0 {
                    return;
                }
                self.slots[to] = Some(ItemStack { kind: dst.kind, count: dst.count + n });
                let left = src.count - n;
                self.slots[from] = (left > 0).then_some(ItemStack { kind: src.kind, count: left });
            }
            Some(_) => self.slots.swap(from, to),
        }
    }

    /// Total number of `kind` items held.
    pub fn count(&self, kind: ItemKind) -> u32 {
        self.slots.iter().flatten().filter(|s| s.kind == kind).map(|s| s.count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backpack_adds_three_rows_that_can_only_be_taken_away_when_what_is_in_them_fits_elsewhere() {
        let mut inv = Inventory::new();
        assert_eq!(inv.slots.len(), INVENTORY_SLOTS);
        inv.add_pack_rows();
        assert_eq!(inv.slots.len(), INVENTORY_SLOTS + BACKPACK_EXTRA_ROWS * HOTBAR_SLOTS);
        assert_eq!((INVENTORY_SLOTS, BACKPACK_EXTRA_ROWS), (45, 3), "one more row of slots, and one more row from the pack");
        assert!(inv.has_pack_rows());
        // The new slots take items like any other.
        assert_eq!(inv.add(ItemKind::Stone, MAX_STACK * (INVENTORY_SLOTS + BACKPACK_SLOTS) as u32), 0);
        assert_eq!(inv.add(ItemKind::Stone, 1), 1, "and then it is full");
        let mut full = Inventory::new();
        full.add_pack_rows();
        for slot in 0..full.slots.len() {
            full.slots[slot] = Some(ItemStack { kind: ItemKind::Stone, count: MAX_STACK });
        }
        assert!(!full.remove_pack_rows(), "the rows are full of things that have nowhere to go");
        assert_eq!(full.slots.len(), INVENTORY_SLOTS + BACKPACK_SLOTS, "nothing changed");
        // Stones merge into the stacks that are there, so a light load comes off.
        let mut light = Inventory::new();
        light.add_pack_rows();
        light.slots[INVENTORY_SLOTS + 3] = Some(ItemStack { kind: ItemKind::Gold, count: 7 });
        assert!(light.remove_pack_rows());
        assert_eq!((light.slots.len(), light.count(ItemKind::Gold)), (INVENTORY_SLOTS, 7));
    }

    fn with(slot: usize, kind: ItemKind) -> Inventory {
        let mut inv = Inventory::new();
        inv.slots[slot] = Some(ItemStack { kind, count: 1 });
        inv
    }

    #[test]
    fn a_backpack_and_a_cloak_share_the_one_worn_place() {
        let mut inv = with(2, ItemKind::LeatherBackpack);
        let mut worn = None;
        assert_eq!(inv.put_on(&mut worn, 2), Ok(ItemKind::LeatherBackpack));
        assert_eq!((worn, inv.slots.len(), inv.count(ItemKind::LeatherBackpack)), (Some(ItemKind::LeatherBackpack), INVENTORY_SLOTS + BACKPACK_SLOTS, 0));
        // Things in the pack's rows come along into the other slots when a cloak takes its place.
        inv.slots[INVENTORY_SLOTS + 1] = Some(ItemStack { kind: ItemKind::Gold, count: 4 });
        inv.slots[5] = Some(ItemStack { kind: ItemKind::PeltCloak, count: 1 });
        assert_eq!(inv.put_on(&mut worn, 5), Ok(ItemKind::PeltCloak));
        assert_eq!(worn, Some(ItemKind::PeltCloak));
        assert_eq!((inv.slots.len(), inv.count(ItemKind::Gold), inv.count(ItemKind::LeatherBackpack)), (INVENTORY_SLOTS, 4, 1), "the pack is back in the inventory, its rows gone");
        // And a backpack in turn sends the cloak back.
        let slot = inv.slots.iter().position(|s| s.map_or(false, |s| s.kind == ItemKind::LeatherBackpack)).unwrap();
        assert_eq!(inv.put_on(&mut worn, slot), Ok(ItemKind::LeatherBackpack));
        assert_eq!((worn, inv.count(ItemKind::PeltCloak), inv.slots.len()), (Some(ItemKind::LeatherBackpack), 1, INVENTORY_SLOTS + BACKPACK_SLOTS));
        assert_eq!(inv.put_on(&mut worn, 30), Err("That cannot be worn"), "an empty slot is nothing to wear");
    }

    #[test]
    fn a_full_pack_cannot_be_swapped_for_a_cloak_and_nothing_changes() {
        let mut inv = Inventory::new();
        inv.add_pack_rows();
        for slot in 0..inv.slots.len() {
            inv.slots[slot] = Some(ItemStack { kind: ItemKind::Stone, count: MAX_STACK });
        }
        inv.slots[3] = Some(ItemStack { kind: ItemKind::PeltCloak, count: 1 });
        let mut worn = Some(ItemKind::LeatherBackpack);
        let before = inv.clone();
        assert!(inv.put_on(&mut worn, 3).is_err());
        assert_eq!((inv, worn), (before.clone(), Some(ItemKind::LeatherBackpack)));
        let mut inv = before;
        assert!(inv.take_off(&mut worn).is_err());
        assert_eq!(worn, Some(ItemKind::LeatherBackpack));
    }

    #[test]
    fn taking_off_returns_the_garment_and_drops_the_pack_rows() {
        let mut inv = Inventory::new();
        let mut worn = None;
        assert_eq!(inv.take_off(&mut worn), Ok(None));
        inv.add_pack_rows();
        worn = Some(ItemKind::LeatherBackpack);
        inv.slots[INVENTORY_SLOTS] = Some(ItemStack { kind: ItemKind::Log, count: 3 });
        assert_eq!(inv.take_off(&mut worn), Ok(Some(ItemKind::LeatherBackpack)));
        assert_eq!((worn, inv.slots.len(), inv.count(ItemKind::Log), inv.count(ItemKind::LeatherBackpack)), (None, INVENTORY_SLOTS, 3, 1));
    }

    #[test]
    fn items_stack_into_one_slot() {
        let mut inv = Inventory::new();
        assert_eq!(inv.add(ItemKind::Stick, 3), 0);
        assert_eq!(inv.add(ItemKind::Stick, 2), 0);
        assert_eq!(inv.slots[0], Some(ItemStack { kind: ItemKind::Stick, count: 5 }));
        assert_eq!(inv.slots.iter().flatten().count(), 1);
        assert_eq!(inv.count(ItemKind::Stick), 5);
    }

    #[test]
    fn different_items_use_different_slots() {
        let mut inv = Inventory::new();
        inv.add(ItemKind::Stick, 1);
        inv.add(ItemKind::Pebble, 1);
        inv.add(ItemKind::Stone, 1);
        assert_eq!(inv.slots.iter().flatten().count(), 3);
        for k in [ItemKind::Stick, ItemKind::Pebble, ItemKind::Stone] {
            assert_eq!(inv.count(k), 1);
        }
        assert_eq!(inv.count(ItemKind::Log), 0);
    }

    #[test]
    fn full_stacks_overflow_into_the_next_slot() {
        let mut inv = Inventory::new();
        assert_eq!(inv.add(ItemKind::Pebble, MAX_STACK + 10), 0);
        assert_eq!(inv.slots[0].unwrap().count, MAX_STACK);
        assert_eq!(inv.slots[1].unwrap().count, 10);
        assert_eq!(inv.count(ItemKind::Pebble), MAX_STACK + 10);
    }

    #[test]
    fn a_full_inventory_rejects_extra_items() {
        let mut inv = Inventory::new();
        let capacity = INVENTORY_SLOTS as u32 * MAX_STACK;
        assert_eq!(inv.add(ItemKind::Stone, capacity), 0);
        assert_eq!(inv.add(ItemKind::Stone, 5), 5);
        assert_eq!(inv.add(ItemKind::Stick, 1), 1);
        assert_eq!(inv.count(ItemKind::Stone), capacity);
    }

    #[test]
    fn partial_fit_reports_the_remainder() {
        let mut inv = Inventory::new();
        let capacity = INVENTORY_SLOTS as u32 * MAX_STACK;
        inv.add(ItemKind::Stone, capacity - 3);
        assert_eq!(inv.add(ItemKind::Stone, 10), 7);
        assert_eq!(inv.count(ItemKind::Stone), capacity);
    }

    #[test]
    fn remove_takes_from_the_back_and_clears_empty_slots() {
        let mut inv = Inventory::new();
        inv.slots[0] = Some(ItemStack { kind: ItemKind::Stick, count: 3 });
        inv.slots[10] = Some(ItemStack { kind: ItemKind::Stick, count: 2 });
        assert!(inv.remove(ItemKind::Stick, 3));
        assert_eq!(inv.slots[10], None);
        assert_eq!(inv.slots[0], Some(ItemStack { kind: ItemKind::Stick, count: 2 }));
        assert!(!inv.remove(ItemKind::Stick, 3), "not enough");
        assert_eq!(inv.count(ItemKind::Stick), 2, "failed removal changes nothing");
        assert!(inv.remove(ItemKind::Stick, 0));
    }

    #[test]
    fn take_from_slot_takes_one_and_clears_empty_slots() {
        let mut inv = Inventory::new();
        inv.slots[4] = Some(ItemStack { kind: ItemKind::Log, count: 2 });
        assert_eq!(inv.take_from_slot(4), Some(ItemKind::Log));
        assert_eq!(inv.slots[4].unwrap().count, 1);
        assert_eq!(inv.take_from_slot(4), Some(ItemKind::Log));
        assert_eq!(inv.slots[4], None);
        assert_eq!(inv.take_from_slot(4), None);
        assert_eq!(inv.take_from_slot(INVENTORY_SLOTS), None);
    }

    #[test]
    fn move_stack_to_an_empty_slot() {
        let mut inv = Inventory::new();
        inv.add(ItemKind::Stone, 5);
        inv.move_stack(0, 20);
        assert_eq!(inv.slots[0], None);
        assert_eq!(inv.slots[20], Some(ItemStack { kind: ItemKind::Stone, count: 5 }));
    }

    #[test]
    fn move_stack_swaps_different_kinds() {
        let mut inv = Inventory::new();
        inv.add(ItemKind::Stone, 5);
        inv.add(ItemKind::Stick, 2);
        inv.move_stack(0, 1);
        assert_eq!(inv.slots[0].unwrap().kind, ItemKind::Stick);
        assert_eq!(inv.slots[1], Some(ItemStack { kind: ItemKind::Stone, count: 5 }));
    }

    #[test]
    fn move_stack_merges_same_kinds_up_to_the_stack_limit() {
        let mut inv = Inventory::new();
        inv.slots[0] = Some(ItemStack { kind: ItemKind::Pebble, count: 10 });
        inv.slots[1] = Some(ItemStack { kind: ItemKind::Pebble, count: MAX_STACK - 4 });
        inv.move_stack(0, 1);
        assert_eq!(inv.slots[1].unwrap().count, MAX_STACK);
        assert_eq!(inv.slots[0].unwrap().count, 6);
        // A full target accepts nothing.
        inv.move_stack(0, 1);
        assert_eq!(inv.slots[0].unwrap().count, 6);
        assert_eq!(inv.count(ItemKind::Pebble), MAX_STACK + 6);
        // A merge that fits entirely empties the source.
        inv.slots[1] = Some(ItemStack { kind: ItemKind::Pebble, count: 1 });
        inv.move_stack(0, 1);
        assert_eq!(inv.slots[0], None);
        assert_eq!(inv.slots[1].unwrap().count, 7);
    }

    #[test]
    fn move_stack_ignores_bad_moves() {
        let mut inv = Inventory::new();
        inv.add(ItemKind::Stick, 1);
        let before = inv.clone();
        inv.move_stack(0, 0);
        inv.move_stack(0, INVENTORY_SLOTS);
        inv.move_stack(INVENTORY_SLOTS, 0);
        inv.move_stack(5, 6); // empty source
        assert_eq!(inv, before);
    }
}
