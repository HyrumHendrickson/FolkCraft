//! Chunk meshing: face culling, ambient occlusion and mesh splitting.

use macroquad::prelude::*;

use crate::block::Block;
use crate::chunk::{idx, used_height};
use crate::config::*;
use crate::rng::hash3;
use crate::terrain::berry_bush_is_purple;
use crate::world::{in_render_range, World};

pub struct Face {
    pub dir: (i32, i32, i32),
    pub corners: [(f32, f32, f32); 4],
    pub shade: f32,
}

// Corners are ordered counter-clockwise when viewed from outside the block.
pub const FACES: [Face; 6] = [
    Face { dir: (1, 0, 0),  corners: [(1.,0.,1.), (1.,0.,0.), (1.,1.,0.), (1.,1.,1.)], shade: 0.80 },
    Face { dir: (-1, 0, 0), corners: [(0.,0.,0.), (0.,0.,1.), (0.,1.,1.), (0.,1.,0.)], shade: 0.80 },
    Face { dir: (0, 1, 0),  corners: [(0.,1.,1.), (1.,1.,1.), (1.,1.,0.), (0.,1.,0.)], shade: 1.00 },
    Face { dir: (0, -1, 0), corners: [(0.,0.,0.), (1.,0.,0.), (1.,0.,1.), (0.,0.,1.)], shade: 0.50 },
    Face { dir: (0, 0, 1),  corners: [(0.,0.,1.), (1.,0.,1.), (1.,1.,1.), (0.,1.,1.)], shade: 0.65 },
    Face { dir: (0, 0, -1), corners: [(1.,0.,0.), (0.,0.,0.), (0.,1.,0.), (1.,1.,0.)], shade: 0.65 },
];


impl World {
    /// Rebuild up to `budget` dirty chunk meshes, nearest to `focus` first.
    /// Only chunks within render range are meshed.
    pub fn rebuild_dirty(&mut self, focus: Vec3, budget: usize) {
        let centre = Self::chunk_of_pos(focus);
        let mut dirty: Vec<((i32, i32), f32)> = self
            .chunks
            .iter()
            .filter(|(k, c)| c.dirty && in_render_range(centre, **k))
            .map(|(k, _)| {
                let middle = vec2(
                    (k.0 * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
                    (k.1 * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
                );
                (*k, middle.distance_squared(vec2(focus.x, focus.z)))
            })
            .collect();
        dirty.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (key, _) in dirty.into_iter().take(budget) {
            let (meshes, water) = (self.build_meshes(key.0, key.1), self.build_water_meshes(key.0, key.1));
            if let Some(c) = self.chunks.get_mut(&key) {
                c.meshes = meshes;
                c.water = water;
                c.dirty = false;
            }
        }
    }

    /// Whether the block at (x, y, z) is a full solid cube that hides its neighbours' faces.
    pub fn is_opaque(&self, x: i32, y: i32, z: i32) -> bool {
        self.block_at(x, y, z).opaque()
    }

    /// For a leaf block: which of its -x, +x, -z, +z sides touch a (thin) trunk. A leaf reaches
    /// out toward such a trunk by `TRUNK_INSET`, so no gap shows between leaf and trunk.
    fn leaf_reach(&self, block: Block, x: i32, y: i32, z: i32) -> [bool; 4] {
        if !matches!(block, Block::Leaves | Block::Needles) {
            return [false; 4];
        }
        let wood = |dx: i32, dz: i32| self.block_at(x + dx, y, z + dz) == Block::Wood;
        [wood(-1, 0), wood(1, 0), wood(0, -1), wood(0, 1)]
    }

    /// The opaque meshes of a chunk: everything but the water's own surface.
    pub fn build_meshes(&self, cx: i32, cz: i32) -> Vec<Mesh> {
        let chunk = &self.chunks[&(cx, cz)];
        let mut out = MeshBuilder::new();
        for y in 0..used_height(&chunk.blocks) {
            for lz in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    let block = chunk.blocks[idx(lx, y, lz)];
                    if block == Block::Air || block.is_liquid() {
                        continue;
                    }
                    let (wx, wz) = (cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz);
                    if block.is_full_cube() {
                        self.emit_cube(&mut out, block, wx, y, wz);
                    } else {
                        self.emit_parts(&mut out, block, wx, y, wz);
                    }
                }
            }
        }
        out.finish()
    }

