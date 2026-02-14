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

impl BrushState {
    pub fn new() -> Self {
        Self {
            shape: BrushShape::Single,
            radius: 0,
        }
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
}
