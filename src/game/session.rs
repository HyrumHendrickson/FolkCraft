//! Local multiplayer: the game-side half (`net.rs` is the transport).
//!
//! **One shared world, simulated by the host.** The host runs the ordinary `Game`: every creature, boss, projectile, fire and item
//! lives there, and every creature picks the *nearest player* (the host or any guest) as the one to chase, strike and shoot at. A guest
//! runs its own `Game` for its own body only (walking, mining, inventory, health, armour), and for everything shared it shows what the
//! host reports:
//!
//! - **creatures, arrows, projectiles, acid, tornadoes, falling stars, items on the ground**: a `Snapshot` 20 times a second; the guest
//!   keeps stand-in `Animal`s (never run, only drawn and aimed at) and eases them towards the reported places;
//! - **blocks, fires, cooking, pots, doors**: `WorldOp`s, replayed on every machine (`World::net_log`, `World::apply_op`);
//! - **blows a guest lands**: sent to the host (`ToHost::Hurt`) by creature number; the host does the damage, the drops, the kill counts
//!   and whatever a boss's death does, exactly as for its own blows;
//! - **damage to a guest**: the host works out who is hit (a bear's claws, a boss's smash, an arrow, a burst) and tells that guest
//!   (`ToGuest::Hurt`), whose own armour then applies;
//! - **items on the ground** belong to the host; a guest asks for one (`Pickup`) and is granted it if nobody was quicker;
//! - **day and night** are changed by whoever turns them, in the dimension they are in, behind that player's blackout;
//! - **dimensions**: every player travels on their own (the well, the cyclone, a fall). The host keeps the creatures of every dimension
//!   a player is in alive: its own is in the fields of `Game`, and the others are swapped in one at a time (`simulate_in`, `Sim`);
//! - **the story** (what is slain, summoned or rested, the rainbow) is shared as `WorldOp::Progress` whenever it changes;
//! - **what only the host's creatures can do** (the horn, the pipes' call, a Holly Rod's bolt, the king's spirits) is asked of the host
//!   as an `Intent`, which it carries out where the guest stands.
//!
//! The host keeps what each guest carries, by name, in `<world>.players`, and gives it back when that name joins again.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::*;
use crate::net::*;

/// How often (seconds) the host sends each guest a snapshot and each guest sends its state: 20 times a second.
const PUBLISH_INTERVAL: f32 = 0.05;
/// Creatures beyond this many blocks from a guest are not reported to it (bosses always are).
const NET_ANIMAL_RANGE: f32 = 160.0;
/// Items on the ground beyond this many blocks from a guest are not reported to it.
const NET_ITEM_RANGE: f32 = 100.0;
/// How many guests a host takes.
pub const MAX_GUESTS: usize = 7;
/// How fast a guest's stand-ins ease towards the reported place (per second; higher is snappier).
const SMOOTHING: f32 = 18.0;
/// A reported place further than this from the stand-in is jumped to, not eased to.
const SNAP_DISTANCE: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Solo,
    Host,
    Guest,
}

/// A guest, as the host knows it.
pub struct GuestSlot {
    pub id: u32,
    pub name: String,
    pub link: Link,
    pub state: PlayerState,
    pub arrows: Vec<NetArrow>,
}

/// Another player, as drawn: where they are shown (eased towards `state.pos`) and how far they have walked (for the swing of the legs).
#[derive(Clone, Debug)]
pub struct RemotePlayer {
    pub id: u32,
    pub name: String,
    pub state: PlayerState,
    pub shown: Vec3,
    pub walked: f32,
}

/// Somebody the creatures can notice.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Which guest (`Session::guests`), or `None` for the host's own player.
    pub guest: Option<usize>,
    pub cloaked: bool,
    pub holly_rod: bool,
    pub rabbit_foot: bool,
    /// Where the player is looking (a falling star lands where it can be seen).
    pub look: Vec3,
}

/// The index of the target nearest to `at`.
pub fn nearest_target(targets: &[Target], at: Vec3) -> usize {
    let d = |t: &Target| vec2(t.pos.x - at.x, t.pos.z - at.z).length_squared() + (t.pos.y - at.y).powi(2) * 0.01;
    (0..targets.len()).min_by(|&a, &b| d(&targets[a]).partial_cmp(&d(&targets[b])).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0)
}

pub struct Session {
    pub role: Role,
    /// The name this player goes by, and the host's.
    pub my_name: String,
    pub host_name: String,
    /// 0 for the host.
    pub my_id: u32,
    // Host.
    listener: Option<Listener>,
    beacon: Option<Beacon>,
    pending: Vec<(Link, Instant)>,
    pub guests: Vec<GuestSlot>,
    next_player: u32,
    next_entity: u32,
    // Guest.
    link: Option<Link>,
    /// Where the host last was (a guest who dies in another dimension wakes beside them).
    pub host_pos: Vec3,
    placed: bool,
    requested: HashMap<u32, f32>,
    out_items: Vec<(ItemKind, Vec3, Vec3)>,
    out_night: Option<(Dimension, bool)>,
    out_shot: Option<Vec3>,
    targets: HashMap<u32, Vec3>,
    tornado_burn: f32,
    // Both.
    pub remotes: Vec<RemotePlayer>,
    /// Arrows other players have in the air, for drawing.
    pub remote_arrows: Vec<Arrow>,
    publish_timer: f32,
    last_dim: Dimension,
    /// Inside the replay of somebody else's day or night change (which must not be announced again).
    applying: bool,
    /// What the player who struck the creature now dying was wearing (set while the host handles a guest's blow).
    credit_worn: Option<Option<ItemKind>>,
    /// Why the session ended, to be shown on the menu.
    pub notice: Option<String>,
    /// Seconds since the last state was heard from the host (a guest).
    silence: f32,
    /// The dimension whose creatures are swapped into `Game` just now while the host's own player is in another (see `simulate_in`).
    away_sim: Option<Dimension>,
    /// Things a guest asks the host's creatures to do, sent at the end of the frame.
    out_intents: Vec<Intent>,
    /// What each guest carried when it was last here, by name, as `profile_text` writes it (a host).
    profiles: HashMap<String, String>,
    /// A guest: whether the host has said what this player carried before (until then nothing is reported back, or an empty pack
    /// would replace the real one), when it was last reported, and what was reported.
    profile_synced: bool,
    profile_timer: f32,
    last_profile: String,
}

impl Session {
    pub fn solo() -> Session {
        Session {
            role: Role::Solo,
            my_name: String::new(),
            host_name: String::new(),
            my_id: 0,
            listener: None,
            beacon: None,
            pending: Vec::new(),
            guests: Vec::new(),
            next_player: 1,
            next_entity: 1,
            link: None,
            host_pos: Vec3::ZERO,
            placed: false,
            requested: HashMap::new(),
            out_items: Vec::new(),
            out_night: None,
            out_shot: None,
            targets: HashMap::new(),
            tornado_burn: 0.0,
            remotes: Vec::new(),
            remote_arrows: Vec::new(),
            publish_timer: 0.0,
            last_dim: Dimension::Overworld,
            applying: false,
            credit_worn: None,
            notice: None,
            silence: 0.0,
            away_sim: None,
            out_intents: Vec::new(),
            profiles: HashMap::new(),
            profile_synced: false,
            profile_timer: 0.0,
            last_profile: String::new(),
        }
    }

    /// The port the host listens on (0 when not hosting).
    pub fn port(&self) -> u16 {
        self.listener.as_ref().map_or(0, |l| l.port)
    }
}

fn dim_to_wire(d: Dimension) -> u8 {
    match d {
        Dimension::Overworld => 0,
        Dimension::Horizen => 1,
        Dimension::Skylands => 2,
    }
}

fn dim_from_wire(n: u8) -> Option<Dimension> {
    match n {
        0 => Some(Dimension::Overworld),
        1 => Some(Dimension::Horizen),
        2 => Some(Dimension::Skylands),
        _ => None,
    }
}

fn clean_name(name: &str, fallback: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(16).collect();
    let n = n.trim().to_string();
    if n.is_empty() { fallback.to_string() } else { n }
}

fn net_arrow(a: &Arrow) -> NetArrow {
    NetArrow { kind: a.kind, pos: a.pos, vel: a.vel, age: a.age }
}

fn arrow_from_net(a: &NetArrow) -> Arrow {
    Arrow { kind: a.kind, pos: a.pos, vel: a.vel, age: a.age }
}

fn projectile_kind_to_wire(k: ProjectileKind) -> u8 {
    match k {
        ProjectileKind::IceBall => 0,
        ProjectileKind::Fireball => 1,
        ProjectileKind::Rock => 2,
        ProjectileKind::DragonFire => 3,
    }
}

fn projectile_kind_from_wire(n: u8) -> Option<ProjectileKind> {
    [ProjectileKind::IceBall, ProjectileKind::Fireball, ProjectileKind::Rock, ProjectileKind::DragonFire].get(n as usize).copied()
}

fn burst_shape_to_wire(s: BurstShape) -> u8 {
    match s {
        BurstShape::Ice => 0,
        BurstShape::Fire => 1,
        BurstShape::Smash => 2,
        BurstShape::OrangutanSmash => 3,
        BurstShape::Rock => 4,
    }
}

fn burst_shape_from_wire(n: u8) -> Option<BurstShape> {
    [BurstShape::Ice, BurstShape::Fire, BurstShape::Smash, BurstShape::OrangutanSmash, BurstShape::Rock].get(n as usize).copied()
}

