// commands.wgsl — Pass 1: Player command application (M5: multi-chunk).
// Reads the command buffer and writes voxels into the write pool.
// Workgroup: 64x1x1 (one thread per command, max 64 commands).
//
// Buffers: see binding layout below (8 bindings).

const TOOL_PLACE: u32 = 1u;
const TOOL_REMOVE: u32 = 2u;
const TOOL_HEAT: u32 = 3u;

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

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cmd_index = gid.x;
    if cmd_index >= sim_params.command_count {
        return;
    }

    let cmd = cmd_buf[cmd_index];
    let pos = vec3<i32>(cmd.pos_x, cmd.pos_y, cmd.pos_z);
    let chunk_idx = cmd.chunk_dispatch_idx;

    if !in_bounds(pos) {
        return;
    }

    let idx = write_pool_voxel_index(pos, chunk_idx);

    switch cmd.tool_type {
        case 1u: {
            // PLACE: write material at position
            // For materials with decay (e.g. fire), start at 3x decay_threshold
            // so they don't immediately disappear. Otherwise use ambient temp.
            var temp = 150u;
            let mat_id = cmd.material_id;
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
        case 2u: {
            // REMOVE: write air
            write_pool[idx] = vec2<u32>(0u, 0u);
        }
        case 3u: {
            // HEAT: modify temperature of existing voxel
            let current_voxel = write_pool[idx];
            let current_mat = unpack_material_id(current_voxel);
            if current_mat != 0u {
                let current_temp = unpack_temperature(current_voxel);
                let delta = bitcast<i32>(cmd.material_id);
                var new_temp = i32(current_temp) + delta;
                new_temp = clamp(new_temp, 0, 4095);
                write_pool[idx] = repack_material_temp(current_voxel, current_mat, u32(new_temp));
            }
        }
        default: {}
    }
}
