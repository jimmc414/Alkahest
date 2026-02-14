use crate::commands;
use alkahest_sim::pipeline::SimPipeline;

/// Execute the push tool: apply directional velocity to voxels at the given position.
/// `dir_x, dir_y, dir_z` are the velocity deltas to apply (each -127..127).
pub fn execute_push(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    dir_x: i32,
    dir_y: i32,
    dir_z: i32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
) {
    commands::push_voxel(
        sim, x, y, z, dir_x, dir_y, dir_z, chunk_dispatch_idx, brush_radius, brush_shape,
    );
}
