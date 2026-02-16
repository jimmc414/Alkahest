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

/// Maximum number of materials supported by the rule engine.
/// Used to size the interaction lookup table (MAX_RULE_MATERIALS^2 entries).
pub const MAX_RULE_MATERIALS: u32 = 1024;

/// Sentinel value in the rule lookup buffer: no rule exists for this pair.
pub const NO_RULE: u32 = 0xFFFFFFFF;

/// Sentinel value in rule data: material unchanged by this rule.
pub const MATERIAL_UNCHANGED: u32 = 0xFFFF;

/// Thermal diffusion rate per tick. CFL constraint: DIFFUSION_RATE * max_conductivity * 26 < 1.0.
pub const DIFFUSION_RATE: f32 = 0.03;

/// Per-tick temperature drain toward ambient (quantized units).
pub const ENTROPY_DRAIN_RATE: u32 = 1;

/// Temperature delta above ambient that triggers convection (quantized units).
pub const CONVECTION_THRESHOLD: u32 = 50;

/// Heat tool: signed temperature delta applied per command (quantized units, ~1000K).
pub const TOOL_HEAT_DELTA: i32 = 500;

/// Freeze tool: signed temperature delta applied per command (quantized units, ~400K).
pub const TOOL_FREEZE_DELTA: i32 = -200;

// ── M5: Multi-Chunk World Constants ──────────────────────────────────

/// World dimensions in chunks.
pub const WORLD_CHUNKS_X: u32 = 8;
pub const WORLD_CHUNKS_Y: u32 = 4;
pub const WORLD_CHUNKS_Z: u32 = 8;

/// Maximum pool slots for chunk voxel data. Actual capacity may be lower
/// due to `maxBufferSize` limits queried at init (C-GPU-2).
pub const MAX_CHUNK_SLOTS: u32 = 256;

/// Number of ticks a chunk stays active with no voxel changes before
/// transitioning to Static (sleep).
pub const CHUNK_SLEEP_TICKS: u32 = 8;

/// Sentinel value for unloaded neighbor slots in the chunk descriptor buffer.
/// Shaders treat this as air (C-SIM-7).
pub const SENTINEL_NEIGHBOR: u32 = 0xFFFFFFFF;

/// Stride in u32s per chunk descriptor entry (128 bytes = 32 × u32).
/// Layout: [0] pool_slot_offset, [1..27] neighbor_pool_slot_offsets, [27..31] padding.
pub const CHUNK_DESC_STRIDE: u32 = 32;

/// Stride in u32s per chunk activity flags entry.
pub const ACTIVITY_FLAGS_STRIDE: u32 = 1;

// ── M6: Pressure and Structural Integrity Constants ─────────────────

/// Pressure diffusion rate per tick. Controls how fast pressure equalizes.
pub const PRESSURE_DIFFUSION_RATE: f32 = 0.15;

/// Maximum pressure value (6-bit field, 0–63).
pub const MAX_PRESSURE: u32 = 63;

/// Pressure gain per tick for enclosed gas/liquid above ambient temperature.
pub const THERMAL_PRESSURE_FACTOR: u32 = 1;

/// Maximum BFS radius for structural collapse detection (CPU-side).
pub const STRUCTURAL_BFS_RADIUS: u32 = 32;

// ── M10: Rendering Polish Constants ─────────────────────────────────

/// Maximum number of dynamic point lights extracted from emissive voxels.
pub const MAX_DYNAMIC_LIGHTS: u32 = 64;

/// Maximum shadow rays traced per pixel (C-RENDER-4: budget ≤ 8).
pub const MAX_SHADOW_RAYS_PER_PIXEL: u32 = 4;

/// Maximum transparent voxel traversals per ray before cutoff.
pub const MAX_TRANSPARENT_STEPS: u32 = 32;

/// LOD distance threshold in voxels. Beyond this, use octree averaged color.
pub const LOD_DISTANCE_THRESHOLD: f32 = 128.0;
