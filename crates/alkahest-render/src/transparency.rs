//! Transparency rendering configuration.
//! Transparency is handled inline in the ray march shader using front-to-back compositing.
//! The ray continues through transparent voxels (opacity < 0.99) instead of stopping.

/// Opacity threshold below which a material is treated as fully opaque (early exit).
pub const OPACITY_OPAQUE_THRESHOLD: f32 = 0.99;

/// Accumulated opacity threshold at which the ray stops traversing transparent voxels.
pub const OPACITY_CUTOFF: f32 = 0.99;

/// Default absorption color bias for liquid materials (RGB exponential decay).
/// Water absorbs more red than blue, creating depth-dependent blue tinting.
pub const LIQUID_ABSORPTION_BIAS: [f32; 3] = [0.3, 0.1, 0.05];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_absorption_bias_relative_order() {
        // Red should be absorbed most, blue least (water looks blue at depth)
        assert!(LIQUID_ABSORPTION_BIAS[0] > LIQUID_ABSORPTION_BIAS[1]);
        assert!(LIQUID_ABSORPTION_BIAS[1] > LIQUID_ABSORPTION_BIAS[2]);
    }
}
