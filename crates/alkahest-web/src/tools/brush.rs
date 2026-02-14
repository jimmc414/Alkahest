/// Brush shape for area tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushShape {
    #[default]
    Single,
    Cube,
    Sphere,
}

impl BrushShape {
    /// GPU-compatible integer value matching commands.wgsl constants.
    pub fn as_u32(self) -> u32 {
        match self {
            BrushShape::Single => 0,
            BrushShape::Cube => 1,
            BrushShape::Sphere => 2,
        }
    }

    /// Cycle to the next brush shape.
    pub fn next(self) -> Self {
        match self {
            BrushShape::Single => BrushShape::Cube,
            BrushShape::Cube => BrushShape::Sphere,
            BrushShape::Sphere => BrushShape::Single,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BrushShape::Single => "Single",
            BrushShape::Cube => "Cube",
            BrushShape::Sphere => "Sphere",
        }
    }
}

/// Brush state: shape + radius.
#[derive(Debug, Clone)]
pub struct BrushState {
    pub shape: BrushShape,
    /// 0 = single voxel, 1–16 for area brushes.
    pub radius: u32,
}

impl Default for BrushState {
    fn default() -> Self {
        Self {
            shape: BrushShape::Single,
            radius: 0,
        }
    }
}

impl BrushState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increase radius by 1, clamped to 16.
    pub fn increase_radius(&mut self) {
        self.radius = (self.radius + 1).min(16);
        if self.radius > 0 && self.shape == BrushShape::Single {
            self.shape = BrushShape::Cube;
        }
    }

    /// Decrease radius by 1, clamped to 0. Reset shape to Single at radius 0.
    pub fn decrease_radius(&mut self) {
        self.radius = self.radius.saturating_sub(1);
        if self.radius == 0 {
            self.shape = BrushShape::Single;
        }
    }

    /// Compute the expected voxel count for a sphere brush of given radius.
    /// Used for test verification: discrete sphere volume = sum of voxels where dx²+dy²+dz² ≤ r².
    pub fn sphere_voxel_count(radius: u32) -> u32 {
        let r = radius as i32;
        let r_sq = r * r;
        let mut count = 0u32;
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if dx * dx + dy * dy + dz * dz <= r_sq {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Compute the expected voxel count for a cube brush of given radius.
    /// Side length = 2*radius + 1.
    pub fn cube_voxel_count(radius: u32) -> u32 {
        let side = 2 * radius + 1;
        side * side * side
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brush_shape_cycle() {
        let s = BrushShape::Single;
        assert_eq!(s.next(), BrushShape::Cube);
        assert_eq!(s.next().next(), BrushShape::Sphere);
        assert_eq!(s.next().next().next(), BrushShape::Single);
    }

    #[test]
    fn test_brush_shape_gpu_values() {
        assert_eq!(BrushShape::Single.as_u32(), 0);
        assert_eq!(BrushShape::Cube.as_u32(), 1);
        assert_eq!(BrushShape::Sphere.as_u32(), 2);
    }

    #[test]
    fn test_brush_radius_increase_clamp() {
        let mut b = BrushState::new();
        assert_eq!(b.radius, 0);
        for _ in 0..20 {
            b.increase_radius();
        }
        assert_eq!(b.radius, 16);
    }

    #[test]
    fn test_brush_radius_decrease_clamp() {
        let mut b = BrushState::new();
        b.decrease_radius();
        assert_eq!(b.radius, 0);
        assert_eq!(b.shape, BrushShape::Single);
    }

    #[test]
    fn test_brush_auto_shape_on_radius_increase() {
        let mut b = BrushState::new();
        assert_eq!(b.shape, BrushShape::Single);
        b.increase_radius();
        assert_eq!(b.shape, BrushShape::Cube);
        assert_eq!(b.radius, 1);
    }

    #[test]
    fn test_brush_reset_shape_on_radius_zero() {
        let mut b = BrushState::new();
        b.increase_radius();
        b.increase_radius();
        b.shape = BrushShape::Sphere;
        b.decrease_radius();
        b.decrease_radius();
        assert_eq!(b.radius, 0);
        assert_eq!(b.shape, BrushShape::Single);
    }

    #[test]
    fn test_sphere_voxel_count_r8() {
        let count = BrushState::sphere_voxel_count(8);
        // Discrete sphere r=8: should be close to (4/3)π(8)³ ≈ 2145
        let expected_continuous = (4.0 / 3.0) * std::f64::consts::PI * 8.0_f64.powi(3);
        let diff = (count as f64 - expected_continuous).abs() / expected_continuous;
        assert!(
            diff < 0.05,
            "Sphere r=8 count {} deviates >5% from expected {:.0}",
            count,
            expected_continuous
        );
    }

    #[test]
    fn test_cube_voxel_count_r4() {
        let count = BrushState::cube_voxel_count(4);
        // Cube r=4: side = 9, volume = 729
        assert_eq!(count, 729);
    }
}
