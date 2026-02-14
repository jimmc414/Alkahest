use alkahest_core::constants::*;
use alkahest_core::types::ChunkCoord;
use glam::IVec3;

/// Node in the flat-array sparse voxel octree.
/// Packed as 2 × u32 for GPU upload.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct OctreeNode {
    /// Bits [0:7]: child_mask (which of 8 children are non-empty).
    /// Bits [8:31]: reserved flags.
    pub mask_and_flags: u32,
    /// Index of the first child in the node array.
    /// Children are stored contiguously; use popcount(child_mask & ((1 << i) - 1))
    /// to find the offset of child i.
    pub first_child_offset: u32,
}

impl OctreeNode {
    pub fn child_mask(&self) -> u8 {
        (self.mask_and_flags & 0xFF) as u8
    }

    pub fn set_child_mask(&mut self, mask: u8) {
        self.mask_and_flags = (self.mask_and_flags & 0xFFFFFF00) | mask as u32;
    }

    pub fn is_empty(&self) -> bool {
        self.child_mask() == 0
    }
}

/// Manages the sparse voxel octree used for empty-space skipping in the ray march.
pub struct Octree {
    /// Flat array of nodes. Index 0 = root.
    nodes: Vec<OctreeNode>,
    /// Whether the octree needs to be re-uploaded to the GPU.
    dirty: bool,
}

impl Default for Octree {
    fn default() -> Self {
        Self::new()
    }
}

impl Octree {
    /// Create a new empty octree.
    pub fn new() -> Self {
        Self {
            nodes: vec![OctreeNode::default()], // root node, empty
            dirty: true,
        }
    }

    /// Rebuild the entire octree from chunk occupancy data.
    /// `chunk_has_non_air` maps chunk coordinates to whether the chunk
    /// contains any non-air voxels.
    ///
    /// The octree uses a three-level structure:
    /// - Level 0 (root): children are octants of the world grid
    /// - Level 1 (mid): sub-octants within each root octant
    /// - Level 2 (leaf): individual chunks
    pub fn rebuild(&mut self, chunk_has_non_air: &[(ChunkCoord, bool)]) {
        self.nodes.clear();

        // Root node at index 0
        self.nodes.push(OctreeNode::default());

        // The world is 8×4×8 chunks. We subdivide into an octree:
        // Root splits at x=4, y=2, z=4 → 2×2×2 = 8 octants
        // Each octant covers 4×2×4 chunks.
        // Sub-octant splits each dimension in half again → 2×1×2 chunks per leaf group.

        // Build occupancy map
        let mut occupied = std::collections::HashSet::new();
        for (coord, has_content) in chunk_has_non_air {
            if *has_content {
                occupied.insert(*coord);
            }
        }

        // Determine root child mask
        let mut root_mask: u8 = 0;
        for octant in 0u8..8 {
            let ox = (octant & 1) as i32;
            let oy = ((octant >> 1) & 1) as i32;
            let oz = ((octant >> 2) & 1) as i32;

            let has_content = Self::octant_has_content(ox, oy, oz, &occupied);
            if has_content {
                root_mask |= 1 << octant;
            }
        }

        self.nodes[0].set_child_mask(root_mask);

        if root_mask == 0 {
            self.dirty = true;
            return;
        }

        // Allocate children for root
        let root_first_child = self.nodes.len() as u32;
        self.nodes[0].first_child_offset = root_first_child;
        let root_child_count = root_mask.count_ones() as usize;
        self.nodes
            .resize(self.nodes.len() + root_child_count, OctreeNode::default());

        // For each non-empty root octant, build its chunk-level children
        let mut child_idx = 0usize;
        for octant in 0u8..8 {
            if root_mask & (1 << octant) == 0 {
                continue;
            }

            let ox = (octant & 1) as i32;
            let oy = ((octant >> 1) & 1) as i32;
            let oz = ((octant >> 2) & 1) as i32;

            // This octant covers a 4×2×4 region of chunks.
            // Subdivide into 2×2×2 = 8 sub-groups (2×1×2 chunks each).
            let mut octant_mask: u8 = 0;
            for sub in 0u8..8 {
                let sx = (sub & 1) as i32;
                let sy = ((sub >> 1) & 1) as i32;
                let sz = ((sub >> 2) & 1) as i32;

                let has_content = Self::sub_octant_has_content(ox, oy, oz, sx, sy, sz, &occupied);
                if has_content {
                    octant_mask |= 1 << sub;
                }
            }

            let node_idx = root_first_child as usize + child_idx;
            self.nodes[node_idx].set_child_mask(octant_mask);

            if octant_mask != 0 {
                let first_child = self.nodes.len() as u32;
                self.nodes[node_idx].first_child_offset = first_child;
                let sub_count = octant_mask.count_ones() as usize;
                self.nodes
                    .resize(self.nodes.len() + sub_count, OctreeNode::default());

                // For each sub-octant, create leaf nodes representing individual chunks
                let mut sub_idx = 0usize;
                for sub in 0u8..8 {
                    if octant_mask & (1 << sub) == 0 {
                        continue;
                    }

                    let sx = (sub & 1) as i32;
                    let sy = ((sub >> 1) & 1) as i32;
                    let sz = ((sub >> 2) & 1) as i32;

                    // Individual chunks in this 2×1×2 sub-region
                    let base_cx = ox * 4 + sx * 2;
                    let base_cy = oy * 2 + sy;
                    let base_cz = oz * 4 + sz * 2;

                    let mut leaf_mask: u8 = 0;
                    // Each sub-octant covers 2×1×2 chunks, but we use 8 children (2×2×2)
                    for li in 0u8..8 {
                        let lx = (li & 1) as i32;
                        let ly = ((li >> 1) & 1) as i32;
                        let lz = ((li >> 2) & 1) as i32;

                        let cx = base_cx + lx;
                        let cy = base_cy + ly;
                        let cz = base_cz + lz;

                        if cx < WORLD_CHUNKS_X as i32
                            && cy < WORLD_CHUNKS_Y as i32
                            && cz < WORLD_CHUNKS_Z as i32
                            && occupied.contains(&IVec3::new(cx, cy, cz))
                        {
                            leaf_mask |= 1 << li;
                        }
                    }

                    let leaf_idx = first_child as usize + sub_idx;
                    self.nodes[leaf_idx].set_child_mask(leaf_mask);
                    // Leaf nodes don't have children
                    self.nodes[leaf_idx].first_child_offset = 0;
                    sub_idx += 1;
                }
            }

            child_idx += 1;
        }

        self.dirty = true;
    }

