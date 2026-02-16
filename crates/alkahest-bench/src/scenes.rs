use alkahest_core::constants::{
    AMBIENT_TEMP_QUANTIZED, CHUNK_SIZE, VOXELS_PER_CHUNK, WORLD_CHUNKS_X, WORLD_CHUNKS_Y,
    WORLD_CHUNKS_Z,
};
use alkahest_core::math::pack_voxel;
use alkahest_core::types::MaterialId;

/// Configuration for a single benchmark scene.
pub struct SceneConfig {
    pub name: &'static str,
    pub target_active_voxels: u32,
    pub camera_position: [f32; 3],
    pub camera_target: [f32; 3],
}

/// Return the standard suite of benchmark scenes (100K to 3M active voxels).
pub fn standard_scenes() -> Vec<SceneConfig> {
    let cx = (WORLD_CHUNKS_X * CHUNK_SIZE) as f32 / 2.0;
    let cy = (WORLD_CHUNKS_Y * CHUNK_SIZE) as f32 / 4.0;
    let cz = (WORLD_CHUNKS_Z * CHUNK_SIZE) as f32 / 2.0;

    vec![
        SceneConfig {
            name: "100K",
            target_active_voxels: 100_000,
            camera_position: [cx + 40.0, cy + 30.0, cz + 40.0],
            camera_target: [cx, cy, cz],
        },
        SceneConfig {
            name: "250K",
            target_active_voxels: 250_000,
            camera_position: [cx + 60.0, cy + 40.0, cz + 60.0],
            camera_target: [cx, cy, cz],
        },
        SceneConfig {
            name: "500K",
            target_active_voxels: 500_000,
            camera_position: [cx + 80.0, cy + 50.0, cz + 80.0],
            camera_target: [cx, cy, cz],
        },
        SceneConfig {
            name: "1M",
            target_active_voxels: 1_000_000,
            camera_position: [cx + 100.0, cy + 60.0, cz + 100.0],
            camera_target: [cx, cy, cz],
        },
        SceneConfig {
            name: "2M",
            target_active_voxels: 2_000_000,
            camera_position: [cx + 120.0, cy + 70.0, cz + 120.0],
            camera_target: [cx, cy, cz],
        },
        SceneConfig {
            name: "3M",
            target_active_voxels: 3_000_000,
            camera_position: [cx + 140.0, cy + 80.0, cz + 140.0],
            camera_target: [cx, cy, cz],
        },
    ]
}

/// Number of chunks needed to hold the target voxel count.
pub fn chunks_needed(target_voxels: u32) -> u32 {
    target_voxels.div_ceil(VOXELS_PER_CHUNK)
}

/// Generate voxel data for a single chunk with a deterministic material mix.
/// Material distribution: ~40% sand(2), ~20% water(3), ~15% wood(8)+fire(5),
/// ~15% stone(1), ~10% smoke(6).
/// Only fills voxels up to `fill_count` (rest are air).
pub fn generate_bench_chunk(chunk_index: u32, fill_count: u32) -> Vec<[u32; 2]> {
    let mut data = vec![[0u32; 2]; VOXELS_PER_CHUNK as usize];
    let fill = fill_count.min(VOXELS_PER_CHUNK) as usize;

    for (i, slot) in data.iter_mut().enumerate().take(fill) {
        // Deterministic pseudo-random material selection based on position
        let hash = ((chunk_index as usize).wrapping_mul(31337) ^ i.wrapping_mul(7919)) % 100;
        let (mat_id, temp) = match hash {
            0..=39 => (2u16, AMBIENT_TEMP_QUANTIZED), // Sand
            40..=59 => (3, AMBIENT_TEMP_QUANTIZED),   // Water
            60..=67 => (8, AMBIENT_TEMP_QUANTIZED),   // Wood
            68..=74 => (5, 2000),                     // Fire (hot)
            75..=89 => (1, AMBIENT_TEMP_QUANTIZED),   // Stone
            _ => (6, AMBIENT_TEMP_QUANTIZED + 50),    // Smoke (slightly warm)
        };

        let flags = 0x01u8; // active flag set
        let voxel = pack_voxel(MaterialId(mat_id), temp, 0, 0, 0, 0, flags);
        *slot = [voxel.low, voxel.high];
    }

    data
}

/// Calculate the number of chunks to use within world bounds.
pub fn scene_chunk_count(config: &SceneConfig) -> u32 {
    let max_chunks = WORLD_CHUNKS_X * WORLD_CHUNKS_Y * WORLD_CHUNKS_Z;
    chunks_needed(config.target_active_voxels).min(max_chunks)
}
