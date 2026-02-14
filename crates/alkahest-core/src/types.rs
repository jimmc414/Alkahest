use glam::IVec3;

/// Newtype for material identifiers. 0 = air/empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MaterialId(pub u16);

/// Chunk coordinate in chunk-space (each unit = CHUNK_SIZE voxels).
pub type ChunkCoord = IVec3;

/// World coordinate in voxel-space.
pub type WorldCoord = IVec3;

/// Packed voxel data: 8 bytes stored as two u32 values.
///
/// Bit layout (matching architecture.md spec):
///   low  [0:15]   material_id (u16)
///   low  [16:27]  temperature (12-bit quantized)
///   low  [28:31]  velocity_x low 4 bits
///   high [0:3]    velocity_x high 4 bits
///   high [4:11]   velocity_y (i8)
///   high [12:19]  velocity_z (i8)
///   high [20:25]  pressure (6-bit)
///   high [26:31]  flags (6-bit)
///
/// velocity_x straddles the u32 boundary intentionally — this matches
/// the spec in architecture.md and the WGSL shader vec2<u32> access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct VoxelData {
    pub low: u32,
    pub high: u32,
}
