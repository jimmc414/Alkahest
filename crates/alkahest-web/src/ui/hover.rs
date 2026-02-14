use alkahest_render::PickResult;

/// Voxel hover info panel. Shows data from the GPU pick buffer when valid.
pub fn show(ctx: &egui::Context, pick: &PickResult, material_names: &[&str]) {
    if !pick.valid {
        return;
    }

    egui::Window::new("Voxel Info")
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -8.0))
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .show(ctx, |ui| {
            let mat_name = material_names
                .get(pick.material_id as usize)
                .unwrap_or(&"Unknown");

            ui.label(format!("{} (ID: {})", mat_name, pick.material_id));
            ui.label(format!(
                "Pos: ({}, {}, {})",
                pick.world_x, pick.world_y, pick.world_z
            ));

            // Temperature: stored as quantized 0..4095, mapping to 0..8000 K
            let temp_k = (pick.temperature as f32) * (8000.0 / 4095.0);
            ui.label(format!("Temp: {:.0} K", temp_k));

            ui.label(format!("Pressure: {}", pick.pressure));

            if pick.vel_x != 0 || pick.vel_y != 0 || pick.vel_z != 0 {
                ui.label(format!(
                    "Vel: ({}, {}, {})",
                    pick.vel_x, pick.vel_y, pick.vel_z
                ));
            }

            let active = pick.flags & 1 != 0;
            let bonded = pick.flags & 4 != 0;
            if active || bonded {
                let mut flags_str = String::new();
                if active {
                    flags_str.push_str("active");
                }
                if bonded {
                    if !flags_str.is_empty() {
                        flags_str.push_str(", ");
                    }
                    flags_str.push_str("bonded");
                }
                ui.label(format!("Flags: {}", flags_str));
            }
        });
}
