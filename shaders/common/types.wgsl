// types.wgsl — Shared voxel data types and unpack functions.
// Must produce identical results to alkahest-core/src/math.rs.
// VoxelData is packed as vec2<u32>: .x = low word, .y = high word.
//
// Bit layout:
//   low  [0:15]   material_id (u16)
//   low  [16:27]  temperature (12-bit quantized)
//   low  [28:31]  velocity_x low 4 bits
//   high [0:3]    velocity_x high 4 bits
//   high [4:11]   velocity_y (i8)
//   high [12:19]  velocity_z (i8)
//   high [20:25]  pressure (6-bit)
//   high [26:31]  flags (6-bit)

fn unpack_material_id(v: vec2<u32>) -> u32 {
    return v.x & 0xFFFFu;
}

fn unpack_temperature(v: vec2<u32>) -> u32 {
    return (v.x >> 16u) & 0xFFFu;
}

fn unpack_vel_x(v: vec2<u32>) -> i32 {
    // vel_x straddles the u32 boundary: low 4 bits from low[28:31], high 4 from high[0:3]
    let vx_low = (v.x >> 28u) & 0xFu;
    let vx_high = v.y & 0xFu;
    let vx_u8 = vx_low | (vx_high << 4u);
    // Sign-extend from 8-bit to i32
    return i32(vx_u8) - select(0, 256, vx_u8 >= 128u);
}

fn unpack_vel_y(v: vec2<u32>) -> i32 {
    let vy_u8 = (v.y >> 4u) & 0xFFu;
    return i32(vy_u8) - select(0, 256, vy_u8 >= 128u);
}

fn unpack_vel_z(v: vec2<u32>) -> i32 {
    let vz_u8 = (v.y >> 12u) & 0xFFu;
    return i32(vz_u8) - select(0, 256, vz_u8 >= 128u);
}

fn unpack_pressure(v: vec2<u32>) -> u32 {
    return (v.y >> 20u) & 0x3Fu;
}

fn unpack_flags(v: vec2<u32>) -> u32 {
    return (v.y >> 26u) & 0x3Fu;
}
