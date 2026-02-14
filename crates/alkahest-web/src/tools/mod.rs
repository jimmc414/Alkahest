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
    #[allow(dead_code)]
    Heat,
    Push,
}

impl ActiveTool {
    #[allow(dead_code)]
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

impl Default for ToolState {
    fn default() -> Self {
        Self {
            active: ActiveTool::default(),
            place_material: 2, // sand
            brush: BrushState::new(),
        }
    }
}

impl ToolState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_state_default() {
        let ts = ToolState::new();
        assert_eq!(ts.active, ActiveTool::Place);
        assert_eq!(ts.place_material, 2);
        assert_eq!(ts.brush.radius, 0);
    }

    #[test]
    fn test_active_tool_names() {
        assert_eq!(ActiveTool::Place.name(), "Place");
        assert_eq!(ActiveTool::Remove.name(), "Remove");
        assert_eq!(ActiveTool::Heat.name(), "Heat");
        assert_eq!(ActiveTool::Push.name(), "Push");
    }

    /// Simulate the tick accumulator at 0.25x speed for 1 second at 60fps.
    /// Expected: ~15 ticks (60 frames × 16.67ms × 0.25 × 60 ticks/sec = 15).
    #[test]
    fn test_sim_speed_quarter() {
        let sim_speed: f64 = 0.25;
        let frame_delta_ms: f64 = 16.667; // ~60fps
        let total_frames = 60u32;
        let mut tick_accumulator: f64 = 0.0;
        let mut total_ticks = 0u32;

        for _ in 0..total_frames {
            let delta_sec = frame_delta_ms / 1000.0;
            tick_accumulator += delta_sec * sim_speed * 60.0;
            while tick_accumulator >= 1.0 && total_ticks < 10000 {
                total_ticks += 1;
                tick_accumulator -= 1.0;
            }
        }

        assert!(
            (14..=16).contains(&total_ticks),
            "0.25x speed for 1 second should yield ~15 ticks, got {}",
            total_ticks
        );
    }

    /// Simulate the tick accumulator at 4x speed for 1 second at 60fps.
    /// Expected: ~240 ticks (60 frames × 16.67ms × 4 × 60 ticks/sec = 240).
    /// Capped at 4 ticks/frame → max 240.
    #[test]
    fn test_sim_speed_4x() {
        let sim_speed: f64 = 4.0;
        let frame_delta_ms: f64 = 16.667;
        let total_frames = 60u32;
        let mut tick_accumulator: f64 = 0.0;
        let mut total_ticks = 0u32;

        for _ in 0..total_frames {
            let delta_sec = frame_delta_ms / 1000.0;
            tick_accumulator += delta_sec * sim_speed * 60.0;
            let mut ticks_this_frame = 0u32;
            while tick_accumulator >= 1.0 && ticks_this_frame < 4 {
                total_ticks += 1;
                tick_accumulator -= 1.0;
                ticks_this_frame += 1;
            }
        }

        assert!(
            (235..=245).contains(&total_ticks),
            "4x speed for 1 second should yield ~240 ticks, got {}",
            total_ticks
        );
    }
}
