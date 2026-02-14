use crate::commands;
use alkahest_sim::pipeline::SimPipeline;

/// Execute the place tool: add a single voxel at the given position.
pub fn execute(sim: &mut SimPipeline, x: i32, y: i32, z: i32, material_id: u32) {
    commands::place_voxel(sim, x, y, z, material_id);
}
