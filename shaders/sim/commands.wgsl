// commands.wgsl — Pass 1: Player command application.
// Reads the command buffer and writes voxels into the write buffer.
// Workgroup: 64x1x1 (one thread per command, max 64 commands).
//
// Buffers:
//   @group(0) @binding(0) read_buf    — storage, read
//   @group(0) @binding(1) write_buf   — storage, read_write
//   @group(0) @binding(2) materials   — storage, read (material properties)
//   @group(0) @binding(3) cmd_buf     — storage, read (command array)
//   @group(0) @binding(4) sim_params  — uniform (tick, command_count, etc.)

// Command tool types
const TOOL_PLACE: u32 = 1u;
const TOOL_REMOVE: u32 = 2u;

// Phase constants
const PHASE_GAS: u32 = 0u;
const PHASE_SOLID: u32 = 2u;
const PHASE_POWDER: u32 = 3u;

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

fn pack_voxel_simple(material_id: u32, temperature: u32) -> vec2<u32> {
    let low = material_id | (temperature << 16u);
    let high = 0u;
    return vec2<u32>(low, high);
}

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
            // PLACE: write material at position with ambient temperature
            let voxel = pack_voxel_simple(cmd.material_id, 150u);
            write_buf[idx] = voxel;
        }
        case 2u: {
            // REMOVE: write air (material 0) at position
            write_buf[idx] = vec2<u32>(0u, 0u);
        }
        default: {}
    }
}
