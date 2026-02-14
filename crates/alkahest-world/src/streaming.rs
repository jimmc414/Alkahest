use crate::chunk_map::ChunkMap;

/// Policy for loading/unloading chunks based on camera position.
/// In the initial implementation, all world chunks are loaded at init
/// and streaming is a no-op. Future milestones may implement distance-based
/// streaming for larger worlds.
pub struct StreamingPolicy {
    // Reserved for future use (e.g., load distance, unload distance)
}

impl Default for StreamingPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingPolicy {
    pub fn new() -> Self {
        Self {}
    }

    /// Update streaming state. Currently a no-op since all chunks are loaded.
    pub fn update(&mut self, _chunk_map: &mut ChunkMap, _camera_pos: glam::Vec3) {
        // M5 initial: all chunks loaded at init, no streaming needed.
        // Future: implement distance-based load/unload here.
    }
}
