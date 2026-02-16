//! Sky rendering configuration.
//! The procedural sky is rendered inline in the ray march shader when a ray misses all voxels.
//! Sky colors are configurable via the LightConfig uniform (zenith + horizon colors).
//! The gradient uses a squared falloff: t = (ray_dir.y * 0.5 + 0.5)^2 for natural-looking sky.

/// Default sky zenith color (deep blue, looking straight up).
pub const DEFAULT_SKY_ZENITH: [f32; 3] = [0.1, 0.15, 0.4];

/// Default sky horizon color (warm haze at the horizon).
pub const DEFAULT_SKY_HORIZON: [f32; 3] = [0.5, 0.45, 0.35];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sky_colors_valid() {
        for c in DEFAULT_SKY_ZENITH {
            assert!(c >= 0.0 && c <= 1.0);
        }
        for c in DEFAULT_SKY_HORIZON {
            assert!(c >= 0.0 && c <= 1.0);
        }
    }
}
