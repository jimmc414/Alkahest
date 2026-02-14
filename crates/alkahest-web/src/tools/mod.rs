pub mod place;
pub mod remove;

/// Active tool type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // Remove variant used in future milestones
pub enum ActiveTool {
    #[default]
    Place,
    Remove,
}

/// Tool state tracking.
pub struct ToolState {
    #[allow(dead_code)] // Used when tool switching UI is added
    pub active: ActiveTool,
    /// Material ID to place (default: sand = 2).
    pub place_material: u32,
}

impl ToolState {
    pub fn new() -> Self {
        Self {
            active: ActiveTool::default(),
            place_material: 2, // sand
        }
    }
}
