use alkahest_sim::pipeline::SimPipeline;

/// Encode a place-voxel command and enqueue it.
pub fn place_voxel(sim: &mut SimPipeline, x: i32, y: i32, z: i32, material_id: u32) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 1, // TOOL_PLACE
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    });
}

/// Encode a remove-voxel command and enqueue it.
pub fn remove_voxel(sim: &mut SimPipeline, x: i32, y: i32, z: i32) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 2, // TOOL_REMOVE
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id: 0,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    });
}
