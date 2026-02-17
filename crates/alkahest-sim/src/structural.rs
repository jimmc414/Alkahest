//! Structural collapse detection via bounded CPU-side BFS flood-fill (M6).
//!
//! When a structural voxel is destroyed, nearby solid voxels may become
//! disconnected from the ground (y=0). This module detects such disconnected
//! components and returns their indices so the caller can mark them for
//! gravity collapse (velocity_y = -1).
//!
//! The BFS is bounded to STRUCTURAL_BFS_RADIUS to keep frame-time impact
//! predictable. Runs on the main thread (Worker integration deferred per M5
//! precedent).

use alkahest_core::constants::{CHUNK_SIZE, STRUCTURAL_BFS_RADIUS};
use std::collections::{HashSet, VecDeque};

/// Face-adjacent neighbor offsets (6-connected).
const FACE_OFFSETS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

/// Convert (x, y, z) to linear index within a chunk.
fn voxel_index(x: u32, y: u32, z: u32) -> usize {
    (x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE) as usize
}

/// Extract material_id from packed voxel data (low 16 bits of first u32).
fn extract_material_id(voxel: [u32; 2]) -> u16 {
    (voxel[0] & 0xFFFF) as u16
}

/// Detect disconnected structural components within a single chunk.
///
/// `chunk_data` — The chunk's voxel data as `[u32; 2]` pairs (read back from GPU).
/// `structural_ids` — Material IDs that count as structural (solids with integrity > 0).
///
/// Returns indices of voxels in disconnected components (should have velocity_y set to -1).
///
/// Algorithm:
/// 1. Find all structural voxels in the chunk.
/// 2. For each unvisited structural voxel, BFS flood-fill (bounded to STRUCTURAL_BFS_RADIUS).
/// 3. If the component touches y=0, it's grounded — skip.
/// 4. If the component does NOT touch y=0 within the BFS radius, mark all its voxels.
pub fn detect_collapse(chunk_data: &[[u32; 2]], structural_ids: &[u16]) -> Vec<usize> {
    let structural_set: HashSet<u16> = structural_ids.iter().copied().collect();
    let cs = CHUNK_SIZE;
    let total = (cs * cs * cs) as usize;

    if chunk_data.len() < total {
        return Vec::new();
    }

    // Build set of structural voxel indices
    let mut is_structural = vec![false; total];
    for z in 0..cs {
        for y in 0..cs {
            for x in 0..cs {
                let idx = voxel_index(x, y, z);
                let mat_id = extract_material_id(chunk_data[idx]);
                if structural_set.contains(&mat_id) {
                    is_structural[idx] = true;
                }
            }
        }
    }

    let mut visited = vec![false; total];
    let mut disconnected = Vec::new();

    for start_idx in 0..total {
        if !is_structural[start_idx] || visited[start_idx] {
            continue;
        }

        // BFS from this structural voxel
        let mut queue = VecDeque::new();
        let mut component = Vec::new();
        let mut grounded = false;

        queue.push_back(start_idx);
        visited[start_idx] = true;

        while let Some(current) = queue.pop_front() {
            component.push(current);

            // Decode position
            let ci = current as u32;
            let x = ci % cs;
            let y = (ci / cs) % cs;
            let z = ci / (cs * cs);

            // Check if grounded (y=0)
            if y == 0 {
                grounded = true;
            }

            // Check BFS radius bound from start
            let si = start_idx as u32;
            let sx = si % cs;
            let sy = (si / cs) % cs;
            let sz = si / (cs * cs);
            let dist = x.abs_diff(sx) + y.abs_diff(sy) + z.abs_diff(sz);
            if dist >= STRUCTURAL_BFS_RADIUS {
                continue;
            }

            // Expand to face neighbors
            for &(dx, dy, dz) in &FACE_OFFSETS {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                let nz = z as i32 + dz;

                if nx < 0
                    || nx >= cs as i32
                    || ny < 0
                    || ny >= cs as i32
                    || nz < 0
                    || nz >= cs as i32
                {
                    // Out of chunk bounds — treat as potentially grounded
                    // (cross-chunk connectivity not resolved here)
                    continue;
                }

                let neighbor_idx = voxel_index(nx as u32, ny as u32, nz as u32);
                if !visited[neighbor_idx] && is_structural[neighbor_idx] {
                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor_idx);
                }
            }
        }

        if !grounded {
            disconnected.extend_from_slice(&component);
        }
    }

    disconnected
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create empty chunk data (all air).
    fn empty_chunk() -> Vec<[u32; 2]> {
        vec![[0u32; 2]; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]
    }

    /// Set a voxel's material ID in chunk data.
    fn set_material(chunk: &mut [[u32; 2]], x: u32, y: u32, z: u32, mat_id: u16) {
        let idx = voxel_index(x, y, z);
        chunk[idx][0] = (chunk[idx][0] & !0xFFFF) | mat_id as u32;
    }

    #[test]
    fn test_structural_flood_fill_connected() {
        // Stone column from y=0 to y=5 — all connected to ground
        let mut chunk = empty_chunk();
        for y in 0..6 {
            set_material(&mut chunk, 5, y, 5, 1); // Stone = 1
        }

        let result = detect_collapse(&chunk, &[1]);
        assert!(result.is_empty(), "Connected column should not be flagged");
    }

    #[test]
    fn test_structural_flood_fill_disconnect() {
        // Floating stone block at y=10, not touching ground
        let mut chunk = empty_chunk();
        for x in 4..7 {
            for z in 4..7 {
                set_material(&mut chunk, x, 10, z, 1); // Stone
            }
        }

        let result = detect_collapse(&chunk, &[1]);
        assert_eq!(
            result.len(),
            9,
            "3x1x3 floating block = 9 disconnected voxels"
        );
    }

    #[test]
    fn test_structural_flood_fill_bounded() {
        // Very tall column NOT touching ground (starts at y=1)
        // BFS should still process within bounds
        let mut chunk = empty_chunk();
        for y in 1..CHUNK_SIZE {
            set_material(&mut chunk, 5, y, 5, 1); // Stone
        }

        let result = detect_collapse(&chunk, &[1]);
        // All 31 voxels should be disconnected (none touch y=0)
        assert_eq!(result.len(), (CHUNK_SIZE - 1) as usize);
    }

    #[test]
    fn test_structural_mixed_materials() {
        // Only Stone (1) is structural, Sand (2) is not
        let mut chunk = empty_chunk();
        // Stone on ground
        set_material(&mut chunk, 5, 0, 5, 1);
        // Sand floating (not structural, should be ignored)
        set_material(&mut chunk, 10, 10, 10, 2);

        let result = detect_collapse(&chunk, &[1]);
        assert!(
            result.is_empty(),
            "Grounded stone + non-structural sand = no collapse"
        );
    }

    #[test]
    fn test_structural_empty_chunk() {
        let chunk = empty_chunk();
        let result = detect_collapse(&chunk, &[1]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_l_shaped_connected() {
        // L-shape touching y=0: vertical arm y=0..3 at x=5, horizontal arm x=5..8 at y=0
        let mut chunk = empty_chunk();
        // Vertical arm
        for y in 0..4 {
            set_material(&mut chunk, 5, y, 5, 1);
        }
        // Horizontal arm on ground
        for x in 6..9 {
            set_material(&mut chunk, x, 0, 5, 1);
        }

        let result = detect_collapse(&chunk, &[1]);
        assert!(
            result.is_empty(),
            "L-shape touching y=0 should be fully connected, got {} disconnected",
            result.len()
        );
    }

    #[test]
    fn test_multiple_disconnected_components() {
        // Two separate floating blocks, neither touching ground
        let mut chunk = empty_chunk();
        // Block A: at y=10
        for x in 2..4 {
            for z in 2..4 {
                set_material(&mut chunk, x, 10, z, 1);
            }
        }
        // Block B: at y=20
        for x in 20..22 {
            for z in 20..22 {
                set_material(&mut chunk, x, 20, z, 1);
            }
        }

        let result = detect_collapse(&chunk, &[1]);
        assert_eq!(
            result.len(),
            8,
            "two 2x1x2 floating blocks = 8 disconnected voxels, got {}",
            result.len()
        );
    }

    #[test]
    fn test_multiple_structural_ids() {
        // Both Stone (1) and Brick (3) are structural; they should connect
        let mut chunk = empty_chunk();
        // Stone on ground
        set_material(&mut chunk, 5, 0, 5, 1);
        // Brick on top of stone
        set_material(&mut chunk, 5, 1, 5, 3);
        // Brick floating separately
        set_material(&mut chunk, 20, 15, 20, 3);

        let result = detect_collapse(&chunk, &[1, 3]);
        // The stone+brick column is grounded, so only the floating brick is disconnected
        assert_eq!(
            result.len(),
            1,
            "only the floating brick should be disconnected, got {}",
            result.len()
        );
    }
}
