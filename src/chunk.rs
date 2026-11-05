//! Chunk storage.

use macroquad::prelude::Mesh;

use crate::block::Block;
use crate::config::*;

pub fn idx(lx: i32, y: i32, lz: i32) -> usize {
    ((y * CHUNK_SIZE + lz) * CHUNK_SIZE + lx) as usize
}

pub struct Chunk {
    pub blocks: Vec<Block>,
    pub meshes: Vec<Mesh>,
    /// The translucent surfaces of the water, drawn after everything else so the river bed, and whatever
    /// lies on it, shows through.
    pub water: Vec<Mesh>,
    pub dirty: bool,
}
