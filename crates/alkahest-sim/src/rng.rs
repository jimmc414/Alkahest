//! Deterministic per-voxel PRNG (C-SIM-4).
//!
//! Pure function: `sim_hash(x, y, z, tick) -> u32`.
//! No state, no atomics. Identical algorithm exists in `shaders/common/rng.wgsl`.
//! Uses PCG-style mixing to produce well-distributed u32 values.

/// Hash a voxel position and tick into a deterministic pseudo-random u32.
///
/// This must produce identical results to the WGSL `sim_hash` function.
#[allow(dead_code)] // Used by tests and future shader verification
pub(crate) fn sim_hash(x: i32, y: i32, z: i32, tick: u32) -> u32 {
    // Combine inputs into a single seed using prime multipliers
    let mut state = (x as u32)
        .wrapping_mul(0x9E3779B9)
        .wrapping_add((y as u32).wrapping_mul(0x517CC1B7))
        .wrapping_add((z as u32).wrapping_mul(0x6C62272E))
        .wrapping_add(tick.wrapping_mul(0x2545F491));

    // PCG-style mixing rounds
    state = state ^ (state >> 16);
    state = state.wrapping_mul(0x45D9F3B);
    state = state ^ (state >> 16);
    state = state.wrapping_mul(0x45D9F3B);
    state = state ^ (state >> 16);

    state
}

/// Convert a hash value to a float in [0, 1).
#[allow(dead_code)] // Used by shaders and future milestones
pub(crate) fn hash_to_float(hash: u32) -> f32 {
    (hash >> 8) as f32 / 16_777_216.0 // 2^24
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let a = sim_hash(5, 10, 3, 42);
        let b = sim_hash(5, 10, 3, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn test_different_inputs_differ() {
        let a = sim_hash(0, 0, 0, 0);
        let b = sim_hash(1, 0, 0, 0);
        let c = sim_hash(0, 1, 0, 0);
        let d = sim_hash(0, 0, 1, 0);
        let e = sim_hash(0, 0, 0, 1);
        // All should be different (with overwhelming probability)
        let vals = [a, b, c, d, e];
        for i in 0..vals.len() {
            for j in (i + 1)..vals.len() {
                assert_ne!(vals[i], vals[j], "hash collision at indices {i}, {j}");
            }
        }
    }

    #[test]
    fn test_distribution() {
        // Check that hashes are roughly uniformly distributed
        let mut low_count = 0u32;
        let mut high_count = 0u32;
        for x in 0..100 {
            for y in 0..100 {
                let h = sim_hash(x, y, 0, 0);
                if h < u32::MAX / 2 {
                    low_count += 1;
                } else {
                    high_count += 1;
                }
            }
        }
        // Expect roughly 50/50 split, allow 10% tolerance
        let total = 10_000f32;
        let _ = high_count; // used for implied total check
        let low_frac = low_count as f32 / total;
        assert!(
            low_frac > 0.4 && low_frac < 0.6,
            "poor distribution: {low_frac}"
        );
    }

    #[test]
    fn test_hash_to_float_range() {
        for i in 0..1000 {
            let h = sim_hash(i, 0, 0, 0);
            let f = hash_to_float(h);
            assert!(f >= 0.0 && f < 1.0, "out of range: {f}");
        }
    }

    #[test]
    fn test_prng_same_inputs_same_output() {
        // Verify determinism across multiple calls with identical inputs
        let inputs = [
            (0, 0, 0, 0u32),
            (-1, -1, -1, 0),
            (i32::MAX, i32::MIN, 0, u32::MAX),
            (100, 200, 300, 999),
        ];
        for &(x, y, z, tick) in &inputs {
            let a = sim_hash(x, y, z, tick);
            let b = sim_hash(x, y, z, tick);
            let c = sim_hash(x, y, z, tick);
            assert_eq!(a, b, "determinism failed for ({x},{y},{z},{tick})");
            assert_eq!(b, c, "determinism failed for ({x},{y},{z},{tick})");
        }
    }
}
