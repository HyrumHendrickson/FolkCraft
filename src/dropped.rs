//! Items lying in the world: spawning, physics, pickup and drawing.

use macroquad::prelude::*;

use crate::config::*;
use crate::inventory::Inventory;
use crate::item::ItemKind;
use crate::mesh::FACES;
use crate::rng::Rng;
use crate::world::{in_render_range, World};

/// Horizontal half-size of an item's collision box.
const COLLIDE_HALF_XZ: f32 = 0.1;

#[derive(Clone, Debug)]
pub struct DroppedItem {
    pub kind: ItemKind,
    /// Centre of the item's collision box.
    pub pos: Vec3,
    pub vel: Vec3,
    /// Seconds since the item appeared (items cannot be picked up for `PICKUP_DELAY`).
    pub age: f32,
}

impl ItemKind {
    /// Half-height of the collision box, so each item rests flat on the ground.
    fn rest_half_height(self) -> f32 {
        match self {
            ItemKind::Stick => 0.04,
            ItemKind::Pebble => 0.07,
            ItemKind::Stone => 0.11,
            ItemKind::Log => 0.10,
            ItemKind::StoneAxe | ItemKind::CopperAxe | ItemKind::BronzeAxe | ItemKind::IronAxe => 0.05,
            ItemKind::Grass => 0.04,
            ItemKind::Berries | ItemKind::PurpleBerries => 0.05,
            ItemKind::Twine => 0.04,
            ItemKind::BowDrill => 0.04,
            ItemKind::Charcoal => 0.07,
            ItemKind::Plank => 0.04,
            ItemKind::Feather => 0.02,
            ItemKind::Bow => 0.04,
            ItemKind::Arrow => 0.03,
            ItemKind::Venison | ItemKind::CookedVenison | ItemKind::RabbitMeat | ItemKind::CookedRabbit => 0.06,
            ItemKind::MooseMeat | ItemKind::CookedMoose => 0.07,
            ItemKind::Poultry | ItemKind::CookedPoultry => 0.06,
            ItemKind::Leather => 0.03,
            ItemKind::BearMeat | ItemKind::CookedBearMeat => 0.07,
            ItemKind::BearPelt => 0.03,
            ItemKind::Bellows => 0.05,
            ItemKind::Ash => 0.04,
            ItemKind::Copper | ItemKind::Tin | ItemKind::Iron | ItemKind::Gold | ItemKind::Bronze => 0.04,
            ItemKind::Clay => 0.06,
            ItemKind::IronOre | ItemKind::GoldOre => 0.11,
            ItemKind::LeatherCord => 0.02,
            ItemKind::BronzeMallet => 0.06,
            ItemKind::ClayForge => 0.10,
            ItemKind::RabbitFoot => 0.04,
            ItemKind::PeepStone => 0.08,
            ItemKind::HollyRod => 0.04,
            ItemKind::TinPot => 0.09,
            ItemKind::ClayJar | ItemKind::WaterJar => 0.10,
            ItemKind::VenisonStew | ItemKind::PoultryStew | ItemKind::BearStew | ItemKind::RabbitStew | ItemKind::MooseStew
            | ItemKind::RedBerryStew | ItemKind::PurpleBerryStew => 0.06,
            ItemKind::Sling => 0.03,
            ItemKind::Loadstone => 0.08,
            ItemKind::Compass => 0.05,
            ItemKind::Antlers => 0.06,
            ItemKind::BoneTotem => 0.07,
            ItemKind::PeltCloak => 0.03,
            ItemKind::RoyalArmor => 0.06,
            ItemKind::SilverKey => 0.03,
            ItemKind::PinkSalt => 0.07,
            ItemKind::PhilosophersStone => 0.09,
            ItemKind::AncientCrown => 0.08,
            ItemKind::HornOfTheHunt => 0.07,
            ItemKind::DeerBone => 0.04,
            ItemKind::MusicalPipes => 0.05,
            ItemKind::TinPan => 0.03,
            ItemKind::Excalibur => 0.04,
            ItemKind::LeatherBackpack => 0.11,
        }
    }

