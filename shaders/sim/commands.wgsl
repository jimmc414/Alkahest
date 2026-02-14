// commands.wgsl — Pass 1: Player command application.
// Reads the command buffer and writes voxels into the write buffer.
// Workgroup: 64x1x1 (one thread per command, max 64 commands).
//
// Buffers:
//   @group(0) @binding(0) read_buf    — storage, read
//   @group(0) @binding(1) write_buf   — storage, read_write
//   @group(0) @binding(2) materials   — storage, read (material properties, 3x vec4<f32> per material)
//   @group(0) @binding(3) cmd_buf     — storage, read (command array)
//   @group(0) @binding(4) sim_params  — uniform (tick, command_count, etc.)

// Command tool types
const TOOL_PLACE: u32 = 1u;
const TOOL_REMOVE: u32 = 2u;
const TOOL_HEAT: u32 = 3u;

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

struct SimParams {
    tick: u32,
    command_count: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read> read_buf: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> write_buf: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> materials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> cmd_buf: array<SimCommand>;
@group(0) @binding(4) var<uniform> sim_params: SimParams;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cmd_index = gid.x;
    if cmd_index >= sim_params.command_count {
        return;
    }

    let cmd = cmd_buf[cmd_index];
    let pos = vec3<i32>(cmd.pos_x, cmd.pos_y, cmd.pos_z);

    if !in_bounds(pos) {
        return;
    }

    let idx = voxel_index(pos);

    switch cmd.tool_type {
        case 1u: {
            // PLACE: write material at position
            // For materials with decay (e.g. fire), start at 3x decay_threshold
            // so they don't immediately disappear. Otherwise use ambient temp.
            var temp = 150u; // ambient
            let mat_id = cmd.material_id;
            if mat_id > 0u {
                // props_1 = (decay_rate, decay_threshold, decay_product_id, viscosity)
                let props_1 = materials[mat_id * 3u + 1u];
                let decay_rate = u32(props_1.x);
                let decay_threshold = u32(props_1.y);
                if decay_rate > 0u && decay_threshold > 0u {
                    temp = min(decay_threshold * 3u, 4095u);
                }
            }
            let voxel = pack_voxel(mat_id, temp, 0, 0, 0, 0u, 0u);
            write_buf[idx] = voxel;
        }
        case 2u: {
            // REMOVE: write air (material 0) at position
            write_buf[idx] = vec2<u32>(0u, 0u);
        }
        case 3u: {
            // HEAT: modify temperature of existing voxel
            // material_id field is reused as signed temperature delta
            let current_voxel = write_buf[idx];
            let current_mat = unpack_material_id(current_voxel);
            if current_mat != 0u {
                let current_temp = unpack_temperature(current_voxel);
                let delta = bitcast<i32>(cmd.material_id);
                var new_temp = i32(current_temp) + delta;
                new_temp = clamp(new_temp, 0, 4095);
                write_buf[idx] = repack_material_temp(current_voxel, current_mat, u32(new_temp));
            }
        }
        default: {}
    }
}
