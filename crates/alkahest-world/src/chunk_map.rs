use crate::chunk::{Chunk, ChunkState};
use alkahest_core::constants::*;
use alkahest_core::types::ChunkCoord;
use glam::IVec3;
use std::collections::HashMap;

/// Spatial container for all chunks in the world.
pub struct ChunkMap {
    chunks: HashMap<ChunkCoord, Chunk>,
    /// Simple slot allocator: stack of free slot indices.
    free_slots: Vec<u32>,
    /// Total number of slots in the pool.
    slot_capacity: u32,
}

impl Default for ChunkMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkMap {
    pub fn new() -> Self {
        Self::with_capacity(MAX_CHUNK_SLOTS)
    }

    pub fn with_capacity(slot_capacity: u32) -> Self {
        let free_slots = (0..slot_capacity).rev().collect();
        Self {
            chunks: HashMap::new(),
            free_slots,
            slot_capacity,
        }
    }

    /// Set the actual pool capacity (called after GPU pool creation).
    pub fn set_capacity(&mut self, capacity: u32) {
        self.slot_capacity = capacity;
        self.free_slots.clear();
        // Rebuild free list excluding already-allocated slots
        let used: std::collections::HashSet<u32> =
            self.chunks.values().filter_map(|c| c.pool_slot).collect();
        for slot in (0..capacity).rev() {
            if !used.contains(&slot) {
                self.free_slots.push(slot);
            }
        }
    }

    /// Allocate a pool slot. Returns None if pool is full.
    pub fn alloc_slot(&mut self) -> Option<u32> {
        self.free_slots.pop()
    }

    /// Free a pool slot back to the allocator.
    pub fn free_slot(&mut self, slot: u32) {
        self.free_slots.push(slot);
    }

    /// Load a chunk at the given coordinate (transitions to Active).
    /// Returns the allocated pool slot, or None if the pool is full.
    pub fn load_chunk(&mut self, coord: ChunkCoord) -> Option<u32> {
        if self.chunks.contains_key(&coord) {
            // Already loaded — just activate it
            if let Some(chunk) = self.chunks.get_mut(&coord) {
                chunk.activate();
                return chunk.pool_slot;
            }
        }

        let slot = self.alloc_slot()?;
        let chunk = Chunk::new_active(coord, slot, false); // has_non_air set after terrain gen
        self.chunks.insert(coord, chunk);
        Some(slot)
    }

    /// Unload a chunk, freeing its pool slot.
    pub fn unload_chunk(&mut self, coord: &ChunkCoord) {
        if let Some(chunk) = self.chunks.remove(coord) {
            if let Some(slot) = chunk.pool_slot {
                self.free_slot(slot);
            }
        }
    }

    /// Get a chunk by coordinate.
    pub fn get(&self, coord: &ChunkCoord) -> Option<&Chunk> {
        self.chunks.get(coord)
    }

    /// Get a mutable chunk by coordinate.
    pub fn get_mut(&mut self, coord: &ChunkCoord) -> Option<&mut Chunk> {
        self.chunks.get_mut(coord)
    }

    /// Check if a chunk coordinate is within world bounds.
    pub fn in_world_bounds(coord: &ChunkCoord) -> bool {
        coord.x >= 0
            && coord.x < WORLD_CHUNKS_X as i32
            && coord.y >= 0
            && coord.y < WORLD_CHUNKS_Y as i32
            && coord.z >= 0
            && coord.z < WORLD_CHUNKS_Z as i32
    }

    /// Get the 6 face-adjacent neighbor coordinates for a chunk.
    pub fn face_neighbors(coord: &ChunkCoord) -> [ChunkCoord; 6] {
        [
            *coord + IVec3::new(-1, 0, 0),
            *coord + IVec3::new(1, 0, 0),
            *coord + IVec3::new(0, -1, 0),
            *coord + IVec3::new(0, 1, 0),
            *coord + IVec3::new(0, 0, -1),
            *coord + IVec3::new(0, 0, 1),
        ]
    }

