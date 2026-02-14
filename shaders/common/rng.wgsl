// rng.wgsl — Deterministic per-voxel PRNG (C-SIM-4).
// Pure function: sim_hash(x, y, z, tick) -> u32.
// No state, no atomics. Identical algorithm exists in alkahest-sim/src/rng.rs.

fn sim_hash(x: i32, y: i32, z: i32, tick: u32) -> u32 {
    // Combine inputs using prime multipliers (wrapping arithmetic is default in WGSL u32)
    var state = u32(x) * 0x9E3779B9u
              + u32(y) * 0x517CC1B7u
              + u32(z) * 0x6C62272Eu
              + tick * 0x2545F491u;

    // PCG-style mixing rounds
    state = state ^ (state >> 16u);
    state = state * 0x45D9F3Bu;
    state = state ^ (state >> 16u);
    state = state * 0x45D9F3Bu;
    state = state ^ (state >> 16u);

    return state;
}

fn hash_to_float(hash_val: u32) -> f32 {
    return f32(hash_val >> 8u) / 16777216.0;
}
