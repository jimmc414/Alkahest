use alkahest_sim::pipeline::SimPipeline;

/// Encode a place-voxel command and enqueue it.
/// `x, y, z` are local coordinates within the chunk (0..CHUNK_SIZE).
/// `chunk_dispatch_idx` is the dispatch list index for the target chunk.
pub fn place_voxel(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    material_id: u32,
    chunk_dispatch_idx: u32,
) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 1, // TOOL_PLACE
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id,
        chunk_dispatch_idx,
        _pad1: 0,
        _pad2: 0,
    });
}

/// Encode a remove-voxel command and enqueue it.
/// `x, y, z` are local coordinates within the chunk (0..CHUNK_SIZE).
/// `chunk_dispatch_idx` is the dispatch list index for the target chunk.
pub fn remove_voxel(sim: &mut SimPipeline, x: i32, y: i32, z: i32, chunk_dispatch_idx: u32) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 2, // TOOL_REMOVE
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id: 0,
        chunk_dispatch_idx,
        _pad1: 0,
        _pad2: 0,
    });
}
