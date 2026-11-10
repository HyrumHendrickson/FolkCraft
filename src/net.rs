//! Local-network multiplayer, the transport half: a small binary wire format, framed TCP links with a reader and a writer thread
//! each, and UDP beacons so that a game on the same wifi can be found without typing an address.
//!
//! Nothing here knows about the game's rules (`game/session.rs` does); it only turns messages into bytes and back. The decoders
//! never panic on bad input: they return `None`, and the caller drops the link.

use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use macroquad::prelude::{vec3, Vec3};

use crate::animal::{AirPhase, AnimalKind, BossAction, HuntPhase, Mode, Pounce};
use crate::block::Block;
use crate::item::ItemKind;

/// The TCP port a host listens on, and the UDP port its beacon is broadcast to.
pub const PORT: u16 = 24680;
pub const DISCOVERY_PORT: u16 = 24681;
/// Bumped whenever the messages change, so that two different versions of the game refuse each other instead of misreading.
pub const PROTOCOL: u32 = 2;
/// The largest frame accepted (the world's save text is the biggest thing sent).
pub const MAX_FRAME: usize = 64 << 20;

// --- Encoding ------------------------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new(tag: u8) -> Writer {
        Writer { buf: vec![tag] }
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn vec3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }
    pub fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
    }
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, at: 0 }
    }
    fn remaining(&self) -> usize {
        self.data.len() - self.at
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if n > self.remaining() {
            return None;
        }
        let out = &self.data[self.at..self.at + n];
        self.at += n;
        Some(out)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    pub fn bool(&mut self) -> Option<bool> {
        self.u8().map(|b| b != 0)
    }
    pub fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.take(4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits).filter(|v| v.is_finite())
    }
    pub fn vec3(&mut self) -> Option<Vec3> {
        Some(vec3(self.f32()?, self.f32()?, self.f32()?))
    }
    pub fn str(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    /// A count of items that each take at least `min_bytes`, refused if the message is too short to hold them (so a lie about the
    /// count cannot make the decoder allocate gigabytes).
    pub fn count(&mut self, min_bytes: usize) -> Option<usize> {
        let n = self.u32()? as usize;
        (n.checked_mul(min_bytes.max(1))? <= self.remaining()).then_some(n)
    }
}

// --- Numbers for the game's enums ---------------------------------------------------------------------------------------

pub fn item_to_wire(kind: ItemKind) -> u16 {
    ItemKind::ALL.iter().position(|&k| k == kind).unwrap_or(0) as u16 + 1
}

pub fn item_from_wire(n: u16) -> Option<ItemKind> {
    ItemKind::ALL.get((n as usize).checked_sub(1)?).copied()
}

/// `0` for nothing.
pub fn opt_item_to_wire(kind: Option<ItemKind>) -> u16 {
    kind.map_or(0, item_to_wire)
}

pub fn opt_item_from_wire(n: u16) -> Option<ItemKind> {
    item_from_wire(n)
}

fn block_to_wire(block: Block) -> u8 {
    Block::PLAYER_MADE.iter().position(|&b| b == block).unwrap_or(0) as u8
}

fn block_from_wire(n: u8) -> Option<Block> {
    Block::PLAYER_MADE.get(n as usize).copied()
}

// --- What is sent ---------------------------------------------------------------------------------------------------------

/// `PlayerState::flags`: what a player carries that the creatures care about (the host's creatures react to every player, not
/// only the host's own).
pub const FLAG_HOLLY_ROD: u8 = 1;
pub const FLAG_IDOL: u8 = 2;
pub const FLAG_RABBIT_FOOT: u8 = 4;
pub const FLAG_CLOAKED: u8 = 8;
pub const FLAG_SWINGING: u8 = 16;

/// Where a player is and what they are doing, 20 times a second.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: u8,
    pub held: u16,
    pub worn: u16,
    pub flags: u8,
    /// Which dimension the player is in (0 the normal world, 1 the Horizen, 2 the Skylands).
    pub dim: u8,
}

