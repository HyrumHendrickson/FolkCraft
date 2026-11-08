//! The general store on the Skylands' first island, run by a ghostly figure who sells goods for gold.
//!
//! This file is the stock (`wares`) and the shape of the ghost (`figure_parts`); the buying rules are `Game::buy` and the drawing is
//! `effects::draw_shopkeeper` and `play::draw_shop_dialog`.

use macroquad::prelude::*;

use crate::config::*;
use crate::item::ItemKind;

/// One thing for sale: `count` of `item` for `price` gold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ware {
    pub item: ItemKind,
    pub count: u32,
    pub price: u32,
}

/// The meat stews he sells, one of each kind (a stew is a bowl of stew).
pub const STEWS: [ItemKind; 5] = [ItemKind::VenisonStew, ItemKind::PoultryStew, ItemKind::BearStew, ItemKind::RabbitStew, ItemKind::MooseStew];

/// Everything for sale, in the order the shop lists it: arrows, a bow, a sword, then each meat stew.
pub fn wares() -> Vec<Ware> {
    let mut list = vec![
        Ware { item: ItemKind::Arrow, count: SHOP_ARROWS, price: SHOP_ARROW_PRICE },
        Ware { item: ItemKind::Bow, count: 1, price: SHOP_BOW_PRICE },
        Ware { item: ItemKind::Sword, count: 1, price: SHOP_SWORD_PRICE },
    ];
    list.extend(STEWS.iter().map(|&item| Ware { item, count: 1, price: SHOP_STEW_PRICE }));
    list
}

/// Half-extents of the box round the figure, for aiming at him (centre `post` + half height).
pub const FIGURE_HALF: Vec3 = Vec3::new(0.45, 1.0, 0.45);

/// The ghost as boxes: ((x, y, z) offset from his feet, half extents, colour with alpha). He floats a little, in a tattered hooded robe
/// with a lantern, and fades to nothing at the hem. `time` drives the bob.
pub fn figure_parts(time: f32) -> Vec<((f32, f32, f32), (f32, f32, f32), Color)> {
    let bob = (time * 1.6).sin() * 0.06;
    let robe = |a: f32| Color::new(0.78, 0.86, 0.95, a);
    vec![
        ((0.0, 0.25 + bob, 0.0), (0.34, 0.25, 0.30), robe(0.18)),
        ((0.0, 0.65 + bob, 0.0), (0.30, 0.20, 0.26), robe(0.32)),
        ((0.0, 1.05 + bob, 0.0), (0.26, 0.25, 0.22), robe(0.45)),
        ((0.0, 1.50 + bob, 0.0), (0.20, 0.16, 0.20), Color::new(0.88, 0.93, 1.0, 0.55)),
        ((0.0, 1.50 + bob, 0.14), (0.12, 0.08, 0.06), Color::new(0.02, 0.04, 0.10, 0.85)),
        ((0.30, 1.05 + bob, 0.18), (0.08, 0.08, 0.20), robe(0.5)),
        ((-0.30, 1.05 + bob, 0.18), (0.08, 0.08, 0.20), robe(0.5)),
        ((0.34, 0.85 + bob, 0.36), (0.07, 0.09, 0.07), Color::new(1.0, 0.85, 0.40, 0.9)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stock_and_prices_are_as_promised() {
        let list = wares();
        let find = |item| list.iter().find(|w| w.item == item).copied().expect("for sale");
        assert_eq!((find(ItemKind::Arrow).count, find(ItemKind::Arrow).price), (10, 2));
        assert_eq!((find(ItemKind::Bow).count, find(ItemKind::Bow).price), (1, 10));
        assert_eq!((find(ItemKind::Sword).count, find(ItemKind::Sword).price), (1, 20));
        for stew in STEWS {
            assert_eq!(find(stew).price, SHOP_STEW_PRICE, "{stew:?}");
            assert!(stew.stew_base().is_some(), "only meat stews");
        }
        assert_eq!(list.len(), 3 + 5);
        assert_eq!(SWORD_DAMAGE, IRON_AXE_DAMAGE + 2);
    }
}
