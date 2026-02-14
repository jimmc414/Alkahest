use crate::constants::{CHUNK_SIZE, TEMP_QUANT_MAX_K, TEMP_QUANT_MAX_VALUE};
use crate::types::{ChunkCoord, MaterialId, VoxelData, WorldCoord};
use glam::IVec3;

/// Convert a temperature in Kelvin to a 12-bit quantized integer.
/// Clamps to [0, TEMP_QUANT_MAX_VALUE].
pub fn temp_to_quantized(kelvin: f32) -> u16 {
    let clamped = kelvin.clamp(0.0, TEMP_QUANT_MAX_K);
    let ratio = clamped / TEMP_QUANT_MAX_K;
    let quantized = (ratio * TEMP_QUANT_MAX_VALUE as f32).round() as u16;
    quantized.min(TEMP_QUANT_MAX_VALUE)
}

/// Convert a 12-bit quantized integer back to temperature in Kelvin.
pub fn temp_from_quantized(quantized: u16) -> f32 {
    let clamped = quantized.min(TEMP_QUANT_MAX_VALUE);
    (clamped as f32 / TEMP_QUANT_MAX_VALUE as f32) * TEMP_QUANT_MAX_K
}

/// Convert a world-space voxel coordinate to its containing chunk coordinate.
pub fn world_to_chunk(world: WorldCoord) -> ChunkCoord {
    let cs = CHUNK_SIZE as i32;
    IVec3::new(
        world.x.div_euclid(cs),
        world.y.div_euclid(cs),
        world.z.div_euclid(cs),
    )
}

/// Convert a world-space voxel coordinate to its local offset within a chunk.
pub fn world_to_local(world: WorldCoord) -> IVec3 {
    let cs = CHUNK_SIZE as i32;
    IVec3::new(
        world.x.rem_euclid(cs),
        world.y.rem_euclid(cs),
        world.z.rem_euclid(cs),
    )
}

/// Convert a chunk coordinate and local offset back to world-space.
pub fn chunk_local_to_world(chunk: ChunkCoord, local: IVec3) -> WorldCoord {
    let cs = CHUNK_SIZE as i32;
    IVec3::new(
        chunk.x * cs + local.x,
        chunk.y * cs + local.y,
        chunk.z * cs + local.z,
    )
}

/// Pack voxel fields into the two-u32 representation.
///
/// velocity_x straddles the u32 boundary: low 4 bits in `low[28:31]`,
/// high 4 bits in `high[0:3]`.
pub fn pack_voxel(
    material_id: MaterialId,
    temperature: u16,
    vel_x: i8,
    vel_y: i8,
    vel_z: i8,
    pressure: u8,
    flags: u8,
) -> VoxelData {
    let mat = material_id.0 as u32;
    let temp = (temperature.min(TEMP_QUANT_MAX_VALUE)) as u32;
    let vx = vel_x as u8 as u32; // reinterpret i8 as u8 for bit packing

    // low word: material[0:15] | temperature[16:27] | vel_x_low[28:31]
    let low = mat | (temp << 16) | ((vx & 0x0F) << 28);

    let vy = vel_y as u8 as u32;
    let vz = vel_z as u8 as u32;
    let pr = (pressure & 0x3F) as u32; // 6 bits
    let fl = (flags & 0x3F) as u32; // 6 bits

    // high word: vel_x_high[0:3] | vel_y[4:11] | vel_z[12:19] | pressure[20:25] | flags[26:31]
    let high = ((vx >> 4) & 0x0F) | (vy << 4) | (vz << 12) | (pr << 20) | (fl << 26);

    VoxelData { low, high }
}