impl PlayerState {
    fn write(&self, w: &mut Writer) {
        w.vec3(self.pos);
        w.vec3(self.vel);
        w.f32(self.yaw);
        w.f32(self.pitch);
        w.u8(self.health);
        w.u16(self.held);
        w.u16(self.worn);
        w.u8(self.flags);
        w.u8(self.dim);
    }
    fn read(r: &mut Reader) -> Option<PlayerState> {
        Some(PlayerState { pos: r.vec3()?, vel: r.vec3()?, yaw: r.f32()?, pitch: r.f32()?, health: r.u8()?, held: r.u16()?, worn: r.u16()?, flags: r.u8()?, dim: r.u8().filter(|d| *d < 3)? })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub state: PlayerState,
}

/// One creature as a guest needs to draw it. The guest never runs a creature's brain: it only shows what the host reports.
#[derive(Clone, Debug, PartialEq)]
pub struct NetAnimal {
    pub id: u32,
    pub kind: AnimalKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub home: Vec3,
    pub heading: f32,
    pub phase: f32,
    pub health: i32,
    pub mode: Mode,
    pub action: BossAction,
    pub pounce: Pounce,
    pub hunt_phase: HuntPhase,
    pub air_phase: AirPhase,
    pub on_ground: bool,
    pub enraged: bool,
    pub provoked: bool,
    pub summoned: bool,
    pub bonded: bool,
    pub hunting: bool,
    pub trail: Vec<Vec3>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetItem {
    pub id: u32,
    pub kind: ItemKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetArrow {
    pub kind: ItemKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetProjectile {
    pub kind: u8,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetBurst {
    pub pos: Vec3,
    pub shape: u8,
    pub age: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetTornado {
    pub pos: Vec3,
    pub target: Vec3,
    pub arrived: bool,
    pub age: f32,
    pub scale: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetStar {
    pub from: Vec3,
    pub to: Vec3,
    pub age: f32,
}

/// What the shared simulation of a dimension says about its weather and its last great hunt.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NetFlags {
    pub storm_active: bool,
    pub hunt_total: i32,
}

/// Everything about the host's shared simulation that a guest does not work out for itself, for one guest.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Snapshot {
    /// The dimension this is the simulation of: a guest that has since crossed into another ignores it.
    pub dim: u8,
    pub players: Vec<PlayerInfo>,
    pub animals: Vec<NetAnimal>,
    pub items: Vec<NetItem>,
    pub arrows: Vec<NetArrow>,
    pub hostile_arrows: Vec<NetArrow>,
    pub projectiles: Vec<NetProjectile>,
    pub bursts: Vec<NetBurst>,
    pub acid: Vec<(i32, i32, f32)>,
    pub tornadoes: Vec<NetTornado>,
    pub crater: Option<NetTornado>,
    pub stars: Vec<NetStar>,
    pub flags: NetFlags,
}

/// The story of one world that every player shares: what has been slain, summoned or rested, and where the rainbow stands.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Progress {
    pub yeti_slain: bool,
    pub orangutan_slain: bool,
    pub unicorn_summoned: bool,
    pub unicorn_slain: bool,
    pub graveyard_rested: bool,
    pub game_won: bool,
    pub phoenix_slain: bool,
    /// 0 unseen, 1 active, 2 spent; an active rainbow starts at `rainbow_from` and ends in gold at `rainbow_gold`.
    pub rainbow: u8,
    pub rainbow_from: (i32, i32),
    pub rainbow_gold: (i32, i32, i32),
    pub spent_gold: Option<(i32, i32, i32)>,
}

impl Progress {
    fn write(&self, w: &mut Writer) {
        let bits = self.yeti_slain as u8 | (self.orangutan_slain as u8) << 1 | (self.unicorn_summoned as u8) << 2 | (self.unicorn_slain as u8) << 3 | (self.graveyard_rested as u8) << 4 | (self.game_won as u8) << 5 | (self.phoenix_slain as u8) << 6;
        w.u8(bits);
        w.u8(self.rainbow);
        w.i32(self.rainbow_from.0);
        w.i32(self.rainbow_from.1);
        write_pos(w, self.rainbow_gold);
        w.bool(self.spent_gold.is_some());
        if let Some(p) = self.spent_gold {
            write_pos(w, p);
        }
    }

    fn read(r: &mut Reader) -> Option<Progress> {
        let bits = r.u8()?;
        let rainbow = r.u8().filter(|n| *n < 3)?;
        let rainbow_from = (r.i32()?, r.i32()?);
        let rainbow_gold = read_pos(r)?;
        let spent_gold = if r.bool()? { Some(read_pos(r)?) } else { None };
        Some(Progress {
            yeti_slain: bits & 1 != 0,
            orangutan_slain: bits & 2 != 0,
            unicorn_summoned: bits & 4 != 0,
            unicorn_slain: bits & 8 != 0,
            graveyard_rested: bits & 16 != 0,
            game_won: bits & 32 != 0,
            phoenix_slain: bits & 64 != 0,
            rainbow,
            rainbow_from,
            rainbow_gold,
            spent_gold,
        })
    }
}

/// A change to a world that the player's own actions or the host's fires make, replayed on every other machine. Each says what
/// the state is now (not what was done to get there), so replaying one twice, or on a machine whose chunk is not loaded, is harmless.
#[derive(Clone, Debug, PartialEq)]
pub enum WorldOp {
    Break((i32, i32, i32)),
    Set((i32, i32, i32), Block),
    /// Seconds a fire at that place has left (`None`: not burning).
    Fire((i32, i32, i32), Option<f32>),
    /// Seconds of bellows glow left.
    Glow((i32, i32, i32), Option<f32>),
    /// The piece cooking there: what went on, and the seconds of heat it still needs.
    Cook((i32, i32, i32), Option<(ItemKind, f32)>),
    /// The pot there: water, raw meat, seconds of heat the stew still needs, servings taken.
    PotAt((i32, i32, i32), Option<(bool, Option<ItemKind>, f32, u32)>),
    /// A bush there has had its berries picked.
    Berry((i32, i32, i32)),
    MonsterSlain(i32, i32),
    OpenKingDoor(u8),
    OpenTemple,
    UnlockTower(i32, i32),
    Progress(Progress),
}

fn write_pos(w: &mut Writer, p: (i32, i32, i32)) {
    w.i32(p.0);
    w.i32(p.1);
    w.i32(p.2);
}

fn read_pos(r: &mut Reader) -> Option<(i32, i32, i32)> {
    Some((r.i32()?, r.i32()?, r.i32()?))
}

fn write_opt_f32(w: &mut Writer, v: Option<f32>) {
    w.bool(v.is_some());
    if let Some(v) = v {
        w.f32(v);
    }
}

fn read_opt_f32(r: &mut Reader) -> Option<Option<f32>> {
    Some(if r.bool()? { Some(r.f32()?) } else { None })
}

impl WorldOp {
    fn write(&self, w: &mut Writer) {
        match self {
            WorldOp::Break(p) => { w.u8(0); write_pos(w, *p); }
            WorldOp::Set(p, b) => { w.u8(1); write_pos(w, *p); w.u8(block_to_wire(*b)); }
            WorldOp::Fire(p, s) => { w.u8(2); write_pos(w, *p); write_opt_f32(w, *s); }
            WorldOp::Glow(p, s) => { w.u8(3); write_pos(w, *p); write_opt_f32(w, *s); }
            WorldOp::Cook(p, c) => {
                w.u8(4);
                write_pos(w, *p);
                w.bool(c.is_some());
                if let Some((k, left)) = c {
                    w.u16(item_to_wire(*k));
                    w.f32(*left);
                }
            }
            WorldOp::PotAt(p, pot) => {
                w.u8(5);
                write_pos(w, *p);
                w.bool(pot.is_some());
                if let Some((water, meat, left, served)) = pot {
                    w.bool(*water);
                    w.u16(opt_item_to_wire(*meat));
                    w.f32(*left);
                    w.u32(*served);
                }
            }
            WorldOp::Berry(p) => { w.u8(6); write_pos(w, *p); }
            WorldOp::MonsterSlain(x, z) => { w.u8(7); w.i32(*x); w.i32(*z); }
            WorldOp::OpenKingDoor(room) => { w.u8(8); w.u8(*room); }
            WorldOp::OpenTemple => w.u8(9),
            WorldOp::UnlockTower(x, z) => { w.u8(10); w.i32(*x); w.i32(*z); }
            WorldOp::Progress(p) => { w.u8(11); p.write(w); }
        }
    }

    fn read(r: &mut Reader) -> Option<WorldOp> {
        Some(match r.u8()? {
            0 => WorldOp::Break(read_pos(r)?),
            1 => WorldOp::Set(read_pos(r)?, block_from_wire(r.u8()?)?),
            2 => WorldOp::Fire(read_pos(r)?, read_opt_f32(r)?),
            3 => WorldOp::Glow(read_pos(r)?, read_opt_f32(r)?),
            4 => {
                let p = read_pos(r)?;
                WorldOp::Cook(p, if r.bool()? { Some((item_from_wire(r.u16()?)?, r.f32()?)) } else { None })
            }
            5 => {
                let p = read_pos(r)?;
                WorldOp::PotAt(p, if r.bool()? { Some((r.bool()?, opt_item_from_wire(r.u16()?), r.f32()?, r.u32()?)) } else { None })
            }
            6 => WorldOp::Berry(read_pos(r)?),
            7 => WorldOp::MonsterSlain(r.i32()?, r.i32()?),
            8 => WorldOp::OpenKingDoor(r.u8()?),
            9 => WorldOp::OpenTemple,
            10 => WorldOp::UnlockTower(r.i32()?, r.i32()?),
            11 => WorldOp::Progress(Progress::read(r)?),
            _ => return None,
        })
    }
}

fn write_ops(w: &mut Writer, dim: u8, ops: &[WorldOp]) {
    w.u8(dim);
    w.u32(ops.len() as u32);
    for op in ops {
        op.write(w);
    }
}

fn read_ops(r: &mut Reader) -> Option<(u8, Vec<WorldOp>)> {
    let dim = r.u8()?;
    let n = r.count(1)?;
    let mut ops = Vec::with_capacity(n);
    for _ in 0..n {
        ops.push(WorldOp::read(r)?);
    }
    Some((dim, ops))
}

fn write_arrow(w: &mut Writer, a: &NetArrow) {
    w.u16(item_to_wire(a.kind));
    w.vec3(a.pos);
    w.vec3(a.vel);
    w.f32(a.age);
}

fn read_arrow(r: &mut Reader) -> Option<NetArrow> {
    Some(NetArrow { kind: item_from_wire(r.u16()?)?, pos: r.vec3()?, vel: r.vec3()?, age: r.f32()? })
}

fn write_arrows(w: &mut Writer, list: &[NetArrow]) {
    w.u32(list.len() as u32);
    for a in list {
        write_arrow(w, a);
    }
}

fn read_arrows(r: &mut Reader) -> Option<Vec<NetArrow>> {
    let n = r.count(30)?;
    (0..n).map(|_| read_arrow(r)).collect()
}

fn write_tornado(w: &mut Writer, t: &NetTornado) {
    w.vec3(t.pos);
    w.vec3(t.target);
    w.bool(t.arrived);
    w.f32(t.age);
    w.f32(t.scale);
}

fn read_tornado(r: &mut Reader) -> Option<NetTornado> {
    Some(NetTornado { pos: r.vec3()?, target: r.vec3()?, arrived: r.bool()?, age: r.f32()?, scale: r.f32()? })
}

impl Snapshot {
    fn write(&self, w: &mut Writer) {
        w.u8(self.dim);
        w.u32(self.players.len() as u32);
        for p in &self.players {
            w.u32(p.id);
            w.str(&p.name);
            p.state.write(w);
        }
        w.u32(self.animals.len() as u32);
        for a in &self.animals {
            w.u32(a.id);
            w.u8(a.kind.wire());
            w.vec3(a.pos);
            w.vec3(a.vel);
            w.vec3(a.home);
            w.f32(a.heading);
            w.f32(a.phase);
            w.i32(a.health);
            w.u8(a.mode.wire());
            w.u8(a.action.wire());
            w.u8(a.pounce.wire());
            w.u8(a.hunt_phase.wire());
            w.u8(a.air_phase.wire());
            let bits = a.on_ground as u8 | (a.enraged as u8) << 1 | (a.provoked as u8) << 2 | (a.summoned as u8) << 3 | (a.bonded as u8) << 4 | (a.hunting as u8) << 5;
            w.u8(bits);
            w.u32(a.trail.len() as u32);
            for &p in &a.trail {
                w.vec3(p);
            }
        }
        w.u32(self.items.len() as u32);
        for i in &self.items {
            w.u32(i.id);
            w.u16(item_to_wire(i.kind));
            w.vec3(i.pos);
            w.vec3(i.vel);
            w.f32(i.age);
        }
        write_arrows(w, &self.arrows);
        write_arrows(w, &self.hostile_arrows);
        w.u32(self.projectiles.len() as u32);
        for p in &self.projectiles {
            w.u8(p.kind);
            w.vec3(p.pos);
            w.vec3(p.vel);
            w.f32(p.age);
        }
        w.u32(self.bursts.len() as u32);
        for b in &self.bursts {
            w.vec3(b.pos);
            w.u8(b.shape);
            w.f32(b.age);
        }
        w.u32(self.acid.len() as u32);
        for &(x, z, left) in &self.acid {
            w.i32(x);
            w.i32(z);
            w.f32(left);
        }
        w.u32(self.tornadoes.len() as u32);
        for t in &self.tornadoes {
            write_tornado(w, t);
        }
        w.bool(self.crater.is_some());
        if let Some(t) = &self.crater {
            write_tornado(w, t);
        }
        w.u32(self.stars.len() as u32);
        for s in &self.stars {
            w.vec3(s.from);
            w.vec3(s.to);
            w.f32(s.age);
        }
        w.bool(self.flags.storm_active);
        w.i32(self.flags.hunt_total);
    }

    fn read(r: &mut Reader) -> Option<Snapshot> {
        let mut s = Snapshot { dim: r.u8().filter(|d| *d < 3)?, ..Snapshot::default() };
        for _ in 0..r.count(8)? {
            s.players.push(PlayerInfo { id: r.u32()?, name: r.str()?, state: PlayerState::read(r)? });
        }
        for _ in 0..r.count(60)? {
            let id = r.u32()?;
            let kind = AnimalKind::from_wire(r.u8()?)?;
            let (pos, vel, home) = (r.vec3()?, r.vec3()?, r.vec3()?);
            let (heading, phase, health) = (r.f32()?, r.f32()?, r.i32()?);
            let mode = Mode::from_wire(r.u8()?)?;
            let action = BossAction::from_wire(r.u8()?)?;
            let pounce = Pounce::from_wire(r.u8()?)?;
            let hunt_phase = HuntPhase::from_wire(r.u8()?)?;
            let air_phase = AirPhase::from_wire(r.u8()?)?;
            let bits = r.u8()?;
            let mut trail = Vec::new();
            for _ in 0..r.count(12)? {
                trail.push(r.vec3()?);
            }
            s.animals.push(NetAnimal {
                id, kind, pos, vel, home, heading, phase, health, mode, action, pounce, hunt_phase, air_phase,
                on_ground: bits & 1 != 0, enraged: bits & 2 != 0, provoked: bits & 4 != 0, summoned: bits & 8 != 0, bonded: bits & 16 != 0, hunting: bits & 32 != 0,
                trail,
            });
        }
        for _ in 0..r.count(34)? {
            s.items.push(NetItem { id: r.u32()?, kind: item_from_wire(r.u16()?)?, pos: r.vec3()?, vel: r.vec3()?, age: r.f32()? });
        }
        s.arrows = read_arrows(r)?;
        s.hostile_arrows = read_arrows(r)?;
        for _ in 0..r.count(29)? {
            s.projectiles.push(NetProjectile { kind: r.u8()?, pos: r.vec3()?, vel: r.vec3()?, age: r.f32()? });
        }
        for _ in 0..r.count(17)? {
            s.bursts.push(NetBurst { pos: r.vec3()?, shape: r.u8()?, age: r.f32()? });
        }
        for _ in 0..r.count(12)? {
            s.acid.push((r.i32()?, r.i32()?, r.f32()?));
        }
        for _ in 0..r.count(33)? {
            s.tornadoes.push(read_tornado(r)?);
        }
        if r.bool()? {
            s.crater = Some(read_tornado(r)?);
        }
        for _ in 0..r.count(28)? {
            s.stars.push(NetStar { from: r.vec3()?, to: r.vec3()?, age: r.f32()? });
        }
        s.flags = NetFlags { storm_active: r.bool()?, hunt_total: r.i32()? };
        Some(s)
    }
}

/// Something a guest does that only the host's simulation can carry out, because it acts on the creatures there.
#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    /// The Horn of the Hunt was blown (the host checks that the guest stands in Stonehenge).
    Horn,
    /// Musical pipes were played: they call birds by day and bats by night.
    Pipes,
    /// A Holly Rod called lightning to this point (one following whisp is spent).
    Lightning(Vec3),
    /// The guest gave the king this many offerings of gold (already taken from the guest's pack): spirit archers come to follow them.
    KingGold(u32),
}

/// What a guest tells the host.
#[derive(Clone, Debug, PartialEq)]
pub enum ToHost {
    Hello { protocol: u32, name: String },
    /// The guest's own state, with the arrows it has in the air (so that everyone sees them).
    State { state: PlayerState, arrows: Vec<NetArrow> },
    Ops { dim: u8, ops: Vec<WorldOp> },
    /// Blows the guest landed: (creature id, damage), struck from `from`, by a guest wearing `worn`.
    Hurt { hits: Vec<(u32, i32)>, from: Vec3, worn: u16 },
    SpawnItems(Vec<(ItemKind, Vec3, Vec3)>),
    /// The guest wants these items on the ground (the host says yes to the first to ask).
    Pickup(Vec<u32>),
    /// The sky of this dimension was turned (a crystal ball, a stew).
    Night { dim: u8, night: bool },
    /// A bow or sling was shot here (Nessie listens).
    Shot(Vec3),
    Intent(Intent),
    /// What the guest carries, in the host's save format (see `Game::profile_text`), so that the host can keep it for next time.
    Profile(String),
    Bye,
}

/// What the host tells a guest.
#[derive(Clone, Debug, PartialEq)]
pub enum ToGuest {
    Welcome { protocol: u32, id: u32, host_name: String, save: String },
    Refused(String),
    Snapshot(Box<Snapshot>),
    Ops { dim: u8, ops: Vec<WorldOp> },
    /// The guest was hurt by a creature or a boss attack: the guest applies its own armour.
    Hurt { amount: u32, death: String, from: Vec3, launch: f32 },
    Night { dim: u8, night: bool },
    /// What this guest carried when it was last here (empty if it was never here).
    Profile(String),
    /// An item the guest asked for is theirs.
    Grant { kind: ItemKind },
    Toast(String),
    Victory,
    Bye(String),
}

impl ToHost {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ToHost::Hello { protocol, name } => {
                let mut w = Writer::new(1);
                w.u32(*protocol);
                w.str(name);
                w.finish()
            }
            ToHost::State { state, arrows } => {
                let mut w = Writer::new(2);
                state.write(&mut w);
                write_arrows(&mut w, arrows);
                w.finish()
            }
            ToHost::Ops { dim, ops } => {
                let mut w = Writer::new(3);
                write_ops(&mut w, *dim, ops);
                w.finish()
            }
            ToHost::Hurt { hits, from, worn } => {
                let mut w = Writer::new(4);
                w.u32(hits.len() as u32);
                for &(id, damage) in hits {
                    w.u32(id);
                    w.i32(damage);
                }
                w.vec3(*from);
                w.u16(*worn);
                w.finish()
            }
            ToHost::SpawnItems(items) => {
                let mut w = Writer::new(5);
                w.u32(items.len() as u32);
                for &(kind, pos, vel) in items {
                    w.u16(item_to_wire(kind));
                    w.vec3(pos);
                    w.vec3(vel);
                }
                w.finish()
            }
            ToHost::Pickup(ids) => {
                let mut w = Writer::new(6);
                w.u32(ids.len() as u32);
                for &id in ids {
                    w.u32(id);
                }
                w.finish()
            }
            ToHost::Night { dim, night } => {
                let mut w = Writer::new(7);
                w.u8(*dim);
                w.bool(*night);
                w.finish()
            }
            ToHost::Intent(i) => {
                let mut w = Writer::new(10);
                match i {
                    Intent::Horn => w.u8(0),
                    Intent::Pipes => w.u8(1),
                    Intent::Lightning(p) => {
                        w.u8(2);
                        w.vec3(*p);
                    }
                    Intent::KingGold(n) => {
                        w.u8(3);
                        w.u32(*n);
                    }
                }
                w.finish()
            }
            ToHost::Profile(text) => {
                let mut w = Writer::new(11);
                w.str(text);
                w.finish()
            }
            ToHost::Shot(p) => {
                let mut w = Writer::new(8);
                w.vec3(*p);
                w.finish()
            }
            ToHost::Bye => Writer::new(9).finish(),
        }
    }