impl Game {
    // --- Who is who -----------------------------------------------------------------------------------------------------

    pub fn is_guest(&self) -> bool {
        self.net.role == Role::Guest
    }

    pub fn is_host(&self) -> bool {
        self.net.role == Role::Host
    }

    /// Whether this game is part of a session (as host or guest).
    pub fn in_session(&self) -> bool {
        self.net.role != Role::Solo
    }

    /// Everyone the creatures of the world now in `self.world` can notice: the host's player (unless the world is one swapped in for
    /// guests, see `simulate_in`), then each guest who is in this dimension.
    pub fn targets(&self) -> Vec<Target> {
        let here = dim_to_wire(self.world.dimension);
        let mut v = Vec::new();
        if self.net.away_sim.is_none() {
            v.push(Target {
                pos: self.player.pos,
                vel: self.player.vel,
                guest: None,
                cloaked: self.worn == Some(ItemKind::PeltCloak),
                holly_rod: self.holly_rod_in_hotbar(),
                rabbit_foot: self.rabbit_foot_in_hotbar(),
                look: self.player.look_dir(),
            });
        }
        for (i, g) in self.net.guests.iter().enumerate().filter(|(_, g)| g.state.dim == here) {
            let (yaw, pitch) = (g.state.yaw, g.state.pitch);
            v.push(Target {
                pos: g.state.pos,
                vel: g.state.vel,
                guest: Some(i),
                cloaked: g.state.flags & FLAG_CLOAKED != 0,
                holly_rod: g.state.flags & FLAG_HOLLY_ROD != 0,
                rabbit_foot: g.state.flags & FLAG_RABBIT_FOOT != 0,
                look: vec3(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()),
            });
        }
        v
    }

    /// Whether the host's own player is in another dimension than the one swapped in just now.
    pub fn host_is_away(&self) -> bool {
        self.net.away_sim.is_some()
    }

    /// Whether a guest is in `dim`.
    pub fn guest_in(&self, dim: Dimension) -> bool {
        self.net.guests.iter().any(|g| g.state.dim == dim_to_wire(dim))
    }

    /// Run `f` with the world and creatures of `dim` swapped in (when it is not the one the host's player is in): the host keeps the
    /// dimensions its guests are in alive this way. `None` if there is no such world to swap in.
    pub fn simulate_in<R>(&mut self, dim: Dimension, f: impl FnOnce(&mut Game) -> R) -> Option<R> {
        if self.world.dimension == dim {
            return Some(f(self));
        }
        let mut other = self.take_parked(dim)?;
        std::mem::swap(&mut self.world, &mut other.world);
        std::mem::swap(&mut self.items, &mut other.items);
        let mut sim = std::mem::take(&mut other.sim);
        self.swap_sim(&mut sim);
        self.net.away_sim = Some(dim);
        let out = f(self);
        // What happened to the story there (a boss slain) is shared like any other.
        self.world.note_progress();
        self.net.away_sim = None;
        self.swap_sim(&mut sim);
        other.sim = sim;
        std::mem::swap(&mut self.items, &mut other.items);
        std::mem::swap(&mut self.world, &mut other.world);
        self.park(other);
        Some(out)
    }

    /// The dimensions guests are in other than the host's own.
    fn guest_dimensions(&self) -> Vec<Dimension> {
        let mut dims = Vec::new();
        for g in &self.net.guests {
            if let Some(d) = dim_from_wire(g.state.dim) {
                if d != self.world.dimension && !dims.contains(&d) {
                    dims.push(d);
                }
            }
        }
        dims
    }

    /// Keep the world of each dimension a guest is in (and the host is not) alive: its creatures move, its fires burn, its bosses come
    /// and its items fall. Call once a frame, after the host's own dimension has been updated.
    pub fn update_remote_dimensions(&mut self, dt: f32) {
        for dim in self.guest_dimensions() {
            self.simulate_in(dim, |g| g.simulate_for_guests(dt));
        }
        // What nobody is in any more stands down: it is made again, whole, when somebody comes.
        let occupied: Vec<u8> = self.net.guests.iter().map(|g| g.state.dim).collect();
        for a in self.away.iter_mut().chain(self.shelf.iter_mut()) {
            if !occupied.contains(&dim_to_wire(a.world.dimension)) && (!a.sim.animals.is_empty() || !a.sim.projectiles.is_empty() || a.sim.storm_active) {
                a.sim = Sim::default();
            }
        }
    }

    /// One frame of the host's simulation for the world swapped in for its guests.
    fn simulate_for_guests(&mut self, dt: f32) {
        let spots: Vec<Vec3> = self.targets().iter().map(|t| t.pos).filter(|p| p.abs().max_element() < 1.0e6).collect();
        for pos in spots {
            self.world.load_around(pos, 6);
        }
        self.update_guest_idols();
        self.update_king_dungeon();
        self.update_animals(dt);
        self.update_item_physics(dt);
        self.burn_fires(dt);
        self.update_arrows(dt);
        self.update_storm(dt);
    }

    /// Move the items on the ground (not picking any up).
    pub fn update_item_physics(&mut self, dt: f32) {
        for item in &mut self.items {
            item.update(&self.world, dt);
        }
        self.limit_items();
    }

    /// Ask the host's creatures for something (a guest).
    pub(super) fn net_intent(&mut self, intent: Intent) {
        self.net.out_intents.push(intent);
    }

    /// Make sure the other two dimensions exist (untouched ones are made), so that every player's copy of the game holds all three and
    /// every change to any of them has somewhere to land.
    fn ensure_dimensions(&mut self) {
        let seed = self.info.seed;
        for d in [Dimension::Overworld, Dimension::Horizen, Dimension::Skylands] {
            if self.world.dimension != d && self.parked(d).is_none() {
                self.park(Away::fresh(d, seed));
            }
        }
        self.sync_world_flags();
    }

    /// Run `f` with the host's player standing (and moving) as `pos` and `vel`, then put them back. The spawning rules and the dungeon all
    /// look at "the player"; this makes them look at a guest instead.
    pub fn as_player_at<R>(&mut self, pos: Vec3, vel: Vec3, f: impl FnOnce(&mut Game) -> R) -> R {
        let saved = (self.player.pos, self.player.vel);
        self.player.pos = pos;
        self.player.vel = vel;
        let out = f(self);
        self.player.pos = saved.0;
        self.player.vel = saved.1;
        out
    }

    /// The state this player reports about itself.
    pub fn local_state(&self) -> PlayerState {
        let mut flags = 0;
        if self.holly_rod_in_hotbar() { flags |= FLAG_HOLLY_ROD; }
        if self.idol_in_hotbar() { flags |= FLAG_IDOL; }
        if self.rabbit_foot_in_hotbar() { flags |= FLAG_RABBIT_FOOT; }
        if self.worn == Some(ItemKind::PeltCloak) { flags |= FLAG_CLOAKED; }
        if self.attack_cooldown > 0.1 { flags |= FLAG_SWINGING; }
        PlayerState {
            pos: self.player.pos,
            vel: self.player.vel,
            yaw: self.player.yaw,
            pitch: self.player.pitch,
            health: self.health.min(255) as u8,
            held: opt_item_to_wire(self.held_item()),
            worn: opt_item_to_wire(self.worn),
            flags,
            dim: dim_to_wire(self.world.dimension),
        }
    }

    /// Make every world this game holds write its changes to the multiplayer journal (or stop).
    pub fn sync_world_flags(&mut self) {
        let on = self.in_session();
        self.world.net_on = on;
        for away in self.away.iter_mut().chain(self.shelf.iter_mut()) {
            away.world.net_on = on;
        }
    }

    // --- Starting and ending a session ---------------------------------------------------------------------------------

    /// Open this game to the network: listen for guests and announce the game on the local network. Returns the port.
    pub fn start_hosting(&mut self, host_name: &str, port: u16) -> std::io::Result<u16> {
        let listener = Listener::bind(port)?;
        let port = listener.port;
        let name = clean_name(host_name, "Host");
        self.net.beacon = Some(Beacon::start(name.clone(), self.info.name.clone(), port));
        self.net.listener = Some(listener);
        self.net.role = Role::Host;
        self.net.my_name = name.clone();
        self.net.host_name = name;
        self.net.my_id = 0;
        self.ensure_dimensions();
        self.net.profiles = std::fs::read_to_string(self.profiles_path()).map(|t| parse_profiles(&t)).unwrap_or_default();
        Ok(port)
    }

    /// A game joined as a guest, from what the host sent: the world as it stands now, with this player in it empty-handed.
    /// Where the host keeps what its guests carry: beside the world's own file.
    fn profiles_path(&self) -> std::path::PathBuf {
        self.info.path.with_extension("players")
    }

    /// Write what the guests carry (host only, with the world).
    pub(super) fn save_profiles(&self) {
        if !self.is_host() || self.info.path.as_os_str().is_empty() {
            return;
        }
        if let Err(e) = std::fs::write(self.profiles_path(), profiles_text(&self.net.profiles)) {
            eprintln!("Failed to save the guests' packs: {e}");
        }
    }

    /// What this player carries, in the host's save format: health, what is worn and each occupied slot.
    pub fn profile_text(&self) -> String {
        let mut out = format!("health {}\n", self.health.max(1));
        if let Some(w) = self.worn {
            out.push_str(&format!("worn {}\n", w.id()));
        }
        for (i, slot) in self.inventory.slots.iter().enumerate() {
            if let Some(st) = slot {
                out.push_str(&format!("inv {i} {} {}\n", st.kind.id(), st.count));
            }
        }
        out
    }

