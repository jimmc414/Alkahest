use alkahest_sim::pipeline::SimPipeline;

/// Encode a place-voxel command and enqueue it.
/// `x, y, z` are local coordinates within the chunk (0..CHUNK_SIZE).
/// `brush_radius`: 0 = single voxel, 1–16 for area brushes.
/// `brush_shape`: 0 = single, 1 = cube, 2 = sphere.
#[allow(clippy::too_many_arguments)]
pub fn place_voxel(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    material_id: u32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 1, // TOOL_PLACE
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id,
        chunk_dispatch_idx,
        brush_radius,
        brush_shape,
    });
}

/// Encode a remove-voxel command and enqueue it.
/// `x, y, z` are local coordinates within the chunk (0..CHUNK_SIZE).
pub fn remove_voxel(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 2, // TOOL_REMOVE
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id: 0,
        chunk_dispatch_idx,
        brush_radius,
        brush_shape,
    });
}

/// Encode a heat command and enqueue it.
#[allow(clippy::too_many_arguments)]
pub fn heat_voxel(
    sim: &mut SimPipeline,
    x: i32,
    y: i32,
    z: i32,
    temp_delta: i32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
) {
    use alkahest_sim::pipeline::SimCommand;
    sim.enqueue_command(SimCommand {
        tool_type: 3, // TOOL_HEAT
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id: temp_delta as u32,
        chunk_dispatch_idx,
        brush_radius,
        brush_shape,
    });
}

/// Encode a push command and enqueue it.
/// Direction is packed as 3 biased-128 i8 values: dx | (dy << 8) | (dz << 16).
#[allow(clippy::too_many_arguments)]
pub fn push_voxel(
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
    use alkahest_sim::pipeline::SimCommand;
    let dx_biased = ((dir_x + 128).clamp(0, 255) as u32) & 0xFF;
    let dy_biased = ((dir_y + 128).clamp(0, 255) as u32) & 0xFF;
    let dz_biased = ((dir_z + 128).clamp(0, 255) as u32) & 0xFF;
    let dir_packed = dx_biased | (dy_biased << 8) | (dz_biased << 16);
    sim.enqueue_command(SimCommand {
        tool_type: 4, // TOOL_PUSH
        pos_x: x,
        pos_y: y,
        pos_z: z,
        material_id: dir_packed,
        chunk_dispatch_idx,
        brush_radius,
        brush_shape,
    });
}
