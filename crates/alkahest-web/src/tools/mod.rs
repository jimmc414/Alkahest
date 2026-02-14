pub mod brush;
pub mod heat;
pub mod place;
pub mod push;
pub mod remove;

use brush::BrushState;

/// Active tool type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTool {
    #[default]
    Place,
    Remove,
    Heat,
    Push,
}

impl ActiveTool {
    pub fn name(self) -> &'static str {
        match self {
            ActiveTool::Place => "Place",
            ActiveTool::Remove => "Remove",
            ActiveTool::Heat => "Heat",
            ActiveTool::Push => "Push",
        }
    }
}

/// Tool state tracking.
pub struct ToolState {
    pub active: ActiveTool,
    /// Material ID to place (default: sand = 2).
    pub place_material: u32,
    /// Brush shape and radius.
    pub brush: BrushState,
}

impl ToolState {
    pub fn new() -> Self {
        Self {
            active: ActiveTool::default(),
            place_material: 2, // sand
            brush: BrushState::new(),
        }
    }
}
