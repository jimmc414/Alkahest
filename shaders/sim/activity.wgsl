// activity.wgsl — Pass 5: Activity scan (M5: multi-chunk).
// Compares read_pool vs write_pool per voxel to detect chunk-level changes.
// Uses workgroup shared memory with parallel OR reduction (atomicOr).
// Output: one u32 flag per chunk (non-zero = dirty, 0 = idle).
//
// Workgroup: 8×8×4 = 256 threads.
// Dispatch: (CHUNK_SIZE/8, CHUNK_SIZE/8, active_chunk_count * CHUNK_SIZE/4)
//
// Bind group (separate from main sim, 5 bindings):
//   binding 0: read_pool (storage, read)
//   binding 1: write_pool (storage, read) — post-sim state
//   binding 2: activity_flags (storage, read_write) — output, one u32 per chunk
//   binding 3: uniforms (uniform)
//   binding 4: chunk_descriptors (storage, read) — pool slot offsets

struct ActivityUniforms {
    tick: u32,
    active_chunk_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
    _pad4: u32,
    _pad5: u32,
}

@group(0) @binding(0) var<storage, read> read_pool: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read> write_pool_ro: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read_write> activity_flags: array<atomic<u32>>;
@group(0) @binding(3) var<uniform> activity_uniforms: ActivityUniforms;
@group(0) @binding(4) var<storage, read> chunk_descriptors: array<u32>;

var<workgroup> wg_dirty: atomic<u32>;

@compute @workgroup_size(8, 8, 4)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    // Initialize workgroup shared atomic (first thread clears it)
    if lid == 0u {
        atomicStore(&wg_dirty, 0u);
    }
    // All threads must reach this barrier (C-WGSL-4)
    workgroupBarrier();

    let chunk_idx = gid.z / CHUNK_SIZE;
    let local_z = gid.z % CHUNK_SIZE;

    // Compare read vs write pool voxels — only for valid chunks
    if chunk_idx < activity_uniforms.active_chunk_count {
        let slot_offset = chunk_descriptors[chunk_idx * CHUNK_DESC_STRIDE];
        let vi = gid.x + gid.y * CHUNK_SIZE + local_z * CHUNK_SIZE * CHUNK_SIZE;
        let pool_idx = (slot_offset / 8u) + vi;

        let read_val = read_pool[pool_idx];
        let write_val = write_pool_ro[pool_idx];

        // Any bit difference = dirty (C-SIM-8: errs toward false positives)
        if read_val.x != write_val.x || read_val.y != write_val.y {
            atomicOr(&wg_dirty, 1u);
        }
    }

    // All threads must reach this barrier (C-WGSL-4)
    workgroupBarrier();

    // Thread 0 of each workgroup writes dirty flag for the chunk
    if lid == 0u && chunk_idx < activity_uniforms.active_chunk_count {
        let dirty = atomicLoad(&wg_dirty);
        if dirty != 0u {
            atomicOr(&activity_flags[chunk_idx], dirty);
        }
    }
}
