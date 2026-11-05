//! Chunk storage.

use macroquad::prelude::Mesh;

use crate::block::Block;
use crate::config::*;

pub fn idx(lx: i32, y: i32, lz: i32) -> usize {
    ((y * CHUNK_SIZE + lz) * CHUNK_SIZE + lx) as usize
}

/// Number of layers of a chunk's blocks that can hold anything: one more than the highest layer with a block in it.
/// The world is tall (`WORLD_HEIGHT`) but most chunks are nearly all air above the trees, and the mesher stops there.
pub fn used_height(blocks: &[Block]) -> i32 {
    let layer = (CHUNK_SIZE * CHUNK_SIZE) as usize;
    (0..WORLD_HEIGHT).rev().find(|&y| blocks[y as usize * layer..(y as usize + 1) * layer].iter().any(|&b| b != Block::Air)).map_or(0, |y| y + 1)
}

pub struct Chunk {
    pub blocks: Vec<Block>,
    pub meshes: Vec<Mesh>,
    /// The translucent surfaces of the water, drawn after everything else so the river bed, and whatever
    /// lies on it, shows through.
    pub water: Vec<Mesh>,
    pub dirty: bool,
}