    /// The translucent surfaces of the chunk's water. They are kept apart from the opaque meshes because
    /// they have to be drawn last, over the river bed and whatever else lies behind them.
    pub fn build_water_meshes(&self, cx: i32, cz: i32) -> Vec<Mesh> {
        let chunk = &self.chunks[&(cx, cz)];
        let mut out = MeshBuilder::new();
        for y in 0..used_height(&chunk.blocks) {
            for lz in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    if chunk.blocks[idx(lx, y, lz)].is_wet() {
                        self.emit_water(&mut out, cx * CHUNK_SIZE + lx, y, cz * CHUNK_SIZE + lz);
                    }
                }
            }
        }
        out.finish()
    }

    /// The surface of the water in the cell, a little below its top, and any side that meets neither more water
    /// nor a solid block (the bank hides the rest).
    fn emit_water(&self, out: &mut MeshBuilder, wx: i32, y: i32, wz: i32) {
        let top = 1.0 - WATER_SURFACE_DROP;
        for (fi, face) in FACES.iter().enumerate() {
            let (dx, dy, dz) = face.dir;
            if dy < 0 {
                continue;
            }
            if dy == 0 {
                let next = self.block_at(wx + dx, y, wz + dz);
                if next.is_wet() || next.opaque() {
                    continue;
                }
            }
            let h = hash3(self.seed, wx, y * 8 + fi as i32, wz);
            let k = face.shade * (0.97 + (h % 1000) as f32 / 1000.0 * 0.06);
            // The pools of a mossy cave are green.
            let mossy = self.dimension == crate::horizen::Dimension::Horizen && crate::horizen::in_moss_pool(self.seed, wx, wz);
            let (wr, wg, wb) = if mossy { (0.18, 0.55, 0.40) } else { (0.22, 0.47, 0.75) };
            let color = Color::new(wr * k, wg * k, wb * k, WATER_ALPHA);
            let mut pos = [[0.0_f32; 3]; 4];
            for (ci, &(ox, oy, oz)) in face.corners.iter().enumerate() {
                pos[ci] = [wx as f32 + ox, y as f32 + oy * top, wz as f32 + oz];
            }
            out.quad(pos, [color; 4], false);
        }
    }

    /// A plain cube: culled faces, per-corner ambient occlusion, and (for leaves next to a
    /// trunk) sides pushed out to meet the trunk.
    fn emit_cube(&self, out: &mut MeshBuilder, block: Block, wx: i32, y: i32, wz: i32) {
        let reach = self.leaf_reach(block, wx, y, wz);
        for (fi, face) in FACES.iter().enumerate() {
            let (dx, dy, dz) = face.dir;
            let (nx, ny, nz) = (wx + dx, y + dy, wz + dz);
            if self.is_opaque(nx, ny, nz) {
                // Two leaves with different reach have differently shaped faces, so the face
                // between them may show a sliver and must not be culled.
                let nb = self.block_at(nx, ny, nz);
                let nb_reach = self.leaf_reach(nb, nx, ny, nz);
                let differs = match fi / 2 {
                    0 => reach[2] != nb_reach[2] || reach[3] != nb_reach[3],
                    1 => reach != nb_reach,
                    _ => reach[0] != nb_reach[0] || reach[1] != nb_reach[1],
                };
                if !differs {
                    continue;
                }
            }

            // Base colour, with a few per-block visual tweaks.
            let h = hash3(self.seed, wx, y * 8 + fi as i32, wz);
            let (mut r, mut g, mut b) = block.base_color();
            match block {
                // The ground is grassy on top, except on a river or lake bed, where it is mud under the water.
                // (Dirt is never covered by anything but air, plants, items and water, so its exposed top is
                // always the top of the ground.)
                Block::Dirt if fi == 2 && self.block_at(wx, y + 1, wz).is_wet() => (r, g, b) = (0.40, 0.31, 0.22),
                // The floor round a mossy cave's pools is moss, and the rock of a mossy cave is green.
                Block::Dirt if fi == 2 && self.dimension == crate::horizen::Dimension::Horizen && crate::horizen::moss_at(self.seed, wx, wz) => (r, g, b) = (0.20, 0.46, 0.16),
                Block::Stone if self.dimension == crate::horizen::Dimension::Horizen && crate::horizen::mossy_wall(self.seed, wx, wz) && h % 100 < 70 => (r, g, b) = (0.28, 0.44, 0.26),
                // The floor of the Horizen's cave is bare earth, not grass.
                Block::Dirt if fi == 2 && self.dimension == crate::horizen::Dimension::Horizen => (r, g, b) = (0.38, 0.34, 0.30),
                // The Skylands' islands are grass all the way up, whatever the height.
                Block::Dirt if fi == 2 && self.dimension == crate::horizen::Dimension::Skylands => (r, g, b) = (0.30, 0.62, 0.22),
                // Above the tree line the mountain is bare rock, not grass.
                Block::Dirt if fi == 2 && y > GROUND_TOP + TREE_LINE_RISE => (r, g, b) = (0.46, 0.43, 0.40),
                Block::Dirt if fi == 2 => (r, g, b) = (0.28, 0.58, 0.20),
                // Night makes the tree canopy look lifeless instead of merely turning a green tree dark.
                Block::Leaves if self.night => (r, g, b) = (0.25, 0.23, 0.17),
                Block::Needles if self.night => (r, g, b) = (0.19, 0.21, 0.16),
                Block::Leaves if (h >> 20) % 100 < 30 => (r, g, b) = (0.24, 0.62, 0.20),
                Block::Needles if (h >> 20) % 100 < 30 => (r, g, b) = (0.12, 0.44, 0.27),
                _ => {}
            }
            let jitter = 0.94 + (h % 1000) as f32 / 1000.0 * 0.12;
            let shade = face.shade * jitter;

            // Per-corner ambient occlusion for soft, smooth shading.
            let mut ao = [3u8; 4];
            let mut colors = [WHITE; 4];
            let mut pos = [[0.0_f32; 3]; 4];
            for (ci, &(ox, oy, oz)) in face.corners.iter().enumerate() {
                let sx = if ox > 0.5 { 1 } else { -1 };
                let sy = if oy > 0.5 { 1 } else { -1 };
                let sz = if oz > 0.5 { 1 } else { -1 };
                let (u, v) = match fi / 2 {
                    0 => ((0, sy, 0), (0, 0, sz)),
                    1 => ((sx, 0, 0), (0, 0, sz)),
                    _ => ((sx, 0, 0), (0, sy, 0)),
                };
                let s1 = self.is_opaque(nx + u.0, ny + u.1, nz + u.2);
                let s2 = self.is_opaque(nx + v.0, ny + v.1, nz + v.2);
                let c = self.is_opaque(nx + u.0 + v.0, ny + u.1 + v.1, nz + u.2 + v.2);
                ao[ci] = if s1 && s2 { 0 } else { 3 - (s1 as u8 + s2 as u8 + c as u8) };
                let k = shade * AO_LEVELS[ao[ci] as usize];
                colors[ci] = Color::new((r * k).min(1.0), (g * k).min(1.0), (b * k).min(1.0), 1.0);

                let mut px = wx as f32 + ox;
                let mut pz = wz as f32 + oz;
                if ox < 0.5 && reach[0] { px -= TRUNK_INSET; }
                if ox > 0.5 && reach[1] { px += TRUNK_INSET; }
                if oz < 0.5 && reach[2] { pz -= TRUNK_INSET; }
                if oz > 0.5 && reach[3] { pz += TRUNK_INSET; }
                pos[ci] = [px, y as f32 + oy, pz];
            }
            // Split the quad along its brighter diagonal to avoid shading artefacts.
            out.quad(pos, colors, ao[0] + ao[2] < ao[1] + ao[3]);
        }
    }

    /// A block drawn from boxes (trunk, plants, placed items): flat shading, and a face
    /// is skipped only where it lies on the cell boundary against a solid cube.
    fn emit_parts(&self, out: &mut MeshBuilder, block: Block, wx: i32, y: i32, wz: i32) {
        for (pi, part) in block.parts().iter().enumerate() {
            if block == Block::Bush && pi >= 2 && self.berry_picked.contains(&(wx, y, wz)) {
                continue;
            }
            for (fi, face) in FACES.iter().enumerate() {
                let (dx, dy, dz) = face.dir;
                let (axis, positive) = (fi / 2, fi % 2 == 0);
                let on_boundary = if positive { part.max[axis] >= 1.0 } else { part.min[axis] <= 0.0 };
                if on_boundary && self.is_opaque(wx + dx, y + dy, wz + dz) {
                    continue;
                }
                // A leaf reaches out to meet the trunk, which hides the trunk's side there.
                if block == Block::Wood && axis != 1 {
                    if matches!(self.block_at(wx + dx, y, wz + dz), Block::Leaves | Block::Needles) {
                        continue;
                    }
                }

                let h = hash3(self.seed, wx, y * 8 + fi as i32, wz);
                let mut color = part.color;
                match block {
                    Block::Wood if fi == 2 => color = (0.62, 0.46, 0.26), // cut-log top
                    Block::PlacedLog if axis == 0 => color = (0.62, 0.46, 0.26), // cut ends
                    Block::Fire if axis == 0 => color = (0.85, 0.32, 0.08), // glowing ends
                    Block::Flower if pi == 1 => color = FLOWER_COLORS[((hash3(self.seed, wx, y, wz) >> 8) % 4) as usize],
                    Block::Bush if pi >= 2 => {
                        let purple = berry_bush_is_purple(self.seed, wx, wz) ^ self.night;
                        color = if purple { (0.48, 0.12, 0.76) } else { (0.75, 0.12, 0.20) };
                    }
                    _ => {}
                }
                let k = face.shade * (0.94 + (h % 1000) as f32 / 1000.0 * 0.12);
                let col = Color::new((color.0 * k).min(1.0), (color.1 * k).min(1.0), (color.2 * k).min(1.0), 1.0);
                let mut pos = [[0.0_f32; 3]; 4];
                for (ci, &(ox, oy, oz)) in face.corners.iter().enumerate() {
                    pos[ci] = [
                        wx as f32 + part.min[0] + ox * (part.max[0] - part.min[0]),
                        y as f32 + part.min[1] + oy * (part.max[1] - part.min[1]),
                        wz as f32 + part.min[2] + oz * (part.max[2] - part.min[2]),
                    ];
                }
                out.quad(pos, [col; 4], false);
            }
        }
    }
}

