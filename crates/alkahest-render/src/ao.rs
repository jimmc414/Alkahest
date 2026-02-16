//! Ambient occlusion configuration.
//! AO is computed inline in the ray march shader using 6 face-adjacent neighbor samples.
//! No separate buffer or compute pass needed — just voxel lookups per primary hit.

/// Number of face-adjacent neighbors sampled for AO (6 directions: ±X, ±Y, ±Z).
pub const AO_SAMPLE_COUNT: u32 = 6;

/// AO darkening factor per occluded neighbor. 6 neighbors fully occluded → 0.3 ambient.
/// Formula: ao = 1.0 - occupied_count * AO_FACTOR_PER_NEIGHBOR
pub const AO_FACTOR_PER_NEIGHBOR: f32 = 0.1167;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ao_range() {
        // Fully open: 0 neighbors → ao = 1.0
        let ao_open = 1.0 - 0.0 * AO_FACTOR_PER_NEIGHBOR;
        assert!((ao_open - 1.0).abs() < 0.001);

        // Fully occluded: 6 neighbors → ao ≈ 0.3
        let ao_closed = 1.0 - (AO_SAMPLE_COUNT as f32) * AO_FACTOR_PER_NEIGHBOR;
        assert!(ao_closed > 0.2 && ao_closed < 0.4);
    }
}