    /// Half-extents of the box drawn for this item.
    fn draw_half_extents(self) -> Vec3 {
        match self {
            ItemKind::Stick => vec3(0.24, 0.04, 0.04),
            ItemKind::Pebble => vec3(0.07, 0.07, 0.07),
            ItemKind::Stone => vec3(0.13, 0.11, 0.13),
            ItemKind::Log => vec3(0.22, 0.10, 0.10),
            ItemKind::StoneAxe | ItemKind::CopperAxe | ItemKind::BronzeAxe | ItemKind::IronAxe => vec3(0.22, 0.05, 0.09),
            ItemKind::Grass => vec3(0.12, 0.04, 0.12),
            ItemKind::Berries | ItemKind::PurpleBerries => vec3(0.07, 0.05, 0.07),
            ItemKind::Twine => vec3(0.10, 0.04, 0.10),
            ItemKind::BowDrill => vec3(0.26, 0.04, 0.08),
            ItemKind::Charcoal => vec3(0.10, 0.07, 0.09),
            ItemKind::Plank => vec3(0.20, 0.04, 0.08),
            ItemKind::Feather => vec3(0.12, 0.02, 0.04),
            ItemKind::Bow => vec3(0.24, 0.04, 0.10),
            ItemKind::Arrow => vec3(0.26, 0.03, 0.03),
            ItemKind::Venison | ItemKind::CookedVenison | ItemKind::RabbitMeat | ItemKind::CookedRabbit => vec3(0.14, 0.06, 0.10),
            ItemKind::MooseMeat | ItemKind::CookedMoose => vec3(0.16, 0.07, 0.12),
            ItemKind::Poultry | ItemKind::CookedPoultry => vec3(0.10, 0.06, 0.08),
            ItemKind::Leather => vec3(0.16, 0.03, 0.12),
            ItemKind::BearMeat | ItemKind::CookedBearMeat => vec3(0.16, 0.07, 0.12),
            ItemKind::BearPelt => vec3(0.20, 0.03, 0.15),
            ItemKind::Bellows => vec3(0.20, 0.05, 0.10),
            ItemKind::Ash => vec3(0.12, 0.04, 0.10),
            ItemKind::Copper | ItemKind::Tin | ItemKind::Iron | ItemKind::Gold | ItemKind::Bronze => vec3(0.13, 0.04, 0.07),
            ItemKind::Clay => vec3(0.12, 0.06, 0.10),
            ItemKind::IronOre | ItemKind::GoldOre => vec3(0.13, 0.11, 0.13),
            ItemKind::LeatherCord => vec3(0.12, 0.02, 0.04),
            ItemKind::BronzeMallet => vec3(0.24, 0.06, 0.10),
            ItemKind::ClayForge => vec3(0.16, 0.10, 0.16),
            ItemKind::RabbitFoot => vec3(0.08, 0.04, 0.06),
            ItemKind::PeepStone => vec3(0.10, 0.08, 0.10),
            ItemKind::HollyRod => vec3(0.24, 0.04, 0.04),
            ItemKind::TinPot => vec3(0.12, 0.09, 0.12),
            ItemKind::ClayJar | ItemKind::WaterJar => vec3(0.09, 0.10, 0.09),
            ItemKind::VenisonStew | ItemKind::PoultryStew | ItemKind::BearStew | ItemKind::RabbitStew | ItemKind::MooseStew
            | ItemKind::RedBerryStew | ItemKind::PurpleBerryStew => vec3(0.12, 0.06, 0.12),
            ItemKind::Sling => vec3(0.14, 0.03, 0.08),
            ItemKind::Loadstone => vec3(0.10, 0.08, 0.09),
            ItemKind::Compass => vec3(0.10, 0.05, 0.10),
            ItemKind::Antlers => vec3(0.18, 0.06, 0.12),
            ItemKind::BoneTotem => vec3(0.20, 0.07, 0.07),
            ItemKind::PeltCloak => vec3(0.20, 0.03, 0.15),
            ItemKind::RoyalArmor => vec3(0.16, 0.06, 0.12),
            ItemKind::SilverKey => vec3(0.16, 0.03, 0.07),
            ItemKind::PinkSalt => vec3(0.11, 0.08, 0.10),
            ItemKind::PhilosophersStone => vec3(0.12, 0.09, 0.12),
            ItemKind::AncientCrown => vec3(0.14, 0.08, 0.14),
            ItemKind::HornOfTheHunt => vec3(0.20, 0.07, 0.10),
            ItemKind::DeerBone => vec3(0.20, 0.04, 0.05),
            ItemKind::MusicalPipes => vec3(0.15, 0.05, 0.09),
            ItemKind::TinPan => vec3(0.17, 0.03, 0.17),
            ItemKind::Excalibur => vec3(0.30, 0.04, 0.07),
            ItemKind::LeatherBackpack => vec3(0.15, 0.11, 0.10),
        }
    }
}

impl DroppedItem {
    /// A freshly dropped item popping out of the block at `block` with a small random toss.
    pub fn spawn(kind: ItemKind, block: (i32, i32, i32), rng: &mut Rng) -> Self {
        let centre = vec3(block.0 as f32 + 0.5, block.1 as f32 + 0.5, block.2 as f32 + 0.5);
        Self::spawn_at(kind, centre, rng)
    }

