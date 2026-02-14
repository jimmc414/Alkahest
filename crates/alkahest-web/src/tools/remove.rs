use crate::commands;
use alkahest_sim::pipeline::SimPipeline;

/// Execute the remove tool: clear a single voxel at the given local position.
/// `x, y, z` are local coords within the chunk (0..CHUNK_SIZE).
/// `chunk_dispatch_idx` is the dispatch list index for the target chunk.
pub fn execute(sim: &mut SimPipeline, x: i32, y: i32, z: i32, chunk_dispatch_idx: u32) {
    commands::remove_voxel(sim, x, y, z, chunk_dispatch_idx);
}