/// Petal colours; each flower picks one by position.
const FLOWER_COLORS: [(f32, f32, f32); 4] =
    [(0.90, 0.25, 0.30), (0.95, 0.85, 0.25), (0.95, 0.95, 0.95), (0.65, 0.40, 0.85)];

/// Collects quads into meshes, starting a new mesh before `MAX_QUADS_PER_MESH` is exceeded.
struct MeshBuilder {
    meshes: Vec<Mesh>,
    verts: Vec<Vertex>,
    inds: Vec<u16>,
}

impl MeshBuilder {
    fn new() -> Self {
        MeshBuilder { meshes: Vec::new(), verts: Vec::new(), inds: Vec::new() }
    }

    /// Add one quad (corners counter-clockwise seen from outside). `flip` chooses the other diagonal.
    fn quad(&mut self, pos: [[f32; 3]; 4], colors: [Color; 4], flip: bool) {
        if self.verts.len() / 4 >= MAX_QUADS_PER_MESH {
            self.flush();
        }
        let base = self.verts.len() as u16;
        for i in 0..4 {
            self.verts.push(Vertex::new(pos[i][0], pos[i][1], pos[i][2], 0.0, 0.0, colors[i]));
        }
        if flip {
            self.inds.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        } else {
            self.inds.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    fn flush(&mut self) {
        if !self.verts.is_empty() {
            self.meshes.push(Mesh {
                vertices: std::mem::take(&mut self.verts),
                indices: std::mem::take(&mut self.inds),
                texture: None,
            });
        }
    }

    fn finish(mut self) -> Vec<Mesh> {
        self.flush();
        self.meshes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn meshes_build() {
        let mut w = World::new(1, HashSet::new());
        w.stream_chunks(vec3(10.0, 6.0, 10.0));
        w.rebuild_dirty(vec3(10.0, 6.0, 10.0), usize::MAX);
        assert!(!w.chunks[&(0, 0)].meshes.is_empty());
    }

    #[test]
    fn mesh_budget_is_respected() {
        let mut w = World::new(1, HashSet::new());
        let pos = vec3(10.0, 6.0, 10.0);
        w.stream_chunks(pos);
        let centre = World::chunk_of_pos(pos);
        let pending = |w: &World| w.chunks.iter().filter(|(k, c)| c.dirty && in_render_range(centre, **k)).count();
        let render_side = (2 * render_radius() + 1) as usize;
        assert_eq!(pending(&w), render_side * render_side);
        w.rebuild_dirty(pos, 2);
        assert_eq!(pending(&w), render_side * render_side - 2);
        w.rebuild_dirty(pos, usize::MAX);
        assert_eq!(pending(&w), 0);
    }

    #[test]
    fn meshes_fit_draw_call_limits_and_keep_tree_tops() {
        // Macroquad's default per-draw-call capacity.
        const MAX_VERTS: usize = 10_000;
        const MAX_INDS: usize = 5_000;
        for seed in 0..6 {
            let mut w = World::new(seed, HashSet::new());
            for cz in 0..2 {
                for cx in 0..2 {
                    w.ensure_chunk(cx, cz);
                }
            }
            for cz in 0..2 {
                for cx in 0..2 {
                    let meshes = w.build_meshes(cx, cz);
                    let mut top = 0.0_f32;
                    let mut quads = 0;
                    for m in &meshes {
                        assert!(m.vertices.len() < MAX_VERTS, "{} verts", m.vertices.len());
                        assert!(m.indices.len() < MAX_INDS, "{} indices", m.indices.len());
                        quads += m.vertices.len() / 4;
                        for v in &m.vertices {
                            top = top.max(v.position.y);
                        }
                    }
                    // Regression: leaves are the highest blocks and used to be truncated away.
                    let b = &w.chunks[&(cx, cz)].blocks;
                    let mut highest_foliage = 0;
                    for y in 0..WORLD_HEIGHT {
                        for lz in 0..CHUNK_SIZE {
                            for lx in 0..CHUNK_SIZE {
                                if matches!(b[idx(lx, y, lz)], Block::Leaves | Block::Needles) {
                                    highest_foliage = y;
                                }
                            }
                        }
                    }
                    assert!(top >= highest_foliage as f32 + 1.0 - 1e-3, "top {top} < foliage {highest_foliage}");
                    assert!(quads > 0);
                }
            }
        }
    }

    #[test]
    fn large_chunks_are_actually_split() {
        let mut w = World::new(3, HashSet::new());
        w.ensure_chunk(0, 0);
        let meshes = w.build_meshes(0, 0);
        let quads: usize = meshes.iter().map(|m| m.vertices.len() / 4).sum();
        assert!(quads > MAX_QUADS_PER_MESH, "test chunk should exceed one mesh ({quads} quads)");
        assert!(meshes.len() >= 2);
    }

    /// Chunk (0, 0) with nothing above the ground, so blocks can be placed exactly.
    fn sky_world() -> World {
        let mut w = World::new(1, HashSet::new());
        w.ensure_chunk(0, 0);
        let c = w.chunks.get_mut(&(0, 0)).unwrap();
        for y in GROUND_TOP + 1..WORLD_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    c.blocks[idx(x, y, z)] = Block::Air;
                }
            }
        }
        w
    }