    /// Take back what this player carried before (see `profile_text`). Unreadable lines are ignored.
    pub fn apply_profile(&mut self, text: &str) {
        let mut inv = Inventory::new();
        let mut worn = None;
        let mut health = MAX_HEALTH;
        let lines: Vec<Vec<&str>> = text.lines().map(|l| l.split_whitespace().collect()).collect();
        for l in &lines {
            match l.as_slice() {
                ["health", n] => health = n.parse().unwrap_or(MAX_HEALTH).clamp(1, MAX_HEALTH),
                ["worn", id] => worn = ItemKind::from_id(id),
                _ => {}
            }
        }
        if worn.map_or(false, |k| k.is_backpack()) {
            inv.add_pack_rows();
        }
        for l in &lines {
            if let ["inv", slot, id, count] = l.as_slice() {
                if let (Ok(slot), Some(kind), Ok(count)) = (slot.parse::<usize>(), ItemKind::from_id(id), count.parse::<u32>()) {
                    if slot < inv.slots.len() && count > 0 {
                        inv.slots[slot] = Some(crate::item::ItemStack { kind, count: count.min(MAX_STACK) });
                    }
                }
            }
        }
        self.inventory = inv;
        self.worn = worn;
        self.health = health;
    }

    pub fn new_guest(id: u32, my_name: &str, host_name: String, save: &str, link: Link) -> Game {
        let name = save.lines().next().unwrap_or("World").to_string();
        let seed = save.lines().nth(1).and_then(|l| l.trim().parse::<i64>().ok()).unwrap_or(0);
        let info = WorldInfo { name: format!("{host_name}'s {name}"), seed, path: std::path::PathBuf::new() };
        let mut g = Game::from_save(info, parse_save(save), clock_seed());
        g.inventory = Inventory::new();
        g.worn = None;
        g.health = MAX_HEALTH;
        g.items.clear();
        g.animals.clear();
        g.net.role = Role::Guest;
        g.net.my_id = id;
        g.net.my_name = clean_name(my_name, "Guest");
        g.net.host_name = host_name;
        g.net.link = Some(link);
        g.net.host_pos = g.player.pos;
        g.net.last_dim = g.world.dimension;
        g.sync_world_flags();
        g
    }

    /// Act on messages from the host that arrived with the welcome, before this game existed to hear them.
    pub fn replay_messages(&mut self, messages: Vec<Vec<u8>>) {
        for bytes in messages {
            if let Some(msg) = ToGuest::decode(&bytes) {
                self.guest_hears(msg);
            }
        }
    }

