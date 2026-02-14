// movement.wgsl — Pass 2: Movement with checkerboard sub-passes (M5: multi-chunk).
// Each sub-pass handles one direction with one checkerboard parity.
// Dispatched multiple times per tick with different uniform parameters.
//
// Workgroup: 8x8x4 = 256 threads.
// Dispatch: (CHUNK_SIZE/8, CHUNK_SIZE/8, active_chunk_count * CHUNK_SIZE/4)

const PHASE_GAS: u32 = 0u;
const PHASE_LIQUID: u32 = 1u;
const PHASE_SOLID: u32 = 2u;
const PHASE_POWDER: u32 = 3u;

struct MovementParams {
    dir_x: i32,
    dir_y: i32,
    dir_z: i32,
    parity: u32,
    tick: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

struct SimCommand {
    tool_type: u32,
    pos_x: i32,
    pos_y: i32,
    pos_z: i32,
    material_id: u32,
    chunk_dispatch_idx: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> read_pool: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> write_pool: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> materials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> cmd_buf: array<SimCommand>;
@group(0) @binding(4) var<uniform> move_params: MovementParams;
@group(0) @binding(5) var<storage, read> rule_lookup: array<u32>;
@group(0) @binding(6) var<storage, read> rule_data: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read> chunk_descriptors: array<u32>;

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Extract chunk index and local position from batched z-dispatch
    let chunk_idx = gid.z / CHUNK_SIZE;
    let local_z = gid.z % CHUNK_SIZE;
    let pos = vec3<i32>(i32(gid.x), i32(gid.y), i32(local_z));

    // Bounds check (C-WGSL-6: i32 for coords, u32 only for final index)
    if !in_bounds(pos) {
        return;
    }

    // Checkerboard filter: only process cells matching current parity
    let cell_parity = u32(pos.x + pos.z) % 2u;
    if cell_parity != move_params.parity {
        return;
    }

    let src_idx = write_pool_voxel_index(pos, chunk_idx);
    let src_voxel = write_pool[src_idx];
    let src_mat_id = unpack_material_id(src_voxel);

    // Air doesn't move
    if src_mat_id == 0u {
        return;
    }

    // Look up source material properties (density-driven movement, C-DESIGN-1)
    let src_props_0 = materials[src_mat_id * 3u];
    let src_density = src_props_0.x;
    let src_phase = u32(src_props_0.y);

    // Solid phase doesn't move
    if src_phase == PHASE_SOLID {
        return;
    }

    // Phase-direction filtering (C-DESIGN-1)
    let dir_y = move_params.dir_y;
    if dir_y < 0 {
        if src_phase != PHASE_POWDER && src_phase != PHASE_LIQUID {
            return;
        }
    } else if dir_y == 0 {
        if src_phase != PHASE_LIQUID {
            return;
        }
        let src_props_1 = materials[src_mat_id * 3u + 1u];
        let viscosity = src_props_1.w;
        if viscosity > 0.0 {
            let h = sim_hash(pos.x, pos.y, pos.z, move_params.tick);
            let roll = hash_to_float(h);
            if roll < viscosity {
                return;
            }
        }
    } else {
        if src_phase != PHASE_GAS {
            return;
        }
        if src_density <= 0.0 {
            return;
        }
    }

    // Compute destination position (may cross chunk boundary)
    let dir = vec3<i32>(move_params.dir_x, move_params.dir_y, move_params.dir_z);
    let dst_pos = pos + dir;

    // Cross-chunk voxel read for destination
    let dst_voxel = cross_chunk_voxel(dst_pos, chunk_idx);
    let dst_mat_id = unpack_material_id(dst_voxel);

    // For cross-chunk writes, we can only write to our own chunk's write pool.
    // If destination is in another chunk, skip (neighbor chunk handles it from its side).
    if !in_bounds(dst_pos) {
        return;
    }

    let dst_idx = write_pool_voxel_index(dst_pos, chunk_idx);

    // If destination is air, move there
    if dst_mat_id == 0u {
        write_pool[dst_idx] = src_voxel;
        write_pool[src_idx] = vec2<u32>(0u, 0u);
        return;
    }

    // Density-driven displacement: if destination is lighter, swap
    let dst_props_0 = materials[dst_mat_id * 3u];
    let dst_density = dst_props_0.x;
    let dst_phase = u32(dst_props_0.y);

    if dst_phase == PHASE_SOLID {
        return;
    }

    if src_density > dst_density {
        write_pool[dst_idx] = src_voxel;
        write_pool[src_idx] = dst_voxel;
    }
}
