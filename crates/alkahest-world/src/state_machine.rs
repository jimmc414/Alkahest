use crate::chunk::ChunkState;
use crate::chunk_map::ChunkMap;

/// Process activity flags read back from the GPU activity scan pass.
/// Each flag corresponds to a dispatched chunk:
///   0 = no voxel changes this tick (idle)
///   non-zero = at least one voxel changed (active)
///
/// Chunks that have been idle for CHUNK_SLEEP_TICKS consecutive ticks
/// transition to Static. When a chunk transitions to Active (from GPU
/// activity or neighbor activation), its face-adjacent neighbors are
/// also activated to ensure cross-boundary effects propagate.
pub fn process_activity_flags(chunk_map: &mut ChunkMap, flags: &[u32]) {
    // Collect coords and their activity status.
    // We need to collect first because we'll mutate the map during neighbor activation.
    let mut to_activate: Vec<glam::IVec3> = Vec::new();
    let mut to_sleep: Vec<glam::IVec3> = Vec::new();

    // Map dispatch index to chunk: iterate active chunks in the same order
    // as build_dispatch_list produces them. Since HashMap iteration order
    // is not guaranteed, we match by dispatch_index stored on the chunk.
    //
    // For now, iterate all loaded chunks and match flags by dispatch order.
    // The caller should ensure flags.len() matches the dispatch list length.
    let dispatched: Vec<(glam::IVec3, u32)> = chunk_map
        .iter()
        .filter(|(_, c)| c.state == ChunkState::Active && c.pool_slot.is_some())
        .map(|(coord, _)| *coord)
        .collect::<Vec<_>>()
        .into_iter()
        .enumerate()
        .filter_map(|(i, coord)| flags.get(i).map(|&flag| (coord, flag)))
        .collect();

    for (coord, flag) in &dispatched {
        if *flag != 0 {
            // Chunk had activity — reset idle counter
            if let Some(chunk) = chunk_map.get_mut(coord) {
                chunk.activate();
            }
            // Also activate face neighbors (cross-boundary propagation)
            let neighbors = ChunkMap::face_neighbors(coord);
            for n in &neighbors {
                if ChunkMap::in_world_bounds(n) {
                    to_activate.push(*n);
                }
            }
        } else {
            // Chunk was idle this tick — increment idle counter
            if let Some(chunk) = chunk_map.get_mut(coord) {
                if chunk.tick_idle() {
                    to_sleep.push(*coord);
                }
            }
        }
    }

    // Apply neighbor activations
    for coord in &to_activate {
        if let Some(chunk) = chunk_map.get_mut(coord) {
            if chunk.state == ChunkState::Static {
                chunk.activate();
            }
        }
    }

    // Apply sleep transitions
    for coord in &to_sleep {
        if let Some(chunk) = chunk_map.get_mut(coord) {
            chunk.sleep();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkahest_core::constants::CHUNK_SLEEP_TICKS;
    use glam::IVec3;

    #[test]
    fn test_sleep_after_idle_ticks() {
        let mut map = ChunkMap::with_capacity(64);
        let coord = IVec3::new(1, 1, 1);
        map.load_chunk(coord);

        // Simulate CHUNK_SLEEP_TICKS of idle flags
        for _ in 0..CHUNK_SLEEP_TICKS {
            process_activity_flags(&mut map, &[0]);
        }

        let chunk = map.get(&coord).expect("chunk exists");
        assert_eq!(chunk.state, ChunkState::Static);
    }

    #[test]
    fn test_activity_resets_idle() {
        let mut map = ChunkMap::with_capacity(64);
        let coord = IVec3::new(1, 1, 1);
        map.load_chunk(coord);

        // Almost sleep
        for _ in 0..(CHUNK_SLEEP_TICKS - 1) {
            process_activity_flags(&mut map, &[0]);
        }

        // Activity resets counter
        process_activity_flags(&mut map, &[1]);

        let chunk = map.get(&coord).expect("chunk exists");
        assert_eq!(chunk.state, ChunkState::Active);
        assert_eq!(chunk.idle_ticks, 0);
    }

    #[test]
    fn test_neighbor_activation_on_activity() {
        let mut map = ChunkMap::with_capacity(64);
        let center = IVec3::new(2, 2, 2);
        let neighbor = IVec3::new(3, 2, 2);
        map.load_chunk(center);
        map.load_chunk(neighbor);

        // Put neighbor to sleep
        if let Some(chunk) = map.get_mut(&neighbor) {
            chunk.sleep();
        }
        assert_eq!(
            map.get(&neighbor).expect("exists").state,
            ChunkState::Static
        );

        // Activity on center should wake neighbor
        // flags[0] corresponds to center (first active chunk found by iterator)
        // We need to be careful: HashMap iteration order is non-deterministic.
        // For this test, we process flags for all active chunks.
        // Only center is Active at this point, so flags = [1] maps to center.
        process_activity_flags(&mut map, &[1]);

        let n_chunk = map.get(&neighbor).expect("exists");
        assert_eq!(n_chunk.state, ChunkState::Active);
    }
}
