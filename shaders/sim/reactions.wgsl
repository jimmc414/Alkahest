// reactions.wgsl — Pass 3: Pairwise reactions and self-decay.
// Reads write_buf (post-movement state) + materials + rule_lookup + rule_data.
// Writes write_buf (own voxel only — no cross-voxel writes).
//
// Workgroup: 8x8x4 = 256 threads.
//
// Buffers:
//   @group(0) @binding(0) read_buf      — storage, read (unused, shared layout)
//   @group(0) @binding(1) write_buf     — storage, read_write (post-movement state)
//   @group(0) @binding(2) materials     — storage, read (3x vec4<f32> per material)
//   @group(0) @binding(3) cmd_buf       — storage, read (unused, shared layout)
//   @group(0) @binding(4) uniforms      — uniform (tick, material_count)
//   @group(0) @binding(5) rule_lookup   — storage, read (flat 2D: mat_a * count + mat_b -> rule index)
//   @group(0) @binding(6) rule_data     — storage, read (2x vec4<u32> per rule entry)
//
// Algorithm per voxel:
//   1. Read own voxel from write_buf. If air, return.
//   2. Self-decay: if material has nonzero decay_rate, decrement temperature.
//      If temp < decay_threshold, transform to decay_product. Return after decay.
//   3. For each of 6 face-adjacent neighbors:
//      - Read neighbor material from write_buf
//      - Look up rule_lookup[my_mat * material_count + neighbor_mat]
//      - If valid rule: check temp conditions (integer, C-GPU-11), probability (PRNG, C-SIM-4)
//      - If conditions met: apply input_a_becomes and temp_delta
//      - First matching rule wins (ARCH 6.4). Break after first match.
//   4. Write updated voxel to write_buf.

const NO_RULE: u32 = 0xFFFFFFFFu;

// 6 face-adjacent neighbor offsets
const NEIGHBOR_OFFSETS: array<vec3<i32>, 6> = array<vec3<i32>, 6>(
    vec3<i32>(0, -1, 0),  // Down
    vec3<i32>(0, 1, 0),   // Up
    vec3<i32>(0, 0, -1),  // North
    vec3<i32>(0, 0, 1),   // South
    vec3<i32>(1, 0, 0),   // East
    vec3<i32>(-1, 0, 0),  // West
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
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> read_buf: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> write_buf: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> materials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> cmd_buf: array<SimCommand>;
@group(0) @binding(4) var<uniform> reaction_uniforms: ReactionUniforms;
@group(0) @binding(5) var<storage, read> rule_lookup: array<u32>;
@group(0) @binding(6) var<storage, read> rule_data: array<vec4<u32>>;

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pos = vec3<i32>(i32(gid.x), i32(gid.y), i32(gid.z));

    // Bounds check (C-WGSL-6)
    if !in_bounds(pos) {
        return;
    }

    let idx = voxel_index(pos);
    var voxel = write_buf[idx];
    let mat_id = unpack_material_id(voxel);

    // Air doesn't react
    if mat_id == 0u {
        return;
    }

    var my_temp = unpack_temperature(voxel);

    // --- Self-decay ---
    // Read material properties: props_1 = (decay_rate, decay_threshold, decay_product_id, viscosity)
    let props_1 = materials[mat_id * 3u + 1u];
    let decay_rate = u32(props_1.x);
    let decay_threshold = u32(props_1.y);
    let decay_product = u32(props_1.z);

    if decay_rate > 0u {
        // Decrement temperature by decay_rate
        if my_temp > decay_rate {
            my_temp = my_temp - decay_rate;
        } else {
            my_temp = 0u;
        }

        // Check if below decay threshold -> transform
        if my_temp < decay_threshold {
            // Transform to decay product with threshold temperature
            write_buf[idx] = repack_material_temp(voxel, decay_product, my_temp);
            return;
        }

        // Update temperature in voxel (even if no transform)
        voxel = repack_material_temp(voxel, mat_id, my_temp);
    }

    // --- Upward phase change (heating: e.g. Ice->Water, Water->Steam, Stone->Lava) ---
    {
        let props_2 = materials[unpack_material_id(voxel) * 3u + 2u];
        let phase_change_temp_q = u32(props_2.y);
        let phase_change_product = u32(props_2.z);
        if phase_change_temp_q > 0u && my_temp >= phase_change_temp_q {
            voxel = repack_material_temp(voxel, phase_change_product, my_temp);
        }
    }

    // --- Pairwise reactions ---
    let mc = reaction_uniforms.material_count;

    // Check all 6 face neighbors, first match wins
    for (var n = 0u; n < 6u; n = n + 1u) {
        let neighbor_pos = pos + NEIGHBOR_OFFSETS[n];

        if !in_bounds(neighbor_pos) {
            continue;
        }

        let neighbor_idx = voxel_index(neighbor_pos);
        let neighbor_voxel = write_buf[neighbor_idx];
        let neighbor_mat = unpack_material_id(neighbor_voxel);

        // Look up rule index
        let lookup_idx = mat_id * mc + neighbor_mat;
        let rule_idx = rule_lookup[lookup_idx];

        if rule_idx == NO_RULE {
            continue;
        }

        // Read rule data: 2x vec4<u32> at rule_idx * 2 and rule_idx * 2 + 1
        let rule_0 = rule_data[rule_idx * 2u];
        let rule_1 = rule_data[rule_idx * 2u + 1u];

        let input_a_becomes = rule_0.x;
        let probability_u32 = rule_0.w;

        // Temperature conditions (integer comparison, C-GPU-11)
        let min_temp = rule_1.z;
        let max_temp = rule_1.w;

        if min_temp > 0u && my_temp < min_temp {
            continue;
        }
        if max_temp > 0u && my_temp > max_temp {
            continue;
        }

        // Probability check (C-SIM-4: deterministic PRNG)
        if probability_u32 < 0xFFFFFFFFu {
            let h = sim_hash(pos.x + i32(n), pos.y, pos.z, reaction_uniforms.tick);
            if h > probability_u32 {
                continue;
            }
        }

        // Apply reaction: transform own voxel
        let temp_delta = bitcast<i32>(rule_1.x);
        var new_temp = i32(my_temp) + temp_delta;
        new_temp = clamp(new_temp, 0, 4095);

        voxel = repack_material_temp(voxel, input_a_becomes, u32(new_temp));

        // First matching rule wins
        break;
    }

    write_buf[idx] = voxel;
}
