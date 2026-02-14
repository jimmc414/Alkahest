// thermal.wgsl — Pass 4: Thermal diffusion, entropy drain, and convection bias.
// Reads/writes write_buf (in-place, own voxel only). Reads materials buffer.
//
// Workgroup: 8x8x4 = 256 threads.
//
// Buffers:
//   @group(0) @binding(0) read_buf      — storage, read (unused, shared layout)
//   @group(0) @binding(1) write_buf     — storage, read_write (post-reactions state)
//   @group(0) @binding(2) materials     — storage, read (3x vec4<f32> per material)
//   @group(0) @binding(3) cmd_buf       — storage, read (unused, shared layout)
//   @group(0) @binding(4) uniforms      — uniform (tick, material_count)
//   @group(0) @binding(5) rule_lookup   — storage, read (unused, shared layout)
//   @group(0) @binding(6) rule_data     — storage, read (unused, shared layout)
//
// Algorithm per voxel:
//   1. Read own voxel. If air (mat_id == 0), return.
//   2. Read thermal_conductivity from materials[mat_id * 3u + 2u].x.
//   3. 26-neighbor weighted average: face=1.0, edge=0.7, corner=0.5.
//   4. Apply diffusion: new_temp = clamp(my_temp + i32(DIFFUSION_RATE * delta), 0, 4095).
//   5. Entropy drain: nudge temperature toward AMBIENT_TEMP_QUANTIZED.
//   6. Convection: if liquid/gas and temp > ambient + threshold, set velocity_y = +1.
//   7. Write updated voxel.

// Thermal constants (injected from alkahest-core/constants.rs via preamble)
// DIFFUSION_RATE, ENTROPY_DRAIN_RATE, CONVECTION_THRESHOLD, AMBIENT_TEMP_QUANTIZED
// are defined in the constants preamble.

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
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> read_buf: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> write_buf: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> materials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> cmd_buf: array<SimCommand>;
@group(0) @binding(4) var<uniform> thermal_uniforms: ThermalUniforms;
@group(0) @binding(5) var<storage, read> rule_lookup: array<u32>;
@group(0) @binding(6) var<storage, read> rule_data: array<vec4<u32>>;

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(i32(gid.x), i32(gid.y), i32(gid.z));

    // Bounds check (C-WGSL-6: i32 for coords)
    if !in_bounds(pos) {
        return;
    }

    let idx = voxel_index(pos);
    var voxel = write_buf[idx];
    let mat_id = unpack_material_id(voxel);

    // Air doesn't conduct heat
    if mat_id == 0u {
        return;
    }

    var my_temp = unpack_temperature(voxel);

    // Read thermal conductivity from props_2
    let props_2 = materials[mat_id * 3u + 2u];
    let my_conductivity = props_2.x;

    // --- 26-neighbor diffusion ---
    var delta = 0.0;
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }

                let neighbor_pos = pos + vec3<i32>(dx, dy, dz);
                if !in_bounds(neighbor_pos) {
                    continue;
                }

                let neighbor_idx = voxel_index(neighbor_pos);
                let neighbor_voxel = write_buf[neighbor_idx];
                let neighbor_mat = unpack_material_id(neighbor_voxel);

                // Skip air neighbors (air doesn't participate in conduction)
                if neighbor_mat == 0u {
                    continue;
                }

                let neighbor_temp = unpack_temperature(neighbor_voxel);
                let neighbor_props_2 = materials[neighbor_mat * 3u + 2u];
                let neighbor_conductivity = neighbor_props_2.x;

                // Weight: face=1.0, edge=0.7, corner=0.5
                let abs_sum = abs(dx) + abs(dy) + abs(dz);
                var weight = 0.5; // corner (3 axes differ)
                if abs_sum == 1 {
                    weight = 1.0; // face neighbor
                } else if abs_sum == 2 {
                    weight = 0.7; // edge neighbor
                }

                let k_avg = (my_conductivity + neighbor_conductivity) * 0.5;
                delta += weight * k_avg * (f32(neighbor_temp) - f32(my_temp));
            }
        }
    }

    // Apply diffusion (CFL-stable: DIFFUSION_RATE * max_k * 26 < 1.0)
    var new_temp = i32(my_temp) + i32(DIFFUSION_RATE * delta / 26.0);
    new_temp = clamp(new_temp, 0, i32(TEMP_QUANT_MAX_VALUE));

    // --- Entropy drain toward ambient ---
    if new_temp > i32(AMBIENT_TEMP_QUANTIZED) {
        new_temp = max(new_temp - i32(ENTROPY_DRAIN_RATE), i32(AMBIENT_TEMP_QUANTIZED));
    } else if new_temp < i32(AMBIENT_TEMP_QUANTIZED) {
        new_temp = min(new_temp + i32(ENTROPY_DRAIN_RATE), i32(AMBIENT_TEMP_QUANTIZED));
    }

    // --- Convection bias ---
    // Heated liquids/gases get upward velocity
    let props_0 = materials[mat_id * 3u];
    let phase = u32(props_0.y);
    var vy = unpack_vel_y(voxel);
    if (phase == PHASE_LIQUID || phase == PHASE_GAS) && u32(new_temp) > AMBIENT_TEMP_QUANTIZED + CONVECTION_THRESHOLD {
        vy = 1;
    }

    // Write updated voxel
    let vx = unpack_vel_x(voxel);
    let vz = unpack_vel_z(voxel);
    let pressure = unpack_pressure(voxel);
    let flags = unpack_flags(voxel);
    write_buf[idx] = pack_voxel(mat_id, u32(new_temp), vx, vy, vz, pressure, flags);
}
