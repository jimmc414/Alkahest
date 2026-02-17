// electrical.wgsl — Pass 4b: Electrical charge propagation, Joule heating (M15).
// Reads read_pool (for cross-chunk voxel material lookups).
// Reads/writes write_pool (Joule heating temperature updates).
// Reads charge_read, writes charge_write (double-buffered charge propagation).
// Uses 6 face-adjacent neighbors only (not 26 like thermal).
//
// Workgroup: 8x8x4 = 256 threads.
// Dispatch: (CHUNK_SIZE/8, CHUNK_SIZE/8, active_chunk_count * CHUNK_SIZE/4)

// 6 face-adjacent neighbor offsets
const FACE_OFFSETS: array<vec3<i32>, 6> = array<vec3<i32>, 6>(
    vec3<i32>(1, 0, 0),
    vec3<i32>(-1, 0, 0),
    vec3<i32>(0, 1, 0),
    vec3<i32>(0, -1, 0),
    vec3<i32>(0, 0, 1),
    vec3<i32>(0, 0, -1),
);

struct ElectricalUniforms {
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
@group(0) @binding(3) var<storage, read> charge_read: array<u32>;
@group(0) @binding(4) var<storage, read_write> charge_write: array<u32>;
@group(0) @binding(5) var<uniform> electrical_uniforms: ElectricalUniforms;
@group(0) @binding(6) var<storage, read> chunk_descriptors: array<u32>;

/// Map a local voxel position to charge buffer index.
/// Charge buffer uses 1 u32 per voxel, indexed by slot * VOXELS_PER_CHUNK + voxel_index.
/// The pool slot_offset (in bytes) / 8 gives the voxel offset, which equals the charge offset.
fn charge_buf_index(pos: vec3<i32>, chunk_idx: u32) -> u32 {
    let slot_offset = chunk_descriptors[chunk_idx * CHUNK_DESC_STRIDE];
    return (slot_offset / 8u) + voxel_index(pos);
}

/// Read charge from a neighbor position, handling cross-chunk boundaries.
/// Returns 0 for air, out-of-bounds, or unloaded neighbor chunks.
fn read_neighbor_charge(pos: vec3<i32>, chunk_idx: u32) -> u32 {
    if in_bounds(pos) {
        return charge_read[charge_buf_index(pos, chunk_idx)];
    }

    // Cross-chunk neighbor lookup
    let neighbor_dir = compute_neighbor_dir(pos);
    let neighbor_slot_offset = chunk_descriptors[chunk_idx * CHUNK_DESC_STRIDE + 1u + neighbor_dir];

    if neighbor_slot_offset == SENTINEL_NEIGHBOR {
        return 0u;
    }

    let remapped = remap_coords(pos);
    return charge_read[(neighbor_slot_offset / 8u) + voxel_index(remapped)];
}

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_idx = gid.z / CHUNK_SIZE;
    let local_z = gid.z % CHUNK_SIZE;
    let pos = vec3<i32>(i32(gid.x), i32(gid.y), i32(local_z));

    if !in_bounds(pos) {
        return;
    }

    let pool_idx = write_pool_voxel_index(pos, chunk_idx);
    let ch_idx = charge_buf_index(pos, chunk_idx);
    var voxel = write_pool[pool_idx];
    let mat_id = unpack_material_id(voxel);

    // Air: zero charge
    if mat_id == 0u {
        charge_write[ch_idx] = 0u;
        return;
    }

    // Read electrical properties: vec4[3] = (conductivity, resistance, activation_threshold, charge_emission)
    let props_3 = materials[mat_id * 4u + 3u];
    let conductivity = props_3.x;
    let resistance = props_3.y;
    let activation_threshold = u32(props_3.z);
    let charge_emission = u32(props_3.w);

    let current_charge = charge_read[ch_idx];

    // Power source: constant emission
    if charge_emission > 0u {
        charge_write[ch_idx] = charge_emission;
        return;
    }

    // Insulator (conductivity == 0): decay only, no propagation
    if conductivity == 0.0 {
        if current_charge > CHARGE_DECAY_RATE {
            charge_write[ch_idx] = current_charge - CHARGE_DECAY_RATE;
        } else {
            charge_write[ch_idx] = 0u;
        }
        return;
    }

    // Ground: absorb all charge (high conductivity, zero resistance, zero emission)
    if conductivity > 0.9 && resistance == 0.0 {
        charge_write[ch_idx] = 0u;
        return;
    }

    // Conductor: diffuse charge from 6 face-adjacent neighbors
    var charged_count = 0u;
    var charge_sum = 0u;

    for (var n = 0u; n < 6u; n = n + 1u) {
        let neighbor_pos = pos + FACE_OFFSETS[n];
        let neighbor_charge = read_neighbor_charge(neighbor_pos, chunk_idx);

        if neighbor_charge > 0u {
            charged_count = charged_count + 1u;
            charge_sum = charge_sum + neighbor_charge;
        }
    }

    var new_charge: u32;

    if charged_count >= activation_threshold {
        // Diffuse: sum of neighbor charges weighted by conductivity and diffusion rate
        let diffused = f32(charge_sum) * conductivity * ELECTRICAL_DIFFUSION_RATE;
        new_charge = min(u32(diffused), CHARGE_MAX);
        // Preserve momentum: don't drop below current charge minus decay
        if current_charge > CHARGE_DECAY_RATE {
            new_charge = max(new_charge, current_charge - CHARGE_DECAY_RATE);
        }
    } else {
        // Below activation threshold: decay toward zero
        if current_charge > CHARGE_DECAY_RATE {
            new_charge = current_charge - CHARGE_DECAY_RATE;
        } else {
            new_charge = 0u;
        }
    }

    charge_write[ch_idx] = new_charge;

    // Joule heating: temp_increase = charge² × resistance × JOULE_HEATING_FACTOR
    if new_charge > 0u && resistance > 0.0 {
        let my_temp = unpack_temperature(voxel);
        let heat = f32(new_charge * new_charge) * resistance * JOULE_HEATING_FACTOR;
        var new_temp = i32(my_temp) + i32(heat);
        new_temp = clamp(new_temp, 0, i32(TEMP_QUANT_MAX_VALUE));

        if u32(new_temp) != my_temp {
            let vx = unpack_vel_x(voxel);
            let vy = unpack_vel_y(voxel);
            let vz = unpack_vel_z(voxel);
            let pressure = unpack_pressure(voxel);
            let flags = unpack_flags(voxel);
            write_pool[pool_idx] = pack_voxel(mat_id, u32(new_temp), vx, vy, vz, pressure, flags);
        }
    }
}