    pub fn decode(bytes: &[u8]) -> Option<ToHost> {
        let mut r = Reader::new(bytes);
        Some(match r.u8()? {
            1 => ToHost::Hello { protocol: r.u32()?, name: r.str()? },
            2 => ToHost::State { state: PlayerState::read(&mut r)?, arrows: read_arrows(&mut r)? },
            3 => {
                let (dim, ops) = read_ops(&mut r)?;
                ToHost::Ops { dim, ops }
            }
            4 => {
                let n = r.count(8)?;
                let mut hits = Vec::with_capacity(n);
                for _ in 0..n {
                    hits.push((r.u32()?, r.i32()?));
                }
                ToHost::Hurt { hits, from: r.vec3()?, worn: r.u16()? }
            }
            5 => {
                let n = r.count(26)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push((item_from_wire(r.u16()?)?, r.vec3()?, r.vec3()?));
                }
                ToHost::SpawnItems(items)
            }
            6 => {
                let n = r.count(4)?;
                ToHost::Pickup((0..n).map(|_| r.u32()).collect::<Option<Vec<_>>>()?)
            }
            7 => ToHost::Night { dim: r.u8().filter(|d| *d < 3)?, night: r.bool()? },
            8 => ToHost::Shot(r.vec3()?),
            9 => ToHost::Bye,
            10 => ToHost::Intent(match r.u8()? {
                0 => Intent::Horn,
                1 => Intent::Pipes,
                2 => Intent::Lightning(r.vec3()?),
                3 => Intent::KingGold(r.u32()?),
                _ => return None,
            }),
            11 => ToHost::Profile(r.str()?),
            _ => return None,
        })
    }
}

