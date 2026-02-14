use crate::commands;
use alkahest_sim::pipeline::SimPipeline;

/// Execute the heat tool: raise temperature of existing voxels at the given local position.
/// `x, y, z` are local coords within the chunk (0..CHUNK_SIZE).
/// `chunk_dispatch_idx` is the dispatch list index for the target chunk.
#[allow(clippy::too_many_arguments)]
pub fn execute_heat(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    temp_delta: i32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
) {
    commands::heat_voxel(
        sim,
        x,
        y,
        z,
        temp_delta,
        chunk_dispatch_idx,
        brush_radius,
        brush_shape,
    );
}
