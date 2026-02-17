// pressure.wgsl — Pass 5: Pressure accumulation, diffusion, and rupture (M6).
// Reads/writes write_pool (in-place, own voxel only). Reads materials buffer.
// Uses cross_chunk_voxel() for 6-neighbor pressure diffusion across chunk boundaries.
//
// Workgroup: 8x8x4 = 256 threads.
// Dispatch: (CHUNK_SIZE/8, CHUNK_SIZE/8, active_chunk_count * CHUNK_SIZE/4)
//
// Algorithm per voxel:
//   1. Enclosure check: count non-air face neighbors (6-connected)
//   2. Thermal pressure: enclosed gas/liquid above ambient → +1 pressure/tick
//   3. Pressure diffusion: average with 6 face neighbors weighted by PRESSURE_DIFFUSION_RATE
//   4. Rupture: pressure > structural_integrity → become Air with high outward velocity

const PHASE_GAS: u32 = 0u;
const PHASE_LIQUID: u32 = 1u;

// 6 face-adjacent neighbor offsets
const FACE_OFFSETS: array<vec3<i32>, 6> = array<vec3<i32>, 6>(
    vec3<i32>(1, 0, 0),
    vec3<i32>(-1, 0, 0),
    vec3<i32>(0, 1, 0),
    vec3<i32>(0, -1, 0),
    vec3<i32>(0, 0, 1),
    vec3<i32>(0, 0, -1),
);

struct PressureUniforms {
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
@group(0) @binding(4) var<uniform> pressure_uniforms: PressureUniforms;
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

    let my_temp = unpack_temperature(voxel);
    var my_pressure = i32(unpack_pressure(voxel));

    // Read material properties
    let props_0 = materials[mat_id * 4u];
    let phase = u32(props_0.y);
    let props_2 = materials[mat_id * 4u + 2u];
    let structural_integrity = props_2.w;

    // --- Enclosure check: count non-air face neighbors ---
    var non_air_count = 0u;
    var neighbor_pressure_sum = 0;
    var neighbor_count = 0;

    for (var n = 0u; n < 6u; n = n + 1u) {
        let neighbor_pos = pos + FACE_OFFSETS[n];
        let neighbor_voxel = cross_chunk_voxel(neighbor_pos, chunk_idx);
        let neighbor_mat = unpack_material_id(neighbor_voxel);

        if neighbor_mat != 0u {
            non_air_count = non_air_count + 1u;
        }

        neighbor_pressure_sum = neighbor_pressure_sum + i32(unpack_pressure(neighbor_voxel));
        neighbor_count = neighbor_count + 1;
    }

    let enclosed = non_air_count == 6u;

    // --- Thermal pressure generation ---
    // Enclosed gas/liquid above ambient gains pressure
    if enclosed && (phase == PHASE_GAS || phase == PHASE_LIQUID) && my_temp > AMBIENT_TEMP_QUANTIZED {
        my_pressure = min(my_pressure + i32(THERMAL_PRESSURE_FACTOR), i32(MAX_PRESSURE));
    }

    // --- Pressure diffusion ---
    // Average own pressure with face neighbors
    if neighbor_count > 0 {
        let avg_neighbor = f32(neighbor_pressure_sum) / f32(neighbor_count);
        let diff = avg_neighbor - f32(my_pressure);
        my_pressure = my_pressure + i32(PRESSURE_DIFFUSION_RATE * diff);
        my_pressure = clamp(my_pressure, 0, i32(MAX_PRESSURE));
    }

    // --- Rupture check ---
    // If pressure exceeds structural integrity, voxel ruptures → becomes Air
    if structural_integrity > 0.0 && f32(my_pressure) > structural_integrity {
        // Rupture: become air, keep high pressure for blast wave propagation
        let flags = unpack_flags(voxel);
        // Set outward velocity based on hash for varied blast direction
        let h = sim_hash(pos.x, pos.y, pos.z, pressure_uniforms.tick);
        let dir_idx = h % 6u;
        let blast_dir = FACE_OFFSETS[dir_idx];
        let blast_speed = clamp(my_pressure / 8, 1, 4);
        write_pool[idx] = pack_voxel(
            0u,                          // Air
            my_temp,
            blast_dir.x * blast_speed,
            blast_dir.y * blast_speed,
            blast_dir.z * blast_speed,
            u32(my_pressure),
            flags,
        );
        return;
    }

    // --- Write updated pressure ---
    let vx = unpack_vel_x(voxel);
    let vy = unpack_vel_y(voxel);
    let vz = unpack_vel_z(voxel);
    let flags = unpack_flags(voxel);
    write_pool[idx] = pack_voxel(mat_id, my_temp, vx, vy, vz, u32(my_pressure), flags);
}