impl ToGuest {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ToGuest::Welcome { protocol, id, host_name, save } => {
                let mut w = Writer::new(1);
                w.u32(*protocol);
                w.u32(*id);
                w.str(host_name);
                w.str(save);
                w.finish()
            }
            ToGuest::Refused(why) => {
                let mut w = Writer::new(2);
                w.str(why);
                w.finish()
            }
            ToGuest::Snapshot(s) => {
                let mut w = Writer::new(3);
                s.write(&mut w);
                w.finish()
            }
            ToGuest::Ops { dim, ops } => {
                let mut w = Writer::new(4);
                write_ops(&mut w, *dim, ops);
                w.finish()
            }
            ToGuest::Hurt { amount, death, from, launch } => {
                let mut w = Writer::new(5);
                w.u32(*amount);
                w.str(death);
                w.vec3(*from);
                w.f32(*launch);
                w.finish()
            }
            ToGuest::Profile(text) => {
                let mut w = Writer::new(6);
                w.str(text);
                w.finish()
            }
            ToGuest::Night { dim, night } => {
                let mut w = Writer::new(7);
                w.u8(*dim);
                w.bool(*night);
                w.finish()
            }
            ToGuest::Grant { kind } => {
                let mut w = Writer::new(8);
                w.u16(item_to_wire(*kind));
                w.finish()
            }
            ToGuest::Toast(text) => {
                let mut w = Writer::new(9);
                w.str(text);
                w.finish()
            }
            ToGuest::Victory => Writer::new(10).finish(),
            ToGuest::Bye(why) => {
                let mut w = Writer::new(11);
                w.str(why);
                w.finish()
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> Option<ToGuest> {
        let mut r = Reader::new(bytes);
        Some(match r.u8()? {
            1 => ToGuest::Welcome { protocol: r.u32()?, id: r.u32()?, host_name: r.str()?, save: r.str()? },
            2 => ToGuest::Refused(r.str()?),
            3 => ToGuest::Snapshot(Box::new(Snapshot::read(&mut r)?)),
            4 => {
                let (dim, ops) = read_ops(&mut r)?;
                ToGuest::Ops { dim, ops }
            }
            5 => ToGuest::Hurt { amount: r.u32()?, death: r.str()?, from: r.vec3()?, launch: r.f32()? },
            6 => ToGuest::Profile(r.str()?),
            7 => ToGuest::Night { dim: r.u8().filter(|d| *d < 3)?, night: r.bool()? },
            8 => ToGuest::Grant { kind: item_from_wire(r.u16()?)? },
            9 => ToGuest::Toast(r.str()?),
            10 => ToGuest::Victory,
            11 => ToGuest::Bye(r.str()?),
            _ => return None,
        })
    }
}

// --- Links ---------------------------------------------------------------------------------------------------------------

/// One TCP connection, framed (`u32` length, then the message), with its reading and writing done on threads so the game loop
/// never waits on the network.
pub struct Link {
    rx: Receiver<Vec<u8>>,
    tx: Sender<Vec<u8>>,
    alive: Arc<AtomicBool>,
    queued: Arc<AtomicUsize>,
    stream: TcpStream,
}

/// Bytes waiting to be written beyond which a link is judged stalled and dropped, and beyond which droppable messages (snapshots) are
/// skipped.
const STALLED_BYTES: usize = 48 << 20;
pub const BUSY_BYTES: usize = 256 << 10;

impl Link {
    pub fn new(stream: TcpStream) -> io::Result<Link> {
        stream.set_nonblocking(false)?;
        let _ = stream.set_nodelay(true);
        let alive = Arc::new(AtomicBool::new(true));
        let queued = Arc::new(AtomicUsize::new(0));

        let (in_tx, in_rx) = channel::<Vec<u8>>();
        let mut reader = stream.try_clone()?;
        let flag = alive.clone();
        thread::spawn(move || {
            let mut len = [0u8; 4];
            while flag.load(Ordering::Relaxed) {
                if reader.read_exact(&mut len).is_err() {
                    break;
                }
                let n = u32::from_le_bytes(len) as usize;
                if n > MAX_FRAME {
                    break;
                }
                let mut payload = vec![0u8; n];
                if reader.read_exact(&mut payload).is_err() || in_tx.send(payload).is_err() {
                    break;
                }
            }
            flag.store(false, Ordering::Relaxed);
        });

        let (out_tx, out_rx) = channel::<Vec<u8>>();
        let mut writer = stream.try_clone()?;
        let (flag, backlog) = (alive.clone(), queued.clone());
        thread::spawn(move || {
            while let Ok(payload) = out_rx.recv() {
                let mut frame = Vec::with_capacity(payload.len() + 4);
                frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                frame.extend_from_slice(&payload);
                let ok = writer.write_all(&frame).and_then(|_| writer.flush()).is_ok();
                backlog.fetch_sub(payload.len(), Ordering::Relaxed);
                if !ok {
                    break;
                }
            }
            flag.store(false, Ordering::Relaxed);
        });

        Ok(Link { rx: in_rx, tx: out_tx, alive, queued, stream })
    }