    /// Tell everyone the host is leaving, and close the doors.
    pub fn net_shutdown(&mut self) {
        match self.net.role {
            Role::Host => {
                for g in &self.net.guests {
                    g.link.send(ToGuest::Bye("The host closed the game.".into()).encode());
                }
                // The writer threads need a moment to put the goodbye on the wire before the links are closed.
                if !self.net.guests.is_empty() {
                    std::thread::sleep(std::time::Duration::from_millis(60));
                }
                // The guests' last words (what they carry) are in their links still.
                let ids: Vec<u32> = self.net.guests.iter().map(|g| g.id).collect();
                for id in ids {
                    self.hear_guest(id);
                }
                self.save_profiles();
                self.net.guests.clear();
                self.net.pending.clear();
                self.net.listener = None;
                self.net.beacon = None;
            }
            Role::Guest => {
                if let Some(link) = self.net.link.take() {
                    if self.net.profile_synced {
                        link.send(ToHost::Profile(self.profile_text()).encode());
                    }
                    link.send(ToHost::Bye.encode());
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
            }
            Role::Solo => {}
        }
        self.net.role = Role::Solo;
        self.sync_world_flags();
    }

    // --- Each frame ---------------------------------------------------------------------------------------------------

    /// Take in what the network has to say. Call at the start of a frame.
    pub fn net_before_frame(&mut self, dt: f32) {
        match self.net.role {
            Role::Solo => {}
            Role::Host => self.host_pump(dt),
            Role::Guest => self.guest_pump(dt),
        }
        self.ease_remotes(dt);
    }

    /// Say what happened this frame. Call at the end of a frame.
    pub fn net_after_frame(&mut self, dt: f32) {
        match self.net.role {
            Role::Solo => {}
            Role::Host => self.host_publish(dt),
            Role::Guest => self.guest_publish(dt),
        }
    }

    fn ease_remotes(&mut self, dt: f32) {
        let k = 1.0 - (-SMOOTHING * dt).exp();
        for r in &mut self.net.remotes {
            let step = r.state.pos - r.shown;
            if step.length() > SNAP_DISTANCE {
                r.shown = r.state.pos;
            } else {
                r.shown += step * k;
            }
            r.walked += vec2(r.state.vel.x, r.state.vel.z).length() * dt;
        }
        // The guests' own arrows are shown flying on between reports.
        for a in &mut self.net.remote_arrows {
            a.pos += a.vel * dt;
            a.vel.y -= ARROW_GRAVITY * dt;
        }
    }

    /// The remote players, rebuilt from the latest states (keeping where each is shown).
    fn set_remotes(&mut self, players: Vec<PlayerInfo>) {
        let mut old: HashMap<u32, RemotePlayer> = std::mem::take(&mut self.net.remotes).into_iter().map(|r| (r.id, r)).collect();
        self.net.remotes = players
            .into_iter()
            .filter(|p| p.id != self.net.my_id)
            .map(|p| match old.remove(&p.id) {
                Some(mut r) => {
                    r.name = p.name;
                    r.state = p.state;
                    r
                }
                None => RemotePlayer { id: p.id, name: p.name, shown: p.state.pos, walked: 0.0, state: p.state },
            })
            .collect();
    }

    // --- The host: guests arriving, leaving and speaking ------------------------------------------------------------------

    fn broadcast(&self, msg: &ToGuest, except: Option<u32>) {
        let bytes = msg.encode();
        for g in &self.net.guests {
            if Some(g.id) != except {
                g.link.send(bytes.clone());
            }
        }
    }

    /// Tell every guest something, and show it here too.
    pub fn announce(&mut self, text: &str) {
        self.toasts.push(text.to_string());
        if self.is_host() {
            self.broadcast(&ToGuest::Toast(text.to_string()), None);
        }
    }

    fn host_pump(&mut self, dt: f32) {
        // Arrivals: a connection says hello before it is a guest.
        if let Some(listener) = &self.net.listener {
            for link in listener.accept() {
                self.net.pending.push((link, Instant::now()));
            }
        }
        let mut waiting = std::mem::take(&mut self.net.pending);
        waiting.retain(|(link, since)| link.alive() && since.elapsed().as_secs() < 10);
        for (link, since) in waiting {
            let mut hello = None;
            for bytes in link.poll() {
                if let Some(ToHost::Hello { protocol, name }) = ToHost::decode(&bytes) {
                    hello = Some((protocol, name));
                    break;
                }
            }
            match hello {
                Some((protocol, name)) => self.admit(link, protocol, name),
                None => self.net.pending.push((link, since)),
            }
        }

        // Guests speaking.
        let mut gone = Vec::new();
        let ids: Vec<u32> = self.net.guests.iter().map(|g| g.id).collect();
        for id in ids {
            if self.hear_guest(id) {
                gone.push((id, "left"));
            } else if self.net.guests.iter().find(|g| g.id == id).map_or(false, |g| !g.link.alive()) {
                gone.push((id, "left"));
            }
        }
        for (id, why) in gone {
            self.drop_guest(id, why);
        }
        let _ = dt;
        // The land round each guest in the host's own dimension stays alive, nearest chunks first, a few at a time so that a far-off
        // guest cannot stall the host (guests in other dimensions are looked after by `simulate_for_guests`).
        let here = dim_to_wire(self.world.dimension);
        let spots: Vec<Vec3> = self.net.guests.iter().filter(|g| g.state.dim == here).map(|g| g.state.pos).filter(|p| p.abs().max_element() < 1.0e6).collect();
        for pos in spots {
            self.world.load_around(pos, 6);
        }
        // Who is where, as far as the host's own screen goes: the guests who are in the same dimension.
        let players: Vec<PlayerInfo> = self.net.guests.iter().filter(|g| g.state.dim == here).map(|g| PlayerInfo { id: g.id, name: g.name.clone(), state: g.state.clone() }).collect();
        self.set_remotes(players);
        let arrows: Vec<Arrow> = self.net.guests.iter().filter(|g| g.state.dim == here).flat_map(|g| g.arrows.iter().map(arrow_from_net)).collect();
        self.net.remote_arrows = arrows;
    }

    /// Act on everything guest `id` has said since last asked. Returns true if it said goodbye (or made no sense).
    fn hear_guest(&mut self, id: u32) -> bool {
        let Some(g) = self.net.guests.iter().find(|g| g.id == id) else { return false };
        let mut inbox = Vec::new();
        let mut garbled = false;
        for bytes in g.link.poll() {
            match ToHost::decode(&bytes) {
                Some(msg) => inbox.push(msg),
                None => {
                    garbled = true;
                    break;
                }
            }
        }
        for msg in inbox {
            if self.host_hears(id, msg) {
                return true;
            }
        }
        garbled
    }

    fn admit(&mut self, link: Link, protocol: u32, name: String) {
        if protocol != PROTOCOL {
            link.send(ToGuest::Refused(format!("This game uses version {PROTOCOL} of multiplayer; yours uses {protocol}. Both players need the same version of Folk Craft.")).encode());
            std::thread::sleep(std::time::Duration::from_millis(30));
            return;
        }
        if self.net.guests.len() >= MAX_GUESTS {
            link.send(ToGuest::Refused("This game is full.".into()).encode());
            std::thread::sleep(std::time::Duration::from_millis(30));
            return;
        }
        let taken: Vec<String> = self.net.guests.iter().map(|g| g.name.clone()).chain(std::iter::once(self.net.host_name.clone())).collect();
        let mut name = clean_name(&name, "Guest");
        if taken.contains(&name) {
            let base = name.clone();
            name = (2..).map(|n| format!("{base} {n}")).find(|c| !taken.contains(c)).unwrap_or(base);
        }
        let id = self.net.next_player;
        self.net.next_player += 1;
        // The guest gets the world as it is at this moment; everything after this arrives as ops and snapshots.
        link.send(ToGuest::Welcome { protocol: PROTOCOL, id, host_name: self.net.host_name.clone(), save: self.save_text() }.encode());
        // What this name carried last time, if anything.
        link.send(ToGuest::Profile(self.net.profiles.get(&name).cloned().unwrap_or_default()).encode());
        let mut state = self.local_state();
        state.vel = Vec3::ZERO;
        self.net.guests.push(GuestSlot { id, name: name.clone(), link, state, arrows: Vec::new() });
        self.announce(&format!("{name} joined the game."));
    }

    fn drop_guest(&mut self, id: u32, why: &str) {
        let Some(i) = self.net.guests.iter().position(|g| g.id == id) else { return };
        let g = self.net.guests.remove(i);
        let _ = g.link.send(ToGuest::Bye("You were disconnected.".into()).encode());
        self.save_profiles();
        self.announce(&format!("{} {why}.", g.name));
    }

    /// Act on one message from guest `id`. Returns true if the guest said goodbye.
    fn host_hears(&mut self, id: u32, msg: ToHost) -> bool {
        let Some(gi) = self.net.guests.iter().position(|g| g.id == id) else { return false };
        match msg {
            ToHost::Hello { .. } => {}
            ToHost::Bye => return true,
            ToHost::State { mut state, arrows } => {
                state.health = state.health.min(MAX_HEALTH as u8 + MOONLIGHT_MAX_BONUS as u8);
                self.net.guests[gi].state = state;
                self.net.guests[gi].arrows = arrows.into_iter().take(16).collect();
            }
            ToHost::Ops { dim, ops } => {
                if let Some(d) = dim_from_wire(dim) {
                    self.apply_ops(d, &ops);
                    // Everyone else must see the same change.
                    self.broadcast(&ToGuest::Ops { dim, ops }, Some(id));
                }
            }
            ToHost::Night { dim, night } => {
                if let Some(d) = dim_from_wire(dim) {
                    self.apply_night_to(d, night);
                    self.broadcast(&ToGuest::Night { dim, night }, Some(id));
                }
            }
            ToHost::Profile(text) => {
                let name = self.net.guests[gi].name.clone();
                self.net.profiles.insert(name, text);
            }
            // Everything else acts on the creatures and items of the dimension the guest is in.
            other => {
                if let Some(dim) = dim_from_wire(self.net.guests[gi].state.dim) {
                    self.simulate_in(dim, |g| g.host_acts(gi, other));
                }
            }
        }
        false
    }

    /// Carry out, in the dimension swapped in for it, what guest `gi` has asked: its blows, the things it dropped, the items it wants,
    /// the shots Nessie hears, and what only the host's creatures can do.
    fn host_acts(&mut self, gi: usize, msg: ToHost) {
        let (spot, vel) = (self.net.guests[gi].state.pos, self.net.guests[gi].state.vel);
        match msg {
            ToHost::Hurt { hits, from, worn } => self.host_hurt(gi, hits, from, opt_item_from_wire(worn)),
            ToHost::SpawnItems(items) => {
                for (kind, pos, vel) in items.into_iter().take(40) {
                    self.items.push(DroppedItem { net: 0, kind, pos, vel, age: 0.0 });
                }
            }
            ToHost::Pickup(ids) => {
                for want in ids.into_iter().take(40) {
                    let Some(at) = self.items.iter().position(|i| i.net == want && i.age >= PICKUP_DELAY * 0.5 && (i.pos - spot).length() <= 8.0) else { continue };
                    let item = self.items.swap_remove(at);
                    self.net.guests[gi].link.send(ToGuest::Grant { kind: item.kind }.encode());
                }
            }
            ToHost::Shot(pos) => {
                self.as_player_at(pos, Vec3::ZERO, |g| g.alarm_nessie());
            }
            ToHost::Intent(intent) => match intent {
                Intent::Horn => self.as_player_at(spot, vel, |g| {
                    if g.in_stonehenge() && !g.hunt_alive() {
                        g.summon_hunt();
                    }
                }),
                Intent::Pipes => self.as_player_at(spot, vel, |g| g.pipes_call_creatures()),
                Intent::Lightning(strike) => {
                    if self.storm_active && (strike - spot).length() < 250.0 {
                        let struck = self.as_player_at(spot, vel, |g| g.rod_lightning(strike));
                        let _ = struck;
                    }
                }
                Intent::KingGold(offerings) => {
                    let room = MAX_SPIRIT_ARCHERS.saturating_sub(self.spirit_count()) as u32;
                    let n = offerings.min(room);
                    self.as_player_at(spot, vel, |g| g.send_spirits(n));
                }
            },
            _ => {}
        }
    }

    /// Change the sky of `dim`: with a blackout if it is the world this game is in, at once if it is another.
    fn apply_night_to(&mut self, dim: Dimension, night: bool) {
        if dim == Dimension::Horizen {
            return;
        }
        if self.world.dimension == dim {
            let before = self.net.applying;
            self.net.applying = true;
            self.set_night(night);
            self.net.applying = before;
        } else if let Some(w) = self.away.iter_mut().chain(self.shelf.iter_mut()).map(|a| &mut a.world).find(|w| w.dimension == dim) {
            w.set_night(night);
        }
    }

    /// A guest's blows: hurt the creatures it names, as if the host's own player had struck them (the kill counts and summonings happen
    /// where the guest stands, and the guest's cloak counts for the Great Bear).
    fn host_hurt(&mut self, gi: usize, hits: Vec<(u32, i32)>, from: Vec3, worn: Option<ItemKind>) {
        let mut list: Vec<(usize, i32)> = Vec::new();
        for (id, damage) in hits.into_iter().take(32) {
            if !(1..=400).contains(&damage) {
                continue;
            }
            if let Some(i) = self.animals.iter().position(|a| a.net == id && a.kind.is_huntable()) {
                if self.animals[i].kind != AnimalKind::SpiritArcher {
                    self.animals[i].marked = SPIRIT_MARK_SECONDS;
                }
                list.push((i, damage));
            }
        }
        if list.is_empty() {
            return;
        }
        let vel = self.net.guests[gi].state.vel;
        self.net.credit_worn = Some(worn);
        self.as_player_at(from, vel, |g| g.hurt_animals_from(&list, from));
        self.net.credit_worn = None;
    }

    /// What the player who landed the blow now being handled was wearing.
    pub(super) fn credited_worn(&self) -> Option<ItemKind> {
        self.net.credit_worn.unwrap_or(self.worn)
    }

    /// Tell guest `gi` it has been hurt. It works out its own armour, knockback and death.
    pub fn hurt_guest(&mut self, gi: usize, amount: u32, death: &str, from: Vec3, launch: f32) {
        if let Some(g) = self.net.guests.get(gi) {
            g.link.send(ToGuest::Hurt { amount, death: death.to_string(), from, launch }.encode());
        }
    }

    /// Hurt whoever `t` is (the host's player or a guest), throwing them back from `from` (`launch` blocks if it is a hard throw).
    pub fn hurt_target(&mut self, t: &Target, amount: u32, death: &str, from: Vec3, launch: f32) {
        match t.guest {
            None => {
                if !self.hurt_player(amount, death) {
                    if launch > 0.0 {
                        let away = self.player.pos - from;
                        self.player.launch(away, launch);
                    } else {
                        self.knock_player(from);
                    }
                }
            }
            Some(gi) => self.hurt_guest(gi, amount, death, from, launch),
        }
    }

    /// Replay changes made elsewhere on the world of `dim` (the one being played in, or a parked one).
    pub fn apply_ops(&mut self, dim: Dimension, ops: &[WorldOp]) {
        let world = if self.world.dimension == dim {
            Some(&mut self.world)
        } else {
            self.away.iter_mut().chain(self.shelf.iter_mut()).map(|a| &mut a.world).find(|w| w.dimension == dim)
        };
        if let Some(w) = world {
            for op in ops {
                w.apply_op(op);
            }
        }
    }

    /// Collect the changes every world has journalled since the last call, by dimension.
    fn drain_ops(&mut self) -> Vec<(Dimension, Vec<WorldOp>)> {
        let mut out = Vec::new();
        let log = self.world.take_net_log();
        if !log.is_empty() {
            out.push((self.world.dimension, log));
        }
        for a in self.away.iter_mut().chain(self.shelf.iter_mut()) {
            let log = a.world.take_net_log();
            if !log.is_empty() {
                out.push((a.world.dimension, log));
            }
        }
        out
    }

    // --- The host: what it says every tick ------------------------------------------------------------------------------

    fn host_publish(&mut self, dt: f32) {
        // Changes to the story and to the world, as they happened.
        self.world.note_progress();
        for (dim, ops) in self.drain_ops() {
            for chunk in ops.chunks(400) {
                self.broadcast(&ToGuest::Ops { dim: dim_to_wire(dim), ops: chunk.to_vec() }, None);
            }
        }
        self.net.publish_timer += dt;
        if self.net.publish_timer < PUBLISH_INTERVAL {
            return;
        }
        self.net.publish_timer = 0.0;
        // Each guest is told about the dimension it is in, with that dimension's creatures swapped in.
        let mut dims: Vec<u8> = self.net.guests.iter().map(|g| g.state.dim).collect();
        dims.sort_unstable();
        dims.dedup();
        for d in dims {
            let Some(dim) = dim_from_wire(d) else { continue };
            self.simulate_in(dim, |g| {
                g.number_things();
                for gi in 0..g.net.guests.len() {
                    if g.net.guests[gi].state.dim != d || g.net.guests[gi].link.backlog() > BUSY_BYTES {
                        continue;
                    }
                    let snapshot = g.snapshot_for(gi);
                    g.net.guests[gi].link.send(ToGuest::Snapshot(Box::new(snapshot)).encode());
                }
            });
        }
    }

    /// Give every creature and item the host has not yet reported a number of its own.
    fn number_things(&mut self) {
        let mut seen = HashSet::new();
        for a in &mut self.animals {
            if a.net == 0 || !seen.insert(a.net) {
                a.net = self.net.next_entity;
                self.net.next_entity += 1;
                seen.insert(a.net);
            }
        }
        let mut seen = HashSet::new();
        for i in &mut self.items {
            if i.net == 0 || !seen.insert(i.net) {
                i.net = self.net.next_entity;
                self.net.next_entity += 1;
                seen.insert(i.net);
            }
        }
    }

    fn snapshot_for(&self, gi: usize) -> Snapshot {
        let guest = &self.net.guests[gi];
        let at = guest.state.pos;
        let near = |p: Vec3, range: f32| vec2(p.x - at.x, p.z - at.z).length() <= range;
        // Only the players in the same dimension are seen.
        let mut players = Vec::new();
        if !self.host_is_away() {
            players.push(PlayerInfo { id: 0, name: self.net.host_name.clone(), state: self.local_state() });
        }
        let same = |g: &&GuestSlot| g.id != guest.id && g.state.dim == guest.state.dim;
        players.extend(self.net.guests.iter().filter(same).map(|g| PlayerInfo { id: g.id, name: g.name.clone(), state: g.state.clone() }));
        let mut arrows: Vec<NetArrow> = self.arrows.iter().map(net_arrow).collect();
        arrows.extend(self.net.guests.iter().filter(same).flat_map(|g| g.arrows.iter().cloned()));
        Snapshot {
            dim: dim_to_wire(self.world.dimension),
            players,
            animals: self.animals.iter().filter(|a| a.kind.is_boss() || near(a.pos, NET_ANIMAL_RANGE)).take(400).map(Animal::to_net).collect(),
            items: self
                .items
                .iter()
                .filter(|i| near(i.pos, NET_ITEM_RANGE))
                .take(MAX_DROPPED_ITEMS * 2)
                .map(|i| NetItem { id: i.net, kind: i.kind, pos: i.pos, vel: i.vel, age: i.age })
                .collect(),
            arrows,
            hostile_arrows: self.hostile_arrows.iter().map(net_arrow).collect(),
            projectiles: self.projectiles.iter().map(|p| NetProjectile { kind: projectile_kind_to_wire(p.kind), pos: p.pos, vel: p.vel, age: p.age }).collect(),
            bursts: self.bursts.iter().map(|b| NetBurst { pos: b.pos, shape: burst_shape_to_wire(b.shape), age: b.age }).collect(),
            acid: self.acid.iter().map(|c| (c.x, c.z, c.left)).collect(),
            tornadoes: self.tornadoes.iter().map(net_tornado).collect(),
            crater: self.crater_tornado.as_ref().map(net_tornado),
            stars: self.falling_stars.iter().map(|s| NetStar { from: s.from, to: s.to, age: s.age }).collect(),
            flags: NetFlags { storm_active: self.storm_active, hunt_total: self.hunt_total },
        }
    }

    // --- The guest ------------------------------------------------------------------------------------------------------

    fn guest_pump(&mut self, dt: f32) {
        self.net.silence += dt;
        let Some(link) = self.net.link.as_ref() else { return };
        let mut inbox = Vec::new();
        for bytes in link.poll() {
            match ToGuest::decode(&bytes) {
                Some(msg) => inbox.push(msg),
                None => {
                    self.guest_disconnect("The host sent something this game could not read.");
                    return;
                }
            }
        }
        let alive = link.alive();
        for msg in inbox {
            self.guest_hears(msg);
            if self.net.link.is_none() {
                return;
            }
        }
        if !alive {
            self.guest_disconnect("Lost the connection to the host.");
        } else if self.net.silence > 15.0 {
            self.guest_disconnect("The host stopped answering.");
        }
    }

    fn guest_disconnect(&mut self, why: &str) {
        if self.net.notice.is_none() {
            self.net.notice = Some(why.to_string());
        }
        self.net.link = None;
        self.net.role = Role::Solo;
        self.sync_world_flags();
    }

    fn guest_hears(&mut self, msg: ToGuest) {
        match msg {
            ToGuest::Welcome { .. } | ToGuest::Refused(_) => {}
            ToGuest::Bye(why) => self.guest_disconnect(&why),
            ToGuest::Snapshot(s) => {
                self.net.silence = 0.0;
                self.apply_snapshot(*s);
            }
            ToGuest::Ops { dim, ops } => {
                if let Some(d) = dim_from_wire(dim) {
                    self.apply_ops(d, &ops);
                }
            }
            ToGuest::Hurt { amount, death, from, launch } => {
                if !self.hurt_player(amount, &death) {
                    if launch > 0.0 {
                        let away = self.player.pos - from;
                        self.player.launch(away, launch);
                    } else {
                        self.knock_player(from);
                    }
                }
            }
            ToGuest::Night { dim, night } => {
                if let Some(d) = dim_from_wire(dim) {
                    self.apply_night_to(d, night);
                }
            }
            ToGuest::Profile(text) => {
                if !text.trim().is_empty() {
                    self.apply_profile(&text);
                }
                self.net.profile_synced = true;
            }
            ToGuest::Grant { kind } => {
                if self.inventory.add(kind, 1) == 0 {
                    self.toasts.push(format!("+1 {}", kind.name()));
                } else {
                    // No room after all: put it back where the guest stands.
                    self.net.out_items.push((kind, self.player.pos + vec3(0.0, 0.5, 0.0), Vec3::ZERO));
                    self.toasts.push("Your pack is full".to_string());
                }
            }
            ToGuest::Toast(text) => self.toasts.push(text),
            ToGuest::Victory => {
                self.victory = Some(VICTORY_TEXT_SECONDS);
                self.world.landmarks.game_won = true;
            }
        }
    }

    fn stand_beside(&mut self, pos: Vec3) {
        self.world.stream_chunks(pos);
        // The first free place on a ring round the host (not inside a tree or a wall); the host's own place if there is none.
        let half = vec3(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
        let start = self.net.my_id as f32 * 2.1;
        let free = (0..8).map(|i| start + i as f32 * std::f32::consts::TAU / 8.0).map(|a| pos + vec3(a.cos() * 1.6, 0.2, a.sin() * 1.6)).find(|&p| !self.world.box_blocked(p - half + vec3(0.0, 0.05, 0.0), p + half + vec3(0.0, PLAYER_HEIGHT, 0.0)));
        self.player.pos = free.unwrap_or(pos + vec3(0.0, 0.1, 0.0));
        self.player.vel = Vec3::ZERO;
        self.player.knock = Vec3::ZERO;
        self.world.stream_chunks(self.player.pos);
        self.world.rebuild_dirty(self.player.pos, usize::MAX);
        self.player.lift_out_of_blocks(&self.world);
    }

    fn apply_snapshot(&mut self, s: Snapshot) {
        // A report about the dimension this guest has just left (or not yet reached) is of no use here.
        if s.dim != dim_to_wire(self.world.dimension) {
            return;
        }
        if let Some(host) = s.players.iter().find(|p| p.id == 0) {
            self.net.host_pos = host.state.pos;
            if !self.net.placed {
                self.net.placed = true;
                self.stand_beside(host.state.pos);
            }
        }
        self.set_remotes(s.players);
        self.net.remote_arrows = s.arrows.iter().map(arrow_from_net).collect();

        // Creatures: keep the stand-ins already made, make new ones, drop the ones the host no longer reports.
        let mut old: HashMap<u32, Animal> = std::mem::take(&mut self.animals).into_iter().map(|a| (a.net, a)).collect();
        self.net.targets.clear();
        for n in &s.animals {
            let mut a = match old.remove(&n.id) {
                Some(mut a) if a.kind == n.kind => {
                    a.apply_net(n);
                    a
                }
                _ => Animal::from_net(n),
            };
            if (a.pos - n.pos).length() > SNAP_DISTANCE {
                a.pos = n.pos;
            }
            self.net.targets.insert(n.id, n.pos);
            self.animals.push(a);
        }
        // Items: the host's list, less the ones this guest has already asked for.
        self.net.requested.retain(|id, _| s.items.iter().any(|i| i.id == *id));
        self.items = s
            .items
            .iter()
            .filter(|i| !self.net.requested.contains_key(&i.id))
            .map(|i| DroppedItem { net: i.id, kind: i.kind, pos: i.pos, vel: i.vel, age: i.age })
            .collect();
        self.hostile_arrows = s.hostile_arrows.iter().map(arrow_from_net).collect();
        self.projectiles = s
            .projectiles
            .iter()
            .filter_map(|p| Some(Projectile { kind: projectile_kind_from_wire(p.kind)?, pos: p.pos, vel: p.vel, age: p.age }))
            .collect();
        self.bursts = s.bursts.iter().filter_map(|b| Some(ShownBurst { pos: b.pos, shape: burst_shape_from_wire(b.shape)?, age: b.age })).collect();
        self.acid = s.acid.iter().map(|&(x, z, left)| AcidCell { x, z, left }).collect();
        self.tornadoes = s.tornadoes.iter().map(tornado_from_net).collect();
        self.crater_tornado = s.crater.as_ref().map(tornado_from_net);
        self.falling_stars = s
            .stars
            .iter()
            .map(|st| {
                let mut star = FallingStar::new(st.from, st.to);
                star.age = st.age;
                star.pile_made = true;
                star
            })
            .collect();
        self.storm_active = s.flags.storm_active;
        self.hunt_total = s.flags.hunt_total;
    }

    /// Between snapshots: ease the stand-ins to where the host last put them, and let what flies keep flying.
    pub fn guest_smooth(&mut self, dt: f32) {
        let k = 1.0 - (-SMOOTHING * dt).exp();
        for a in &mut self.animals {
            if let Some(&target) = self.net.targets.get(&a.net) {
                a.pos += (target - a.pos) * k;
            }
            // The walking cycle runs on between reports.
            let speed = vec2(a.vel.x, a.vel.z).length().min(12.0);
            a.phase += speed * dt * 0.5;
        }
        for arrow in &mut self.hostile_arrows {
            arrow.pos += arrow.vel * dt;
        }
        for p in &mut self.projectiles {
            p.pos += p.vel * dt;
            p.vel.y -= p.kind.gravity() * dt;
        }
        for b in &mut self.bursts {
            b.age += dt;
        }
        for c in &mut self.acid {
            c.left -= dt;
        }
        for t in self.tornadoes.iter_mut().chain(self.crater_tornado.iter_mut()) {
            t.update(dt);
        }
        for star in &mut self.falling_stars {
            star.age += dt;
        }
        self.world.tick_timers(dt);
        self.world.drain_released();
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.guest_hazards(dt);
    }

    /// The hazards a guest suffers from by itself: fire tornadoes and acid water (the host reports where they are).
    fn guest_hazards(&mut self, dt: f32) {
        let p = self.player.pos;
        let hit = self.tornadoes.iter().chain(self.crater_tornado.iter()).find(|t| t.hits(p)).map(|t| t.pos);
        match hit {
            Some(source) => {
                self.net.tornado_burn += dt;
                if self.net.tornado_burn >= SKY_TORNADO_DAMAGE_INTERVAL {
                    self.net.tornado_burn = 0.0;
                    if !self.hurt_player(SKY_TORNADO_DAMAGE, "The fire tornado burns you") {
                        self.knock_player(source);
                    }
                }
            }
            None => self.net.tornado_burn = 0.0,
        }
        self.bursts.retain(|b| b.age < BURST_SHOW_SECONDS);
        self.acid.retain(|c| c.left > 0.0);
        self.update_acid_burn(dt);
    }

    /// Items within reach are asked for from the host, which grants each to whoever asked first.
    pub fn guest_collect(&mut self) {
        let spot = self.player.pos;
        let mut want = Vec::new();
        for i in &self.items {
            if i.net != 0 && !self.net.requested.contains_key(&i.net) && i.can_pickup(spot) && self.inventory.clone().add(i.kind, 1) == 0 {
                want.push(i.net);
            }
        }
        if want.is_empty() {
            return;
        }
        for id in &want {
            self.net.requested.insert(*id, 0.0);
        }
        self.items.retain(|i| !want.contains(&i.net));
        if let Some(link) = &self.net.link {
            link.send(ToHost::Pickup(want).encode());
        }
    }

    fn guest_publish(&mut self, dt: f32) {
        // Items this guest made (drops, mined goods, a thrown knife that landed) belong to the host's list: send them and take them off
        // the local one; they come back in the next snapshot with a number.
        let mut made = std::mem::take(&mut self.net.out_items);
        self.items.retain(|i| {
            if i.net == 0 {
                made.push((i.kind, i.pos, i.vel));
                false
            } else {
                true
            }
        });
        for t in self.net.requested.values_mut() {
            *t += dt;
        }
        self.net.requested.retain(|_, t| *t < 3.0);
        self.world.note_progress();
        let ops: Vec<(Dimension, Vec<WorldOp>)> = self.drain_ops();
        let Some(link) = self.net.link.as_ref() else { return };
        for (dim, ops) in ops {
            for chunk in ops.chunks(400) {
                link.send(ToHost::Ops { dim: dim_to_wire(dim), ops: chunk.to_vec() }.encode());
            }
        }
        if !made.is_empty() {
            link.send(ToHost::SpawnItems(made).encode());
        }
        if let Some((dim, night)) = self.net.out_night.take() {
            link.send(ToHost::Night { dim: dim_to_wire(dim), night }.encode());
        }
        for intent in std::mem::take(&mut self.net.out_intents) {
            link.send(ToHost::Intent(intent).encode());
        }
        if let Some(at) = self.net.out_shot.take() {
            link.send(ToHost::Shot(at).encode());
        }
        // What this player carries is kept by the host, a few seconds behind at most.
        self.net.profile_timer += dt;
        if self.net.profile_synced && self.net.profile_timer >= 3.0 {
            self.net.profile_timer = 0.0;
            let text = self.profile_text();
            if text != self.net.last_profile {
                if let Some(link) = self.net.link.as_ref() {
                    link.send(ToHost::Profile(text.clone()).encode());
                }
                self.net.last_profile = text;
            }
        }
        self.net.publish_timer += dt;
        if self.net.publish_timer >= PUBLISH_INTERVAL {
            self.net.publish_timer = 0.0;
            let arrows: Vec<NetArrow> = self.arrows.iter().take(16).map(net_arrow).collect();
            if let Some(link) = self.net.link.as_ref() {
                link.send(ToHost::State { state: self.local_state(), arrows }.encode());
            }
        }
    }

    /// A guest's blow on creatures (indices into its stand-ins): sent to the host, which does the damage.
    pub(super) fn guest_hurt(&mut self, hits: &[(usize, i32)], from: Vec3) {
        let named: Vec<(u32, i32)> = hits.iter().filter_map(|&(i, d)| self.animals.get(i).filter(|a| a.net != 0).map(|a| (a.net, d))).collect();
        if named.is_empty() {
            return;
        }
        // Show the blow at once: the stand-in's health drops (the host's next report corrects it).
        for &(i, d) in hits {
            if let Some(a) = self.animals.get_mut(i) {
                a.health = (a.health - d).max(1);
            }
        }
        let worn = opt_item_to_wire(self.worn);
        if let Some(link) = &self.net.link {
            link.send(ToHost::Hurt { hits: named, from, worn }.encode());
        }
    }

    /// A guest says the sky was turned (a crystal ball, a stew).
    pub(super) fn guest_says_night(&mut self, night: bool) {
        self.net.out_night = Some((self.world.dimension, night));
    }

    /// A guest says a bow or sling was shot here.
    pub(super) fn guest_says_shot(&mut self) {
        self.net.out_shot = Some(self.player.pos);
    }

    /// Whether the session is announcing a change of sky that came from elsewhere (so the local game does not announce it again).
    pub(super) fn applying_remote_night(&self) -> bool {
        self.net.applying
    }

    /// The host's own change of sky goes to every guest.
    pub(super) fn host_says_night(&mut self, night: bool) {
        self.broadcast(&ToGuest::Night { dim: dim_to_wire(self.world.dimension), night }, None);
    }

    /// Tell guests the game is won.
    pub(super) fn host_says_victory(&mut self) {
        self.broadcast(&ToGuest::Victory, None);
    }

    /// The idol of a guest in the open normal world calls the Unicorn as the host's own would.
    pub(super) fn guest_idol_bearer(&self) -> Option<(Vec3, Vec3)> {
        if !self.in_overworld() {
            return None;
        }
        self.net
            .guests
            .iter()
            .find(|g| g.state.dim == 0 && g.state.flags & FLAG_IDOL != 0 && !crate::terrain::in_jungle(self.info.seed, g.state.pos.x.floor() as i32, g.state.pos.z.floor() as i32))
            .map(|g| (g.state.pos, g.state.vel))
    }
}

fn net_tornado(t: &FireTornado) -> NetTornado {
    NetTornado { pos: t.pos, target: t.target, arrived: t.arrived, age: t.age, scale: t.scale }
}

fn tornado_from_net(t: &NetTornado) -> FireTornado {
    FireTornado { pos: t.pos, target: t.target, arrived: t.arrived, damage_timer: 0.0, age: t.age, scale: t.scale }
}

/// The file of what guests carry: a `guest <name>` line, then what `Game::profile_text` wrote, for each.
fn profiles_text(profiles: &HashMap<String, String>) -> String {
    let mut names: Vec<&String> = profiles.keys().collect();
    names.sort();
    names.iter().map(|n| format!("guest {n}\n{}", profiles[*n])).collect()
}

fn parse_profiles(text: &str) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("guest ") {
            current = Some(name.to_string());
            out.entry(name.to_string()).or_default();
        } else if let Some(name) = &current {
            if let Some(body) = out.get_mut(name) {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animal::BossEvent;
    use crate::landmarks::RainbowState;
    use crate::save::SaveData;
    use std::net::SocketAddr;
    use std::time::Duration;

    const DT: f32 = 0.05;

    struct Pair {
        host: Game,
        guest: Game,
    }

    fn pair() -> Pair {
        let info = WorldInfo { name: "Test".into(), seed: 11, path: std::path::PathBuf::new() };
        let mut host = Game::from_save(info, SaveData::new(), 7);
        host.spawn_timer = -1.0e9;
        let port = host.start_hosting("Hosty", 0).unwrap();
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);
        let handle = std::thread::spawn(move || crate::lobby::join(addr, "Gus"));
        for _ in 0..600 {
            host.net_before_frame(DT);
            host.net_after_frame(DT);
            if handle.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let w = handle.join().unwrap().expect("the guest is welcomed");
        let mut guest = Game::new_guest(w.id, "Gus", w.host_name, &w.save, w.link);
        guest.replay_messages(w.rest);
        let mut p = Pair { host, guest };
        p.run_until(200, |p| p.host.net.guests.len() == 1 && !p.guest.net.remotes.is_empty());
        p
    }

    impl Pair {
        fn step(&mut self) {
            let (h, g) = (&mut self.host, &mut self.guest);
            h.net_before_frame(DT);
            h.update_guest_idols();
            h.update_animals(DT);
            h.update_remote_dimensions(DT);
            h.update_items(DT);
            h.burn_fires(DT);
            h.update_arrows(DT);
            h.net_after_frame(DT);
            g.net_before_frame(DT);
            g.guest_smooth(DT);
            g.update_items(DT);
            g.update_arrows(DT);
            g.net_after_frame(DT);
            std::thread::sleep(Duration::from_millis(4));
        }

        fn run_until(&mut self, max: usize, done: impl Fn(&Pair) -> bool) -> bool {
            for _ in 0..max {
                self.step();
                if done(self) {
                    return true;
                }
            }
            done(self)
        }
    }

    fn free_spot(g: &mut Game, x: i32, z: i32) -> Vec3 {
        g.world.stream_chunks(vec3(x as f32, 6.0, z as f32));
        let y = g.world.ground_top(x, z) as f32 + 1.01;
        vec3(x as f32 + 0.5, y, z as f32 + 0.5)
    }

    #[test]
    fn a_guest_joins_and_the_two_see_each_other() {
        let p = pair();
        assert_eq!(p.host.net.remotes.len(), 1);
        assert_eq!(p.host.net.remotes[0].name, "Gus");
        assert_eq!(p.guest.net.remotes[0].name, "Hosty");
        assert_eq!(p.guest.net.my_name, "Gus");
        assert!(p.guest.is_guest() && p.host.is_host());
        assert!(p.guest.inventory.slots.iter().all(|s| s.is_none()), "a guest arrives empty-handed");
    }

    #[test]
    fn blocks_and_fires_made_on_either_side_appear_on_the_other() {
        let mut p = pair();
        let spot = free_spot(&mut p.guest, 40, 40);
        let (x, y, z) = (spot.x as i32, spot.y as i32, spot.z as i32);
        p.host.world.stream_chunks(spot);
        p.guest.world.set_block(x, y, z, Block::PlacedLog);
        assert!(p.run_until(200, |p| p.host.world.block_at(x, y, z) == Block::PlacedLog), "guest's log reaches the host");
        // The host lights it: the guest sees fire, and later charcoal when it burns out.
        assert!(p.host.world.ignite((x, y, z), 0.3));
        assert!(p.run_until(200, |p| p.guest.world.fires.contains_key(&(x, y, z)) && p.guest.world.block_at(x, y, z) == Block::Fire));
        assert!(p.run_until(400, |p| p.guest.world.block_at(x, y, z) == Block::Charcoal && !p.guest.world.fires.contains_key(&(x, y, z))), "burn-out reaches the guest");
        // The guest breaks it: the host loses it.
        p.guest.world.break_block(x, y, z);
        assert!(p.run_until(200, |p| p.host.world.block_at(x, y, z) != Block::Charcoal));
    }

    #[test]
    fn a_guest_sees_creatures_and_its_blows_hurt_them_on_the_host() {
        let mut p = pair();
        let spot = p.host.player.pos + vec3(6.0, 0.0, 0.0);
        let (x, z) = (spot.x as i32, spot.z as i32);
        assert!(p.host.spawn_animal(AnimalKind::Deer, x, z));
        assert!(p.run_until(200, |p| p.guest.animals.iter().any(|a| a.kind == AnimalKind::Deer)));
        let idx = p.guest.animals.iter().position(|a| a.kind == AnimalKind::Deer).unwrap();
        let id = p.guest.animals[idx].net;
        assert_ne!(id, 0);
        p.guest.hurt_animals(&[(idx, 1)]);
        let max = AnimalKind::Deer.max_health();
        assert!(p.run_until(200, |p| p.host.animals.iter().any(|a| a.net == id && a.health < max)), "the host took the damage");
        // A killing blow: the host drops the loot, the guest sees it on the ground and can pick it up.
        let idx = p.guest.animals.iter().position(|a| a.net == id).unwrap();
        p.guest.hurt_animals(&[(idx, 300)]);
        assert!(p.run_until(200, |p| !p.host.animals.iter().any(|a| a.net == id)));
        assert!(p.run_until(200, |p| !p.guest.animals.iter().any(|a| a.net == id)));
        assert!(!p.host.items.is_empty(), "loot on the host");
    }

    #[test]
    fn a_bear_goes_for_the_nearer_player_and_only_that_player_is_hurt() {
        let mut p = pair();
        // Put the guest far from the host and a bear on the guest.
        let far = free_spot(&mut p.host, 90, 0);
        p.guest.player.pos = far;
        assert!(p.run_until(300, |p| (p.host.net.guests[0].state.pos - far).length() < 1.0));
        let bear_at = far + vec3(1.0, 0.0, 0.0);
        let mut bear = Animal::new(AnimalKind::Bear, bear_at, &mut Rng::new(3));
        bear.provoked = true;
        p.host.animals.push(bear);
        let (host_hp, guest_hp) = (p.host.health, p.guest.health);
        assert!(p.run_until(400, |p| p.guest.health < guest_hp), "the guest is clawed");
        assert_eq!(p.host.health, host_hp, "the host, far away, is not");
        assert!(p.host.animals.iter().any(|a| a.kind == AnimalKind::Bear), "and the bear was alive and moving on the host");
    }

    #[test]
    fn a_boss_killed_by_a_guest_changes_the_story_for_everyone() {
        let mut p = pair();
        let at = free_spot(&mut p.host, 20, 20);
        let mut yeti = Animal::new(AnimalKind::Yeti, at, &mut Rng::new(5));
        yeti.health = 1;
        p.host.animals.push(yeti);
        p.guest.player.pos = at + vec3(3.0, 0.0, 0.0);
        assert!(p.run_until(300, |p| p.guest.animals.iter().any(|a| a.kind == AnimalKind::Yeti)));
        let idx = p.guest.animals.iter().position(|a| a.kind == AnimalKind::Yeti).unwrap();
        p.guest.hurt_animals(&[(idx, 50)]);
        assert!(p.run_until(300, |p| p.host.world.landmarks.yeti_slain && p.guest.world.landmarks.yeti_slain));
    }

    #[test]
    fn a_guest_crosses_alone_and_the_host_keeps_that_dimension_alive_for_it() {
        let mut p = pair();
        p.guest.enter_horizen();
        assert!(p.run_until(200, |p| p.host.net.guests[0].state.dim == 1), "the host learns where the guest is");
        assert!(!p.host.in_horizen(), "the host stays where it was");
        // A bat in the Horizen's creatures, which the host swaps in for the guest: the guest sees it, the host's own world has none.
        let at = p.guest.player.pos + vec3(3.0, 0.0, 0.0);
        p.host.simulate_in(Dimension::Horizen, |g| {
            g.world.stream_chunks(at);
            let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
            let y = g.world.ground_top(x, z) as f32 + 1.01;
            g.animals.push(Animal::new(AnimalKind::Newt, vec3(at.x, y, at.z), &mut Rng::new(2)));
        });
        assert!(p.run_until(300, |p| p.guest.animals.iter().any(|a| a.kind == AnimalKind::Newt)), "the guest sees the Horizen's creature");
        assert!(!p.host.animals.iter().any(|a| a.kind == AnimalKind::Newt));
        // And the guest's blow lands on it there.
        let idx = p.guest.animals.iter().position(|a| a.kind == AnimalKind::Newt).unwrap();
        let id = p.guest.animals[idx].net;
        p.guest.hurt_animals(&[(idx, 300)]);
        assert!(p.run_until(300, |p| !p.guest.animals.iter().any(|a| a.net == id)), "the newt dies in the dimension the host is not in");
        // The host walks in too and the two meet again.
        p.host.enter_horizen();
        assert!(p.run_until(200, |p| !p.host.net.remotes.is_empty() && !p.guest.net.remotes.is_empty()));
    }

    #[test]
    fn a_guest_in_another_dimension_is_hurt_by_that_dimensions_creatures_only() {
        let mut p = pair();
        p.guest.enter_horizen();
        assert!(p.run_until(200, |p| p.host.net.guests[0].state.dim == 1));
        let at = p.guest.player.pos;
        let hp = p.guest.health;
        p.host.simulate_in(Dimension::Horizen, |g| {
            g.world.stream_chunks(at);
            let mut bear = Animal::new(AnimalKind::Bear, at + vec3(0.5, 0.0, 0.0), &mut Rng::new(9));
            bear.provoked = true;
            g.animals.push(bear);
        });
        let host_hp = p.host.health;
        assert!(p.run_until(400, |p| p.guest.health < hp), "the guest is clawed in the Horizen");
        assert_eq!(p.host.health, host_hp);
    }

    #[test]
    fn the_horn_the_pipes_and_the_holly_rod_work_for_a_guest() {
        let mut p = pair();
        // The horn, blown in Stonehenge, brings the Hunt to the guest on the host.
        let (ax, az) = crate::stonehenge::arrival(p.host.info.seed);
        let spot = free_spot(&mut p.host, ax as i32, az as i32);
        p.guest.world.stream_chunks(spot);
        p.guest.player.pos = spot;
        p.guest.inventory.add(ItemKind::HornOfTheHunt, 1);
        p.guest.select_slot(0);
        assert!(p.run_until(300, |p| (p.host.net.guests[0].state.pos - spot).length() < 1.0));
        assert!(p.guest.blow_horn());
        assert!(p.run_until(300, |p| p.host.hunt_alive() && p.guest.hunt_alive()), "the Hunt rides out for the guest");
        // The pipes call birds on the host's side.
        p.host.animals.clear();
        p.guest.inventory.add(ItemKind::MusicalPipes, 1);
        p.guest.select_slot(1);
        p.host.world.night = false;
        let mut quail = Animal::new(AnimalKind::Quail, spot + vec3(10.0, 0.0, 0.0), &mut Rng::new(4));
        quail.pos.y = p.host.world.ground_top(spot.x as i32 + 10, spot.z as i32) as f32 + 1.01;
        p.host.animals.push(quail);
        assert!(p.guest.play_pipes());
        assert!(p.run_until(100, |p| p.host.animals.iter().any(|a| a.kind == AnimalKind::Quail && a.lure_target.is_some())), "the quail is called to the guest");
    }

    #[test]
    fn a_guests_rainbow_and_resting_ghost_become_everyones_story() {
        let mut p = pair();
        p.guest.world.landmarks.graveyard_rested = true;
        p.guest.world.landmarks.rainbow = RainbowState::Spent;
        assert!(p.run_until(200, |p| p.host.world.landmarks.graveyard_rested && p.host.world.landmarks.rainbow == RainbowState::Spent));
        // The host's own change reaches the guest too.
        p.host.world.landmarks.unicorn_slain = true;
        assert!(p.run_until(200, |p| p.guest.world.landmarks.unicorn_slain));
    }

    #[test]
    fn what_a_guest_carries_is_kept_by_the_host_for_next_time() {
        let dir = std::env::temp_dir().join(format!("folkcraft_players_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let info = WorldInfo { name: "Keep".into(), seed: 5, path: dir.join("Keep.world") };
        let mut host = Game::from_save(info.clone(), SaveData::new(), 7);
        let port = host.start_hosting("Hosty", 0).unwrap();
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);
        let join = |host: &mut Game, name: &'static str| {
            let handle = std::thread::spawn(move || crate::lobby::join(addr, name));
            for _ in 0..600 {
                host.net_before_frame(DT);
                host.net_after_frame(DT);
                if handle.is_finished() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let w = handle.join().unwrap().unwrap();
            let mut g = Game::new_guest(w.id, name, w.host_name, &w.save, w.link);
            g.replay_messages(w.rest);
            g
        };
        let mut guest = join(&mut host, "Gus");
        guest.inventory.add(ItemKind::Gold, 7);
        guest.inventory.add(ItemKind::StoneAxe, 1);
        guest.health = 4;
        for _ in 0..200 {
            host.net_before_frame(DT);
            host.net_after_frame(DT);
            guest.net_before_frame(DT);
            guest.net_after_frame(DT);
            std::thread::sleep(Duration::from_millis(4));
        }
        guest.net_shutdown();
        for _ in 0..100 {
            host.net_before_frame(DT);
            host.net_after_frame(DT);
            std::thread::sleep(Duration::from_millis(4));
        }
        host.save();
        assert!(dir.join("Keep.players").exists(), "the host wrote the guests' packs");
        host.net_shutdown();
        // A fresh session of the same world: the same name gets its things back, another name does not.
        let mut host2 = Game::from_save(info, SaveData::new(), 8);
        let port = host2.start_hosting("Hosty", 0).unwrap();
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);
        let handle = std::thread::spawn(move || crate::lobby::join(addr, "Gus"));
        for _ in 0..600 {
            host2.net_before_frame(DT);
            host2.net_after_frame(DT);
            if handle.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let w = handle.join().unwrap().unwrap();
        let mut again = Game::new_guest(w.id, "Gus", w.host_name, &w.save, w.link);
        again.replay_messages(w.rest);
        for _ in 0..200 {
            host2.net_before_frame(DT);
            host2.net_after_frame(DT);
            again.net_before_frame(DT);
            again.net_after_frame(DT);
            std::thread::sleep(Duration::from_millis(4));
            if again.inventory.count(ItemKind::Gold) == 7 {
                break;
            }
        }
        assert_eq!(again.inventory.count(ItemKind::Gold), 7);
        assert_eq!(again.inventory.count(ItemKind::StoneAxe), 1);
        assert_eq!(again.health, 4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn profiles_survive_their_file_format() {
        let mut m = HashMap::new();
        m.insert("Ana".to_string(), "health 7\ninv 0 gold 3\n".to_string());
        m.insert("Bo Two".to_string(), "health 10\n".to_string());
        assert_eq!(parse_profiles(&profiles_text(&m)), m);
    }

    #[test]
    fn items_are_granted_to_one_guest_and_nobody_gets_two() {
        let mut p = pair();
        let at = p.guest.player.pos;
        p.host.items.push(DroppedItem::at_rest(ItemKind::Gold, at + vec3(0.2, 0.2, 0.0)));
        assert!(p.run_until(300, |p| p.guest.inventory.count(ItemKind::Gold) == 1));
        p.run_until(40, |_| false);
        assert_eq!(p.guest.inventory.count(ItemKind::Gold), 1);
        assert!(p.host.items.iter().all(|i| i.kind != ItemKind::Gold));
        // What the guest drops goes into the host's list and comes back numbered.
        p.guest.items.push(DroppedItem::at_rest(ItemKind::Stick, at + vec3(5.0, 1.0, 0.0)));
        p.guest.net_after_frame(DT);
        assert!(p.run_until(200, |p| p.host.items.iter().any(|i| i.kind == ItemKind::Stick)));
    }

    #[test]
    fn a_guests_change_of_sky_reaches_the_host_and_the_host_reaches_back() {
        let mut p = pair();
        assert!(!p.host.world.night);
        p.guest.set_night(true);
        let mut ok = false;
        for _ in 0..400 {
            p.step();
            p.host.update_blackout(DT);
            p.guest.update_blackout(DT);
            if p.host.world.night && p.guest.world.night {
                ok = true;
                break;
            }
        }
        assert!(ok);
    }

    #[test]
    fn a_host_leaving_sends_the_guest_home() {
        let mut p = pair();
        p.host.net_shutdown();
        assert!(p.run_until(200, |p| p.guest.net.notice.is_some()), "the guest is told");
        assert!(!p.guest.is_guest());
    }

    #[test]
    fn a_guest_who_leaves_is_forgotten_by_the_host() {
        let mut p = pair();
        p.guest.net_shutdown();
        assert!(p.run_until(300, |p| p.host.net.guests.is_empty() && p.host.net.remotes.is_empty()));
    }

    #[test]
    fn a_bosss_smash_and_missiles_hurt_the_guest_it_is_near_and_not_the_far_host() {
        let mut p = pair();
        let far = free_spot(&mut p.host, 90, 0);
        p.guest.player.pos = far;
        assert!(p.run_until(300, |p| (p.host.net.guests[0].state.pos - far).length() < 1.0));
        let (host_hp, guest_hp) = (p.host.health, p.guest.health);
        let targets = p.host.targets();
        let gi = nearest_target(&targets, far);
        assert_eq!(targets[gi].guest, Some(0));
        p.host.carry_out_for(BossEvent::Smash { at: far + vec3(1.0, 0.0, 0.0) }, &targets, gi);
        assert!(p.run_until(200, |p| p.guest.health < guest_hp), "smashed");
        assert_eq!(p.host.health, host_hp);
        // A fireball thrown at the guest flies on the host and bursts on the guest.
        let hp = p.guest.health;
        p.host.carry_out_for(BossEvent::Fireball { from: far + vec3(6.0, 2.0, 0.0) }, &targets, gi);
        assert_eq!(p.host.projectiles.len(), 1);
        p.guest.health = MAX_HEALTH;
        assert!(p.run_until(300, |p| p.host.projectiles.is_empty()));
        assert!(p.run_until(100, |p| p.guest.health < MAX_HEALTH || p.guest.health != hp));
        assert_eq!(p.host.health, host_hp);
    }
}
