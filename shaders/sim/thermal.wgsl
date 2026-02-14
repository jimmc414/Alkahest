// thermal.wgsl — Pass 4: Thermal diffusion, entropy drain, convection (M5: multi-chunk).
// Reads/writes write_pool (in-place, own voxel only). Reads materials buffer.
// Uses cross_chunk_voxel() for 26-neighbor diffusion across chunk boundaries.
//
// Workgroup: 8x8x4 = 256 threads.
// Dispatch: (CHUNK_SIZE/8, CHUNK_SIZE/8, active_chunk_count * CHUNK_SIZE/4)

const PHASE_GAS: u32 = 0u;
const PHASE_LIQUID: u32 = 1u;

struct ThermalUniforms {
    tick: u32,
    material_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
    _pad4: u32,
    _pad5: u32,
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
@group(0) @binding(4) var<uniform> thermal_uniforms: ThermalUniforms;
@group(0) @binding(5) var<storage, read> rule_lookup: array<u32>;
@group(0) @binding(6) var<storage, read> rule_data: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read> chunk_descriptors: array<u32>;

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_idx = gid.z / CHUNK_SIZE;
    let local_z = gid.z % CHUNK_SIZE;
    let pos = vec3<i32>(i32(gid.x), i32(gid.y), i32(local_z));

    if !in_bounds(pos) {
        return;
    }

    let idx = write_pool_voxel_index(pos, chunk_idx);
    var voxel = write_pool[idx];
    let mat_id = unpack_material_id(voxel);

    if mat_id == 0u {
        return;
    }

    var my_temp = unpack_temperature(voxel);

    let props_2 = materials[mat_id * 3u + 2u];
    let my_conductivity = props_2.x;

    // --- 26-neighbor diffusion with cross-chunk reads ---
    var delta = 0.0;
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }

                let neighbor_pos = pos + vec3<i32>(dx, dy, dz);

                // Cross-chunk neighbor read
                let neighbor_voxel = cross_chunk_voxel(neighbor_pos, chunk_idx);
                let neighbor_mat = unpack_material_id(neighbor_voxel);

                if neighbor_mat == 0u {
                    continue;
                }

                let neighbor_temp = unpack_temperature(neighbor_voxel);
                let neighbor_props_2 = materials[neighbor_mat * 3u + 2u];
                let neighbor_conductivity = neighbor_props_2.x;

                let abs_sum = abs(dx) + abs(dy) + abs(dz);
                var weight = 0.5;
                if abs_sum == 1 {
                    weight = 1.0;
                } else if abs_sum == 2 {
                    weight = 0.7;
                }

                let k_avg = (my_conductivity + neighbor_conductivity) * 0.5;
                delta += weight * k_avg * (f32(neighbor_temp) - f32(my_temp));
            }
        }
    }

    var new_temp = i32(my_temp) + i32(DIFFUSION_RATE * delta / 26.0);
    new_temp = clamp(new_temp, 0, i32(TEMP_QUANT_MAX_VALUE));

    // --- Entropy drain ---
    if new_temp > i32(AMBIENT_TEMP_QUANTIZED) {
        new_temp = max(new_temp - i32(ENTROPY_DRAIN_RATE), i32(AMBIENT_TEMP_QUANTIZED));
    } else if new_temp < i32(AMBIENT_TEMP_QUANTIZED) {
        new_temp = min(new_temp + i32(ENTROPY_DRAIN_RATE), i32(AMBIENT_TEMP_QUANTIZED));
    }

    // --- Convection ---
    let props_0 = materials[mat_id * 3u];
    let phase = u32(props_0.y);
    var vy = unpack_vel_y(voxel);
    if (phase == PHASE_LIQUID || phase == PHASE_GAS) && u32(new_temp) > AMBIENT_TEMP_QUANTIZED + CONVECTION_THRESHOLD {
        vy = 1;
    }

    let vx = unpack_vel_x(voxel);
    let vz = unpack_vel_z(voxel);
    let pressure = unpack_pressure(voxel);
    let flags = unpack_flags(voxel);
    write_pool[idx] = pack_voxel(mat_id, u32(new_temp), vx, vy, vz, pressure, flags);
}
