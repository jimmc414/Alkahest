// movement.wgsl — Pass 2: Gravity movement with checkerboard sub-passes.
// Each sub-pass handles one direction with one checkerboard parity.
// Dispatched multiple times per tick with different uniform parameters.
//
// Workgroup: 8x8x4 = 256 threads (C-GPU-5: at limit).
//
// Buffers:
//   @group(0) @binding(0) read_buf    — storage, read (previous tick state)
//   @group(0) @binding(1) write_buf   — storage, read_write (next tick state)
//   @group(0) @binding(2) materials   — storage, read (material properties)
//   @group(0) @binding(3) cmd_buf     — storage, read (unused in this pass, shared layout)
//   @group(0) @binding(4) move_params — uniform (direction, parity, tick)
//
// Algorithm:
//   For each voxel in the checkerboard-filtered set:
//   1. Read source voxel from write buffer (post-commands state)
//   2. If source is air or solid phase, skip (density-driven, C-DESIGN-1)
//   3. Read destination in the sub-pass direction from write buffer
//   4. If destination is air: move source there, write air to source
//   5. If destination has lower density: swap source and destination
//   6. Atomic at sub-pass level (C-SIM-6): both writes in same invocation

// Phase constants for material lookup
const PHASE_GAS: u32 = 0u;
const PHASE_LIQUID: u32 = 1u;
const PHASE_SOLID: u32 = 2u;
const PHASE_POWDER: u32 = 3u;

// Material properties layout: vec4<f32>(density, phase_as_float, pad, pad)
// density is f32, phase is stored as f32 but compared as u32

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
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> read_buf: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> write_buf: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> materials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> cmd_buf: array<SimCommand>;
@group(0) @binding(4) var<uniform> move_params: MovementParams;

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(i32(gid.x), i32(gid.y), i32(gid.z));

    // Bounds check (C-WGSL-6: i32 for coords, u32 only for final index)
    if !in_bounds(pos) {
        return;
    }

    // Checkerboard filter: only process cells matching current parity
    let cell_parity = u32(pos.x + pos.z) % 2u;
    if cell_parity != move_params.parity {
        return;
    }

    let src_idx = voxel_index(pos);
    let src_voxel = write_buf[src_idx];
    let src_mat_id = unpack_material_id(src_voxel);

    // Air (material 0) doesn't move
    if src_mat_id == 0u {
        return;
    }

    // Look up source material properties (density-driven movement, C-DESIGN-1)
    let src_props = materials[src_mat_id];
    let src_density = src_props.x;
    let src_phase = u32(src_props.y);

    // Solid phase doesn't move under gravity
    if src_phase == PHASE_SOLID {
        return;
    }

    // Only powders move in M2 (skip liquid/gas for now — they'll be handled when those phases exist)
    if src_phase != PHASE_POWDER {
        return;
    }

    // Compute destination position
    let dir = vec3<i32>(move_params.dir_x, move_params.dir_y, move_params.dir_z);
    let dst_pos = pos + dir;

    // Out of bounds = blocked (chunk boundary, no cross-chunk at M2)
    if !in_bounds(dst_pos) {
        return;
    }

    let dst_idx = voxel_index(dst_pos);
    let dst_voxel = write_buf[dst_idx];
    let dst_mat_id = unpack_material_id(dst_voxel);

    // If destination is air, move there
    if dst_mat_id == 0u {
        // C-SIM-6: atomic swap at sub-pass level — both writes in same invocation
        write_buf[dst_idx] = src_voxel;
        write_buf[src_idx] = vec2<u32>(0u, 0u); // air
        return;
    }

    // Density-driven displacement: if destination is lighter, swap
    let dst_props = materials[dst_mat_id];
    let dst_density = dst_props.x;
    let dst_phase = u32(dst_props.y);

    // Don't displace solids
    if dst_phase == PHASE_SOLID {
        return;
    }

    // Swap if source is denser than destination (C-DESIGN-1)
    if src_density > dst_density {
        write_buf[dst_idx] = src_voxel;
        write_buf[src_idx] = dst_voxel;
    }
}
