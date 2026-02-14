pub mod chunk;
pub mod chunk_map;
pub mod dispatch;
pub mod state_machine;
pub mod streaming;
pub mod terrain;

use alkahest_core::constants::*;
use alkahest_core::types::ChunkCoord;
use chunk_map::ChunkMap;
use dispatch::DispatchList;
use glam::IVec3;
use streaming::StreamingPolicy;
use terrain::TerrainGenerator;

/// Primary public struct for the alkahest-world crate.
/// Manages chunk lifecycle, terrain generation, and dispatch list building.
pub struct World {
    chunk_map: ChunkMap,
    terrain: TerrainGenerator,
    streaming: StreamingPolicy,
    /// Activity flags read back from GPU (one u32 per active chunk).
    activity_flags: Vec<u32>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// Create a new world and generate initial terrain chunks around the origin.
    pub fn new() -> Self {
        let mut chunk_map = ChunkMap::new();
        let terrain = TerrainGenerator::new(42); // fixed seed for determinism
        let streaming = StreamingPolicy::new();

        // Generate all chunks in the world grid
        for cx in 0..WORLD_CHUNKS_X as i32 {
            for cy in 0..WORLD_CHUNKS_Y as i32 {
                for cz in 0..WORLD_CHUNKS_Z as i32 {
                    let coord = IVec3::new(cx, cy, cz);
                    chunk_map.load_chunk(coord);
                }
            }
        }

        Self {
            chunk_map,
            terrain,
            streaming,
            activity_flags: Vec::new(),
        }
    }

    /// Generate terrain voxel data for a specific chunk.
    pub fn generate_chunk_data(&self, coord: ChunkCoord) -> Vec<[u32; 2]> {
        self.terrain.generate_chunk(coord)
    }

    /// Process activity flags read back from the GPU.
    /// Updates chunk states (active/static transitions, activation propagation).
    pub fn process_activity(&mut self, flags: &[u32]) {
        self.activity_flags = flags.to_vec();
        state_machine::process_activity_flags(&mut self.chunk_map, flags);
    }

    /// Update world state and return a dispatch list for the simulation pipeline.
    /// Called once per frame before sim tick.
    pub fn update(&mut self, camera_pos: glam::Vec3) -> DispatchList {
        // Update streaming (load/unload based on camera)
        self.streaming.update(&mut self.chunk_map, camera_pos);

        // Build dispatch list from active chunks
        dispatch::build_dispatch_list(&self.chunk_map)
    }

    /// Get the chunk map for reading.
    pub fn chunk_map(&self) -> &ChunkMap {
        &self.chunk_map
    }

    /// Get mutable chunk map.
    pub fn chunk_map_mut(&mut self) -> &mut ChunkMap {
        &mut self.chunk_map
    }

    /// Get the terrain generator (for generating chunk data on demand).
    pub fn terrain(&self) -> &TerrainGenerator {
        &self.terrain
    }

    /// Get counts for debug display: (total_loaded, active, static_count)
    pub fn chunk_counts(&self) -> (u32, u32, u32) {
        self.chunk_map.chunk_counts()
    }
}
