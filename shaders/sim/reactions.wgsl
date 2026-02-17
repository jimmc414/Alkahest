// reactions.wgsl — Pass 3: Pairwise reactions and self-decay (M5: multi-chunk).
// Reads write_pool (post-movement state) + materials + rule_lookup + rule_data.
// Writes write_pool (own voxel only — no cross-voxel writes).
//
// Workgroup: 8x8x4 = 256 threads.
// Dispatch: (CHUNK_SIZE/8, CHUNK_SIZE/8, active_chunk_count * CHUNK_SIZE/4)

const NO_RULE: u32 = 0xFFFFFFFFu;

// 6 face-adjacent neighbor offsets
const NEIGHBOR_OFFSETS: array<vec3<i32>, 6> = array<vec3<i32>, 6>(
    vec3<i32>(0, -1, 0),
    vec3<i32>(0, 1, 0),
    vec3<i32>(0, 0, -1),
    vec3<i32>(0, 0, 1),
    vec3<i32>(1, 0, 0),
    vec3<i32>(-1, 0, 0),
);

struct ReactionUniforms {
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
@group(0) @binding(4) var<uniform> reaction_uniforms: ReactionUniforms;
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

    // --- Self-decay ---
    let props_1 = materials[mat_id * 4u + 1u];
    let decay_rate = u32(props_1.x);
    let decay_threshold = u32(props_1.y);
    let decay_product = u32(props_1.z);

    if decay_rate > 0u {
        if my_temp > decay_rate {
            my_temp = my_temp - decay_rate;
        } else {
            my_temp = 0u;
        }

        if my_temp < decay_threshold {
            write_pool[idx] = repack_material_temp(voxel, decay_product, my_temp);
            return;
        }

        voxel = repack_material_temp(voxel, mat_id, my_temp);
    }

    // --- Upward phase change ---
    {
        let props_2 = materials[unpack_material_id(voxel) * 4u + 2u];
        let phase_change_temp_q = u32(props_2.y);
        let phase_change_product = u32(props_2.z);
        if phase_change_temp_q > 0u && my_temp >= phase_change_temp_q {
            voxel = repack_material_temp(voxel, phase_change_product, my_temp);
        }
    }

    // --- Pairwise reactions with cross-chunk neighbor reads ---
    let mc = reaction_uniforms.material_count;

    for (var n = 0u; n < 6u; n = n + 1u) {
        let neighbor_pos = pos + NEIGHBOR_OFFSETS[n];

        // Use cross_chunk_voxel for cross-boundary reads
        let neighbor_voxel = cross_chunk_voxel(neighbor_pos, chunk_idx);
        let neighbor_mat = unpack_material_id(neighbor_voxel);

        let lookup_idx = mat_id * mc + neighbor_mat;
        let rule_idx = rule_lookup[lookup_idx];

        if rule_idx == NO_RULE {
            continue;
        }

        let rule_0 = rule_data[rule_idx * 2u];
        let rule_1 = rule_data[rule_idx * 2u + 1u];

        let input_a_becomes = rule_0.x;
        let probability_u32 = rule_0.w;

        let min_temp = rule_1.z;
        let max_temp = rule_1.w;

        if min_temp > 0u && my_temp < min_temp {
            continue;
        }
        if max_temp > 0u && my_temp > max_temp {
            continue;
        }

        if probability_u32 < 0xFFFFFFFFu {
            let h = sim_hash(pos.x + i32(n), pos.y, pos.z, reaction_uniforms.tick);
            if h > probability_u32 {
                continue;
            }
        }

        let temp_delta = bitcast<i32>(rule_1.x);
        var new_temp = i32(my_temp) + temp_delta;
        new_temp = clamp(new_temp, 0, 4095);

        // Apply pressure_delta from rule (rule_0.y, bitcast to i32)
        let pressure_delta = bitcast<i32>(rule_0.y);
        var new_pressure = i32(unpack_pressure(voxel)) + pressure_delta;
        new_pressure = clamp(new_pressure, 0, 63);

        let vx = unpack_vel_x(voxel);
        let vy = unpack_vel_y(voxel);
        let vz = unpack_vel_z(voxel);
        let flags = unpack_flags(voxel);
        voxel = pack_voxel(input_a_becomes, u32(new_temp), vx, vy, vz, u32(new_pressure), flags);
        break;
    }

    write_pool[idx] = voxel;
}
