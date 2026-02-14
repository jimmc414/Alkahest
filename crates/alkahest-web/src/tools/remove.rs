use crate::commands;
use alkahest_sim::pipeline::SimPipeline;

/// Execute the remove tool: clear a single voxel at the given position.
pub fn execute(sim: &mut SimPipeline, x: i32, y: i32, z: i32) {
    commands::remove_voxel(sim, x, y, z);
}