/// Unpack voxel fields from the two-u32 representation.
///
/// Returns (material_id, temperature, vel_x, vel_y, vel_z, pressure, flags).
pub fn unpack_voxel(voxel: VoxelData) -> (MaterialId, u16, i8, i8, i8, u8, u8) {
    let material_id = MaterialId((voxel.low & 0xFFFF) as u16);
    let temperature = ((voxel.low >> 16) & 0x0FFF) as u16;

    // vel_x straddles boundary: low 4 bits from low[28:31], high 4 bits from high[0:3]
    let vx_low = (voxel.low >> 28) & 0x0F;
    let vx_high = voxel.high & 0x0F;
    let vx_u8 = (vx_low | (vx_high << 4)) as u8;
    let vel_x = vx_u8 as i8; // reinterpret u8 as i8

    let vel_y = ((voxel.high >> 4) & 0xFF) as u8 as i8;
    let vel_z = ((voxel.high >> 12) & 0xFF) as u8 as i8;
    let pressure = ((voxel.high >> 20) & 0x3F) as u8;
    let flags = ((voxel.high >> 26) & 0x3F) as u8;

    (
        material_id,
        temperature,
        vel_x,
        vel_y,
        vel_z,
        pressure,
        flags,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::AMBIENT_TEMP_K;

    #[test]
    fn test_temp_roundtrip_ambient() {
        let q = temp_to_quantized(AMBIENT_TEMP_K);
        assert_eq!(q, 150);
        let back = temp_from_quantized(q);
        // Allow small quantization error (~2K resolution)
        assert!((back - AMBIENT_TEMP_K).abs() < 3.0, "got {back}");
    }

    #[test]
    fn test_temp_edge_cases() {
        assert_eq!(temp_to_quantized(0.0), 0);
        assert_eq!(temp_to_quantized(TEMP_QUANT_MAX_K), TEMP_QUANT_MAX_VALUE);
        assert_eq!(temp_to_quantized(-100.0), 0); // clamps negative
        assert_eq!(temp_to_quantized(99999.0), TEMP_QUANT_MAX_VALUE); // clamps over max

        assert_eq!(temp_from_quantized(0), 0.0);
        assert_eq!(temp_from_quantized(TEMP_QUANT_MAX_VALUE), TEMP_QUANT_MAX_K);
    }

    #[test]
    fn test_world_to_chunk_positive() {
        assert_eq!(world_to_chunk(IVec3::new(0, 0, 0)), IVec3::ZERO);
        assert_eq!(world_to_chunk(IVec3::new(31, 31, 31)), IVec3::ZERO);
        assert_eq!(world_to_chunk(IVec3::new(32, 0, 0)), IVec3::new(1, 0, 0));
    }

    #[test]
    fn test_world_to_chunk_negative() {
        assert_eq!(world_to_chunk(IVec3::new(-1, 0, 0)), IVec3::new(-1, 0, 0));
        assert_eq!(world_to_chunk(IVec3::new(-32, 0, 0)), IVec3::new(-1, 0, 0));
        assert_eq!(world_to_chunk(IVec3::new(-33, 0, 0)), IVec3::new(-2, 0, 0));
    }

    #[test]
    fn test_world_to_local_positive() {
        assert_eq!(world_to_local(IVec3::new(0, 0, 0)), IVec3::ZERO);
        assert_eq!(world_to_local(IVec3::new(5, 10, 31)), IVec3::new(5, 10, 31));
        assert_eq!(world_to_local(IVec3::new(33, 0, 0)), IVec3::new(1, 0, 0));
    }

    #[test]
    fn test_world_to_local_negative() {
        assert_eq!(world_to_local(IVec3::new(-1, 0, 0)), IVec3::new(31, 0, 0));
        assert_eq!(world_to_local(IVec3::new(-32, 0, 0)), IVec3::new(0, 0, 0));
    }

    #[test]
    fn test_chunk_local_roundtrip() {
        let world = IVec3::new(-50, 100, 3);
        let chunk = world_to_chunk(world);
        let local = world_to_local(world);
        let back = chunk_local_to_world(chunk, local);
        assert_eq!(back, world);
    }

    #[test]
    fn test_pack_unpack_zeros() {
        let voxel = pack_voxel(MaterialId(0), 0, 0, 0, 0, 0, 0);
        assert_eq!(voxel, VoxelData { low: 0, high: 0 });
        let (mat, temp, vx, vy, vz, pr, fl) = unpack_voxel(voxel);
        assert_eq!(mat, MaterialId(0));
        assert_eq!(temp, 0);
        assert_eq!(vx, 0);
        assert_eq!(vy, 0);
        assert_eq!(vz, 0);
        assert_eq!(pr, 0);
        assert_eq!(fl, 0);
    }

    #[test]
    fn test_pack_unpack_max_values() {
        let voxel = pack_voxel(
            MaterialId(65535),
            TEMP_QUANT_MAX_VALUE,
            127,
            127,
            127,
            63,
            63,
        );
        let (mat, temp, vx, vy, vz, pr, fl) = unpack_voxel(voxel);
        assert_eq!(mat, MaterialId(65535));
        assert_eq!(temp, TEMP_QUANT_MAX_VALUE);
        assert_eq!(vx, 127);
        assert_eq!(vy, 127);
        assert_eq!(vz, 127);
        assert_eq!(pr, 63);
        assert_eq!(fl, 63);
    }

    #[test]
    fn test_pack_unpack_negative_velocities() {
        let voxel = pack_voxel(MaterialId(1), 150, -128, -1, -50, 10, 5);
        let (mat, temp, vx, vy, vz, pr, fl) = unpack_voxel(voxel);
        assert_eq!(mat, MaterialId(1));
        assert_eq!(temp, 150);
        assert_eq!(vx, -128);
        assert_eq!(vy, -1);
        assert_eq!(vz, -50);
        assert_eq!(pr, 10);
        assert_eq!(fl, 5);
    }

    #[test]
    fn test_pack_unpack_vel_x_boundary_cases() {
        // vel_x straddles the u32 boundary — test edge cases
        for val in [-128i8, -1, 0, 1, 15, 16, 127] {
            let voxel = pack_voxel(MaterialId(42), 100, val, 0, 0, 0, 0);
            let (_, _, vx, _, _, _, _) = unpack_voxel(voxel);
            assert_eq!(vx, val, "vel_x roundtrip failed for {val}");
        }
    }

    #[test]
    fn test_pack_unpack_typical_voxel() {
        // Sand at ambient temp, falling at 1 voxel/tick
        let voxel = pack_voxel(
            MaterialId(2), // sand
            150,           // ambient temp quantized
            0,
            -1, // falling down
            0,
            0,
            0b000001, // active flag
        );
        let (mat, temp, vx, vy, vz, pr, fl) = unpack_voxel(voxel);
        assert_eq!(mat, MaterialId(2));
        assert_eq!(temp, 150);
        assert_eq!(vx, 0);
        assert_eq!(vy, -1);
        assert_eq!(vz, 0);
        assert_eq!(pr, 0);
        assert_eq!(fl, 1);
    }

    #[test]
    fn test_voxel_data_size() {
        assert_eq!(std::mem::size_of::<super::VoxelData>(), 8);
    }
}
