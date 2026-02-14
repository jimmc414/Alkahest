// coords.wgsl — Voxel coordinate helpers.
// CHUNK_SIZE and VOXELS_PER_CHUNK are injected at runtime from alkahest-core constants.

fn voxel_index(pos: vec3<i32>) -> u32 {
    return u32(pos.x) + u32(pos.y) * CHUNK_SIZE + u32(pos.z) * CHUNK_SIZE * CHUNK_SIZE;
}

fn in_bounds(pos: vec3<i32>) -> bool {
    return pos.x >= 0 && pos.x < i32(CHUNK_SIZE)
        && pos.y >= 0 && pos.y < i32(CHUNK_SIZE)
        && pos.z >= 0 && pos.z < i32(CHUNK_SIZE);
}
