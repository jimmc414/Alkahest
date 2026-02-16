use crate::app::SaveStatus;

/// Settings panel for cross-section, sim speed, render mode, and save/load.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ctx: &egui::Context,
    clip_axis: &mut u32,
    clip_position: &mut f32,
    sim_speed: &mut f32,
    render_mode: &mut u32,
    save_status: &mut SaveStatus,
    auto_save_enabled: &mut bool,
    rule_mismatch_warning: &mut Option<Vec<String>>,
    trigger_save: &mut bool,
    trigger_load: &mut bool,
    save_idle: bool,
    audio_enabled: &mut bool,
    audio_volume: &mut f32,
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

            ui.separator();

            // Save/Load section
            ui.label("World Persistence");
            ui.horizontal(|ui| {
                let save_enabled = save_idle;
                if ui
                    .add_enabled(save_enabled, egui::Button::new("Save"))
                    .clicked()
                {
                    *trigger_save = true;
                }
                let load_enabled = save_idle;
                if ui
                    .add_enabled(load_enabled, egui::Button::new("Load"))
                    .clicked()
                {
                    *trigger_load = true;
                }
            });

            ui.checkbox(auto_save_enabled, "Auto-save (5 min)");

            ui.separator();

            // Audio section
            ui.label("Audio");
            ui.checkbox(audio_enabled, "Enable Audio");
            if *audio_enabled {
                ui.add(egui::Slider::new(audio_volume, 0.0..=1.0).text("Volume"));
            }

            ui.separator();

            // Status indicator
            match save_status {
                SaveStatus::None => {}
                SaveStatus::Saving => {
                    ui.colored_label(egui::Color32::YELLOW, "Saving...");
                }
                SaveStatus::Saved => {
                    ui.colored_label(egui::Color32::GREEN, "Saved!");
                }
                SaveStatus::Loading => {
                    ui.colored_label(egui::Color32::YELLOW, "Loading...");
                }
                SaveStatus::Error(msg) => {
                    ui.colored_label(egui::Color32::RED, format!("Error: {msg}"));
                }
            }
        });

    // Rule mismatch warning dialog
    if let Some(warnings) = rule_mismatch_warning.as_ref() {
        let mut open = true;
        egui::Window::new("Rule Mismatch Warning")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("The loaded save was created with different rules:");
                for w in warnings {
                    ui.label(format!("  - {w}"));
                }
                ui.label("The world may behave differently than expected.");
                if ui.button("Dismiss").clicked() {
                    // Will be cleared below
                }
            });
        if !open {
            *rule_mismatch_warning = None;
        }
    }
}
