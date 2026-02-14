// Cross-chunk coordinate utilities for multi-chunk world (M5).
// Included by all sim and render shaders via concatenation.
// Reads from chunk_descriptors and pool buffers declared in the including shader.
//
// Constants injected by build preamble:
//   CHUNK_SIZE, CHUNK_DESC_STRIDE, SENTINEL_NEIGHBOR,
//   WORLD_CHUNKS_X, WORLD_CHUNKS_Y, WORLD_CHUNKS_Z

/// Linear voxel index within a chunk (unchanged from M1).
fn voxel_index(pos: vec3<i32>) -> u32 {
    return u32(pos.x) + u32(pos.y) * CHUNK_SIZE + u32(pos.z) * CHUNK_SIZE * CHUNK_SIZE;
}

/// Check if position is within a single chunk [0, CHUNK_SIZE).
fn in_bounds(pos: vec3<i32>) -> bool {
    return pos.x >= 0 && pos.x < i32(CHUNK_SIZE)
        && pos.y >= 0 && pos.y < i32(CHUNK_SIZE)
        && pos.z >= 0 && pos.z < i32(CHUNK_SIZE);
}

/// Extract chunk index from z-batched dispatch global_invocation_id.
/// Dispatch z = active_chunk_count * (CHUNK_SIZE / 4u).
/// Each chunk gets CHUNK_SIZE/4 z-slices.
fn chunk_index_from_gid(gid: vec3<u32>) -> u32 {
    return gid.z / (CHUNK_SIZE / 4u);
}

/// Extract local z coordinate within a chunk from batched dispatch.
fn chunk_local_z(gid: vec3<u32>) -> u32 {
    return gid.z % (CHUNK_SIZE / 4u);
}

/// Compute the linear index into the pool buffer for a voxel.
/// chunk_idx is the index into the chunk descriptor array (dispatch order).
/// pos is the local [0, CHUNK_SIZE) coordinate.
fn pool_voxel_index(pos: vec3<i32>, chunk_idx: u32) -> u32 {
    let slot_offset = chunk_descriptors[chunk_idx * CHUNK_DESC_STRIDE];
    // slot_offset is in bytes; each voxel is 8 bytes = 2 u32s
    return (slot_offset / 8u) + voxel_index(pos);
}

/// Compute which of the 26 neighbor directions an out-of-bounds position maps to.
/// Returns an index in [0, 25] matching the neighbor_pool_slot_offsets layout.
///
/// Neighbor index layout (matches alkahest_core::direction::Direction ordering):
///   The 26 directions are enumerated as: for dz in {-1,0,1} for dy in {-1,0,1} for dx in {-1,0,1},
///   skipping (0,0,0). This gives a deterministic mapping.
///
/// We use a different scheme here for clarity:
///   index = (dz+1)*9 + (dy+1)*3 + (dx+1), then subtract 1 for indices > 13 (the center).
fn compute_neighbor_dir(pos: vec3<i32>) -> u32 {
    var dx: i32 = 0;
    var dy: i32 = 0;
    var dz: i32 = 0;

    if pos.x < 0 { dx = -1; }
    else if pos.x >= i32(CHUNK_SIZE) { dx = 1; }

    if pos.y < 0 { dy = -1; }
    else if pos.y >= i32(CHUNK_SIZE) { dy = 1; }

    if pos.z < 0 { dz = -1; }
    else if pos.z >= i32(CHUNK_SIZE) { dz = 1; }

    // Flat index in 3×3×3 grid, center (0,0,0) = index 13
    let flat = u32(dz + 1) * 9u + u32(dy + 1) * 3u + u32(dx + 1);
    // Skip center: indices 0-12 stay, 14-26 become 13-25
    if flat < 13u {
        return flat;
    }
    return flat - 1u;
}

/// Remap out-of-bounds coordinates into [0, CHUNK_SIZE) for the neighbor chunk.
fn remap_coords(pos: vec3<i32>) -> vec3<i32> {
    let cs = i32(CHUNK_SIZE);
    return vec3<i32>(
        ((pos.x % cs) + cs) % cs,
        ((pos.y % cs) + cs) % cs,
        ((pos.z % cs) + cs) % cs,
    );
}

/// Read a voxel from the read pool, handling cross-chunk access.
/// pos is in local coordinates (may be outside [0, CHUNK_SIZE) for neighbor access).
/// chunk_idx is the dispatch-order index of the current chunk.
/// Returns vec2<u32> packed voxel data (air sentinel = vec2(0,0) for unloaded neighbors).
fn cross_chunk_voxel(pos: vec3<i32>, chunk_idx: u32) -> vec2<u32> {
    if in_bounds(pos) {
        // Same chunk — direct local access
        let idx = pool_voxel_index(pos, chunk_idx);
        return read_pool[idx];
    }

    // Determine which neighbor direction
    let neighbor_dir = compute_neighbor_dir(pos);
    let neighbor_slot_offset = chunk_descriptors[chunk_idx * CHUNK_DESC_STRIDE + 1u + neighbor_dir];

    // Sentinel means neighbor is unloaded — return air
    if neighbor_slot_offset == SENTINEL_NEIGHBOR {
        return vec2<u32>(0u, 0u);
    }

    // Remap coordinates into neighbor chunk's local space
    let remapped = remap_coords(pos);
    let idx = (neighbor_slot_offset / 8u) + voxel_index(remapped);
    return read_pool[idx];
}

/// Read a voxel from the write pool for the current chunk only (no cross-chunk).
/// Used when writing to the current chunk's slot.
fn write_pool_voxel_index(pos: vec3<i32>, chunk_idx: u32) -> u32 {
    let slot_offset = chunk_descriptors[chunk_idx * CHUNK_DESC_STRIDE];
    return (slot_offset / 8u) + voxel_index(pos);
}