    fn put(w: &mut World, x: i32, y: i32, z: i32, b: Block) {
        w.chunks.get_mut(&(0, 0)).unwrap().blocks[idx(x, y, z)] = b;
    }

    fn vertices(w: &World) -> Vec<Vec3> {
        w.build_meshes(0, 0).iter().flat_map(|m| m.vertices.iter().map(|v| v.position)).collect()
    }

    /// Positions of the vertices lying inside the cell `(cx, cy, cz)` (borders included).
    fn in_cell(vs: &[Vec3], cx: i32, cy: i32, cz: i32) -> Vec<Vec3> {
        let (lo, hi) = (vec3(cx as f32, cy as f32, cz as f32), vec3(cx as f32 + 1.0, cy as f32 + 1.0, cz as f32 + 1.0));
        vs.iter().copied().filter(|v| v.cmpge(lo - Vec3::splat(1e-4)).all() && v.cmple(hi + Vec3::splat(1e-4)).all()).collect()
    }

    #[test]
    fn a_trunk_is_thinner_than_a_block() {
        let mut w = sky_world();
        put(&mut w, 5, 6, 5, Block::Wood);
        let vs = in_cell(&vertices(&w), 5, 6, 5);
        // Everything of the trunk's own lies inside 5.15..5.85 (the dirt below is at y <= 6).
        let trunk: Vec<_> = vs.iter().filter(|v| v.y > 6.0 + 1e-4 || (v.x > 5.01 && v.x < 5.99)).collect();
        assert!(!trunk.is_empty());
        for v in trunk {
            assert!(v.x >= 5.0 + TRUNK_INSET - 1e-4 && v.x <= 6.0 - TRUNK_INSET + 1e-4, "x = {}", v.x);
            assert!(v.z >= 5.0 + TRUNK_INSET - 1e-4 && v.z <= 6.0 - TRUNK_INSET + 1e-4, "z = {}", v.z);
        }
        // The dirt top face under the trunk is still drawn: a ring of it shows around the base.
        let ground = vertices(&w).into_iter().filter(|v| (v.y - (GROUND_TOP as f32 + 1.0)).abs() < 1e-4);
        assert!(ground.clone().any(|v| (v.x - 5.0).abs() < 1e-4 && (v.z - 5.0).abs() < 1e-4));
    }

