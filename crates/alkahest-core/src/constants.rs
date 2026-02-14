//! Single source of truth for shared constants (C-DESIGN-3).
//! These values are used by both Rust and WGSL. The build script
//! will inject them into shader preambles in later milestones.

/// Side length of a chunk in voxels.
pub const CHUNK_SIZE: u32 = 32;

/// Bytes per voxel (packed into two u32 values).
pub const VOXEL_BYTES: u32 = 8;

/// Maximum material ID (u16::MAX). Material 0 = air.
pub const MAX_MATERIALS: u32 = 65535;

/// Ambient temperature in Kelvin (~20 °C).
pub const AMBIENT_TEMP_K: f32 = 293.0;

/// Maximum representable temperature in Kelvin.
pub const TEMP_QUANT_MAX_K: f32 = 8000.0;

/// Number of bits used for temperature quantization.
pub const TEMP_QUANT_BITS: u32 = 12;

/// Maximum quantized temperature value (2^12 - 1).
pub const TEMP_QUANT_MAX_VALUE: u16 = 4095;

/// Ambient temperature as a quantized integer: round(293.0 / 8000.0 * 4095.0) = 150.
pub const AMBIENT_TEMP_QUANTIZED: u16 = 150;

/// Total voxels per chunk (32^3).
pub const VOXELS_PER_CHUNK: u32 = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

/// Total bytes per chunk.
pub const BYTES_PER_CHUNK: u32 = VOXELS_PER_CHUNK * VOXEL_BYTES;