    /// Get all 26 neighbor coordinates for a chunk.
    pub fn all_neighbors(coord: &ChunkCoord) -> Vec<ChunkCoord> {
        let mut neighbors = Vec::with_capacity(26);
        for dx in -1..=1i32 {
            for dy in -1..=1i32 {
                for dz in -1..=1i32 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    neighbors.push(*coord + IVec3::new(dx, dy, dz));
                }
            }
        }
        neighbors
    }

    /// Iterator over all loaded chunks.
    pub fn iter(&self) -> impl Iterator<Item = (&ChunkCoord, &Chunk)> {
        self.chunks.iter()
    }

    /// Mutable iterator over all loaded chunks.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&ChunkCoord, &mut Chunk)> {
        self.chunks.iter_mut()
    }

    /// Number of loaded chunks.
    pub fn loaded_count(&self) -> u32 {
        self.chunks.len() as u32
    }

    /// Get counts: (total_loaded, active, static_count)
    pub fn chunk_counts(&self) -> (u32, u32, u32) {
        let mut active = 0u32;
        let mut static_count = 0u32;
        for chunk in self.chunks.values() {
            match chunk.state {
                ChunkState::Active => active += 1,
                ChunkState::Static => static_count += 1,
                ChunkState::Unloaded => {}
            }
        }
        (self.chunks.len() as u32, active, static_count)
    }

    /// Get the pool slot capacity.
    pub fn slot_capacity(&self) -> u32 {
        self.slot_capacity
    }

    /// Number of free slots.
    pub fn free_slot_count(&self) -> u32 {
        self.free_slots.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_map_spatial_queries() {
        let mut map = ChunkMap::with_capacity(64);
        let coord = IVec3::new(1, 1, 1);
        let slot = map.load_chunk(coord);
        assert!(slot.is_some());
        assert!(map.get(&coord).is_some());

        // Face neighbors
        let neighbors = ChunkMap::face_neighbors(&coord);
        assert_eq!(neighbors.len(), 6);
        assert!(neighbors.contains(&IVec3::new(0, 1, 1)));
        assert!(neighbors.contains(&IVec3::new(2, 1, 1)));

        // All 26 neighbors
        let all = ChunkMap::all_neighbors(&coord);
        assert_eq!(all.len(), 26);

        // Bounds checking
        assert!(ChunkMap::in_world_bounds(&IVec3::new(0, 0, 0)));
        assert!(ChunkMap::in_world_bounds(&IVec3::new(7, 3, 7)));
        assert!(!ChunkMap::in_world_bounds(&IVec3::new(8, 0, 0)));
        assert!(!ChunkMap::in_world_bounds(&IVec3::new(-1, 0, 0)));

        // Unload
        map.unload_chunk(&coord);
        assert!(map.get(&coord).is_none());
    }

    #[test]
    fn test_load_chunk_idempotent() {
        let mut map = ChunkMap::with_capacity(64);
        let coord = IVec3::new(2, 1, 3);
        let slot1 = map.load_chunk(coord);
        let slot2 = map.load_chunk(coord);
        assert!(slot1.is_some());
        assert!(slot2.is_some());
        assert_eq!(
            slot1.unwrap(),
            slot2.unwrap(),
            "loading same coord twice should return same slot"
        );
        // Should still only have one chunk loaded
        assert_eq!(map.loaded_count(), 1);
    }

    #[test]
    fn test_chunk_counts_reflect_state() {
        let mut map = ChunkMap::with_capacity(64);
        let c0 = IVec3::new(0, 0, 0);
        let c1 = IVec3::new(1, 0, 0);
        let c2 = IVec3::new(2, 0, 0);

        map.load_chunk(c0);
        map.load_chunk(c1);
        map.load_chunk(c2);

        // All 3 are active
        let (total, active, static_count) = map.chunk_counts();
        assert_eq!(total, 3);
        assert_eq!(active, 3);
        assert_eq!(static_count, 0);

        // Sleep c1
        if let Some(chunk) = map.get_mut(&c1) {
            chunk.sleep();
        }
        let (total, active, static_count) = map.chunk_counts();
        assert_eq!(total, 3);
        assert_eq!(active, 2);
        assert_eq!(static_count, 1);

        // Sleep c2
        if let Some(chunk) = map.get_mut(&c2) {
            chunk.sleep();
        }
        let (total, active, static_count) = map.chunk_counts();
        assert_eq!(total, 3);
        assert_eq!(active, 1);
        assert_eq!(static_count, 2);
    }

    #[test]
    fn test_pool_slot_allocation_and_free() {
        let mut map = ChunkMap::with_capacity(4);

        // Allocate all slots
        let s0 = map.alloc_slot();
        let s1 = map.alloc_slot();
        let s2 = map.alloc_slot();
        let s3 = map.alloc_slot();
        assert!(s0.is_some());
        assert!(s1.is_some());
        assert!(s2.is_some());
        assert!(s3.is_some());

        // All unique
        let slots = vec![s0.unwrap(), s1.unwrap(), s2.unwrap(), s3.unwrap()];
        let unique: std::collections::HashSet<u32> = slots.iter().copied().collect();
        assert_eq!(unique.len(), 4);

        // Pool full
        assert!(map.alloc_slot().is_none());

        // Free and reallocate
        map.free_slot(s1.unwrap());
        let s4 = map.alloc_slot();
        assert_eq!(s4, s1);

        // Full again
        assert!(map.alloc_slot().is_none());
    }
}