    /// A freshly dropped item popping out of the point `pos` with a small random toss.
    pub fn spawn_at(kind: ItemKind, pos: Vec3, rng: &mut Rng) -> Self {
        let angle = rng.unit() * std::f32::consts::TAU;
        let speed = 0.6 + rng.unit() * 1.2;
        DroppedItem { kind, pos, vel: vec3(angle.cos() * speed, 2.5 + rng.unit(), angle.sin() * speed), age: 0.0 }
    }

    /// An item loaded from a save: already old enough to pick up, and not moving.
    pub fn at_rest(kind: ItemKind, pos: Vec3) -> Self {
        DroppedItem { kind, pos, vel: Vec3::ZERO, age: PICKUP_DELAY }
    }

    fn half(&self) -> Vec3 {
        vec3(COLLIDE_HALF_XZ, self.kind.rest_half_height(), COLLIDE_HALF_XZ)
    }

    fn collides(world: &World, centre: Vec3, half: Vec3) -> bool {
        let eps = Vec3::splat(1e-4);
        world.box_blocked(centre - half + eps, centre + half - eps)
    }

    /// Advance the item: gravity, collision with blocks, and friction on the ground.
    /// Items in chunks that have not been generated yet stay frozen.
    pub fn update(&mut self, world: &World, dt: f32) {
        self.age += dt;
        if !world.chunks.contains_key(&World::chunk_of_pos(self.pos)) {
            return;
        }
        self.vel.y = (self.vel.y - GRAVITY * dt).max(-MAX_FALL_SPEED);

        const STEPS: i32 = 3;
        let h = dt / STEPS as f32;
        let half = self.half();
        let mut on_ground = false;
        for _ in 0..STEPS {
            for axis in 0..3 {
                let mut next = self.pos;
                match axis {
                    0 => next.x += self.vel.x * h,
                    1 => next.y += self.vel.y * h,
                    _ => next.z += self.vel.z * h,
                }
                if Self::collides(world, next, half) {
                    match axis {
                        0 => self.vel.x = 0.0,
                        1 => {
                            if self.vel.y < 0.0 {
                                on_ground = true;
                            }
                            self.vel.y = 0.0;
                        }
                        _ => self.vel.z = 0.0,
                    }
                } else {
                    self.pos = next;
                }
            }
        }
        if on_ground {
            let drag = (1.0 - 10.0 * dt).max(0.0);
            self.vel.x *= drag;
            self.vel.z *= drag;
        }
    }

    /// Whether a player standing at `player_pos` (feet) can pick this item up.
    pub fn can_pickup(&self, player_pos: Vec3) -> bool {
        let dx = self.pos.x - player_pos.x;
        let dz = self.pos.z - player_pos.z;
        self.age >= PICKUP_DELAY
            && dx * dx + dz * dz <= PICKUP_RADIUS * PICKUP_RADIUS
            && self.pos.y >= player_pos.y - PICKUP_BELOW
            && self.pos.y <= player_pos.y + PLAYER_HEIGHT + 0.5
    }
}

/// Move every item the player can reach into the inventory. Items that do not fit stay
/// on the ground. Returns the kinds that were picked up.
pub fn collect_nearby(items: &mut Vec<DroppedItem>, inventory: &mut Inventory, player_pos: Vec3) -> Vec<ItemKind> {
    let mut picked = Vec::new();
    let mut i = 0;
    while i < items.len() {
        if items[i].can_pickup(player_pos) && inventory.add(items[i].kind, 1) == 0 {
            picked.push(items.swap_remove(i).kind);
        } else {
            i += 1;
        }
    }
    picked
}

/// Draw a rotated, flat-shaded box (translucent if the colour's alpha is below 1). Call while the 3D camera is active.
pub fn draw_box(centre: Vec3, half: Vec3, yaw: f32, color: Color) {
    draw_box_tilted(centre, half, yaw, 0.0, color);
}