    #[test]
    fn leaves_reach_in_to_meet_a_neighbouring_trunk_on_every_side() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for leaf in [Block::Leaves, Block::Needles] {
                let mut w = sky_world();
                let (tx, tz) = (8, 8);
                put(&mut w, tx, 8, tz, Block::Wood);
                put(&mut w, tx + dx, 8, tz + dz, leaf);
                let (lx, lz) = (tx + dx, tz + dz);
                // The leaf's own vertices: the ones at the leaf cell's boundaries across the
                // trunk's direction (the trunk's vertices are inset from them).
                let leaf_vs: Vec<Vec3> = vertices(&w)
                    .into_iter()
                    .filter(|v| v.y > 8.0 - 1e-4 && v.y < 9.0 + 1e-4)
                    .filter(|v| {
                        let across = if dx != 0 { v.z } else { v.x };
                        let base = if dx != 0 { lz as f32 } else { lx as f32 };
                        near(across, base) || near(across, base + 1.0)
                    })
                    .collect();
                assert!(!leaf_vs.is_empty());
                // The face toward the trunk is at the trunk's surface: pulled TRUNK_INSET into its cell.
                let (face, trunk_surface) = match (dx, dz) {
                    (1, 0) => (leaf_vs.iter().map(|v| v.x).fold(f32::MAX, f32::min), tx as f32 + 1.0 - TRUNK_INSET),
                    (-1, 0) => (leaf_vs.iter().map(|v| v.x).fold(f32::MIN, f32::max), tx as f32 + TRUNK_INSET),
                    (0, 1) => (leaf_vs.iter().map(|v| v.z).fold(f32::MAX, f32::min), tz as f32 + 1.0 - TRUNK_INSET),
                    _ => (leaf_vs.iter().map(|v| v.z).fold(f32::MIN, f32::max), tz as f32 + TRUNK_INSET),
                };
                assert!(near(face, trunk_surface), "{leaf:?} at ({dx},{dz}): leaf face {face}, trunk surface {trunk_surface}");
            }
        }
    }

    #[test]
    fn leaves_away_from_a_trunk_stay_full_blocks() {
        let mut w = sky_world();
        put(&mut w, 8, 8, 8, Block::Wood);
        put(&mut w, 12, 8, 12, Block::Leaves); // far from the trunk
        put(&mut w, 9, 9, 9, Block::Leaves); // diagonal to the trunk's cell (touching only at an edge)
        let vs = vertices(&w);
        for (x, y, z) in [(12, 8, 12), (9, 9, 9)] {
            let cell = in_cell(&vs, x, y, z);
            assert!(cell.iter().all(|v| v.x >= x as f32 - 1e-4 && v.x <= x as f32 + 1.0 + 1e-4
                && v.z >= z as f32 - 1e-4 && v.z <= z as f32 + 1.0 + 1e-4));
            let (min_x, max_x) = cell.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v.x), b.max(v.x)));
            assert!((min_x - x as f32).abs() < 1e-4 && (max_x - (x as f32 + 1.0)).abs() < 1e-4, "leaf at {x},{z}");
        }
    }

    #[test]
    fn a_leaf_beside_a_trunk_and_a_plain_leaf_leave_no_sliver_between_them() {
        // Trunk at (8,8); leaf A at (9,8) reaches toward it; leaf B at (9,9) does not. A's +z face
        // is wider than B's -z face, so the face between them must still be drawn.
        let mut w = sky_world();
        put(&mut w, 8, 8, 8, Block::Wood);
        put(&mut w, 9, 8, 8, Block::Leaves);
        put(&mut w, 9, 8, 9, Block::Leaves);
        let vs = vertices(&w);
        let sliver = vs.iter().any(|v| (v.z - 9.0).abs() < 1e-4 && (v.x - (9.0 - TRUNK_INSET)).abs() < 1e-4 && v.y >= 8.0 - 1e-4 && v.y <= 9.0 + 1e-4);
        assert!(sliver, "the extended side of the first leaf must have a face against the second");
    }

    #[test]
    fn plants_and_small_items_are_drawn_small() {
        for (block, max_h, max_w) in [
            (Block::Grass, 0.7, 0.9),
            (Block::Flower, 0.7, 0.4),
            (Block::Bush, 0.8, 0.9),
            (Block::PlacedPebble, 0.15, 0.5),
            (Block::PlacedStone, 0.4, 0.6),
            (Block::Charcoal, 0.4, 0.8),
            (Block::PlacedLog, 0.5, 0.95),
        ] {
            let mut w = sky_world();
            put(&mut w, 8, 6, 8, block);
            // Skip the dirt top below (its corners are the only vertices on whole-number x and z).
            let whole = |a: f32| (a - a.round()).abs() < 1e-4;
            let vs: Vec<_> = vertices(&w)
                .into_iter()
                .filter(|v| v.y > GROUND_TOP as f32 + 1.0 - 1e-4 && v.x > 7.5 && v.x < 9.5 && v.z > 7.5 && v.z < 9.5)
                .filter(|v| !(whole(v.x) && whole(v.z)))
                .collect();
            assert!(!vs.is_empty(), "{block:?} draws something");
            let top = vs.iter().map(|v| v.y).fold(f32::MIN, f32::max) - 6.0;
            let (lo_x, hi_x) = vs.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v.x), b.max(v.x)));
            assert!(top <= max_h + 1e-3, "{block:?} height {top}");
            assert!(hi_x - lo_x <= max_w + 1e-3, "{block:?} width {}", hi_x - lo_x);
        }
    }

    fn water_vertices(w: &World) -> Vec<Vertex> {
        w.build_water_meshes(0, 0).iter().flat_map(|m| m.vertices.clone()).collect()
    }

    #[test]
    fn water_is_a_translucent_surface_a_little_below_the_ground_with_no_wall_between_neighbours() {
        let mut w = sky_world();
        for x in [8, 9] {
            put(&mut w, x, GROUND_TOP, 8, Block::Water);
        }
        let vs = water_vertices(&w);
        let surface = GROUND_TOP as f32 + 1.0 - WATER_SURFACE_DROP;
        assert_eq!(vs.len(), 2 * 4, "one surface quad per water block and no sides: the banks and the bed hide them");
        assert!(vs.iter().all(|v| (v.position.y - surface).abs() < 1e-4));
        assert!(vs.iter().all(|v| v.position.x >= 8.0 - 1e-4 && v.position.x <= 10.0 + 1e-4 && v.position.z >= 8.0 - 1e-4 && v.position.z <= 9.0 + 1e-4));
        assert!(vs.iter().all(|v| v.color[3] > 0 && (v.color[3] as f32) < 255.0 * 0.75), "see-through: alpha {}", vs[0].color[3]);
        // None of it is in the opaque meshes, which are drawn first and must not hide the bed.
        assert!(!vertices(&w).iter().any(|v| (v.y - surface).abs() < 1e-4));
    }

    #[test]
    fn the_river_bed_under_the_water_is_drawn_as_mud() {
        let mut w = sky_world();
        let bed = GROUND_TOP as f32; // the bed's top face lies at the bottom of the water cell
        put(&mut w, 8, GROUND_TOP, 8, Block::Water);
        let q = w.build_meshes(0, 0).iter().flat_map(|m| m.vertices.chunks(4).map(|c| c.to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>();
        let top = q
            .iter()
            .find(|c| c.iter().all(|v| (v.position.y - bed).abs() < 1e-4 && v.position.x >= 7.99 && v.position.x <= 9.01 && v.position.z >= 7.99 && v.position.z <= 9.01))
            .expect("the bed shows through the water");
        let (r, g) = (top[0].color[0], top[0].color[1]);
        assert!(r > g, "mud brown, not grass green ({r}, {g})");
        // Dry ground keeps its grass.
        let dry = q.iter().find(|c| c.iter().all(|v| (v.position.y - (bed + 1.0)).abs() < 1e-4 && v.position.x >= 3.99 && v.position.x <= 5.01 && v.position.z >= 3.99 && v.position.z <= 5.01)).unwrap();
        assert!(dry[0].color[1] > dry[0].color[0]);
    }

    #[test]
    fn things_on_the_bed_are_opaque_meshes_under_a_water_surface_of_their_own_cell() {
        for thing in [Block::WetClay, Block::WetPebble, Block::WetStone] {
            let mut w = sky_world();
            put(&mut w, 8, GROUND_TOP, 8, thing);
            let surface = GROUND_TOP as f32 + 1.0 - WATER_SURFACE_DROP;
            assert_eq!(water_vertices(&w).len(), 4, "{thing:?} lies in water, which is drawn over it");
            let lump: Vec<_> = vertices(&w).into_iter().filter(|v| v.x > 7.5 && v.x < 9.5 && v.z > 7.5 && v.z < 9.5 && v.y > GROUND_TOP as f32 + 1e-3 && v.y < GROUND_TOP as f32 + 1.0).collect();
            assert!(!lump.is_empty() && lump.iter().all(|v| v.y < surface), "{thing:?} is drawn, below the surface");
        }
    }

    #[test]
    fn a_chunk_keeps_its_water_apart_and_forgets_it_with_its_meshes() {
        let mut w = World::new(3, HashSet::new());
        let (cx, cz) = (-12..12).flat_map(|z| (-12..12).map(move |x| (x, z))).find(|&(cx, cz)| {
            (0..CHUNK_SIZE).any(|lz| (0..CHUNK_SIZE).any(|lx| crate::river::is_river(3, cx * CHUNK_SIZE + lx, cz * CHUNK_SIZE + lz)))
        }).expect("a river chunk");
        let pos = vec3((cx * CHUNK_SIZE + 10) as f32, 6.0, (cz * CHUNK_SIZE + 10) as f32);
        w.stream_chunks(pos);
        w.rebuild_dirty(pos, usize::MAX);
        assert!(!w.chunks[&(cx, cz)].water.is_empty() && !w.chunks[&(cx, cz)].meshes.is_empty());
        w.stream_chunks(vec3(pos.x + 2000.0, 6.0, pos.z));
        assert!(w.chunks[&(cx, cz)].water.is_empty() && w.chunks[&(cx, cz)].meshes.is_empty());
    }

    #[test]
    fn plants_do_not_hide_the_ground_beneath_or_beside_them() {
        let mut w = sky_world();
        put(&mut w, 8, 6, 8, Block::Grass);
        let vs = vertices(&w);
        // The dirt top at (8,5,8) still has its face (drawn at y = 6) because grass is not opaque.
        assert!(vs.iter().any(|v| (v.y - 6.0).abs() < 1e-4 && (v.x - 8.0).abs() < 1e-4 && (v.z - 8.0).abs() < 1e-4));
    }

    #[test]
    fn meshes_with_lots_of_parts_still_split_below_the_draw_call_limit() {
        let mut w = sky_world();
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                put(&mut w, x, 6, z, if (x + z) % 2 == 0 { Block::Bush } else { Block::Flower });
            }
        }
        let meshes = w.build_meshes(0, 0);
        assert!(meshes.len() >= 2);
        for m in &meshes {
            assert!(m.vertices.len() < 10_000 && m.indices.len() < 5_000);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        }
    }
}
