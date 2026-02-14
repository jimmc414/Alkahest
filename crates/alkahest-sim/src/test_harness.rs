/// Deterministic test infrastructure for simulation snapshot testing.
///
/// GPU tests are gated behind `#[cfg(feature = "gpu_tests")]` since they
/// require a GPU device. CPU-only unit tests for direction, rng, and
/// conflict scheduling run without this feature.

#[cfg(test)]
mod tests {
    use crate::rng::sim_hash;

    #[test]
    fn test_prng_determinism_across_ticks() {
        // Same position at different ticks must produce different values
        let h0 = sim_hash(10, 5, 3, 0);
        let h1 = sim_hash(10, 5, 3, 1);
        let h2 = sim_hash(10, 5, 3, 2);
        assert_ne!(h0, h1);
        assert_ne!(h1, h2);
        assert_ne!(h0, h2);
    }

    #[test]
    fn test_prng_symmetry_broken() {
        // Mirrored positions should not produce same hash
        let a = sim_hash(1, 2, 3, 0);
        let b = sim_hash(3, 2, 1, 0);
        assert_ne!(a, b);
    }
}