/// `draw_box`, tilted first by `pitch` radians about the box's own z axis (positive tips the top of the box back, towards
/// -x, and swings the bottom forward) and then turned by `yaw` about the vertical. Used to pose limbs, necks and ears.
pub fn draw_box_tilted(centre: Vec3, half: Vec3, yaw: f32, pitch: f32, color: Color) {
    let (s, c) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let mut vertices = Vec::with_capacity(24);
    let mut indices: Vec<u16> = Vec::with_capacity(36);
    for face in FACES.iter() {
        let base = vertices.len() as u16;
        let shade = face.shade;
        let col = Color::new(color.r * shade, color.g * shade, color.b * shade, color.a);
        for &(ox, oy, oz) in &face.corners {
            let (px, py, lz) = ((ox - 0.5) * 2.0 * half.x, (oy - 0.5) * 2.0 * half.y, (oz - 0.5) * 2.0 * half.z);
            let (lx, ly) = (px * cp - py * sp, px * sp + py * cp);
            let (rx, rz) = (lx * c - lz * s, lx * s + lz * c);
            vertices.push(Vertex::new(centre.x + rx, centre.y + ly, centre.z + rz, 0.0, 0.0, col));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

/// Draw items in range of the player as small spinning, bobbing boxes.
pub fn draw_dropped_items(items: &[DroppedItem], time: f32, player_chunk: (i32, i32)) {
    for item in items {
        if !in_render_range(player_chunk, World::chunk_of_pos(item.pos)) {
            continue;
        }
        let phase = item.pos.x * 3.1 + item.pos.z * 7.7;
        let bob = 0.07 + 0.04 * (time * 2.5 + phase).sin();
        let centre = item.pos + vec3(0.0, bob, 0.0);
        let yaw = time * 1.6 + phase;
        match item.kind {
            ItemKind::ClayJar | ItemKind::WaterJar => draw_jar(centre, yaw, item.kind == ItemKind::WaterJar),
            _ => draw_box(centre, item.kind.draw_half_extents(), yaw, item.kind.color()),
        }
    }
}

/// A jar lying about: a round-bellied clay jar with a neck and a lip, or the same jar once it holds water, when it has gone
/// clear: translucent glass with blue water in it and only the lip still clay.
fn draw_jar(centre: Vec3, yaw: f32, water: bool) {
    let clay = ItemKind::ClayJar.color();
    let lip = (vec3(0.0, 0.10, 0.0), vec3(0.065, 0.015, 0.065));
    if !water {
        draw_box(centre + vec3(0.0, -0.035, 0.0), vec3(0.09, 0.065, 0.09), yaw, clay);
        draw_box(centre + vec3(0.0, -0.01, 0.0), vec3(0.092, 0.012, 0.092), yaw, Color::new(0.50, 0.28, 0.20, 1.0));
        draw_box(centre + vec3(0.0, 0.065, 0.0), vec3(0.05, 0.04, 0.05), yaw, clay);
        draw_box(centre + lip.0, lip.1, yaw, Color::new(clay.r + 0.04, clay.g + 0.04, clay.b + 0.04, 1.0));
        return;
    }
    // Opaque parts first, then the glass over them.
    draw_box(centre + lip.0, lip.1, yaw, clay);
    draw_box(centre + vec3(0.0, -0.05, 0.0), vec3(0.074, 0.046, 0.074), yaw, Color::new(0.20, 0.50, 0.92, 0.80));
    draw_box(centre + vec3(0.0, -0.004, 0.0), vec3(0.074, 0.003, 0.074), yaw, Color::new(0.70, 0.90, 1.0, 0.85));
    draw_box(centre + vec3(0.0, -0.035, 0.0), vec3(0.09, 0.065, 0.09), yaw, Color::new(0.82, 0.93, 1.0, 0.30));
    draw_box(centre + vec3(0.0, 0.065, 0.0), vec3(0.05, 0.04, 0.05), yaw, Color::new(0.82, 0.93, 1.0, 0.30));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn world() -> World {
        let mut w = World::new(1, HashSet::new());
        w.stream_chunks(vec3(10.0, 6.0, 10.0));
        // A boulder is a solid bump, unlike the pebbles and stones: clear any from the spot the tests use.
        for x in 8..14 {
            for z in 8..14 {
                if w.block_at(x, GROUND_TOP + 1, z) == crate::block::Block::Boulder {
                    w.break_block(x, GROUND_TOP + 1, z);
                }
            }
        }
        w
    }

    fn settle(item: &mut DroppedItem, w: &World, seconds: f32) {
        for _ in 0..(seconds * 60.0) as i32 {
            item.update(w, 1.0 / 60.0);
        }
    }

    #[test]
    fn spawned_item_pops_out_then_rests_on_the_ground() {
        let w = world();
        let mut rng = Rng::new(3);
        let mut item = DroppedItem::spawn(ItemKind::Pebble, (10, 6, 10), &mut rng);
        assert!(item.vel.y > 0.0);
        settle(&mut item, &w, 3.0);
        let floor = (GROUND_TOP + 1) as f32 + ItemKind::Pebble.rest_half_height();
        assert!((item.pos.y - floor).abs() < 0.02, "y = {}", item.pos.y);
        assert!(item.vel.length() < 0.05);
    }

    #[test]
    fn items_come_to_rest_for_every_kind() {
        let w = world();
        let mut rng = Rng::new(4);
        for kind in ItemKind::ALL {
            let mut item = DroppedItem::spawn(kind, (10, 8, 10), &mut rng);
            settle(&mut item, &w, 4.0);
            let floor = (GROUND_TOP + 1) as f32 + kind.rest_half_height();
            assert!((item.pos.y - floor).abs() < 0.02, "{:?} y = {}", kind, item.pos.y);
        }
    }

    #[test]
    fn items_fall_into_a_dug_hole_but_not_through_the_stone() {
        let mut w = world();
        for y in 1..=5 {
            w.break_block(10, y, 10);
        }
        let mut item = DroppedItem::at_rest(ItemKind::Stone, vec3(10.5, 8.0, 10.5));
        settle(&mut item, &w, 3.0);
        let floor = STONE_LAYERS as f32 + ItemKind::Stone.rest_half_height();
        assert!((item.pos.y - floor).abs() < 0.02, "y = {}", item.pos.y);
    }

    #[test]
    fn items_in_ungenerated_chunks_stay_frozen() {
        let w = world();
        let far = vec3(10_000.5, 8.0, 10.5);
        let mut item = DroppedItem::at_rest(ItemKind::Stick, far);
        settle(&mut item, &w, 1.0);
        assert_eq!(item.pos, far);
    }

    #[test]
    fn pickup_needs_delay_and_proximity() {
        let player = vec3(10.5, 6.0, 10.5);
        let mut item = DroppedItem::at_rest(ItemKind::Stick, vec3(10.9, 6.05, 10.5));
        assert!(item.can_pickup(player));
        item.age = 0.1;
        assert!(!item.can_pickup(player), "too fresh");
        item.age = 1.0;
        item.pos = vec3(13.0, 6.05, 10.5);
        assert!(!item.can_pickup(player), "too far");
        item.pos = vec3(10.5, 3.0, 10.5);
        assert!(!item.can_pickup(player), "far below the player");
    }

    #[test]
    fn items_in_the_one_block_hole_you_just_dug_can_be_picked_up() {
        // The player stands on the ground (y = 6) next to a freshly dug hole.
        let mut w = world();
        w.break_block(11, GROUND_TOP, 10);
        let player = vec3(10.5, 6.0, 10.5);
        let mut item = DroppedItem::at_rest(ItemKind::Pebble, vec3(11.5, 7.0, 10.5));
        settle(&mut item, &w, 2.0);
        assert!(item.pos.y < 5.2, "item should have fallen into the hole, y = {}", item.pos.y);
        assert!(item.can_pickup(player));
    }

    #[test]
    fn collect_nearby_fills_inventory_and_skips_far_items() {
        let player = vec3(10.5, 6.0, 10.5);
        let mut items = vec![
            DroppedItem::at_rest(ItemKind::Stick, vec3(10.8, 6.05, 10.5)),
            DroppedItem::at_rest(ItemKind::Stick, vec3(10.2, 6.05, 10.9)),
            DroppedItem::at_rest(ItemKind::Pebble, vec3(11.0, 6.07, 10.5)),
            DroppedItem::at_rest(ItemKind::Stone, vec3(20.0, 6.1, 20.0)),
        ];
        let mut inv = Inventory::new();
        let picked = collect_nearby(&mut items, &mut inv, player);
        assert_eq!(picked.len(), 3);
        assert_eq!(inv.count(ItemKind::Stick), 2);
        assert_eq!(inv.count(ItemKind::Pebble), 1);
        assert_eq!(inv.count(ItemKind::Stone), 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Stone);
    }

    #[test]
    fn a_full_inventory_leaves_the_item_on_the_ground() {
        let player = vec3(10.5, 6.0, 10.5);
        let mut inv = Inventory::new();
        inv.add(ItemKind::Stone, INVENTORY_SLOTS as u32 * MAX_STACK);
        let mut items = vec![DroppedItem::at_rest(ItemKind::Stick, vec3(10.6, 6.05, 10.5))];
        assert!(collect_nearby(&mut items, &mut inv, player).is_empty());
        assert_eq!(items.len(), 1);
        // A stone would still not fit, but freeing a slot lets the stick in.
        inv.slots[5] = None;
        assert_eq!(collect_nearby(&mut items, &mut inv, player).len(), 1);
        assert!(items.is_empty());
    }
}
