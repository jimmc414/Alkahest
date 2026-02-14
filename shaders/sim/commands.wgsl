// commands.wgsl — Pass 1: Player command application (M7: brush expansion + push tool).
// Reads the command buffer and writes voxels into the write pool.
// Workgroup: 64x1x1 (one thread per command, max 64 commands).
// Each command may expand into a brush volume (up to radius 16 = ~17K writes).
//
// Buffers: see binding layout below (8 bindings).

const TOOL_PLACE: u32 = 1u;
const TOOL_REMOVE: u32 = 2u;
const TOOL_HEAT: u32 = 3u;
const TOOL_PUSH: u32 = 4u;

const BRUSH_SINGLE: u32 = 0u;
const BRUSH_CUBE: u32 = 1u;
const BRUSH_SPHERE: u32 = 2u;

struct SimCommand {
    tool_type: u32,
    pos_x: i32,
    pos_y: i32,
    pos_z: i32,
    material_id: u32,
    chunk_dispatch_idx: u32,
    brush_radius: u32,
    brush_shape: u32,
}

struct SimParams {
    tick: u32,
    command_count: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read> read_pool: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> write_pool: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> materials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> cmd_buf: array<SimCommand>;
@group(0) @binding(4) var<uniform> sim_params: SimParams;
@group(0) @binding(5) var<storage, read> rule_lookup: array<u32>;
@group(0) @binding(6) var<storage, read> rule_data: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read> chunk_descriptors: array<u32>;

/// Apply a single PLACE operation at a local position.
fn apply_place(pos: vec3<i32>, chunk_idx: u32, mat_id: u32) {
    if !in_bounds(pos) {
        return;
    }
    let idx = write_pool_voxel_index(pos, chunk_idx);
    var temp = 150u;
    if mat_id > 0u {
        let props_1 = materials[mat_id * 3u + 1u];
        let decay_rate = u32(props_1.x);
        let decay_threshold = u32(props_1.y);
        if decay_rate > 0u && decay_threshold > 0u {
            temp = min(decay_threshold * 3u, 4095u);
        }
    }
    let voxel = pack_voxel(mat_id, temp, 0, 0, 0, 0u, 0u);
    write_pool[idx] = voxel;
}

/// Apply a single REMOVE operation at a local position.
fn apply_remove(pos: vec3<i32>, chunk_idx: u32) {
    if !in_bounds(pos) {
        return;
    }
    let idx = write_pool_voxel_index(pos, chunk_idx);
    write_pool[idx] = vec2<u32>(0u, 0u);
}

/// Apply a single HEAT operation at a local position.
fn apply_heat(pos: vec3<i32>, chunk_idx: u32, delta: i32) {
    if !in_bounds(pos) {
        return;
    }
    let idx = write_pool_voxel_index(pos, chunk_idx);
    let current_voxel = write_pool[idx];
    let current_mat = unpack_material_id(current_voxel);
    if current_mat != 0u {
        let current_temp = unpack_temperature(current_voxel);
        var new_temp = i32(current_temp) + delta;
        new_temp = clamp(new_temp, 0, 4095);
        write_pool[idx] = repack_material_temp(current_voxel, current_mat, u32(new_temp));
    }
}

/// Apply a single PUSH operation at a local position.
/// Direction is packed in material_id as 3 biased-128 i8 values: dx | (dy << 8) | (dz << 16).
fn apply_push(pos: vec3<i32>, chunk_idx: u32, dir_packed: u32) {
    if !in_bounds(pos) {
        return;
    }
    let idx = write_pool_voxel_index(pos, chunk_idx);
    let current_voxel = write_pool[idx];
    let current_mat = unpack_material_id(current_voxel);
    if current_mat != 0u {
        // Unpack direction: each component is biased by 128 (0 = -128, 128 = 0, 255 = +127)
        let dx = i32(dir_packed & 0xFFu) - 128;
        let dy = i32((dir_packed >> 8u) & 0xFFu) - 128;
        let dz = i32((dir_packed >> 16u) & 0xFFu) - 128;
        // Add direction to existing velocity, clamped to i8 range
        let cur_vx = unpack_vel_x(current_voxel);
        let cur_vy = unpack_vel_y(current_voxel);
        let cur_vz = unpack_vel_z(current_voxel);
        let new_vx = clamp(cur_vx + dx, -127, 127);
        let new_vy = clamp(cur_vy + dy, -127, 127);
        let new_vz = clamp(cur_vz + dz, -127, 127);
        let temp = unpack_temperature(current_voxel);
        let pressure = unpack_pressure(current_voxel);
        let flags = unpack_flags(current_voxel);
        write_pool[idx] = pack_voxel(current_mat, temp, new_vx, new_vy, new_vz, pressure, flags);
    }
}

/// Check if an offset is within the brush shape for a given radius.
fn in_brush(dx: i32, dy: i32, dz: i32, radius: i32, shape: u32) -> bool {
    if shape == BRUSH_SPHERE {
        return (dx * dx + dy * dy + dz * dz) <= (radius * radius);
    }
    // BRUSH_CUBE and BRUSH_SINGLE: cube check (single = radius 0, always true for center)
    return true;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cmd_index = gid.x;
    if cmd_index >= sim_params.command_count {
        return;
    }

    let cmd = cmd_buf[cmd_index];
    let center = vec3<i32>(cmd.pos_x, cmd.pos_y, cmd.pos_z);
    let chunk_idx = cmd.chunk_dispatch_idx;
    let radius = i32(cmd.brush_radius);
    let shape = cmd.brush_shape;

    // Single voxel mode: radius 0 or brush_shape 0
    if radius == 0 || shape == BRUSH_SINGLE {
        switch cmd.tool_type {
            case 1u: { apply_place(center, chunk_idx, cmd.material_id); }
            case 2u: { apply_remove(center, chunk_idx); }
            case 3u: { apply_heat(center, chunk_idx, bitcast<i32>(cmd.material_id)); }
            case 4u: { apply_push(center, chunk_idx, cmd.material_id); }
            default: {}
        }
        return;
    }

    // Brush expansion: iterate over cube from -radius to +radius (bounded to max 16)
    let r = min(radius, 16);
    for (var dz = -r; dz <= r; dz++) {
        for (var dy = -r; dy <= r; dy++) {
            for (var dx = -r; dx <= r; dx++) {
                if !in_brush(dx, dy, dz, r, shape) {
                    continue;
                }
                let pos = vec3<i32>(center.x + dx, center.y + dy, center.z + dz);
                switch cmd.tool_type {
                    case 1u: { apply_place(pos, chunk_idx, cmd.material_id); }
                    case 2u: { apply_remove(pos, chunk_idx); }
                    case 3u: { apply_heat(pos, chunk_idx, bitcast<i32>(cmd.material_id)); }
                    case 4u: { apply_push(pos, chunk_idx, cmd.material_id); }
                    default: {}
                }
            }
        }
    }
}