    /// Queue one message. A link whose queue has grown absurdly is cut off (its other end has stopped listening).
    pub fn send(&self, payload: Vec<u8>) {
        if self.queued.fetch_add(payload.len(), Ordering::Relaxed) > STALLED_BYTES {
            self.close();
            return;
        }
        if self.tx.send(payload).is_err() {
            self.alive.store(false, Ordering::Relaxed);
        }
    }

    /// Every message that has arrived since the last call.
    pub fn poll(&self) -> Vec<Vec<u8>> {
        self.rx.try_iter().collect()
    }

    /// Whether the connection is still up. Messages that arrived before it dropped are still returned by `poll`.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Bytes queued but not yet written.
    pub fn backlog(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        self.alive.store(false, Ordering::Relaxed);
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.close();
    }
}

/// The host's listening socket, polled from the game loop (it never blocks).
pub struct Listener {
    socket: TcpListener,
    pub port: u16,
}

impl Listener {
    pub fn bind(port: u16) -> io::Result<Listener> {
        let socket = TcpListener::bind(("0.0.0.0", port))?;
        socket.set_nonblocking(true)?;
        let port = socket.local_addr()?.port();
        Ok(Listener { socket, port })
    }

    /// The connections that have arrived since the last call.
    pub fn accept(&self) -> Vec<Link> {
        let mut out = Vec::new();
        while let Ok((stream, _)) = self.socket.accept() {
            if let Ok(link) = Link::new(stream) {
                out.push(link);
            }
        }
        out
    }
}

/// Connect to a host, waiting at most a few seconds.
pub fn connect(addr: SocketAddr) -> io::Result<Link> {
    Link::new(TcpStream::connect_timeout(&addr, Duration::from_secs(5))?)
}

/// An address as a person types it: `192.168.1.20`, `192.168.1.20:24680` or a host name.
pub fn parse_address(text: &str) -> Option<SocketAddr> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Some(addr);
    }
    if let Ok(ip) = text.parse::<IpAddr>() {
        return Some(SocketAddr::new(ip, PORT));
    }
    let with_port = if text.contains(':') { text.to_string() } else { format!("{text}:{PORT}") };
    with_port.to_socket_addrs().ok()?.next()
}

