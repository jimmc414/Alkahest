use alkahest_sim::pipeline::SimPipeline;

/// Execute the heat tool: raise temperature of existing voxels at the given local position.
/// `x, y, z` are local coords within the chunk (0..CHUNK_SIZE).
/// `chunk_dispatch_idx` is the dispatch list index for the target chunk.
pub fn execute_heat(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    temp_delta: i32,
    chunk_dispatch_idx: u32,
) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 3, // TOOL_HEAT
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id: temp_delta as u32, // reused as signed temp delta (bitcast in shader)
        chunk_dispatch_idx,
        _pad1: 0,
        _pad2: 0,
    });
}
