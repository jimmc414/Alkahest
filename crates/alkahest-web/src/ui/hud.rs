use crate::tools::ToolState;

/// Non-intrusive HUD overlay showing key game state.
pub fn show(
    ctx: &egui::Context,
    tool_state: &ToolState,
    material_names: &[&str],
    sim_speed: f32,
    sim_paused: bool,
) {
    egui::Area::new(egui::Id::new("hud"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let mat_name = material_names
                    .get(tool_state.place_material as usize)
                    .unwrap_or(&"?");

                let tool_name = match tool_state.active {
                    crate::tools::ActiveTool::Place => "Place",
                    crate::tools::ActiveTool::Remove => "Remove",
                    crate::tools::ActiveTool::Heat => "Heat",
                    crate::tools::ActiveTool::Push => "Push",
                };

                ui.label(format!("Tool: {}", tool_name));
                ui.label(format!("Material: {}", mat_name));

                if tool_state.brush.radius > 0 {
                    ui.label(format!(
                        "Brush: {} r={}",
                        tool_state.brush.shape.name(),
                        tool_state.brush.radius,
                    ));
                }

                let speed_str = if sim_paused {
                    "PAUSED".to_string()
                } else {
                    format!("{:.2}x", sim_speed)
                };
                ui.label(format!("Speed: {}", speed_str));
            });
        });
}