    fn octant_has_content(
        ox: i32,
        oy: i32,
        oz: i32,
        occupied: &std::collections::HashSet<ChunkCoord>,
    ) -> bool {
        for cx in (ox * 4)..((ox + 1) * 4) {
            for cy in (oy * 2)..((oy + 1) * 2) {
                for cz in (oz * 4)..((oz + 1) * 4) {
                    if occupied.contains(&IVec3::new(cx, cy, cz)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn sub_octant_has_content(
        ox: i32,
        oy: i32,
        oz: i32,
        sx: i32,
        sy: i32,
        sz: i32,
        occupied: &std::collections::HashSet<ChunkCoord>,
    ) -> bool {
        let base_cx = ox * 4 + sx * 2;
        let base_cy = oy * 2 + sy;
        let base_cz = oz * 4 + sz * 2;

        for lx in 0..2i32 {
            for ly in 0..2i32 {
                for lz in 0..2i32 {
                    let cx = base_cx + lx;
                    let cy = base_cy + ly;
                    let cz = base_cz + lz;
                    if cx < WORLD_CHUNKS_X as i32
                        && cy < WORLD_CHUNKS_Y as i32
                        && cz < WORLD_CHUNKS_Z as i32
                        && occupied.contains(&IVec3::new(cx, cy, cz))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Mark the octree as needing re-upload.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if the octree data needs to be re-uploaded to GPU.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag after uploading.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Get the raw node data for GPU upload.
    /// Returns pairs of u32 values per node.
    pub fn gpu_data(&self) -> Vec<u32> {
        let mut data = Vec::with_capacity(self.nodes.len() * 2);
        for node in &self.nodes {
            data.push(node.mask_and_flags);
            data.push(node.first_child_offset);
        }
        // Ensure at least one node (empty root) for valid buffer binding
        if data.is_empty() {
            data.push(0);
            data.push(0);
        }
        data
    }

    /// Number of nodes in the octree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_octree() {
        let octree = Octree::new();
        assert_eq!(octree.node_count(), 1);
        assert!(octree.nodes[0].is_empty());
    }

    #[test]
    fn test_rebuild_with_occupied_chunks() {
        let mut octree = Octree::new();
        let chunks = vec![
            (IVec3::new(0, 0, 0), true),
            (IVec3::new(1, 0, 0), true),
            (IVec3::new(4, 2, 4), true),
        ];
        octree.rebuild(&chunks);
        assert!(octree.node_count() > 1);
        assert!(!octree.nodes[0].is_empty());
    }

    #[test]
    fn test_rebuild_all_empty() {
        let mut octree = Octree::new();
        let chunks: Vec<(ChunkCoord, bool)> = vec![(IVec3::new(0, 0, 0), false)];
        octree.rebuild(&chunks);
        assert!(octree.nodes[0].is_empty());
    }

    #[test]
    fn test_gpu_data_format() {
        let mut octree = Octree::new();
        let chunks = vec![(IVec3::new(0, 0, 0), true)];
        octree.rebuild(&chunks);
        let data = octree.gpu_data();
        assert_eq!(data.len() % 2, 0); // pairs of u32
        assert!(data.len() >= 2); // at least root node
    }
}
