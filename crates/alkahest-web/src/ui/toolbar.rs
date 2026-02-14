use crate::tools::brush::BrushShape;
use crate::tools::{ActiveTool, ToolState};

/// Toolbar side panel: tool selector, brush shape, radius slider.
pub fn show(ctx: &egui::Context, tool_state: &mut ToolState) {
    egui::Window::new("Tools")
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
        .resizable(false)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label("Tool");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut tool_state.active, ActiveTool::Place, "Place");
                ui.selectable_value(&mut tool_state.active, ActiveTool::Remove, "Remove");
                ui.selectable_value(&mut tool_state.active, ActiveTool::Push, "Push");
            });

            ui.separator();
            ui.label("Brush Shape");
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(tool_state.brush.shape == BrushShape::Single, "Single")
                    .clicked()
                {
                    tool_state.brush.shape = BrushShape::Single;
                    tool_state.brush.radius = 0;
                }
                if ui
                    .selectable_label(tool_state.brush.shape == BrushShape::Cube, "Cube")
                    .clicked()
                {
                    tool_state.brush.shape = BrushShape::Cube;
                    if tool_state.brush.radius == 0 {
                        tool_state.brush.radius = 1;
                    }
                }
                if ui
                    .selectable_label(tool_state.brush.shape == BrushShape::Sphere, "Sphere")
                    .clicked()
                {
                    tool_state.brush.shape = BrushShape::Sphere;
                    if tool_state.brush.radius == 0 {
                        tool_state.brush.radius = 1;
                    }
                }
            });

            if tool_state.brush.shape != BrushShape::Single {
                ui.separator();
                let mut r = tool_state.brush.radius as f32;
                ui.add(egui::Slider::new(&mut r, 1.0..=16.0).text("Radius"));
                tool_state.brush.radius = r as u32;
            }
        });
}
