use crate::chunk::ChunkState;
use crate::chunk_map::ChunkMap;
use alkahest_core::constants::*;
use alkahest_core::types::ChunkCoord;
use glam::IVec3;

/// A dispatch entry for one chunk to be simulated this tick.
#[derive(Debug, Clone)]
pub struct DispatchEntry {
    /// Chunk coordinate.
    pub coord: ChunkCoord,
    /// Pool slot index for this chunk's voxel data.
    pub pool_slot: u32,
    /// Pool slot byte offsets of the 26 neighbors.
    /// Order matches compute_neighbor_dir() in coords.wgsl:
    /// enumerate (dz,dy,dx) in {-1,0,1}^3 skipping (0,0,0).
    /// SENTINEL_NEIGHBOR if neighbor is unloaded or out of bounds.
    pub neighbor_slot_offsets: [u32; 26],
}

/// List of chunks to dispatch to the GPU simulation this frame.
#[derive(Debug, Clone)]
pub struct DispatchList {
    pub entries: Vec<DispatchEntry>,
}

impl Default for DispatchList {
    fn default() -> Self {
        Self::new()
    }
}

impl DispatchList {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Number of chunks to dispatch.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build the chunk descriptor buffer data (u32 array) for uploading to GPU.
    /// Each entry is CHUNK_DESC_STRIDE u32s:
    ///   [0]: pool_slot_byte_offset
    ///   [1..27]: neighbor_pool_slot_byte_offsets (26 neighbors)
    ///   [27..32]: padding (zeros)
    pub fn build_descriptor_data(&self) -> Vec<u32> {
        let stride = CHUNK_DESC_STRIDE as usize;
        let mut data = vec![0u32; self.entries.len() * stride];
        for (i, entry) in self.entries.iter().enumerate() {
            let base = i * stride;
            // Slot offset in bytes
            data[base] = entry.pool_slot * BYTES_PER_CHUNK;
            for (n, &neighbor_offset) in entry.neighbor_slot_offsets.iter().enumerate() {
                data[base + 1 + n] = neighbor_offset;
            }
            // Remaining [27..32] are padding, left as 0
        }
        data
    }
}

/// Build a dispatch list from the current chunk map state.
/// Only Active chunks are dispatched. All 26 neighbors are resolved.
pub fn build_dispatch_list(chunk_map: &ChunkMap) -> DispatchList {
    let mut list = DispatchList::new();

    for (coord, chunk) in chunk_map.iter() {
        if chunk.state != ChunkState::Active {
            continue;
        }

        let pool_slot = match chunk.pool_slot {
            Some(s) => s,
            None => continue,
        };

        // Resolve all 26 neighbors in the same order as compute_neighbor_dir() in coords.wgsl:
        // Iterate (dz, dy, dx) in {-1,0,1}^3, skipping (0,0,0).
        let mut neighbor_slot_offsets = [SENTINEL_NEIGHBOR; 26];
        let mut idx = 0usize;
        for dz in -1..=1i32 {
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let neighbor_coord = *coord + IVec3::new(dx, dy, dz);
                    if ChunkMap::in_world_bounds(&neighbor_coord) {
                        if let Some(neighbor_chunk) = chunk_map.get(&neighbor_coord) {
                            if let Some(slot) = neighbor_chunk.pool_slot {
                                neighbor_slot_offsets[idx] = slot * BYTES_PER_CHUNK;
                            }
                        }
                    }
                    idx += 1;
                }
            }
        }

        list.entries.push(DispatchEntry {
            coord: *coord,
            pool_slot,
            neighbor_slot_offsets,
        });
    }

    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_list_active_only() {
        let mut map = ChunkMap::with_capacity(64);
        let c0 = IVec3::new(0, 0, 0);
        let c1 = IVec3::new(1, 0, 0);
        map.load_chunk(c0);
        map.load_chunk(c1);

        // Put c1 to sleep
        if let Some(chunk) = map.get_mut(&c1) {
            chunk.sleep();
        }

        let list = build_dispatch_list(&map);
        // Only c0 should be dispatched (c1 is Static)
        assert_eq!(list.len(), 1);
        assert_eq!(list.entries[0].coord, c0);
    }

    #[test]
    fn test_dispatch_list_construction() {
        let mut map = ChunkMap::with_capacity(64);
        let c0 = IVec3::new(1, 1, 1);
        let c_right = IVec3::new(2, 1, 1);

        let slot0 = map.load_chunk(c0).expect("slot for c0");
        let slot_right = map.load_chunk(c_right).expect("slot for c_right");

        let list = build_dispatch_list(&map);
        let entry = list
            .entries
            .iter()
            .find(|e| e.coord == c0)
            .expect("c0 entry");
        assert_eq!(entry.pool_slot, slot0);

        // +X neighbor (dx=1, dy=0, dz=0) should resolve to c_right's slot byte offset.
        // In the 3x3x3 enumeration (dz,dy,dx), skipping center at flat=13:
        // dz=0,dy=0,dx=1 → flat = 1*9 + 1*3 + 2 = 14, index = 14-1 = 13
        let expected_offset = slot_right * BYTES_PER_CHUNK;
        assert_eq!(entry.neighbor_slot_offsets[13], expected_offset);

        // Out-of-bounds or unloaded neighbors should be SENTINEL_NEIGHBOR
        // (-X neighbor at (0,1,1) is not loaded)
        // dz=0,dy=0,dx=-1 → flat = 1*9 + 1*3 + 0 = 12, index = 12
        assert_eq!(entry.neighbor_slot_offsets[12], SENTINEL_NEIGHBOR);
    }

    #[test]
    fn test_dispatch_list_empty_when_all_static() {
        let mut map = ChunkMap::with_capacity(64);
        let c0 = IVec3::new(0, 0, 0);
        let c1 = IVec3::new(1, 0, 0);
        let c2 = IVec3::new(2, 0, 0);
        map.load_chunk(c0);
        map.load_chunk(c1);
        map.load_chunk(c2);

        // Put all chunks to sleep
        for coord in &[c0, c1, c2] {
            if let Some(chunk) = map.get_mut(coord) {
                chunk.sleep();
            }
        }

        let list = build_dispatch_list(&map);
        assert!(
            list.is_empty(),
            "dispatch list should be empty when all chunks are Static"
        );
    }

    #[test]
    fn test_descriptor_data_layout() {
        let mut map = ChunkMap::with_capacity(64);
        map.load_chunk(IVec3::new(0, 0, 0));
        let list = build_dispatch_list(&map);

        let data = list.build_descriptor_data();
        let stride = CHUNK_DESC_STRIDE as usize;
        assert_eq!(data.len(), stride);
        // First entry is pool_slot * BYTES_PER_CHUNK
        assert_eq!(data[0], list.entries[0].pool_slot * BYTES_PER_CHUNK);
        // All neighbors of corner chunk should be sentinel (no loaded neighbors)
        for i in 1..27 {
            // Some might be sentinel, some might not depending on which direction
            // At (0,0,0), all negative-direction neighbors are out of bounds
            // and no positive neighbors are loaded
            assert_eq!(data[i], SENTINEL_NEIGHBOR);
        }
    }
}
