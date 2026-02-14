use crate::commands;
use alkahest_sim::pipeline::SimPipeline;

/// Execute the place tool: add voxels at the given local position with brush settings.
/// `x, y, z` are local coords within the chunk (0..CHUNK_SIZE).
/// `chunk_dispatch_idx` is the dispatch list index for the target chunk.
pub fn execute(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    material_id: u32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
) {
    commands::place_voxel(sim, x, y, z, material_id, chunk_dispatch_idx, brush_radius, brush_shape);
}
