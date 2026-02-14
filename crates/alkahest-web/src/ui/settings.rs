/// Settings panel for cross-section, sim speed, and render mode.
pub fn show(
    ctx: &egui::Context,
    clip_axis: &mut u32,
    clip_position: &mut f32,
    sim_speed: &mut f32,
    render_mode: &mut u32,
) {
    egui::Window::new("Settings")
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 250.0))
        .resizable(false)
        .collapsible(true)
        .default_open(false)
        .show(ctx, |ui| {
            // Render mode
            ui.label("Render Mode");
            ui.horizontal(|ui| {
                if ui.selectable_label(*render_mode == 0, "Normal").clicked() {
                    *render_mode = 0;
                }
                if ui.selectable_label(*render_mode == 1, "Heatmap").clicked() {
                    *render_mode = 1;
                }
            });

            ui.separator();

            // Simulation speed
            ui.label("Sim Speed");
            ui.add(egui::Slider::new(sim_speed, 0.25..=4.0).text("x"));

            ui.separator();

            // Cross-section
            ui.label("Cross-Section");
            ui.horizontal(|ui| {
                if ui.selectable_label(*clip_axis == 0, "Off").clicked() {
                    *clip_axis = 0;
                }
                if ui.selectable_label(*clip_axis == 1, "X").clicked() {
                    *clip_axis = 1;
                }
                if ui.selectable_label(*clip_axis == 2, "Y").clicked() {
                    *clip_axis = 2;
                }
                if ui.selectable_label(*clip_axis == 3, "Z").clicked() {
                    *clip_axis = 3;
                }
            });

            if *clip_axis != 0 {
                let max_val = match *clip_axis {
                    1 => {
                        alkahest_core::constants::WORLD_CHUNKS_X
                            * alkahest_core::constants::CHUNK_SIZE
                    }
                    2 => {
                        alkahest_core::constants::WORLD_CHUNKS_Y
                            * alkahest_core::constants::CHUNK_SIZE
                    }
                    3 => {
                        alkahest_core::constants::WORLD_CHUNKS_Z
                            * alkahest_core::constants::CHUNK_SIZE
                    }
                    _ => 256,
                } as f32;
                ui.add(egui::Slider::new(clip_position, 0.0..=max_val).text("pos"));
            }
        });
}