/// The address other machines on the network can reach this one at, if it can be worked out (no packet is sent).
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

// --- Finding games on the network -----------------------------------------------------------------------------------------

const BEACON_MAGIC: &str = "FOLKCRAFT1";

/// A host's beacon: once a second it tells the whole local network which game is open and where.
pub struct Beacon {
    stop: Arc<AtomicBool>,
}

impl Beacon {
    pub fn start(host_name: String, world_name: String, port: u16) -> Beacon {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        thread::spawn(move || {
            let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else { return };
            let _ = socket.set_broadcast(true);
            let clean = |s: &str| s.replace(['\n', '\r'], " ");
            let text = format!("{BEACON_MAGIC}\n{}\n{}\n{port}", clean(&host_name), clean(&world_name));
            while !flag.load(Ordering::Relaxed) {
                let _ = socket.send_to(text.as_bytes(), ("255.255.255.255", DISCOVERY_PORT));
                // Some routers drop the global broadcast: also try the broadcast address of this machine's /24.
                if let Some(IpAddr::V4(ip)) = local_ip() {
                    let o = ip.octets();
                    let _ = socket.send_to(text.as_bytes(), (std::net::Ipv4Addr::new(o[0], o[1], o[2], 255), DISCOVERY_PORT));
                }
                for _ in 0..10 {
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Beacon { stop }
    }
}

impl Drop for Beacon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A game seen on the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub addr: SocketAddr,
    pub host_name: String,
    pub world_name: String,
    pub seen: Instant,
}

/// Listens for beacons. If the port is taken (another copy of the game on this machine is already listening) it simply finds
/// nothing, and the address can still be typed in.
pub struct Finder {
    socket: Option<UdpSocket>,
    pub games: Vec<Found>,
}

impl Finder {
    pub fn start() -> Finder {
        let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)).ok().filter(|s| s.set_nonblocking(true).is_ok());
        Finder { socket, games: Vec::new() }
    }

    /// Read what has arrived and forget games not heard from for five seconds.
    pub fn poll(&mut self) {
        if let Some(socket) = &self.socket {
            let mut buf = [0u8; 512];
            while let Ok((n, from)) = socket.recv_from(&mut buf) {
                if let Some(found) = parse_beacon(&buf[..n], from.ip()) {
                    match self.games.iter_mut().find(|g| g.addr == found.addr) {
                        Some(g) => *g = found,
                        None => self.games.push(found),
                    }
                }
            }
        }
        self.games.retain(|g| g.seen.elapsed() < Duration::from_secs(5));
    }
}

fn parse_beacon(bytes: &[u8], from: IpAddr) -> Option<Found> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    if lines.next()? != BEACON_MAGIC {
        return None;
    }
    let host_name = lines.next()?.to_string();
    let world_name = lines.next()?.to_string();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    Some(Found { addr: SocketAddr::new(from, port), host_name, world_name, seen: Instant::now() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> PlayerState {
        PlayerState { pos: vec3(1.5, 6.0, -3.25), vel: vec3(0.0, -1.0, 2.0), yaw: 1.0, pitch: -0.5, health: 7, held: 4, worn: 0, flags: FLAG_IDOL | FLAG_SWINGING, dim: 1 }
    }

    fn sample_animal(id: u32, kind: AnimalKind) -> NetAnimal {
        NetAnimal {
            id, kind, pos: vec3(1.0, 2.0, 3.0), vel: vec3(0.1, 0.0, 0.2), home: vec3(9.0, 9.0, 9.0), heading: 2.0, phase: 7.5, health: 33,
            mode: Mode::Charge, action: BossAction::Smash, pounce: Pounce::Leap, hunt_phase: HuntPhase::Retreat, air_phase: AirPhase::Dive,
            on_ground: true, enraged: true, provoked: false, summoned: true, bonded: false, hunting: true, trail: vec![vec3(1.0, 1.0, 1.0), vec3(2.0, 2.0, 2.0)],
        }
    }

    #[test]
    fn every_message_to_the_host_survives_the_wire() {
        let arrow = NetArrow { kind: ItemKind::Arrow, pos: vec3(1.0, 2.0, 3.0), vel: vec3(4.0, 5.0, 6.0), age: 0.25 };
        let messages = vec![
            ToHost::Hello { protocol: PROTOCOL, name: "Ana ✓".into() },
            ToHost::State { state: sample_state(), arrows: vec![arrow.clone(), arrow] },
            ToHost::Ops { dim: 1, ops: vec![WorldOp::Break((1, 2, 3)), WorldOp::Set((-4, 5, 6), Block::Fire), WorldOp::Cook((0, 1, 0), Some((ItemKind::Venison, 12.5))), WorldOp::OpenTemple] },
            ToHost::Hurt { hits: vec![(7, 4), (9, 12)], from: vec3(1.0, 1.0, 1.0), worn: 3 },
            ToHost::SpawnItems(vec![(ItemKind::Stick, vec3(1.0, 2.0, 3.0), vec3(0.0, 1.0, 0.0))]),
            ToHost::Pickup(vec![1, 2, 3]),
            ToHost::Night { dim: 2, night: true },
            ToHost::Intent(Intent::Horn),
            ToHost::Intent(Intent::Pipes),
            ToHost::Intent(Intent::Lightning(vec3(1.0, 2.0, 3.0))),
            ToHost::Intent(Intent::KingGold(3)),
            ToHost::Profile("guest Ana\ninv 0 1 3\n".into()),
            ToHost::Shot(vec3(5.0, 6.0, 7.0)),
            ToHost::Bye,
        ];
        for m in messages {
            assert_eq!(ToHost::decode(&m.encode()), Some(m.clone()), "{m:?}");
        }
    }

    #[test]
    fn every_message_to_a_guest_survives_the_wire() {
        let snapshot = Snapshot {
            players: vec![PlayerInfo { id: 3, name: "Host".into(), state: sample_state() }],
            animals: vec![sample_animal(1, AnimalKind::Yeti), sample_animal(2, AnimalKind::FlyingSnake)],
            items: vec![NetItem { id: 5, kind: ItemKind::Gold, pos: vec3(1.0, 2.0, 3.0), vel: Vec3::ZERO, age: 1.0 }],
            arrows: vec![NetArrow { kind: ItemKind::Pebble, pos: vec3(1.0, 2.0, 3.0), vel: vec3(0.0, 1.0, 0.0), age: 0.5 }],
            hostile_arrows: vec![NetArrow { kind: ItemKind::IronFeather, pos: vec3(3.0, 2.0, 1.0), vel: vec3(0.0, 1.0, 0.0), age: 0.5 }],
            projectiles: vec![NetProjectile { kind: 2, pos: vec3(1.0, 1.0, 1.0), vel: vec3(1.0, 0.0, 0.0), age: 0.1 }],
            bursts: vec![NetBurst { pos: vec3(1.0, 2.0, 3.0), shape: 3, age: 0.2 }],
            acid: vec![(4, -5, 3.5)],
            tornadoes: vec![NetTornado { pos: vec3(1.0, 2.0, 3.0), target: vec3(4.0, 5.0, 6.0), arrived: false, age: 2.0, scale: 1.0 }],
            crater: Some(NetTornado { pos: vec3(7.0, 8.0, 9.0), target: vec3(7.0, 8.0, 9.0), arrived: true, age: 0.0, scale: 3.0 }),
            stars: vec![NetStar { from: vec3(0.0, 90.0, 0.0), to: vec3(1.0, 5.0, 1.0), age: 0.5 }],
            flags: NetFlags { storm_active: true, hunt_total: 120 },
            dim: 2,
        };
        let messages = vec![
            ToGuest::Welcome { protocol: PROTOCOL, id: 2, host_name: "Host".into(), save: "world\n1\n0 6 0 0 0\n".into() },
            ToGuest::Refused("full".into()),
            ToGuest::Snapshot(Box::new(snapshot)),
            ToGuest::Snapshot(Box::new(Snapshot::default())),
            ToGuest::Ops { dim: 2, ops: vec![WorldOp::Fire((1, 2, 3), None), WorldOp::Fire((1, 2, 3), Some(60.0)), WorldOp::Glow((1, 2, 3), Some(5.0)), WorldOp::PotAt((1, 2, 3), Some((true, Some(ItemKind::Venison), 30.0, 2))), WorldOp::PotAt((4, 5, 6), None), WorldOp::Berry((7, 8, 9)), WorldOp::MonsterSlain(10, -20), WorldOp::OpenKingDoor(3), WorldOp::UnlockTower(5, 6), WorldOp::Progress(Progress { yeti_slain: true, graveyard_rested: true, rainbow: 1, rainbow_from: (3, -4), rainbow_gold: (5, 6, 7), spent_gold: Some((1, 2, 3)), ..Progress::default() })] },
            ToGuest::Hurt { amount: 3, death: "The Yeti smashed you flat".into(), from: vec3(1.0, 2.0, 3.0), launch: 12.0 },
            ToGuest::Profile(String::new()),
            ToGuest::Night { dim: 0, night: false },
            ToGuest::Grant { kind: ItemKind::Bone },
            ToGuest::Toast("hello".into()),
            ToGuest::Victory,
            ToGuest::Bye("closed".into()),
        ];
        for m in messages {
            assert_eq!(ToGuest::decode(&m.encode()), Some(m.clone()), "{m:?}");
        }
    }

    #[test]
    fn every_kind_of_creature_and_item_has_a_number_that_comes_back() {
        for kind in AnimalKind::ALL {
            assert_eq!(AnimalKind::from_wire(kind.wire()), Some(kind));
        }
        for kind in ItemKind::ALL {
            assert_eq!(item_from_wire(item_to_wire(kind)), Some(kind));
        }
        for block in Block::PLAYER_MADE {
            assert_eq!(block_from_wire(block_to_wire(block)), Some(block));
        }
        assert_eq!(item_from_wire(0), None);
        assert_eq!(item_from_wire(60000), None);
    }

    #[test]
    fn garbage_and_truncated_messages_are_refused_without_a_panic() {
        let good = ToGuest::Snapshot(Box::new(Snapshot { animals: vec![sample_animal(1, AnimalKind::Bear)], ..Snapshot::default() })).encode();
        for cut in 0..good.len() {
            assert_eq!(ToGuest::decode(&good[..cut]), None, "cut at {cut}");
        }
        // A lie about how many animals follow cannot make the decoder reserve memory for them.
        let mut lie = vec![3u8, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f];
        assert_eq!(ToGuest::decode(&lie), None);
        lie[0] = 200;
        assert_eq!(ToGuest::decode(&lie), None);
        assert_eq!(ToHost::decode(&[]), None);
        assert_eq!(ToHost::decode(&[99]), None);
        // Not-a-number floats are refused too: they would poison positions.
        let mut w = Writer::new(8);
        w.f32(f32::NAN);
        w.f32(0.0);
        w.f32(0.0);
        assert_eq!(ToHost::decode(&w.finish()), None);
        // Pseudo-random noise never panics.
        let mut seed = 12345u64;
        for _ in 0..2000 {
            let len = (crate::rng::splitmix64(seed) % 200) as usize;
            let noise: Vec<u8> = (0..len).map(|i| crate::rng::splitmix64(seed + i as u64 + 1) as u8).collect();
            seed += 1000;
            let _ = ToHost::decode(&noise);
            let _ = ToGuest::decode(&noise);
        }
    }

    #[test]
    fn addresses_are_read_the_way_people_type_them() {
        assert_eq!(parse_address("192.168.1.20"), Some(SocketAddr::new("192.168.1.20".parse().unwrap(), PORT)));
        assert_eq!(parse_address(" 10.0.0.5:9000 "), Some("10.0.0.5:9000".parse().unwrap()));
        assert_eq!(parse_address("127.0.0.1").map(|a| a.port()), Some(PORT));
        assert_eq!(parse_address(""), None);
        assert_eq!(parse_address("   "), None);
    }

    #[test]
    fn a_beacon_names_the_game_and_the_port() {
        let text = format!("{BEACON_MAGIC}\nAna\nMy World\n24680");
        let found = parse_beacon(text.as_bytes(), "192.168.1.9".parse().unwrap()).unwrap();
        assert_eq!(found.addr, "192.168.1.9:24680".parse().unwrap());
        assert_eq!((found.host_name.as_str(), found.world_name.as_str()), ("Ana", "My World"));
        assert!(parse_beacon(b"SOMETHINGELSE\na\nb\n1", "1.2.3.4".parse().unwrap()).is_none());
        assert!(parse_beacon(b"FOLKCRAFT1\na\nb\nnotaport", "1.2.3.4".parse().unwrap()).is_none());
    }

    #[test]
    fn two_links_on_this_machine_talk_to_each_other() {
        let listener = Listener::bind(0).unwrap();
        let client = connect(SocketAddr::new("127.0.0.1".parse().unwrap(), listener.port)).unwrap();
        let mut server = Vec::new();
        for _ in 0..100 {
            server = listener.accept();
            if !server.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let server = server.pop().expect("the host sees the guest arrive");
        client.send(ToHost::Hello { protocol: PROTOCOL, name: "Ana".into() }.encode());
        server.send(ToGuest::Toast("welcome".into()).encode());
        let (mut got_host, mut got_guest) = (Vec::new(), Vec::new());
        for _ in 0..100 {
            got_host.extend(server.poll());
            got_guest.extend(client.poll());
            if !got_host.is_empty() && !got_guest.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(ToHost::decode(&got_host[0]), Some(ToHost::Hello { protocol: PROTOCOL, name: "Ana".into() }));
        assert_eq!(ToGuest::decode(&got_guest[0]), Some(ToGuest::Toast("welcome".into())));
        // Closing one end is noticed at the other.
        drop(client);
        for _ in 0..100 {
            if !server.alive() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!server.alive());
    }
}
